//! UC-08 · Smart import of new files.
//!
//! Full port of the reference Python implementation (scan.py / execute.py /
//! picture-organizer.md).  The entry points used by the TUI are:
//!
//!   1. `scan_source(source_root, existing_hashes)` → `Vec<ImportEntry>`
//!   2. `apply_folder_mtime_consensus(&mut entries)`
//!   3. `assign_counters(&mut entries, target_root, db_conn)`
//!   4. `execute_import(entries, source_root, target_root, db_conn, import_date)`

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

// ── Supported extensions ──────────────────────────────────────────────────────

const PHOTO_EXTS: &[&str] = &[
    "jpg", "jpeg", "heic", "heif", "png", "gif", "webp", "tiff", "tif", "bmp",
];
const RAW_EXTS: &[&str] = &[
    "raw", "cr2", "cr3", "nef", "arw", "orf", "rw2", "dng", "raf", "pef",
];
const VIDEO_EXTS: &[&str] = &[
    "mp4", "mov", "avi", "mkv", "m4v", "mts", "m2ts", "3gp", "wmv", "flv", "mpg", "mpeg",
];

fn is_supported_ext(ext: &str) -> bool {
    let e = ext.to_lowercase();
    PHOTO_EXTS.iter().any(|&x| x == e)
        || RAW_EXTS.iter().any(|&x| x == e)
        || VIDEO_EXTS.iter().any(|&x| x == e)
}

const ALWAYS_SKIP: &[&str] = &[
    "thumbs.db",
    "ehthumbs.db",
    "desktop.ini",
    ".ds_store",
];
const THUMBNAIL_PREFIXES: &[&str] = &["TN_", "TN "];

// ── Folder / file classification ──────────────────────────────────────────────

const FOLDER_JUNK: &[&str] = &[
    "web",
    "small",
    "thumbnail",
    "thumbnails",
    "thumb",
    "thumbs",
    "preview",
    "previews",
    "original",
    "originals",
];
const GENERIC_ORG: &[&str] = &[
    "others", "other", "various", "assorted", "unsorted", "general", "rest", "sonstiges",
];

fn is_camera_folder_pattern(name: &str) -> bool {
    // \d{3}[A-Z]+ (e.g. 100CANON, 118NIKON) or DCIM or MISC (all-caps only)
    let bytes = name.as_bytes();
    if bytes.len() >= 4
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3..].iter().all(|b| b.is_ascii_uppercase())
    {
        return true;
    }
    name == "DCIM" || name == "MISC"
}

pub fn is_junk_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    if FOLDER_JUNK.iter().any(|&j| j == lower) || GENERIC_ORG.iter().any(|&g| g == lower) {
        return true;
    }
    is_camera_folder_pattern(name)
}

/// Returns true if the folder name contains a quality-variant token as a standalone word.
pub fn is_quality_variant(name: &str) -> bool {
    let lower = name.to_lowercase();
    for token in &["small", "web", "thumbnail", "thumbnails", "thumb", "thumbs", "preview", "previews"] {
        // Standalone: surrounded by space, hyphen, underscore, or at start/end
        let bytes = lower.as_bytes();
        let tk = token.as_bytes();
        let mut i = 0usize;
        while i + tk.len() <= bytes.len() {
            if bytes[i..i + tk.len()] == *tk {
                let before_ok = i == 0 || matches!(bytes[i - 1], b' ' | b'-' | b'_');
                let after_ok = i + tk.len() == bytes.len()
                    || matches!(bytes[i + tk.len()], b' ' | b'-' | b'_');
                if before_ok && after_ok {
                    return true;
                }
            }
            i += 1;
        }
    }
    false
}

const DUMP_FOLDER_WORDS: &[&str] = &["bilder", "fotos", "videos", "photos", "pictures"];

/// True if folder name is just a date prefix + generic dump word(s).
/// E.g. "2024-01-11-Bilder", "2024-Videos", "2025-11-Bilder".
pub fn is_dump_folder(name: &str) -> bool {
    let mut s = name.to_lowercase();
    // transliterate umlauts
    s = transliterate(&s);
    // strip 4-digit years
    s = regex_replace_all(&s, r"\b(?:19|20)\d{2}\b", " ");
    // strip short numbers (month/day)
    s = regex_replace_all(&s, r"\b\d{1,2}\b", " ");
    // normalise separators
    s = regex_replace_all(&s, r"[-_.\s]+", " ");
    let tokens: Vec<&str> = s.split_whitespace().collect();
    !tokens.is_empty() && tokens.iter().all(|t| DUMP_FOLDER_WORDS.iter().any(|&d| d == *t))
}

// ── Transliteration ───────────────────────────────────────────────────────────

fn transliterate(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            'ä' | 'Ä' => out.push_str("ae"),
            'ö' | 'Ö' => out.push_str("oe"),
            'ü' | 'Ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            _ => out.push(ch),
        }
    }
    out
}

// ── Minimal regex helpers (avoid regex crate dependency) ─────────────────────
// We implement the handful of patterns needed with hand-rolled matchers.

/// Replace all occurrences of a simple literal sequence with `replacement`.
/// Only used for sep-normalisation; not a general regex engine.
fn replace_sep(s: &str) -> String {
    // Replace runs of [-_.\s] with single space
    let mut out = String::with_capacity(s.len());
    let mut prev_was_sep = false;
    for ch in s.chars() {
        if matches!(ch, '-' | '_' | '.' | ' ' | '\t') {
            if !prev_was_sep {
                out.push(' ');
            }
            prev_was_sep = true;
        } else {
            out.push(ch);
            prev_was_sep = false;
        }
    }
    out
}

