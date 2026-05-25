#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub text: String,
    pub tags: Vec<String>,
    pub types: Vec<String>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.tags.is_empty() && self.types.is_empty()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.tags.clear();
        self.types.clear();
    }
}
