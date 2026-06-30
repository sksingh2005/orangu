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

#[derive(Debug, Clone)]
pub struct ContextualFragment {
    pub tag_name: String,
    pub content: String,
    pub attributes: Vec<(String, String)>,
}

impl ContextualFragment {
    pub fn new(tag_name: &str, content: &str) -> Self {
        Self {
            tag_name: tag_name.to_string(),
            content: content.to_string(),
            attributes: Vec::new(),
        }
    }

    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.push((key.to_string(), value.to_string()));
        self
    }

    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut buf = String::with_capacity(
            self.tag_name.len() * 2 + self.content.len() + self.attributes.len() * 20 + 10,
        );
        let _ = write!(&mut buf, "<{}", self.tag_name);
        for (k, v) in &self.attributes {
            let _ = write!(&mut buf, " {}=\"{}\"", k, v);
        }
        let _ = write!(&mut buf, ">\n{}\n</{}>", self.content, self.tag_name);
        buf
    }
}