/// Very small hand-rolled "regex" replacer for the specific patterns we need.
fn regex_replace_all(s: &str, pattern: &str, replacement: &str) -> String {
    match pattern {
        // strip 4-digit years 1900–2099
        r"\b(?:19|20)\d{2}\b" => {
            let bytes = s.as_bytes();
            let mut out = String::with_capacity(s.len());
            let mut i = 0usize;
            while i < bytes.len() {
                if i + 4 <= bytes.len() {
                    let b0 = bytes[i];
                    let b1 = bytes.get(i + 1).copied().unwrap_or(0);
                    let b2 = bytes.get(i + 2).copied().unwrap_or(0);
                    let b3 = bytes.get(i + 3).copied().unwrap_or(0);
                    if (b0 == b'1' || b0 == b'2')
                        && b1.is_ascii_digit()
                        && b2.is_ascii_digit()
                        && b3.is_ascii_digit()
                    {
                        // Check word boundaries
                        let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                        let after_ok = i + 4 >= bytes.len() || !bytes[i + 4].is_ascii_digit();
                        if before_ok && after_ok {
                            out.push_str(replacement);
                            i += 4;
                            continue;
                        }
                    }
                }
                out.push(s.chars().nth(i).unwrap_or(' '));
                i += 1;
            }
            out
        }
        // strip short numbers (1-2 digit)
        r"\b\d{1,2}\b" => {
            let bytes = s.as_bytes();
            let mut out = String::with_capacity(s.len());
            let mut i = 0usize;
            while i < bytes.len() {
                if bytes[i].is_ascii_digit() {
                    let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                    // consume digits
                    let mut j = i;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    let len = j - i;
                    let after_ok = j >= bytes.len() || !bytes[j].is_ascii_alphanumeric();
                    if before_ok && after_ok && len <= 2 {
                        out.push_str(replacement);
                        i = j;
                        continue;
                    } else {
                        for k in i..j {
                            out.push(bytes[k] as char);
                        }
                        i = j;
                        continue;
                    }
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            out
        }
        // normalise separators
        r"[-_.\s]+" => replace_sep(s),
        _ => s.to_string(),
    }
}

// ── UUID helpers ─────────────────────────────────────────────────────────────

/// Returns true if the byte slice at position 0..36 matches a UUID hex pattern.
fn match_uuid_at(b: &[u8]) -> bool {
    if b.len() < 36 {
        return false;
    }
    let groups: &[usize] = &[8, 4, 4, 4, 12];
    let mut pos = 0usize;
    for (gi, &len) in groups.iter().enumerate() {
        for _ in 0..len {
            if !b[pos].is_ascii_hexdigit() {
                return false;
            }
            pos += 1;
        }
        if gi < 4 {
            if b[pos] != b'-' {
                return false;
            }
            pos += 1;
        }
    }
    true
}

/// Returns true if `s` is exactly a UUID (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).
pub fn is_uuid_stem(s: &str) -> bool {
    s.len() == 36 && match_uuid_at(s.as_bytes())
}

/// Remove all UUID sub-strings (8-4-4-4-12 hex) from `s`, replacing with a space.
fn strip_uuids(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((byte_pos, ch)) = chars.next() {
        // UUIDs start with an ASCII hex digit; only attempt match at ASCII boundaries
        if ch.is_ascii_hexdigit() && byte_pos + 36 <= b.len() && match_uuid_at(&b[byte_pos..]) {
            out.push(' ');
            // Consume the remaining 35 ASCII characters of the UUID
            for _ in 0..35 {
                chars.next();
            }
        } else {
            out.push(ch);
        }
    }
    out
}

// ── Camera code detection ─────────────────────────────────────────────────────

const CAMERA_CODE_PREFIXES: &[&str] = &[
    "img", "dsc", "dscn", "dscf", "cimg", "mvc", "sdc", "pic", "pict", "kif", "ssl", "dcp",
    "picture", "bild", "pano", "vid", "mvi", "save", "burst", "snap",
];

pub fn is_camera_code(filename: &str) -> bool {
    let (base, _) = split_ext(filename);
    let lower = base.to_lowercase();
    // Camera code + optional digits: e.g. IMG_1234, DSC0042, CIMG0486
    for prefix in CAMERA_CODE_PREFIXES {
        if lower == *prefix {
            return true;
        }
        if lower.starts_with(prefix) {
            let rest = &lower[prefix.len()..];
            let rest = rest.trim_start_matches(|c| c == '_' || c == '-');
            // Camera codes are followed only by digits and separators (no real words)
            if rest.is_empty()
                || rest.chars().all(|c| c.is_ascii_digit() || c == '_' || c == '-')
            {
                return true;
            }
        }
    }
    // "Kopie von *" / "Copy of *"
    if lower.starts_with("kopie von ") || lower.starts_with("copy of ") {
        return true;
    }
    // WhatsApp: IMG-YYYYMMDD-WA0004 format
    if lower.starts_with("img-") || lower.starts_with("img_") {
        if let Some(wa_pos) = lower.find("-wa") {
            let after_wa = &lower[wa_pos + 3..];
            if !after_wa.is_empty() && after_wa.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    // FAT32 all-uppercase alpha + digits, no separator: e.g. PARIS1, LONDON3
    is_camera_fat32(base)
}

/// Matches `^[A-Z]+\d+$`
fn is_camera_fat32(base: &str) -> bool {
    let bytes = base.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let alpha_end = bytes.iter().position(|b| !b.is_ascii_uppercase());
    match alpha_end {
        None => false, // all letters, no digits
        Some(0) => false, // starts with digit
        Some(pos) => bytes[pos..].iter().all(|b| b.is_ascii_digit()),
    }
}

// ── Slug primitives ───────────────────────────────────────────────────────────

const SHORT_SLUG_WHITELIST: &[&str] = &["uni", "sun", "fkn", "ocp", "pab"];

const JUNK_WORDS: &[&str] = &[
    "img", "dsc", "dscn", "dscf", "cimg", "mvc", "sdc", "pic", "pict", "kif", "ssl", "dcp",
    "picture", "bild", "kopie", "copy", "von", "of", "foto", "photo", "image",
    "screenshot", "screenshots", "bilder", "videos",
    "the", "and", "und", "mit", "an", "im", "oder", "der", "die", "das", "ein", "eine",
    // Camera / app prefixes that produce no useful slug
    "save", "snap", "pano", "vid", "mvi", "burst",
    // Android Camera folder — produces a meaningless "camera" slug
    "camera",
    // Common stop words (German + English) that slip through as 2-3 char tokens
    "in", "on", "at", "zu", "am", "bei", "auf", "aus", "vor", "vom", "from",
    // Generic file/download names
    "file", "download", "export",
];

/// Extract a slug from a name (filename or folder name).
/// Returns `None` if no meaningful tokens remain after stripping.
pub fn extract_slug(name: &str) -> Option<String> {
    let (base, ext) = split_ext(name);
    // UUID stems produce no slug (and have no useful date either)
    if is_uuid_stem(base) {
        return None;
    }
    // Only strip extension if it looks like a real file extension (≤5 chars, no spaces).
    let s = if !ext.is_empty() && ext.len() <= 5 && !ext.contains(' ') {
        base
    } else {
        name
    };
    // Strip embedded UUIDs before they get broken apart by replace_sep
    let s = strip_uuids(s);
    let s = transliterate(&s);
    let s = replace_sep(&s);
    // Split on letter→digit transitions ("snap202405051452" → "snap 202405051452")
    let s = split_letter_digit(&s);
    // Strip 4-digit years
    let s = regex_replace_all(&s, r"\b(?:19|20)\d{2}\b", " ");
    // Strip 6–8 digit sequences
    let s = strip_long_numbers(&s);
    // Strip all remaining digit-only tokens
    let s = strip_short_numbers(&s);

    let tokens: Vec<String> = s
        .split_whitespace()
        .map(|t| {
            // lowercase + keep only a-z 0-9 (mirrors Python's name.lower() + re.sub(r'[^a-z0-9]','',t))
            t.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>()
        })
        .filter(|t| {
            !t.is_empty()
                && t.len() >= 3                  // ≥3 chars: filters "in","zu","wa","am" etc.
                && t.len() <= 20                 // ≤20 chars: filters extreme random IDs
                && !t.chars().all(|c| c.is_ascii_digit())
                && !JUNK_WORDS.iter().any(|&j| j == t.as_str())
                && !is_hex_garbage(t)            // filter UUID/hash hex fragments
                && !is_seq_number(t)             // filter "wa0004", "p6agdx"-style IDs
        })
        .collect();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens[..tokens.len().min(4)].join("-"))
    }
}

/// Returns true if a token consists entirely of hex characters and is long enough
/// to be a UUID fragment or hash (≥5 chars, all [0-9a-f]).
fn is_hex_garbage(t: &str) -> bool {
    t.len() >= 5 && t.chars().all(|c| c.is_ascii_hexdigit())
}

/// Returns true if a token looks like a sequence/counter code:
/// 1–3 alphabetic chars followed by 3+ digits (e.g. "wa0004", "wA0013", "p6a...").
/// Also catches short opaque IDs like "p6agdx" (≤3 letters then alphanumeric mixed).
fn is_seq_number(t: &str) -> bool {
    let bytes = t.as_bytes();
    let alpha_end = bytes
        .iter()
        .position(|b| !b.is_ascii_alphabetic())
        .unwrap_or(bytes.len());
    if alpha_end == 0 || alpha_end > 3 {
        return false;
    }
    // Rest must be all digits and at least 3 of them
    bytes[alpha_end..].iter().all(|b| b.is_ascii_digit()) && bytes.len() - alpha_end >= 3
}

/// Insert a space between an alphabetic char and a following digit
/// ("snap202405051452" → "snap 202405051452", "phoneImageCapture1764…" → "…Capture 1764…").
fn split_letter_digit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_alpha = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() && prev_alpha {
            out.push(' ');
        }
        out.push(ch);
        prev_alpha = ch.is_ascii_alphabetic();
    }
    out
}

