# PRD-03: Dynamic Theme Rotation

| Meta | Data |
|------|------|
| **Status** | `Implemented` |

## 1. Overview
The application must provide a premium, personalized visual experience by allowing the user to dynamically cycle through curated aesthetic themes at runtime without restarting the application.

## 2. Requirements

**FR-1: Multiple Themes**
The application must ship with at least three distinct, curated color palettes (themes). For example:
- A warm/amber default.
- A cyberpunk/neon aesthetic.
- A calm/cool forest aesthetic.

**FR-2: Comprehensive Semantic Highlighting**
Every theme must provide a complete, cohesive palette that guarantees specific UI elements stand out from the primary text. The following elements MUST be assigned distinct, prominent colors or modifiers:
- **Semantic File Components**: Slugs, Captions, and Tags must be visually distinct from each other and the base filename.
- **Navigation & Selection**: The active cursor position and batch-selected items must be immediately identifiable using distinct background colors.
- **Structural Layout**: Borders and Titles for layout blocks (e.g., Filter, Command, Status) must be rendered clearly to delineate interactive areas.
- **State Indicators**: Missing files, text filter matches, and success/error status messages must use high-contrast alert colors (e.g., red/green equivalents for the specific theme).

**FR-3: Dynamic Runtime Rotation**
The user must be able to switch the active theme instantly during normal execution.
- Keybinding: Pressing `t` in Normal mode must cycle the active theme to the next one in the list, wrapping around to the first theme when reaching the end.

**NFR-1: Visual Harmony and Legibility**
Cursor highlighting must never render text illegible. Foreground semantic colors (like slugs and captions) must remain distinctly readable against the cursor's background color in every theme.

## 3. Acceptance Criteria

- **AC-1**: Given the application is running in Normal mode, when the user presses `t`, then the application's entire color palette updates instantly to the next theme without requiring a restart.
- **AC-2**: Given a list of media items, when the cursor is moved over an item, then the semantic colors of the item's slug and caption remain readable against the theme's specific cursor background color.
- **AC-3**: Given the application is running, when the user presses `t` repeatedly, then the application cycles through the distinct color themes and wraps back to the initial theme seamlessly.
