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

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub struct CompressionStore {
    cache_dir: Option<PathBuf>,
}

impl CompressionStore {
    pub fn new(session_dir: Option<PathBuf>) -> Self {
        let cache_dir = session_dir.map(|dir| dir.join("compression_cache"));
        Self { cache_dir }
    }

    /// Stores the full content and returns a hash ID, or None if no cache directory is available.
    pub fn store(&self, content: &str) -> Option<String> {
        let dir = self.cache_dir.as_ref()?;
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!(
                "Failed to create compression cache dir {}: {}",
                dir.display(),
                e
            );
            return None;
        }

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let short_hash = &hash[..16]; // 16 chars is enough for uniqueness in a single session

        let path = dir.join(format!("{}.txt", short_hash));
        if let Err(e) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, content.as_bytes()))
            && e.kind() != std::io::ErrorKind::AlreadyExists
        {
            eprintln!(
                "Failed to write compression cache file {}: {}",
                path.display(),
                e
            );
            return None;
        }

        Some(short_hash.to_string())
    }

    /// Retrieves the content by its hash ID.
    pub fn retrieve(&self, id: &str) -> Result<String> {
        if !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(anyhow::anyhow!("Invalid cache ID format"));
        }
        let dir = self
            .cache_dir
            .as_ref()
            .context("No active session directory for cache")?;
        let path = dir.join(format!("{}.txt", id));
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "Context cache ID '{}' not found. It may have expired or never existed.",
                id
            ));
        }
        let content =
            std::fs::read_to_string(&path).context("Failed to read cached context file")?;
        Ok(content)
    }
}
