# Development checks

**English** | [Русский](ru/development.md) · [Documentation](README.md)

## Setup

Install [mise](https://mise.jdx.dev/getting-started.html) **2026.11.1 or newer**
before running repository commands: an older mise cannot load this configuration,
even though the project also pins mise. Review `mise.toml` and `hk.pkl`, then
run from the repository root; no global mise configuration changes are needed:

```sh
mise trust
mise install
mise exec -- npm ci
mise run hooks:install
```

Native hk hook launchers resolve mise through `PATH`. Every developer terminal
and GUI Git client must have mise **2026.11.1 or newer** available on its `PATH`.

`mise.toml` pins the toolchain and defines tasks; `hk.pkl` shares checks between
Git hooks and whole-project validation. Use the pinned tools rather than local
global versions. Re-run `npm ci` through mise after npm lockfile changes.
Python 3.12.16 is primary for acceptance/release tooling; Python 3.14.10 is
also installed for the agent-generator helper.

## Checks

```sh
mise run check
```

This is the complete CI aggregate and must pass before work is considered done.
Pre-commit runs fast staged-file checks without auto-fixing or staging changes;
commit-msg runs Commitlint; pre-push runs `mise run check:offline` across the
whole project. That task omits OSV requests and is useful for local iteration;
it does not replace the complete aggregate. Hooks must be installed in each clone.
Use `mise run fix` to apply supported fixes explicitly, then review the diff
and rerun checks.

The gates retain Rust formatting, Clippy, build/tests, Python acceptance tests,
and JavaScript release-tooling tests. They add Ruff for Python, Oxlint/oxfmt for
JavaScript, cargo-machete for unused Rust dependencies, jactionlint for workflows,
and a pinned stable Betterleaks release for secret scanning. The configuration
files are the source of truth for exact versions and checked paths.

`mise run check:release` runs `npm run test:release`
(`node --test scripts/*.test.mjs`). These release-automation tests use mocked GitHub APIs to
check orchestration, exact-head validation, publication, and retry behavior.
The full Rust and Python suites remain part of `mise run check`.

For hosted three-platform checks on a selected branch, use
[read-only build verification](releasing.md#read-only-build-verification).
It runs the full aggregate and uploads Linux/macOS/Windows archives without
creating or merging release PRs or publishing a release.

Acceptance tests use the Python standard library and a local mock HTTP API.
They check grouping with predefined model responses, input/output formats,
coordinates, limits, filters, and errors. Neither the Jev on OpenCode Zen server
nor model weights are required; these tests do not measure real model accuracy.

## Security and findings

```sh
mise run check:security
```

`mise run check` includes this security task: OSV-Scanner checks explicit
`Cargo.lock` and `package-lock.json` inputs, and the shared workflow lint checks
workflow policy and exception expiration. The scheduled/manual
[Security workflow](../.github/workflows/security.yml) also runs it independently.
Vulnerability advisory data can change after a successful aggregate run;
scheduled runs catch newly disclosed vulnerabilities and enforce the approved
workflow and OSV exception expirations even without code changes.

Fix findings at their source and rerun the affected check, then the aggregate.
If blocked, retain the complete failure output and report the blocker. Do not
add suppressions, baselines, disable rules, or skip checks without an explicit
human decision. An approved exception must record its reason, affected path,
and rule. Security exceptions must also have an expiration enforced
by the check; a date in a comment alone is insufficient.

The approved [jactionlint exception](../.github/jactionlint.yaml) covers only
`dangerous-triggers` in `.github/workflows/dependabot-auto-merge.yml`, whose
privileged `workflow_run` merges the exact tested Dependabot or generated
release-please PR head without executing PR code. Release PRs must also pass
the workflow/job, bot, repository, branch, label, and file guards described in
the [release guide](releasing.md#guarded-bot-auto-merge).
It expires on **2027-02-04**; an expired exception fails the workflow lint check.

The temporary [OSV exception](../osv-scanner.toml) covers
**GHSA-g3hv-75wg-m249** in `package-lock.json` through
`release-please@17.11.2 → @google-automations/git-file-utils@3.2.1 → minimatch@10.2.6`.
The fix starts at minimatch 11.0.1, outside the parent's `^10.2.4` range. The
user-approved ignore expires at **2026-12-05 00:00 UTC**; OSV stops ignoring this
advisory after that time. Remove it when a compatible upstream dependency fix
is available.

The migration removes 18 of the old dependency graph's 19 advisories; this one
remains temporarily accepted. `npm audit` still reports it even when the
configured OSV check passes. Passing checks do not mean zero vulnerabilities.
The new release-tooling dependency graph has no bundled dependencies; the npm
CLI itself is unchanged. The private root tooling package need not declare a
version: the released Rust version is tracked by Cargo and the release-please
manifest.

Update English and Russian guides together. For generated `openwiki/` pages,
follow `AGENTS.md`: update source/docs rather than hand-editing generated pages
unless explicitly asked.

Automatic releases from `main`, Conventional Commits, and release-tooling
checks are described in the [release guide](releasing.md).
