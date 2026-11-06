# Zen deployment and local credentials

**English** | [Русский](ru/deployment.md) · [Documentation](README.md)

`jev-grep` is a client for OpenCode Zen; fnox supplies its API key at process
launch. The application defaults to the Zen System One endpoint
`https://opencode.ai/zen/v1/systemone` and model `jev-1.13`. The model is sent
explicitly. `fnox` is a launcher, not a runtime dependency of the binary.

## Requirements

- Install the `jev-grep` binary using the [installation instructions](installation.md).
- Install fnox and make it available on `PATH` for credential-backed runs.
- On Linux, provide an available, unlocked Secret Service (commonly through a
  desktop keyring such as GNOME Keyring, or another Secret Service-compatible
  daemon). Headless/SSH sessions may not have an unlocked service; unlock or
  arrange access locally before running fnox. There is deliberately no
  plaintext fallback.

The project `fnox.toml` refers to a keychain entry; it contains no API key.
`root = true` makes this project configuration the fnox root instead of
searching parent directories. It does not disable fnox's global configuration.

## TLS certificates and system trust

HTTPS verification uses the union of the operating system's trusted roots and
the binary's built-in WebPKI roots. System roots add trust; they do not replace
the built-in set, and installing a private CA does not bypass certificate-chain,
hostname, or validity-period checks. The CA trust store is separate from the
system keychain entry used by fnox for `JEV_GREP_API_KEY`: installing a CA does
not configure an API key, and storing the API key does not trust a server CA.

The certificate loader recognizes `SSL_CERT_FILE` and `SSL_CERT_DIR` when they
are set; these override its system-certificate source. The built-in WebPKI roots
remain available. Unset both variables when checking the actual OS trust store.
Trust is read when the HTTP client is created, so install or remove a CA before
starting `jev-grep`, and launch a new process after each trust-store change.

### Verify a temporary CA in a disposable environment

Run this check only in disposable environments, never on a work machine. The
three required release targets are **Linux GNU** (`x86_64-unknown-linux-gnu`),
**macOS ARM64** (`aarch64-apple-darwin`), and **Windows MSVC**
(`x86_64-pc-windows-msvc`), matching the [release build matrix](../.github/workflows/release.yml).
Install the matching `jev-grep` binary, Python 3, and OpenSSL. Python's standard
library provides the HTTPS fixture; Python, OpenSSL, and the CLI must run in the
same disposable environment. On Linux use Bash/coreutils; on macOS use zsh or
Bash and the built-in `security` utility; on Windows use 64-bit PowerShell 7
and OpenSSL (for example, from Git for Windows) plus the built-in
`Import-Certificate`/certificate-store cmdlets. The OpenSSL command block below
is POSIX shell (Bash/zsh); on Windows run it in Git Bash and use PowerShell for
the store commands. No admin rights are required to run the CLI, but installing
the CA in the machine root store requires elevation.

Use this executable harness to verify native OS trust with the actual local
HTTPS fixture. It imports the certificate-generation and server helpers from
[`tests/test_cli.py`](../tests/test_cli.py), but does **not** use that test's
`SSL_CERT_FILE`/`SSL_CERT_DIR` overrides. Save the following as
`/tmp/jev-native-ca-check.py` (on Windows use `%TEMP%\jev-native-ca-check.py`),
then run `PYTHONPATH=. python3 /tmp/jev-native-ca-check.py` (Windows PowerShell:
`$env:PYTHONPATH=(Get-Location).Path; py -3 "$env:TEMP\jev-native-ca-check.py"`).
Run it from the repository root after `cargo build --locked`. To use a
target-specific/release binary instead, set `JEV_GREP_TEST_BIN` to its path
before launching the harness. It prints the exact CA certificate path and HTTPS URL needed
for the OS commands below, then waits while you install/remove that CA in the
disposable OS trust store. Keep this process running throughout all stages.

