## UC-16 · Slugify & Deslugify

Two complementary commands for managing slugs on already-imported files.

**Slug detection:** the slug is always derived at runtime from the `target_path` filename
using the REGEXP.md filename convention. The legacy `derived_slug` DB column is not read
or written by either command.

- Slug-based filename: `yyyy-MM-<slug>-####.<ext>` or `yyyy-MM-<slug>-####-<caption>.<ext>`
  — the slug is the kebab-case token between the month and the 4-digit counter.
- Day-based filename: `yyyy-MM-DD-…` — no slug.

---

### Command: `:deslugify`

```
:deslugify
```

Operates on the selected file(s) (or cursor file). Useful to fix files that were
imported with a spurious slug (e.g. `camera`) before it was added to the junk-word
list. Can also be run a second time to fix counters that were wrong after a prior
repair — subsequent invocations are idempotent.

**Per-file behaviour:**

Counters are **pre-assigned for the entire batch before any file is renamed**. This means
selecting all files from a given day restarts the counter at 0001 rather than building on
top of each other's existing (potentially wrong) values.

1. Load all file records for the batch from the DB. Parse each file's slug from
   `target_path` (REGEXP.md).
2. For each unique `yyyy-mm-dd` day prefix in the batch, calculate the base counter:
   - Query `MAX(counter)` for paths matching `yyyy/yyyy-mm-dd-%` in the DB
     (`status IN ('moved','trashed','deleted')`), **excluding all files in the batch**.
   - Scan the year directory on disk for filenames starting with `yyyy-mm-dd-`,
     **skipping the basenames of all batch files**.
   - `base_counter = max(db_max, fs_max) + 1`.
3. Assign a new path to each file (sorted by `os_date` then `source_basename` then current
   path within each day for **chronological** counter assignment):
   - **With caption** (no collision): `yyyy/yyyy-mm-dd-{caption}.{ext}`
     (counter omitted — mirrors import behaviour for no-slug + caption files).
   - **With caption** (collision — plain path already assigned in this batch,
     or exists in DB/disk as a non-batch file):
     `yyyy/yyyy-mm-dd-{caption}-{N}.{ext}` where N ≥ 2.
   - **No caption**: `yyyy/yyyy-mm-dd-{new_counter:04}.{ext}`.
4. For each file: if the new path equals the current path **and** the filename has no
   slug → skip (no-op; idempotent).
5. If the filename has a slug: save it as a tag with type `"slug"` (backup, visible in
   the tag list).
6. Rename the file on disk (only when the file exists and the path actually changes).
   Returns an error if the destination path already exists (collision guard).
7. Update `target_path` + `counter` in the DB.

**Status messages:**

| Outcome | Message |
|---------|---------|
| All repaired | `deslugify: repaired N file(s)` |
| Some already clean | `deslugify: repaired N file(s), M already clean` |
| All already clean | `deslugify: M file(s) already clean` |
| Error(s) | `deslugify: N error(s) — <first error>` |

The command runs in a background thread. A full-screen yellow progress overlay
(`Repairing slugs…`) with a spinner and ASCII progress bar is shown while the repair
is in progress; all key input is blocked until the thread finishes.

---

### Command: `:slugify <slug>`

```
:slugify <slug>
```

Operates on the selected file(s) (or cursor file). Groups them under a new or existing
slug. The slug is always read from and written to the filename; no DB slug column is
touched.

#### Slug input behaviour

The slug argument is **normalized character-by-character while typing**, matching the
caption editor (UC-14):

- `Space` → `-`; `ä/Ä` → `ae`; `ö/Ö` → `oe`; `ü/Ü` → `ue`; `ß` → `ss`
- Only `a-z`, `0-9`, and `-` are accepted. Other characters are rejected with
  an "Invalid character for slug" warning.
- Leading and consecutive hyphens are silently skipped.
- Maximum **42 characters**. A counter `[N/42]` is shown in the command bar
  after the cursor: `:slugify my-event_ [8/42]`
- When no argument has been typed yet, a dim `<slug>` placeholder is shown.

At execution:
- Trailing hyphens are trimmed.
- A slug shorter than **3 characters** is rejected
  (`slugify: slug must be at least 3 characters`), matching the REGEXP.md
  ambiguity rule for `DD` vs `<slug>`.

#### Guards

Both checks run before any rename is executed:

1. **Multi-month guard** — derive `yyyy-mm` from each file's `target_path`. If not all
   the same → error.
2. **Multi-slug guard** — parse the slug from each file's `target_path` (REGEXP.md). If
   the selection contains files from two or more distinct slugs → error. Files with no
   slug in their filename may be freely mixed with files from one existing slug.

#### Two modes

**Rename mode** — activated when ALL selected files have the SAME slug X in their
filename, that set is ALL files with slug X in the same `yyyy-mm` (no other files with
slug X exist in the DB for that month), and no slug-free files are mixed in.

- Only the slug token in each filename is swapped: `yyyy/yyyy-mm-X-{N}.ext` →
  `yyyy/yyyy-mm-<slug>-{N}.ext`. Counters are preserved unchanged.
- Caption files: `yyyy/yyyy-mm-dd-X-{caption}.ext` → `yyyy/yyyy-mm-dd-<slug>-{caption}.ext`.
- Only `target_path` is updated in the DB; no recount, no base-counter query.

**Assign mode** — everything else (slug-free files, partial slug split, or a mix of one
existing slug + slug-free files).

Counter pre-assignment (computed before any rename):

1. If the selection is a partial subset of existing slug X: identify the **remaining
   files** of slug X (files in DB whose `target_path` parses to slug X in the same
   `yyyy-mm`, NOT in the batch). Recount those remaining files starting from `0001`
   (sorted by `os_date` then `source_basename`).
2. Resolve the **target group** = DB files whose `target_path` parses to slug `<slug>`
   in the same `yyyy-mm` (those not in the selection) + all selected files.
3. Sort the target group by `os_date` then `source_basename`.
4. Assign sequential counters `0001..N` to non-caption files.
5. Caption files: path = `yyyy/yyyy-mm-dd-<slug>-{caption}.{ext}`. Collision (same path
   already taken within the target group or exists in DB/disk) →
   `yyyy/yyyy-mm-dd-<slug>-{caption}-{N}.{ext}` where N ≥ 2.

Per-file execution (both modes):

1. If new path equals the current path → skip (no-op; idempotent).
2. Rename the file on disk (only when the file exists and the path actually changes).
   Returns an error if the destination path already exists (collision guard).
3. Update `target_path` + `counter` in the DB.

**Status messages:**

| Outcome | Message |
|---------|---------|
| Rename mode success | `slugify: renamed slug to '<slug>' for N file(s)` |
| Assign mode success | `slugify: grouped N file(s) under '<slug>'` |
| Some already clean | `slugify: grouped N file(s) under '<slug>', M already clean` |
| All already clean | `slugify: M file(s) already in slug '<slug>'` |
| Multi-month error | `slugify: all selected files must share the same yyyy-mm prefix` |
| Multi-slug error | `slugify: cannot mix files from different slug groups — select from one slug at a time` |
| Error(s) | `slugify: N error(s) — <first error>` |

The command runs in a background thread. A full-screen yellow progress overlay
(`Slugifying files…`) with a spinner and ASCII progress bar is shown while the
operation is in progress; all key input is blocked until the thread finishes.
