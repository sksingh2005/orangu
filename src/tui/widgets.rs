use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use std::path::Path;

use crate::tui::header::{Banner, ConnStatus, HeaderStatus, display_model_name};

const CLIENT_LOGO_ART: &[&str] = &[
    " ██████  ██████   █████  ███    ██  ██████  ██    ██ ",
    "██    ██ ██   ██ ██   ██ ████   ██ ██       ██    ██ ",
    "██    ██ ██████  ███████ ██ ██  ██ ██   ███ ██    ██ ",
    "██    ██ ██   ██ ██   ██ ██  ██ ██ ██    ██ ██    ██ ",
    " ██████  ██   ██ ██   ██ ██   ████  ██████   ██████  ",
];

pub struct HeaderWidget<'a> {
    pub version: &'a str,
    pub current_model: &'a str,
    pub endpoint: &'a str,
    pub workspace: &'a Path,
    pub status: HeaderStatus,
    pub alignment: Banner,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let current_model = display_model_name(self.status.is_coordinator, self.current_model);

        let mut lines = Vec::new();
        let mut add_status_line = |text: &str, status: Option<ConnStatus>| {
            let mut spans = vec![Span::raw(text.to_string())];
            if let Some(s) = status {
                spans.push(Span::raw(" "));
                match s {
                    ConnStatus::Pending => {
                        spans.push(Span::styled("●", Style::default().fg(Color::White)))
                    }
                    ConnStatus::Ok => {
                        spans.push(Span::styled("●", Style::default().fg(Color::Green)))
                    }
                    ConnStatus::Failed => {
                        spans.push(Span::styled("●", Style::default().fg(Color::Red)))
                    }
                }
            }
            lines.push(Line::from(spans));
        };

        add_status_line(&format!("Version: {}", self.version), None);
        add_status_line("", None);
        add_status_line(
            &format!("Workspace: {}", self.workspace.display()),
            Some(ConnStatus::from_bool(self.status.workspace_ok)),
        );
        add_status_line(
            &format!("Server: {}", self.endpoint),
            Some(self.status.server_ok),
        );
        add_status_line(
            &format!("Model: {}", current_model),
            Some(self.status.model_ok),
        );
        add_status_line("", None);
        add_status_line("Help: /help", None);

        // Combine logo and status lines
        let mut combined_lines = Vec::new();
        let line_count = CLIENT_LOGO_ART.len().max(lines.len());

        let logo_width = CLIENT_LOGO_ART[0].chars().count();
        let gap = 2;

        for i in 0..line_count {
            let mut spans = Vec::new();

            // Logo part
            if i < CLIENT_LOGO_ART.len() {
                spans.push(Span::styled(
                    CLIENT_LOGO_ART[i],
                    Style::default().fg(Color::Rgb(139, 90, 43)), // ORANGU_BROWN
                ));
            } else {
                spans.push(Span::raw(" ".repeat(logo_width)));
            }

            // Gap
            spans.push(Span::raw(" ".repeat(gap)));

            // Status part
            if i < lines.len() {
                spans.extend(lines[i].spans.clone());
            }

            combined_lines.push(Line::from(spans));
        }

        let alignment = match self.alignment {
            Banner::Left => Alignment::Left,
            Banner::Center => Alignment::Center,
            Banner::Right => Alignment::Right,
        };

        let paragraph = Paragraph::new(combined_lines).alignment(alignment);

        paragraph.render(area, buf);
    }
}

pub struct TopBarWidget<'a> {
    pub branch: Option<&'a str>,
    pub workspace: &'a Path,
}

impl<'a> Widget for TopBarWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let branch_name = self.branch.unwrap_or("main");
        let path = self.workspace.display().to_string();

        let text = format!("  {} {}", branch_name, path);
        let span = Span::styled(text, Style::default().fg(Color::DarkGray));

        Paragraph::new(span).render(area, buf);
    }
}

pub struct PromptFrameWidget<'a> {
    pub current_model: &'a str,
    pub prompt_prefix: &'a str,
    pub input: &'a str,
    pub cursor: usize,
    pub ghost: &'a str,
    pub valid_command_len: usize,
}

impl<'a> Widget for PromptFrameWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let width = area.width as usize;
        let mut lines = Vec::new();

        let input_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        };

        // 1. Input lines

        let input_lines_wrapped = crate::tui::screen::wrapped_input_lines(
            self.input,
            width.saturating_sub(4),
            self.prompt_prefix,
        );
        let prompt_width = self.prompt_prefix.chars().count();
        let cmd_len = self.valid_command_len;
        let last_input_index = input_lines_wrapped.len().saturating_sub(1);
        let mut char_offset = 0;

        for (index, input_line) in input_lines_wrapped.iter().enumerate() {
            let content = crate::tui::screen::truncate_to_width(
                input_line,
                width.saturating_sub(4 + prompt_width),
            );
            let content_width = content.chars().count();

            let prefix = if index == 0 {
                self.prompt_prefix.to_string()
            } else {
                " ".repeat(prompt_width)
            };
            let mut spans = vec![Span::raw(prefix)];

            if cmd_len > 0 {
                for (i, ch) in content.chars().enumerate() {
                    let global_idx = char_offset + i;
                    let style = if global_idx < cmd_len {
                        Style::default().fg(Color::Rgb(210, 140, 70))
                    } else {
                        Style::default()
                    };
                    spans.push(Span::styled(ch.to_string(), style));
                }
            } else {
                spans.push(Span::raw(content.clone()));
            }
            char_offset += content_width;

            let used = content_width + prompt_width;

            if index == last_input_index && !self.ghost.is_empty() {
                let ghost = crate::tui::screen::truncate_to_width(
                    self.ghost,
                    width.saturating_sub(4 + used),
                );
                let ghost_width = ghost.chars().count();
                if ghost_width > 0 {
                    // ghost text color: grey
                    spans.push(Span::styled(ghost, Style::default().fg(Color::DarkGray)));
                }
            }

            lines.push(Line::from(spans));
        }

        let mut block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        if !self.current_model.is_empty() {
            block = block.title_bottom(
                ratatui::text::Line::from(self.current_model)
                    .alignment(ratatui::layout::Alignment::Right),
            );
        }

        Paragraph::new(lines).block(block).render(input_area, buf);
    }
}
