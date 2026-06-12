// Copyright (C) 2026 The orangu community
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use anyhow::{Context, Result, anyhow};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use crate::commands::shell_words;
use orangu::tools::resolve_workspace_path;

/// Resolve the `$EDITOR` invocation for a workspace file into the program, its
/// leading arguments, and the absolute target path.
pub fn resolve_editor_command(
    workspace: &Path,
    raw_path: &str,
) -> Result<(String, Vec<String>, PathBuf)> {
    let editor = std::env::var("EDITOR").context("EDITOR is not set")?;
    let editor_parts = shell_words(&editor)?;
    let path = resolve_workspace_path(workspace, raw_path)?;
    let (program, args) = editor_parts
        .split_first()
        .ok_or_else(|| anyhow!("EDITOR is empty"))?;
    Ok((program.clone(), args.to_vec(), path))
}

/// Whether `$EDITOR` is a terminal editor that needs its own terminal window
/// (vim, nano, `emacs -nw`, …) as opposed to a GUI editor that opens its own
/// window (code, gvim, plain emacs, …).
pub fn editor_needs_terminal(program: &str, args: &[String]) -> bool {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program);
    let name = name
        .strip_suffix(".exe")
        .unwrap_or(name)
        .to_ascii_lowercase();

    const TERMINAL_EDITORS: &[&str] = &[
        "vi", "vim", "nvim", "nvi", "elvis", "vis", "nano", "pico", "micro", "helix", "hx", "kak",
        "kakoune", "joe", "jed", "ne", "mg", "ed",
    ];
    if TERMINAL_EDITORS.contains(&name.as_str()) {
        return true;
    }
    // emacs/emacsclient open a GUI window by default and only run in the
    // terminal when explicitly asked to.
    if name == "emacs" || name == "emacsclient" {
        return args
            .iter()
            .any(|arg| matches!(arg.as_str(), "-nw" | "-t" | "--tty" | "--no-window-system"));
    }
    false
}

