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

use std::{fs, sync::RwLock};

use crate::commands::{COMMENT_AUTO_REVIEW_KEYWORD, COMMENT_REVIEW_KEYWORD, strip_ascii_prefix};
use crate::git::PullRequest;

/// Open pull/merge requests fetched once at startup (see
/// `crate::git::fetch_active_pull_requests`) and cached here in memory, so `/pull`
/// completion can offer numbers without shelling out to `gh`/`glab` on every
/// keystroke. Holds the request number paired with its title; only the number is
/// ever inserted into the prompt, the title is kept for future menu display.
static ACTIVE_PULL_REQUESTS: RwLock<Vec<(u64, String)>> = RwLock::new(Vec::new());

/// Replace the cached open pull/merge requests. Called once when the startup
/// fetch finishes; a poisoned lock is recovered rather than panicking, since a
/// stale cache is harmless.
pub fn set_active_pull_requests(requests: &[PullRequest]) {
    let mut guard = ACTIVE_PULL_REQUESTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = requests
        .iter()
        .map(|request| (request.number, request.title.clone()))
        .collect();
}

/// The cached open pull/merge request numbers whose decimal spelling starts with
/// `token`, as the strings `/pull` completion inserts. Numeric order, so the
/// lowest matching number is offered first.
pub(crate) fn pull_number_candidates(token: &str) -> Vec<String> {
    let guard = ACTIVE_PULL_REQUESTS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut numbers: Vec<u64> = guard
        .iter()
        .map(|(number, _)| *number)
        .filter(|number| number.to_string().starts_with(token))
        .collect();
    numbers.sort_unstable();
    numbers.iter().map(u64::to_string).collect()
}

/// For a `/pull <number>` argument (or its natural-language aliases `pull `,
/// `pull pr `, `pull request `, `pull #`), the offset where the number token
/// starts and the partial number typed so far. Mirrors the prefixes
/// `commands::parse_pull_pr_number` accepts, longest first so `pull request 5`
/// keeps `5` as the token rather than treating `request 5` as the number.
/// Returns `None` for anything else, including the bare `/pull` slash command
/// still being typed (no argument yet).
pub fn pull_completion_prefix(prefix: &str) -> Option<(usize, &str)> {
    if let Some(number) = prefix.strip_prefix("/pull ") {
        return Some(("/pull ".len(), number));
    }
    for command_prefix in ["pull request ", "pull pr ", "pull #", "pull "] {
        if let Some(number) = strip_ascii_prefix(prefix, command_prefix) {
            return Some((prefix.len() - number.len(), number));
        }
    }
    None
}

/// Whether the session currently holds a `/review` summary and an
/// `/auto_review` report. Gates the `/comment` report keywords: `with review`
/// and `with auto review` are only offered (completed and ghosted) once the
/// matching report exists. Set by the main loop whenever a review mode exits.
static AVAILABLE_REVIEW_REPORTS: RwLock<(bool, bool)> = RwLock::new((false, false));

/// Record which review reports the session holds (see
/// [`AVAILABLE_REVIEW_REPORTS`]). A poisoned lock is recovered rather than
/// panicking, since stale availability only affects hints.
pub fn set_available_review_reports(review: bool, auto_review: bool) {
    let mut guard = AVAILABLE_REVIEW_REPORTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = (review, auto_review);
}

/// The `/comment` report keywords whose report exists, in offer order.
pub(crate) fn available_report_keywords() -> Vec<&'static str> {
    let (review, auto_review) = *AVAILABLE_REVIEW_REPORTS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut keywords = Vec::new();
    if review {
        keywords.push(COMMENT_REVIEW_KEYWORD);
    }
    if auto_review {
        keywords.push(COMMENT_AUTO_REVIEW_KEYWORD);
    }
    keywords
}

