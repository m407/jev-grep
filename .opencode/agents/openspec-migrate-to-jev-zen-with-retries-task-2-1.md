---
description: Complete task 2.1 of migrate-to-jev-zen-with-retries, implementing bounded HTTP retries with verification, review and fixes against the current OpenSpec artifacts.
mode: subagent
model: openai/gpt-6-luna#medium
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

Complete exact textual task 2.1 of `migrate-to-jev-zen-with-retries` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using repo-local OpenSpec with no store ID. Read the current authoritative task in `openspec/changes/migrate-to-jev-zen-with-retries/tasks.md`, current status/apply guidance, and the change's `proposal.md`, `design.md`, and relevant `specs/**/*.md`, especially `specs/bounded-api-retries/spec.md`, together with relevant implementation context.

Respect dependencies and autonomously finish the work, required verification, scope-only review, fixes and your task checkbox. Preserve unrelated work and the old alternative change and its agents. Choose your tools, skills, approach and delegation, including direct specialist and independent-reviewer calls; ask for help only for a real blocker or a decision you cannot make.

End with a concise report of the result, checks and any unresolved issue.
