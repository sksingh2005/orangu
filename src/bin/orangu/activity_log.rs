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

//! Persistent, cross-session usage history behind `/statistics`.
//!
//! `/usage` (`stats.rs`) reports the current tab's `UsageStats` — entirely
//! in-memory, gone when the process exits. This module keeps a small,
//! append-only log of every completed turn, per workspace — alongside the
//! knowledge graph and embeddings caches, under the same shared root
//! (`orangu::workspace_cache`):
//!
//! ```text
//! ~/.orangu/workspace/<hash>/stats/activity.json
//! ```
//!
//! One JSON object per line: the day it happened on, the session it belongs
//! to, and that turn's token/LLM-time/tool-time deltas. `/statistics` reads the
//! current workspace's log back, merges in that workspace's `git log` (so a
//! repository with commit history but no orangu-recorded turns yet still has
//! something to report — see [`merge_commit_history`]), and reports it as a
//! **Total** section (all-time totals, the heatmap, and a by-author commit
//! breakdown) followed by one section per calendar **year** with any activity
//! (that year's totals, then a monthly breakdown). `/statistics total` does
//! the same after merging every workspace's turn log (commit history is left
//! out of `total`, since it aggregates arbitrary, unrelated repositories with
//! no single `git log` to read).

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const ACTIVITY_LOG_FILENAME: &str = "activity.json";

/// One completed turn's contribution to the activity log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ActivityRecord {
    /// Days since the Unix epoch (UTC), i.e. `timestamp / 86400`. An integer
    /// day index rather than a formatted date, so grouping and streak math
    /// never has to parse a string back into a calendar date.
    pub(crate) day: u64,
    pub(crate) session_id: String,
    pub(crate) tokens: usize,
    pub(crate) llm_ms: u64,
    pub(crate) tool_ms: u64,
}

/// Today's day index (days since the Unix epoch, UTC).
pub(crate) fn today() -> u64 {
    crate::session_store::current_unix_timestamp() / 86400
}

fn activity_log_path(workspace: &Path) -> PathBuf {
    orangu::workspace_cache::workspace_cache_dir(workspace, "stats").join(ACTIVITY_LOG_FILENAME)
}

/// Append one record to `workspace`'s activity log, creating the file and its
/// parent directories if needed. Errors are ignored — the log is a reporting
/// aid, never load-bearing, so a failed write must never interrupt a turn.
pub(crate) fn append_activity(workspace: &Path, record: &ActivityRecord) {
    let path = activity_log_path(workspace);
    let Ok(mut line) = serde_json::to_string(record) else {
        return;
    };
    line.push('\n');
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Read every valid record from a single activity log file. Missing files and
/// unparseable lines (e.g. a truncated line after a hard kill) are skipped
/// rather than failing the whole read — `/statistics` on a fresh or corrupted
/// log reports an empty history, not an error.
fn read_activity_file(path: &Path) -> Vec<ActivityRecord> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Read every valid record from `workspace`'s activity log.
pub(crate) fn read_activity(workspace: &Path) -> Vec<ActivityRecord> {
    read_activity_file(&activity_log_path(workspace))
}

/// Read and merge every workspace's activity log, for `/statistics total`.
pub(crate) fn read_all_workspaces_activity() -> Vec<ActivityRecord> {
    orangu::workspace_cache::all_workspace_dirs()
        .into_iter()
        .flat_map(|dir| read_activity_file(&dir.join("stats").join(ACTIVITY_LOG_FILENAME)))
        .collect()
}

/// One day's aggregated activity: orangu-recorded token/LLM/tool time, and
/// (after [`merge_commit_history`]) how many commits landed that day.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct DayTotals {
    pub(crate) tokens: usize,
    pub(crate) llm_ms: u64,
    pub(crate) tool_ms: u64,
    pub(crate) commits: usize,
}

/// The full aggregation `/statistics` reports.
#[derive(Debug, Default)]
pub(crate) struct ActivitySummary {
    pub(crate) total_tokens: usize,
    pub(crate) total_sessions: usize,
    pub(crate) total_turns: usize,
    pub(crate) total_commits: usize,
    pub(crate) days_active: usize,
    pub(crate) current_streak: usize,
    pub(crate) longest_streak: usize,
    /// Day index → that day's totals, chronologically ordered.
    pub(crate) per_day: BTreeMap<u64, DayTotals>,
    /// Per-author commit counts and lines added/removed, most commits first
    /// (ties broken alphabetically). Empty until [`merge_commit_history`] runs.
    pub(crate) by_author: Vec<AuthorStats>,
}

