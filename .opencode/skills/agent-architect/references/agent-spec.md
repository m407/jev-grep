# OpenCode V2 Agent Reference

Sources of truth: [Agents](https://opencode.ai/v2/docs/agents),
[Permissions](https://opencode.ai/v2/docs/permissions), and
[V1 migration](https://opencode.ai/v2/docs/migrate-v1).
Fetch these guides when verifying semantics. The published `config.json` schema may
describe V1; do not use it to infer native V2 fields.

## Location and identity

| Scope | Path |
|-------|------|
| Project | `.opencode/agents/<name>.md` |
| Global | `~/.config/opencode/agents/<name>.md` |

The relative path determines the ID: `team/reviewer.md` becomes `team/reviewer`.
Do not put `name` in agent frontmatter. This differs from skill frontmatter, where
`name` is required. In JSONC, definitions go under `agents`, not `agent`.

## Native frontmatter fields

| Field | Shape and behavior |
|-------|--------------------|
| `description` | Purpose and invocation trigger; strongly recommended for subagent selection |
| `mode` | `primary`, `subagent`, or `all`; defaults to `primary` for a new custom agent |
| `model` | `provider/model` or `provider/model#variant`; omitted subagent model inherits the parent session model |
| `permissions` | Ordered list of `{action, resource, effect}` rules |
| `steps` | Positive integer model-step limit; final step removes tools and requests a text summary; new user input resets allowance |
| `hidden` | Boolean; hides the agent from normal listings, interactive discovery and the subagent catalog, not a security boundary |
| `disabled` | Boolean; removes the agent at that point in configuration loading |
| `color` | Quoted six-digit hex color, e.g. `"#ff6b6b"` |
| `request` | Per-agent headers/body overlays; currently stored but not sent by the V2 session runner |

The Markdown body becomes `system`. A nonempty agent system prompt replaces the
provider base prompt; project instructions, skills and other instruction sources
are still added. Use `system` in JSONC; keep Markdown instructions in the body.
Selecting a primary agent does not change the session's separately stored model.

```yaml
description: Reviews changed code for correctness. Use before merging.
mode: subagent
model: anthropic/claude-sonnet-4-5#high
permissions:
  - action: edit
    resource: "*"
    effect: deny
  - action: shell
    resource: "*"
    effect: deny
```

Specify a model only when requested or needed; verify its availability instead of
assuming the example model is installed. JSONC also supports the expanded model
form `{ "providerID": "anthropic", "model": "claude-sonnet-4-5", "variant": "high" }`.

### Request overlays

```yaml
request:
  headers:
    x-agent: reviewer
  body:
    temperature: 0.2
```

This is valid storage, not an active temperature control in the current V2 runner.
Configure active request settings on the provider, model or variant. Do not emit
top-level `temperature`, `top_p`, or arbitrary provider options.

## Permission semantics

Rules require three strings: `action`, `resource`, and `effect` (`allow`, `ask`, `deny`).
The last matching rule wins; global rules precede agent rules. Later agent definitions
append permissions rather than replacing the earlier array. Scalar fields replace
earlier values, while request maps merge by key.

```yaml
permissions:
  - action: shell
    resource: "*"
    effect: ask
  - action: shell
    resource: "git status *"
    effect: allow
  - action: shell
    resource: "git diff *"
    effect: allow
  - action: subagent
    resource: "*"
    effect: deny
  - action: subagent
    resource: reviewer
    effect: allow
```

Actions and resources support whole-value `*` and `?` wildcards. A shell resource
ending in ` *` also matches the bare command. `curl` alone does not allow arbitrary
arguments; use `curl *` for that intent. Shell text does not expand `~` or `$HOME`.

| Action | Resource |
|--------|----------|
| `read` | Internal location-relative path or canonical absolute external path |
| `edit` | Target path for edit, write and patch; no separate create-only rule |
| `shell` | Scanner-produced command text; a compound command may check multiple resources |
| `subagent` | Target child-agent ID |
| `skill` | Skill ID |
| `glob` | Requested glob pattern |
| `grep` | Requested regular expression, not the search directory |
| `webfetch` | URL |
| `websearch` | Query |
| `question` | `*` |
| `external_directory` | Canonical external directory boundary, normally ending in `/*` |
| `execute` | `*`; nested tools still enforce their own rules |
| `<server>_<tool>` | `*` for MCP tools; unsupported characters in names become `_` |

For example, a server named `chrome-devtools` uses `chrome_devtools_*` action patterns.
Plugins may define other actions; confirm their actual IDs. V2 Core has no `lsp` or
`doom_loop` action and does not expose V1 LSP functionality. Do not enumerate obsolete
`list`, `todowrite`, or `todoread` as guaranteed V2 tools.

For `read`, `edit` and `external_directory`, `~` and `$HOME` expand. Access outside
both the active location and its non-root project worktree also requires an
`external_directory` check. Multi-resource operations deny if any resource denies,
otherwise ask if any asks. Unmatched checks ask.

### Defaults and child agents

Every custom agent begins with the base policy: allow actions, ask for external
directories and sensitive `.env` reads, allow `.env.example`. Global and agent
rules can override it. There are also managed-directory exceptions.

Omitting permissions does not inherit agent-specific `build` overrides. A child
agent uses its own permissions, not a subset of the parent's. The parent's `subagent`
rules control which children it may launch. Do not claim a parent edit denial makes
its children read-only. Read the effective policy in the correct workspace when
exact equivalence is required.

## Built-in agents

| ID | Mode | Shipped behavior before later configuration overrides |
|----|------|------------------------------------------------------|
| `build` | primary | Base policy plus questions allowed |
| `plan` | primary | Denies normal edits, permits plan-file writes; shell remains permission-controlled |
| `general` | subagent | Broad access; questions and nested subagents denied |
| `explore` | subagent | Read/search/web access; external-directory and sensitive-file approvals |

Hidden `compaction`, `title`, and `summary` agents handle maintenance. V2 has no
built-in `scout` agent.

## Migration checklist

Supported V1 definitions are normalized automatically; native migration is optional.
When requested, convert each entire agent entry, preserving its body and identity.

| Legacy | Native V2 |
|--------|-----------|
| `permission` and `tools` | Ordered `permissions` list |
| `bash`, `task` | `shell`, `subagent` actions |
| `write`, `patch` | `edit` action; resolve conflicting legacy rules |
| `disable`, `maxSteps` | `disabled`, `steps` |
| Separate `variant` | Append `#variant` to `model` |
| `temperature`, `top_p`, provider options | `request.body` (current runtime limitation above) |
| JSON `prompt` | JSON `system`; Markdown body stays the prompt |
| `name` | Remove; identity is path-derived |

Do not mix native and legacy fields inside a single agent. Skill frontmatter is a
different format: do not apply these agent-field migrations to `SKILL.md` metadata.
