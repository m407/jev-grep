# CLI, configuration, and contracts

This reference supplements the [main skill](../SKILL.md). The commands below
are recipes, not permission to install tools, change credentials, or send data
to an external API. Examples assume existing files and an already configured,
authorized backend.

## Check installed tools

```sh
command -v jev-grep
jev-grep --version
jev-grep --help
```

If the binary is missing, ask the user to install it; do not install or build
automatically. Do not assume source files or a local build directory are
available. Check the installed binary's version and help before using these
recipes. `ast-grep` and `agrind` are separate optional tools; check them only
when the selected workflow needs them. Python/PyTorch are not needed to run
the CLI binary.

## Backend and API key

| Setting | Environment variable | Default |
| --- | --- | --- |
| Full System One endpoint | `JEV_GREP_API_URL` | `https://opencode.ai/zen/v1/systemone` |
| Model | `JEV_GREP_MODEL` | `jev-1.13` |
| Bearer token | `JEV_GREP_API_KEY` | No built-in key |

Flags `--api-url`, `--model`, and `--api-key` take precedence over environment
variables, then defaults. Prefer an already configured environment variable
for the key, not `--api-key`. The endpoint must implement **System One**: this
is not a chat-completions API. There is no automatic provider, endpoint, or
model switching. Legacy `LG_*` environment variables are not used.

Credential provisioning is outside this skill. Use the user's existing secure
setup to supply `JEV_GREP_API_KEY` to the CLI process. If it is missing, ask the
user to configure it outside the agent conversation. Do not read or print the
key, request it in chat, put it in process arguments, or save it to a file.
Do not change credential storage or create a plaintext fallback.

## Standalone usage

The CLI has no subcommands:

```text
jev-grep [OPTIONS] <QUERY> [PATHS]...
```

```sh
# Original file; human is the default output format.
jev-grep 'Certificate validation or trust chain failure' application.log

# Limited scope, a higher threshold, up to three selected fragments per file.
jev-grep -g '*.log' --threshold 0.8 -m 3 --format json \
  'Database connection failure' ./logs

# Already prepared records.
jev-grep --input-format log-jsonl --format json \
  'Connection failure that was not resolved' prepared.jsonl
```

Explicit stdin and process output (Bash):

```bash
set -o pipefail
printf '%s\n' 'certificate validation failed for service alpha' |
  jev-grep --format json 'Certificate validation failure' -

# Running the build requires separate authorization from analyzing a saved log.
npm run build 2>&1 |
  jev-grep --format json 'Compilation or dependency error' -
```

Without paths, the CLI reads redirected stdin; otherwise, it searches `.`.
Explicit paths do not automatically add stdin; `-` may appear at most once.
Adjacent records from different input files are not grouped together.

Traversal respects `.ignore` and Git ignore rules; hidden files are excluded
by default, and symlinks inside directories are not followed. `--hidden` and
`--no-ignore` widen the scope, so use them deliberately. An inclusive `-g` does
not override these rules. Binary data, invalid UTF-8, and NUL cause errors;
restrict inputs to the relevant text files.

## Thresholds, limits, and short output modes

| Option | Meaning |
| --- | --- |
| `--threshold P` | Select a completed fragment with score ≥ P; default `0.5` |
| `--join-threshold P` | Group log records when `same_event` **or** `useful_context` ≥ P; default `0.5` |
| `--max-line-bytes N` | Physical input line including its line ending; default `16384` |
| `--max-fragment-bytes N` | Evaluated fragment text in UTF-8 bytes; default `16384` |
| `--timeout SECONDS` | One evaluation including retries, reading, and waits; default `30` |
| `-m N`, `--max-count N` | Up to N selected fragments per source |
| `-v`, `--invert-match` | Select scores below the threshold; grouping is unchanged |
| `-c`, `--count` | `source:count`, including zero |
| `-l`, `--files-with-matches` | Sources with selected fragments |
| `-q`, `--quiet` | No output; exit after the first completed selected fragment |

```sh
jev-grep -c 'Build failure' build.log
jev-grep -l -g '*.log' 'Database connection failure' ./logs
jev-grep -q 'Build failure' build.log
jev-grep -v 'Routine successful operation' application.log
```

`-c/-l/-q` are mutually exclusive; do not add `--format` (even `human`) or
`--json`. `--json` is an alias for fragment JSONL, but cannot be combined with
`--format`. Thresholds must be finite and in [0,1]; sizes, timeout, and `-m`
must be positive. `-n` and `-H` are accepted for compatibility, but location
is always printed anyway. `--no-filename` is an error.

In AST mode, the source is upstream `file`, even across multiple transport
files or interleaved candidates. `-c` prints totals after EOF; files without
candidates are unknown. `-l` prints each path once, but does not stop evaluating
later candidates. After reaching `-m`, further candidates for that source are
validated but not sent to the model; other sources continue to be processed.

## Grouping and latency

The first log record receives `match`; the next record triggers one HTTP
request with four questions: `same_event`, `useful_context`, `match_next`, and
`match_extended`. On joining, the score becomes `match_extended`. A successful
retry can therefore exclude a preceding error. In AST mode, there is only one
`match` per candidate and no grouping; `--join-threshold` has no effect.

