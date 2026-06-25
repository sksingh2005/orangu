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

pub fn clip_line(line: &str, x_offset: usize, visible_width: usize) -> String {
    let mut result = String::new();
    let mut col = 0usize;
    let mut pre_clip_ansi = String::new();
    let mut in_visible = false;
    let mut truncated = false;
    let mut chars = line.chars().peekable();

    'outer: while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            let mut seq = String::from('\x1b');
            match chars.peek() {
                Some(&'[') => {
                    seq.push(chars.next().unwrap());
                    loop {
                        match chars.next() {
                            Some(c) => {
                                let done = c.is_ascii_alphabetic() || c == '~' || c == '@';
                                seq.push(c);
                                if done {
                                    break;
                                }
                            }
                            None => break 'outer,
                        }
                    }
                }
                Some(&'O') => {
                    seq.push(chars.next().unwrap());
                    if let Some(c) = chars.next() {
                        seq.push(c);
                    }
                }
                // An OSC sequence (e.g. an OSC 8 hyperlink): `ESC ] ... ST`,
                // where the terminator is BEL or `ESC \`. It draws nothing, so
                // it is carried through but never counts toward a column.
                Some(&']') => {
                    seq.push(chars.next().unwrap());
                    loop {
                        match chars.next() {
                            Some('\x07') => {
                                seq.push('\x07');
                                break;
                            }
                            Some('\x1b') => {
                                seq.push('\x1b');
                                if chars.peek() == Some(&'\\') {
                                    seq.push(chars.next().unwrap());
                                }
                                break;
                            }
                            Some(c) => seq.push(c),
                            None => break 'outer,
                        }
                    }
                }
                _ => {}
            }
            if col < x_offset {
                pre_clip_ansi.push_str(&seq);
            } else {
                result.push_str(&seq);
            }
            continue;
        }

        if col < x_offset {
            col += 1;
            continue;
        }

        let vis_col = col - x_offset;
        if vis_col >= visible_width {
            truncated = true;
            break;
        }

        if !in_visible {
            result.push_str(&pre_clip_ansi);
            in_visible = true;
        }

        result.push(ch);
        col += 1;
    }

    if truncated {
        result.push_str("\x1b[0m");
    }

    result
}

