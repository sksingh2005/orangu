# Context-layer follow-up work

This file tracks what the current v1 covers, which external projects were
reviewed, and what is still missing in orangu without double-counting the same
idea across multiple projects.

## Current v1

The current implementation delivers:

- session-local repeated `read_file` suppression for unchanged whole-file reads
- model-facing cache stub for unchanged files
- first-pass shell output compression for common noisy commands
- config toggle: `[orangu].compression = on|off`
- tool contract updates
- documentation updates
- behavior tests, `cargo test`, and `cargo clippy` validation

This is merge-ready as a v1.

## Related projects reviewed

The maintainer asked that the implementation look beyond a single project and
adopt the best ideas natively in orangu.

To keep the direction clear, each project is treated as the primary reference
for one area.

### 1. lean-ctx

Primary reference for: coding-agent runtime context management.

Why:

- it is the closest match to what this PR already implements
- it directly covers repeated reads, shell compression, read shaping, and
  persistent agent context

What orangu already adopted:

- repeated unchanged file suppression
- shell output compression
- local config toggle for enabling/disabling the behavior

What orangu still does not have from this area:

- structural read modes (`signatures`, `map`, compact `diff`)
- cross-session persistence
- transcript compaction / deeper context lifecycle management

### 2. headroom

Primary reference for: compression framing and benchmark discipline.

Why:

- it treats compression as a first-class product feature
- it emphasizes proving savings with benchmarks rather than assuming them

What orangu already adopted:

- explicit compression toggle
- “compress before the model sees it” framing

What orangu still does not have from this area:

- benchmark results for `compression=on/off`
- surfaced savings metrics
- broader and more systematic compression coverage

### 3. fastcontext

Primary reference for: focused exploration and retrieval.

Why:

- it addresses a problem orangu does not yet solve well: finding the right
  files and line ranges before loading too much code into context

What orangu already adopted:

- nothing directly yet in runtime behavior

What orangu still does not have from this area:

- path/range-first retrieval
- focused exploration mode
- separation between exploration context and solving context

### 4. RTK-style reducers

Primary reference for: reducer evaluation and evidence-preserving reduction.

Why:

- this is the least specific maintainer reference, so it is safest to treat it
  as an evaluation/reduction-methodology influence unless a precise RTK project
  is pinned down

What orangu already adopted:

- first-pass reducer behavior for shell output

What orangu still does not have from this area:

- reducer quality evaluation
- evidence-preservation checks for reduced shell/log output
- comparative reduction methodology for real workloads

## What is still missing in orangu

After removing overlap between the reviewed projects, the real missing work is:

### 1. Structural read modes

**[COMPLETED]** We have implemented:

- `signatures`-style extraction for quick API reviews without loading implementations.
- `map` overview mode to grasp file layouts.
- `diff`-oriented compact reads for quick code-review feedback.

This was the biggest missing feature relative to the lean-ctx-style runtime and is now fully active.

### 2. Better compression coverage

**[COMPLETED]** We have implemented:

- Extended command coverage (`ls`, `find`, `npm`, `yarn`, `pip`, `rg`, `grep`, `git grep`).
- Robust prefix stripping for wrapped commands (`time`, `sudo`, env assignments).
- Context-aware deduplication and grouping for complex search/install outputs.

This fulfills the headroom-style compression direction.

### 3. Focused exploration / path-and-range retrieval

**[COMPLETED]** We have implemented:

- A native, bounded subagent (`explore_repository`) in `src/explorer.rs`.
- Read-only tools (`read_file`, `list_directory`, `run_shell_command`) exposed exclusively to the subagent.
- Automatic extraction of `<final_answer>` citation blocks to keep the main agent's context clean.

This fulfills the fastcontext-style exploration subagent architecture natively in Rust.

### 4. Cross-session persistence

**[COMPLETED]** We have implemented:

- Persistent file-context caching (`context-cache.json`) to avoid re-reading unchanged files upon session restart.
- Serde serialization for `FileFingerprint` and `ContextCache`.