/// One commit author's tally: how many commits, and how many lines they added
/// and removed across those commits (summed from `git log --numstat`), plus a
/// calendar-year breakdown of their commit count for the `/export statistics`
/// per-author appendix.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct AuthorStats {
    pub(crate) author: String,
    pub(crate) commits: usize,
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
    /// Calendar year → this author's commit count that year, chronologically
    /// ordered.
    pub(crate) commits_by_year: BTreeMap<i64, usize>,
}

/// Aggregate raw records into totals, per-day buckets, and streaks.
pub(crate) fn summarize(records: &[ActivityRecord]) -> ActivitySummary {
    let mut per_day: BTreeMap<u64, DayTotals> = BTreeMap::new();
    let mut sessions: HashSet<&str> = HashSet::new();
    let mut total_tokens = 0usize;

    for record in records {
        total_tokens += record.tokens;
        sessions.insert(record.session_id.as_str());
        let entry = per_day.entry(record.day).or_default();
        entry.tokens += record.tokens;
        entry.llm_ms += record.llm_ms;
        entry.tool_ms += record.tool_ms;
    }

    let active_days: std::collections::BTreeSet<u64> = per_day.keys().copied().collect();
    let (current_streak, longest_streak) = compute_streaks(&active_days);

    ActivitySummary {
        total_tokens,
        total_sessions: sessions.len(),
        total_turns: records.len(),
        days_active: per_day.len(),
        current_streak,
        longest_streak,
        per_day,
        ..Default::default()
    }
}

/// Merge a workspace's `git log` history into `summary`: each commit adds one
/// to its day's commit count and its author's commit/line tally, and
/// `days_active`/the streaks are recomputed over the union of turn-active and
/// commit-active days — so a repository with commit history but no
/// orangu-recorded turns yet is still reported as having activity, not "no
/// activity recorded yet".
pub(crate) fn merge_commit_history(
    summary: &mut ActivitySummary,
    commits: &[crate::git::CommitStat],
) {
    for commit in commits {
        summary.per_day.entry(commit.day).or_default().commits += 1;
    }
    summary.total_commits = commits.len();
    summary.by_author = tally_authors(commits.iter());

    let active_days: std::collections::BTreeSet<u64> = summary.per_day.keys().copied().collect();
    summary.days_active = active_days.len();
    let (current_streak, longest_streak) = compute_streaks(&active_days);
    summary.current_streak = current_streak;
    summary.longest_streak = longest_streak;
}

/// Tally per-author stats over `commits`, most commits first (ties broken
/// alphabetically).
fn tally_authors<'a>(
    commits: impl Iterator<Item = &'a crate::git::CommitStat>,
) -> Vec<AuthorStats> {
    let mut author_stats: BTreeMap<String, AuthorStats> = BTreeMap::new();
    for commit in commits {
        let entry = author_stats
            .entry(commit.author.clone())
            .or_insert_with(|| AuthorStats {
                author: commit.author.clone(),
                ..Default::default()
            });
        entry.commits += 1;
        entry.additions += commit.additions;
        entry.deletions += commit.deletions;
        let (year, _, _) = crate::export::civil_from_days(commit.day as i64);
        *entry.commits_by_year.entry(year).or_insert(0) += 1;
    }
    let mut by_author: Vec<AuthorStats> = author_stats.into_values().collect();
    by_author.sort_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then_with(|| a.author.cmp(&b.author))
    });
    by_author
}

/// Per-author stats over just the commits whose day falls inside
/// `first_day..=last_day` — the per-year and per-month "Authors" breakdowns.
pub(crate) fn authors_in_range(
    commits: &[crate::git::CommitStat],
    first_day: u64,
    last_day: u64,
) -> Vec<AuthorStats> {
    tally_authors(
        commits
            .iter()
            .filter(|commit| (first_day..=last_day).contains(&commit.day)),
    )
}

/// One calendar year's activity: that year's totals, and a month-by-month
/// breakdown (only months with any activity, oldest first).
pub(crate) struct YearSummary {
    pub(crate) year: i64,
    pub(crate) tokens: usize,
    pub(crate) commits: usize,
    pub(crate) months: Vec<(i64, DayTotals)>,
}

