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

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Stable on-disk representation of a file's identity.
/// `SystemTime` is serialized as seconds-since-UNIX-EPOCH (u64) for
/// portability across session restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub size: u64,
    #[serde(with = "system_time_serde")]
    pub modified: SystemTime,
    pub hash: String,
}

/// Serde helper: serialize/deserialize `SystemTime` as a seconds-since-epoch u64.
mod system_time_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
        let secs = t
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        s.serialize_u64(secs)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SystemTime, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }
}

#[derive(Debug, Clone)]
pub enum CacheResult {
    Miss,
    Hit { fingerprint: String },
    Changed,
}

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub total_reads: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub bytes_saved: usize,
}

/// Per-session in-memory cache that can be persisted to and restored from
/// a `context-cache.json` sidecar in the session directory.
#[derive(Debug, Default, Clone)]
pub struct ContextCache {
    cache: HashMap<PathBuf, FileFingerprint>,
    stats: CacheStats,
}

impl ContextCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fingerprint(&self, content: &str, metadata: &Metadata) -> FileFingerprint {
        let size = metadata.len();
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        FileFingerprint {
            size,
            modified,
            hash,
        }
    }

    pub fn check_file(
        &mut self,
        path: &Path,
        content: &str,
        fingerprint: &FileFingerprint,
    ) -> CacheResult {
        self.stats.total_reads += 1;

        if let Some(cached) = self.cache.get(path) {
            if cached.size == fingerprint.size
                && cached.modified == fingerprint.modified
                && cached.hash == fingerprint.hash
            {
                self.stats.cache_hits += 1;
                self.stats.bytes_saved += content.len();
                return CacheResult::Hit {
                    fingerprint: fingerprint.hash.clone(),
                };
            } else {
                self.stats.cache_misses += 1;
                return CacheResult::Changed;
            }
        }

        self.stats.cache_misses += 1;
        CacheResult::Miss
    }

    pub fn record_read(&mut self, path: &Path, fingerprint: FileFingerprint) {
        self.cache.insert(path.to_path_buf(), fingerprint);
    }

    pub fn invalidate(&mut self, path: &Path) {
        self.cache.remove(path);
    }

    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.stats = CacheStats::default();
    }

    /// Persist the fingerprint cache to `path` (typically
    /// `<session_dir>/context-cache.json`). Silently no-ops if the cache is
    /// empty so we don't write useless sidecar files for offline sessions.
    pub fn persist(&self, path: &Path) -> std::io::Result<()> {
        if self.cache.is_empty() {
            return Ok(());
        }
        // Serialise as a JSON object mapping path-string → fingerprint.
        let serialisable: HashMap<String, &FileFingerprint> = self
            .cache
            .iter()
            .map(|(k, v)| (k.display().to_string(), v))
            .collect();
        let json = serde_json::to_string(&serialisable).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Restore a previously persisted fingerprint cache from `path`. Missing
    /// file is silently treated as an empty cache (first run). Parse errors
    /// are also silently ignored — a stale/corrupt sidecar just means the
    /// first reads of the session will be cache misses as normal.
    pub fn load(&mut self, path: &Path) {
        let Ok(json) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(map) = serde_json::from_str::<HashMap<String, FileFingerprint>>(&json) else {
            return;
        };
        for (k, v) in map {
            self.cache.insert(PathBuf::from(k), v);
        }
    }
}

pub fn format_cache_stub(path: &str, size: u64, fingerprint: &str) -> String {
    format!(
        "[cached] {path} is unchanged from its previous full read in this conversation ({} bytes, sha256:{fingerprint}). Reuse the earlier content already in context; call read_file with start_line/end_line if you need a fresh focused excerpt.",
        size
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_context_cache() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        let mut cache = ContextCache::new();

        // Create file
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "hello world").unwrap();
        }

        let content = fs::read_to_string(&file_path).unwrap();
        let metadata = fs::metadata(&file_path).unwrap();

        // First read should be Miss
        let fingerprint = cache.fingerprint(&content, &metadata);
        assert!(matches!(
            cache.check_file(&file_path, &content, &fingerprint),
            CacheResult::Miss
        ));
        cache.record_read(&file_path, fingerprint);

        // Second read should be Hit
        let fingerprint = cache.fingerprint(&content, &metadata);
        assert!(matches!(
            cache.check_file(&file_path, &content, &fingerprint),
            CacheResult::Hit { .. }
        ));

        // Modify file
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "hello world 2").unwrap();
        }

        let content2 = fs::read_to_string(&file_path).unwrap();
        let metadata2 = fs::metadata(&file_path).unwrap();

        // Read after modification should be Changed
        let fingerprint2 = cache.fingerprint(&content2, &metadata2);
        assert!(matches!(
            cache.check_file(&file_path, &content2, &fingerprint2),
            CacheResult::Changed
        ));
        cache.record_read(&file_path, fingerprint2);

        // Verify stats
        let stats = cache.stats();
        assert_eq!(stats.total_reads, 3);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 2);
    }
}