fn strip_long_numbers(s: &str) -> String {
    // Remove 6–8 digit sequences
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let len = j - i;
            if len >= 6 {
                out.push(' ');
            } else {
                for k in i..j {
                    out.push(bytes[k] as char);
                }
            }
            i = j;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn strip_short_numbers(s: &str) -> String {
    // Remove standalone digit-only tokens
    s.split_whitespace()
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Walk from outermost to innermost path component (relative to source_root),
/// accumulating meaningful slug tokens (max 4).
pub fn slug_from_path(file_path: &Path, source_root: &Path) -> Option<String> {
    let dir = file_path.parent().unwrap_or(file_path);
    let rel = match dir.strip_prefix(source_root) {
        Ok(r) => r,
        Err(_) => dir,
    };

    let parts: Vec<&str> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    let mut acc: Vec<String> = Vec::new();
    for part in &parts {
        if is_junk_folder(part) || is_quality_variant(part) {
            continue;
        }
        let s = match extract_slug(part) {
            Some(s) => s,
            None => continue,
        };
        let tokens: Vec<&str> = s.split('-').collect();
        // Skip single opaque short token not in whitelist
        if tokens.len() == 1
            && tokens[0].len() <= 3
            && !SHORT_SLUG_WHITELIST.iter().any(|&w| w == tokens[0])
        {
            continue;
        }
        for t in tokens {
            acc.push(t.to_string());
            if acc.len() >= 4 {
                break;
            }
        }
        if acc.len() >= 4 {
            break;
        }
    }

    if acc.is_empty() {
        None
    } else {
        Some(acc[..acc.len().min(4)].join("-"))
    }
}

/// Derive a caption slug from a filename, stripping tokens that duplicate the folder slug.
pub fn derive_caption_slug(filename: &str, folder_slug: Option<&str>) -> Option<String> {
    let (base, _) = split_ext(filename);
    let cap = extract_slug(base)?;
    let folder_slug = match folder_slug {
        Some(f) => f,
        None => return Some(cap),
    };
    let fs: Vec<&str> = folder_slug.split('-').collect();
    let mut cs: Vec<String> = cap.split('-').map(|s| s.to_string()).collect();

    // Strip leading tokens that duplicate folder slug prefix
    let mut fs2 = fs.clone();
    while !cs.is_empty() && !fs2.is_empty() && cs[0] == fs2[0] {
        cs.remove(0);
        fs2.remove(0);
    }
    // Strip trailing tokens that duplicate folder slug suffix
    let mut fs3 = fs.clone();
    while !cs.is_empty() && !fs3.is_empty() && cs.last().unwrap() == fs3.last().unwrap() {
        cs.pop();
        fs3.pop();
    }

    if cs.is_empty() {
        None
    } else {
        Some(cs.join("-"))
    }
}

/// Split a filename into (base_without_ext, ext_without_dot).
fn split_ext(name: &str) -> (&str, &str) {
    if let Some(dot) = name.rfind('.') {
        let base = &name[..dot];
        let ext = &name[dot + 1..];
        // Guard: only treat as extension if ≤5 chars, no spaces
        if ext.len() <= 5 && !ext.contains(' ') {
            return (base, ext);
        }
    }
    (name, "")
}

// ── Date parsing ──────────────────────────────────────────────────────────────

const GERMAN_MONTHS: &[(&str, u32)] = &[
    ("januar", 1),
    ("jänner", 1),
    ("jaenner", 1),
    ("februar", 2),
    ("märz", 3),
    ("maerz", 3),
    ("april", 4),
    ("mai", 5),
    ("juni", 6),
    ("juli", 7),
    ("august", 8),
    ("september", 9),
    ("oktober", 10),
    ("november", 11),
    ("dezember", 12),
];

/// Parse Nokia/Motorola feature phone filenames: `DD-MM-YY_HHMM` → `YYYY-MM-DD`
pub fn parse_mobile_date(s: &str) -> Option<String> {
    // Matches ^(\d{2})-(\d{2})-(\d{2})_(\d{4})
    let bytes = s.as_bytes();
    if bytes.len() < 11 {
        return None;
    }
    if bytes[2] != b'-' || bytes[5] != b'-' || bytes[8] != b'_' {
        return None;
    }
    for i in [0, 1, 3, 4, 6, 7, 9, 10] {
        if !bytes[i].is_ascii_digit() {
            return None;
        }
    }
    let dd: u32 = parse_2digit(bytes, 0)?;
    let mm: u32 = parse_2digit(bytes, 3)?;
    let yy: u32 = parse_2digit(bytes, 6)?;
    let year = if yy <= 30 { 2000 + yy } else { 1900 + yy };
    if mm == 0 || mm > 12 || dd == 0 || dd > 31 {
        return None;
    }
    Some(format!("{year:04}-{mm:02}-{dd:02}"))
}

fn parse_2digit(bytes: &[u8], i: usize) -> Option<u32> {
    let a = (bytes[i] - b'0') as u32;
    let b = (bytes[i + 1] - b'0') as u32;
    Some(a * 10 + b)
}

/// Parse Apple Photos German folder names: `"D. MonthDE YYYY"` or `"title, D. MonthDE YYYY"`
pub fn parse_german_folder_date(name: &str) -> Option<String> {
    // Pattern: optional prefix ending in ", " then (\d{1,2})\. (\w+) (\d{4}) at end
    let s = name.trim();
    // Find the last occurrence of the pattern
    let lower = s.to_lowercase();
    let lower = transliterate(&lower);
    // Try to find: optional ", " + digits + ". " + month + " " + year at end
    // We scan for " YYYY" at the end first
    let year_end = {
        let b = lower.as_bytes();
        if b.len() < 4 {
            return None;
        }
        let tail = &b[b.len() - 4..];
        if tail.iter().all(|x| x.is_ascii_digit()) {
            let yr: u32 = std::str::from_utf8(tail).ok()?.parse().ok()?;
            if yr < 1900 || yr > 2050 {
                return None;
            }
            yr
        } else {
            return None;
        }
    };

    // Now scan backwards for month name
    let without_year = lower.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
    let month_num = GERMAN_MONTHS
        .iter()
        .find(|(m, _)| without_year.ends_with(m))
        .map(|(m, n)| (*m, *n))?;

    let without_month = without_year
        .trim_end_matches(month_num.0)
        .trim_end();

    // Expect a day number like "15. " or "1. " before the month
    // without_month should end with "D." or "D. "
    let day_str = without_month.trim_end_matches(|c: char| c == '.' || c == ' ');
    let day_str = day_str.split(|c: char| c == ',' || c == ' ').last()?;
    let dd: u32 = day_str.parse().ok()?;
    if dd == 0 || dd > 31 {
        return None;
    }

    let mm = month_num.1;
    let year = year_end;
    Some(format!("{year:04}-{mm:02}-{dd:02}"))
}

/// Parse date embedded in a filename.
pub fn parse_date_filename(filename: &str) -> Option<String> {
    let (base, _) = split_ext(filename);
    // UUID stems: skip all filename date extraction (hex digits produce false positives)
    if is_uuid_stem(base) {
        return None;
    }
    // 1. Mobile phone format first
    if let Some(d) = parse_mobile_date(base) {
        return Some(d);
    }
    // 2. yyyyMMdd or yyyy-MM-dd
    if let Some(d) = find_ymd_in(base) {
        return Some(d);
    }
    // 3. yyMMdd (6 consecutive digits)
    if let Some(d) = find_yymmdd_in(base) {
        return Some(d);
    }
    // 4. yy-MM-dd or yy_MM_dd with separators (e.g. Picsart_24-11-24_...)
    if let Some(d) = find_yymmdd_sep_in(base) {
        return Some(d);
    }
    // 5. App-specific: snap/SnapWidget prefix + YYYYMMDD+time (e.g. snap202405051452)
    if let Some(d) = find_snap_date(base) {
        return Some(d);
    }
    // 6. yyyy-MM at start (already-organised files)
    if let Some(d) = find_year_month_prefix(base) {
        return Some(d);
    }
    // 7. 13-digit Unix millisecond timestamp anywhere in the name
    //    (e.g. "phoneImageCapture1764072777554", "Revolut_receipt_..._1703345353881")
    if let Some(d) = find_unix_ms_date(base) {
        return Some(d);
    }
    None
}

fn find_ymd_in(s: &str) -> Option<String> {
    let b = s.as_bytes();
    // Pattern 1: yyyyMMdd (8 consecutive digits where yyyy in 1900-2099)
    let mut i = 0;
    while i + 8 <= b.len() {
        if is_4year(b, i) && all_digit(b, i, 8) {
            let mm = parse_2digit_u(b, i + 4);
            let dd = parse_2digit_u(b, i + 6);
            let after_ok = i + 8 >= b.len() || !b[i + 8].is_ascii_digit();
            if mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 && after_ok {
                let yr = parse_4year(b, i);
                return Some(format!("{yr:04}-{mm:02}-{dd:02}"));
            }
        }
        i += 1;
    }
    // Pattern 2: yyyy-MM-dd or yyyy_MM_dd
    i = 0;
    while i + 10 <= b.len() {
        if is_4year(b, i)
            && is_sep(b, i + 4)
            && all_digit(b, i + 5, 2)
            && is_sep(b, i + 7)
            && all_digit(b, i + 8, 2)
        {
            let mm = parse_2digit_u(b, i + 5);
            let dd = parse_2digit_u(b, i + 8);
            let after_ok = i + 10 >= b.len() || !b[i + 10].is_ascii_digit();
            if mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 && after_ok {
                let yr = parse_4year(b, i);
                return Some(format!("{yr:04}-{mm:02}-{dd:02}"));
            }
        }
        i += 1;
    }
    None
}

fn find_yymmdd_in(s: &str) -> Option<String> {
    let b = s.as_bytes();
    // yyMMdd (6 consecutive digits, boundaries)
    let mut i = 0;
    while i + 6 <= b.len() {
        let before_ok = i == 0 || !b[i - 1].is_ascii_digit();
        if before_ok && all_digit(b, i, 6) {
            let after_ok = i + 6 >= b.len() || !b[i + 6].is_ascii_digit();
            if after_ok {
                let yy = parse_2digit_u(b, i);
                let mm = parse_2digit_u(b, i + 2);
                let dd = parse_2digit_u(b, i + 4);
                if mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 {
                    let yr = if yy <= 30 { 2000 + yy } else { 1900 + yy };
                    return Some(format!("{yr:04}-{mm:02}-{dd:02}"));
                }
            }
        }
        i += 1;
    }
    None
}

fn find_year_month_prefix(s: &str) -> Option<String> {
    // yyyy-MM or yyyy_MM at start, not followed by another digit immediately
    let b = s.as_bytes();
    if b.len() < 7 {
        return None;
    }
    if is_4year(b, 0) && is_sep(b, 4) && all_digit(b, 5, 2) {
        let after_ok = b.len() == 7 || !b[7].is_ascii_digit();
        if after_ok {
            let yr = parse_4year(b, 0);
            let mm = parse_2digit_u(b, 5);
            if mm >= 1 && mm <= 12 {
                return Some(format!("{yr:04}-{mm:02}-01"));
            }
        }
    }
    None
}

/// Find `yy[-_]mm[-_]dd` patterns (2-digit year with separator), e.g. Picsart `24-11-24`.
/// Returned as `YYYY-MM-DD`.
fn find_yymmdd_sep_in(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i + 8 <= b.len() {
        let before_ok = i == 0 || !b[i - 1].is_ascii_digit();
        if before_ok
            && all_digit(b, i, 2)
            && is_sep(b, i + 2)
            && all_digit(b, i + 3, 2)
            && is_sep(b, i + 5)
            && all_digit(b, i + 6, 2)
        {
            let after_ok = i + 8 >= b.len() || !b[i + 8].is_ascii_digit();
            if after_ok {
                let yy = parse_2digit_u(b, i);
                let mm = parse_2digit_u(b, i + 3);
                let dd = parse_2digit_u(b, i + 6);
                let yr = if yy <= 30 { 2000 + yy } else { 1900 + yy };
                if mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 {
                    return Some(format!("{yr:04}-{mm:02}-{dd:02}"));
                }
            }
        }
        i += 1;
    }
    None
}

/// Detect snap/SnapWidget filenames: `snap` + exactly 8 date digits + 4 time digits.
/// e.g. `snap202405051452` → `2024-05-05`.
fn find_snap_date(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    let rest = lower.strip_prefix("snap")?;
    if rest.len() >= 12 && rest[..12].bytes().all(|b| b.is_ascii_digit()) {
        find_ymd_in(&rest[..8])
    } else {
        None
    }
}

/// Find a 13-digit Unix millisecond timestamp anywhere in `s` and convert to a date.
/// Valid range: roughly 2000-01-01 to 2100-01-01 (946_684_800_000 … 4_102_444_800_000 ms).
fn find_unix_ms_date(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i + 13 <= b.len() {
        let before_ok = i == 0 || !b[i - 1].is_ascii_digit();
        if before_ok && b[i..i + 13].iter().all(|x| x.is_ascii_digit()) {
            let after_ok = i + 13 >= b.len() || !b[i + 13].is_ascii_digit();
            if after_ok {
                let slice = std::str::from_utf8(&b[i..i + 13]).ok()?;
                let ms: i64 = slice.parse().ok()?;
                let secs = ms / 1000;
                // 2000-01-01 … 2100-01-01
                if secs >= 946_684_800 && secs <= 4_102_444_800 {
                    return Some(secs_to_date_pub(secs as u64));
                }
            }
        }
        i += 1;
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
pub enum DatePrecision {
    Full,
    YearMonth,
    YearOnly,
}

/// Try to derive a date from path folder components (innermost first → pass 1, 2, 3, 4).
/// Returns `(date_str "YYYY-MM-DD", precision)` or `None`.
pub fn parse_date_folders(parts: &[&str]) -> Option<(String, DatePrecision)> {
    // Pass 1: full date patterns (innermost first)
    for part in parts.iter().rev() {
        if let Some(d) = parse_german_folder_date(part) {
            return Some((d, DatePrecision::Full));
        }
        if let Some(d) = find_ymd_in(part) {
            return Some((d, DatePrecision::Full));
        }
        if let Some(d) = find_yymmdd_in(part) {
            return Some((d, DatePrecision::Full));
        }
    }

    // Pass 2: year + German month name embedded
    for part in parts.iter().rev() {
        let lower = transliterate(&part.to_lowercase());
        if let Some(yr) = find_4year_str(&lower) {
            for (mname, mnum) in GERMAN_MONTHS {
                if lower.contains(mname) {
                    return Some((format!("{yr:04}-{mnum:02}-01"), DatePrecision::YearMonth));
                }
            }
        }
    }

    // Pass 3: year only or yy-suffix
    for part in parts.iter().rev() {
        // yy-suffix: ends with -YY or _YY
        let b = part.as_bytes();
        if b.len() >= 3 {
            let sep = b[b.len() - 3];
            if (sep == b'-' || sep == b'_') && b[b.len() - 2].is_ascii_digit() && b[b.len() - 1].is_ascii_digit() {
                let yy = parse_2digit_u(b, b.len() - 2);
                let yr = if yy <= 30 { 2000 + yy } else { 1900 + yy };
                return Some((format!("{yr:04}-01-01"), DatePrecision::YearOnly));
            }
        }
        // 4-digit year anywhere (word-bounded)
        if let Some(yr) = find_4year_str(part) {
            return Some((format!("{yr:04}-01-01"), DatePrecision::YearOnly));
        }
    }

    // Pass 4: standalone 6-digit folder (yyMMdd)
    for part in parts.iter().rev() {
        let b = part.as_bytes();
        if b.len() == 6 && all_digit(b, 0, 6) {
            let yy = parse_2digit_u(b, 0);
            let mm = parse_2digit_u(b, 2);
            let dd = parse_2digit_u(b, 4);
            if mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 {
                let yr = if yy <= 30 { 2000 + yy } else { 1900 + yy };
                return Some((format!("{yr:04}-{mm:02}-{dd:02}"), DatePrecision::Full));
            }
        }
    }

    None
}

/// Find a 4-digit year (1900–2099) anywhere in string, word-bounded.
fn find_4year_str(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    let mut i = 0;
    while i + 4 <= b.len() {
        let before_ok = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if before_ok && is_4year(b, i) {
            let after_ok = i + 4 >= b.len() || !b[i + 4].is_ascii_digit();
            if after_ok {
                return Some(parse_4year(b, i));
            }
        }
        i += 1;
    }
    None
}

// ── Small bit-helpers ─────────────────────────────────────────────────────────

fn all_digit(b: &[u8], start: usize, len: usize) -> bool {
    b[start..start + len].iter().all(|x| x.is_ascii_digit())
}

fn is_4year(b: &[u8], i: usize) -> bool {
    if i + 4 > b.len() {
        return false;
    }
    (b[i] == b'1' || b[i] == b'2') && all_digit(b, i, 4)
}

fn is_sep(b: &[u8], i: usize) -> bool {
    b.get(i).map(|&c| c == b'-' || c == b'_').unwrap_or(false)
}

fn parse_4year(b: &[u8], i: usize) -> u32 {
    ((b[i] - b'0') as u32) * 1000
        + ((b[i + 1] - b'0') as u32) * 100
        + ((b[i + 2] - b'0') as u32) * 10
        + ((b[i + 3] - b'0') as u32)
}

fn parse_2digit_u(b: &[u8], i: usize) -> u32 {
    ((b[i] - b'0') as u32) * 10 + ((b[i + 1] - b'0') as u32)
}

// ── EXIF date via kamadak-exif ────────────────────────────────────────────────

/// Extract date from EXIF metadata.  Reads `DateTimeOriginal`, then `DateTime`.
/// Returns `"YYYY-MM-DD"` or `None`.
pub fn exif_date(path: &Path) -> Option<String> {
    use exif::{In, Tag};
    let file = fs::File::open(path).ok()?;
    let mut buf = BufReader::new(file);
    let exif = exif::Reader::new()
        .read_from_container(&mut buf)
        .ok()?;

    for tag in &[Tag::DateTimeOriginal, Tag::DateTime, Tag::DateTimeDigitized] {
        if let Some(field) = exif.get_field(*tag, In::PRIMARY) {
            let raw = field.display_value().to_string();
            if let Some(d) = parse_exif_date_str(&raw) {
                return Some(d);
            }
        }
    }
    None
}

fn parse_exif_date_str(raw: &str) -> Option<String> {
    // EXIF format: "YYYY:MM:DD HH:MM:SS"
    let s = raw.trim();
    if s.len() < 10 {
        return None;
    }
    let b = s.as_bytes();
    if b[4] == b':' && b[7] == b':' && all_digit(b, 0, 4) && all_digit(b, 5, 2) && all_digit(b, 8, 2) {
        let yr = parse_4year(b, 0);
        let mm = parse_2digit_u(b, 5);
        let dd = parse_2digit_u(b, 8);
        if yr >= 1900 && yr <= 2050 && mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 {
            return Some(format!("{yr:04}-{mm:02}-{dd:02}"));
        }
    }
    None
}

// ── XMP sidecar ───────────────────────────────────────────────────────────────

/// Look for a paired `.xmp` or `.XMP` sidecar and extract `photoshop:DateCreated`
/// or `xmp:CreateDate`.  Returns `"YYYY-MM-DD"` or `None`.

// ── Format / extension mismatch detection ────────────────────────────────────

/// Returns the canonical extension for the detected format if the file's magic
/// bytes indicate a format different from `claimed_ext`.  Returns `None` when
/// they match or the format cannot be detected (e.g. RAW, video).
pub(crate) fn detect_wrong_ext(path: &Path, claimed_ext: &str) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut header = [0u8; 16];
    let n = f.read(&mut header).ok()?;
    if n < 4 {
        return None;
    }
    let fmt = image::guess_format(&header[..n]).ok()?;
    let valid_exts = fmt.extensions_str();
    let claimed = claimed_ext.to_lowercase();
    if valid_exts.iter().any(|&e| e == claimed) {
        None
    } else {
        Some(valid_exts[0].to_string())
    }
}

// ── SHA-256 ───────────────────────────────────────────────────────────────────

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Compute SHA-256 of the first `chunk_bytes` bytes of `path`.
///
/// If the file is shorter than `chunk_bytes` the whole file is hashed.
/// This is the fast duplicate-probe hash: reading only a small prefix over a
/// slow MTP connection is far cheaper than reading the entire file.
pub fn partial_hash_chunk(path: &Path, chunk_bytes: usize) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut remaining = chunk_bytes;
    let mut buf = [0u8; 65536];
    while remaining > 0 {
        let want = remaining.min(buf.len());
        let n = file.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n;
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Chunk size used for the partial-hash duplicate probe (256 KiB).
///
/// Large enough to distinguish real camera files (JPEG DCT blocks, MP4 moov
/// atoms) while remaining fast over slow MTP connections (~0.25 s at 1 MB/s).
pub const PARTIAL_HASH_BYTES: usize = 256 * 1024;

// ── Import types ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ImportStatus {
    Pending,
    Duplicate,
    Skipped,    // quality variant
    UnknownDate,
}

#[derive(Clone, Debug)]
pub struct ImportEntry {
    pub source_path: PathBuf,
    pub content_hash: String,
    pub partial_hash: String,
    pub file_size: u64,
    pub ext: String,
    /// `Some("jpg")` when magic bytes reveal a format that differs from `ext`.
    pub wrong_ext: Option<String>,
    pub derived_date: Option<String>,
    pub date_source: String,
    pub exif_raw: Option<String>,
    pub derived_slug: Option<String>,
    pub caption_slug: Option<String>,
    pub slug_source: String,
    pub counter: Option<u32>,
    pub target_path: Option<String>,
    pub status: ImportStatus,
}

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub copied: usize,
    pub skipped_dup: usize,
    pub errors: usize,
    pub unknown_date: usize,
}

// ── Main scan loop ────────────────────────────────────────────────────────────

/// Scan `source_root` for media files.
///
/// Returns one `ImportEntry` per media file found. Deduplication against
/// already-imported files is deferred to the execute phase (no full file reads
/// happen during scan — only metadata and a 16-byte magic header per file).
///
/// Returns one `ImportEntry` per media file found (including skipped, unknown-date, etc.).
pub fn scan_source(
    source_root: &Path,
    progress_cb: &mut dyn FnMut(usize, &str),
) -> Result<Vec<ImportEntry>> {
    let mut entries: Vec<ImportEntry> = Vec::new();
    let mut found = 0usize;

    // Collect quality-variant dirs first
    let mut variant_dirs: HashSet<PathBuf> = HashSet::new();
    for entry in walkdir::WalkDir::new(source_root).min_depth(1).max_depth(5) {
        let e = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if e.file_type().is_dir() {
            if let Some(name) = e.file_name().to_str() {
                if is_quality_variant(name) {
                    variant_dirs.insert(e.path().to_path_buf());
                }
            }
        }
    }

    // Walk the source tree
    let walker = walkdir::WalkDir::new(source_root)
        .min_depth(1)
        .follow_links(false)
        .sort_by_file_name();

    for entry in walker {
        let e = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if e.file_type().is_dir() {
            continue;
        }
        let path = e.path();
        let filename = match path.file_name().and_then(OsStr::to_str) {
            Some(n) => n,
            None => continue,
        };

        // Skip macOS AppleDouble forks
        if filename.starts_with("._") {
            continue;
        }
        // Skip hidden files and always-skip list
        if filename.starts_with('.') {
            continue;
        }
        let fn_lower = filename.to_lowercase();
        if ALWAYS_SKIP.iter().any(|&s| s == fn_lower) {
            continue;
        }
        // Skip thumbnail files
        if THUMBNAIL_PREFIXES.iter().any(|&p| filename.starts_with(p)) {
            continue;
        }
        // Skip XMP sidecars
        if fn_lower.ends_with(".xmp") {
            continue;
        }

        let ext = path.extension().and_then(OsStr::to_str).unwrap_or("").to_lowercase();

        // Check if in a quality-variant dir
        let in_variant = variant_dirs.iter().any(|vd| {
            path.starts_with(vd)
        });

        if in_variant {
            found += 1;
            entries.push(ImportEntry {
                source_path: path.to_path_buf(),
                content_hash: String::new(),
                partial_hash: String::new(),
                file_size: 0,
                ext: ext.clone(),
                wrong_ext: None,
                derived_date: None,
                date_source: "skipped".into(),
                exif_raw: None,
                derived_slug: None,
                caption_slug: None,
                slug_source: "none".into(),
                counter: None,
                target_path: None,
                status: ImportStatus::Skipped,
            });
            continue;
        }

        // Skip junk parent folders
        let in_junk = path.ancestors()
            .skip(1) // skip file itself
            .take_while(|p| *p != source_root)
            .any(|p| {
                p.file_name()
                    .and_then(OsStr::to_str)
                    .map(|n| is_junk_folder(n) && !is_quality_variant(n))
                    .unwrap_or(false)
            });
        // Note: we still process files inside junk folders (DCIM, etc.) —
        // they just get no slug (fall back to day-prefix). Only quality-variant
        // dirs are fully skipped.

        if !is_supported_ext(&ext) {
            continue;
        }

        found += 1;
        let filename_str = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        progress_cb(found, filename_str);

        // Use walkdir's cached metadata — no extra stat() call over MTP.
        let meta = e.metadata().ok();
        let file_size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime_d = meta.as_ref().and_then(|m| {
            m.modified().ok().and_then(|st| {
                use std::time::UNIX_EPOCH;
                let secs = st.duration_since(UNIX_EPOCH).ok()?.as_secs();
                Some(secs_to_date(secs))
            })
        });

        // No file-content reads during scan (EXIF/XMP/magic header all require
        // opening the file, which is expensive over MTP and slow USB mounts).
        // Date is derived from filename patterns, folder path, and OS mtime only.
        // EXIF will be re-read during execute for DB storage if needed.

        // Folder path components (relative to source_root)
        let dir = path.parent().unwrap_or(source_root);
        let rel = dir.strip_prefix(source_root).unwrap_or(dir);
        let folder_parts: Vec<&str> = rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect();

        // Immediate folder for dump-folder detection
        let immediate_folder = folder_parts.last().copied().unwrap_or("");

        // Date derivation
        let folder_result = if is_dump_folder(immediate_folder) {
            None
        } else {
            parse_date_folders(&folder_parts)
        };

        let (derived_date, date_source) = derive_date(
            folder_result,
            None,   // EXIF deferred — no file read during scan
            filename,
            mtime_d.as_deref(),
        );

        // Slug derivation
        let (derived_slug, caption_slug, slug_source) =
            derive_slug(path, source_root, filename, in_junk);

        // Status (dedup against DB is deferred to execute phase)
        let status = if derived_date.is_none() {
            ImportStatus::UnknownDate
        } else {
            ImportStatus::Pending
        };

        entries.push(ImportEntry {
            source_path: path.to_path_buf(),
            content_hash: String::new(),
            partial_hash: String::new(),
            file_size,
            ext,
            wrong_ext: None,   // magic-byte check deferred to execute
            derived_date,
            date_source: date_source.to_string(),
            exif_raw: None,    // EXIF deferred to execute
            derived_slug,
            caption_slug,
            slug_source,
            counter: None,
            target_path: None,
            status,
        });
    }

    Ok(entries)
}

fn derive_date(
    folder_result: Option<(String, DatePrecision)>,
    exif_raw: Option<&str>,
    filename: &str,
    mtime_d: Option<&str>,
) -> (Option<String>, &'static str) {
    if let Some((folder_date, prec)) = folder_result {
        if prec == DatePrecision::YearOnly {
            if let Some(ex) = exif_raw {
                if ex.starts_with(&folder_date[..4]) {
                    return (Some(ex.to_string()), "exif");
                }
            }
            if let Some(mt) = mtime_d {
                if mt.starts_with(&folder_date[..4]) {
                    return (Some(mt.to_string()), "file");
                }
            }
            return (Some(folder_date), "folder");
        }
        return (Some(folder_date), "folder");
    }
    if let Some(ex) = exif_raw {
        return (Some(ex.to_string()), "exif");
    }
    if let Some(d) = parse_date_filename(filename) {
        return (Some(d), "filename");
    }
    if let Some(mt) = mtime_d {
        let yr: u32 = mt[..4].parse().unwrap_or(0);
        if yr >= 1990 && yr <= 2050 {
            return (Some(mt.to_string()), "file");
        }
    }
    (None, "unknown")
}

fn derive_slug(
    file_path: &Path,
    source_root: &Path,
    filename: &str,
    _in_junk: bool,
) -> (Option<String>, Option<String>, String) {
    let (base, _) = split_ext(filename);

    if is_camera_code(filename) {
        let fs = slug_from_path(file_path, source_root);
        let slug_src = if fs.is_some() { "folder" } else { "none" };
        // FAT32 code: extract alpha prefix if whitelisted (e.g. PAB2 → "pab")
        let cap = if is_camera_fat32(base) {
            let alpha: String = base.chars().take_while(|c| c.is_alphabetic()).collect();
            let alpha_lower = alpha.to_lowercase();
            if SHORT_SLUG_WHITELIST.iter().any(|&w| w == alpha_lower) {
                derive_caption_slug(filename, fs.as_deref())
            } else {
                None
            }
        } else {
            None
        };
        return (fs, cap, slug_src.to_string());
    }

    // Detect caption filenames (multiple meaningful words)
    let words: Vec<&str> = base
        .split(|c: char| c == '-' || c == '_' || c == ' ')
        .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit()) && w.len() > 1)
        .collect();
    let is_caption = words.len() >= 2;

    if is_caption {
        let fs = slug_from_path(file_path, source_root);
        let cap = if let Some(ref folder_s) = fs {
            derive_caption_slug(filename, Some(folder_s))
        } else {
            extract_slug(base)
        };
        let slug_src = if fs.is_some() { "folder" } else { "none" };
        return (fs, cap, slug_src.to_string());
    }

    // Single meaningful word
    let file_slug = extract_slug(base);
    if let Some(ref fs2) = file_slug {
        let fs_folder = slug_from_path(file_path, source_root);
        if let Some(ref ff) = fs_folder {
            // Strip tokens duplicating folder slug from this single-word caption
            let cap = derive_caption_slug(filename, Some(ff));
            return (Some(ff.clone()), cap, "folder".to_string());
        } else {
            // No folder context: single word becomes caption only
            return (None, Some(fs2.clone()), "filename".to_string());
        }
    }

    // Fallback: folder slug only
    let fs3 = slug_from_path(file_path, source_root);
    let slug_src = if fs3.is_some() { "folder" } else { "none" };
    (fs3, None, slug_src.to_string())
}

