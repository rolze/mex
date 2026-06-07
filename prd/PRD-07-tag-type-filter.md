# PRD-07 · Tag & Type Filtering

| Meta | Data |
|------|------|
| **Status** | `Draft` |
| **Derived from** | UC-05 (Tag and tag-type filtering) |

## Problem

Users organise their media with tags (e.g. "travel", "alice") and tag types (e.g. "person", "camera"). To locate specific files they need to combine free-text filename search with tag-based and type-based filters. Without a unified, composable filter system the user is forced to scroll manually through large collections, losing context and time.

## User stories

- As a user, I want to filter my media list by one or more tags so that I can quickly narrow down to files I care about.
- As a user, I want to filter by tag type so that I can view all files associated with a category of tags (e.g. all "person" tags) without remembering individual tag names.
- As a user, I want to combine text, tag, and type filters in a single query so that I can express precise multi-dimensional searches.
- As a user, I want autocompletion while typing tags or types so that I can discover and select valid values without memorising them.
- As a user, I want to see a clear visual representation of the active filter expression so that I understand exactly what criteria are applied.

## Requirements

### Functional requirements — Tag filter mode

- **FR-1**: The user must be able to enter tag filter mode by typing the `#` prefix character while in the filter bar.
- **FR-2**: While in tag filter mode, the system must display inline autocompletion suggestions drawn from the set of existing tags in the collection.
- **FR-3**: The user must be able to cycle through autocompletion suggestions using Up/Down navigation.
- **FR-4**: The user must be able to accept the current autocompletion suggestion into the input using Tab.
- **FR-5**: The user must be able to confirm the current tag selection using Enter, adding it to the set of active tag filters.
- **FR-6**: Confirming a tag that is already active (case-insensitive match) must be a no-op — no duplicate tags are permitted.
- **FR-7**: Multiple confirmed tags must be combined using OR logic: a file matches if it carries *any* of the confirmed tags.

### Functional requirements — Tag-type filter mode

- **FR-8**: The user must be able to enter tag-type filter mode by typing the `@` prefix character while in the filter bar. The interaction mirrors tag filter mode (autocompletion, cycling, Tab-complete, Enter-confirm).
- **FR-9**: While in tag-type filter mode, the system must display inline autocompletion suggestions drawn from the set of existing tag types in the collection.
- **FR-10**: Confirming a type that is already active (case-insensitive match) must be a no-op.
- **FR-11**: Multiple confirmed types must be combined using OR logic: a file matches if it carries a tag belonging to *any* of the confirmed types.

### Functional requirements — Combined filter logic

- **FR-12**: The full filter expression must follow the boolean structure: `text AND (@types OR …) AND (#tags OR …)`. All three dimensions — text, types, and tags — are combined with AND. Within each dimension, multiple values are combined with OR.
- **FR-13**: The text filter must search filenames only. It must not match against tag names or tag-type names.
- **FR-14**: Tag matching and type matching must both be case-insensitive.

### Functional requirements — Mode switching and clearing

- **FR-15**: Pressing Backspace while the tag/type input is empty must exit the respective typing mode without removing any confirmed tags or types.
- **FR-16**: Pressing Backspace when no text is present and no typing mode is active must remove the most recently confirmed tag or type.
- **FR-17**: Pressing Esc must reset the entire filter state: all text, all confirmed tags, all confirmed types, and any active typing mode.
- **FR-18**: Typing `#` while in `@` typing mode (or `@` while in `#` typing mode) must exit the current mode, discarding any partial input, and enter the other mode.

### Functional requirements — Filter bar display

- **FR-19**: When no filter is active, the filter bar must display hint text.
- **FR-20**: When a filter is active, the filter bar must render the full boolean expression inline, clearly distinguishing each component:
  - The text portion must be visually emphasised (e.g. bold).
  - Boolean connectives (`AND`, `OR`, parentheses) must be visually subdued (e.g. dimmed).
  - Tag-type tokens (`@type`) must use a distinct colour that differentiates them from tags and text.
  - Tag-name tokens (`#tag`) must use a distinct colour that differentiates them from types and text.
