---
description: Complete task 2.1 of support-system-root-certificates, recording native certificate-store verification and reporting checks and unresolved issues.
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

Autonomously complete exact textual task 2.1 of `support-system-root-certificates` in `/home/andrei/IdeaProjects/github/m407/jev-grep` (repo-local OpenSpec; no store). Read the current task in `openspec/changes/support-system-root-certificates/tasks.md`, its sibling `proposal.md`, `design.md`, and `specs/jev-zen-integration/spec.md`, and the relevant procedures in `docs/deployment.md` and `docs/ru/deployment.md`; these artifacts are authoritative for requirements and verification recorded in the change's `verification.md`.

Respect the task's dependencies and finish its work, required verification, independent review, fixes, and task checkbox only when its requirements are met. Preserve unrelated work. Choose your tools, skills, approach, and delegation freely, including direct calls to specialists and reviewers; ask for help only for a real blocker or a decision you cannot make. End with a concise report of the result, checks, and any unresolved issue.