A log fragment finishes at a semantic boundary, EOF, or a forced size split.
**A stream pause does not finish a fragment**; timeout is not a stdin-idle
timeout. An AST candidate can be emitted before the entire stream reaches EOF.
For `tail -f`, line numbers refer to the received stream, not the full source
file.

Log fragments are size-limited by splitting **between** records. An indivisible
line, decoded `text`, or AST candidate exceeding the limit causes an error.
The physical-line limit is checked before JSON decoding. A byte limit does not
guarantee the model's token context; the client does not set `max_len` or prove
the absence of server-side truncation.

Requests are sequential, without batching or caching. A complete successful
pass typically requires one HTTP request per log record or evaluated AST
candidate. Four questions do not mean four HTTP requests. Transient failures
receive at most four total attempts within the shared deadline; retries after
transport failures may incur another charge. Do not present copied, outdated
prices or direct TypeSafe context limits as guarantees for the selected gateway.

## Input contracts

### log-jsonl

One JSON object per physical line, with a required **string-valued** `text`:

```jsonl
{"text":"Connection failed: PKIX path building failed\nstack trace continuation"}
{"text":"Connection established after retry"}
```

Only `text` is used. Fields such as `level`, `source`, and `range` do not become
model context or replace coordinates; include relevant values in `text` itself.
Arrays, blank physical lines, missing/non-string `text`, NUL, and invalid UTF-8
are rejected. Escaped newlines are allowed; adjacent records still undergo
grouping.

### ast-grep

One native object per line; a minimal, consistent record looks like this:

```json
{"text":"fn f() {}","file":"src/example.rs","language":"Rust","range":{"byteOffset":{"start":0,"end":9},"start":{"line":0,"column":0},"end":{"line":0,"column":9}}}
```

`file` is a nonempty string; `text` is a string; all ranges are required,
nonnegative, and must agree with the text. The byte span equals the UTF-8 length
of `text`; input lines/columns are zero-based, and columns are checked using
Unicode characters, not UTF-8 bytes. `language` is optional but must be a string
if present. Use real producer records, not approximately calculated coordinates.
Literal `STDIN` denotes stdin as the source. The source is not reread, and the
freshness of a saved candidate is not checked.

## Result JSONL and location

`--format json` emits one object per selected fragment, for example (the score
is illustrative):

```json
{"kind":"log","source":{"kind":"file","path":"build.log"},"range":{"start_line":1,"end_line":1,"start_byte":0,"end_byte":13},"text":"PKIX failure\n","text_basis":"source","probability":0.93,"split_before":false,"split_after":false}
```

- `kind`: `log` or `code`; code may include `language`.
- `source`: a file with `path`, or `{"kind":"stdin","path":null}`.
- `range`: **1-based inclusive lines**, **0-based UTF-8 bytes [start,end)**.
  ast-grep lines are converted from zero-based numbering; byte ranges are preserved.
- `text_basis: source`: original text, including CRLF and an absent final LF;
  for code, this is the supplied ast-grep `text`.
- `text_basis: decoded-records`: decoded `text` fields joined by one additional
  LF; the range describes physical JSONL records, not bytes of this decoded text.
- `probability`: the criterion score, not the relation score between records.
- `split_before/split_after`: forced size boundaries; always false for code.

Human output adds a final LF for readability when needed; JSON preserves `text`.
The old `path/line_number/text/probability` contract has been replaced by the
fragment contract; do not use it. Result JSONL cannot automatically be fed back
as native AST input or the original log: the schema is different, and log result
`text` may already be grouped.

## Exit codes and errors

0 means a fragment was selected; 1 means none were selected; 2 means an error.
Diagnostics go to stderr. An error stops processing but does not retract earlier
stdout. An unfinished pending fragment is not emitted as successful on error.
A broken downstream pipe is treated as clean termination without a BrokenPipe
diagnostic.

Bash `pipefail` exposes upstream failures, but pipeline status does not always
equal consumer status. If needed, capture `statuses=("${PIPESTATUS[@]}")`
**immediately after the pipeline**: the next command overwrites `PIPESTATUS`.
Do not use unconditional `set -e` when no-match exit 1 is an expected outcome.
`-q` or early log-mode `-m` can cause upstream SIGPIPE.

For HTTP/TLS/auth errors, fix the cause; do not disable TLS or switch providers.
Redirects and URL credentials are rejected. Responses are limited to 64 KiB;
malformed or oversized responses do not become search results. Preserve complete
failure output without secrets.

## Version and verification checks

Use the installed CLI's `--version` and `--help` to check flag support; do not
assume these recipes apply unchanged to every release. If behavior differs,
report the version and discrepancy rather than silently changing the command.

An authorized local mock System One server can check input/output and protocol
behavior without external inference. Do not assume a test suite or fixtures
are already available. Mock scores are not model-quality measurements. Clearly
distinguish static recipe inspection, executed mock checks, and authorized live
inference; report which checks actually ran.
