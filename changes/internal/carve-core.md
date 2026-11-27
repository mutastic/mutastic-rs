- **What every edition shares is a crate of its own, `mutastic-core`.** The Swift edition, a
  repository of its own, depends on mutastic-rs's crates by a git tag, so the code that knows no
  language moves out of `mutastic`: errors and their codes, the command line written back to copy,
  mutant ids, ignores and proposals, the lines a change touched, and amounts and times.
  `mutastic` re-exports it, or wraps it with Rust's own words: where to report a bug, Cargo among
  the tools an error's command runs, `rust` in an id, `*.rs` for `--since`. `tests/layers.rs` holds
  the core to depending on the output kit alone and naming nothing only the Rust edition knows,
  and ARCHITECTURE.md says how another edition depends on it. `publish = false`; the weekly
  self-run runs it whole, as it does the terminal crate. Nothing mutastic prints changes.
