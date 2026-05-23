## UC-01 · Config

**Actor:** User  
**Goal:** Configure mex settings — the local base directory of the media files, the views root, and the database file path. The command `:version` displays all relevant settings, dependency versions, and debugging information in the main screen.

**Context:** The `.mex.db` media database is shared across devices and locations. Each local
mex installation keeps its own config in `~/.config/mex/config.toml` so that each machine
can map the shared DB to its own local path. On first run mex also guides the user to
locate or create the database file.

**Config keys (`~/.config/mex/config.toml`):**

| Key | Required | Description |
|---|---|---|
| `target_root` | Yes | Absolute path to the local media root (prefixed to all `target_path` values from the DB) |
| `views_root` | Yes | Absolute path where `:create-view` materialises named view directories (see UC-06); created on disk if it does not exist |
| `db_path` | Yes | Path to the `.mex.db` SQLite database file; persisted after first resolution so subsequent launches skip discovery |

Example:
```
target_root = /mnt/photos
views_root = /home/user/mex-views
db_path = /home/user/photos/.mex.db
```

**Startup sequence:**

### Step 1 — Resolve database path
1. If `db_path` is in config and the file exists → use it.
2. If `db_path` is in config but the file is missing → print reason and prompt the user to enter a new path (pre-filled with the current value).
3. If `db_path` is absent from config → attempt auto-discovery (search `.`, `../`, `../../` for `.mex.db`); if found, adopt silently and save to config.
4. If still unresolved → prompt the user; default answer is `./.mex.db` (press Enter to accept).
5. The resolved path is saved to config. If the file does not yet exist it is **created as an empty database** with the full schema (tables: `media`, `tags`, `media_tags`).
6. Cancelling at the prompt (empty input when no current value and no default accepted) exits mex.

### Step 2 — Resolve media root (`target_root`)
1. If `target_root` is missing or the directory does not exist, mex prints the reason and prompts the user to enter a path before opening the TUI.
2. The entered path is validated (exists, is a directory, is readable) and saved to config.
3. If the user provides empty input when no value exists, mex exits.

### Step 3 — Resolve views root (`views_root`)
1. If `views_root` is not yet configured, mex prompts the user (same flow as `target_root`).
2. If `views_root` resolves to a non-existent path, mex creates the directory (including parents).

### Step 4 — Open TUI
The main browser opens using the confirmed database, media root, and views root.

### :version command

1. User types `:version` and presses Enter.
2. mex collects the following information and replaces the media list with a version screen:
   - **Sem & Mex versions**: mex version from build metadata; sem version obtained by
     running `sem --version` (errors shown verbatim for debugging).
   - **OS and architecture**: detected at runtime via `std::env::consts`.
   - **Config file path**: `~/.config/mex/config.toml` (tilde-abbreviated).
   - **Settings**: all four config values (`target_root`, `views_root`, `db_path`,
     `mpv_path`), plus the active image protocol (e.g. kitty / sixel / halfblocks).
   - **Database**: file size and total file count from `db_path`.
   - **Dependencies**: `sem`, `mpv`, `socat` — each shows ✓ with resolved path when
     found on PATH, or ✗ with an installation hint when missing.
3. Pressing **Esc** closes the version screen and returns to the media list.
4. All other keys are ignored while the version screen is active.

**Acceptance Criteria:**
- First-time users are prompted for `db_path` before anything else; pressing Enter creates `./.mex.db`.
- A new database file is scaffolded with the correct schema and starts empty (0 files).
- First-time users are prompted for `target_root` and `views_root` before the TUI opens.
- A broken/moved root or missing DB file is detected on next launch and re-prompted.
- The resolved `db_path` is saved to config so subsequent launches skip discovery.
- The media DB is never written to during configuration.
- Users can preview referenced media files (images load from `target_root + target_path`).
- Config is stored in `~/.config/mex/config.toml`, not in `.mex.db`.