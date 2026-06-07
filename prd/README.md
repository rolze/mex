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
| [PRD-02](PRD-02-browse.md) | Browse Media Files | `Implemented` |
| [PRD-03](PRD-03-themes.md) | Dynamic Theme Rotation | `Implemented` |
| [PRD-04](PRD-04-navigation-and-zoom.md) | Navigation & Semantic Zoom | `Implemented` |

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
