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

use petgraph::algo::is_cyclic_directed;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::extract::{Confidence, ExtractedEdge, ExtractedNode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub source_location: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub relation: String,
    pub confidence: Confidence,
}

/// The central in-memory knowledge graph.
///
/// - Backed by a `petgraph::DiGraph` for O(1) traversal.
/// - A companion `HashMap<id → NodeIndex>` provides O(1) lookup by symbol id.
/// - Deduplication: inserting a node whose id already exists *overwrites* the
///   existing entry (semantic nodes win over structural ones).
#[derive(Debug)]
pub struct GraphStore {
    graph: StableDiGraph<GraphNode, GraphEdge>,
    node_map: HashMap<String, NodeIndex>,
}

impl Default for GraphStore {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStore {
    pub fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    // ── Mutation ─────────────────────────────────────────────────────────────

    pub fn add_node(&mut self, node: ExtractedNode) -> NodeIndex {
        if let Some(&index) = self.node_map.get(&node.id) {
            // Overwrite to keep the richest semantic context.
            self.graph[index] = GraphNode {
                id: node.id,
                label: node.label,
                source_file: node.source_file,
                source_location: node.source_location,
                kind: node.kind,
            };
            index
        } else {
            let id = node.id.clone();
            let index = self.graph.add_node(GraphNode {
                id: node.id,
                label: node.label,
                source_file: node.source_file,
                source_location: node.source_location,
                kind: node.kind,
            });
            self.node_map.insert(id, index);
            index
        }
    }

    pub fn add_edge(&mut self, edge: ExtractedEdge) {
        let src = self.node_map.get(&edge.source).cloned();
        let tgt = self.node_map.get(&edge.target).cloned();

        if let (Some(s), Some(t)) = (src, tgt) {
            // Deduplicate: skip if an edge with the same relation already exists
            // between these two nodes.
            let already_exists = self
                .graph
                .edges_directed(s, petgraph::Direction::Outgoing)
                .any(|e| e.target() == t && e.weight().relation == edge.relation);

            if !already_exists {
                self.graph.add_edge(
                    s,
                    t,
                    GraphEdge {
                        relation: edge.relation,
                        confidence: edge.confidence,
                    },
                );
            }
        }
    }

    /// Removes all nodes (and their edges) whose `source_file` matches `file_path`.
    /// Called before re-extracting a file so stale graph data is fully replaced.
    pub fn remove_nodes_for_file(&mut self, file_path: &str) {
        // Collect indices to remove — must not mutate the graph while iterating.
        let to_remove: Vec<NodeIndex> = self
            .graph
            .node_indices()
            .filter(|&idx| self.graph[idx].source_file == file_path)
            .collect();

        for idx in to_remove {
            let id = self.graph[idx].id.clone();
            self.node_map.remove(&id);
            self.graph.remove_node(idx);
        }
    }

    // ── Analysis ─────────────────────────────────────────────────────────────

    /// Returns the top-`n` nodes sorted by total degree (in + out edges),
    /// often called "God Nodes" — symbols that are deeply central to the graph.
    pub fn find_god_nodes(&self, top_n: usize) -> Vec<GodNodeEntry> {
        let mut scored: Vec<GodNodeEntry> = self
            .graph
            .node_indices()
            .map(|idx| {
                let degree = self.graph.edges(idx).count()
                    + self
                        .graph
                        .edges_directed(idx, petgraph::Direction::Incoming)
                        .count();
                GodNodeEntry {
                    id: self.graph[idx].id.clone(),
                    label: self.graph[idx].label.clone(),
                    kind: self.graph[idx].kind.clone(),
                    degree,
                }
            })
            .collect();

        scored.sort_by_key(|b| std::cmp::Reverse(b.degree));
        scored.truncate(top_n);
        scored
    }

