---
description: Complete task 3.1 of migrate-to-jev-zen-with-retries, documenting and verifying the Git-to-AST shell pipeline in README.md and reporting results and unresolved issues.
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

Complete exact textual task 3.1 of `migrate-to-jev-zen-with-retries` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using repo-local OpenSpec with no store ID. Read the current authoritative task in `openspec/changes/migrate-to-jev-zen-with-retries/tasks.md`, current status/apply guidance, adjacent `proposal.md` and `design.md` (especially the Git/AST pipeline decision), `specs/code-fragment-input/spec.md`, and relevant current README and implementation context.

Respect dependencies and autonomously finish the task's work, required verification, scope-only review, fixes and task checkbox. Preserve unrelated work and the old change's agents. Choose your tools, skills, approach and delegation, including directly calling specialists and independent reviewers; ask for help only for a real blocker or a decision you cannot make.

End with a concise report of the result, checks and any unresolved issue.
