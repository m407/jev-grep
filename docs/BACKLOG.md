# Deferred directions

This backlog records future work only; none of these capabilities is implemented by the current change. The synthetic examples in [`tests/fixtures/jev-quality-seed.jsonl`](../tests/fixtures/jev-quality-seed.jsonl) are an unexecuted seed, not Jev quality evidence.

## 1. Estimate

Explore an estimate or range for evaluation cost and duration that accounts for log-record grouping and model-dependent boundaries. Any estimate must expose assumptions and uncertainty; do not imply a fixed request count per input line.

## 2. Quality-evaluation methodology

Define selection boundaries, case selection, prompt wording, language coverage, and reproducibility before running a representative evaluation. The preserved synthetic seed can inform the methodology, but its labels are not measured Jev outcomes and it is not a benchmark by itself.

## 3. Statistics

Consider collecting real request counts, retries, requested/returned model and usage metadata, and elapsed time. Keep instrumentation distinct from inferred flow counts, and do not change the current JSONL output contract as part of this backlog item.

## 4. Response cache

Assess a cache keyed by endpoint, requested model, state, and questions. Account for aliases versus the model version actually served, and avoid reusing responses across semantically different requests.

## 5. Parallel evaluation

Consider evaluating independent AST candidates with bounded concurrency and an ordered result buffer. Log-record grouping has sequential dependencies and must retain its current ordering semantics.
