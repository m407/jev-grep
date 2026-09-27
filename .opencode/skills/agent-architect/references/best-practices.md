# Agent Design Best Practices

Patterns for creating effective OpenCode V2 agents. See `agent-spec.md` and the
official V2 guides for field names and runtime semantics.

## Single-Responsibility Principle

**Each agent = one clear goal.**

| Good | Bad |
|------|-----|
| "Reviews code for security vulnerabilities" | "Reviews code, writes tests, deploys" |
| "Extracts Mermaid diagrams from markdown" | "Processes all documentation" |
| "Validates pipeline outputs" | "Runs entire pipeline" |

**Benefits**:
- Easier to test and debug
- Clearer tool scoping
- Better auto-delegation by description

## Permission Hygiene

Express the required policy with ordered `permissions`, not a `tools` map.
Custom agents start with the base policy plus global configuration, not the parent's
or `build` agent's specific overrides. Add role restrictions and exceptions deliberately;
the last matching rule wins. Permissions do not create tools absent from the harness.

### Read-Heavy Agents (Analyzer, Reviewer, Planner)

```yaml
permissions:
  - action: edit
    resource: "*"
    effect: deny
  - action: shell
    resource: "*"
    effect: deny
  - action: subagent
    resource: "*"
    effect: deny
```

### Write-Enabled Agents (Implementer, Builder)

```yaml
permissions:
  - action: edit
    resource: "*"
    effect: allow
  - action: shell
    resource: "*"
    effect: ask
  - action: shell
    resource: "git status *"
    effect: allow
  - action: shell
    resource: "git diff *"
    effect: allow
  - action: shell
    resource: "npm test *"
    effect: allow
  - action: shell
    resource: "npm run build *"
    effect: allow
```

### Orchestrator Agents

```yaml
permissions:
  - action: edit
    resource: "*"
    effect: deny
  - action: shell
    resource: "*"
    effect: deny
  - action: subagent
    resource: "*"
    effect: allow
```

An orchestrator's denials apply to its own actions; children use their own policies.
For read-only delegation, allow only suitable read-only agents. `edit` controls both
file creation and modification. Do not combine a write allow with an edit deny.

## HITL (Human-in-the-Loop) Rules

Embed explicit "ask first" rules in agent prompts.

### Pattern

```markdown
# HITL Rules

- If acceptance criteria are ambiguous → ask numbered questions and WAIT
- If design implies public API change → STOP and request approval
- If refactors exceed ADR scope → ask before proceeding
- If tests require changes beyond scope → ask before modifying
```

### Examples by Role

**PM/Spec Agent**:
```markdown
# HITL Rules
- If requirements are incomplete → ask clarifying questions
- If scope is unclear → propose options and wait for selection
```

**Architect Agent**:
```markdown
# HITL Rules
- If design affects public API → STOP and request approval
- If migration risk detected → document and ask before finalizing
```

**Implementer Agent**:
```markdown
# HITL Rules
- If refactors exceed scope → ask before proceeding
- If tests fail unexpectedly → report and wait for guidance
- Only mark DONE if all tests pass
```

## Definition of Done

Every agent should have completion criteria.

### Pattern

```markdown
# Definition of Done

- [ ] All target files processed
- [ ] Issues documented with severity levels
- [ ] Recommendations provided with rationale
- [ ] Summary written in specified format
- [ ] No errors in execution log
```

### Examples

**Code Reviewer**:
```markdown
# Definition of Done
- [ ] All changed files reviewed
- [ ] Security issues flagged (if any)
- [ ] Performance concerns noted (if any)
- [ ] Code style violations listed
- [ ] Summary with approve/request-changes recommendation
```

**Test Runner**:
```markdown
# Definition of Done
- [ ] All tests executed
- [ ] Failed tests documented with error messages
- [ ] Coverage report generated (if applicable)
- [ ] Summary: passed/failed/skipped counts
```

## Description Writing

The `description` field is critical for auto-delegation.

### Good Descriptions

```yaml
# Action-oriented, specific triggers
description: Reviews code for security vulnerabilities and OWASP compliance. Use after code changes or before security audits.
```

```yaml
description: Extracts Mermaid diagrams from markdown files and saves as .mmd. Use when processing documentation with embedded diagrams.
```

```yaml
description: Validates pipeline outputs for completeness and quality. Use as final step before delivery.
```

### Bad Descriptions

```yaml
# Too vague
description: Helps with code
```

```yaml
# No trigger context
description: Security agent
```

```yaml
# Too broad
description: Does everything related to testing
```

## Agent Chaining Pattern

For complex workflows, use orchestrator + specialized subagents.

### Structure

```
orchestrator (primary)
    ├── spec-writer (subagent)
    ├── architect (subagent)
    ├── implementer (subagent)
    └── validator (subagent)
```

### Orchestrator Example

```markdown
---
description: Coordinates multi-step workflows by delegating to specialized subagents
mode: primary
permissions:
  - action: edit
    resource: "*"
    effect: deny
  - action: shell
    resource: "*"
    effect: deny
  - action: subagent
    resource: "*"
    effect: allow
---

# Role
Coordinate workflow phases by delegating to appropriate subagents.

# Process
1. Analyze request → determine required phases
2. Delegate to spec-writer → wait for completion
3. Delegate to architect → wait for completion
4. Delegate to implementer → wait for completion
5. Delegate to validator → report final status
```

## Model and Request Settings

Use `model: provider/model#variant` only when a specific available model or variant
is needed. Otherwise subagents inherit the parent's model. Agent request overlays
belong under `request.body` (for example, `temperature`), but the current V2 runner
does not send those overlays. Configure active settings on the provider/model/variant;
do not imply that a stored per-agent temperature guarantees a generation behavior.

## Common Anti-Patterns

### 1. Tool Sprawl
**Problem**: Assuming a role name or the parent's restrictions define child permissions.
**Fix**: Review the effective policy and add native V2 rules where the role needs them.

### 2. Missing HITL
**Problem**: Agent makes risky changes without asking.
**Fix**: Add HITL rules for destructive operations.

### 3. Vague Description
**Problem**: Agent not auto-delegated correctly.
**Fix**: Include action verbs and trigger contexts.

### 4. No Definition of Done
**Problem**: Unclear when agent task is complete.
**Fix**: Add explicit completion checklist.

### 5. Overly Broad Scope
**Problem**: Agent tries to do too much, fails at everything.
**Fix**: Split into focused single-responsibility agents.
