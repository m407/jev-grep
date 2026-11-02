---
description: Complete task 1.1 of support-system-root-certificates, implementing native-root support and local TLS coverage with verification and review.
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

Complete exact textual task 1.1 of `support-system-root-certificates` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using repo-local OpenSpec with no store. Read the current task in `openspec/changes/support-system-root-certificates/tasks.md` and relevant context in that change's `proposal.md`, `design.md`, and `specs/jev-zen-integration/spec.md`, together with applicable existing specs and implementation.

Respect dependencies and autonomously finish this task, including its required verification, review, fixes, and task checkbox; preserve unrelated work. Choose your tools, skills, approach, and delegation freely, including direct specialist and independent-reviewer calls. Ask for help only for a real blocker or a decision you cannot make. End with a concise report of the result, checks, and any unresolved issue.
