## What and why

<!-- What this changes, and why: what was wrong or missing. -->

## Checked

- [ ] `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
- [ ] A test that fails without the change
- [ ] Docs updated, and `MUTASTIC_DOCS_WRITE=1 cargo test --test docs` run if what mutastic prints changed
- [ ] `MUTASTIC_IDS_WRITE=1 cargo test --test ids` run if mutants were added, with only lines added to the list