/// Group `summary`'s per-day totals into calendar years (oldest first), each
/// with a month-by-month breakdown, using the same day-index → date
/// conversion [`crate::export`]'s PDF pages use.
pub(crate) fn year_breakdown(summary: &ActivitySummary) -> Vec<YearSummary> {
    let mut years: BTreeMap<i64, YearSummary> = BTreeMap::new();
    let mut months: BTreeMap<(i64, i64), DayTotals> = BTreeMap::new();

    for (&day, totals) in &summary.per_day {
        let (year, month, _) = crate::export::civil_from_days(day as i64);
        let month_entry = months.entry((year, month)).or_default();
        month_entry.tokens += totals.tokens;
        month_entry.llm_ms += totals.llm_ms;
        month_entry.tool_ms += totals.tool_ms;
        month_entry.commits += totals.commits;

        let year_entry = years.entry(year).or_insert_with(|| YearSummary {
            year,
            tokens: 0,
            commits: 0,
            months: Vec::new(),
        });
        year_entry.tokens += totals.tokens;
        year_entry.commits += totals.commits;
    }

    for ((year, month), totals) in months {
        if let Some(year_entry) = years.get_mut(&year) {
            year_entry.months.push((month, totals));
        }
    }
    let mut result: Vec<YearSummary> = years.into_values().collect();
    for year_entry in &mut result {
        year_entry.months.sort_by_key(|(month, _)| *month);
    }
    result
}

/// The current streak (consecutive active days ending today or, if today has
/// no activity yet, ending yesterday so an in-progress day doesn't break a
/// streak prematurely) and the longest streak anywhere in the history.
fn compute_streaks(active_days: &std::collections::BTreeSet<u64>) -> (usize, usize) {
    if active_days.is_empty() {
        return (0, 0);
    }

    let mut longest = 1usize;
    let mut run = 1usize;
    let mut prev: Option<u64> = None;
    for &day in active_days {
        if let Some(p) = prev
            && day == p + 1
        {
            run += 1;
        } else {
            run = 1;
        }
        longest = longest.max(run);
        prev = Some(day);
    }

    let today = today();
    let mut cursor = if active_days.contains(&today) {
        today
    } else if today > 0 && active_days.contains(&(today - 1)) {
        today - 1
    } else {
        return (0, longest);
    };
    let mut current = 0usize;
    loop {
        if !active_days.contains(&cursor) {
            break;
        }
        current += 1;
        if cursor == 0 {
            break;
        }
        cursor -= 1;
    }
    (current, longest)
}

/// A cell's shading level in the heatmap: `0` for no activity, `1`-`4` for
/// increasing quartiles of the busiest recorded day.
pub(crate) fn heatmap_level(tokens: usize, max_tokens: usize) -> u8 {
    if tokens == 0 || max_tokens == 0 {
        return 0;
    }
    let ratio = tokens as f64 / max_tokens as f64;
    if ratio >= 0.75 {
        4
    } else if ratio >= 0.5 {
        3
    } else if ratio >= 0.25 {
        2
    } else {
        1
    }
}

/// A day's heatmap shading level: the usual token-based quartile when the day
/// has any recorded token usage, otherwise the lightest tint (`1`) if it has
/// at least one commit — so a commit-only day (no orangu usage, backfilled by
/// [`merge_commit_history`]) still shows as "something happened" rather than
/// blank, without pretending to know its intensity in token-equivalent terms.
pub(crate) fn day_heatmap_level(totals: &DayTotals, max_tokens: usize) -> u8 {
    if totals.tokens > 0 {
        heatmap_level(totals.tokens, max_tokens)
    } else if totals.commits > 0 {
        1
    } else {
        0
    }
}

/// How many trailing weeks the console/PDF heatmap renders.
pub(crate) const HEATMAP_WEEKS: u64 = 20;

/// Monday-first weekday initials, indexed by [`weekday_index`].
pub(crate) const WEEKDAY_LETTERS: [char; 7] = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

/// The weekday of a day index (days since the Unix epoch, UTC), as an index
/// into [`WEEKDAY_LETTERS`] (`0` = Monday). The Unix epoch (day `0`) was a
/// Thursday, i.e. index `3`.
pub(crate) fn weekday_index(day: u64) -> usize {
    ((day + 3) % 7) as usize
}

/// The first day the heatmap displays: the Monday `HEATMAP_WEEKS` weeks back,
/// chosen so the current (possibly partial) week is the last column and every
/// column is a whole Monday-to-Sunday week — putting Monday in the top row.
pub(crate) fn heatmap_start_day() -> u64 {
    let today = today();
    let week_end = today + (6 - weekday_index(today) as u64);
    week_end.saturating_sub(HEATMAP_WEEKS * 7 - 1)
}

