# Orangu TUI → Ratatui Migration Strategy

## Goals

-   Replace manual ANSI rendering with Ratatui.
-   Preserve all backend logic and application behavior.
-   Improve maintainability, extensibility, and UI consistency.
-   Minimize regression risk.

------------------------------------------------------------------------

# Guiding Principles

1.  **Backend must not know Ratatui exists.**
2.  **Keep rendering isolated in the presentation layer.**
3.  **Perform the migration incrementally.**
4.  **Avoid global state where possible.**
5.  **Replace ANSI completely instead of mixing approaches.**

------------------------------------------------------------------------

# Recommended Architecture

    Application
    │
    ├── Backend
    │   ├── input
    │   ├── dispatch
    │   ├── commands
    │   └── state
    │
    └── Presentation
        ├── TerminalUiGuard
        ├── Renderer
        ├── Screen
        ├── Header
        ├── Review
        ├── Auto Review
        └── Theme

------------------------------------------------------------------------

# Migration Strategy 1: Terminal Ownership

## Recommended

Keep terminal ownership inside `TerminalUiGuard`.

``` rust
pub struct TerminalUiGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}
```

Benefits:

-   RAII cleanup
-   No global state
-   Easier testing
-   Future multi-window support
-   Cleaner separation of concerns

Avoid relying on `ratatui::init()` / `restore()` unless the application
is intentionally built around global initialization.

------------------------------------------------------------------------

# Migration Strategy 2: Single Rendering Entry Point

Only one place should call:

``` rust
terminal.draw(|frame| {
    render(frame, &app_state);
});
```

Avoid passing `&mut Terminal` throughout the backend.

------------------------------------------------------------------------

# Migration Strategy 3: Introduce AppState

Instead of passing many parameters:

``` rust
render(
    messages,
    tabs,
    mode,
    status,
    input,
    review,
    ...
)
```

Prefer

``` rust
render(frame, &AppState)
```

Benefits:

-   Simpler APIs
-   Easier testing
-   Future extensibility

------------------------------------------------------------------------

# Migration Strategy 4: Renderer Layer

Introduce a dedicated renderer.

``` text
src/tui/

renderer.rs
screen.rs
header.rs
review.rs
auto_review.rs
layout.rs
theme.rs
widgets/
```

Optional trait:

``` rust
trait ScreenRenderer {
    fn draw(&self, frame: &mut Frame, state: &AppState);
}
```

------------------------------------------------------------------------

# Migration Strategy 5: Replace ANSI Completely

Replace

``` text
\x1b[31m
```

with

``` rust
Style::default().fg(Color::Red)
```

Never mix ANSI and Ratatui rendering.

------------------------------------------------------------------------

# Migration Strategy 6: Theme System

Create

``` text
theme.rs
```

Example:

``` rust
Theme {
    accent,
    success,
    warning,
    error,
    muted,
}
```

Benefits:

-   Easy color changes
-   Light/Dark themes
-   Consistent styling

------------------------------------------------------------------------

# Migration Strategy 7: Layout-Driven UI

Use `Layout` instead of manual cursor positioning.

Example:

    Header
    -------------------
    Conversation
    -------------------
    Input

implemented using vertical constraints.

------------------------------------------------------------------------

# Migration Strategy 8: Widget-Based Rendering

Prefer Ratatui widgets:

-   Paragraph
-   Block
-   Tabs
-   List
-   Table
-   Gauge
-   Clear
-   Borders

instead of manual string concatenation.

------------------------------------------------------------------------

# Migration Strategy 9: Event-Driven Redraws

Redraw only when:

-   key press
-   resize
-   backend update
-   state change

Avoid continuous repainting.

------------------------------------------------------------------------

# Migration Strategy 10: Incremental File Migration

Suggested order:

1.  Cargo.toml
2.  TerminalUiGuard
3.  AppState
4.  screen.rs
5.  header.rs
6.  review.rs
7.  auto_review.rs
8.  Remove ANSI utilities
9.  Introduce theme/layout
10. Cleanup

