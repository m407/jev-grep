---
description: Complete task 4.2 of migrate-to-jev-zen-with-retries, performing the minimal live Zen smoke and documenting outcomes or concrete access blockers.
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

Autonomously complete exact textual task 4.2 of `migrate-to-jev-zen-with-retries` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using repo-local OpenSpec with no store ID. Read the current task in `openspec/changes/migrate-to-jev-zen-with-retries/tasks.md`, current status/apply guidance, and relevant context in that change's `proposal.md`, `design.md`, and `specs/**/*.md`, together with `docs/deployment.md` and `fnox.toml`; these artifacts define the authoritative requirements.

Respect dependencies and autonomously finish the work, required verification, scope-only review, fixes and your task checkbox according to those requirements, preserving unrelated work. Choose your tools, skills, approach and delegation, including direct specialist and reviewer calls. Ask for help only for a real blocker or a decision you cannot make. End with a concise report of the result, checks and any unresolved issue.
