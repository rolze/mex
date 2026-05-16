# mex — Media Explorer

A personal, opinionated terminal media browser and importer written in **Rust + Ratatui**. It fits one mental model (mine) — fork it if yours differs.

## What it does

- **Browses** a large media library (tested to 50 000+ files) from a SQLite-backed index (`.mex.db`) — instant, keyboard-only, no mouse required.
- **Imports** new media from phones, cameras, and drives: deduplicates by SHA-256, extracts dates from EXIF/XMP/filename/folder, derives slugs from source paths, and normalises filenames to a consistent scheme.
- **Filters** the list live by filename text, tags (`#tag`), and tag types (`@type`) — combined with AND/OR logic.
- **Selects** files individually or in groups (Shift-arrows, Shift-Home/End) for bulk tagging or view creation.
- **Tags** files with typed, autocompleted tags (`:tag name@type` / `:untag`).
- **Creates views** on demand as hard-linked directory trees (`:vc <name>`) for album sharing without copying files.
- **Fixes** metadata in-place: `:fix-date` to correct date prefixes and `:fix-ext` to repair wrong file extensions.
- **Previews** images inline using Kitty / Sixel / iTerm2 / halfblock protocols — auto-detected, overridable via `MEX_PROTOCOL`.

## Filename convention

```
<yyyy>/  yyyy-MM-<slug>-####-<caption>.<ext>   ← slug + caption
         yyyy-MM-<slug>-####.<ext>              ← slug only
         yyyy-MM-DD-<caption>.<ext>             ← caption only
         yyyy-MM-DD-####.<ext>                  ← counter only
```

- Year-based folder, date prefix on every file.
- **Slug** — the only file-based grouper (derived from source folder names; falls back to `yyyy-MM-DD`).
- **Caption** — optional short description embedded in the filename; user-editable.

## Configuration

Per-machine config in `~/.config/mex/config.toml`. The shared `.mex.db` is never written during configuration.

| Key | Description |
|---|---|
| `target_root` | Absolute path to the local media root |
| `views_root` | Absolute path where `:create-view` materialises view directories |
| `db_path` | Path to the `.mex.db` SQLite database |

On first run mex guides you through each setting interactively — no manual editing required. If no database is found, you are prompted for a path (default: `./.mex.db`); a fresh empty database is created automatically.