\newpage

## Compression

`orangu` has a small built-in compression layer for reducing repeated or noisy
tool output before it is sent back to the model.

This area is expected to grow over time.

## Goals

The goals are:

- Avoid resending unchanged file content
- Reduce noisy shell output
- Preserve the lines most useful to the model
- Improve context efficiency without adding an external runtime dependency

## Design

Compression is implemented natively inside orangu.

It currently sits on the model-facing tool boundary: tool output is shortened
or replaced only when orangu can do so deterministically.

This means it currently applies to the tool results sent back to the model,
as well as the context populated by native user commands. For example, `/show_file`
and `/diff` render their output directly to the user in full, but they queue their
content locally. When the LLM is actually called, this queued context is then
compressed and injected into the active session. This ensures we only compress
and transmit data when the model's reasoning is actively requested.

The setting is controlled by `[orangu].compression`:

```ini
[orangu]
compression = on
```

The default is `on`.

Accepted values are:

- `on`
- `true`
- `1`
- `off`
- `false`
- `0`

When it is `off`, orangu returns the raw `read_file` and `run_shell_command`
tool output.

## Features

Compression currently affects two model-facing tools, as well as the LLM context
populated by native commands.

While native commands like `/show_file` and `/diff` continue to display their full,
uncompressed output directly to the user, they now automatically compress their
payload when injecting it into the model's context. This lets the LLM follow along
with your local investigation without flooding its context window.

### `read_file`

When the model asks for the same whole file again, and the file is unchanged,
orangu may return a cache stub instead of resending the full file content.
This context cache is **persistent across sessions**: when you close and reopen
orangu, the cache state is restored from `.orangu/context-cache.json` so you
do not waste tokens re-reading unmodified files.

This applies only to repeated whole-file reads. Line-range reads still return
the requested lines normally.

Additionally, `read_file` supports **structural read modes** to grab a high-level
understanding of large files without reading their full bodies:
- `signatures`: Extracts only the public interfaces (functions, structs) while stripping private bodies.
- `map`: Extracts top-level item declarations for an overview.

### `run_shell_command`

For recognized high-volume commands, orangu compresses noisy output before the
usual output truncation is applied. Command wrappers and prefixes like `time` or
`CARGO_TERM_COLOR=always` are stripped transparently before matching.

Current patterns include:

- **Rust:** `cargo build`, `cargo check`, `cargo test`
- **Python:** `pytest`, `python -m unittest`
- **Node:** `npm test`, `jest`, `yarn test`
- **Java:** `mvn test`, `gradle test`
- **Version Control:** `git log`, `git diff`, `git show`
- **System:** Directory listings (`ls`, `find`), Search outputs (`rg`, `grep`)
- **Package Managers:** `npm install`, `yarn`, `pip install`

### Hot-Line Context Extractor (Generic Fallback)

For any unrecognised or extremely large generic output (e.g., custom shell scripts or Makefiles), orangu uses a sophisticated "Hot-Line Context Extractor". Instead of blindly truncating the middle of a log, it scans for universal failure markers (`error:`, `Exception`, `Traceback`, `panic:`, etc.) and perfectly preserves those specific lines along with their surrounding context (+/- 3 lines). All surrounding noise is dynamically collapsed. This ensures the LLM never misses a stack trace, regardless of the language.

### Metrics & `/stats`

You can monitor the effectiveness of the compression layer by running the `/stats`
command during your session. It displays real-time metrics including total file reads,
cache hits/misses, bytes saved by the context cache, and lines stripped by shell compression.

## Implementation

The current implementation is a v1.

It provides:

- Repeated unchanged whole-file `read_file` suppression
- Cross-session persistent file caching
- Structural read modes (`signatures` and `map`)
- Shell output compression for common noisy commands
- Tracking metrics exposed via `/stats`
- Native command integration (e.g., `/show_file`, `/diff`) sending compressed context to the LLM
- A single config switch for enabling or disabling the behavior

The current design intentionally separates user display from model context: native tools show full content to the user while sending compressed summaries to the model.

It does not yet provide:

- Adaptive compression strategies
- Full transcript compaction
