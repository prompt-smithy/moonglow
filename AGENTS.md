# Multi-Agent Collaboration Protocol (Muxage Protocol)

All AI agents working in this repository must follow this protocol for seamless multi-agent relay without user copy-pasting.

## 3-Tier Document Hierarchy
1. **`outcome.md` (Ephemeral Relay Baton)**:
   - Overwritten on every iteration (never appended).
   - If an agent performs a review and finds issues, describe actionable findings and test failures in `outcome.md`.
   - If the review passes with NO findings or all tasks are verified complete, **`outcome.md` MUST BE EMPTY (0 bytes)**.
   - When an implementing agent completes fixes, it **must commit code changes together with `outcome.md`**.

2. **`system.md` (Master Architectural State of Truth)**:
   - Living specification of system architecture, components, and data flow.
   - Contains a living **Mermaid diagram** representing actual functionality and relationships.
   - Any architectural change, new module, or changed contract **must update `system.md`**.

3. **`README.md` (Customer-Facing Overview)**:
   - High-level project purpose, quickstart instructions, and user-facing features.
   - Kept clean and customer-oriented.

## Agent Relay Rules
- **Rule 1 (Inspect Baton)**: At the start of a turn, always read `outcome.md`. If non-empty, address the findings.
- **Rule 2 (Reviewer Hand-off)**: When reviewing, check git diff against `system.md`. Write findings to `outcome.md`. If clean, truncate `outcome.md` to empty.
- **Rule 3 (Commit Baton)**: Always commit code changes alongside `outcome.md` so the commit history captures the baton state.
- **Rule 4 (Keep Architecture Current)**: If modifying behavior or contracts, update `system.md` and the Mermaid diagram.
