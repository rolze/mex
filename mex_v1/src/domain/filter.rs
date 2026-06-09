use regex::Regex;
use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub struct Filter {
    text: String,
    text_regex: Option<Regex>,
    pub tags: Vec<String>,
    pub types: Vec<String>,
}

pub struct MatchResult {
    pub full_match: Range<usize>,
    pub literals: Vec<Range<usize>>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.tags.is_empty() && self.types.is_empty()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.update_regex();
    }

    pub fn push_text_str(&mut self, s: &str) {
        self.text.push_str(s);
        self.update_regex();
    }

    pub fn pop_text(&mut self) {
        self.text.pop();
        self.update_regex();
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.text_regex = None;
        self.tags.clear();
        self.types.clear();
    }

    fn update_regex(&mut self) {
        if self.text.is_empty() {
            self.text_regex = None;
            return;
        }

        let mut pattern = String::from("(?i)");
        let mut chars = self.text.char_indices().peekable();
        let mut last_idx = 0;
        let mut trailing_wildcard = false;

        while let Some((idx, c)) = chars.next() {
            if c == '*' {
                if idx > last_idx {
                    pattern.push('(');
                    pattern.push_str(&regex::escape(&self.text[last_idx..idx]));
                    pattern.push(')');
                }
                if let Some(&(next_idx, '*')) = chars.peek() {
                    pattern.push_str(".*");
                    chars.next();
                    last_idx = next_idx + 1;
                    trailing_wildcard = true;
                } else {
                    pattern.push_str(".*?");
                    last_idx = idx + 1;
                    trailing_wildcard = true;
                }
            }
        }
        if last_idx < self.text.len() {
            pattern.push('(');
            pattern.push_str(&regex::escape(&self.text[last_idx..]));
            pattern.push(')');
            trailing_wildcard = false;
        }

        if trailing_wildcard {
            pattern.push_str(".*$");
        }

        self.text_regex = Regex::new(&pattern).ok();
    }

    pub fn match_text(&self, filename: &str) -> Option<MatchResult> {
        if self.text.is_empty() {
            return Some(MatchResult {
                full_match: 0..0,
                literals: Vec::new(),
            });
        }

        let re = self.text_regex.as_ref()?;
        let caps = re.captures(filename)?;
        let full_match = caps.get(0)?.range();

        let mut literals = Vec::new();
        for i in 1..caps.len() {
            if let Some(m) = caps.get(i) {
                literals.push(m.range());
            }
        }

        Some(MatchResult {
            full_match,
            literals,
        })
    }
}