/// Returns `(start, candidates)` for a comment command's `<number> <body-prefix>` argument
/// where the body argument is a bare word (no leading quote), completing against
/// `~/.orangu/comments/` plus the report keywords (`with review`, `with auto review`).
/// The template files come first so an existing template — even one starting with
/// `w` — keeps its completion (and ghost) priority; the keywords follow, offered
/// only once the matching report exists in the session (a missing directory does
/// not suppress them). Handles both `/comment` and the natural-language forms
/// (`add comment on`, `add comment to`, `comment on`).
pub fn comment_file_completion_candidates(prefix: &str) -> Option<(usize, Vec<String>)> {
    let rest = if let Some(rest) = prefix.strip_prefix("/comment ") {
        rest
    } else {
        let mut found = None;
        for command_prefix in ["add comment on ", "add comment to ", "comment on "] {
            if let Some(rest) = strip_ascii_prefix(prefix, command_prefix) {
                found = Some(rest);
                break;
            }
        }
        found?
    };
    let rest = rest.trim_start();
    // skip the issue number token
    let (_, after_number) = rest.split_once(char::is_whitespace)?;
    let file_prefix = after_number.trim_start();
    // quoted argument = inline comment body, not a file
    if file_prefix.starts_with('"') || file_prefix.starts_with('\'') {
        return None;
    }
    let mut candidates: Vec<String> = home::home_dir()
        .map(|home| home.join(".orangu/comments"))
        .and_then(|comments_dir| fs::read_dir(comments_dir).ok())
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    if !entry.file_type().ok()?.is_file() {
                        return None;
                    }
                    let name = entry.file_name().to_str()?.to_string();
                    name.starts_with(file_prefix).then_some(name)
                })
                .collect()
        })
        .unwrap_or_default();
    candidates.sort();
    candidates.extend(
        available_report_keywords()
            .into_iter()
            .filter(|keyword| keyword.starts_with(file_prefix))
            .map(str::to_string),
    );
    let start = prefix.len() - file_prefix.len();
    Some((start, candidates))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_completion_prefix_keeps_number_token_offset() {
        // The token offset must point at the number, not mid-command, so the
        // accepted candidate replaces just the `9` (e.g. `pull 9` -> `pull 90`)
        // rather than splicing the number into the middle of the command word.
        assert_eq!(pull_completion_prefix("/pull 9"), Some((6, "9")));
        assert_eq!(pull_completion_prefix("pull 9"), Some((5, "9")));
        assert_eq!(pull_completion_prefix("pull #9"), Some((6, "9")));
        assert_eq!(pull_completion_prefix("pull pr 9"), Some((8, "9")));
        assert_eq!(pull_completion_prefix("pull request 9"), Some((13, "9")));
        // Empty argument is offered (all numbers); bare slash command is not.
        assert_eq!(pull_completion_prefix("/pull "), Some((6, "")));
        assert_eq!(pull_completion_prefix("/pull"), None);
        assert_eq!(pull_completion_prefix("/pull_request"), None);
    }

    #[test]
    fn pull_number_candidates_filter_and_sort() {
        set_active_pull_requests(&[
            PullRequest {
                number: 90,
                title: "Add pull completion".to_string(),
            },
            PullRequest {
                number: 9,
                title: "Older".to_string(),
            },
            PullRequest {
                number: 58,
                title: "Fix rebase".to_string(),
            },
        ]);
        // `9` matches 9 and 90, numeric order; the candidate is the bare number.
        assert_eq!(pull_number_candidates("9"), vec!["9", "90"]);
        assert_eq!(pull_number_candidates("5"), vec!["58"]);
        assert!(pull_number_candidates("7").is_empty());
        // Empty token offers every cached number.
        assert_eq!(pull_number_candidates(""), vec!["9", "58", "90"]);
        set_active_pull_requests(&[]);
    }

    #[test]
    fn comment_completion_offers_report_keywords_after_templates() {
        // Without a stored report the keywords are ignored: only the template
        // files (if any) are offered.
        set_available_review_reports(false, false);
        let (_, candidates) =
            comment_file_completion_candidates("/comment 48 ").expect("candidates");
        assert!(
            !candidates.contains(&"with review".to_string()),
            "{candidates:?}"
        );
        assert!(
            !candidates.contains(&"with auto review".to_string()),
            "{candidates:?}"
        );

        // With both reports stored the keywords are offered — even when
        // `~/.orangu/comments/` does not exist — after any template files, so
        // a template (e.g. one starting with `w`) keeps its completion
        // priority.
        set_available_review_reports(true, true);
        let (start, candidates) =
            comment_file_completion_candidates("/comment 48 ").expect("candidates");
        assert_eq!(start, "/comment 48 ".len());
        let len = candidates.len();
        assert!(len >= 2, "{candidates:?}");
        assert_eq!(
            &candidates[len - 2..],
            &["with review".to_string(), "with auto review".to_string()],
            "keywords must come last: {candidates:?}"
        );

        // Typing narrows: `with a` leaves only the auto review keyword (plus
        // any template that genuinely shares the prefix).
        let (_, narrowed) =
            comment_file_completion_candidates("comment on 48 with a").expect("candidates");
        assert!(
            narrowed.contains(&"with auto review".to_string()),
            "{narrowed:?}"
        );
        assert!(
            !narrowed.contains(&"with review".to_string()),
            "{narrowed:?}"
        );

        // The natural-language form keeps the token offset at the body.
        let (start, _) =
            comment_file_completion_candidates("comment on 48 with a").expect("candidates");
        assert_eq!(start, "comment on 48 ".len());

        // Each keyword is gated by its own report.
        set_available_review_reports(false, true);
        let (_, partial) =
            comment_file_completion_candidates("/comment 48 with ").expect("candidates");
        assert!(
            partial.contains(&"with auto review".to_string()),
            "{partial:?}"
        );
        assert!(!partial.contains(&"with review".to_string()), "{partial:?}");

        // A quoted argument is an inline body — no candidates.
        assert!(comment_file_completion_candidates("/comment 48 \"w").is_none());
        set_available_review_reports(false, false);
    }
}
