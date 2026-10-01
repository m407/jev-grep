# Documentation

**English** | [Русский](ru/README.md) · [Project overview](../README.md)

English is the default documentation language. The user and developer guides
below have Russian translations in `docs/ru` with matching filenames and
reciprocal language links. The portable agent skill in `docs/skill/jev-grep`
is an English-only package shared by both documentation versions.

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

## Agent skill

The portable [jev-grep skill](skill/jev-grep/SKILL.md) is distributed in English.
It covers standalone CLI use, `ast-grep` candidates, `agrind` log preparation,
data disclosure, and result coordinates. Its self-contained references are
[CLI and data contracts](skill/jev-grep/references/cli.md) and
[ast-grep and agrind pipelines](skill/jev-grep/references/pipelines.md).
Examples of requests:

```text
Use jev-grep to find unresolved certificate failures in ./logs
Use ast-grep and jev-grep to find input-validation functions in ./src
Prepare app.jsonl with agrind and use jev-grep to find unresolved connection errors
```

To use it, copy the entire `docs/skill/jev-grep/` directory, including
`references/`, to a skill location supported by your agent. Storing the package
in `docs/skill` does not activate it automatically. Install/configure the CLI
and optional tools separately; credentials are not included. All local links
inside the skill stay within its package. The instructions do not depend on a
particular agent, credential manager, or this repository's files/configuration.
External API requests still require authorization to disclose the selected data;
invoking the skill does not grant it.

## Maintaining translations

Update both language versions when changing documented behavior. Keep CLI flags,
environment variables, JSON field names, and executable examples equivalent;
translate prose and comments, not API identifiers. Link to pages in the current
language and retain a reciprocal language link.

Natural-language request examples may be localized; keep criteria in executable
CLI examples equivalent so translations do not change evaluation semantics.
Keep `docs/skill/jev-grep` in English rather than duplicating it under `docs/ru`;
update the skill links and usage guidance in both documentation indexes together.

OpenSpec planning artifacts and archived records are not part of these translated
user/developer guides.
