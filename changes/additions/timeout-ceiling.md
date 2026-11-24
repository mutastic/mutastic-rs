- **`timeout-ceiling:` in `mutastic.yaml` sets the most one mutant's tests may take.** Without it,
  the ceiling is ten minutes, or twenty times the suite's own time when that's longer. It takes a
  whole number with its unit, `timeout-ceiling: 30m`, `90s` or `2h`, and a mutant stopped there is
  timed out with a note that says so: `its tests ran for the most a mutant's tests may, 30m00s
  (timeout-ceiling:)`. The Java edition doesn't read it yet
  ([docs/reference/configuration.md](docs/reference/configuration.md)).