/// Convert Unix seconds to "YYYY-MM-DD" string (public for use in app.rs).
pub fn secs_to_date_pub(secs: u64) -> String {
    secs_to_date(secs)
}

/// Convert Unix seconds to "YYYY-MM-DD" string.
fn secs_to_date(secs: u64) -> String {
    // Simple Julian-day calculation; good enough for 1970–2100.
    let days_since_epoch = secs / 86400;
    // Days from 1970-01-01 to year/month/day
    let days = days_since_epoch as i64 + 2440588; // Julian day number for 1970-01-01 = 2440588
    let a = days + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (b * 146097) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let day = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = b * 100 + d - 4800 + m / 10;
    format!("{year:04}-{month:02}-{day:02}")
}

// ── Date consensus post-processing ───────────────────────────────────────────

/// Normalize per-folder date months when the folder gives only a year.
///
/// Three-tier priority (per source directory):
/// 1. EXIF majority month → overrides `file` + `folder-year-only` rows
/// 2. mtime majority month → overrides `folder-year-only` rows + normalises mtime outliers
/// 3. No reference → leave as-is
pub fn apply_folder_mtime_consensus(entries: &mut Vec<ImportEntry>) {
    use std::collections::BTreeMap;

    // Group entries by source directory
    let mut folder_map: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.status == ImportStatus::Pending || e.status == ImportStatus::UnknownDate {
            let dir = e.source_path.parent().unwrap_or(Path::new(".")).to_path_buf();
            folder_map.entry(dir).or_default().push(i);
        }
    }

    for (_dir, indices) in &folder_map {
        let mut exif_months: Vec<String> = Vec::new(); // "YYYY-MM"
        let mut file_months: Vec<(usize, String)> = Vec::new(); // (entry_idx, "YYYY-MM")
        let mut fonly_rows: Vec<(usize, String)> = Vec::new(); // folder-year-only rows

        for &i in indices {
            let e = &entries[i];
            let date = match &e.derived_date {
                Some(d) => d,
                None => continue,
            };
            let ym = date[..7.min(date.len())].to_string();
            match e.date_source.as_str() {
                "exif" => exif_months.push(ym),
                "file" => file_months.push((i, ym)),
                "folder" if date.ends_with("-01-01") => fonly_rows.push((i, ym)),
                _ => {}
            }
        }

        if !exif_months.is_empty() {
            // EXIF majority wins
            let majority = majority_month(&exif_months);
            for (i, ym) in &file_months {
                if ym != &majority {
                    entries[*i].derived_date = Some(format!("{majority}-01"));
                }
            }
            for (i, ym) in &fonly_rows {
                if ym != &majority {
                    entries[*i].derived_date = Some(format!("{majority}-01"));
                }
            }
        } else if !file_months.is_empty() {
            // mtime majority wins
            let all_months: Vec<String> = file_months.iter().map(|(_, m)| m.clone()).collect();
            let majority = majority_month(&all_months);
            // Normalize folder-year-only rows
            for (i, ym) in &fonly_rows {
                if ym != &majority {
                    entries[*i].derived_date = Some(format!("{majority}-01"));
                }
            }
            // Normalize mtime outliers only when all mtime files share the same year
            let years: HashSet<&str> = all_months.iter().map(|m| &m[..4]).collect();
            if years.len() == 1 {
                for (i, ym) in &file_months {
                    if ym != &majority {
                        entries[*i].derived_date = Some(format!("{majority}-01"));
                    }
                }
            }
        }
    }
}

