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

use std::sync::RwLock;

use crate::commands::{ISSUE_FIELDS, IssueField};
use crate::git::IssueMetadata;

/// The repository's candidate reviewers, assignees, and labels, fetched once at
/// startup (see `crate::git::fetch_issue_metadata`) and cached here so `/issue`
/// value completion needs no network call on every keystroke.
static ISSUE_METADATA: RwLock<IssueMetadata> = RwLock::new(IssueMetadata {
    reviewers: Vec::new(),
    assignees: Vec::new(),
    labels: Vec::new(),
});

/// Replace the cached `/issue` metadata. Called once the startup fetch finishes;
/// a poisoned lock is recovered rather than panicking, since a stale cache only
/// affects completion hints.
pub fn set_issue_metadata(metadata: IssueMetadata) {
    let mut guard = ISSUE_METADATA
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = metadata;
}

/// The cached completion values for a `/issue` field whose decimal/text spelling
/// starts with `token`: reviewers and assignees are logins, labels are names.
fn issue_value_candidates(field: IssueField, token: &str) -> Vec<String> {
    let guard = ISSUE_METADATA
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let list = match field {
        IssueField::Reviewer => &guard.reviewers,
        IssueField::Assignee => &guard.assignees,
        IssueField::Label => &guard.labels,
    };
    list.iter()
        .filter(|value| value.starts_with(token))
        .cloned()
        .collect()
}

/// Tab/ghost completion for `/issue <field> <number> <value>`, as
/// `(token_start, candidates)`:
///
/// - typing the **field** offers `reviewer`, `assignee`, `label`;
/// - typing the **number** offers nothing (it is typed directly);
/// - typing the **value** offers the cached reviewers / assignees / labels for
///   the chosen field.
///
/// So `/issue re` → `reviewer`, and `/issue reviewer 114 je` → the matching
/// logins. Returns `None` when `prefix` is not an `/issue` argument.
pub fn issue_completion_candidates(prefix: &str) -> Option<(usize, Vec<String>)> {
    let base = "/issue ".len();
    let after = prefix.strip_prefix("/issue ")?;

    // First token: the field. While it carries no trailing space it is still
    // being typed, so offer the field names that extend it.
    let Some(field_ws) = after.find(char::is_whitespace) else {
        let candidates = ISSUE_FIELDS
            .iter()
            .filter(|name| name.starts_with(after))
            .map(|name| (*name).to_string())
            .collect();
        return Some((base, candidates));
    };
    let field = &after[..field_ws];

    // Second token: the number. No completion — but it must be complete (have a
    // trailing space) before the value can be.
    let rest = &after[field_ws..];
    let number_lead = rest.len() - rest.trim_start().len();
    let after_field = rest.trim_start();
    if after_field.is_empty() {
        return None;
    }
    let number_ws = after_field.find(char::is_whitespace)?;

    // Third token onward: the value (the rest of the line, so multi-word labels
    // match as one). Offer the cached values for the chosen field.
    let field = IssueField::parse(field)?;
    let value_rel = &after_field[number_ws..];
    let value_lead = value_rel.len() - value_rel.trim_start().len();
    let value = value_rel.trim_start();
    let value_start = base + field_ws + number_lead + number_ws + value_lead;
    Some((value_start, issue_value_candidates(field, value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metadata() {
        set_issue_metadata(IssueMetadata {
            reviewers: vec!["jesperpedersen".to_string(), "alice".to_string()],
            assignees: vec!["bob".to_string(), "jesperpedersen".to_string()],
            labels: vec!["bug".to_string(), "needs triage".to_string()],
        });
    }

    #[test]
    fn completes_the_field_subcommand() {
        let (start, candidates) = issue_completion_candidates("/issue re").expect("candidates");
        assert_eq!(start, "/issue ".len());
        assert_eq!(candidates, vec!["reviewer".to_string()]);

        // No prefix offers all three fields.
        let (_, all) = issue_completion_candidates("/issue ").expect("candidates");
        assert_eq!(all, vec!["reviewer", "assignee", "label"]);
    }

    #[test]
    fn offers_nothing_while_typing_the_number() {
        // Mid-number: still typing the second token, so no candidates.
        assert!(issue_completion_candidates("/issue reviewer 11").is_none());
        // Field token complete but the number not yet started.
        assert!(issue_completion_candidates("/issue reviewer ").is_none());
    }

    #[test]
    fn completes_the_value_per_field() {
        sample_metadata();

        // Reviewers come from the reviewer list; the token start points at the
        // value so the accepted candidate replaces just `je`.
        let (start, candidates) =
            issue_completion_candidates("/issue reviewer 114 je").expect("candidates");
        assert_eq!(start, "/issue reviewer 114 ".len());
        assert_eq!(candidates, vec!["jesperpedersen".to_string()]);

        // Assignees come from the assignee list.
        let (_, assignees) =
            issue_completion_candidates("/issue assignee 114 ").expect("candidates");
        assert_eq!(assignees, vec!["bob", "jesperpedersen"]);

        // Labels can carry spaces and still match as one value.
        let (start, labels) =
            issue_completion_candidates("/issue label 5 needs").expect("candidates");
        assert_eq!(start, "/issue label 5 ".len());
        assert_eq!(labels, vec!["needs triage".to_string()]);

        set_issue_metadata(IssueMetadata::default());
    }

    #[test]
    fn ignores_non_issue_input() {
        assert!(issue_completion_candidates("/issuelike").is_none());
        assert!(issue_completion_candidates("/close 5").is_none());
    }
}