```python
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from tests.test_cli import BIN, CliTests, clean_test_env

binary = Path(os.environ.get('JEV_GREP_TEST_BIN', BIN))
if os.name == 'nt' and binary.suffix.lower() != '.exe':
    binary = binary.with_suffix('.exe')

fixture = CliTests(methodName='test_tls_trusted_ca_allows_single_and_group_evaluation')
fixture.setUp()
folder = tempfile.TemporaryDirectory(prefix='jev-native-ca-')
ca, cert, key, _, _ = fixture.make_test_certificates(folder.name)
try:
    with fixture.run_tls_server(cert, key) as server:
        url = f'https://localhost:{server.server_port}/v1/systemone'
        env = clean_test_env()
        for name in ('SSL_CERT_FILE', 'SSL_CERT_DIR'):
            env.pop(name, None)

        def check(stage, expected):
            for label, args, data, expected_requests in (
                ('single', ['criterion'], 'PKIX\n', 1),
                ('grouped', ['--format', 'json', 'criterion'],
                 'PKIX first\njoin continuation\n', 2),
            ):
                fixture.requests.clear()
                server.connection_count = 0
                result = subprocess.run(
                    [str(binary), '--api-url', url, *args], input=data, text=True,
                    capture_output=True, timeout=10, env=env)
                assert result.returncode == expected, (
                    stage, label, result.returncode, result.stderr)
                if expected == 0:
                    assert len(fixture.requests) == expected_requests, (
                        stage, label, len(fixture.requests))
                    assert server.connection_count == expected_requests
                else:
                    assert not fixture.requests, (stage, label, fixture.requests)
                    assert server.connection_count == 1, (
                        stage, label, server.connection_count)
                    assert not result.stdout, (stage, label, result.stdout)
                print(f'{stage}: {label}: exit={result.returncode}, '
                      f'connections={server.connection_count}, '
                      f'HTTP requests={len(fixture.requests)}')

        print(f'Temporary CA certificate (install this file): {Path(ca).resolve()}')
        print(f'Local fixture URL: {url}')
        check('before installation', 2)
        input('Install this CA in the disposable OS store; then press Enter: ')
        check('after installation', 0)
        input('Remove this exact CA from the OS store; then press Enter: ')
        check('after removal', 2)
finally:
    folder.cleanup()
    fixture.doCleanups()
```

The **single** command sends `PKIX\n`; the **grouped** command sends
`PKIX first\njoin continuation\n`. At each stage the harness starts a fresh
CLI subprocess for each path, with both CA override variables absent. It asserts
exit 2, one TLS connection, zero HTTP requests, and empty stdout before/after
installation; and exit 0, valid fixture response processing, one request for
single, and two for grouped after installation. A failed assertion aborts the
harness. The fixture uses a disposable loopback port and only synthetic input;
no Zen endpoint or credentials are involved.

Install only the CA file printed by the harness in the disposable environment,
then remove exactly that certificate by its fingerprint/thumbprint and update
the Linux bundle if applicable:

| Environment | Install CA | Remove exactly this CA |
| --- | --- | --- |
| Linux GNU (Debian/Ubuntu) | `sudo install -m 0644 '<printed-CA-path>' /usr/local/share/ca-certificates/jev-grep-test.crt && sudo update-ca-certificates` | `sudo rm /usr/local/share/ca-certificates/jev-grep-test.crt && sudo update-ca-certificates` |
| macOS ARM64 | `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain '<printed-CA-path>'` | `sudo security delete-certificate -Z "$(openssl x509 -in '<printed-CA-path>' -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)" /Library/Keychains/System.keychain` |
| Windows MSVC (elevated PowerShell 7) | `Import-Certificate -FilePath '<printed-CA-path>' -CertStoreLocation Cert:\LocalMachine\Root` | `$ca = Get-PfxCertificate -FilePath '<printed-CA-path>'; Get-ChildItem Cert:\LocalMachine\Root | Where-Object Thumbprint -eq $ca.Thumbprint | Remove-Item` |

Launch new CLI processes after each store change. Confirm the CA is absent from
the isolated store before deleting temporary keys/certificates and destroying
the environment. Never remove or alter any other certificate. See the
[installation guide](installation.md) and the [Russian deployment guide](ru/deployment.md).

## Store the key

From the repository root, run the following command without providing a value:

```sh
fnox set JEV_GREP_API_KEY --provider zen-keychain
```

fnox prompts interactively with hidden input. Do not pass the key as a command
argument, put it in shell history, or commit it. The secret is stored by the
system keychain provider under the `jev-grep` service. Git ignores
`fnox.local.toml` for any local fnox overrides.

Run the client through fnox so the secret is resolved before the child starts:

```sh
fnox exec -- jev-grep --json 'Certificate validation failure' sample.log
```

If the keychain is unavailable or the entry is missing, fnox must stop with an
error before starting `jev-grep`. Do not replace this with an empty key or a
plaintext file. A direct invocation without fnox remains possible for
explicitly configured mock/custom endpoints; it does not provide the normal
Zen credential setup.