/// Whether an executable named `name` exists on `$PATH`.
fn binary_on_path(name: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

/// The command prefix used to launch a terminal editor in its own window, e.g.
/// `["gnome-terminal", "--"]`. Uses the configured `terminal` command (a full
/// command, with flags) when non-empty, otherwise auto-detects a known emulator
/// on `$PATH`. The editor command is appended as separate arguments, and these
/// emulators close the window when that command exits.
fn terminal_launcher(configured: &str) -> Option<Vec<String>> {
    if !configured.trim().is_empty() {
        let parts = shell_words(configured).ok()?;
        if !parts.is_empty() {
            return Some(parts);
        }
    }

    // (binary, arguments that introduce the command to run)
    const TERMINALS: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["-e"]),
        ("ptyxis", &["--"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("kitty", &[]),
        ("alacritty", &["-e"]),
        ("wezterm", &["start", "--"]),
        ("foot", &[]),
        ("xfce4-terminal", &["-x"]),
        ("terminator", &["-x"]),
        ("st", &["-e"]),
        ("urxvt", &["-e"]),
        ("rxvt", &["-e"]),
        ("xterm", &["-e"]),
    ];
    for (binary, prefix) in TERMINALS {
        if binary_on_path(binary) {
            let mut launcher = vec![(*binary).to_string()];
            launcher.extend(prefix.iter().map(|arg| (*arg).to_string()));
            return Some(launcher);
        }
    }
    None
}

/// Build the full argument vector for opening `raw_path` in `$EDITOR`. Terminal
/// editors are wrapped in a terminal emulator (the configured `terminal`, or an
/// auto-detected one) so they get their own window; GUI editors are launched
/// directly.
fn editor_launch_argv(workspace: &Path, raw_path: &str, terminal: &str) -> Result<Vec<String>> {
    let (program, args, path) = resolve_editor_command(workspace, raw_path)?;

    let mut argv = Vec::new();
    if editor_needs_terminal(&program, &args) {
        let launcher = terminal_launcher(terminal).ok_or_else(|| {
            anyhow!(
                "no terminal emulator found to open '{program}' in a new window; \
                 set the `terminal` option in orangu.conf (e.g. \"xterm -e\") or use a GUI editor"
            )
        })?;
        argv.extend(launcher);
    }
    argv.push(program);
    argv.extend(args);
    argv.push(path.to_string_lossy().into_owned());
    Ok(argv)
}

/// Open `$EDITOR` on a workspace file in a separate window so orangu stays
/// usable. Terminal editors (vim, nano, `emacs -nw`, …) open in a new terminal
/// window (the configured `terminal` command, or an auto-detected emulator) that
/// closes when the editor exits; GUI editors open their own window. The process
/// is detached and not waited on.
pub fn open_in_editor(workspace: &Path, raw_path: &str, terminal: &str) -> Result<()> {
    let argv = editor_launch_argv(workspace, raw_path, terminal)?;
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| anyhow!("editor command is empty"))?;

    std::process::Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch editor '{program}'"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::EnvVarGuard;
    use crate::process_env_lock;
    use tempfile::tempdir;

    #[test]
    fn resolve_editor_command_handles_plain_terminal_editors() {
        let _env_lock = process_env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let workspace = tempdir().expect("workspace");

        for editor in ["vim", "vi", "nano", "emacs"] {
            let _editor = EnvVarGuard::set_value("EDITOR", editor);
            let (program, args, path) =
                resolve_editor_command(workspace.path(), "file.txt").expect("resolve");
            assert_eq!(program, editor);
            assert!(args.is_empty(), "unexpected args for {editor}: {args:?}");
            assert!(path.ends_with("file.txt"), "unexpected path: {path:?}");
        }
    }

    #[test]
    fn resolve_editor_command_splits_arguments() {
        let _env_lock = process_env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let workspace = tempdir().expect("workspace");

        let _editor = EnvVarGuard::set_value("EDITOR", "emacs -nw");
        let (program, args, _path) =
            resolve_editor_command(workspace.path(), "src/main.rs").expect("resolve");
        assert_eq!(program, "emacs");
        assert_eq!(args, vec!["-nw".to_string()]);
    }

    #[test]
    fn resolve_editor_command_rejects_empty_editor() {
        let _env_lock = process_env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let workspace = tempdir().expect("workspace");
        let _editor = EnvVarGuard::set_value("EDITOR", "");
        assert!(resolve_editor_command(workspace.path(), "file.txt").is_err());
    }

    #[test]
    fn editor_needs_terminal_classifies_editors() {
        // Terminal editors → need a window.
        for editor in [
            "vim",
            "vi",
            "nvim",
            "nano",
            "micro",
            "hx",
            "helix",
            "/usr/bin/vim",
        ] {
            assert!(
                editor_needs_terminal(editor, &[]),
                "{editor} should need a terminal"
            );
        }
        // GUI / self-windowing editors → launched directly.
        for editor in [
            "code",
            "codium",
            "subl",
            "gvim",
            "gedit",
            "emacs",
            "emacsclient",
        ] {
            assert!(
                !editor_needs_terminal(editor, &[]),
                "{editor} should not need a terminal"
            );
        }
        // emacs only counts as a terminal editor when explicitly asked.
        assert!(editor_needs_terminal("emacs", &["-nw".to_string()]));
        assert!(editor_needs_terminal("emacsclient", &["-t".to_string()]));
    }

    #[test]
    fn editor_launch_wraps_terminal_editors_and_passes_through_gui() {
        let _env_lock = process_env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let workspace = tempdir().expect("workspace");
        // Use a deterministic configured terminal launcher.
        let terminal = "myterm --run";

        // A terminal editor is wrapped: <launcher> <editor> <args> <path>.
        let _editor = EnvVarGuard::set_value("EDITOR", "vim");
        let argv = editor_launch_argv(workspace.path(), "file.txt", terminal).expect("argv");
        assert_eq!(argv[0], "myterm");
        assert_eq!(argv[1], "--run");
        assert_eq!(argv[2], "vim");
        assert!(argv.last().unwrap().ends_with("file.txt"));

        // A GUI editor is launched directly (no terminal launcher prefix).
        let _editor = EnvVarGuard::set_value("EDITOR", "code --wait");
        let argv = editor_launch_argv(workspace.path(), "file.txt", terminal).expect("argv");
        assert_eq!(argv[0], "code");
        assert_eq!(argv[1], "--wait");
        assert!(argv.last().unwrap().ends_with("file.txt"));
        assert!(!argv.contains(&"myterm".to_string()));
    }

    #[test]
    fn terminal_launcher_honors_configured_terminal() {
        assert_eq!(
            terminal_launcher("gnome-terminal --"),
            Some(vec!["gnome-terminal".to_string(), "--".to_string()])
        );
    }
}