    /// Returns `true` if the graph contains at least one directed cycle.
    /// A cycle in a dependency/call graph signals a circular dependency.
    pub fn has_cycles(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Looks up all nodes whose `id` or `label` contains `symbol` (case-insensitive).
    /// For each match returns the node itself plus all its in-edges (callers) and
    /// out-edges (callees), formatted for direct use in the `graph_lookup` tool.
    pub fn lookup(&self, symbol: &str) -> Vec<LookupResult> {
        let needle = symbol.to_lowercase();

        let mut results: Vec<LookupResult> = self
            .graph
            .node_indices()
            .filter(|&idx| {
                let n = &self.graph[idx];
                n.id.to_lowercase().contains(&needle) || n.label.to_lowercase().contains(&needle)
            })
            .map(|idx| {
                let node = &self.graph[idx];

                let callers: Vec<NeighbourEdge> = self
                    .graph
                    .edges_directed(idx, petgraph::Direction::Incoming)
                    .map(|e| NeighbourEdge {
                        node_id: self.graph[e.source()].id.clone(),
                        node_label: self.graph[e.source()].label.clone(),
                        relation: e.weight().relation.clone(),
                        confidence: e.weight().confidence.clone(),
                    })
                    .collect();

                let callees: Vec<NeighbourEdge> = self
                    .graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .map(|e| NeighbourEdge {
                        node_id: self.graph[e.target()].id.clone(),
                        node_label: self.graph[e.target()].label.clone(),
                        relation: e.weight().relation.clone(),
                        confidence: e.weight().confidence.clone(),
                    })
                    .collect();

                LookupResult {
                    node: node.clone(),
                    callers,
                    callees,
                    god_rank: None,
                }
            })
            .collect();

        // Sort results so the most highly connected matches appear first.
        results.sort_by(|a, b| {
            let degree_a = a.callers.len() + a.callees.len();
            let degree_b = b.callers.len() + b.callees.len();
            degree_b.cmp(&degree_a)
        });

        results
    }

    /// Returns all nodes in the graph as a flat list.
    pub fn all_nodes(&self) -> Vec<&GraphNode> {
        self.graph.node_weights().collect()
    }

    /// Returns the total number of nodes and edges.
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.graph.node_count(),
            edge_count: self.graph.edge_count(),
        }
    }

    // ── Serialisation ─────────────────────────────────────────────────────────

    /// Returns all edges as `(source_id, target_id, &edge_weight)` tuples.
    /// Used by `GraphCache::save()` for incremental persistence.
    pub fn all_edge_data(&self) -> Vec<(String, String, &GraphEdge)> {
        self.graph
            .edge_references()
            .map(|e| {
                (
                    self.graph[e.source()].id.clone(),
                    self.graph[e.target()].id.clone(),
                    e.weight(),
                )
            })
            .collect()
    }

    /// Serialises the complete graph to a JSON string suitable for persistence.
    pub fn to_json(&self) -> anyhow::Result<String> {
        #[derive(Serialize)]
        struct Export<'a> {
            nodes: Vec<&'a GraphNode>,
            edges: Vec<ExportEdge<'a>>,
        }

        #[derive(Serialize)]
        struct ExportEdge<'a> {
            source: &'a str,
            target: &'a str,
            relation: &'a str,
            confidence: &'a Confidence,
        }

        let edges: Vec<ExportEdge> = self
            .graph
            .edge_references()
            .map(|e| ExportEdge {
                source: &self.graph[e.source()].id,
                target: &self.graph[e.target()].id,
                relation: &e.weight().relation,
                confidence: &e.weight().confidence,
            })
            .collect();

        let export = Export {
            nodes: self.all_nodes(),
            edges,
        };

        Ok(serde_json::to_string_pretty(&export)?)
    }
}

// ── Supporting types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GodNodeEntry {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub degree: usize,
}

#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
}

/// A single edge in a lookup result — either a caller or a callee of the matched node.
#[derive(Debug, Clone)]
pub struct NeighbourEdge {
    pub node_id: String,
    pub node_label: String,
    pub relation: String,
    pub confidence: Confidence,
}

