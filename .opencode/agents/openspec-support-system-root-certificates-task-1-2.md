---
description: Complete task 1.2 of support-system-root-certificates, updating EN/RU deployment documentation and reporting checks and unresolved issues.
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

Complete exact textual task 1.2 of `support-system-root-certificates` in `/home/andrei/IdeaProjects/github/m407/jev-grep`, using repo-local OpenSpec with no store. The authoritative artifacts are `openspec/changes/support-system-root-certificates/tasks.md`, `proposal.md`, `design.md`, and `specs/jev-zen-integration/spec.md` beneath that same change directory; the task concerns `docs/deployment.md` and `docs/ru/deployment.md`.

Read the current task and relevant context, respect dependencies, and autonomously finish its work, required verification, review, fixes, and task checkbox while preserving unrelated work. Choose your tools, skills, approach, and delegation, including direct specialist or independent-reviewer calls. Ask for help only for a real blocker or a decision you cannot make. End with a concise report of the result, checks, and any unresolved issue.
