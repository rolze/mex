use regex::Regex;
use std::sync::OnceLock;

static RE_FILE: OnceLock<Regex> = OnceLock::new();

pub struct ParsedFilename<'a> {
    pub date: &'a str,
    pub slug: Option<&'a str>,
    pub counter: Option<&'a str>,
    pub caption: Option<&'a str>,
    pub collision: Option<&'a str>,
    pub ext: Option<&'a str>,
}

pub fn parse_filename(name: &str) -> Option<ParsedFilename> {
    let re = RE_FILE.get_or_init(|| {
        Regex::new(r"^(?P<date>\d{4}-\d{2}(?:-\d{2})?)(?:-(?P<part1>[a-z0-9-]+?))?(?:-(?P<part2>[a-z0-9-]+?))?(?:-(?P<part3>[a-z0-9-]+?))?\.(?P<ext>[a-z0-9]+)$").unwrap()
    });
    
    // Actually, writing a simple string parser might be more robust and faster than Regex for this specific case!
    None
}
