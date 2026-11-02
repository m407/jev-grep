# Verification evidence

## Task 2.2 — general acceptance and minimal live Zen smoke

**Date: 2026-11-02.** Required offline checks passed:

- `cargo fmt --check` — PASS.
- `cargo clippy --locked --all-targets -- -D warnings` — PASS.
- `cargo test --locked` — PASS, 14 tests.
- `cargo build --locked --release` — PASS.
- `python3 -m unittest discover -s tests -v` — PASS, 30 tests.
- `git diff --check` — PASS.

Cargo checks used an isolated `CARGO_HOME=/tmp/opencode/jev-cargo-home` because
the configured user cargo-registry cache failed to create a cache directory
(`File exists`); using the isolated cache allowed the required locked checks to
complete without modifying the existing cache. The build target remained the
repository's normal `target/` directory.

Minimal live smoke used the release binary, default requested model
`jev-1.13`, and endpoint `https://opencode.ai/zen/v1/systemone`. fnox confirmed
that a key was available, but its value was never displayed or stored. Both
inputs were synthetic: one AST-grep Rust candidate and a two-record log file.
Both invocations ran with `SSL_CERT_FILE` pointing to a temporary empty file and
`SSL_CERT_DIR` pointing to a temporary empty directory, which were removed after
the run. The AST invocation exited 1 with empty stderr (the expected
no-selected-candidate outcome in the existing live-smoke contract); the
two-record log invocation exited 0 with empty stderr. No TLS/API/input error
was reported, and provider response bodies/stdout were not retained or exposed.
These results establish protocol/integration success only, not model quality.

The commands below are sanitized reproductions of the commands used in this
session. They show synthetic-input construction, credential-safe invocation via
fnox, empty certificate-source overrides, and output handling. stdout/stderr are
captured only in the temporary directory and are deleted; only exit status and
stderr byte count were inspected. The two-line log is supplied as a single
input file, with neither `-q` nor `-m`, so processing continues through the
second record and uses the grouped assessment path before EOF.

```sh
work=$(mktemp -d /tmp/opencode/jev-live-XXXXXX)
trap 'rm -rf "$work"' EXIT
: > "$work/empty-ca.pem"
mkdir "$work/empty-ca-dir"
printf '%s\n' '{"text":"fn validate_certificate() { true }","file":"synthetic.rs","language":"Rust","range":{"byteOffset":{"start":0,"end":34},"start":{"line":0,"column":0},"end":{"line":0,"column":34}}}' > "$work/ast.jsonl"
printf '%s\n' \
  'certificate validation failed for service alpha' \
  'certificate validation succeeded for service beta' > "$work/log.txt"
export SSL_CERT_FILE="$work/empty-ca.pem"
export SSL_CERT_DIR="$work/empty-ca-dir"

# Capture both streams privately; report only status and stderr byte count.
if fnox exec -- target/release/jev-grep \
  --input-format ast-grep --json \
  'Find certificate validation code that may explain a validation failure' \
  "$work/ast.jsonl" >"$work/ast.out" 2>"$work/ast.err"; then
  ast_status=0
else
  ast_status=$?
fi
ast_stderr_bytes=$(wc -c < "$work/ast.err") # observed: status 1, 0 bytes

if fnox exec -- target/release/jev-grep --json \
  'Find log records relevant to a certificate validation failure' \
  "$work/log.txt" >"$work/logs.out" 2>"$work/logs.err"; then
  logs_status=0
else
  logs_status=$?
fi
logs_stderr_bytes=$(wc -c < "$work/logs.err") # observed: status 0, 0 bytes
```

No CLI stdout or provider response body was printed or saved outside the
temporary directory.

Task 2.2's acceptance checks are satisfied. This does not claim native trust
store acceptance on Linux GNU, macOS ARM64, or Windows MSVC; that separate work
remains explicitly deferred and incomplete below.

## Task 2.1 — native system trust-store acceptance

**Status: incomplete.** No trust-store installation or removal was performed. The
developer host trust store was not modified. The documentation harness could
not be run because the only available matching platform is this ordinary
developer Linux host, not a disposable environment; the documented procedure
explicitly forbids using a work machine. No temporary CA was created or
installed, so there are no before/install/after observations to report.

### Environment availability checked

- `uname -a` reported `Linux hawai 7.3.0-1-default ... x86_64 GNU/Linux`.
- `rustc -Vv` reported host `x86_64-unknown-linux-gnu` (compiler target only;
  this is not native-store acceptance evidence).
- Python 3.13.16 and `/usr/bin/openssl` are installed.
- `command -v security`, `command -v pwsh`, and
  `command -v qemu-system-aarch64` returned no executable. No macOS ARM64 or
  Windows MSVC disposable environment is exposed in this session.
- `podman` is present, but `podman info --format '{{.Host.OCIRuntime.Name}}'`
  failed with `Error: creating runtime volume path directory: mkdir
  /home/andrei/.local/share/containers/storage: file exists`; no usable
  disposable container runtime was established. `systemd-detect-virt
  --container` reported `none`.
- The documented harness was not started: `/tmp/jev-native-ca-check.py` was
  absent at inspection, and starting it from this host would still violate the
  disposable-environment requirement.

### Required observations not performed

For Linux GNU, macOS ARM64, and Windows MSVC, respectively: before installation
(`exit 2`, one TLS connection, zero HTTP requests) — **not run**; after native
store installation (single and grouped evaluations both `exit 0`) — **not
run**; after removal (`exit 2`, one TLS connection, zero HTTP requests) — **not
run**. No platform acceptance is inferred from compilation, host identity, or
the documentation harness instructions. Repeat the exact three-stage procedure
in `docs/deployment.md` / `docs/ru/deployment.md` in each disposable target
environment. Task 2.1 remains incomplete until all three are observed.
