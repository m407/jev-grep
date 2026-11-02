---
description: Complete task 2.2 of support-system-root-certificates using its current artifacts; perform final acceptance and report checks, results and blockers.
mode: subagent
model: openai/gpt-6-luna#low
permissions:
  - action: "*"
    resource: "*"
    effect: allow
  - action: read
    resource: "*.env"
    effect: ask
  - action: read
    resource: "*.env.*"
    effect: ask
  - action: read
    resource: "*.env.example"
    effect: allow
  - action: question
    resource: "*"
    effect: allow
  - action: browser
    resource: "*"
    effect: deny
  - action: skill
    resource: ccc
    effect: allow
  - action: browser
    resource: "*"
    effect: deny
---

Own exact textual task 2.2 of `support-system-root-certificates` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using repo-local OpenSpec with no store. Read the current task in `openspec/changes/support-system-root-certificates/tasks.md`, its `proposal.md`, `design.md`, `specs/jev-zen-integration/spec.md`, and existing `verification.md` when present, together with the live-smoke contract in `openspec/specs/jev-zen-integration/spec.md` and relevant deployment documentation.

Respect task dependencies and autonomously finish the assigned work, required verification, review and fixes, recording the required results in the change's `verification.md` and updating only your task checkbox when its requirements are satisfied. Preserve unrelated work. Choose your tools, skills, approach and delegation, including direct specialist and independent-reviewer calls; ask for help only for a real blocker or a decision you cannot make.

End with a concise report of the result, checks and any unresolved issue.
