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

//! Configuration for `orangu-gguf`: a single `[orangu-gguf]` section naming
//! the directory `list`/`show` scan for `.gguf` files, mirroring the shape of
//! `orangu.conf` and `orangu-coordinator.conf`.

use anyhow::{Context, Result, anyhow};
use orangu::config::parse_ini_sections;
use std::path::{Path, PathBuf};

pub const CLIENT_SECTION: &str = "orangu-gguf";

#[derive(Clone, Debug)]
pub struct GgufConfiguration {
    /// Directory `list`/`show` scan (recursively) for `.gguf` files.
    pub models: PathBuf,
}

/// Expands a leading `~` or `~/` to the user's home directory. Config values
/// are otherwise taken literally, but a models directory is the one place a
/// user is likely to type a `~`-relative path, same as a shell would accept.
fn expand_tilde(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) => match home::home_dir() {
            Some(home) => home.join(rest.trim_start_matches('/')),
            None => PathBuf::from(path),
        },
        None => PathBuf::from(path),
    }
}

pub fn default_gguf_config_path() -> Option<PathBuf> {
    let cwd_path = std::env::current_dir().ok()?.join("orangu-gguf.conf");
    if cwd_path.exists() {
        return Some(cwd_path);
    }

    let config_path = home::home_dir()?.join(".orangu/orangu-gguf.conf");
    config_path.exists().then_some(config_path)
}

pub fn load_gguf_configuration(path: &Path) -> Result<GgufConfiguration> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read configuration {}", path.display()))?;
    let mut sections = parse_ini_sections(&contents)
        .with_context(|| format!("failed to parse configuration {}", path.display()))?;

    let client = sections.remove(CLIENT_SECTION).unwrap_or_default();

    let models = client
        .get("models")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("[{CLIENT_SECTION}].models must be set to a models directory"))?;

    Ok(GgufConfiguration {
        models: expand_tilde(&models),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn loads_models_directory() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "[orangu-gguf]\nmodels = /srv/models\n").unwrap();

        let conf = load_gguf_configuration(file.path()).unwrap();
        assert_eq!(conf.models, PathBuf::from("/srv/models"));
    }

    #[test]
    fn requires_models_key() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "[orangu-gguf]\n").unwrap();

        let err = load_gguf_configuration(file.path()).unwrap_err();
        assert!(
            err.to_string().contains("models"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn rejects_blank_models_value() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "[orangu-gguf]\nmodels =    \n").unwrap();

        let err = load_gguf_configuration(file.path()).unwrap_err();
        assert!(
            err.to_string().contains("models"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn expands_leading_tilde() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "[orangu-gguf]\nmodels = ~/models\n").unwrap();

        let conf = load_gguf_configuration(file.path()).unwrap();
        let home = home::home_dir().unwrap();
        assert_eq!(conf.models, home.join("models"));
    }
}
