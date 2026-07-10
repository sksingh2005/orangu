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

//! Interactive `--init` flow that writes `~/.orangu/orangu-gguf.conf`.

use crate::prompt::prompt_bool;
use anyhow::{Context, Result, anyhow};
use rustyline::{
    Config, Context as RlContext, Editor, Helper,
    completion::{Completer, FilenameCompleter, Pair},
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    history::DefaultHistory,
    validate::Validator,
};
use std::path::PathBuf;

pub fn run_init() -> Result<()> {
    println!("orangu-gguf configuration");
    println!("=========================\n");

    let models = prompt_dir("models", huggingface_cache_dir().as_deref())?;

    let contents = format!("[orangu-gguf]\nmodels = {models}\n");

    println!("\nConfiguration to write:\n");
    println!("{contents}");

    if !prompt_bool("Write this configuration?", true)? {
        println!("Aborted. No changes written.");
        return Ok(());
    }

    let dir = home::home_dir()
        .context("failed to resolve home directory")?
        .join(".orangu");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create directory {}", dir.display()))?;
    let path = dir.join("orangu-gguf.conf");
    std::fs::write(&path, contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    println!("Wrote {}", path.display());

    Ok(())
}

/// By default, Hugging Face models are downloaded and cached in
/// `~/.cache/huggingface/hub` on Linux and macOS, or
/// `%USERPROFILE%\.cache\huggingface\hub` on Windows — the same directory
/// llama.cpp's own `-hf` falls back to when `LLAMA_CACHE`/`HF_HUB_CACHE`/etc.
/// aren't set. Offered as `--init`'s default `models` value so pointing
/// `orangu-gguf` at whatever's likely already there is just pressing Enter.
fn huggingface_cache_dir() -> Option<PathBuf> {
    Some(
        home::home_dir()?
            .join(".cache")
            .join("huggingface")
            .join("hub"),
    )
}

/// A rustyline helper that TAB-completes a typed path against the
/// filesystem exactly like a shell would, by delegating wholesale to
/// rustyline's own `FilenameCompleter`.
struct DirCompleter {
    inner: FilenameCompleter,
}

impl Completer for DirCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &RlContext<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.inner.complete(line, pos, ctx)
    }
}

impl Hinter for DirCompleter {
    type Hint = String;
}

impl Highlighter for DirCompleter {}
impl Validator for DirCompleter {}
impl Helper for DirCompleter {}

/// Prompts for the models directory, re-prompting on an empty entry with no
/// `default` (there is no sensible one) or a path that doesn't exist as a
/// directory. TAB-completes the typed path against the filesystem.
fn prompt_dir(label: &str, default: Option<&std::path::Path>) -> Result<String> {
    let default_display = default.map(|d| d.display().to_string());
    let config = Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut editor: Editor<DirCompleter, DefaultHistory> = Editor::with_config(config)?;
    editor.set_helper(Some(DirCompleter {
        inner: FilenameCompleter::new(),
    }));

    loop {
        let prompt_label = match &default_display {
            Some(d) => format!("{label} [{d}]: "),
            None => format!("{label} []: "),
        };
        let value = match editor.readline(&prompt_label) {
            Ok(line) => line.trim().to_string(),
            Err(ReadlineError::Eof | ReadlineError::Interrupted) => {
                return Err(anyhow!("aborted: reached end of input"));
            }
            Err(err) => return Err(err.into()),
        };
        let value = if value.is_empty() {
            match &default_display {
                Some(d) => d.clone(),
                None => {
                    println!("A value is required.");
                    continue;
                }
            }
        } else {
            value
        };
        let path = PathBuf::from(&value);
        if !path.is_dir() {
            println!("'{value}' does not exist or is not a directory.");
            if !prompt_bool("Use it anyway?", false)? {
                continue;
            }
        }
        return Ok(value);
    }
}