fn majority_month(months: &[String]) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for m in months {
        *counts.entry(m.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(m, _)| m.to_string())
        .unwrap_or_default()
}

// ── Counter assignment ────────────────────────────────────────────────────────

/// Assign 4-digit counters to all `Pending` entries, setting `target_path`.
///
/// Counter resets to `0001` for each new date-prefix (`yyyy-MM-slug` or `yyyy-MM-DD`).
/// Checks existing DB max counters AND filesystem to avoid collisions with already-moved files.
pub fn assign_counters(
    entries: &mut Vec<ImportEntry>,
    target_root: &Path,
    conn: &rusqlite::Connection,
) -> Result<()> {
    // Sort pending entries by derived_date, derived_slug, source_path for stable ordering
    let pending_indices: Vec<usize> = {
        let mut v: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.status == ImportStatus::Pending)
            .map(|(i, _)| i)
            .collect();
        v.sort_by(|&a, &b| {
            let ea = &entries[a];
            let eb = &entries[b];
            let da = ea.derived_date.as_deref().unwrap_or("");
            let db = eb.derived_date.as_deref().unwrap_or("");
            da.cmp(db)
                .then(ea.derived_slug.as_deref().unwrap_or("").cmp(eb.derived_slug.as_deref().unwrap_or("")))
                .then(ea.source_path.cmp(&eb.source_path))
        });
        v
    };

    let mut counters: HashMap<String, u32> = HashMap::new(); // date_prefix → next counter
    let mut seen_paths: HashSet<String> = HashSet::new(); // collision guard for no-slug+caption

    for idx in pending_indices {
        let e = &entries[idx];
        let date = match &e.derived_date {
            Some(d) => d.clone(),
            None => continue,
        };
        let year = &date[..4];
        let month = &date[5..7];
        let day = &date[8..10];
        let slug = e.derived_slug.clone();

        let date_prefix = if let Some(ref s) = slug {
            format!("{year}-{month}-{s}")
        } else {
            format!("{year}-{month}-{day}")
        };

        if !counters.contains_key(&date_prefix) {
            // Get max from DB
            let pattern = format!("{year}/{date_prefix}-%");
            let db_max: u32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(counter), 0) FROM media WHERE target_path LIKE ?1 AND status='moved'",
                    rusqlite::params![pattern],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap_or(0);

            // Get max from filesystem
            let mut fs_max: u32 = 0;
            let yr_dir = target_root.join(year);
            if let Ok(rd) = fs::read_dir(&yr_dir) {
                for entry in rd.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(&format!("{date_prefix}-")) {
                            let rest = &name[date_prefix.len() + 1..];
                            // Counter is the first 4-digit group
                            let digits: String = rest.chars().take(4).collect();
                            if digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()) {
                                let n: u32 = digits.parse().unwrap_or(0);
                                if n > fs_max {
                                    fs_max = n;
                                }
                            }
                        }
                    }
                }
            }

            counters.insert(date_prefix.clone(), db_max.max(fs_max) + 1);
        }

        let counter = counters[&date_prefix];
        *counters.get_mut(&date_prefix).unwrap() += 1;

        let ext = &entries[idx].ext;
        let ext_with_dot = format!(".{ext}");
        let cap = entries[idx].caption_slug.clone();

        let target_path = if slug.is_none() {
            if let Some(ref c) = cap {
                // No event slug + caption: prefer without counter; add counter only on collision
                let plain = format!("{year}/{date_prefix}-{c}{ext_with_dot}");
                if seen_paths.contains(&plain) {
                    let p = format!("{year}/{date_prefix}-{c}-{counter:04}{ext_with_dot}");
                    seen_paths.insert(p.clone());
                    p
                } else {
                    seen_paths.insert(plain.clone());
                    plain
                }
            } else {
                format!("{year}/{date_prefix}-{counter:04}{ext_with_dot}")
            }
        } else if let Some(ref c) = cap {
            format!("{year}/{date_prefix}-{counter:04}-{c}{ext_with_dot}")
        } else {
            format!("{year}/{date_prefix}-{counter:04}{ext_with_dot}")
        };

        entries[idx].counter = Some(counter);
        entries[idx].target_path = Some(target_path);
    }

    Ok(())
}

