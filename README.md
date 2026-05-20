# mex — Media Explorer

A personal, opinionated terminal media browser and importer written in **Rust + Ratatui**. It fits one mental model (mine) — fork it if yours differs.

![Sem&Mex](sem-and-mex.png)

## What it does

- **Browses** a large media library (tested to 50 000+ files) from a SQLite-backed index (`.mex.db`) — instant, keyboard-only, no mouse required.
- **Imports** new media from phones, cameras, and drives: deduplicates by SHA-256, extracts dates from EXIF/XMP/filename/folder, derives slugs from source paths, and normalises filenames to a consistent scheme.
- **Filters** the list live by filename text, tags (`#tag`), and tag types (`@type`) — combined with AND/OR logic.
- **Selects** files individually or in groups (Shift-arrows, Shift-Home/End) for bulk tagging or view creation.
- **Tags** files with typed, autocompleted tags (`:tag name@type` / `:untag`).
- **Creates views** on demand as hard-linked directory trees (`:create-view <name>`) for album sharing without copying files.
- **Fixes** metadata in-place: `:fix-date` to correct date prefixes and `:fix-ext` to repair wrong file extensions.
- **Previews** images inline using Kitty / Sixel / iTerm2 / halfblock protocols — auto-detected, overridable via `MEX_PROTOCOL`.
- **Opens images** in [sem](sem/README.md), a companion GTK4 viewer — press `p` on an image file, or select multiple images and press `p` for a thumbnail grid.

## Companion: sem

[`sem/`](sem/README.md) is a lightweight GTK4 image viewer that mex launches as a detached subprocess:

- Single image — `p` on any image file opens it in a native window with tags shown.
- Grid view — `p` with 2+ images selected opens a 256 px thumbnail grid; click any thumbnail to view full-size. Thumbnails are cached on disk for instant repeat opens.

See [sem/README.md](sem/README.md) for build instructions and details.

## Filename Convention

The library enforces a strict filename and directory structure to ensure long-term maintainability and easy browsing. See [REGEXP.md](REGEXP.md) for the formal specification, component definitions, and validation rules.

## Configuration

Per-machine config in `~/.config/mex/config.toml`. The shared `.mex.db` is never written during configuration.

| Key | Description |
|---|---|
| `target_root` | Absolute path to the local media root |
| `views_root` | Absolute path where `:create-view` materialises view directories |
| `db_path` | Path to the `.mex.db` SQLite database |

On first run mex guides you through each setting interactively — no manual editing required. If no database is found, you are prompted for a path (default: `./.mex.db`); a fresh empty database is created automatically.