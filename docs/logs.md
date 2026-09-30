# Log search

**English** | [Русский](ru/logs.md) · [Documentation](README.md)

This guide covers plain text, live streams, and prepared JSONL. For output
coordinates, options, and exit codes, see the [CLI reference](reference.md).

## Files and directories

```sh
# One file, human-readable output.
jev-grep 'Certificate validation or trust chain failure' application.log

# Recursively search only .log files with a higher selection threshold.
jev-grep -g '*.log' --threshold 0.8 'Database connection failure' ./logs

# Machine-readable output.
jev-grep --format json 'Unresolved network failure' build.log

# Up to three selected fragments per input file.
jev-grep -m 3 'Dependency resolution failure' first.log second.log
```

These examples assume credentials are already supplied; for Zen, use
`fnox exec -- jev-grep ...` as described in the [deployment guide](deployment.md).

With no paths, `jev-grep` reads redirected stdin, or searches the current
directory when invoked from a terminal. `-` explicitly selects stdin. When
paths are supplied, stdin is not added automatically. Records from different
input files are never grouped together.

Directory traversal respects `.ignore` and applicable Git ignore rules. Hidden
files are excluded by default, and symlinks inside directories are not followed.
Use `--hidden` and `--no-ignore` to widen the search. An inclusive `-g` does not
override hidden/ignore filters. Use a glob in directories containing binary
files: invalid UTF-8 and NUL bytes cause an error.

## Command output and live streams

These pipeline examples use Bash. Enable `pipefail` so a producer failure also
affects the pipeline's exit status:

```bash
set -o pipefail

./gradlew --console=plain build 2>&1 |
  jev-grep 'Build failure caused by dependencies or certificate validation'

npm run build 2>&1 |
  jev-grep --format json 'Compilation or dependency error'

buildah bud . 2>&1 |
  jev-grep 'Image build failed because a command or download failed'

tail -f application.log |
  jev-grep 'Connection failure that was not resolved by a retry'
```

`2>&1` combines the command's stdout and stderr into one input stream. Stdin
coordinates start at the beginning of the **received stream**. For example,
`tail -f` does not send the file's entire history: result line numbers are not
line numbers in the complete `application.log`. To locate a match in a saved
log, pass the path directly: `jev-grep 'Build failure' build.log`.

## How fragments are assembled

The first record receives a `match` score. For the current fragment and the
next record, `jev-grep` sends **one HTTP request with four questions**:

- `same_event`: does the record continue the same specific event?
- `useful_context`: does it substantially explain, clarify, resolve, or
  contradict the current fragment with respect to the criterion?
- `match_next`: does the new record match on its own?
- `match_extended`: does the fragment together with the new record match?

A record is joined if `same_event` **or** `useful_context` reaches
`--join-threshold` (default `0.5`). A shared topic alone is insufficient.
On joining, the score becomes `match_extended`; otherwise, the current
fragment keeps its previous score and the new record starts the next fragment.

A fragment finishes at a semantic boundary, source EOF, or a size-limit split.
It is emitted if its final score is at least `--threshold` (default `0.5`).
A successful retry message can therefore change whether an earlier error is
selected.

**A pause in a live stream does not finish a fragment.** Even matching text
waits for the next boundary or EOF; completed matches are emitted immediately
with stdout flushed. `--timeout` (default 30 seconds) limits one complete model
evaluation, including sending, reading the response, retries, and waits.
Transient HTTP/network failures receive at most four total attempts; waits
grow (200/400/800 ms) with jitter and honor `Retry-After`. Authentication, TLS,
malformed responses, and oversized responses are not retried. A retry after a
network failure may incur another charge; exactly-once inference is not
guaranteed.

## Prepared logs: JSONL and agrind

### The log-jsonl input contract

Each physical line must be a JSON object with a string-valued `text` field:

```jsonl
{"text":"Connection failed: PKIX path building failed"}
{"text":"Retrying connection"}
{"text":"Connection established"}
```

```sh
jev-grep --input-format log-jsonl --format json \
  'Connection failure that was not resolved' prepared.jsonl
```

`text` may contain escaped `\n`, representing an already multiline record.
Adjacent JSONL records still undergo model-based grouping checks.

**Only `text` is used.** Extra fields such as `level`, `task`, `source`, and
`range` are ignored: they are neither sent to the model nor used to replace
coordinates. Include any information needed for evaluation in the `text`
string. A JSON array, empty physical line, missing `text`, or non-string `text`
is an input error.

### agrind: rename the message to text

[Angle Grinder](https://github.com/rcoh/angle-grinder) is installed separately;
its executable is `agrind`. These examples were checked with version 0.19.6.

For source JSONL records such as `{"level":"error","message":"PKIX failure"}`:

```bash
set -o pipefail

agrind --output json '* | json | message as text | fields text' < app.jsonl |
  jev-grep --input-format log-jsonl --format json 'Certificate validation failure'
```

`message` must be a string in every record. If the log level matters to the
criterion and both fields are strings, include it in the evaluated text:

```bash
set -o pipefail

agrind --output json \
  '* | json | concat(level, ": ", message) as text | fields text' < app.jsonl |
  jev-grep --input-format log-jsonl 'Unresolved application error'
```

You can also wrap each plain-text line:

```bash
set -o pipefail

agrind --output json '* | parse "*" as text noconvert' < build.log |
  jev-grep --input-format log-jsonl 'Dependency resolution failure'
```

`noconvert` keeps numeric-looking strings as strings, as required by
`jev-grep`. However, `agrind parse` strips leading and trailing whitespace,
including stack-trace indentation. To preserve the original log exactly, use
`jev-grep ... build.log` without conversion.

`agrind` prepares records; **the model inside `jev-grep` assembles multiline
fragments**. Do not prefilter only `error` records if you need stack-trace
continuations or recovery messages. The agrind `json` operator drops invalid
JSON lines; `parse` without `nodrop` drops unmatched lines. `jev-grep` cannot
detect these upstream losses.

### Locating a match after transformation

For `log-jsonl`, the range refers to the **JSONL records actually read**, not
the original log. Result text consists of decoded `text` fields joined with
one additional LF between records (`text_basis: decoded-records`).

If you need a file in which to open the reported range, save the prepared
stream and search it:

```bash
agrind --output json '* | json | message as text | fields text' < app.jsonl > prepared.jsonl
jev-grep --input-format log-jsonl 'Certificate validation failure' prepared.jsonl
```

The result path is `prepared.jsonl`. Arbitrary JSON fields do not carry over
the `app.jsonl` path or its original coordinates.
