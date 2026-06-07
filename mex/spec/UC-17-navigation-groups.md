## UC-17 · Collapsible Navigation Groups

**Actor:** User  
**Goal:** Quickly scan and navigate past large groups of related files.

**Preconditions:** `.mex.db` exists.

**Main Flow:**
1. The user navigates the file list. Items belonging to the same group key (`yyyy-MM-<slug>` or `yyyy-MM-DD`) appear contiguous in the list.
2. `Left arrow` on a file belonging to a group collapses the entire group into a single summary row and moves the cursor to that summary row.
3. The summary row displays the group key, a summary of its contents (e.g. `(4 images, 3 videos)`), and a `+` in the tags column to indicate hidden items.
4. `Right arrow` on a summary row expands the group back to individual items and places the cursor on the first item.
5. While the cursor rests on a summary row, file-specific operations (tagging, viewing) are disabled.
6. Pressing `Left arrow` on an item that does not belong to a group has no effect.
7. Expanding and collapsing operations preserve the overall file list filter and selection state.

**Visual indicators:**
- Summary row has the group key and media count in the filename column.
- The tag column shows a dim `+` instead of tags.
- Cursor styling applies normally to the summary row.

**Acceptance Criteria:**
- Left arrow collapses a group into a summary row in under 16ms.
- Right arrow expands a group back out.
- Cursor is correctly positioned on collapse (to the summary row) and expand (to the first item of the group).
- Attempting to view or tag a summary row gracefully does nothing.
- Unrelated items remain visible and selectable.
