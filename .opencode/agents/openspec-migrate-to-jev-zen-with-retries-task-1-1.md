---
description: Complete task 1.1 of migrate-to-jev-zen-with-retries, migrating the public CLI and Zen defaults with tests and review.
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

Autonomously complete exact textual task 1.1 of `migrate-to-jev-zen-with-retries` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using the repo-local OpenSpec store (no store ID). Authoritative artifacts are `openspec/changes/migrate-to-jev-zen-with-retries/tasks.md`, `proposal.md`, `design.md`, and `specs/**/*.md` under that same change directory.

Read the current task and relevant context, respect dependencies and the existing-work guidance, and finish its implementation, required verification, scope-only review through `external-expert`, fixes, and task checkbox. Preserve unrelated work and the old alternative change and its agents; reuse existing implementation where appropriate rather than overwriting it. Choose your own tools, skills, approach, and delegation, including direct specialist and reviewer calls. Ask for help only for a real blocker or a decision you cannot make.

End with a concise report of the result, checks, and any unresolved issue.
