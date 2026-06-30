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

use crate::diff::compress_git_diff;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, Default)]
pub struct WorldState {
    pub open_files: Vec<PathBuf>,
    pub env_vars: HashMap<String, String>,
}

impl WorldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diff(&self, previous: &WorldState) -> WorldStateDiff {
        let mut added_files = Vec::new();
        let mut removed_files = Vec::new();

        for file in &self.open_files {
            if !previous.open_files.contains(file) {
                added_files.push(file.clone());
            }
        }

        for file in &previous.open_files {
            if !self.open_files.contains(file) {
                removed_files.push(file.clone());
            }
        }

        let mut changed_env = HashMap::new();
        for (k, v) in &self.env_vars {
            if previous.env_vars.get(k) != Some(v) {
                changed_env.insert(k.clone(), v.clone());
            }
        }

        WorldStateDiff {
            added_files,
            removed_files,
            changed_env,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldStateDiff {
    pub added_files: Vec<PathBuf>,
    pub removed_files: Vec<PathBuf>,
    pub changed_env: HashMap<String, String>,
}

pub async fn get_current_workspace_diff(
    workspace_path: &Path,
    diff_file_cap: usize,
) -> Option<(u64, String)> {
    // 1. Get tracked changes
    let tracked_output = Command::new("git")
        .args(["diff", "HEAD", "-M"])
        .current_dir(workspace_path)
        .output()
        .await
        .ok()?;
    let tracked_diff = String::from_utf8_lossy(&tracked_output.stdout).to_string();

    // 2. Get untracked files
    let untracked_output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(workspace_path)
        .output()
        .await
        .ok()?;
    let untracked_files = String::from_utf8_lossy(&untracked_output.stdout);

    let mut untracked_diff = String::new();
    for file in untracked_files.lines() {
        let file = file.trim();
        if file.is_empty() {
            continue;
        }
        let file_path = workspace_path.join(file);
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            let line_count = content.lines().count();
            untracked_diff.push_str("--- /dev/null\n");
            untracked_diff.push_str(&format!("+++ b/{}\n", file));
            untracked_diff.push_str(&format!("@@ -0,0 +1,{} @@\n", line_count));
            for line in content.lines() {
                untracked_diff.push_str(&format!("+{}\n", line));
            }
        }
    }

    let combined_diff = format!("{}{}", tracked_diff, untracked_diff);
    if combined_diff.trim().is_empty() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(combined_diff.as_bytes());
    let hash_array = hasher.finalize();
    // We only need a fast equality check, so folding into a u64 is fine
    let hash = u64::from_le_bytes(hash_array[0..8].try_into().unwrap());

    let final_diff = if combined_diff.len() > 500 * 1024 {
        // Massive diff, fallback to summary
        let summary_output = Command::new("git")
            .args(["diff", "--compact-summary", "HEAD"])
            .current_dir(workspace_path)
            .output()
            .await
            .ok()?;
        let mut summary = String::from_utf8_lossy(&summary_output.stdout).to_string();
        if !untracked_diff.is_empty() {
            summary.push_str("\nUntracked files:\n");
            summary.push_str(&untracked_files);
        }
        summary
    } else {
        compress_git_diff(&combined_diff, diff_file_cap)
    };

    Some((hash, final_diff))
}
