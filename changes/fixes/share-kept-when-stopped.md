- **A share stopped part way keeps what it finished.** A share's `events.jsonl` was written only
  at the end, so one stopped by a job's time limit, `timeout` or Ctrl-C left nothing: base64's share
  ran 35 of its 36 mutants, then lost them all. The file is now written as the run goes, each
  mutant as it finishes, and has no `run-finished` until the share does, so `--from-shards` still
  refuses it, now saying how many mutants it holds. A mutant whose tests the same Ctrl-C or kill
  ended isn't kept as caught ([docs/concepts/how-a-run-works.md](docs/concepts/how-a-run-works.md)).