------------------------------------------------------------------------

# Migration Strategy 11: Preserve Backend Boundaries

Keep these files largely untouched:

-   input.rs
-   dispatch.rs
-   backend operations
-   command execution
-   business logic

Only adapt how rendering is invoked.

------------------------------------------------------------------------

# Migration Strategy 12: Testing

## Compile

-   cargo check
-   cargo test

## Manual

-   Startup
-   Typing
-   Scrolling
-   Tabs
-   Review mode
-   Resize handling
-   Long conversations
-   Unicode rendering
-   Color consistency

------------------------------------------------------------------------

# Migration Strategy 13: Future-Proofing

The migration should enable future features without another
architectural rewrite:

-   Modal dialogs
-   Command palette
-   Search
-   Popups
-   Progress bars
-   Split panes
-   Mouse support
-   Status bar
-   Notifications
-   Theme switching

------------------------------------------------------------------------

# Recommended Migration Timeline

## Phase 1

-   Add Ratatui dependency
-   Introduce TerminalUiGuard
-   Create AppState

## Phase 2

-   Port main screen
-   Port header
-   Port input area

## Phase 3

-   Port review screens
-   Port auto review
-   Replace ANSI styling

## Phase 4

-   Introduce themes
-   Improve layouts
-   Performance tuning

## Phase 5

-   Remove legacy ANSI renderer
-   Cleanup
-   Regression testing

------------------------------------------------------------------------

# Final Recommendation

-   Keep Ratatui confined to the presentation layer.
-   Let `TerminalUiGuard` own the terminal instance.
-   Render exclusively through `terminal.draw(...)`.
-   Pass immutable `AppState` to rendering functions.
-   Replace ANSI with Ratatui styling entirely.
-   Use layouts and widgets instead of manual cursor positioning.
-   Migrate incrementally and verify each stage before removing legacy
    code.

## Remaining work to make the ratatui migration complete, not just partially wrapped:

1. Native ratatui review screens
   - Convert `/review` and `/auto_review` from ANSI string renderers wrapped in `Paragraph` to real ratatui widgets/layouts.
   - Current code still renders old strings through `ansi_to_tui`.

2. Fix review cursor regression
   - `tui::review::tests::multiline_cursor_position_counts_logical_lines_and_wraps` still fails.
   - This should be fixed before continuing visual work because cursor math affects editing UX.

3. Replace ANSI transcript rendering
   - `render_transcript_line_multi` still returns ANSI strings, then converts with `ansi_to_tui`.
   - Better target: return `Vec<ratatui::text::Line>` or a custom transcript widget.

4. Proper tool/thought event model
   - Tool calls should be structured UI events, not parsed from `<tool_call>` tags in assistant text.
   - Add a transcript enum variant like `ToolCall { id, name, args, expanded }` populated from session/wait events.

5. Terminal lifecycle cleanup
   - Disable mouse capture on drop.
   - Replace `draw(...).unwrap()` with error propagation or safe handling.

6. Ratatui-specific tests
   - Add buffer snapshot tests using `ratatui::backend::TestBackend`.
   - Cover output layout, dropdown placement, sticky user prompt, collapsibles, cursor position, and narrow terminals.

7. Remove migration shims
   - Once native widgets exist, remove `ansi-to-tui` if no longer needed.
   - Remove old string render functions or keep only for exports/tests if still required.

8. Visual polish pass
   - Make output, prompt, tabs, status, dropdown, and collapsibles visually coherent.
   - Ratatui gives better primitives, but the current state is still hybrid.

Recommended order:
1. Fix review cursor test.
2. Terminal lifecycle cleanup.
3. Convert transcript/output area to native ratatui lines.
4. Convert review and auto-review screens.
5. Replace tool-call parsing with structured events.
6. Add ratatui buffer snapshot tests.