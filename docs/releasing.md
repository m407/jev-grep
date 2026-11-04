# Automatic releases and Conventional Commits

**English** | [Русский](ru/releasing.md) · [Documentation](README.md)

Every push to `main` starts [Release](../.github/workflows/release.yml).
`semantic-release` analyzes commits after the last `v*` tag, determines the
next version, and generates release notes. No manual version bump is required.

## Dependabot auto-merge

Pull requests and releases run the shared `mise run check` aggregate described
in the [development guide](development.md), with Conventional Commit validation.
After the entire PR workflow succeeds,
[Dependabot auto-merge](../.github/workflows/dependabot-auto-merge.yml)
squash-merges Dependabot PRs into `main`, including major and security updates.
It verifies the author, source repository, and tested head SHA before merging.
Failed checks and merge conflicts block automatic merging.

[Dependabot configuration](../.github/dependabot.yml) covers Cargo, npm, and
GitHub Actions with a cooldown for version updates. This does not replace the
successful-workflow and exact-head-SHA merge gate. Scheduled OSV scanning is
described in the [development guide](development.md#security-and-findings).

The merge workflow uses `workflow_run` and the built-in `GITHUB_TOKEN`, without
checking out PR code or requiring a personal token. It explicitly dispatches
Release on `main` afterward because merges made with `GITHUB_TOKEN` do not
trigger push workflows. The successful CI workflow is the merge gate; this
does not depend on branch protection being enabled.

## Commit format

```text
fix: correct timeout handling
feat(cli): add a new output format
docs: clarify installation
feat(api)!: change the response format
```

| Change | Version |
| --- | --- |
| `fix:`, `perf:` | patch |
| `feat:` | minor |
| `!` after type/scope or a `BREAKING CHANGE: ...` footer | major |
| `docs:`, `chore:`, `test:`, `ci:`, `build:`, `refactor:`, `style:` without a breaking change | No release |

The largest bump among all new commits wins. A breaking change means major
even for `0.x` versions. PR titles and all PR commits are checked by
`commitlint`; preserve a Conventional Commit in the final squash-merge
message. Commits in each push to `main` are also checked before release.

After [development setup](development.md#setup), the installed commit-msg hook
checks local commit messages. For a manual check and full validation:

```sh
mise run check:commits -- --last --verbose
mise run check
```

`commitlint` is an alias for the `check:commits` task.

## What the workflow does

1. Checks commit messages and runs `mise run check`, including the existing
   release-tooling, Rust, and Python tests and the shared quality gates.
2. Computes the version with a semantic-release dry run. If no release is
   needed, archive builds and publication are skipped.
3. Updates package versions in `Cargo.toml` and `Cargo.lock` for each build,
   preserving dependency versions. Builds with `cargo build --release --locked`:

   | Platform | Target | Archive |
   | --- | --- | --- |
   | Linux x86-64 (glibc, Ubuntu 24.04) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
   | macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
   | Windows x86-64 | `x86_64-pc-windows-msvc` | `.zip` |

4. Checks `--help` and that `--version` exactly matches the computed version.
5. Only after all builds succeed, creates `SHA256SUMS` and rechecks the computed
   version before publication.
6. Commits `Cargo.toml` and `Cargo.lock` to `main` with the message
   `chore(release): X.Y.Z [skip ci]`, creates a `vX.Y.Z` tag, and publishes a
   GitHub Release with archives and release notes. semantic-release manages
   publication.

Archives contain the binary and README. Verify downloaded files on Linux:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Builds and publication run in one workflow: no separate tag-triggered run is
needed. The workflow uses the built-in `GITHUB_TOKEN`; Zen access and a model
key are not required. Neither an npm package nor a crates.io package is
published.

## First release and GitHub settings

Without previous release tags, semantic-release starts at **1.0.0**, regardless
of the current version in `Cargo.toml`. To continue the `0.1.x` line, first tag
an already released commit with `v0.1.0` and push that tag to GitHub. The next
release is computed from commits after it.

GitHub Actions must have contents write access. If `main` is protected, allow
the release bot to push the version commit; otherwise publication stops.
Making `Conventional Commits / commitlint` a required PR check and configuring
squash merge to use the PR title as its commit message are recommended. These
settings are configured in GitHub, not in YAML files.

Release runs are serialized. If `main` advances during a build, an outdated
run may not publish; the next run handles the new commits. Release can also be
started manually on `main` to retry.

If a failure occurs after pushing the version commit/tag, check the existing
tag and GitHub Release before retrying: semantic-release does not automatically
republish an already tagged version. Saved workflow artifacts can be used to
restore missing release assets.
