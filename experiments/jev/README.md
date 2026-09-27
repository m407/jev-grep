# Jev on Zen: synthetic experiment

This is the corpus and runner for real Jev inference. It makes no general live-quality claim; mock-based offline tests are transport/runner checks only. The archival-transfer task is complete, and the current live result is documented in [RESULTS.md](RESULTS.md).

## Reproduction

Build a client separately, then run with the project keychain launcher and a fresh output path:

```sh
cargo build --locked --release
fnox exec -- python3 experiments/jev/run.py \
  --bin target/release/jev-grep \
  --out /tmp/opencode/jev-grep-migrate-to-jev-zen-run
```

The runner reads only `JEV_GREP_API_URL` (default `https://opencode.ai/zen/v1/systemone`), `JEV_GREP_MODEL` (default `jev-1.13`) and `JEV_GREP_API_KEY`. Do not pass a key on a command line. `--out` must not exist; this prevents prior evidence from being mistaken for the current run. A missing key or failed/incomplete authorized probe writes a sanitized `blocker.json` and stops before corpus invocations. It never calls `/health`. The runner uses standard-library Python and does not build/install the binary.

## Corpus and labels

Every JSONL row is synthetic and includes an id, input kind, query, predeclared selected line ranges, and an explanation of the boundary. `gradle-stacktrace` preserves the compiler/stack example; `retry-recovery` joins retry to recovery; `useful-neighbor` includes explanatory context; `independent-errors` expects separate npm and Buildah fragments; `npm` and `buildah` retain the single-failure examples. `byte-edge` retains the 128-byte client edge and is not a token-context test. `long-input` replaces the former context-budget assertion: it measures a long synthetic input only. `ast-rust-candidate` supplies an upstream AST candidate with source and byte/line coordinates, without running ast-grep. `russian-mixed` exercises Russian query and mixed Cyrillic/English diagnostics. Expected boundaries are qualitative labels, not a promised pass score.

Before corpus calls, the runner makes three bearer-authenticated probes: string state/single question, object state/four named questions, and a long-input telemetry observation. It requires complete finite, non-boolean numeric answers in [0,1], records only status/model/usage, and never stores request headers or response bodies. Corpus cases run through the real `jev-grep` binary; raw stdout/stderr, selected line ranges, expected ranges, exit status and wall time are recorded per case. Query, endpoint, model and byte limits are recorded. The JSONL dataset SHA-256 identifies exact inputs. A model-quality mismatch on client exit 0/1 is a completed observation, not infrastructure failure.

For a completed client invocation (exit 0/1), the runner records a **flow-derived** count: one API request per physical text record, including a final unterminated record, or one per supplied AST candidate. This follows the current client control flow and is not server instrumentation. A failed invocation's count is unknown. The three completed direct probes are counted separately; their usage is not attributed to corpus calls. Long input length/usage does not prove context occupancy or a model context limit. If a corpus client exits with an error, the runner preserves the case evidence, writes an incomplete status and sanitized blocker, and exits nonzero. Do not commit evidence containing private input or credentials; evidence is synthetic here and the runner omits Authorization headers and redacts the key from stderr.

The methodology enumerates every labelled case in `dataset.jsonl`. For a live report, compare selected ranges to each `expected` label, list every false join/split, miss and error, report latency and distinguish measured probe usage from flow-derived client requests. This small synthetic set is qualitative, not a representative accuracy benchmark.
