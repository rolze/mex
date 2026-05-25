# Sem & Mex — Product Requirements Documents

Technology-agnostic requirements derived from the use cases in `spec/`.

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

_No PRDs yet. The requirements engineer will populate this index as PRDs are created._

## Relationship to spec/

```
spec/UC-XX.md          →  Human-written use cases (implementation-flavoured)
    ↓ requirements-engineer distills
prd/PRD-XX-name.md     →  Technology-agnostic requirements (agent-owned)
    ↓ drives
implementation plan    →  How to build it (architect + developer)
```

The UC docs remain the human's voice. PRDs are the agent-consumable
translation with testable acceptance criteria. When a UC and PRD conflict,
the human resolves it — the requirements engineer escalates.
