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

//! Shared line-based stdin prompt helpers, used by both `--init` and the
//! interactive role wizard (bare `orangu-gguf` invocation).

use anyhow::{Context, Result, anyhow};
use std::io::{self, Write};

/// Read a line from stdin after printing `label`. A closed stdin (EOF, e.g.
/// Ctrl-D) is reported as an error rather than an empty line, so callers
/// abort instead of looping forever or silently accepting every default.
pub fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut line = String::new();
    let read = io::stdin()
        .read_line(&mut line)
        .context("failed to read from standard input")?;
    if read == 0 {
        return Err(anyhow!("aborted: reached end of input"));
    }
    Ok(line.trim().to_string())
}

/// Prompt for a Yes/No value, accepting `Yes`/`Y`/`No`/`N` case-insensitively.
/// An empty entry keeps `default`.
pub fn prompt_bool(label: &str, default: bool) -> Result<bool> {
    let default_label = if default { "Yes" } else { "No" };
    loop {
        let value = prompt(&format!("{label} (Yes/No) [{default_label}]: "))?;
        if value.is_empty() {
            return Ok(default);
        }
        match value.to_lowercase().as_str() {
            "yes" | "y" => return Ok(true),
            "no" | "n" => return Ok(false),
            _ => println!("Please answer Yes/Y or No/N."),
        }
    }
}
