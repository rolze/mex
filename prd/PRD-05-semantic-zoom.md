# PRD-05 · Progressive Semantic Zoom

## Problem

Users face an overwhelming flat list of items when browsing large directories. Grouping everything rigidly creates deep, complex trees that are hard to navigate and take up too much horizontal space. Users need a fluid, intuitive way to progressively zoom out from details to high-level temporal overviews, and contextually zoom back in on specific areas of interest without getting lost in indentation.

## User stories

- As a user browsing a massive collection, I want to progressively collapse items into broader time and slug categories using a single key, so that I can quickly gain a high-level timeline overview.
- As a user exploring grouped categories, I want to expand specific groups contextually under my cursor without expanding everything, so that I can inspect a specific time period while keeping the rest of the timeline collapsed.
- As a user navigating these levels, I want to see hints and status updates, so that I understand my current zoom level and what the next action will do.
- As a user wanting to see more context, I want to repeatedly expand siblings and parent groups sequentially, so that I can gradually reveal the surrounding context of a selected item.

## Requirements

### Functional requirements

FR-1: **Progressive Zoom Out (Global Left)**. The application must support progressive grouping triggered sequentially. 
  - 1st Left: Groups items by slug/day (collapsing items into slug/day groups).
  - 2nd Left: Groups by month (collapsing slug/day groups into month groups).
  - 3rd Left: Groups by year (collapsing month groups into year groups).

FR-2: **Contextual Zoom In (Cursor Right)**. When focused on a collapsed group, the action must expand *only the group under the cursor* to its immediate child level (e.g., Year expands to Months, Month expands to Slug/Days, Slug/Day expands to individual items).

FR-3: **Cascading Zoom In (Subsequent Rights)**. When focused on a fully expanded item, subsequent expand actions must progressively cascade outward, expanding all siblings at the current level, then parents, up to a fully flat list.
  - 1st Cascading Right: Expands all Slug/Days in the current Month.
  - 2nd Cascading Right: Expands all Months in the current Year.
  - 3rd Cascading Right: Expands everything globally.

FR-4: **Status Updates & Hints**. The application must display status messages or hints indicating the current grouping/zoom level and explaining the effect of the next Left/Right actions.

FR-5: **Inline Expansion**. Expanding groups must display contents inline in the list. It must not rely on deep hierarchical visual indentation that pushes content off-screen.

### Non-functional requirements

NFR-1: **Performance**. Interaction latency when grouping or expanding must be perceptually instant, feeling fluid and responsive.
NFR-2: **Focus Preservation**. The focused item or group must remain cleanly in view when the list expands or collapses around it.

## Acceptance criteria

AC-1: **Global Zoom Out 1**: Given a flat list of items, when the user presses Left, then items are grouped and collapsed by slug/day, and a status hint predicts the next left action (e.g., "Left to group by Month").
AC-2: **Global Zoom Out 2**: Given a list grouped by slug/day, when the user presses Left, then items are grouped by Month, and a status hint indicates the next left action (e.g., "Left to group by Year").
AC-3: **Global Zoom Out 3**: Given a list grouped by Month, when the user presses Left, then items are grouped by Year, and a status hint indicates maximum zoom out.
AC-4: **Contextual Zoom In (Year)**: Given a list collapsed to Years, when the user focuses a Year and presses Right, then only that Year expands to show its contained Month groups, while other Years remain collapsed.
AC-5: **Contextual Zoom In (Month)**: Given an expanded Year showing Month groups, when the user focuses a Month and presses Right, then that Month expands to show Slug/Day groups.
AC-6: **Contextual Zoom In (Slug/Day)**: Given an expanded Month showing Slug/Day groups, when the user focuses a Slug/Day and presses Right, then that group expands to show individual items.
AC-7: **Cascading Zoom In (Months)**: Given a focused item inside a fully expanded Slug/Day group, when the user presses Right, then all other Slug/Day groups within the current Month are expanded.
AC-8: **Cascading Zoom In (Years)**: Given a focus in a fully expanded Month, when the user presses Right, then all other Month groups within the current Year are expanded.
AC-9: **Cascading Zoom In (Global)**: Given a focus in a fully expanded Year, when the user presses Right, then all Year groups are expanded globally.

## Success metrics

- Users can navigate from a flat list to a decade overview and back down to a specific item quickly using only the Left/Right keys.
- User confusion is minimized; hints accurately predict the exact outcome of the next arrow key press.

## Constraints

- This interaction model replaces traditional deep-tree directory navigation.
- The UI must handle "mixed-level" state where some parts of the list are flat items and other parts are high-level year groups.

## Traceability

| Source | Reference |
|--------|-----------|
| Human Feature Request | Progressive Semantic Zoom |

## Open questions

- How exactly do we display mixed-level groupings without deep indentation while preserving clear visual hierarchy?

answer> the summary of the grouped item reflects it properly. Examples:
* 1992-12-fkn (...)
* 1992-12 (...)
* 1992 (...)

- Does Left always zoom out globally, or does it zoom out contextually if the user is inside a specific expanded group? (Assumption in this PRD: Left is a global progressive zoom-out).

answer> always start on cursor and zoom out contextually
