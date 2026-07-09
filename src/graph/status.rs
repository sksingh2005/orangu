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

/// Build status of a workspace's knowledge graph — the background scan that
/// powers `graph_lookup`, `/graph`, and Deep `/auto_review`'s cross-file
/// context. A caller holds this behind an `Arc<Mutex<GraphBuildStatus>>`
/// (see `ToolExecutor::graph_status`) alongside the graph itself
/// (`ToolExecutor::graph_store`), updating it from `Building` once when the
/// scan starts, then to `Ready` or `Failed` once it ends.
///
/// This is a UI-facing signal, not a correctness gate: every graph query
/// already tolerates a graph that isn't built yet (e.g.
/// `auto_review_graph_context` and the `graph_lookup` tool both treat a
/// `None` store as "nothing found" rather than an error) — `GraphBuildStatus`
/// exists so that behavior can be *surfaced* (a status-bar dot, a warning)
/// instead of silently read as "no results."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphBuildStatus {
    /// The scan hasn't finished yet: every graph query comes back empty.
    #[default]
    Building,
    /// The scan completed and the graph is populated.
    Ready,
    /// The scan task itself failed (panicked or was cancelled) — the graph
    /// will stay empty for the rest of the session.
    Failed,
}