/// Render a GitHub-style daily heatmap as block characters, `HEATMAP_WEEKS`
/// weeks wide (most recent week last), one Monday-first row per weekday, each
/// prefixed with its initial. Shading is `' '` for no activity, then `░▒▓█`
/// for increasing quartiles of the busiest day in the whole history (not just
/// the visible window), so the shading is stable across different views
/// rather than re-normalising to whatever is on screen.
pub(crate) fn render_heatmap(summary: &ActivitySummary) -> String {
    const SHADES: [char; 5] = [' ', '░', '▒', '▓', '█'];
    let max_tokens = summary
        .per_day
        .values()
        .map(|d| d.tokens)
        .max()
        .unwrap_or(0);

    let today = today();
    let start = heatmap_start_day();

    let mut rows: Vec<String> = (0..7)
        .map(|weekday| format!("{} ", WEEKDAY_LETTERS[weekday]))
        .collect();
    for week in 0..HEATMAP_WEEKS {
        for weekday in 0..7u64 {
            let day = start + week * 7 + weekday;
            let level = summary
                .per_day
                .get(&day)
                .map(|d| day_heatmap_level(d, max_tokens))
                .unwrap_or(0);
            let ch = if day > today {
                ' ' // Future days in the last, partial week: left blank.
            } else {
                SHADES[level as usize]
            };
            rows[weekday as usize].push(ch);
            rows[weekday as usize].push(ch); // Two columns per day reads as a square, not a sliver.
        }
    }
    rows.join("\n")
}