// ── Execute import ────────────────────────────────────────────────────────────

/// Copy all `Pending` entries to target, insert/update DB rows, assign import tag.
///
/// Deduplication happens here: each source file is first probed with a fast
/// partial hash (first 256 KiB).  If the `(file_size, partial_hash)` pair
/// already exists in the DB the file is skipped immediately — no full copy
/// over the slow MTP link.  Only files that pass the probe are
/// stream-copied while their full SHA-256 is computed in one pass.
///
/// `import_date` — today's date as `"YYYY-MM-DD"`, used for the import tag name.
/// `progress_cb(done, total)` — called after each successful copy.
pub fn execute_import(
    entries: &[ImportEntry],
    target_root: &Path,
    conn: &mut rusqlite::Connection,
    import_date: &str,
    progress_cb: &mut dyn FnMut(usize, usize, &str, &ImportSummary) -> bool,
) -> Result<ImportSummary> {
    // Backfill and load are separated: run `migrate-partial-hashes` once
    // (or let the first import handle missing entries gracefully via NULL exclusion).
    let existing_hashes = load_existing_hashes(conn)?;

    // Work with owned clones so we can fill in hashes after streaming.
    let mut pending: Vec<ImportEntry> = entries
        .iter()
        .filter(|e| e.status == ImportStatus::Pending && e.target_path.is_some())
        .cloned()
        .collect();

    let total = pending.len();
    let mut summary = ImportSummary::default();
    summary.unknown_date = entries
        .iter()
        .filter(|e| e.status == ImportStatus::UnknownDate)
        .count();
    // No Duplicate entries from scan any more; they'll be counted below.

    // Within-batch dedup: track (file_size, partial_hash) of files imported
    // in this session so that later duplicates are caught without a full copy.
    let mut seen_hashes: HashMap<(u64, String), PathBuf> = HashMap::new();

    let mut imported_ids: Vec<String> = Vec::new();
    let mut attempted: usize = 0;

    for entry in &mut pending {
        attempted += 1;
        let rel_tgt = entry.target_path.as_ref().unwrap();
        let abs_tgt = target_root.join(rel_tgt);

        // Announce which file is starting — visible immediately, before the
        // (potentially slow) copy begins. Returns false if the user aborted.
        let current_file = entry
            .source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let keep_going = progress_cb(attempted, total, &current_file, &summary);
        if !keep_going {
            // User aborted — assign tag to files already processed this session.
            break;
        }

        // ── Fast duplicate probe ──────────────────────────────────────────────
        // Read only the first 256 KiB from the source (over MTP).  This is
        // orders of magnitude cheaper than a full copy for large files.
        let probe_key: Option<(u64, String)> = partial_hash_chunk(&entry.source_path, PARTIAL_HASH_BYTES)
            .ok()
            .map(|ph| (entry.file_size, ph));

        if let Some(ref key) = probe_key {
            if existing_hashes.contains_key(key) {
                // Confirmed duplicate — skip without touching the target FS.
                summary.skipped_dup += 1;
                continue;
            }
            if let Some(first_seen) = seen_hashes.get(key) {
                let name = first_seen.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                eprintln!("import: batch duplicate of {name}, skipping {}", entry.source_path.display());
                summary.skipped_dup += 1;
                continue;
            }
        }

        // If target already exists on disk, check whether it's the same file.
        // After backfill, any previously-imported file will have been caught by
        // the probe above.  A surviving abs_tgt.exists() means a genuine name
        // collision with an unrelated file — treat as an error.
        if abs_tgt.exists() {
            eprintln!("import: conflict at {} — file exists, skipping", abs_tgt.display());
            summary.errors += 1;
            continue;
        }

        if let Some(parent) = abs_tgt.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }

        // Stream-copy source → dest while computing full SHA-256 in one pass.
        // write_all() returning Ok is sufficient integrity guarantee on modern OS/FS;
        // we skip a destination re-read to avoid doubling I/O for large video files.
        let (source_hash, _) = match stream_copy_and_hash(&entry.source_path, &abs_tgt) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("import: copy error {}: {e}", entry.source_path.display());
                fs::remove_file(&abs_tgt).ok();
                summary.errors += 1;
                continue;
            }
        };

        entry.content_hash = source_hash;
        if let Some(key) = probe_key {
            entry.partial_hash = key.1.clone();
            seen_hashes.insert(key, entry.source_path.clone());
        }

        // Read EXIF from the locally-copied destination — free since dest is on local disk.
        // This is the only per-file extra open during execute (source is not re-opened).
        entry.exif_raw = exif_date(&abs_tgt);

        let id = upsert_media_row(conn, entry, rel_tgt, import_date)?;
        imported_ids.push(id);
        summary.copied += 1;
        // Completion update — false return value means abort (handled at next iteration start).
        progress_cb(attempted, total, &current_file, &summary);
    }

    // Assign import tag to all imported files
    if !imported_ids.is_empty() {
        assign_import_tag(conn, &imported_ids, import_date)?;
    }

    Ok(summary)
}

