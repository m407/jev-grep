# jev-grep

**English** | [Русский](README.ru.md)

An [agent skill](docs/skill/jev-grep/SKILL.md) helps coding agents choose input
modes, build pipelines, and interpret results. Install it with the Skills CLI:

```sh
npx skills add https://github.com/m407/jev-grep/tree/main/docs/skill/jev-grep --skill jev-grep
```

The skill does not install the CLI; see [Quick start](#quick-start) below.

Semantic search for **log and code fragments** using the Jev model over an API.
You can use OpenRouter or other providers offering the model through an API,
provided their endpoint supports the System One request/response protocol used
by `jev-grep`; an OpenAI-compatible chat endpoint alone is not sufficient.
Describe what you want in natural language instead of writing a regular
expression. `jev-grep` returns
matching text with its source, line range, and, in JSONL output, byte offsets.

- **Logs:** read text or JSONL, group adjacent records into meaningful fragments,
  and evaluate each fragment as a whole.
- **Code:** use `ast-grep` to extract functions, methods, or other AST nodes;
  evaluate each candidate independently while preserving its location.

```text
text logs -------------------------------+
logs --> agrind --> JSONL {text: ...} ----+--> jev-grep --> human / JSONL
code --> ast-grep --> JSONL matches -----+
```

The Rust CLI runs locally; model inference runs at the configured API endpoint.
External tools are invoked explicitly in shell pipelines. No repository index
is required.

## Quick start

Install from the repository root (Rust with edition 2024 support required):

```sh
cargo install --path . --locked
jev-grep --help
```

Configure your provider's endpoint, model, and API key using `JEV_GREP_API_URL`,
`JEV_GREP_MODEL`, and `JEV_GREP_API_KEY` (or `--api-url` and `--model`). See the
[configuration guide](docs/installation.md#connect-the-model). The
[credential setup guide](docs/deployment.md) shows a Zen example with fnox;
Zen is the default, not a required provider. With credentials configured in
fnox, run from the repository root:

```sh
fnox exec -- jev-grep 'Certificate validation failure' application.log
fnox exec -- jev-grep --format json 'Unresolved network failure' build.log
```

Input text is sent to the selected API. Only use data you are authorized to
disclose. Selection is probabilistic: validate your criterion and thresholds on
your own examples.

## Documentation

Full documentation: [English](docs/README.md) · [Русский](docs/ru/README.md).

| Guide | Contents |
| --- | --- |
| [Installation](docs/installation.md) | Requirements, defaults, and configuration |
| [Deployment](docs/deployment.md) | Credentials with fnox (Zen example), data handling, and integration smoke evidence |
| [Logs](docs/logs.md) | Files, live streams, JSONL, and Angle Grinder pipelines |
| [Code search](docs/code-search.md) | ast-grep, changed Git files, and AST input contract |
| [CLI reference](docs/reference.md) | Output schema, coordinates, options, limits, and exit codes |
| [Development](docs/development.md) | Offline checks and test scope |
| [Releases](docs/releasing.md) | Conventional Commits and automatic releases |
| [Backlog](docs/BACKLOG.md) | Deferred work and quality-evaluation methodology |
