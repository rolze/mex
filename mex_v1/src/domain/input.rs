#[allow(dead_code)]
pub enum InputContext {
    FilterText,
    FilterTag,
    FilterType,
    CommandSlug,
}

pub enum InputAction {
    Append(String),
    Reject(&'static str),
}

pub fn sanitize_char(ctx: InputContext, text_so_far: &str, c: char) -> InputAction {
    // Control prefixes mid-string
    if c == '#' || c == '@' || c == ':' {
        return InputAction::Reject("Prefix characters cannot be used mid-string");
    }

    // Convert diacritics
    let c = match c {
        'ä' | 'Ä' => return InputAction::Append("ae".to_string()),
        'ö' | 'Ö' => return InputAction::Append("oe".to_string()),
        'ü' | 'Ü' => return InputAction::Append("ue".to_string()),
        'ß' => return InputAction::Append("ss".to_string()),
        _ => c,
    };

    // Auto-convert uppercase
    let c = if c.is_ascii_uppercase() {
        c.to_ascii_lowercase()
    } else {
        c
    };

    match ctx {
        InputContext::FilterTag | InputContext::FilterType => {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                InputAction::Append(c.to_string())
            } else if c == '-' || c == '_' || c == ' ' {
                if text_so_far.is_empty() || text_so_far.ends_with(c) {
                    InputAction::Reject("Duplicate or leading separators not allowed")
                } else {
                    InputAction::Append(c.to_string())
                }
            } else {
                InputAction::Reject("Character not allowed in tags/types")
            }
        }
        InputContext::FilterText => {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' {
                InputAction::Append(c.to_string())
            } else if c == '*' {
                if text_so_far.ends_with("**") {
                    InputAction::Reject("Maximum 2 wildcards (**) allowed")
                } else {
                    InputAction::Append(c.to_string())
                }
            } else if c == ' ' || c == '_' {
                if text_so_far.is_empty() || text_so_far.ends_with('-') {
                    InputAction::Reject("Duplicate or leading separators not allowed")
                } else {
                    InputAction::Append("-".to_string())
                }
            } else if c == '-' {
                if text_so_far.is_empty() || text_so_far.ends_with('-') {
                    InputAction::Reject("Duplicate or leading separators not allowed")
                } else {
                    InputAction::Append(c.to_string())
                }
            } else {
                InputAction::Reject("Character not allowed in text filter")
            }
        }
        InputContext::CommandSlug => {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                InputAction::Append(c.to_string())
            } else if c == ' ' || c == '_' {
                if text_so_far.is_empty() || text_so_far.ends_with('-') {
                    InputAction::Reject("Duplicate or leading separators not allowed")
                } else {
                    InputAction::Append("-".to_string())
                }
            } else if c == '-' {
                if text_so_far.is_empty() || text_so_far.ends_with('-') {
                    InputAction::Reject("Duplicate or leading separators not allowed")
                } else {
                    InputAction::Append(c.to_string())
                }
            } else {
                InputAction::Reject("Character not allowed in slugs")
            }
        }
    }
}
