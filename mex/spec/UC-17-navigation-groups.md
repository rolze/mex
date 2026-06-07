## UC-17 · Progressive Semantic Zoom

**Actor:** User  
**Goal:** Quickly scan, progressively zoom out to high-level timelines, and drill down into specific periods.

**Preconditions:** `.mex.db` exists.

**Main Flow:**
1. The user navigates the file list.
2. `Left arrow` progressively zooms out contextually: collapsing the current Item to a Slug, then Slug to Month, then Month to Year.
3. Expanded items display inline. Group summaries (Slug, Month, Year) are indicated with visual prefixes (e.g. `▶ 2022`).
4. `Right arrow` progressively zooms in: on a collapsed group, it expands it (contextual zoom in); on an expanded item, it expands siblings (e.g., all slugs in month, then all months in year) up to a global flat list.
5. The status bar provides hints on what the next action will accomplish.
6. The user can seamlessly traverse from a single item up to a decades overview using only Left/Right arrows, preserving context.

**Visual indicators:**
- Summary rows have different indentations and colours depending on the zoom level (Year is yellow bold, Month is cyan, Slug is gray).
- Expanded groups do not use deep hierarchical visual indentation for items, keeping horizontal space efficient.

**Acceptance Criteria:**
- Left arrow contextually zooms out to Slug, Month, and Year sequentially.
- Right arrow expands groups contextually or cascades outward on items.
- Cursor tracks the collapsed group on zoom out.
- UI styling correctly discriminates between Year, Month, and Slug headers.
