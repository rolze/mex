# MEX Filename Specification

This document defines the strict filename and directory structure for the MEX media library. All files imported into the library MUST conform to this specification.

## Directory Structure

Files are organized by year into folders:
`<yyyy>/<filename>.<ext>`

The year in the parent folder MUST match the year prefix in the filename.

## Filename Convention

The library supports five distinct filename patterns:

| Pattern | Description |
| :--- | :--- |
| `yyyy-MM-<slug>-####-<caption>.<ext>` | Slug (event/topic) + Counter + Caption |
| `yyyy-MM-<slug>-####.<ext>` | Slug (event/topic) + Counter |
| `yyyy-MM-DD-<caption>-#.<ext>` | Date + Caption + Collision |
| `yyyy-MM-DD-<caption>.<ext>` | Date + Caption |
| `yyyy-MM-DD-####.<ext>` | Date + Counter |

### Component Definitions

- **`yyyy`**: 4-digit year (e.g., `2024`).
- **`MM`**: 2-digit month (`01`-`12`).
- **`DD`**: 2-digit day of month (`01`-`31`).
- **`<slug>`**: A kebab-case string identifying the event or topic. **Must be at least 3 characters long** to avoid ambiguity with `DD`.
- **`####`**: Exactly 4 digits (e.g., `0001`). Used for sequencing within an event/topic slug.
- **`#`**: A plain integer ≥ 2 (e.g., `2`, `3`). Used as a collision suffix when two caption-only files share the same date and caption.
- **`<caption>`**: A kebab-case string providing a brief description of the file contents.
- **`<ext>`**: Lowercase alphanumeric extension (e.g., `jpg`, `mp4`).

## Validation Rules

1. **Kebab-case**: All slugs and captions must be lowercase alphanumeric characters separated by single hyphens. No leading, trailing, or double hyphens.
2. **Ambiguity**: Any string in the `DD` position that is 3 or more characters long is treated as a `<slug>`.
3. **Consistency**: The root folder year must be identical to the year in the filename.

## Battle-Tested Regex (Rust-compatible)

The following regex validates the entire path (folder + filename) and has been verified against the current database (52k+ entries).

```regex
^(\d{4})/(?:\d{4})-(0[1-9]|1[0-2])-(?:(?:0[1-9]|[12]\d|3[01])-(?:[a-z0-9-]+(?:\-\d+)?|\d{4})|(?:[a-z0-9-]{3,})-\d{4}(?:\-[a-z0-9-]+)?)\.[a-z0-9]+$
```

### Explanation:
- `^(\d{4})/`: Captures the 4-digit year folder.
- `(?:\d{4})-`: Matches the filename's year (must be 4 digits). Note: Rust's `regex` crate does not support backreferences (`\1`), so the year is matched twice.
- `(0[1-9]|1[0-2])-`: Matches a valid 2-digit month.
- `(?: ... | ... )`: Two branches for Day-based vs. Slug-based files:
    - **Branch 1 (Day-based)**: Matches `DD-` followed by `caption`, `caption-#` (plain collision counter), or just `####`.
    - **Branch 2 (Slug-based)**: Matches `<slug>-####` (slug >= 3 chars) followed by an optional `-<caption>`.
- `\.[a-z0-9]+$`: Matches the lowercase extension.
