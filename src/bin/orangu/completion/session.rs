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

use std::{fs, path::PathBuf};

pub(crate) fn session_uuids_newest_first() -> Vec<String> {
    let Some(home) = home::home_dir() else {
        return Vec::new();
    };
    let sessions_dir = home.join(".orangu/sessions");
    let Ok(entries) = std::fs::read_dir(&sessions_dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<(String, u64)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let mtime = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Some((name, mtime))
        })
        .collect();
    dirs.sort_by_key(|e| std::cmp::Reverse(e.1));
    dirs.into_iter().map(|(name, _)| name).collect()
}

/// The distinct workspace paths recorded across all sessions, ordered by the
/// most recently updated session that used each one, newest first. Drives the
/// workspace half of `/session <arg>` completion; empty workspaces (sessions
/// started outside a Git repository) are skipped.
pub(crate) fn session_workspaces_newest_first() -> Vec<String> {
    let Some(home) = home::home_dir() else {
        return Vec::new();
    };
    let sessions_dir = home.join(".orangu/sessions");
    let Ok(entries) = std::fs::read_dir(&sessions_dir) else {
        return Vec::new();
    };
    let mut rows: Vec<(String, u64)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let meta = crate::load_session_metadata(&e.path().join("metadata"))
                .ok()
                .flatten()?;
            if meta.workspace.is_empty() {
                return None;
            }
            Some((meta.workspace, meta.last_updated_at))
        })
        .collect();
    rows.sort_by_key(|(_, updated)| std::cmp::Reverse(*updated));
    let mut workspaces: Vec<String> = Vec::new();
    for (workspace, _) in rows {
        if !workspaces.contains(&workspace) {
            workspaces.push(workspace);
        }
    }
    workspaces
}

/// Filesystem directory completion for a `/session <path>` argument, used as a
/// fallback when the typed text matches no session UUID or known workspace, so a
/// new workspace can be navigated to. Only fires for path-like input (starting
/// with `~`, `/`, or `.`, or containing a `/`). A leading `~`/`~/` is expanded to
/// the home directory for the lookup but kept verbatim in the returned
/// candidates. Only directories are offered, since a workspace is always a
/// directory; candidates carry no trailing slash, matching how the user types
/// the next `/` segment themselves.
pub(crate) fn session_path_completion_candidates(arg: &str) -> Vec<String> {
    let looks_like_path =
        arg.starts_with('~') || arg.starts_with('/') || arg.starts_with('.') || arg.contains('/');
    if !looks_like_path {
        return Vec::new();
    }
    // Split into the directory portion already typed (kept verbatim in each
    // candidate) and the partial final segment being completed.
    let split = arg.rfind('/').map(|i| i + 1).unwrap_or(0);
    let (typed_dir, partial) = arg.split_at(split);
    let Ok(entries) = fs::read_dir(expand_tilde_dir(typed_dir)) else {
        return Vec::new();
    };
    let mut candidates: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            name.starts_with(partial)
                .then(|| format!("{typed_dir}{name}"))
        })
        .collect();
    candidates.sort();
    candidates
}

/// The real filesystem directory to scan for the already-typed directory portion
/// of a `/session` path argument, expanding a leading `~`/`~/` to the home
/// directory. An empty portion (the argument has no `/` yet) scans the current
/// directory.
pub(crate) fn expand_tilde_dir(typed_dir: &str) -> PathBuf {
    if typed_dir == "~" || typed_dir == "~/" {
        return home::home_dir().unwrap_or_else(|| PathBuf::from(typed_dir));
    }
    if let Some(rest) = typed_dir.strip_prefix("~/")
        && let Some(home) = home::home_dir()
    {
        return home.join(rest);
    }
    if typed_dir.is_empty() {
        return PathBuf::from(".");
    }
    PathBuf::from(typed_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn session_path_completion_lists_matching_subdirectories() {
        let root = tempdir().expect("tempdir");
        let base = root.path();
        std::fs::create_dir(base.join("PostgreSQL")).expect("dir");
        std::fs::create_dir(base.join("Postfix")).expect("dir");
        std::fs::create_dir(base.join("Redis")).expect("dir");
        std::fs::write(base.join("Postscript.txt"), b"x").expect("file");

        // The typed directory portion is kept verbatim; only directories whose
        // name extends the partial segment are offered, sorted, with no trailing
        // slash. The plain file is skipped.
        let prefix = format!("{}/Post", base.display());
        let candidates = session_path_completion_candidates(&prefix);
        assert_eq!(
            candidates,
            vec![
                format!("{}/Postfix", base.display()),
                format!("{}/PostgreSQL", base.display()),
            ]
        );
    }

    #[test]
    fn session_path_completion_ignores_non_path_arguments() {
        // A bare token (no separators, not `~`/`/`/`.`) is a UUID/workspace
        // prefix, not a path, so filesystem completion stays out of the way.
        assert!(session_path_completion_candidates("Postgre").is_empty());
    }
}
