# ast-grep and agrind pipelines

[Main skill](../SKILL.md) · [CLI and contracts](cli.md)

All examples use Bash, installed tools, existing inputs, and an authorized,
already configured backend. Replace sample paths with the authorized inputs.
Enable `set -o pipefail`; do not hide upstream failures. An example alone does
not authorize external data transmission.

## ast-grep → semantic code selection

### Rust functions

This example scans a directory named `src`; substitute the authorized source
directory or narrow the scope to a specific file:

```bash
set -o pipefail

ast-grep scan --json=stream --inline-rules '
id: rust-functions
language: Rust
severity: hint
rule:
  kind: function_item
' ./src |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Function that validates input or rejects malformed data' -
```

### Python functions with the filename preserved

```bash
set -o pipefail

ast-grep scan --json=stream --inline-rules '
id: python-functions
language: Python
severity: hint
rule:
  kind: function_definition
' service.py |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Function that retries a failed network request' -
```

`kind` names depend on the language grammar. For methods/declarations, choose
an appropriate AST kind or custom rule; do not reuse Rust `function_item` for
another language without checking. First inspect a small sample of producer
output and the candidate count/size.

### Custom rules and saved candidates

```bash
set -o pipefail

ast-grep scan --json=stream --rule rule.yml ./src |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Code that handles authentication failures' -
```

The rule must extract appropriate nodes; use `severity: hint` for candidate
search. Saving output is a separate local action: choose a new path and do not
overwrite an existing file without checking and obtaining permission.

```sh
ast-grep scan --json=stream --rule rule.yml ./src > candidates.jsonl
jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
  'Code that handles authentication failures' candidates.jsonl
```

Repeating a semantic query on saved candidates **calls the model again**:
this is not a result cache. If the source changed, recreate the candidates
before drawing conclusions about current code. Saved `file/range` alone does
not confirm freshness.

### Important limitations

- **`scan --json=stream`** is required: one object per line. `--json`/`--json=compact`
  produce arrays, and `outline` uses a different schema; the CLI rejects them.
- `severity: hint` avoids an error-level status from the matched nodes themselves;
  still check for real producer errors.
- Do not rely on the `sg` alias: it may not be ast-grep. Use a verified
  `ast-grep` binary and its `--help`.
- Only `text` is evaluated, not `lines`, labels, or neighboring functions.
  Candidates are independent; nested, overlapping, and duplicate nodes are
  neither removed nor grouped. `--join-threshold` has no effect. The producer
  rule determines candidate coverage.
- `262144` is an example physical JSONL-line limit: the producer may repeat the
  body in `text`, `lines`, and labels. **The body limit remains 16384 bytes**
  unless `--max-fragment-bytes` is changed separately. Do not increase it without
  assessing data volume and context constraints.
- Pass code to ast-grep by **path** to preserve the source filename. `--stdin`
  yields a stdin source, even when the code came from a named file.
- Filter actual source files in ast-grep. Consumer options `-g/--hidden/--no-ignore`
  filter JSONL transport files, not the `file` inside a candidate.
- Results preserve producer byte boundaries and convert line numbers to
  1-based inclusive coordinates. The source file is not reread; validation checks
  consistency of the supplied text and range, not agreement with the current
  file on disk.

When searching changed files, handle an empty list: ast-grep without paths may
start searching its default directory. Use NUL-delimited Git paths and a Bash
array; check the Git command's exit status separately. Define the comparison
baseline explicitly and scan only selected files that still exist. Scanning
whole changed files selects candidates in their current versions, not just
changed hunks; removed behavior and untracked files require separate coverage.

## agrind → prepared log records

Angle Grinder runs as `agrind`. Use it to extract/format fields, not to replace
semantic grouping in jev-grep. If the log already works as original text,
direct `text` input is simpler and preserves bytes and original coordinates.

### JSON logs: message → text

For example, `app.jsonl` contains:

```jsonl
{"level":"error","message":"Connection failed: PKIX path building failed"}
{"level":"info","message":"Connection established after retry"}
```

```bash
set -o pipefail

agrind --output json '* | json | message as text | fields text' < app.jsonl |
  jev-grep --input-format log-jsonl --format json \
    'Certificate validation failure' -
```

Each `message` must be a string. If the level matters to the criterion, include
it in the **evaluated text**; the consumer ignores a separate `level` field:

```bash
set -o pipefail

agrind --output json \
  '* | json | concat(level, ": ", message) as text | fields text' < app.jsonl |
  jev-grep --input-format log-jsonl --format json \
    'Unresolved application error' -
```

Both `level` and `message` must be strings for this recipe. Include additional
fields (`task`, `service`, etc.) in `text` only if the criterion needs them;
do not automatically add secret metadata from the original object.

### Wrap plain-text lines

```bash
set -o pipefail

agrind --output json '* | parse "*" as text noconvert' < build.log |
  jev-grep --input-format log-jsonl --format json \
    'Dependency resolution failure' -
```

`noconvert` keeps numeric-looking captures as strings. However, `parse` removes
leading and trailing whitespace, including stack-trace indentation. To preserve
the log exactly, use `jev-grep ... build.log` directly, without wrapping.

### Context loss and location

- `json` drops invalid JSON lines; `parse` without `nodrop` drops unmatched
  lines. jev-grep cannot detect these losses. Validate inputs first if needed.
- Do not filter only `error` when the criterion depends on recovery,
  qualifications, or stack-trace continuations. A narrow upstream filter may
  create a false picture of an unresolved error. The model assembles fragments
  only from records actually received.
- Each output physical record must be a JSON object with string-valued `text`.
  Escaped `\n` is allowed; arrays, blank lines, and non-string `text` are errors.
- Input log-JSONL `source/range` fields are **ignored**. Result ranges refer to
  the JSONL records read by the consumer, not the original `app.jsonl` or bytes
  of decoded `text`.

To open a result by path/range, save the **prepared** stream to a new file,
then search it:

```sh
agrind --output json '* | json | message as text | fields text' < app.jsonl > prepared.jsonl
jev-grep --input-format log-jsonl --format json \
  'Certificate validation failure' prepared.jsonl
```

The result points to `prepared.jsonl`; the original `app.jsonl` line cannot be
reconstructed automatically from the coordinates, especially if records were
dropped.

## Compatibility checks and evidence limits

These recipes are documented for ast-grep **0.45.3** and agrind **0.19.6**.
These are not strict version requirements or evidence that the current command
was executed. Check the installed producer independently: small input → JSONL
framing → string-valued `text` → native AST ranges. Do not use a mock/jq
substitute as proof that real agrind works.

An authorized local mock System One server can check the consumer without
external data transmission. Do not assume a mock server or fixtures are already
available. Checking a real producer through such a mock proves the
wire/input/output contract, **not semantic model quality**. Without installed
agrind, state that its runtime integration was not checked and retain the
documented recipe; do not install it automatically.

## Sources

- [ast-grep: CLI](https://ast-grep.github.io/guide/command-line.html)
- [Angle Grinder: upstream and operators](https://github.com/rcoh/angle-grinder)
