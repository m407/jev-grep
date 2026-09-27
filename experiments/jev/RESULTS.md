# Jev on Zen: live experiment results

The completed run's safe raw evidence is checked in at
[`evidence/full-run-4/`](evidence/full-run-4/). The run metadata names each
case's stdout/stderr file; this repository copy makes the report independently
readable without relying on a machine-local `/tmp` path. The original external
output directory remains unchanged.

## Completed live run — 2026-09-27

The approved release binary (`jev-grep 0.1.0`) and standard experiment runner
completed the full synthetic corpus via `fnox` and the current proxy. The runner
used its truthful application User-Agent `jev-grep-experiment/0.1.0`; no
transport, request-body, prompt, threshold, or retry behavior was substituted.
The run started at `2026-09-27T18:40:28Z` and completed at
`2026-09-27T18:40:46Z`, using endpoint
`https://opencode.ai/zen/v1/systemone` and requested model `jev-1.13`.

Dataset SHA-256:
`6a5e979a60615594a03fcc223030b0129fc038059606c4fc6239c8af669bc26f`.
The original output directory was
`/tmp/opencode/jev-grep-migrate-to-jev-zen-full-run-4/`. Its directory and files
were restricted to the current user. The safe evidence copy in
`evidence/full-run-4/` contains only synthetic inputs' outputs and metadata;
the directory is mode 0700 and files are mode 0600 in this working tree. Neither
copy contains Authorization headers or credentials.

| Case | Expected ranges | Selected ranges | Exit | Wall ms | Client requests* | Match |
|---|---|---|---:|---:|---:|---|
| `gradle-stacktrace` | 1–7 | 1–8 | 0 | 4344.086 | 8 | No — extra line 8 joined |
| `retry-recovery` | 1–2 | 1–2 | 0 | 1276.397 | 2 | Yes |
| `useful-neighbor` | 1–2 | 1–2 | 0 | 1219.170 | 2 | Yes |
| `independent-errors` | 1–2, 3–3 | 1–2, 3–3 | 0 | 1737.537 | 3 | Yes |
| `npm` | 1–4 | 1–4 | 0 | 2151.828 | 4 | Yes |
| `buildah` | 1–4 | 1–4 | 0 | 2116.942 | 4 | Yes |
| `byte-edge` | 1–1 | 1–1 | 0 | 750.310 | 1 | Yes |
| `long-input` | 1–1 | none | 1 | 756.696 | 1 | No — expected line missed |
| `ast-rust-candidate` | 1–3 | 1–3 | 0 | 739.066 | 1 | Yes |
| `russian-mixed` | 1–2 | 1–2 | 0 | 1214.648 | 2 | Yes |

Eight of ten labels matched. The two observed quality discrepancies are an
extra joined line in `gradle-stacktrace` and a missed expected result in
`long-input`. Both invocations completed normally (exit 0/1); these are model
quality observations, not infrastructure failures. The long-input outcome is
not evidence of a context limit.

*`client requests` are flow-derived from the successful client control flow,
not server-instrumented: 28 across the corpus. The three direct probes were
separate and actually completed with HTTP 200; their observed model and usage
were:

| Probe | Returned model | Input tokens | Output tokens |
|---|---|---:|---:|
| single string-state | `jev-1.13` | 281 | 20 |
| grouped object-state/four questions | `jev-1.13` | 359 | 73 |
| long-input telemetry | `jev-1.13` | 1688 | 20 |

Probe usage is not attributed to corpus calls. The client itself does not
expose per-corpus model metadata or token usage. Raw stdout/stderr for every
case can be matched by filename to `run-metadata.json`; all configured
thresholds and byte limits are recorded there. The completed run proves the
runner and client performed the full real inference corpus; it does not claim
zero model-quality errors or that the experiment is representative of general
accuracy. This evidence completes the live-run work for task 3.1. The earlier
blocked attempts below are historical and superseded by this completed run;
their descriptions of task 3.1 as incomplete refer to the state at those dates
only.

## Earlier full-run blockers (historical)

Before the approved truthful application User-Agent update, full runner
attempts stopped at the first probe with HTTP 403. They did not proceed to the
remaining probes or corpus and are retained below as historical diagnostics.

No headers, response body, or credentials were recorded. This is a blocker for
this runner request, not evidence of a bad key or a model-quality result. The
equivalent curl replay with the exact first-probe JSON and Python urllib's
observable non-secret headers also received HTTP 403, recorded at
`/tmp/opencode/jev-grep-runner-equivalent-curl-1/evidence.json`; therefore the
observed difference is not explained solely by selecting Python urllib instead
of curl. That replay differed from the earlier successful curl in both request
content and `Accept` handling, so the cause remains unresolved. No more network
requests were made. A later, separately approved one-variable diagnostic could
hold the exact runner payload and all other curl settings fixed while changing
only `Accept` from suppressed to `application/json`; it would test that header
difference, not establish origin attribution. Task 3.1 remains incomplete.

## Earlier full-run attempt: blocked before corpus

