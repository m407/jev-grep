# Documentation

**English** | [Русский](ru/README.md) · [Project overview](../README.md)

English is the default documentation language. Russian translations live in
`docs/ru` with matching filenames. Each page links to its translation.

## Guides

1. [Installation and configuration](installation.md)
2. [Zen deployment and local credentials](deployment.md)
3. [Log search: text, streams, JSONL, and agrind](logs.md)
4. [Code search with ast-grep](code-search.md)
5. [CLI reference](reference.md): output, options, limits, exit codes, and performance
6. [Development checks](development.md)
7. [Automatic releases and Conventional Commits](releasing.md)
8. [Deferred directions](BACKLOG.md)

## Choose an input mode

| Input | `jev-grep` option | Search unit | Result coordinates |
| --- | --- | --- | --- |
| Plain-text log | `--input-format text` (default) | Adjacent lines grouped by the model | In the input file/stdin |
| JSONL with a `text` field | `--input-format log-jsonl` | Adjacent records grouped by the model | In the physical JSONL file/stdin |
| `ast-grep scan --json=stream` matches | `--input-format ast-grep` | One supplied AST candidate | In source code, as reported by ast-grep |

Select output independently: `--format human` (default) or `--format json`
(JSONL). `--json` is shorthand for `--format json`.

## Maintaining translations

Update both language versions when changing documented behavior. Keep CLI flags,
environment variables, JSON field names, and executable examples equivalent;
translate prose and comments, not API identifiers. Link to pages in the current
language and retain a reciprocal language link.

OpenSpec planning artifacts and archived records are not part of these translated
user/developer guides.
