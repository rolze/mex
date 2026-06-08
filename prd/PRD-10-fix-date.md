# PRD-10 · Fix Date Prefix

| Meta | Data |
|------|------|
| **Status** | `Draft` |

## Problem

Media files in the library carry a date prefix that determines their chronological placement and folder organisation. When files were originally imported with an incorrect date — due to wrong camera settings, batch-import errors, or manual mistakes — the user needs a fast, reliable way to correct the date across one or more files without leaving the application. The correction must update the filename, the file's location on disk, the file's modification timestamp, and all derived data in one atomic operation.

## User stories

- As a user, I want to change the date prefix of selected files via a single command, so that I can fix dating errors without manually renaming files and moving folders.
- As a user, I want the application to preserve the original time-of-day when correcting a date, so that the file's temporal ordering within a day is not lost.
- As a user, I want immediate feedback if I enter an invalid date, so that I don't accidentally corrupt my filenames.
- As a user, I want the command name to autocomplete as I type, so that I can discover and invoke the command quickly.

## Requirements

### Functional requirements

- **FR-1**: The application must provide a `:fix-date <yyyy-mm-dd>` command that replaces the date prefix of the targeted files with the specified date.
- **FR-2**: When the command name has been typed but no date argument is present, the command bar must display a dim `<yyyy-mm-dd>` placeholder hint.
- **FR-3**: Command name autocompletion must be supported while typing the command name (before the first space):
  - Typing a partial command name displays a dim inline suffix showing the best matching completion.
  - Up/Down arrow keys cycle through matching command suggestions.
  - Tab accepts the current suggestion and appends a trailing space.
  - After a space has been typed (argument phase), Tab must be a no-op.
- **FR-4**: The command must operate on all currently selected files. If no files are selected, it must operate on the file under the cursor.
- **FR-5**: Date prefix replacement requires the full `yyyy-mm-dd` format from the user (as it must be reflected in the internal `mex_date` field). For filesystem renaming, it applies as follows:
  - **Day format** filenames: the entire date prefix is replaced with the new year, month, and day.
  - **Slug format** filenames: only the year and month portions are replaced in the filename; the day component is stored in the database but omitted from the filename.
- **FR-6**: When a date change causes a file's year to differ from its current folder, the file must be moved to the correct year folder. The year folder must be created automatically if it does not already exist. If moving or renaming causes a collision in the destination folder, an auto-incrementing collision suffix must be appended.
- **FR-7**: The file's operating-system modification timestamp must be updated to reflect the new date while preserving the original time-of-day component (hours, minutes, seconds). The time-of-day is determined by, in order of priority:
  1. The file's current modification timestamp on disk.
  2. The previously stored date-time value in the application's data store.
  3. Midnight (00:00:00) as a last resort.
- **FR-8**: If the underlying filesystem rejects the modification timestamp update, the operation must fail entirely to prevent an inconsistent state between the filename and the filesystem metadata.
- **FR-8**: All derived data stored by the application (date fields, file path references) must be updated to reflect the new filename, location, and date.
- **FR-9**: After the operation completes, the file list must reload and any active filter must be re-applied.
- **FR-10**: The date argument must be validated before execution:
  - The format must be exactly `yyyy-mm-dd`.
  - The date must be a valid calendar date (e.g., month 13, day 32, or February 30 must be rejected).
  - Invalid input must produce a clear error message in the status area without modifying any files.
- **FR-11**: On success, a confirmation message must be displayed in the status area. On partial or full failure, an error message must describe what went wrong.
- **FR-12**: Files whose filenames do not conform to a recognised naming convention must be left unchanged and must not cause the operation to fail for other files in the batch.

### Non-functional requirements

- **NFR-1**: The fix-date operation must complete without perceptible delay for batches of up to 100 files.
- **NFR-2**: No file must be left in an inconsistent state (e.g., renamed on disk but not updated in the data store, or vice versa). Each file's rename and data update must be treated as an atomic unit.

## Acceptance criteria

- **AC-10-01**: Given a day-format file `2022-04-18-0001.jpg`, when the user executes `:fix-date 2023-06-15`, then the file is renamed to `2023-06-15-0001.jpg` and moved to the `2023` folder.
- **AC-10-02**: Given a bare day-format file `2022-04-18.jpg` with no trailing components, when the user executes `:fix-date 2023-06-15`, then the file is renamed to `2023-06-15.jpg`.
- **AC-10-03**: Given a slug-format file `2022-04-festival-0001.jpg`, when the user executes `:fix-date 2023-06-15`, then only the year and month are replaced, producing `2023-06-festival-0001.jpg`.
- **AC-10-04**: Given a slug-format file with a caption, when the user executes `:fix-date` with a new date, then only the year and month portions of the filename are updated; the slug, counter, and caption remain unchanged.
- **AC-10-05**: Given a file whose name does not match any recognised naming convention, when the fix-date operation runs on a batch that includes it, then that file is left unchanged and the operation succeeds for all other valid files.
- **AC-10-06**: Given the user has typed `fix` in command mode, when command suggestions are displayed, then `fix-date` appears as a suggestion.
- **AC-10-07**: Given the user has typed the full command name `fix-date`, when suggestions are displayed, then the command is still shown as a suggestion.
- **AC-10-08**: Given an empty command buffer, when suggestions are requested, then all known commands are returned.
- **AC-10-09**: Given a command prefix that matches no known command, when suggestions are requested, then the suggestion list is empty.
- **AC-10-10**: Given the user has typed a partial command name `fix`, when Tab is pressed, then the command buffer is completed to `fix-date ` (with trailing space).
- **AC-10-11**: Given the user has already typed a command name and a space (argument phase), when Tab is pressed, then nothing happens (no-op).
- **AC-10-12**: Given the user types a non-date string as the argument (e.g., `hello`), when Enter is pressed, then an error message is displayed and no files are modified.
- **AC-10-13**: Given the user types an impossible date (e.g., month `13` or day `32`), when Enter is pressed, then an error message is displayed and no files are modified.
- **AC-10-14**: Given a file with a current modification time of `14:30:45`, when the user executes `:fix-date 2023-06-15`, then the file's new modification timestamp is `2023-06-15 14:30:45` — the original time-of-day is preserved.
- **AC-10-15**: Given a file whose modification time cannot be read and whose stored date-time is `2022-04-18 09:15:00`, when the date is fixed, then the new modification timestamp uses `09:15:00` as the time-of-day component.
- **AC-10-16**: Given a file with no available time-of-day from any source, when the date is fixed, then the modification timestamp defaults to midnight (`00:00:00`) on the new date.
- **AC-10-17**: Given the fix-date operation succeeds, when the file list refreshes, then the active filter is re-applied and the list reflects the updated filenames and folder positions.

## Success metrics

- Users can correct a date on a single file or a batch of selected files in under 3 seconds of interaction time.
- Zero instances of files left in an inconsistent state (disk vs. data store mismatch) after fix-date.

## Constraints

- Slug-format files intentionally omit the day from the filename; the command must not inject a day component into slug filenames.
- The operation must not silently skip files without reporting why they were skipped.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-07  | mex/spec/UC-07-fix-date.md |


