---
description: Complete task 4.1 offline integration acceptance for migrate-to-jev-zen-with-retries, including verification, review and necessary fixes.
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

Complete exact textual task 4.1 of change `migrate-to-jev-zen-with-retries` in workspace `/home/andrei/IdeaProjects/github/m407/jev-grep`, using its repo-local OpenSpec store (no store ID). The authoritative artifacts are `openspec/changes/migrate-to-jev-zen-with-retries/tasks.md`, `proposal.md`, `design.md`, and `specs/**/*.md` under that same change directory; consult the synchronized main specs in `openspec/specs/` as relevant.

Read the current task and relevant context, respect its dependencies, and autonomously finish its offline acceptance work, including required verification, scope-only review, fixes, and its task checkbox. Follow the artifacts' requirements for reusing successful checks and preserving unrelated work, including the old change and its agents; do not perform the separate live-smoke task. Choose your own tools, skills, approach and delegation, calling specialists and independent reviewers directly when useful. Ask for help only for a real blocker or a decision you cannot make. End with a concise report of the result, checks and any unresolved issue.