/// Stream-copy `src` to `dst` while computing the SHA-256 of `src` in one pass.
/// Returns `(hex_sha256, bytes_copied)`.
fn stream_copy_and_hash(src: &Path, dst: &Path) -> Result<(String, u64)> {
    use std::io::Write;
    let mut hasher = Sha256::new();
    let mut src_file =
        fs::File::open(src).with_context(|| format!("open {}", src.display()))?;
    let mut dst_file =
        fs::File::create(dst).with_context(|| format!("create {}", dst.display()))?;
    let mut buf = [0u8; 65536];
    let mut total: u64 = 0;
    loop {
        let n = src_file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        dst_file.write_all(&buf[..n])?;
        total += n as u64;
    }
    Ok((hex::encode(hasher.finalize()), total))
}

fn upsert_media_row(
    conn: &rusqlite::Connection,
    entry: &ImportEntry,
    rel_tgt: &str,
    _import_date: &str,
) -> Result<String> {
    use rusqlite::OptionalExtension;
    let src = entry.source_path.to_str().unwrap_or("");

    // Reuse existing id if source_path already in DB
    let existing_id: Option<String> = conn
        .query_row("SELECT id FROM media WHERE source_path = ?1", [src], |row| row.get(0))
        .optional()?;

    let id = existing_id.unwrap_or_else(|| uuid_v4());

    conn.execute(
        "INSERT OR REPLACE INTO media \
         (id, source_path, target_path, content_hash, partial_hash, file_size, ext, \
          exif_date, derived_date, date_source, derived_slug, caption_slug, \
          slug_source, counter, status, scanned_at, moved_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'moved',datetime('now'),datetime('now'))",
        rusqlite::params![
            id,
            src,
            rel_tgt,
            entry.content_hash,
            if entry.partial_hash.is_empty() { None } else { Some(&entry.partial_hash) },
            entry.file_size as i64,
            entry.ext,
            entry.exif_raw,
            entry.derived_date,
            entry.date_source,
            entry.derived_slug,
            entry.caption_slug,
            entry.slug_source,
            entry.counter.map(|c| c as i64),
        ],
    )?;
    Ok(id)
}

fn assign_import_tag(
    conn: &mut rusqlite::Connection,
    media_ids: &[String],
    import_date: &str,
) -> Result<()> {
    // Tag name: "import-YY-MM-DD" (short year), type "mex"
    // If the day's base tag already exists, append _2, _3, … so each
    // import session on the same day gets its own unique tag.
    let yy = &import_date[2..4];
    let mm = &import_date[5..7];
    let dd = &import_date[8..10];
    let base = format!("import-{yy}-{mm}-{dd}");

    // Find all existing mex tags that match this day's base or base_N.
    // Use a block so `stmt` is dropped before we need a mutable borrow of conn below.
    let like_pattern = format!("{base}_%");
    let existing_names: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT name FROM tags WHERE type = 'mex' AND (name = ?1 COLLATE NOCASE OR name LIKE ?2 COLLATE NOCASE)",
        )?;
        let names: Vec<String> = stmt
            .query_map(rusqlite::params![&base, &like_pattern], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        names
    };

    // Determine the next suffix: base has implicit index 1; base_2, base_3 follow.
    let max_index = existing_names.iter().fold(
        if existing_names.is_empty() { 0u32 } else { 1u32 },
        |acc, name| {
            let suffix = name[base.len()..].to_ascii_lowercase();
            if let Some(n) = suffix.strip_prefix('_').and_then(|s| s.parse::<u32>().ok()) {
                acc.max(n)
            } else {
                acc
            }
        },
    );

    let tag_name = if max_index == 0 {
        base.clone()
    } else {
        format!("{base}_{}", max_index + 1)
    };

    conn.execute(
        "INSERT INTO tags (name, type) VALUES (?1, 'mex')",
        [&tag_name],
    )?;
    let tag_id = conn.last_insert_rowid();

    let tx = conn.transaction()?;
    for mid in media_ids {
        tx.execute(
            "INSERT OR IGNORE INTO media_tags (media_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![mid, tag_id],
        )?;
    }
    tx.commit()?;

    Ok(())
}

/// Generate a UUID v4 (random).
fn uuid_v4() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};
    // Not cryptographically random, but sufficient for DB row IDs.
    let mut h = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        .hash(&mut h);
    std::thread::current().id().hash(&mut h);
    let v1 = h.finish();
    let mut h2 = DefaultHasher::new();
    v1.hash(&mut h2);
    let v2 = h2.finish();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (v1 >> 32) as u32,
        (v1 >> 16) as u16 & 0xffff,
        (v1 & 0x0fff) as u16,
        ((v2 >> 48) as u16 & 0x3fff) | 0x8000,
        v2 & 0x0000_ffff_ffff_ffff,
    )
}

// ── Public helper: load existing hashes from DB ───────────────────────────────