This bridges the continuity gap relative to lean-ctx.

### 5. Transcript-aware context management

Still missing:

- transcript compaction
- retention/eviction policy for old tool outputs
- cache-aware interaction with any future session summarization

This is still a core orangu gap beyond the current v1.

### 6. Benchmarks and metrics

**[COMPLETED]** We have implemented:

- `CompressionMetrics` tracking per-tool and per-session bytes/lines saved.
- Global `ContextCache` visibility in `ToolExecutor`.
- Human-readable outputs integrated into the `/stats` command.
- A fully fledged `criterion` benchmark suite (`benches/compression.rs`).

This solidifies our validation features relative to headroom and RTK-style reduction evaluation.

### 7. Secret-aware filtering and safety

Still missing:

- secret redaction before sending file content
- optional filtering for sensitive files

This is a useful defensive improvement as tool usage grows.

## Recommended next order

1. Transcript-aware context management
2. Secret-aware filtering

## Phase 2: Advanced Context Compression Proposals (Headroom & FastContext inspired)

Now that we have successfully implemented the foundational context layer (Phase 1), we have identified three highly potent features from **Headroom** that we can implement next to aggressively limit context bloat. 

### What is Already Built (Our Foundation)
Before implementing the new features, we can leverage the robust foundation we have just built:
1. **Structural Read Modes (`signatures` and `map`)**: We already have the logic to parse a file and return just its structural outline instead of the full file body.
2. **Context Caching & File Fingerprinting**: We already track which files have been sent to the LLM and emit lightweight `[cached]` stubs when unchanged.
3. **Advanced Shell Compression & Diff Trimming**: We already intercept raw tool output, group it by files or hunks, and inject it as compressed payloads.
4. **Native FastContext Explorer Subagent**: We have an isolated LLM loop that delegates broad searches to a read-only agent.

### 1. Auto-Downsampling Large Files (Headroom's "CCR" Concept)
**The Concept:** When `compression = on`, automatically intercept any `read_file` request for a massive file (e.g., >300 lines) that doesn't specify line bounds, and downgrade it to `mode="signatures"`.
**Why it's easier now:** We *already built* the `signatures` extraction logic! We simply need to add a line-count check inside `ToolExecutor::read_file` before we decide whether to return `extract_signatures(&content)` or the full file, appending a warning message to guide the LLM to use `start_line` and `end_line`.

### 2. Transcript Tool-Output Eviction (Headroom's "Live Zone")
**The Concept:** Tool outputs (like massive greps or build errors) are dead weight after a few conversational turns. We implement a "Sliding Window" to evict old tool results from the transcript.
**Why it's easier now:** Our `ChatSession` struct in `src/session.rs` perfectly centralizes all message construction. We can simply iterate through `self.messages` before submitting them to the LLM, and if a `ChatMessage::tool_result` is more than 3 user-turns old, replace its payload with `[Tool output evicted to save tokens]`.

### 3. Context-Aware Diff Compression (Headroom's AST Diffing)
**The Concept:** When outputting unified diffs for code changes, standard diffs only show 3 lines of context. We will scan backwards from the diff hunk to find the nearest `fn`, `impl`, or `struct` declaration, and inject it at the top of the hunk as an anchor.
**Why it's easier now:** We just upgraded our diff compression to use hunk-level trimming in `src/compression.rs`. We can simply extend `prepare_llm_diff_context` to scan the lines above each hunk and prepend the nearest structural declaration, allowing the LLM to instantly know *which* function it modified without needing full file context.

## Practical interpretation

If the goal is “borrow the best ideas and adapt them to orangu”, the clean
mapping is:

- `lean-ctx` → runtime context behavior
- `headroom` → compression framing and benchmarks
- `fastcontext` → exploration and retrieval
- `RTK` → reducer evaluation

## Non-blocker status

Everything in this file is follow-up work.

None of these items block the current v1 from merging.
