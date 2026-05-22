## UC-05 · Tag and tag-type filtering with # and @ prefixes in filter bar

**Actor:** User  
**Goal:** Filter by tag name (`#`) and/or tag type (`@`)

**Main Flow — tag filter (`#`):**
1. User can filter by tags if he starts with hashtag #
2. Tool responds with auto suggestions of available tags (inline dim text after current input)
3. User can cycle through suggestions with Up/Down
4. Tab completes the current suggestion into the input
5. User confirms with Enter to complete the current tag selection
6. User can add multiple tags, get ORed
7. User can still filter by text in filename (but text no longer searches in tags)
8. Text filter and tag filters are combined with AND logic
9. Backspace on empty tag input exits tag-typing mode; Backspace with no text and no tag-typing removes the last confirmed tag
10. Esc resets the entire filter (all text, all confirmed tags/types, tag-typing mode)

**Main Flow — tag-type filter (`@`):**
1. User types `@` to enter tag-type-typing mode (mirrors `#` flow)
2. Tool responds with auto suggestions of available tag types (inline dim text)
3. User can cycle through suggestions with Up/Down; Tab to complete; Enter to confirm
4. User can add multiple types, get ORed
5. Backspace on empty type input exits type-typing mode
6. Typing `#` while in `@` mode (or vice versa) silently exits the other mode

**Combined filter logic:**
```
text AND (@types OR …) AND (#tags OR …)
```

**Filter bar display:**
- Empty: hint text
- Active: full boolean expression rendered inline:
  - `/text` (white bold) — text part
  - `AND` / `OR` / `(` / `)` (dim gray) — connectives
  - `@type` (magenta bold) — tag-type tokens; group wrapped in `( … OR … )` when >1
  - `#tag` (cyan bold) — tag-name tokens; group wrapped in `( … OR … )` when >1
  - dim autocomplete suffix shown while typing
  - cursor `_` at end
- Examples:
  - `/london AND (@person OR @camera) AND (#alice OR #travel)_`
  - `/london AND @person AND #alice_`
  - `@person_`

**Acceptance Criteria:**

| Test | What it verifies |
|---|---|
| `tag_filter_single` | `#travel` shows only files tagged "travel" |
| `tag_filter_or_logic` | `#travel #holiday` shows union of both tags |
| `tag_filter_case_insensitive` | `#TRAVEL` matches file tagged "travel" |
| `text_filter_skips_tags` | plain text does not match tag names |
| `combined_filter_and_logic` | text + tag both must match |
| `tag_autocomplete_suggestion` | typing `#tra` returns "travel" as suggestion |
| `tab_complete_fills_input` | Tab fills `tag_input` with suggestion |
| `backspace_exits_tag_mode` | Backspace on empty `tag_input` exits tag_typing |
| `backspace_removes_last_tag` | Backspace on empty `filter_text` and no `tag_typing` pops last confirmed tag |
| `confirm_tag_adds_to_filters` | Enter adds chosen suggestion to `tag_filters` |
| `confirm_tag_no_duplicates` | Confirming an already-active tag (case-insensitive) is a no-op |
| `clear_filter_resets_all` | `clear_filter` empties all filter state |
| `type_filter_single` | `@person` shows only files that have a tag of type "person" |
| `type_filter_or_logic` | `@person @camera` shows union (files with person-type OR camera-type tag) |
| `type_filter_case_insensitive` | `@PERSON` matches files with type "person" |
| `type_and_tag_combined` | `@person #alice` requires both a person-type tag AND the tag "alice" |
| `type_autocomplete_suggestion` | typing `@per` returns "person" as suggestion |
| `type_tab_complete` | Tab fills `tag_type_input` with suggestion |
| `type_backspace_exits_mode` | Backspace on empty `tag_type_input` exits type-typing mode |
| `type_confirm_no_duplicates` | Confirming already-active type is a no-op |
| `switch_modes_abandons_partial` | Typing `#` while in `@` mode (or vice versa) abandons the partial input |
