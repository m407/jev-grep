# Automatic releases and Conventional Commits

**English** | [Русский](ru/releasing.md) · [Documentation](README.md)

Every push to `main` starts [Release](../.github/workflows/release.yml).
The pinned npm package `release-please` **17.11.2** creates or updates a release
PR with the next version and changelog from Conventional Commits. Successful
exact-head CI allows that generated PR to be squash-merged automatically;
full validation of the merged snapshot and three-platform builds then gate
publication. No manual version bump or approval
of the generated release PR is required once the release GitHub App is configured.

[release-please-config.json](../release-please-config.json) configures the root
Rust package; [.release-please-manifest.json](../.release-please-manifest.json)
records its released version. The npm dependency is pinned in
[package-lock.json](../package-lock.json); this workflow does not use the
release-please GitHub Action. Release-please manages the release PR, versions,
and changelog; [repository tooling](../scripts/release.mjs) owns publication.

## Guarded bot auto-merge

Pull requests and releases run the shared `mise run check` aggregate described
in the [development guide](development.md), with Conventional Commit validation.
After the entire PR workflow succeeds,
[Bot PR auto-merge](../.github/workflows/dependabot-auto-merge.yml)
squash-merges Dependabot PRs into `main`, including major and security updates.
It verifies the author, source repository, and tested head SHA before merging.
Failed checks and merge conflicts block automatic merging.
The shared merge concurrency group uses `queue: max` so simultaneous successful
PR checks queue for merging instead of replacing pending merge runs.

The same guarded workflow also handles generated release PRs. It verifies the
Release workflow identity and successful validation jobs, the configured `<RELEASE_APP_SLUG>[bot]`
author, same repository, `main` base, `release-please--branches--main` source
branch, release labels, allowed files, and exact tested head SHA. Allowed release
files are `Cargo.toml`, `Cargo.lock`, `.release-please-manifest.json`, and
`CHANGELOG.md`. Ordinary contributor PRs, including the migration PR #5, are not
eligible for this automatic merge.

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

The largest bump among all new commits wins. PR titles and all PR commits are
checked by `commitlint`; preserve a Conventional Commit in the final squash-merge
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
   JavaScript release-automation, Rust, and Python tests and shared quality gates.
2. On `main`, preparation first looks for a merged release PR still labelled
   `autorelease: pending` and resumes its publication. Otherwise, release-please
   creates or updates the release PR, generating `CHANGELOG.md` and updating
   `Cargo.toml`, `Cargo.lock`, and the manifest together. Without a merged release
   awaiting publication, archive builds and publication are skipped.
3. Creates/updates PRs with a short-lived GitHub App token, so native PR CI starts
   without approval. Also explicitly dispatches Release validation for the exact
   head, including unchanged PRs, to support retries. These two validations can
   both run; the serialized merge workflow rechecks the current PR before merging.
   After successful validation, the guarded bot workflow
   squash-merges that exact head and explicitly dispatches Release on `main`.
4. Preparation outputs the merged release PR's immutable merge SHA and version.
   The separate `release-check` job runs the full `mise run check` aggregate on
   that SHA in parallel with the build matrix.
   Every platform checks out that SHA, verifies the declared version, and builds
   with `cargo build --release --locked`:

   | Platform | Target | Archive |
   | --- | --- | --- |
   | Linux x86-64 (glibc, Ubuntu 24.04) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
   | macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
   | Windows x86-64 | `x86_64-pc-windows-msvc` | `.zip` |

5. Checks `--help` and that `--version` exactly matches the release version.
6. Only after `release-check` and all three builds succeed, creates `SHA256SUMS`.
   The publisher validates the checkout, merged PR, version, exactly three
   archives, and their checksums.
   It creates `vX.Y.Z` at the immutable merge SHA or verifies the existing tag
   points there, then creates or resumes a draft GitHub Release.
7. Uploads the complete asset set and downloads it again to verify names, sizes,
   and checksums. Only then makes the draft public, replaces
   `autorelease: pending` with `autorelease: tagged`, and dispatches Release on
   `main` to reconcile any further work.

Archives contain the binary and README. Verify downloaded files on Linux:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Builds and publication run in one workflow after the release PR is merged: no
separate tag-triggered run is needed. PR creation uses a GitHub App installation
token; merging, publication and explicit dispatch use the built-in `GITHUB_TOKEN`.
A personal access token, Zen access, and a model key are not required.
Neither an npm package nor a crates.io package is
published.

