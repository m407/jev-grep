# Code search with ast-grep

**English** | [Русский](ru/code-search.md) · [Documentation](README.md)

[ast-grep](https://ast-grep.github.io/) defines structural boundaries;
`jev-grep` checks meaning. Use **`scan --json=stream`**: one object per match.
Examples were checked with ast-grep 0.45.3. Configure credentials as described
in the [deployment guide](deployment.md); for Zen, invoke the consumer as
`fnox exec -- jev-grep ...`.

## Find matching Rust functions

Run from the root of a Rust project containing `src`:

```bash
set -o pipefail

ast-grep scan --json=stream --inline-rules '
id: rust-functions
language: Rust
severity: hint
rule:
  kind: function_item
' ./src |
  jev-grep --input-format ast-grep --max-line-bytes 262144 \
    'Function that validates input or rejects malformed data'
```

For machine-readable output, add `--format json` to `jev-grep`.
`severity: hint` lets the rule extract candidates; an error-level ast-grep
rule may return a nonzero exit code because of the matches themselves.

The physical JSONL-line limit is increased separately: ast-grep may repeat text
in `text`, `lines`, and `labels`, making the serialized object larger than the
function body. The body limit, `--max-fragment-bytes`, remains 16384 bytes by
default.

## Check AST candidates in changed Rust files

From the Git repository root, this Bash example compares the tracked index and
working tree with `HEAD`, then scans the current working version of each
changed, existing `.rs` file. It deliberately selects **all** functions in
those files, not just functions intersecting changed hunks. The deleted side
of the diff is not analyzed, so this search cannot detect removed behavior.
Untracked files are excluded.

```bash
set -e
set -o pipefail

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

# Check Git separately: process substitution would hide its exit status.
if ! git diff --name-only -z --diff-filter=ACMRTUXB HEAD -- >"$tmp"; then
  echo 'git diff failed' >&2
  exit 2
fi

files=()
while IFS= read -r -d '' path; do
  # Restrict sources to Rust; the existence check excludes deletions.
  [[ $path == *.rs && -f $path ]] && files+=("./$path")
done < "$tmp"

if ((${#files[@]})); then
  ast-grep scan --json=stream --inline-rules '
id: rust-functions
language: Rust
severity: hint
rule:
  kind: function_item
' -- "${files[@]}" |
    jev-grep --input-format ast-grep --max-line-bytes 262144 \
      'Function that handles an error or validates input'
fi
```

Git's NUL delimiters and the Bash array preserve paths with spaces and other
special characters. The empty-list branch matters: ast-grep without paths
may scan its default directory. Git failure stops the example before scanning
or API calls. `jev-grep` receives only ast-grep candidates; it does not read
Git history or parse the diff.

## Python code through stdin

```bash
set -o pipefail

cat service.py |
  ast-grep scan --stdin --json=stream --inline-rules '
id: python-functions
language: Python
severity: hint
rule:
  kind: function_definition
' |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Function that retries a failed network request'
```

The code source in this pipeline is `stdin`, not `service.py`: ast-grep receives
code without a path. To preserve the filename, pass `service.py` directly to
ast-grep instead of using `cat ... | ... --stdin`.

ast-grep reads code from stdin until EOF. `jev-grep` evaluates and emits the
supplied JSONL candidates one at a time, without waiting for the entire
candidate stream to reach EOF.

## Custom rules and saved candidates

AST node types depend on the language. To select your own functions, methods,
or declarations, define an ast-grep rule, for example in `rule.yml`, with
appropriate `language`, `kind`, and `severity: hint`:

```bash
set -o pipefail

ast-grep scan --json=stream --rule rule.yml ./src |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Code that handles authentication failures'
```

Alternatively, save ast-grep output and repeat semantic queries:

```sh
jev-grep --input-format ast-grep --max-line-bytes 262144 \
  'Code that handles authentication failures' candidates.jsonl
```

## Code-mode input contract

- Each candidate receives one `match` question. Adjacent matches are not
  grouped; `--join-threshold` has no effect in this mode.
- The evaluated content is `text`, not context from `lines`. Nested,
  overlapping, and duplicate nodes remain separate candidates. The upstream
  rule controls their selection.
- Accepted fields are `text`, `file`, `range.byteOffset.start/end`,
  `range.start.line/column`, and `range.end.line/column`; `language` is optional.
  `jev-grep` checks text/range consistency, but does not reopen the source file
  or check whether it changed after ast-grep ran.
- JSON arrays from `--json`/`--json=compact` and `outline` objects are not valid
  input.
- `jev-grep` options `-g`, `--hidden`, and `--no-ignore` filter input JSONL
  files, not the `file` inside candidates. Configure source-code file selection
  in ast-grep.

See the [CLI reference](reference.md) for output coordinates, per-source
counting, and limits.
