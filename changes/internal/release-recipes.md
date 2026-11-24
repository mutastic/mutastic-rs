- **A release is a list of `just` recipes.** `just release-prepare X.Y.Z` makes the release commit
  on a branch (the version in Cargo.toml and Cargo.lock, the changelog's section, the docs' kept
  outputs, what the release takes for compat, and the contract's pins pruned) and opens its pull
  request, through `scripts/release-prepare.sh`, whose `--dry-run` stops before the push.
  `release-check`, `candidate` and `release-verify` wrap the scripts and workflows that were typed
  by hand. `release`, Brian's, asks first, then tags main's release commit and pushes the tag
  (`scripts/release-tag.sh`, which for `X.Y.0` first checks the Java edition is out). RELEASING.md
  opens with the release as these recipes in order, and AGENTS.md says which an agent may run. The
  `release` environment's approval stays what guards a publish: `just --yes` skips a recipe's
  question.
