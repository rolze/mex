# mex — UX Behaviour Reference

## Navigation

### Keyboard

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up one row |
| `↓` / `j` | Move selection down one row |
| `PgUp` | Move selection up one full page |
| `PgDn` | Move selection down one full page |
| `Ctrl-u` | Move selection up half a page |
| `Ctrl-d` | Move selection down half a page |
| `g` | Jump to first item |
| `G` | Jump to last item |
| `Enter` / `Space` | Toggle right-pane preview for selected item |
| `Esc` | Close preview (if open), otherwise clear filter |
| `q` | Quit |

### Mouse

| Action | Behaviour |
|--------|-----------|
| **Scroll wheel up** | Move selection up one row (equivalent to `↑`) |
| **Scroll wheel down** | Move selection down one row (equivalent to `↓`) |
| **Left click** on a row | Immediately moves the selection highlight to that row |

> The scroll wheel moves the *selection*, not just the viewport scroll offset — the highlighted item follows the wheel.  
> The viewport auto-scrolls to keep the selection always visible.

## Filter Bar

- **Type any characters** → live substring filter applied to filename and tags
- **Backspace** → delete last filter character
- **Esc** → clear the filter and restore the full list (scroll position resets to top)
- The filter bar (bottom of screen) shows the active query as `/query_` or a hint when empty

## Split Pane (Preview)

- Activated with `Enter` / `Space` or a click on the highlighted row
- Left pane: file list (45 % width)
- Right pane: metadata (path, date, ext, tags) + `chafa` image render when the file exists on disk
- Close with `Esc` or `Enter` / `Space` again

## Column Layout (Left Pane)

```
┌─────────────────────────────────────────────────────┐
│ folder/   filename (front-truncated)    tags         │
│ 6 chars   remaining width              30 chars      │
└─────────────────────────────────────────────────────┘
```

- **Folder**: the year directory (e.g. `2022/`). Truncated at the front if longer than 6 chars.
- **Filename**: takes all remaining width. If the name is too long, the *beginning* is replaced with `…` so the extension and unique tail are always visible (e.g. `…-long-event-name.jpeg`).
- **Tags**: up to 30 chars. If the tag list is too long, the end is replaced with `…`.
