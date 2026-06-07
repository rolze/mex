# PRD-01 · Configuration & Bootstrap

| Meta | Data |
|------|------|
| **Status** | `Draft` |

## Problem

Before a user can browse or manage media, Sem & Mex must know where to find the media collection, where to store its database, and where views are rooted. New users need guided setup; returning users need the application to resume seamlessly. If configuration is missing or invalid, the application must fail clearly rather than silently corrupt data.

## User stories

- As a first-time user, I want the application to walk me through initial setup, so that I don't have to hand-edit a configuration file.
- As a returning user, I want the application to locate my database automatically, so that I can start working immediately.
- As a user, I want the application to detect new, deleted, and renamed files since my last session, so that my database always reflects reality.
- As a user, I want to inspect the application's version, configuration, and database health at any time, so that I can diagnose problems.

## Requirements

### Functional requirements — Configuration file

- **FR-1**: The application must store its configuration in a user-editable plain-text file located in the platform's standard configuration directory.
- **FR-2**: The configuration file must support at least the following settings: the path to the media collection root, the path to the views root, and the path to the database file.
- **FR-3**: All three settings (media collection root, views root, database path) are required. If any setting is missing or empty, the application must refuse to start and display a clear error message identifying the missing setting.
- **FR-4**: The configuration file format must use simple `key=value` lines. Blank lines and lines without an `=` separator must be silently ignored.

### Functional requirements — First-run guided setup

- **FR-5**: When no configuration file exists, the application must launch an interactive guided setup that prompts the user for each required setting in sequence: database path, media collection root, and views root.
- **FR-6**: During guided setup, the application must validate each path as the user provides it. Directories must exist on the filesystem; the database path's parent directory must exist.
- **FR-7**: All paths entered during guided setup must be resolved to absolute paths before being saved to the configuration file.
- **FR-8**: At the end of guided setup, the application must write the configuration file and proceed to normal startup without requiring the user to restart.

### Functional requirements — Database discovery & bootstrap

- **FR-9**: When the configured database file does not exist, the application must automatically create a new, empty database with the required schema.
- **FR-10**: The application must also support discovering an existing database by searching the current directory and its parent directories, falling back to the configured path if discovery fails.
- **FR-11**: On every startup, the application must validate that the database is accessible and its schema is intact. If the database is corrupted or inaccessible, the application must display a clear error and refuse to proceed.

### Functional requirements — Filesystem reconciliation

- **FR-12**: On every startup after the initial bootstrap, the application must reconcile the database with the current state of the filesystem.
- **FR-13**: Files present on the filesystem but absent from the database must be inserted as new entries.
- **FR-14**: Files present in the database but absent from the filesystem must be marked as missing (not deleted from the database).
- **FR-15**: Files that have been renamed or moved on the filesystem must be detected (e.g., via stable file identity) and their database records updated, preserving all associated metadata such as tags.

### Functional requirements — Version & diagnostics

- **FR-16**: The application must provide a `:version` command that displays: application version, operating system, configuration file location, all configured paths, database statistics (e.g., total entries, missing entries), and the status of external dependencies.

### Non-functional requirements

- **NFR-1**: Guided setup must complete in a single interactive session — no multi-step wizards that require restarting the application.
- **NFR-2**: Filesystem reconciliation must complete within a reasonable time for collections of up to 50,000 files, without blocking the user interface beyond an initial loading phase.
- **NFR-3**: Configuration errors must produce actionable error messages that name the specific setting or path that failed validation.

## Acceptance criteria

- **AC-1**: Given no configuration file exists, when the user launches the application, then the guided setup prompts for each required setting in sequence, validates the paths, writes the configuration file, and proceeds to normal startup.
- **AC-2**: Given a configuration file exists with all required settings, when the user launches the application, then the application starts normally without any prompts.
- **AC-3**: Given a configuration file is missing a required setting, when the user launches the application, then startup fails with an error message that names the missing setting.
- **AC-4**: Given a configured database path where no database file exists, when the application starts, then a new empty database with the correct schema is created automatically.
- **AC-5**: Given new files have been added to the media collection since the last session, when the application starts, then those files appear in the database as new entries.
- **AC-6**: Given files have been deleted from the media collection since the last session, when the application starts, then those files are marked as missing in the database but their records and metadata are preserved.
- **AC-7**: Given a file has been renamed on the filesystem since the last session, when the application starts, then the database record is updated to reflect the new name and all associated metadata (tags, state) is preserved.
- **AC-8**: Given the application is running, when the user executes the `:version` command, then the application displays the version, OS, config file location, all configured paths, database statistics, and dependency status.
- **AC-9**: Given a path entered during guided setup points to a non-existent directory, when the user submits the path, then the application rejects it with an error and re-prompts.

## Success metrics

- First-run setup completes in under 60 seconds for a user who knows their paths.
- Filesystem reconciliation for 50,000 files completes within 10 seconds.
- Zero data loss during reconciliation — metadata is never silently discarded.

## Constraints

- The configuration format must be human-editable with a plain text editor.
- Reconciliation must never delete database records — missing files are marked, not purged.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-01  | mex/spec/UC-01-config-and-start.md |

## Open questions

- Should the application support overriding configuration file location via a command-line argument or environment variable?
- What should happen if the media collection root itself does not exist at startup (as opposed to individual files being missing)?
- Should reconciliation provide progress feedback (e.g., a progress bar) for very large collections, or is a brief blocking pause acceptable?
