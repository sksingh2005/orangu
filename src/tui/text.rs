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
                _ => {}
            }
            continue;
        }
        col += 1;
    }
    col
}
