# Sem & Mex — Product Requirements Documents

Technology-agnostic requirements derived from the use cases in `<variant>/spec/`.

These documents describe **what** the product must do, not **how**.
They are owned by the `requirements-engineer` agent and serve as the
contract between human intent and agent implementation.

## Status legend

| Status | Meaning |
|--------|---------|
| `Draft` | PRD created, awaiting review |
| `Approved` | Human-reviewed, ready for implementation |
| `Implemented` | Shipped in current codebase |
| `Superseded` | Replaced by a newer PRD |

## Index

| Document | Title | Status |
|----------|-------|--------|
| [PRD-00](PRD-00-concepts-and-ux.md) | Core Concepts and UX Guidelines | `Implemented` |
| [PRD-01](PRD-01-config.md) | Configuration & Bootstrap | `Draft` |
| [PRD-02](PRD-02-browse.md) | Browse & Navigate File List | `Draft` |
| [PRD-03](PRD-03-themes.md) | Dynamic Theme Rotation | `Implemented` |
| [PRD-04](PRD-04-navigation-and-zoom.md) | Navigation & Semantic Zoom | `Implemented` |
| [PRD-05](PRD-05-file-details.md) | File Details Panel | `Draft` |
| [PRD-06](PRD-06-selection.md) | File Selection & Bulk Operations | `Draft` |
| [PRD-07](PRD-07-tag-type-filter.md) | Tag & Type Filtering | `Draft` |
| [PRD-08](PRD-08-external-viewer.md) | External Viewer Launch | `Draft` |
| [PRD-09](PRD-09-create-view.md) | Create View of Selection | `Draft` |
| [PRD-10](PRD-10-fix-date.md) | Fix Date Prefix | `Draft` |
| [PRD-11](PRD-11-smart-import.md) | Smart Import | `Draft` |
| [PRD-12](PRD-12-tags.md) | Assign & Remove Tags | `Draft` |
| [PRD-13](PRD-13-fix-extension.md) | Fix File Extension | `Draft` |
| [PRD-14](PRD-14-trash-delete.md) | Trash & Delete | `Draft` |
| [PRD-15](PRD-15-mpv.md) | Video Player Integration | `Draft` |
| [PRD-16](PRD-16-caption.md) | Caption Files | `Draft` |
| [PRD-17](PRD-17-sem-viewer.md) | Image Viewer | `Draft` |
| [PRD-18](PRD-18-slugify.md) | Filename Slugification | `Draft` |

## Relationship to <variant>/spec/

```
<variant>/spec/UC-XX.md          →  Human-written use cases (implementation-flavoured)
    ↓ requirements-engineer distills
prd/PRD-XX-name.md     →  Technology-agnostic requirements (agent-owned)
    ↓ drives
implementation plan    →  How to build it (architect + developer)
```

The UC docs remain the human's voice. PRDs are the agent-consumable
translation with testable acceptance criteria. When a UC and PRD conflict,
the human resolves it — the requirements engineer escalates.
