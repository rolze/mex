# PRD-09 · Create View of Selection

| Meta | Data |
|------|------|
| **Status** | `Draft` |

## Problem

Users need a way to materialize their current working set — whether an explicit selection or a filtered list — as a named, flat directory of linked files. This enables on-demand album creation that can be shared, browsed externally, or fed into other tools, without duplicating file data. Views must be cheap to create, idempotent, and instantly recreatable.

## User stories

- As a user, I want to turn my current selection into a named directory of linked files, so that I can share or process a curated set of media outside the application.
- As a user with no explicit selection, I want the filtered list to be used as the view source, so that I don't have to manually select every file when the filter already captures what I need.
- As a user, I want to recreate a view by running the same command again, so that I can update a previously created view with a new set of files without manual cleanup.
- As a user, I want clear feedback on what was created and where, so that I know the operation succeeded and can locate the output.

## Requirements

### Functional requirements

- **FR-1**: The application must provide a command that accepts a single argument — the view name — and materializes the current working set as a named directory of file links.
- **FR-2**: The target files for the view must be determined as follows: the explicit selection if non-empty, otherwise the entire currently filtered list.
- **FR-3**: The view directory must be created at `<views_root>/<name>/`, where `views_root` is a user-configured directory path.
- **FR-4**: If the view directory already exists, it must be deleted and recreated fresh — the command is fully idempotent.
- **FR-5**: Each target file must be linked into the view directory using its original basename. The view directory must have a flat layout with no subdirectories.
- **FR-6**: File linking must not duplicate file data on disk. The operation must complete in constant time regardless of individual file sizes.
- **FR-7**: When the command is typed but the name argument has not yet been entered, the command input area must display a dim placeholder (e.g., `<name>`) to indicate that a name is expected.
- **FR-8**: Upon successful completion, the application must display a status message reporting the number of files linked and the full path of the created view directory.
- **FR-9**: If the configured `views_root` directory does not exist, the application must create it automatically before creating the view.

### Error handling

- **FR-10**: If the name argument is missing when the command is executed, the application must display an error message and take no filesystem action.
- **FR-11**: If the set of target files is empty (no selection and the filtered list is empty), the application must display an error message and take no filesystem action.
- **FR-12**: If a filesystem error occurs during linking of individual files, the application must log the individual failure, continue processing remaining files, and display a summary indicating partial completion.

### Non-functional requirements

- **NFR-1**: View creation must be perceptually instant for typical view sizes (up to several thousand files).
- **NFR-2**: File basenames are guaranteed to be globally unique by the product's filename convention, so name collisions within a view directory cannot occur.

## Acceptance criteria

- **AC-09-01**: Given a non-empty selection and a valid view name, when the user executes the create-view command, then a directory at `<views_root>/<name>/` is created containing one linked file per selected item, and a status message reports the count and path.
- **AC-09-02**: Given no explicit selection and a non-empty filtered list, when the user executes the create-view command with a valid name, then the entire filtered list is used as the source and all items are linked into the view directory.
- **AC-09-03**: Given a view directory that already exists with the same name, when the user executes the create-view command, then the old directory is fully removed and a fresh view is created from the current working set.
- **AC-09-04**: Given the create-view command has been typed but no name argument entered yet, when the user looks at the command input area, then a dim placeholder indicating the expected argument is visible.
- **AC-09-05**: Given the user executes the create-view command without providing a name, when the command is submitted, then an error message is displayed and no directory is created or modified.
- **AC-09-06**: Given the filtered list is empty and no selection exists, when the user executes the create-view command with a valid name, then an error message is displayed and no directory is created.
- **AC-09-07**: Given one or more files fail to link due to a filesystem error, when the create-view command is executed, then the remaining files are still linked, and the status message indicates partial completion with failure details.
- **AC-09-08**: Given the configured `views_root` does not yet exist on disk, when the user executes the create-view command, then the `views_root` directory is created automatically and the view is created successfully inside it.
- **AC-09-09**: Given a view is created with 500 files of varying sizes, when the command completes, then execution time is not perceptibly affected by individual file sizes.

## Success metrics

- View creation completes in under 1 second for views of up to 5,000 files.
- Zero data duplication — linked files share storage with their originals.
- Users can recreate a view with the same name without manual cleanup steps.

## Constraints

- The view directory layout is strictly flat — no subdirectories are created.
- The product's filename uniqueness convention is a prerequisite; the feature does not handle basename collisions.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-06  | mex/spec/UC-06-create-view.md |

## Open questions

- Should there be a maximum allowed length or character restrictions for view names beyond what the filesystem permits?
- Should the application warn or prompt the user before deleting an existing view directory, or is silent replacement always acceptable?
- Should the status message differentiate between "created new view" and "replaced existing view"?
- How should views handle trashed files? Since views use hard links, trashing a file (moving to a trash directory) will not break the view's link. Is this intended?