/// Backfill `partial_hash` for any DB rows that are missing it.
///
/// Reads the first `PARTIAL_HASH_BYTES` from each target file on local disk
/// (fast — target is local storage, not MTP) and persists the result.  Runs
/// once at the start of `execute_import`; subsequent imports are instant
/// because the column will already be populated.
pub fn ensure_partial_hashes(conn: &rusqlite::Connection, target_root: &Path) -> Result<()> {
    // Collect rows that still need a partial_hash.
    let mut stmt = conn.prepare(
        "SELECT id, file_size, target_path FROM media \
         WHERE status='moved' AND partial_hash IS NULL AND target_path IS NOT NULL",
    )?;
    let rows: Vec<(String, i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (id, _size, rel_path) in rows {
        let abs_path = target_root.join(&rel_path);
        if let Ok(phash) = partial_hash_chunk(&abs_path, PARTIAL_HASH_BYTES) {
            conn.execute(
                "UPDATE media SET partial_hash = ?1 WHERE id = ?2",
                rusqlite::params![phash, id],
            )?;
        }
        // Rows whose target file is missing are left as NULL — harmless; they
        // simply won't participate in the fast dedup set.
    }
    Ok(())
}

/// Load the dedup set from the DB.
///
/// Returns a map keyed by `(file_size, partial_hash)` → `target_path`.
/// Rows where `partial_hash` is NULL are excluded here; they are handled by
/// `ensure_partial_hashes`, which backfills them before the copy loop runs.
pub fn load_existing_hashes(conn: &rusqlite::Connection) -> Result<HashMap<(u64, String), String>> {
    let mut stmt = conn.prepare(
        "SELECT file_size, partial_hash, COALESCE(target_path, '') \
         FROM media \
         WHERE status='moved' AND partial_hash IS NOT NULL AND file_size IS NOT NULL",
    )?;
    let map: Result<HashMap<(u64, String), String>, _> = stmt
        .query_map([], |row| {
            let size: i64 = row.get(0)?;
            let phash: String = row.get(1)?;
            let tpath: String = row.get(2)?;
            Ok(((size as u64, phash), tpath))
        })?
        .collect();
    Ok(map?)
}

// ── Message type for background thread ───────────────────────────────────────

pub enum ImportMsg {
    ScanProgress { count: usize, current_file: String },
    ScanDone(Vec<ImportEntry>),
    ScanError(String),
    CopyProgress { done: usize, total: usize, current_file: String, copied: usize, skipped_dup: usize, errors: usize },
    CopyDone(ImportSummary),
    CopyError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_slug_basic() {
        assert_eq!(extract_slug("paris 2000"), Some("paris".into()));
        assert_eq!(extract_slug("kroatien split"), Some("kroatien-split".into()));
        assert_eq!(extract_slug("IMG_1234.jpg"), None);
    }

    #[test]
    fn test_extract_slug_german() {
        // ö → oe, ü → ue
        assert_eq!(extract_slug("Österreich").map(|s| s.contains("oesterreich")), Some(true));
    }

    #[test]
    fn test_is_camera_code() {
        assert!(is_camera_code("IMG_1234.jpg"));
        assert!(is_camera_code("DSC0042.JPG"));
        assert!(is_camera_code("PARIS1.JPG"));
        assert!(!is_camera_code("paris_roland.jpg"));
    }

    #[test]
    fn test_is_junk_folder() {
        assert!(is_junk_folder("100CANON"));
        assert!(is_junk_folder("DCIM"));
        assert!(is_junk_folder("MISC"));
        assert!(is_junk_folder("web"));
        assert!(is_junk_folder("others"));
        assert!(!is_junk_folder("paris"));
    }

    #[test]
    fn test_is_quality_variant() {
        assert!(is_quality_variant("lignano 2001 - small"));
        assert!(is_quality_variant("web"));
        assert!(is_quality_variant("thumbnails"));
        assert!(!is_quality_variant("paris"));
    }

    #[test]
    fn test_parse_mobile_date() {
        assert_eq!(parse_mobile_date("09-07-06_1915"), Some("2006-07-09".into()));
        assert_eq!(parse_mobile_date("31-12-99_2359"), Some("1999-12-31".into()));
        assert_eq!(parse_mobile_date("IMG_1234"), None);
    }

    #[test]
    fn test_parse_german_folder_date() {
        assert_eq!(parse_german_folder_date("1. April 2019"), Some("2019-04-01".into()));
        assert_eq!(parse_german_folder_date("15. August 2017"), Some("2017-08-15".into()));
    }

    #[test]
    fn test_parse_date_filename_yyyymmdd() {
        assert_eq!(parse_date_filename("20190615_photo.jpg"), Some("2019-06-15".into()));
        assert_eq!(parse_date_filename("2019-06-15 holiday.jpg"), Some("2019-06-15".into()));
    }

    #[test]
    fn test_parse_date_folders_full() {
        let parts = ["paris 2000", "summer"];
        let r = parse_date_folders(&parts);
        assert_eq!(r.as_ref().map(|(_, p)| p), Some(&DatePrecision::YearOnly));
        assert_eq!(r.map(|(d, _)| d), Some("2000-01-01".into()));
    }

    #[test]
    fn test_parse_date_folders_full_date() {
        let parts = ["20190615"];
        let r = parse_date_folders(&parts);
        assert_eq!(r.map(|(d, _)| d), Some("2019-06-15".into()));
    }

    #[test]
    fn test_is_dump_folder() {
        assert!(is_dump_folder("2024-01-11-Bilder"));
        assert!(is_dump_folder("2024-Videos"));
        assert!(!is_dump_folder("paris 2000"));
    }

    #[test]
    fn test_secs_to_date_epoch() {
        // 1970-01-01
        assert_eq!(secs_to_date(0), "1970-01-01");
        // 2019-06-15 = (2019-1970)*365.25*86400 ≈ known
        // 2001-09-01 00:00:00 UTC = 999302400
        assert_eq!(secs_to_date(999302400), "2001-09-01");
    }

    #[test]
    fn test_exif_date_str_parses() {
        assert_eq!(parse_exif_date_str("2022:04:18 14:30:00"), Some("2022-04-18".into()));
        assert_eq!(parse_exif_date_str("0000:00:00 00:00:00"), None);
    }

    #[test]
    fn test_derive_caption_slug_strips_folder_prefix() {
        // folder slug "paris", filename "paris_roland_party" → strip "paris" → "roland-party"
        let cap = derive_caption_slug("paris_roland_party.jpg", Some("paris"));
        assert_eq!(cap, Some("roland-party".into()));
    }

    #[test]
    fn test_uuid_stem_detection() {
        assert!(is_uuid_stem("4b0bdd13-72f4-4f25-99cf-6f4474f6d447"));
        assert!(!is_uuid_stem("not-a-uuid"));
        assert_eq!(parse_date_filename("4b0bdd13-72f4-4f25-99cf-6f4474f6d447.jpg"), None);
        // hex chars inside UUID should not produce false date
        assert_eq!(extract_slug("4b0bdd13-72f4-4f25-99cf-6f4474f6d447"), None);
    }

    #[test]
    fn test_picsart_date() {
        // Picsart_YY-MM-DD_HH-MM-SS-mmm
        assert_eq!(parse_date_filename("Picsart_24-11-24_00-15-22-784.jpg"), Some("2024-11-24".into()));
        assert_eq!(parse_date_filename("Picsart_25-02-09_13-33-35-838.png"), Some("2025-02-09".into()));
    }

    #[test]
    fn test_snap_date() {
        assert_eq!(parse_date_filename("snap202405051452.jpg"), Some("2024-05-05".into()));
        assert_eq!(parse_date_filename("snap202505041450.jpg"), Some("2025-05-04".into()));
    }

    #[test]
    fn test_unix_ms_date() {
        // phoneImageCapture1764072777554 → ~2025-12-25
        assert!(parse_date_filename("phoneImageCapture1764072777554.jpg").is_some());
        // Revolut trailing timestamp
        let f = "Revolut_receipt_transaction_d46bace5-3508-47ea-82fd-cf7215ce5d95_1639813806453.jpg";
        assert_eq!(parse_date_filename(f), Some("2021-12-18".into()));
    }

    #[test]
    fn test_whatsapp_camera_code() {
        assert!(is_camera_code("IMG-20250801-WA0004.jpg"));
        assert!(is_camera_code("IMG-20220709-WA0008.jpg"));
        // normal IMG_ still detected
        assert!(is_camera_code("IMG_20210307_200951.jpg"));
    }

    #[test]
    fn test_slug_improvements() {
        // WA counter suppressed
        assert_eq!(extract_slug("IMG-20250801-WA0004"), None);
        // SmartBG UUID stripped, only "smartbg" remains
        assert_eq!(
            extract_slug("SmartBG_2024-12-07_5554ba2b-20ff-4d67-8d75-9ba83036495d"),
            Some("smartbg".into())
        );
        // snap prefix: no slug (JUNK_WORDS)
        assert_eq!(extract_slug("snap202405051452"), None);
        // hex garbage filtered
        assert_eq!(extract_slug("5554ba2b-20ff-4d67-8d75-9ba83036495d"), None);
    }
}