On 2026-09-25, after the user authorized resuming the full task, `cargo build
--locked --release` succeeded and `target/release/jev-grep` was run through the
documented `fnox exec` runner with a fresh output directory. The runner made
its first of three required direct probes (the one-question string-state
probe) to `https://opencode.ai/zen/v1/systemone`, requesting `jev-1.13`, and
received HTTP 403. It stopped immediately; the remaining probes and all ten
corpus cases were not run. fnox did start the runner. This records a 403 for
this probe only and does not establish its cause or prove the key invalid.

Sanitized blocker evidence is
`/tmp/opencode/jev-grep-migrate-to-jev-zen-full-run-2/blocker.json`. No response
body, headers, or credentials were recorded. Settings were the requested
defaults (`jev-1.13`, threshold `0.5`, join threshold `0.5`, fragment limit
16384 bytes except `byte-edge` at 128, line limit 65536). The dataset SHA-256
is `6a5e979a60615594a03fcc223030b0129fc038059606c4fc6239c8af669bc26f`; its
ten cases are `gradle-stacktrace`, `retry-recovery`, `useful-neighbor`,
`independent-errors`, `npm`, `buildah`, `byte-edge`, `long-input`,
`ast-rust-candidate`, and `russian-mixed`. Since the probe gate failed, there
are no complete-run model metadata, per-case outputs/ranges, quality errors,
latencies, direct-probe usage, or flow-derived client request counts. No
corpus-quality conclusion is possible. The long input has not established a
context limit. Task 3.1 remains incomplete pending a successful full run.

## Blocked during first live probe

The release build had already succeeded (`cargo build --locked --release`;
client version `jev-grep 0.1.0`). On 2026-09-25 the required fnox launch was
retried after the user confirmed the key was set, using the fresh output path
`/tmp/opencode/jev-grep-migrate-to-jev-zen-run-2`. fnox resolved the key and
started the runner, but the first authorized Zen probe returned HTTP 403. The
runner stopped before corpus invocations. This establishes an endpoint access
or authorization denial for this request; it does not establish its cause or
whether any credential is valid for other services. No corpus case ran, no
model metadata or response was obtained, and there are no quality, latency, or
usage measurements. No model-quality conclusion can be drawn.

Sanitized evidence is at
`/tmp/opencode/jev-grep-migrate-to-jev-zen-run-2/blocker.json` (outside the
repository). It records only `status: blocked` and `probe returned HTTP 403`;
no credentials, request headers, or response body were recorded. The earlier
first attempt's absent keychain entry is documented in the prior run's safe
blocker at `/tmp/opencode/jev-grep-migrate-to-jev-zen-run/blocker.json`; its
stale output path was not overwritten. The new runner directory contains no
run metadata or raw case outputs because it stopped at the probe.

The corpus SHA-256 and empirical fields required for a complete results
report remain uncollected. Run parameters were the methodology defaults:
endpoint `https://opencode.ai/zen/v1/systemone`, requested model `jev-1.13`,
threshold `0.5`, join threshold `0.5`, default fragment limit 16384 bytes
(128 for `byte-edge`), and line limit 65536 bytes. These are configured
parameters, not live-confirmed model metadata. Resolve the 403 through the
account/endpoint access setup, then repeat the experiment only when there is a
specific access change and with another fresh output directory. Do not pass or
publish the key. Task 3.1 remains incomplete until all probes and every corpus
case complete against real Zen and safe evidence supports the full report.
The long-input case is not a context-limit test.

## One authorized proxy diagnostic (not a corpus run)

After the user confirmed the configured HTTPS proxy was intentional and
authorized exactly one minimal diagnostic inference request, one POST was
made through fnox using the configured endpoint/model above. Redirects and
retries were disabled; no corpus cases or additional requests were run. The
HTTPS request completed far enough to receive an HTTP-layer 403 rather than a
CONNECT-tunnel failure. Safe allowlisted evidence is at
`/tmp/opencode/jev-grep-migrate-to-jev-zen-proxy-diagnostic-1/evidence.json`;
it records endpoint, requested model, timestamp, request type, HTTP status and
classification only. No response body, headers, key, or proxy address was
recorded. This distinguishes the attempt from a rejected CONNECT tunnel, but
does not establish whether the HTTP 403 was generated by Zen's gateway/origin
or by an intermediary that can return HTTP responses. No model metadata or
corpus measurements were obtained. Task 3.1 remains incomplete; no further
inference request is authorized by this diagnostic approval.

## One credential-check probe after key update

On 2026-09-25, after the user reported updating the key and asked to check,
exactly one fresh minimal authenticated POST was sent through fnox to the same
endpoint and model. The response was again HTTP 403 after the HTTPS proxy
tunnel was established; redirects and retries were disabled. Safe evidence is
at `/tmp/opencode/jev-grep-migrate-to-jev-zen-credential-check-2/evidence.json`.
It contains only the allowlisted request metadata and HTTP status; no response
body, headers, key, or proxy address. This does not prove the updated key is
invalid: the response source and reason remain undetermined. No corpus request
or further inference followed. Task 3.1 remains incomplete.
