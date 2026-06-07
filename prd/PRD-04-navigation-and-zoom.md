# PRD-04 · Navigation & Semantic Zoom

| Meta | Data |
|------|------|
| **Status** | `Implemented` |

## Problem

Users with large media collections often have many files that share the same base subject or date. An un-grouped flat list becomes visually overwhelming and tedious to navigate. Grouping everything rigidly creates deep, complex trees that are hard to navigate and take up too much horizontal space. Users need a fluid, intuitive way to progressively zoom out from details to high-level temporal overviews, and contextually zoom back in on specific areas of interest without getting lost in indentation.

## User stories

- As a user browsing a massive collection, I want to progressively collapse items into broader time and slug categories using a single key, so that I can quickly gain a high-level timeline overview.
- As a user exploring grouped categories, I want to expand specific groups contextually under my cursor without expanding everything, so that I can inspect a specific time period while keeping the rest of the timeline collapsed.
- As a user, I want a clear visual indication when a group is collapsed and what level it represents, without breaking the strictly aligned vertical layout.
- As a user navigating these levels, I want to see hints and status updates, so that I understand my current zoom level and what I just expanded or collapsed.

## Requirements

### Functional requirements

- **FR-1**: **Progressive Zoom Out (Contextual -> Global Left)**. The application must support progressive grouping triggered sequentially via the `Left arrow` key.
  - The zoom out always starts contextually. If the cursor is inside a specific group that was previously expanded, pressing `Left` collapses *just that group*.
  - If no contextual collapse is applicable, `Left` globally groups items by slug/day.
  - Subsequent `Left` presses globally group by month, and then by year.
- **FR-2**: **Contextual Zoom In (Cursor Right)**. When focused on a collapsed group, the `Right arrow` action must expand *only the group under the cursor* to its immediate child level (e.g., Year expands to Months, Month expands to Slug/Days, Slug/Day expands to individual items).
- **FR-3**: **Cascading Zoom In (Subsequent Rights)**. When focused on a fully expanded item, subsequent expand actions must progressively cascade outward, expanding all siblings at the current level, then parents, up to a fully flat list.
- **FR-4**: **Summary Rows**. When a group is collapsed, a dedicated summary row is displayed. It must display the group key (e.g. `1992`, `1992-12`, `1992-12-slug`), a count of the media types it contains (e.g., "4 images, 3 videos"), and omit tags.
- **FR-5**: **Visual Zoom Indicators**. The summary row must preserve the standard padded prefix formatting (`YYYY  / `) to keep all tag boundaries perfectly aligned vertically. The tag column must display multiple `+` symbols indicating the zoom level (`+++` for Year, `++` for Month, `+` for Slug).
- **FR-6**: **Status Updates & Hints**. The application must display explicit status messages in the bottom bar immediately after any zoom action, clearly stating what was just expanded/collapsed (e.g. "Collapsed year: 2024", "Expanded month: 2024-06") or hinting at global changes (e.g. "Grouped by Slug. Left to group by Month.").
- **FR-7**: **Action Guardrails**. When the cursor is on a summary row, actions that apply to specific items (e.g., renaming, viewing) must be disabled or safely rejected without side effects.
- **FR-8**: **Cursor Placement**. When a group is collapsed, the cursor is placed on the newly created summary row.

### Non-functional requirements

- **NFR-1**: **Performance**. Interaction latency when grouping or expanding must be perceptually instant, feeling fluid and responsive (under 16ms).
- **NFR-2**: **Focus Preservation**. The focused item or group must remain cleanly in view when the list expands or collapses around it.

## Acceptance criteria

- **AC-1**: Given a flat list, when the user presses `Left`, then items are grouped and collapsed by slug/day globally, and a status hint predicts the next left action.
- **AC-2**: Given a list grouped by Month, when the user presses `Left`, then items are grouped by Year, and a status hint indicates maximum zoom out.
- **AC-3**: Given a list collapsed to Years, when the user focuses a Year and presses `Right`, then only that Year expands to show its contained Month groups. The status bar reads "Expanded year [Y]: showing months".
- **AC-4**: Given an expanded Year showing Month groups, when the user focuses a Month and presses `Left`, then that specific Month collapses back into the Year, without affecting other groups. The status bar reads "Collapsed month: [M]".
- **AC-5**: Given a collapsed group, its summary row preserves the `YYYY  / ` visual layout and displays `+++`, `++`, or `+` in the tag column depending on the depth. The item count properly pluralizes "1 image" vs "2 images".
- **AC-6**: Given a focused item inside a fully expanded Slug/Day group, when the user presses `Right`, then all other Slug/Day groups within the current Month are expanded.

## Success metrics

- Users can navigate from a flat list to a decade overview and back down to a specific item quickly using only the `Left`/`Right` keys.
- User confusion is minimized; hints explicitly confirm the exact outcome of the arrow key press.
- Zero visual jitter; the tag column remains perfectly straight regardless of nesting depth.

## Constraints

- This interaction model replaces traditional deep-tree directory navigation.
- The UI handles a "mixed-level" state where some parts of the list are flat items and other parts are high-level year groups, without allocating extra strings during rendering.

## Traceability

| Source | Reference |
|--------|-----------|
| Human Feature Request | Navigation Groups & Semantic Zoom |
