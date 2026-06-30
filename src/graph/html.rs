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

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use super::store::GraphStore;

// ── Colour palette (Tableau-10 extended) ────────────────────────────────────
const PALETTE: &[&str] = &[
    "#4E79A7", "#F28E2B", "#E15759", "#76B7B2", "#59A14F", "#EDC948", "#B07AA1", "#FF9DA7",
    "#9C755F", "#BAB0AC", "#86BCB6", "#D4A6C8", "#FFBE7D", "#A0CBE8", "#FABFD2", "#8CD17D",
    "#B6992D", "#499894", "#E15759", "#79706E",
];

/// Escape a string value for embedding inside a JSON string literal.
fn json_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

/// Generates a self-contained HTML file (vis-network) from the live `GraphStore`.
pub fn render_html(store: &GraphStore, workspace: &Path) -> String {
    // ── 1. Assign a community (colour group) per source file ─────────────────
    let nodes = store.all_nodes();
    let edges = store.all_edge_data();

    let mut file_to_community: HashMap<String, usize> = HashMap::new();
    for node in &nodes {
        let file = node.source_file.clone();
        let len = file_to_community.len();
        file_to_community.entry(file).or_insert(len);
    }

    // ── 2. Compute degree per node id ────────────────────────────────────────
    let mut degree: HashMap<&str, usize> = HashMap::new();
    for (src, tgt, _) in &edges {
        *degree.entry(src.as_str()).or_default() += 1;
        *degree.entry(tgt.as_str()).or_default() += 1;
    }

    // ── 3. Serialise nodes ───────────────────────────────────────────────────
    let mut node_json = String::new();
    for (i, node) in nodes.iter().enumerate() {
        let community = file_to_community
            .get(&node.source_file)
            .copied()
            .unwrap_or(0);
        let color = PALETTE[community % PALETTE.len()];
        let deg = degree.get(node.id.as_str()).copied().unwrap_or(0);
        let size = (6.0_f64 + (deg as f64 + 1.0).ln() * 2.0).min(20.0);

        let label = json_str(&node.label);
        let id_js = json_str(&node.id);
        let file_js = json_str(&node.source_file);
        let loc_js = json_str(&node.source_location);
        let kind_js = json_str(&node.kind);

        // Build the color sub-object as a plain string to avoid nested {{ }}
        // escaping issues inside the outer format! call.
        let color_obj = format!(
            "{{\"background\":\"{c}\",\"border\":\"{c}\",\"highlight\":{{\"background\":\"#ffffff\",\"border\":\"{c}\"}}}}",
            c = color
        );

        if i > 0 {
            node_json.push(',');
        }
        let _ = write!(
            node_json,
            "{{\"id\":\"{id_js}\",\"label\":\"{label}\",\"title\":\"{label}\",\"kind\":\"{kind_js}\",\"source_file\":\"{file_js}\",\"source_location\":\"{loc_js}\",\"community\":{community},\"degree\":{deg},\"size\":{size:.1},\"color\":{color_obj}}}",
        );
    }

    // ── 4. Serialise edges ───────────────────────────────────────────────────
    let mut edge_json = String::new();
    for (i, (src, tgt, edge)) in edges.iter().enumerate() {
        let src_js = json_str(src);
        let tgt_js = json_str(tgt);
        let rel_js = json_str(&edge.relation);
        if i > 0 {
            edge_json.push(',');
        }
        let _ = write!(
            edge_json,
            "{{\"from\":\"{src_js}\",\"to\":\"{tgt_js}\",\"title\":\"{rel_js}\"}}",
        );
    }

    let mut main_community = None;
    for node in &nodes {
        if node.label == "main" {
            main_community = file_to_community.get(&node.source_file).copied();
            break;
        }
    }

    // ── 5. Community legend ───────────────────────────────────────────────────
    let mut communities: Vec<(usize, String, &str)> = file_to_community
        .iter()
        .map(|(file, &idx)| {
            let name = Path::new(file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file.as_str())
                .to_string();
            (idx, name, PALETTE[idx % PALETTE.len()])
        })
        .collect();
    communities.sort_by_key(|(idx, _, _)| *idx);

    let mut legend_html = String::new();
    let mut hidden_communities_array = String::new();

    if let Some(main_idx) = main_community {
        let hidden_strs: Vec<String> = communities
            .iter()
            .filter(|(idx, _, _)| *idx != main_idx)
            .map(|(idx, _, _)| idx.to_string())
            .collect();
        hidden_communities_array = hidden_strs.join(",");
    }

    for (idx, name, color) in &communities {
        let name_esc = name.replace('<', "&lt;").replace('>', "&gt;");
        let is_dimmed = main_community.is_some_and(|main_idx| *idx != main_idx);
        let dimmed_class = if is_dimmed { " dimmed" } else { "" };
        let _ = write!(
            legend_html,
            "<div class=\"legend-item{dimmed_class}\" data-community=\"{idx}\" onclick=\"toggleCommunity({idx})\"><span class=\"legend-dot\" style=\"background:{color}\"></span><span class=\"legend-label\" title=\"{name_esc}\">{name_esc}</span></div>",
        );
    }

    let workspace_name = workspace
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");
    let node_count = nodes.len();
    let edge_count = edges.len();
    let community_count = communities.len();

    // The JavaScript section is kept as a raw string so we don't fight with
    // Rust's format! escaping. The only values we inject are the pre-built
    // JSON arrays (node_json, edge_json) which contain no format specifiers.
    let js = format!(
        "const RAW_NODES = [{node_json}];\nconst RAW_EDGES = [{edge_json}];\nconst INITIAL_HIDDEN = [{hidden_communities_array}];"
    );

    build_html(
        workspace_name,
        node_count,
        edge_count,
        community_count,
        &legend_html,
        &js,
    )
}

