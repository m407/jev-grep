# Files

- [AST candidate search and upstream source accounting](code-candidates.md) - How ast-grep streams independently evaluated code candidates to jev-grep, and how validation, limits, and output accounting follow upstream source paths rather than JSONL transport files.
- [Streaming log grouping and selection](log-fragments.md) - How plain-text and log-jsonl records become pending semantic fragments, finish at boundaries, and are selected for output. Covers size limits, early stopping, live streams, and agrind preprocessing without losing sight of received-stream coordinates.
