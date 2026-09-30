# Development checks

**English** | [Русский](ru/development.md) · [Documentation](README.md)

Run from the repository root:

```sh
cargo test --locked
cargo build --locked
python3 -m unittest discover -s tests -v
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Acceptance tests use the Python standard library and a local mock HTTP API.
They check grouping with predefined model responses, input/output formats,
coordinates, limits, filters, and errors. Neither the Jev on OpenCode Zen server
nor model weights are required; these tests do not measure real model accuracy.

Automatic releases from `main`, Conventional Commits, and release-tooling
checks are described in the [release guide](releasing.md).
