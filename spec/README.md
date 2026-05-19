# mex — Spec Index

Each UC file is the source of truth for its feature. This index lists what exists; it ages out only if a file is added, renamed, or removed.

| UC | Title | What it covers |
|----|-------|----------------|
| [UC-00](UC-00.md) | Key concepts & UX | Foundational principles, layout, modes, and interaction model |
| [UC-01](UC-01.md) | Config | Per-machine `target_root`, `views_root`, and `db_path`; first-run guided setup |
| [UC-02](UC-02.md) | Browse Media Files | Keyboard-driven TUI file list with live text filter |
| [UC-03](UC-03.md) | File Details | Split-pane metadata and image preview for the selected file |
| [UC-04](UC-04.md) | Selecting Files | Mark files individually or in ranges for bulk operations |
| [UC-05](UC-05.md) | Tag & type filtering | Filter the list by `#tag` and/or `@type` prefixes |
| [UC-06](UC-06.md) | Create view of selection | Hard-link current selection into a named directory tree |
| [UC-07](UC-07.md) | Fix date | Correct date prefixes in filenames for selected files |
| [UC-08](UC-08.md) | Smart import | Deduplicate, date, slug-normalise, and copy new media to target tree |
| [UC-09](UC-09.md) | Assign / remove tags | Apply or remove tags on selected files via `:tag` / `:untag` |
| [UC-10](UC-10.md) | Fix file extension | Repair mismatched extensions using magic-byte detection |
| [UC-11](UC-11.md) | Trash & delete | Soft-delete files to trash; permanently delete with `:empty-trash` |
| [UC-12](UC-12.md) | Open external viewer | Open the cursor file in the system default viewer/player |
| [UC-13](UC-13.md) | mpv integration | Remote-control mpv for video playback; native Linux and WSL2 supported (see [INSTALL.md](../INSTALL.md)) |