/// The result of a `graph_lookup` query for one matched node.
#[derive(Debug, Clone)]
pub struct LookupResult {
    pub node: GraphNode,
    /// Nodes that have an edge pointing *into* this node (callers / importers).
    pub callers: Vec<NeighbourEdge>,
    /// Nodes that this node has an edge pointing *to* (callees / imports).
    pub callees: Vec<NeighbourEdge>,
    /// Human-readable rank string e.g. "#3 of 142", or None if no edges.
    pub god_rank: Option<String>,
}

impl LookupResult {
    /// Formats the result as a human-readable string the agent can read directly.
    pub fn format(&self) -> String {
        let mut out = format!(
            "[Graph Lookup: \"{}\"]\n{} ({}, {})\n",
            self.node.label, self.node.id, self.node.kind, self.node.source_file,
        );
        if let Some(rank) = &self.god_rank {
            out.push_str(&format!("God Node rank: {}\n", rank));
        }
        if self.callers.is_empty() {
            out.push_str("\nCallers: none\n");
        } else {
            out.push_str("\nCallers (things that call/use this node):\n");
            for c in &self.callers {
                out.push_str(&format!(
                    "  • {}  →  {}  ({:?})\n",
                    c.node_label, c.relation, c.confidence
                ));
            }
        }
        if self.callees.is_empty() {
            out.push_str("\nCallees: none\n");
        } else {
            out.push_str("\nCallees (things this node calls/imports):\n");
            for c in &self.callees {
                out.push_str(&format!(
                    "  • {}  →  {}  ({:?})\n",
                    c.node_label, c.relation, c.confidence
                ));
            }
        }
        out
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::extract::{Confidence, ExtractedEdge, ExtractedNode};

    fn make_node(id: &str, kind: &str) -> ExtractedNode {
        ExtractedNode {
            id: id.to_string(),
            label: id.to_string(),
            source_file: "test.rs".to_string(),
            source_location: "L1-L5".to_string(),
            kind: kind.to_string(),
        }
    }

    fn make_edge(src: &str, tgt: &str, relation: &str) -> ExtractedEdge {
        ExtractedEdge {
            source: src.to_string(),
            target: tgt.to_string(),
            relation: relation.to_string(),
            confidence: Confidence::Extracted,
        }
    }

    #[test]
    fn deduplicates_nodes() {
        let mut store = GraphStore::new();
        store.add_node(make_node("a::foo", "fn"));
        store.add_node(make_node("a::foo", "fn")); // duplicate
        assert_eq!(store.stats().node_count, 1);
    }

    #[test]
    fn finds_god_nodes() {
        let mut store = GraphStore::new();
        for name in ["a::hub", "b::foo", "c::bar", "d::baz"] {
            store.add_node(make_node(name, "fn"));
        }
        // hub is called by all others
        store.add_edge(make_edge("b::foo", "a::hub", "calls"));
        store.add_edge(make_edge("c::bar", "a::hub", "calls"));
        store.add_edge(make_edge("d::baz", "a::hub", "calls"));

        let gods = store.find_god_nodes(1);
        assert_eq!(gods[0].id, "a::hub");
    }

    #[test]
    fn detects_cycles() {
        let mut store = GraphStore::new();
        store.add_node(make_node("a::fn1", "fn"));
        store.add_node(make_node("a::fn2", "fn"));
        store.add_edge(make_edge("a::fn1", "a::fn2", "calls"));
        store.add_edge(make_edge("a::fn2", "a::fn1", "calls")); // cycle!
        assert!(store.has_cycles());
    }

    #[test]
    fn serialises_to_json() {
        let mut store = GraphStore::new();
        store.add_node(make_node("a::foo", "fn"));
        let json = store.to_json().unwrap();
        assert!(json.contains("a::foo"));
    }
}
