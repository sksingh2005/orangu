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

use ratatui::style::{Color, Style};

/// Centralized semantic styling for the Ratatui components.
#[derive(Clone, Debug)]
pub struct Theme {
    /// E.g. Approved dots, feedback OK.
    pub success: Style,
    /// E.g. Rejected dots, feedback error.
    pub error: Style,
    /// Muted or secondary text, e.g. "Ignore" dots (blue).
    pub ignore: Style,
    /// Tertiary or special text, e.g. "Deep" dots (purple).
    pub deep: Style,
    /// Separator lines, dimmed text.
    pub muted: Style,
    /// The background style for the diff/report cursor line.
    pub cursor_line_bg: Style,
    /// The active file in review file lists.
    pub selected_file: Style,
    /// The background style for the inline comment editor.
    pub comment_bg: Style,
    /// Highlights for matching text, command names, etc.
    pub highlight: Style,
    /// Warning or caution text.
    pub warning: Style,
    /// User input text.
    pub user_input: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            success: Style::default().fg(Color::Rgb(80, 200, 120)),
            error: Style::default().fg(Color::Rgb(220, 80, 80)),
            ignore: Style::default().fg(Color::Rgb(100, 160, 230)),
            deep: Style::default().fg(Color::Rgb(170, 120, 220)),
            muted: Style::default().fg(Color::Rgb(88, 88, 88)),
            cursor_line_bg: Style::default()
                .bg(Color::Rgb(145, 92, 38))
                .fg(Color::Rgb(255, 245, 230)),
            selected_file: Style::default()
                .bg(Color::Rgb(210, 140, 70))
                .fg(Color::Black),
            comment_bg: Style::default().bg(Color::Rgb(38, 48, 38)),
            highlight: Style::default().fg(Color::Cyan),
            warning: Style::default().fg(Color::Rgb(230, 200, 120)),
            user_input: Style::default().fg(Color::Rgb(220, 220, 100)),
        }
    }
}