/// Assembles the final HTML string.  The JavaScript body is a raw string
/// constant so it is immune to accidental Rust format-string interpretation.
fn build_html(
    workspace_name: &str,
    node_count: usize,
    edge_count: usize,
    community_count: usize,
    legend_html: &str,
    js_data: &str,
) -> String {
    // The JS logic block: uses {{ / }} to escape literal braces in format!
    let js_logic = r#"
const nodeById = {};
RAW_NODES.forEach(n => nodeById[n.id] = n);

function esc(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

const hiddenCommunities = new Set(typeof INITIAL_HIDDEN !== 'undefined' ? INITIAL_HIDDEN : []);
function visibleNodes() {
  return RAW_NODES.filter(n => !hiddenCommunities.has(n.community));
}
function visibleEdges() {
  const ids = new Set(visibleNodes().map(n => n.id));
  return RAW_EDGES.filter(e => ids.has(e.from) && ids.has(e.to));
}

const container = document.getElementById('graph');
const dataset = {
  nodes: new vis.DataSet(visibleNodes()),
  edges: new vis.DataSet(visibleEdges()),
};
const options = {
  nodes: { shape: 'dot', font: { size: 0, color: '#ffffff' }, borderWidth: 1.5 },
  edges: {
    arrows: { to: { enabled: true, scaleFactor: 0.5 } },
    color: { color: '#2a2a6e', highlight: '#4E79A7' },
    font: { size: 9, color: '#666', strokeWidth: 0 },
    smooth: { type: 'dynamic' },
  },
  physics: {
    solver: 'forceAtlas2Based',
    forceAtlas2Based: { gravitationalConstant: -40, springLength: 100, springConstant: 0.05, damping: 0.9 },
    stabilization: { iterations: 200 },
  },
  interaction: { hover: true, tooltipDelay: 150 },
};
const network = new vis.Network(container, dataset, options);

network.on('click', params => {
  if (!params.nodes.length) {
    // Clicked on background: resume physics
    network.setOptions({ physics: { enabled: true } });
    document.getElementById('info-content').innerHTML = '<span class="empty">Click a node to inspect it</span>';
    return;
  }
  
  // Clicked on a node: stop moving to make inspection easier
  network.setOptions({ physics: { enabled: false } });
  
  const id = params.nodes[0];
  const node = nodeById[id];
  if (!node) return;
  showInfo(node);
});

function showInfo(node) {
  const callers = RAW_EDGES.filter(e => e.to === node.id);
  const callees = RAW_EDGES.filter(e => e.from === node.id);
  let html = '<div class="field"><b>Label:</b> ' + esc(node.label) + '</div>'
           + '<div class="field"><b>Kind:</b> ' + esc(node.kind) + '</div>'
           + '<div class="field"><b>File:</b> ' + esc(node.source_file || '—') + '</div>'
           + '<div class="field"><b>Location:</b> ' + esc(node.source_location || '—') + '</div>'
           + '<div class="field"><b>Degree:</b> ' + node.degree + '</div>';
  if (callers.length) {
    html += '<div class="field" style="margin-top:8px"><b>Called by (' + callers.length + '):</b></div>'
          + '<div id="neighbors-list">';
    callers.slice(0, 20).forEach(e => {
      const n = nodeById[e.from];
      const lbl = n ? n.label : e.from;
      const bc  = n ? n.color.background : '#333';
      html += '<div class="neighbor-link" data-id="' + esc(e.from) + '" style="border-color:' + esc(bc) + '">← ' + esc(lbl) + '</div>';
    });
    html += '</div>';
  }
  if (callees.length) {
    html += '<div class="field" style="margin-top:8px"><b>Calls (' + callees.length + '):</b></div>'
          + '<div id="neighbors-list">';
    callees.slice(0, 20).forEach(e => {
      const n = nodeById[e.to];
      const lbl = n ? n.label : e.to;
      const bc  = n ? n.color.background : '#333';
      html += '<div class="neighbor-link" data-id="' + esc(e.to) + '" style="border-color:' + esc(bc) + '">→ ' + esc(lbl) + '</div>';
    });
    html += '</div>';
  }
  document.getElementById('info-content').innerHTML = html;
  document.querySelectorAll('.neighbor-link[data-id]').forEach(el => {
    el.addEventListener('click', () => focusNode(el.dataset.id));
  });
}

function focusNode(id) {
  network.focus(id, { scale: 1.2, animation: true });
  network.selectNodes([id]);
  const node = nodeById[id];
  if (node) showInfo(node);
}

const searchInput = document.getElementById('search');
const searchResults = document.getElementById('search-results');
searchInput.addEventListener('input', () => {
  const q = searchInput.value.trim().toLowerCase();
  if (!q) { searchResults.style.display = 'none'; return; }
  const hits = RAW_NODES.filter(n =>
    n.label.toLowerCase().includes(q) || n.id.toLowerCase().includes(q)
  ).slice(0, 20);
  if (!hits.length) { searchResults.style.display = 'none'; return; }
  searchResults.innerHTML = hits.map(n =>
    '<div class="search-item" data-id="' + esc(n.id) + '">'
    + esc(n.label) + ' <span style="color:#666;font-size:10px">' + esc(n.kind) + '</span></div>'
  ).join('');
  searchResults.style.display = 'block';
  searchResults.querySelectorAll('.search-item[data-id]').forEach(el => {
    el.addEventListener('click', () => focusNode(el.dataset.id));
  });
});
document.addEventListener('click', e => {
  if (!searchResults.contains(e.target) && e.target !== searchInput) {
    searchResults.style.display = 'none';
  }
});

function toggleCommunity(idx) {
  const el = document.querySelector('[data-community="' + idx + '"]');
  if (hiddenCommunities.has(idx)) {
    hiddenCommunities.delete(idx);
    el.classList.remove('dimmed');
  } else {
    hiddenCommunities.add(idx);
    el.classList.add('dimmed');
  }
  
  // Re-enable physics so the graph re-layouts when adding/removing nodes!
  network.setOptions({ physics: { enabled: true } });
  
  dataset.nodes.clear();
  dataset.edges.clear();
  dataset.nodes.add(visibleNodes());
  dataset.edges.add(visibleEdges());
}

function selectAll() {
  document.querySelectorAll('.legend-item').forEach(el => {
    hiddenCommunities.delete(parseInt(el.dataset.community));
    el.classList.remove('dimmed');
  });
  network.setOptions({ physics: { enabled: true } });
  dataset.nodes.clear();
  dataset.edges.clear();
  dataset.nodes.add(visibleNodes());
  dataset.edges.add(visibleEdges());
}

function clearAll() {
  document.querySelectorAll('.legend-item').forEach(el => {
    hiddenCommunities.add(parseInt(el.dataset.community));
    el.classList.add('dimmed');
  });
  network.setOptions({ physics: { enabled: true } });
  dataset.nodes.clear();
  dataset.edges.clear();
  dataset.nodes.add(visibleNodes());
  dataset.edges.add(visibleEdges());
}
"#;

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Knowledge Graph — {workspace_name}</title>
<script src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"></script>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ background: #0f0f1a; color: #e0e0e0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; display: flex; height: 100vh; overflow: hidden; }}
  #graph {{ flex: 1; }}
  #sidebar {{ width: 300px; background: #1a1a2e; border-left: 1px solid #2a2a4e; display: flex; flex-direction: column; overflow: hidden; }}
  #header {{ padding: 14px; border-bottom: 1px solid #2a2a4e; }}
  #header h1 {{ font-size: 14px; color: #aaa; text-transform: uppercase; letter-spacing: 0.08em; margin-bottom: 4px; }}
  #header .subtitle {{ font-size: 11px; color: #555; }}
  #search-wrap {{ padding: 10px 12px; border-bottom: 1px solid #2a2a4e; }}
  #search {{ width: 100%; background: #0f0f1a; border: 1px solid #3a3a5e; color: #e0e0e0; padding: 7px 10px; border-radius: 6px; font-size: 13px; outline: none; }}
  #search:focus {{ border-color: #4E79A7; }}
  #search-results {{ max-height: 130px; overflow-y: auto; padding: 4px 12px; border-bottom: 1px solid #2a2a4e; display: none; }}
  .search-item {{ padding: 4px 6px; cursor: pointer; border-radius: 4px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
  .search-item:hover {{ background: #2a2a4e; }}
  #info-panel {{ padding: 14px; border-bottom: 1px solid #2a2a4e; min-height: 150px; }}
  #info-panel h3 {{ font-size: 12px; color: #aaa; margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.05em; }}
  #info-content {{ font-size: 12px; color: #ccc; line-height: 1.65; }}
  #info-content .field {{ margin-bottom: 4px; }}
  #info-content .field b {{ color: #e0e0e0; }}
  #info-content .empty {{ color: #555; font-style: italic; }}
  .neighbor-link {{ display: block; padding: 2px 6px; margin: 2px 0; border-radius: 3px; cursor: pointer; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; border-left: 3px solid #333; }}
  .neighbor-link:hover {{ background: #2a2a4e; }}
  #neighbors-list {{ max-height: 130px; overflow-y: auto; margin-top: 4px; }}
  #legend-wrap {{ flex: 1; overflow-y: auto; padding: 12px; }}
  #legend-wrap h3 {{ font-size: 12px; color: #aaa; text-transform: uppercase; letter-spacing: 0.05em; }}
  .legend-controls {{ font-size: 10px; color: #777; }}
  .legend-controls span {{ cursor: pointer; }}
  .legend-controls span:hover {{ color: #ccc; }}
  .legend-item {{ display: flex; align-items: center; gap: 8px; padding: 4px 0; cursor: pointer; border-radius: 4px; font-size: 12px; }}
  .legend-item:hover {{ background: #2a2a4e; padding-left: 4px; }}
  .legend-item.dimmed {{ opacity: 0.3; }}
  .legend-dot {{ width: 11px; height: 11px; border-radius: 50%; flex-shrink: 0; }}
  .legend-label {{ flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
  #stats {{ padding: 10px 14px; border-top: 1px solid #2a2a4e; font-size: 11px; color: #555; }}
</style>
</head>
<body>
<div id="graph"></div>
<div id="sidebar">
  <div id="header">
    <h1>Knowledge Graph</h1>
    <div class="subtitle">{workspace_name}</div>
  </div>
  <div id="search-wrap">
    <input id="search" type="text" placeholder="Search nodes…" autocomplete="off">
    <div id="search-results"></div>
  </div>
  <div id="info-panel">
    <h3>Node Info</h3>
    <div id="info-content"><span class="empty">Click a node to inspect it</span></div>
  </div>
  <div id="legend-wrap">
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:10px;">
      <h3 style="margin:0;">Files</h3>
      <div class="legend-controls">
        <span onclick="selectAll()">All</span> &middot; <span onclick="clearAll()">None</span>
      </div>
    </div>
    <div id="legend">{legend_html}</div>
  </div>
  <div id="stats">{node_count} nodes &middot; {edge_count} edges &middot; {community_count} files</div>
</div>
<script>
{js_data}
{js_logic}
</script>
</body>
</html>"#,
        workspace_name = workspace_name,
        legend_html = legend_html,
        node_count = node_count,
        edge_count = edge_count,
        community_count = community_count,
        js_data = js_data,
        js_logic = js_logic,
    )
}

/// Writes the HTML to `<workspace>/<repository>-<branch>-graph.html` (e.g. `orangu-knowledge-graph-graph.html`) and returns the path.
/// If the branch name cannot be determined, it falls back to `<repository>-workspace-graph.html`.
pub fn write_html(
    store: &GraphStore,
    workspace: &Path,
    repo_name: &str,
    branch_name: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let html = render_html(store, workspace);
    let filename = format!("{}-{}-graph.html", repo_name, branch_name);
    let path = workspace.join(&filename);
    std::fs::write(&path, html)?;
    Ok(path)
}
