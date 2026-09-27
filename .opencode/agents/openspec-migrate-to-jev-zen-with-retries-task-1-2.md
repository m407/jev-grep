---
description: Complete task 1.2 of migrate-to-jev-zen-with-retries, preparing fnox keychain configuration and Zen deployment documentation with verification and review.
mode: subagent
model: openai/gpt-6-luna#low
permissions:
  - action: "*"
    resource: "*"
    effect: allow
  - action: external_directory
    resource: "*"
    effect: ask
  - action: read
    resource: "*.env"
    effect: ask
  - action: read
    resource: "*.env.*"
    effect: ask
  - action: read
    resource: "*.env.example"
    effect: allow
  - action: external_directory
    resource: "/home/andrei/.local/share/opencode/shell/*/*"
    effect: allow
  - action: external_directory
    resource: "/home/andrei/.local/share/opencode/tool-output/*"
    effect: allow
  - action: external_directory
    resource: "/tmp/opencode/*"
    effect: allow
  - action: external_directory
    resource: "/home/andrei/.config/opencode/*"
    effect: allow
  - action: question
    resource: "*"
    effect: allow
  - action: browser
    resource: "*"
    effect: deny
---

Autonomously complete exact textual task 1.2 of `migrate-to-jev-zen-with-retries` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using the repo-local OpenSpec store (no store ID). Read the current task in `openspec/changes/migrate-to-jev-zen-with-retries/tasks.md`, its `proposal.md`, `design.md`, and relevant `specs/**/*.md`, especially `specs/jev-zen-integration/spec.md`, as the authoritative requirements.

Respect dependencies and existing work, including the overlapping-work context in the artifacts; preserve unrelated changes and old-change agents. Finish this task's implementation, required verification, scope-only review, fixes, and its task checkbox according to those artifacts. Choose your tools, skills, approach, and direct specialist or reviewer delegation freely; ask for help only for a real blocker or a decision you cannot make.

End with a concise report of the result, checks performed, and any unresolved issue.
