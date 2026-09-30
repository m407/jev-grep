# Installation and configuration

**English** | [Русский](ru/installation.md) · [Documentation](README.md)

## Install

From the repository root:

```sh
cargo install --path . --locked
jev-grep --help
```

Rust with edition 2024 support is required; builds have been checked with Rust
1.98.1. Dependencies are pinned in `Cargo.lock`. The client itself does not need
Python or PyTorch.

## Connect the model

See [Zen deployment and local credentials](deployment.md) for fnox/keychain
setup. By default, `jev-grep` uses `https://opencode.ai/zen/v1/systemone` with
model `jev-1.13`. There is no automatic endpoint or model fallback.

```sh
export JEV_GREP_API_URL=https://opencode.ai/zen/v1/systemone
export JEV_GREP_MODEL=jev-1.13

# Configure the key through fnox first; see the deployment guide.
fnox exec -- jev-grep 'Certificate validation failure' application.log
```

You can also select the endpoint and model with `--api-url` and `--model`.
The selected model must support the languages of the criterion and input.
Overrides use `JEV_GREP_API_URL`, `JEV_GREP_MODEL`, and `JEV_GREP_API_KEY`;
flags take precedence over environment variables, then defaults.

Next: [choose an input mode](README.md#choose-an-input-mode),
[search logs](logs.md), or [search code](code-search.md).
