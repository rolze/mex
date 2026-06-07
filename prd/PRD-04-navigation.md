# PRD-04 · Navigation Groups

## Problem

Users with large media collections often have many files that share the same base subject or date (e.g., burst shots, grouped variants). An un-grouped flat list becomes visually overwhelming and tedious to navigate. Users need a way to visually group related items and selectively collapse them to reduce clutter and navigate more efficiently.

## User stories

- As a user, I want to collapse files sharing the same slug or day into a single line, so that I can quickly scan over large groups of related items.
- As a user, I want a visual indication when a group is collapsed, so that I know there are hidden items inside.
- As a user, I want to quickly expand or collapse groups using my keyboard, so that I can maintain an efficient workflow without breaking focus.

## Requirements

### Functional requirements

- **FR-1**: The application allows grouping of list items that share the same "group key" (defined as either a slug or a day).
- **FR-2**: When a group is collapsed, all items in that group are hidden and a dedicated summary row is displayed in their place.
- **FR-3**: The summary row must display the group key, a count of the media types it contains (e.g., "4 images, 3 videos"), and omit tags, instead displaying a `+` (plus sign) to indicate it contains collapsed items.
- **FR-4**: The application binds the `Left arrow` key to collapse the current group.
- **FR-5**: The application binds the `Right arrow` key to expand the group currently under the cursor.
- **FR-6**: When a group is collapsed by the user, the cursor must be placed on the newly created summary row.
- **FR-7**: When the cursor is on a summary row, actions that apply to specific items (e.g., renaming, viewing) must be disabled or rejected gracefully without side effects, as no specific item is selected.
- **FR-8**: Pressing the `Left arrow` key on an item that does not belong to any group must have no effect.

### Non-functional requirements

- **NFR-1**: Expanding and collapsing operations must be perceptually instant (no visible UI lag).

## Acceptance criteria

- **AC-1**: Given a list containing multiple files with the same group key, when the group is in an expanded state, then all files are displayed as individual rows.
- **AC-2**: Given a list with an expanded group, when the user presses the `Left arrow` key while navigating within that group, then the group collapses, all constituent items are hidden, and a single summary row replaces them.
- **AC-3**: Given a collapsed group, its summary row displays the group key, a media count (e.g., `(4 images, 3 videos)`), no tags, and a `+` indicator.
- **AC-4**: Given an expanded group, when the group collapses, the cursor is moved to the group's summary row.
- **AC-5**: Given a collapsed group, when the user focuses on the group's summary row and presses the `Right arrow` key, then the group expands to show all of its items.
- **AC-6**: Given the cursor is on a collapsed group summary row, when the user attempts a file-specific action, then the action is ignored or safely rejected without side effects.
- **AC-7**: Given the cursor is on an item that is not part of a group, when the user presses the `Left arrow` key, then nothing happens and the cursor remains in place.

## Success metrics

- Expanding and collapsing actions trigger list updates in under 16ms.

## Constraints

- Grouping logic relies strictly on matching the designated "group key" (slug or day).
- The cursor cannot reach any element within a collapsed group until it is expanded.

## Traceability

| Source | Reference |
|--------|-----------|
| Human Request | Custom Feature Request |
