- **A name given to `--exclude-tests` that matches no test says so.** One that named none of the
  package's tests, mistyped or since renamed, was taken, left nothing out and said nothing. Now
  the run says so in every format, `! left out nothing: tests::big_order names no test of pricing.
  There's one called tests::big_orders.`, with `matched: false` and the close name as `maybe` in
  `run-finished`, and the Score row doesn't count it among the tests left out. It's a warning, as
  in the Java edition: a test renamed since it was listed under `exclude-tests:` doesn't stop the
  run.
