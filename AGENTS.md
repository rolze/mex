# Agent Instructions

## UC document sync

Every UC document (`spec/UC-XX.md`) is the source of truth for its feature.

**Rule: every implementation change must be reflected in the corresponding UC document in the same commit.**

- If you add, remove, or change a behaviour, update the UC doc to match.
- UC docs must be brief and concise — describe *what is implemented*, not aspirations.
- Remove outdated details immediately; do not leave stale text.
- If a change spans multiple UCs, update all affected docs.

This keeps the UC files useful as a living reference rather than historical artefacts.
