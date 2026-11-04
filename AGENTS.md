## Development quality gates

- For setup, check failures, or tool/config changes, read `docs/development.md`; for commit messages, release workflows, or Dependabot changes, read `docs/releasing.md`.
- Keep pinned tools in `mise.toml` and shared hook checks in `hk.pkl`. Preserve Rust/Python/JavaScript tests and Commitlint when changing the gates.
- Before declaring implementation complete, run `mise run check` successfully. Targeted checks support iteration; the whole-project aggregate is the completion gate. Report blocked or failing checks with their complete output.
- Fix findings at their source. Suppressions, baselines, disabled rules, and skipped checks require an explicit human decision. Approved exceptions must name the reason, affected path, and rule; security exceptions also require an enforced expiration.
- Keep English and Russian development/release guides synchronized when their commands or policies change.

<!-- OPENWIKI:START -->

## OpenWiki

This repository has a generated `openwiki/` evidence index. It is optional just-in-time context, not required startup reading.

- Do not enumerate, preload, or search wikis at task start. Use retrieval when the user asks for it, when unfamiliar architecture or dependency behavior materially affects the task, or when source inspection leaves an important uncertainty. Stop once the question is grounded.
- When those conditions apply and OpenWiki retrieval tools are available, use `openwiki_search` for just-in-time context and `openwiki_read` for the relevant complete sections. If search returns `workspace_required`, ask which listed workspace to use and retry with its ID.
- Use `openwiki_list_workspaces` or `openwiki_list_wikis` when workspace membership itself needs to be discovered.
- If the retrieval tools are unavailable, read `openwiki/quickstart.md` and follow its links to the relevant pages.
- Treat source code and tests as authoritative. A brief's unknowns and review items are verification gaps, not automatic requirements.
- Prefer the narrowest quiet validation that proves the changed behavior. Preserve complete failure output.

The scheduled OpenWiki GitHub Actions workflow refreshes the repository wiki. Do not hand-edit generated OpenWiki pages unless explicitly asked; prefer updating source code/docs and letting OpenWiki regenerate.

<!-- OPENWIKI:END -->
