# CLI reference

**English** | [Русский](ru/reference.md) · [Documentation](README.md)

Input guides: [logs](logs.md) · [code](code-search.md).

## Output: human and JSONL

### Human

The source and range are always printed, even without `-n` and `-H`:

```text
build.log:1-2
PKIX failure
    at Client.connect(Client.java:42)
```

For code, the header also includes the byte range, for example:

```text
src/client.rs:1-3 bytes 0-95
```

For prepared logs, the header includes `(JSONL line coordinates)` to distinguish
record numbers from lines within a decoded stack trace. Size splits are marked
with `[split-before]` and `[split-after]`.

### JSONL

`--format json` emits one JSON object per selected fragment. For example, the
first line `PKIX failure\n` in `build.log` has this shape (expanded for
readability; the probability is illustrative):

```json
{
  "kind": "log",
  "source": {"kind": "file", "path": "build.log"},
  "range": {
    "start_line": 1,
    "end_line": 1,
    "start_byte": 0,
    "end_byte": 13
  },
  "text": "PKIX failure\n",
  "text_basis": "source",
  "probability": 0.93,
  "split_before": false,
  "split_after": false
}
```

- `kind`: `log` or `code`; code may include an additional `language` field.
- `source`: a source file or `{"kind":"stdin","path":null}`.
- `range`: **1-based**, inclusive lines; **0-based** UTF-8 byte offsets with
  a **[start,end)** range. ast-grep line numbers are converted from zero-based
  numbering; byte boundaries are preserved.
- `text_basis: source`: original text, including CRLF and an absent final LF.
  For code, this is the text supplied by ast-grep.
- `text_basis: decoded-records`: joined `text` fields from log-jsonl; the byte
  range describes JSONL records, not bytes of this decoded text.
- `probability`: the stored criterion-match score, not the grouping score.
- `split_before` / `split_after`: forced size-limit boundaries; always `false`
  for code.

Both representations select the same results given the same model responses.
Human output adds a final LF for readability if the text lacks one; JSON's
`text` field is preserved without that addition.

For users of the former line-by-line mode: the old JSON contract
`path/line_number/text/probability` has been replaced by the fragment contract
above. `--json` remains an alias, but scripts reading the old schema must be
updated.

## Thresholds, limits, and short output modes

| Option | Behavior |
| --- | --- |
| `--threshold P` | Select a completed fragment with score ≥ P; default `0.5` |
| `--join-threshold P` | Join a log record with relation or usefulness score ≥ P; default `0.5` |
| `--max-line-bytes N` | Physical input-line size, including its line ending; default `16384` |
| `--max-fragment-bytes N` | Fragment text size in UTF-8 bytes; default `16384` |
| `--timeout SECONDS` | Total model-evaluation timeout including retries; default `30` |
| `-v`, `--invert-match` | Select fragments scoring below the threshold; grouping is unchanged |
| `-c`, `--count` | Print `source:count`, the number of selected fragments, including zero |
| `-l`, `--files-with-matches` | Print sources with matches |
| `-q`, `--quiet` | No output; stop after the first completed selected fragment |
| `-m`, `--max-count N` | Limit selected fragments per source |

```sh
jev-grep -c 'Certificate validation failure' application.log
jev-grep -l -g '*.log' 'Database connection failure' ./logs
jev-grep -q 'Build failure' build.log
jev-grep -v 'Routine successful operation' application.log
```

For text/log-jsonl, the source is the input file or stdin. For ast-grep, it is
the upstream `file` field, even when candidates for that file are interleaved
with others or read from several JSONL files. In code mode:

- `-l` prints each upstream path once.
- `-c` prints totals after EOF in order of first appearance; files that supplied
  no candidates are unknown to `jev-grep` and do not appear in counts.
- After reaching `-m`, further candidates for that path are validated but not
  sent to the model; candidates for other files continue to be processed.

`-c`, `-l`, and `-q` are mutually exclusive and reject `--format` (even `human`)
and `--json`. `--json` cannot be combined with `--format`. Thresholds must be
finite numbers in [0,1]; sizes, timeout, and `-m` must be positive. `-n` and `-H`
are accepted for compatibility, but location is already mandatory;
`--no-filename` is an error.

### Oversized fragments

If the next record does not fit in `--max-fragment-bytes`, the log fragment is
split **between records**, before a grouping request. Each part undergoes its
own selection and receives split markers, even if one part is not selected.
Reaching the limit exactly does not itself finish a fragment.

A single text line, decoded `text` field, or AST candidate exceeding this
limit causes an error: `jev-grep` does not split indivisible records. Exceeding
`--max-line-bytes` is a separate error before JSONL decoding.

**The client's byte limit does not increase the model's token context.**
Increase sizes with the Jev on OpenCode Zen configuration in mind: the server
may truncate long input, including the new record during boundary evaluation.
`jev-grep` does not send a `max_len` setting in HTTP requests.

## Exit codes and pipelines

- **0**: at least one fragment was selected.
- **1**: no fragments were selected.
- **2**: an input, option, or API error.

stdout contains results; stderr contains diagnostics. An error stops processing;
already emitted results are not retracted. An unfinished fragment is not
emitted as successfully completed on error. A closed downstream pipe is handled
without a BrokenPipe diagnostic.

`jev-grep` cannot learn the preceding command's exit status from EOF. Use
`set -o pipefail` in Bash: the pipeline status may differ from `jev-grep`'s
status. For example, a build may fail while a fragment is successfully found.
Early stopping with `-q`/`-m` can also close the pipe and cause upstream SIGPIPE.

## Performance and quality

HTTP requests are sequential with connection reuse; transient failures are
retried within the overall deadline under the retry policy. A normal successful
log pass needs one HTTP request per input record: `match` for the first record
or after a forced split, and four questions in one request for the others.
Code uses one `match` per evaluated candidate. Four questions do not mean four
HTTP requests, but increase inference work.

Fast preprocessing by ast-grep/agrind does not replace model evaluation.
Parallel or batched submission of independent states is not implemented in
the client.

The reader is bounded by the physical line, current fragment, and HTTP response
(64 KiB). Transport buffers, traversal state, and, for ast-grep, tracking of all
encountered upstream paths require additional memory.

Grouping and selection are probabilistic. Future evaluation methodology is
described in the [backlog](BACKLOG.md); the synthetic seed is in
[tests/fixtures](../tests/fixtures/jev-quality-seed.jsonl), without executed
results or a Jev quality assessment. Test your criterion, Jev model, and
thresholds on your own examples; these commands do not guarantee any model's
quality.

Evaluated text is sent to the selected API. Use a local server for local
processing. Bearer authentication through `JEV_GREP_API_KEY` and HTTPS with TLS
validation are supported; HTTP redirects and credentials embedded in URLs are
not accepted.