pub fn visible_line_width(line: &str) -> usize {
    let mut col = 0usize;
    let mut chars = line.chars().peekable();
    'outer: while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some(&'[') => {
                    chars.next();
                    loop {
                        match chars.next() {
                            Some(c) => {
                                if c.is_ascii_alphabetic() || c == '~' || c == '@' {
                                    break;
                                }
                            }
                            None => break 'outer,
                        }
                    }
                }
                Some(&'O') => {
                    chars.next();
                    chars.next();
                }
                // An OSC sequence (e.g. an OSC 8 hyperlink) draws nothing, so
                // skip it entirely: `ESC ] ... ST`, terminated by BEL or `ESC \`.
                Some(&']') => {
                    chars.next();
                    loop {
                        match chars.next() {
                            Some('\x07') => break,
                            Some('\x1b') => {
                                if chars.peek() == Some(&'\\') {
                                    chars.next();
                                }
                                break;
                            }
                            Some(_) => {}
                            None => break 'outer,
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        col += 1;
    }
    col
}

/// Hard-wrap an ANSI-styled line at the given visible `width`, breaking at word
/// boundaries (spaces) when possible. Restores active ANSI attributes on wrapped lines.
pub fn wrap_ansi_lines(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut col = 0usize;
    let mut active_ansi = String::new();

    let mut word = String::new();
    let mut word_col = 0usize;

    let mut flush_word = |current_line: &mut String,
                          col: &mut usize,
                          word: &mut String,
                          word_col: &mut usize,
                          active_ansi: &str| {
        if *word_col == 0 && word.is_empty() {
            return;
        }
        if *col + *word_col > width && *col > 0 {
            current_line.push_str("\x1b[0m");
            lines.push(std::mem::take(current_line));
            current_line.push_str(&active_ansi);
            *col = 0;
            // If the word starts with a space, strip it when wrapping to a new line
            if word.starts_with(' ') {
                *word = word.replacen(' ', "", 1);
                *word_col = word_col.saturating_sub(1);
            }
        }
        // If the word itself is longer than width, we have to hard-break it
        while *word_col > width {
            // Find the split point (rough character boundary for visible chars)
            let mut vis_count = 0;
            let mut byte_idx = 0;
            let mut in_ansi = false;
            for (i, c) in word.char_indices() {
                if c == '\x1b' {
                    in_ansi = true;
                }
                if !in_ansi {
                    vis_count += 1;
                }
                if in_ansi && c.is_ascii_alphabetic() {
                    in_ansi = false;
                } // very naive ANSI end
                if vis_count == width {
                    byte_idx = i + c.len_utf8();
                    break;
                }
            }
            if byte_idx == 0 || byte_idx >= word.len() {
                break;
            }
            let chunk = &word[..byte_idx];
            current_line.push_str(chunk);
            current_line.push_str("\x1b[0m");
            lines.push(std::mem::take(current_line));
            current_line.push_str(&active_ansi);

            let remainder = word[byte_idx..].to_string();
            *word = remainder;
            *word_col -= width;
        }

        current_line.push_str(word);
        *col += *word_col;
        word.clear();
        *word_col = 0;
    };

    let mut chars = line.chars().peekable();
    'outer: while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            let mut seq = String::from('\x1b');
            match chars.peek() {
                Some(&'[') => {
                    seq.push(chars.next().unwrap());
                    loop {
                        match chars.next() {
                            Some(c) => {
                                let done = c.is_ascii_alphabetic() || c == '~' || c == '@';
                                seq.push(c);
                                if done {
                                    break;
                                }
                            }
                            None => break 'outer,
                        }
                    }
                }
                Some(&'O') => {
                    seq.push(chars.next().unwrap());
                    if let Some(c) = chars.next() {
                        seq.push(c);
                    }
                }
                Some(&']') => {
                    seq.push(chars.next().unwrap());
                    loop {
                        match chars.next() {
                            Some('\x07') => {
                                seq.push('\x07');
                                break;
                            }
                            Some('\x1b') => {
                                seq.push('\x1b');
                                if chars.peek() == Some(&'\\') {
                                    seq.push(chars.next().unwrap());
                                }
                                break;
                            }
                            Some(c) => seq.push(c),
                            None => break 'outer,
                        }
                    }
                }
                _ => {}
            }
            word.push_str(&seq);
            if seq.starts_with("\x1b[") && seq.ends_with('m') {
                if seq == "\x1b[0m" {
                    active_ansi.clear();
                } else {
                    active_ansi.push_str(&seq);
                }
            }
            continue;
        }

        word.push(ch);
        word_col += 1;

        if ch == ' ' || ch == '-' {
            flush_word(
                &mut current_line,
                &mut col,
                &mut word,
                &mut word_col,
                &active_ansi,
            );
        }
    }

    flush_word(
        &mut current_line,
        &mut col,
        &mut word,
        &mut word_col,
        &active_ansi,
    );

    if !current_line.is_empty() || lines.is_empty() {
        lines.push(current_line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An OSC 8 hyperlink: `label` is shown and clickable, the URL is not drawn.
    fn osc8_link(label: &str, url: &str) -> String {
        format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
    }

    #[test]
    fn visible_width_ignores_osc8_hyperlinks() {
        // Only the label's six glyphs count; the OSC 8 control bytes (and the
        // URL they carry) are zero-width.
        let line = osc8_link("orangu", "https://example.com/orangu/");
        assert_eq!(visible_line_width(&line), "orangu".chars().count());

        // The same holds with a BEL terminator instead of ST.
        let bel = "\x1b]8;;https://example.com\x07orangu\x1b]8;;\x07";
        assert_eq!(visible_line_width(bel), "orangu".chars().count());
    }

    #[test]
    fn clip_line_preserves_osc8_hyperlinks_and_their_width() {
        let line = format!("see {} now", osc8_link("orangu", "https://example.com/"));
        // Wide enough to keep the whole line: the visible text is "see orangu now".
        let clipped = clip_line(&line, 0, 40);
        assert_eq!(
            visible_line_width(&clipped),
            "see orangu now".chars().count()
        );
        // The hyperlink's opening and closing control sequences survive.
        assert!(clipped.contains("\x1b]8;;https://example.com/\x1b\\"));
        assert!(clipped.contains("\x1b]8;;\x1b\\"));
    }
}
