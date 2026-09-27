# Zen deployment and local credentials

`jev-grep` is a client for OpenCode Zen; fnox supplies its API key at process
launch. The application defaults to the Zen System One endpoint
`https://opencode.ai/zen/v1/systemone` and model `jev-1.13`. The model is sent
explicitly. `fnox` is a launcher, not a runtime dependency of the binary.

## Requirements

- Install the `jev-grep` binary using the project installation instructions.
- Install fnox and make it available on `PATH` for credential-backed runs.
- On Linux, provide an available, unlocked Secret Service (commonly through a
  desktop keyring such as GNOME Keyring, or another Secret Service-compatible
  daemon). Headless/SSH sessions may not have an unlocked service; unlock or
  arrange access locally before running fnox. There is deliberately no
  plaintext fallback.

The project `fnox.toml` refers to a keychain entry; it contains no API key.
`root = true` makes this project configuration the fnox root instead of
searching parent directories. It does not disable fnox's global configuration.

## Store the key

From the repository root, run the following command without providing a value:

```sh
fnox set JEV_GREP_API_KEY --provider zen-keychain
```

fnox prompts interactively with hidden input. Do not pass the key as a command
argument, put it in shell history, or commit it. The secret is stored by the
system keychain provider under the `jev-grep` service. Git ignores
`fnox.local.toml` for any local fnox overrides.

Run the client through fnox so the secret is resolved before the child starts:

```sh
fnox exec -- jev-grep --json 'Certificate validation failure' sample.log
```

If the keychain is unavailable or the entry is missing, fnox must stop with an
error before starting `jev-grep`. Do not replace this with an empty key or a
plaintext file. A direct invocation without fnox remains possible for
explicitly configured mock/custom endpoints; it does not provide the normal
Zen credential setup.

## Manual overrides

The client accepts flags and the corresponding environment variables. Flags
take precedence over environment values, which take precedence over defaults.
Use the new `JEV_GREP_*` names only:

```sh
JEV_GREP_API_URL='https://opencode.ai/zen/v1/systemone' \
JEV_GREP_MODEL='jev-1.13-free' \
fnox exec -- jev-grep 'Certificate validation failure' sample.log
```

`--api-url`, `--model`, and `--api-key` can override the matching environment
settings. Prefer fnox for the API key; avoid putting key values in shell
history or process arguments. A manually selected endpoint must be the complete
System One endpoint. There is no automatic endpoint/model fallback.

## Data, cost, and context

Requests send input text to the selected endpoint. Use only data you are
authorized to disclose; synthetic examples are preferable for testing. Zen
model availability and pricing can change; check the [current Zen Jev
documentation](https://opencode.ai/docs/zen/#jev) before deployment rather
than relying on a copied price. Checked on 2026-09-25, that page listed
`jev-1.13` at $0.042 per million input tokens with output tokens free; cached
pricing was not listed. Prices and availability can change. The selected model
is `jev-1.13`; the free
`jev-1.13-free` ID is available only when explicitly selected as above.

As documented for Jev models on 2026-09-25, the direct TypeSafe API describes
64k context for state and all questions together, and 32k for state plus the
longest question at once ([model documentation](https://docs.typesafe.ai/models)).
These are upstream model/API descriptions, not verified Zen gateway limits.
They do not imply a token count from input bytes. The client's fragment limits
are byte-based and are separate from any model token context. Measure with
authorized synthetic inputs; a long input alone does not prove the context
limit.

The [project experiment methodology](../experiments/jev/README.md) and any real-inference report are separate evidence from the published model specifications.
