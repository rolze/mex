# mex — UX Behaviour Reference

## Navigation

### Keyboard

| Key | Action |
|-----|--------|
| `↑` | Move selection up one row |
| `↓` | Move selection down one row |
| `PgUp` | Move selection up one full page |
| `PgDn` | Move selection down one full page |
| `Ctrl-u` | Move selection up half a page |
| `Ctrl-d` | Move selection down half a page |
| `Home` | Jump to first item |
| `End` | Jump to last item |
| `Enter` / `Space` | Toggle right-pane preview for selected item |
| `Esc` | Cancel command mode → close preview → clear filter (in that priority order) |
| `:` | Enter command mode — type a command and press Enter (e.g. `:q` to quit) |
| `Backspace` | Delete last character (from command buffer or filter) |
| Any other key | Appended to the live search filter instantly |

> No letter keys are bound to navigation actions — all letters go directly into search.
> Use `:` + Enter for commands.

## Filter Bar

- **Type any characters** → live substring filter applied to filename and tags
- **Backspace** → delete last filter character
- **Esc** → clear the filter and restore the full list (scroll position resets to top)
- The filter bar (bottom of screen) shows the active query as `/query_` or a hint when empty

## Split Pane (Preview)

- Activated with `Enter` / `Space`
- Left pane: file list (45 % width)
- Right pane: metadata (path, date, ext, tags) + `chafa` image render when the file exists on disk
- Close with `Esc` or `Enter` / `Space` again

See [[UC-03.md]]

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

# Tags

Tags are a core element in the tool. It should be fast and easy to maintain those tags across different ranges of files.

They are displayed with a special highlight (let's ponder how) to spot easily what text comprises one tag.