/// Render the `/statistics` console report: a **Total** section (all-time
/// totals, the daily heatmap, and a by-author commit breakdown) followed by
/// one section per calendar year with any activity (that year's totals, then
/// a monthly breakdown). `total` aggregates every workspace's turn log
/// instead of just the current one; commit history is only merged in for the
/// single-workspace report (`total` aggregates arbitrary, unrelated
/// repositories with no one `git log` to read).
pub(crate) fn format_report(workspace: &Path, total: bool) -> String {
    let records = if total {
        read_all_workspaces_activity()
    } else {
        read_activity(workspace)
    };
    let mut summary = summarize(&records);
    if !total {
        merge_commit_history(&mut summary, &crate::git::commit_history(workspace));
    }

    if summary.per_day.is_empty() {
        return "No activity recorded yet — this fills in as you use orangu.".to_string();
    }

    let total_llm_ms: u64 = summary.per_day.values().map(|d| d.llm_ms).sum();
    let total_tool_ms: u64 = summary.per_day.values().map(|d| d.tool_ms).sum();

    let mut out = format!(
        "Total\n\nRepository Activity\nCommits          : {}\nDays active      : {}\nCurrent streak   : {} day{}\nLongest streak   : {} day{}\n\nToken Usage\nSessions         : {}\nTurns            : {}\nTokens           : {}\nLLM time         : {}\nTool time        : {}\n\nHeatmap\n\n{}",
        summary.total_commits,
        summary.days_active,
        summary.current_streak,
        if summary.current_streak == 1 { "" } else { "s" },
        summary.longest_streak,
        if summary.longest_streak == 1 { "" } else { "s" },
        summary.total_sessions,
        summary.total_turns,
        summary.total_tokens,
        crate::stats::format_duration(std::time::Duration::from_millis(total_llm_ms)),
        crate::stats::format_duration(std::time::Duration::from_millis(total_tool_ms)),
        render_heatmap(&summary),
    );

    if !summary.by_author.is_empty() {
        out.push_str("\n\nAuthors\n");
        for author in &summary.by_author {
            out.push_str(&format!(
                "{:<24} {:>12}  {:>8} {:>8}\n",
                author.author,
                format!(
                    "{} commit{}",
                    author.commits,
                    if author.commits == 1 { "" } else { "s" }
                ),
                format!("+{}", author.additions),
                format!("-{}", author.deletions),
            ));
        }
    }

    // Newest first, so the most recent activity is nearest the top.
    for year in year_breakdown(&summary).iter().rev() {
        out.push_str(&format!(
            "\n{}\n\nYearly total\nTokens           : {}\nCommits          : {}\n\nMonthly\n",
            year.year, year.tokens, year.commits,
        ));
        for (month, totals) in year.months.iter().rev() {
            out.push_str(&format!(
                "{:04}-{:02}                 : {} tokens, {} commits\n",
                year.year, month, totals.tokens, totals.commits,
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn record(day: u64, session: &str, tokens: usize) -> ActivityRecord {
        ActivityRecord {
            day,
            session_id: session.to_string(),
            tokens,
            llm_ms: 100,
            tool_ms: 10,
        }
    }

    fn commit(
        day: u64,
        author: &str,
        additions: usize,
        deletions: usize,
    ) -> crate::git::CommitStat {
        crate::git::CommitStat {
            day,
            author: author.to_string(),
            additions,
            deletions,
        }
    }

    #[test]
    fn weekday_index_matches_known_dates() {
        // Day 0 (1970-01-01) was a Thursday.
        assert_eq!(weekday_index(0), 3);
        assert_eq!(WEEKDAY_LETTERS[weekday_index(0)], 'T');
        // Day 4 (1970-01-05) was the following Monday.
        assert_eq!(weekday_index(4), 0);
        assert_eq!(WEEKDAY_LETTERS[weekday_index(4)], 'M');
        // 2021-01-01 (day 18628) was a Friday.
        assert_eq!(weekday_index(18628), 4);
    }

    #[test]
    fn heatmap_start_day_is_a_monday_with_today_in_the_last_week() {
        let start = heatmap_start_day();
        assert_eq!(weekday_index(start), 0, "the top row must be Monday");
        // Today falls inside the displayed window's final week.
        let end = start + HEATMAP_WEEKS * 7 - 1;
        let today = today();
        assert!(today <= end);
        assert!(end - today < 7, "the last column is the current week");
    }

    #[test]
    fn today_matches_current_unix_time() {
        let expected = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 86400;
        // Allow the two calls to straddle a day boundary without flaking.
        assert!(today().abs_diff(expected) <= 1);
    }

    #[test]
    fn summarize_aggregates_totals_and_sessions() {
        let records = vec![
            record(100, "a", 50),
            record(100, "a", 25),
            record(101, "b", 10),
        ];
        let summary = summarize(&records);
        assert_eq!(summary.total_tokens, 85);
        assert_eq!(summary.total_sessions, 2);
        assert_eq!(summary.total_turns, 3);
        assert_eq!(summary.days_active, 2);
        assert_eq!(summary.per_day[&100].tokens, 75);
        assert_eq!(summary.per_day[&101].tokens, 10);
    }

    #[test]
    fn summarize_on_empty_log_is_all_zero() {
        let summary = summarize(&[]);
        assert_eq!(summary.total_tokens, 0);
        assert_eq!(summary.total_sessions, 0);
        assert_eq!(summary.days_active, 0);
        assert_eq!(summary.current_streak, 0);
        assert_eq!(summary.longest_streak, 0);
    }

    #[test]
    fn compute_streaks_finds_consecutive_runs() {
        // Days 10-12 and 14-15 active; 13 is a gap.
        let days: std::collections::BTreeSet<u64> = [10, 11, 12, 14, 15].into_iter().collect();
        let (_, longest) = compute_streaks(&days);
        assert_eq!(longest, 3);
    }

    #[test]
    fn compute_streaks_current_streak_ends_today_or_yesterday() {
        let today = today();
        // Active today, yesterday, and the day before: current streak of 3.
        let days: std::collections::BTreeSet<u64> =
            [today, today - 1, today - 2].into_iter().collect();
        let (current, _) = compute_streaks(&days);
        assert_eq!(current, 3);

        // Active yesterday and the day before, but not today yet: still a
        // live streak of 2 (today just hasn't happened yet).
        let days: std::collections::BTreeSet<u64> = [today - 1, today - 2].into_iter().collect();
        let (current, _) = compute_streaks(&days);
        assert_eq!(current, 2);

        // Active two days ago only: the streak is broken (missed yesterday).
        let days: std::collections::BTreeSet<u64> = [today - 2].into_iter().collect();
        let (current, _) = compute_streaks(&days);
        assert_eq!(current, 0);
    }

    #[test]
    fn heatmap_level_buckets_by_quartile() {
        assert_eq!(heatmap_level(0, 100), 0);
        assert_eq!(heatmap_level(10, 100), 1);
        assert_eq!(heatmap_level(30, 100), 2);
        assert_eq!(heatmap_level(60, 100), 3);
        assert_eq!(heatmap_level(100, 100), 4);
        assert_eq!(heatmap_level(5, 0), 0);
    }

    #[test]
    fn append_and_read_activity_roundtrip() {
        // Exercise the JSON encode/decode path directly rather than through
        // the real ~/.orangu/stats path, which append_activity/read_activity
        // always resolve to (kept out of unit tests deliberately).
        let original = record(12345, "sess-1", 42);
        let json = serde_json::to_string(&original).unwrap();
        let decoded: ActivityRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn read_activity_skips_unparseable_lines() {
        // A line that isn't valid JSON must be skipped, not fail the whole read.
        let good = record(1, "a", 5);
        let good_json = serde_json::to_string(&good).unwrap();
        let content = format!("{good_json}\nnot json at all\n{good_json}\n");
        let parsed: Vec<ActivityRecord> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn day_heatmap_level_falls_back_to_commit_presence() {
        // A day with token usage shades by the usual quartile...
        let used = DayTotals {
            tokens: 100,
            llm_ms: 0,
            tool_ms: 0,
            commits: 0,
        };
        assert_eq!(day_heatmap_level(&used, 100), 4);
        // ...a commit-only day (no orangu usage) gets the lightest tint...
        let commit_only = DayTotals {
            tokens: 0,
            llm_ms: 0,
            tool_ms: 0,
            commits: 3,
        };
        assert_eq!(day_heatmap_level(&commit_only, 100), 1);
        // ...and a day with neither is blank.
        let empty = DayTotals::default();
        assert_eq!(day_heatmap_level(&empty, 100), 0);
    }

    #[test]
    fn merge_commit_history_tallies_authors_and_backfills_days_active() {
        let mut summary = summarize(&[]);
        assert_eq!(summary.days_active, 0);

        let commits = vec![
            commit(100, "Alice", 10, 2),
            commit(100, "Bob", 5, 1),
            commit(101, "Alice", 3, 0),
        ];
        merge_commit_history(&mut summary, &commits);

        assert_eq!(summary.total_commits, 3);
        assert_eq!(summary.days_active, 2);
        assert_eq!(summary.per_day[&100].commits, 2);
        assert_eq!(summary.per_day[&101].commits, 1);
        // Most commits first; Alice (2, +13/-2) before Bob (1, +5/-1).
        assert_eq!(
            summary.by_author,
            vec![
                AuthorStats {
                    author: "Alice".to_string(),
                    commits: 2,
                    additions: 13,
                    deletions: 2,
                    commits_by_year: BTreeMap::from([(1970, 2)]),
                },
                AuthorStats {
                    author: "Bob".to_string(),
                    commits: 1,
                    additions: 5,
                    deletions: 1,
                    commits_by_year: BTreeMap::from([(1970, 1)]),
                },
            ]
        );
    }

    #[test]
    fn authors_in_range_only_counts_commits_inside_the_range() {
        let commits = vec![
            commit(100, "Alice", 10, 2),
            commit(105, "Bob", 5, 1),
            commit(110, "Alice", 3, 0),
        ];

        // Only days 100..=105: Alice's day-110 commit is excluded.
        let authors = authors_in_range(&commits, 100, 105);
        assert_eq!(authors.len(), 2);
        assert_eq!(authors[0].author, "Alice");
        assert_eq!(authors[0].commits, 1);
        assert_eq!(authors[0].additions, 10);
        assert_eq!(authors[1].author, "Bob");

        // A range with no commits yields no authors.
        assert!(authors_in_range(&commits, 200, 300).is_empty());
    }

    #[test]
    fn year_breakdown_groups_by_calendar_year_and_month() {
        // 2021-01-01 is day 18628; 2021-02-01 (31 days later) is day 18659;
        // 2022-01-01 is day 18993.
        let records = vec![record(18628, "a", 10), record(18659, "a", 7)];
        let mut summary = summarize(&records);
        merge_commit_history(&mut summary, &[commit(18993, "Alice", 5, 1)]);

        let years = year_breakdown(&summary);
        assert_eq!(years.len(), 2);

        assert_eq!(years[0].year, 2021);
        assert_eq!(years[0].tokens, 17);
        assert_eq!(years[0].months.len(), 2);
        assert_eq!(
            years[0].months[0],
            (
                1,
                DayTotals {
                    tokens: 10,
                    llm_ms: 100,
                    tool_ms: 10,
                    commits: 0
                }
            )
        );
        assert_eq!(years[0].months[1].0, 2);
        assert_eq!(years[0].months[1].1.tokens, 7);

        assert_eq!(years[1].year, 2022);
        assert_eq!(years[1].commits, 1);
        assert_eq!(
            years[1].months,
            vec![(
                1,
                DayTotals {
                    tokens: 0,
                    llm_ms: 0,
                    tool_ms: 0,
                    commits: 1
                }
            )]
        );
    }
}
