use crate::ui::theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use regex::Regex;
use std::sync::OnceLock;

static RE_STEM: OnceLock<Regex> = OnceLock::new();

pub fn colorize_stem<'a>(stem: &'a str, base_style: Style) -> Vec<Span<'a>> {
    let re = RE_STEM.get_or_init(|| {
        Regex::new(r"^(?P<year>\d{4})-(?P<month>0[1-9]|1[0-2])-(?:(?P<day>0[1-9]|[12]\d|3[01])-(?:(?P<counter_day>\d{4})|(?P<caption_day>[a-z0-9-]+?)(?:-(?P<collision>\d+))?)|(?P<slug>[a-z0-9-]+?)-(?P<counter_slug>\d{4})(?:-(?P<caption_slug>[a-z0-9-]+?))?)$").unwrap()
    });

    let mut spans = Vec::new();

    if let Some(caps) = re.captures(stem) {
        let mut parts = Vec::new();

        let mut check = |name: &str, style: Style| {
            if let Some(m) = caps.name(name) {
                parts.push((m.start(), m.end(), style));
            }
        };

        check("year", base_style);
        check("month", base_style);
        check("day", base_style);
        check("counter_day", base_style);
        check(
            "caption_day",
            base_style
                .fg(theme::COLOR_CAPTION)
                .add_modifier(Modifier::BOLD),
        );
        check("collision", base_style);
        check(
            "slug",
            base_style
                .fg(theme::COLOR_SLUG)
                .add_modifier(Modifier::BOLD),
        );
        check("counter_slug", base_style);
        check(
            "caption_slug",
            base_style
                .fg(theme::COLOR_CAPTION)
                .add_modifier(Modifier::BOLD),
        );

        parts.sort_by_key(|p| p.0);

        let mut last_end = 0;
        for (start, end, style) in parts {
            if start > last_end {
                spans.push(Span::styled(&stem[last_end..start], base_style));
            }
            spans.push(Span::styled(&stem[start..end], style));
            last_end = end;
        }

        if last_end < stem.len() {
            spans.push(Span::styled(&stem[last_end..], base_style));
        }
    } else {
        spans.push(Span::styled(stem, base_style));
    }

    spans
}
