# Changes

What changes from one release of mutastic for Rust to the next, written as changes land. The
newest come first.

## Unreleased

### Scoring changes

Expect your score to move, for these reasons, not because your tests changed:

- **`mutastic/unwrap-try` is in the defaults.** `f()?` becomes `f().unwrap()`, so the error `?`
  would hand back panics instead. When no test makes that call fail, the mutant is missed: an error
  path no test runs, which line coverage can't see, since the line runs either way. It was in
  `stronger`; now every run has it unless `--operators` says otherwise. A `?` that passes on only a
  formatter's error (inside a `Display` or `Debug` `fmt`, or on a write to a `Formatter`) is skipped
  as equivalent, since no test can make it fail. Crates that use `?` get more mutants and their
  scores usually fall a little: on five real crates, the defaults score moved by between −1.7 and
  +0.4 points. No mutant's id changes, and `--operators=stronger` and `extended` are unchanged. See
  [Unwrap try](docs/operators/returns.md#unwrap-try).
