- **`--operators` given twice adds, as in the Java edition.** `--operators=stronger
  --operators=-stream` runs `stronger` without the `stream` operators, where the second replaced
  the first. A list that only leaves operators out, `--operators=-mutastic/boundary`, leaves them
  out of the defaults, where it ran no operator at all. `run --help` lists `--shard` and
  `--from-shards` after `--workers`, in the editions' shared order.