- **FR-21**: When more than one tag or more than one type is confirmed, the respective group must be visually wrapped in parenthesised OR expressions (e.g. `(@person OR @camera)`).
- **FR-22**: While the user is typing in tag or type mode, a dimmed autocompletion suffix must be shown after the current input.

### Non-functional requirements

- **NFR-1**: Autocompletion suggestions must appear perceptually instantly — the user must not perceive any delay between keystrokes and updated suggestions.
- **NFR-2**: Filter results must update perceptually instantly after a tag or type is confirmed, even on collections of 50,000 items.

## Acceptance criteria

- **AC-1**: Given a collection with tagged files, when the user types `#travel` and confirms with Enter, then only files tagged "travel" are shown.
- **AC-2**: Given a collection with tagged files, when the user confirms `#travel` and `#holiday`, then files tagged with *either* "travel" or "holiday" are shown (OR logic).
- **AC-3**: Given a file tagged "travel", when the user types `#TRAVEL` and confirms, then that file matches (case-insensitive).
- **AC-4**: Given a collection, when the user types plain text without a `#` or `@` prefix, then the filter matches against filenames only — not tag names.
- **AC-5**: Given a text filter "london" and a confirmed tag `#alice`, when both are active, then only files whose filename matches "london" *and* that carry the tag "alice" are shown (AND logic).
- **AC-6**: Given a collection with existing tags, when the user types `#tra`, then an autocompletion suggestion of "travel" (or similar matching tag) appears as dimmed inline text.
- **AC-7**: Given a visible autocompletion suggestion, when the user presses Tab, then the suggestion text fills the input.
- **AC-8**: Given the user is in tag-typing mode with an empty input, when the user presses Backspace, then tag-typing mode is exited but no confirmed tags are removed.
- **AC-9**: Given no typing mode is active and no text is present, when the user presses Backspace, then the most recently confirmed tag or type is removed.
- **AC-10**: Given an already-confirmed tag "travel", when the user confirms "travel" again (case-insensitive), then no duplicate is added.
- **AC-11**: Given a collection, when the user types `@person` and confirms, then only files that carry a tag of type "person" are shown.
- **AC-12**: Given confirmed types `@person` and `@camera`, then files carrying a tag of *either* type are shown (OR logic).
- **AC-13**: Given a file with a tag of type "person", when the user types `@PERSON` and confirms, then that file matches (case-insensitive).
- **AC-14**: Given confirmed type `@person` and confirmed tag `#alice`, then only files that have *both* a person-type tag *and* the tag "alice" are shown.
- **AC-15**: Given a collection with existing tag types, when the user types `@per`, then an autocompletion suggestion appears.
- **AC-16**: Given the user is in `@` mode with a partial input, when the user types `#`, then `@` mode is exited, the partial input is discarded, and `#` mode begins.
- **AC-17**: Given any active filter (text, tags, types), when the user presses Esc, then all filter state is cleared and the full unfiltered list is restored.
- **AC-18**: Given confirmed tags `#alice` and `#travel` and confirmed type `@person` and text "london", when the filter bar is displayed, then it renders the full boolean expression with distinct visual treatment for each component — e.g. `/london AND @person AND (#alice OR #travel)` — with text emphasised, connectives subdued, types in one distinct colour, and tags in another.

## Success metrics

- Users can compose a three-dimensional filter (text + types + tags) within seconds using only the keyboard.
- Zero false positives or false negatives: the displayed list always matches the boolean expression shown in the filter bar.
- Autocompletion reduces keystrokes by at least 50% for tags/types longer than 4 characters.

## Constraints

- The filter bar must not overflow or truncate the boolean expression in a way that hides active filter criteria from the user.
- Mode-switching (`#` ↔ `@`) must never silently confirm a partially typed value.

## Traceability

| Source | Reference |
|--------|-----------|
| UC-05  | mex/spec/UC-05-tag-filtering.md |
| PRD-02 | prd/PRD-02-browse.md (FR-5, FR-6 — filter mode baseline) |

## Open questions

- Should the system support removing a *specific* confirmed tag/type (e.g. via a dedicated keystroke), or is sequential Backspace removal sufficient?
- When the filter bar expression exceeds the available width, how should overflow be handled — truncation with an indicator, or horizontal scrolling?