## Manual overrides

The client accepts flags and the corresponding environment variables. Flags
take precedence over environment values, which take precedence over defaults.
Use the new `JEV_GREP_*` names only:

```sh
JEV_GREP_API_URL='https://opencode.ai/zen/v1/systemone' \
JEV_GREP_MODEL='jev-1.13-free' \
fnox exec -- jev-grep 'Certificate validation failure' sample.log
```

`--api-url`, `--model`, and `--api-key` can override the matching environment
settings. Prefer fnox for the API key; avoid putting key values in shell
history or process arguments. A manually selected endpoint must be the complete
System One endpoint. There is no automatic endpoint/model fallback.

## Data, cost, and context

`--timeout` is a shared 30-second-by-default budget for each model evaluation,
including retries, response-body reads, and ordinary backoff. Local TPM waits and
fixed 60-second HTTP 429 cooldowns are excluded. HTTP 429 ignores provider headers.
Configure `--tpm` or `JEV_GREP_TPM` for a process-local sliding-window budget;
see [estimation and limits](reference.md#local-tpm-budget-and-http-429).
Transient HTTP/network
failures receive at most four total attempts with exponential backoff, jitter,
and (except for 429) `Retry-After`; authentication, TLS validation, malformed responses, and
oversized responses are not retried. A retry after a transport failure may incur
a second inference charge; exactly-once inference is not guaranteed.

Requests send input text to the selected endpoint. Use only data you are
authorized to disclose; synthetic examples are preferable for testing. Zen
model availability and pricing can change; check the [current Zen Jev
documentation](https://opencode.ai/docs/zen/#jev) before deployment rather
than relying on a copied price. Checked on 2026-09-25, that page listed
`jev-1.13` at $0.042 per million input tokens with output tokens free; cached
pricing was not listed. Prices and availability can change. The selected model
is `jev-1.13`; the free
`jev-1.13-free` ID is available only when explicitly selected as above.

As documented for Jev models on 2026-09-25, the direct TypeSafe API describes
64k context for state and all questions together, and 32k for state plus the
longest question at once ([model documentation](https://docs.typesafe.ai/models)).
These are upstream model/API descriptions, not verified Zen gateway limits.
They do not imply a token count from input bytes. The client's fragment limits
are byte-based and are separate from any model token context. Measure with
authorized synthetic inputs; a long input alone does not prove the context
limit.

Future evaluation methodology is deferred to the [project backlog](BACKLOG.md). The synthetic seed is not a Jev quality result.

## Minimal live integration smoke

Live smoke performed on **2026-09-28** with the release binary built by
`cargo build --locked --release`. Requested model: `jev-1.13`; requested
endpoint: `https://opencode.ai/zen/v1/systemone`. Both invocations used
`fnox exec` with the configured keychain entry; no credential value was printed
or stored. Inputs were synthetic and stored temporarily under
`/tmp/opencode/jev-smoke-20260928`.

Reproduce from the repository root after configuring fnox as above:

```sh
cargo build --locked --release
mkdir -p /tmp/opencode/jev-smoke-20260928
printf '%s\n' '{"text":"fn validate_certificate() { true }","file":"synthetic.rs","language":"Rust","range":{"byteOffset":{"start":0,"end":34},"start":{"line":0,"column":0},"end":{"line":0,"column":34}}}' > /tmp/opencode/jev-smoke-20260928/ast.jsonl
printf '%s\n' 'certificate validation failed for service alpha' 'certificate validation succeeded for service beta' > /tmp/opencode/jev-smoke-20260928/log.txt
fnox exec -- target/release/jev-grep --input-format ast-grep --json \
  'Find certificate validation code that may explain a validation failure' \
  /tmp/opencode/jev-smoke-20260928/ast.jsonl
fnox exec -- target/release/jev-grep --json \
  'Find log records relevant to a certificate validation failure' \
  /tmp/opencode/jev-smoke-20260928/log.txt
```

Outcomes: the valid single synthetic AST candidate completed with exit **1**
(no selected fragment), no stderr/API/input error. The two-line text input
completed with exit **0** and emitted the selected first record; without `-q`
or `-m`, processing continued through the second record and the grouped
assessment path before EOF. Neither outcome establishes model quality. This is
protocol/integration evidence only; no benchmark, quality claim, latency or
usage statistics were collected. The run was performed once per scenario.
