# Complete Ratatui Migration Review

Overall, I agree with the migration direction. The remaining work should
focus on fully embracing Ratatui's component model rather than simply
replacing ANSI rendering with equivalent Ratatui code.

## Theme & Styling

-   Introduce a centralized `theme.rs`.
-   Use a `Theme` struct instead of scattered style constants.
-   Enable consistent styling and future theme support.

## Manual Screen

-   Eliminate manual string concatenation.
-   Prefer `ManualScreen::draw(frame, &args)` or a dedicated widget.
-   Split the UI into ManualContent, ManualTOC, and PromptBar using
    Ratatui `Layout`.

## Transcript Rendering

Replace:

Transcript → ANSI → ansi-to-tui → Styled Text → Frame

With:

Transcript Model → Styled Lines → Paragraph → Frame

This removes unnecessary parsing and improves performance.

## Tabs

Create dedicated components such as: - WorkspaceTabs - ConversationTabs

instead of helper functions returning `Line`.

## Review Screen

Temporarily rename the new implementation (e.g. `review_native.rs`)
until feature parity is confirmed, then remove the legacy
implementation.

## Shared Data Structures

Keep models separate from rendering rather than moving everything into
`tui/mod.rs`.

Suggested organization:

review/ - mod.rs - model.rs - widget.rs

## Remove ansi-to-tui

Final rendering pipeline:

Backend → AppState → Widgets → Frame → Terminal

No ANSI conversion should remain.

## Component-Based UI

Suggested layout:

tui/ - theme.rs - layout.rs - widgets/ - screens/

## Terminal Ownership

Keep `TerminalUiGuard` responsible for the terminal and render through:

terminal.draw(\|frame\| { ChatScreen::draw(frame, &state); });

Avoid passing `&mut Terminal` through backend logic.

## Performance

Use Ratatui features: - StatefulList - Scrollbar - Cached Text -
Viewport rendering - Event-driven redraws

## Verification

Verify: - Streaming output - Markdown - Unicode - Resize - Manual
scrolling/search - TOC navigation - Review colors/comments - Multiple
tabs

## Legacy Removal

Only remove ANSI rendering after full feature parity has been verified.

Migration sequence:

Legacy ANSI → Native Ratatui → Feature parity → Remove ANSI

## Suggested Final Architecture

src/ └── tui/ ├── app_state.rs ├── theme.rs ├── layout.rs ├──
terminal.rs ├── screens/ ├── widgets/ └── models/

This keeps rendering modular, separates UI from state, and prepares the
project for future features such as popups, command palettes, split
panes, and mouse support.