## Version baseline and GitHub settings

The migration starts from the existing **1.1.0** root-package release. The
manifest records that baseline, and `bootstrap-sha` anchors initial commit
discovery at `34610d0bed5d374d0ccd0c1304f363614668cb30`. This does not force the
next version: Conventional Commits after the baseline determine the next bump.
Subsequent releases use release-please's merged release history.

### Release GitHub App (required)

Create a dedicated GitHub App under **Settings → Developer settings → GitHub Apps**.
Disable its webhook and grant repository **Contents: Read and write** and
**Pull requests: Read and write** (Metadata read access is implicit). Install it
on this repository only. Generate a private key and configure repository
**Settings → Secrets and variables → Actions**:

| Kind | Name | Value |
| --- | --- | --- |
| Variable | `RELEASE_APP_CLIENT_ID` | App Client ID from the App settings |
| Variable | `RELEASE_APP_SLUG` | Exact slug from `github.com/apps/<slug>`, without `[bot]` |
| Secret | `RELEASE_APP_PRIVATE_KEY` | Entire generated PEM private key |

The SHA-pinned `actions/create-github-app-token` action creates a token scoped to
the current repository with only those two write permissions and revokes it at
job completion. Only preparation on `main` receives the App key. PR CI and
read-only build verification need no App credentials. Preparation verifies the
token's App slug matches configuration; release validation, merging and publication
accept only that App's bot. Missing credentials fail preparation; there is no
fallback to `GITHUB_TOKEN` for creating PRs.

GitHub now creates approval-required PR runs when a workflow creates/updates a PR
using `GITHUB_TOKEN`. The **Allow GitHub Actions to create and approve pull requests**
setting does not remove that requirement. An App installation token is the
[documented solution](https://docs.github.com/en/actions/concepts/security/github_token).
Default workflow permissions remain read-only; individual jobs receive the
contents, pull-request and Actions permissions needed for orchestration.
Any branch rules must permit the bot's squash merge after CI. Configuring squash
merge to use the Conventional Commit PR title is recommended. These repository
settings are configured in GitHub, not in YAML files.

### Migration and rollout verification

Updating a PR with an App token does not change its author. Before the first
App-backed preparation, close the unmerged legacy `github-actions[bot]` release
PR, if one exists, and delete its `release-please--branches--main` branch. Then
run Release on `main` after deploying this configuration and installing the App.
Release-please recreates the pending release from commits with the correct author;
do not manually merge the old PR. Resolve any already-merged legacy pending release
with the old publisher before migration; the new publisher rejects its author.

In this repository, legacy PR #15 was merged and `v1.2.0` published before the
App migration. It does not need recreation; the next release uses a new PR.
The App `m407-jev-grep-release` is installed and its repository variables and
secret are configured. The local key is stored in the system keychain via fnox's
`release` profile (`RELEASE_APP_PRIVATE_KEY`); the downloaded PEM was deleted.
Use `fnox --profile release --no-defaults exec -- <command>` for commands that
need this key without loading the Zen credentials.

Verify the new PR author, an automatically started `pull_request` Release run,
the successful Bot PR auto-merge run, and the subsequent publication with all
three archives and `SHA256SUMS`. Local tests alone do not verify App installation
or this live end-to-end flow. If CI succeeds but its merge follow-up is absent,
rerun the native PR workflow and inspect the follow-up before claiming completion.

## Read-only build verification

To validate a selected branch, including the migration PR #5 branch, replace
`<branch>` with its name and run:

```sh
gh workflow run release.yml --ref <branch> -f verify_builds=true
```

Leave `release_pr` and `release_sha` empty. This mode runs `check` and the
Linux/macOS/Windows build matrix against the exact context SHA and version,
then uploads the archives as workflow artifacts. It skips `commitlint`,
`prepare`, `release-check`, and `publish`: it creates no PR, performs no merge,
and publishes no release, even when the selected branch is `main`.

## Retrying publication

Release runs are serialized per ref. To retry, manually run Release on `main`
with `verify_builds` false and the release-PR inputs left empty. Preparation
resumes the oldest merged pending release before creating another release PR.
Builds remain tied to that merge SHA even when `main` advances.

A retry replaces the entire draft asset set, including `SHA256SUMS`, so assets
from separate build attempts are not mixed. An already published release is
verified before repairing its PR labels. Existing tags pointing to another
commit cause failure; the automation never moves such a tag. Partial uploads
remain in a draft rather than becoming a public, incomplete release.
