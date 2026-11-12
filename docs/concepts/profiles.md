# Profiles

mutastic builds your crate with a profile of its own, `mutastic`, made from Cargo's `test` profile.
It changes three settings, and only for that build. Your `Cargo.toml` is never touched, so your own
`cargo test` is built as it always was.

| Setting | mutastic's | Why |
|---|---|---|
| `overflow-checks` | always `true` | an arithmetic mutant's meaning depends on it |
| `debug-assertions` | always `true` | so does what std checks for you |
| `debug` | `"line-tables-only"` | line numbers in panics, without the rest of the debug info, which slows the build |

Everything else, `opt-level` included, comes from your `test` profile, which inherits `dev` unless
you've set it otherwise.

## Why overflow checks are forced

Integer overflow isn't undefined behaviour in Rust: with overflow checks on it panics, and with them
off it wraps. So the same mutant can do two different things. Take `a + b` changed to `a - b` on
two `u32`s, where `a` is smaller:

- with overflow checks, `a - b` panics with `attempt to subtract with overflow`, the test fails,
  and the mutant is caught;
- without them, it wraps to a huge number, which a test may or may not notice.

If mutastic used your profile as it is, a `[profile.test] overflow-checks = false` would quietly
change which mutants are caught, and two runs of the same code with different profiles couldn't be
compared. So mutastic turns them on, whatever your profiles say.

A mutant caught by the language's own panic says so. This is `a + b` turned into `a - b` in the
unsafety fixture, whose test adds 1 and 2:

<!-- output: concepts/verify-panic -->
```text
mutastic: m24a65ba1 is caught by run::totals, through a panic: attempt to subtract with overflow
```

Its event has the same words in `caughtWith`. That's a real catch, as the test failed, but it's
worth knowing when a test caught a change only because the arithmetic overflowed, and not because
it checked the answer.

## Why debug assertions are forced

`debug_assert!` checks are part of what your code promises, so a mutant that breaks one is caught
by it. They also turn on std's own checks of unsafe preconditions in debug builds: a call such as
`slice::from_raw_parts` with a null pointer stops with `unsafe precondition(s) violated` instead of
going on into undefined behaviour. mutastic reports that stop apart from a test's catch: see
[Unsafe code](unsafe.md#when-std-stops-a-mutant).

## What the run records

The `run-started` event says how the build was made: the settings Cargo reports it used for your
crate, and how unsafe code was treated. This is the unsafety fixture, whose `Cargo.toml` turns both
checks off for tests:

<!-- from: fixtures/unsafety/Cargo.toml -->
```toml
# mutastic turns both back on: what a mutant does mustn't depend on these.
[profile.test]
overflow-checks = false
debug-assertions = false
```

and mutastic's build of it still has them on:

<!-- output: concepts/run-started -->
```text
{
  "schema": "mutastic.events/v1",
  "edition": "rust",
  "reason": "run-started",
  "project": "unsafety",
  "rule": "add or change tests only; never change production code to make a mutant go away: tell the person",
  "decline": "if a mutant's case can't happen in practice, or the change is one no caller can see, don't contrive a test: propose ignoring it, for a person to accept, as the kind it is (equivalent, unreachable, telemetry, deprecated or accepted-risk), with the reason in two sentences: the one case where it differs, and why that can't happen",
  "profile": {
    "name": "mutastic",
    "optLevel": "0",
    "overflowChecks": true,
    "debugAssertions": true
  },
  "unsafe": "skip"
}
```

Compare scores only between runs whose `profile` matches.

## What isn't changed

- **`panic = "abort"`** doesn't reach tests: Cargo always builds tests to unwind, as libtest needs.
- **Your rustflags** in `.cargo/config.toml` or the environment still apply. mutastic adds its one
  flag through `RUSTC_WORKSPACE_WRAPPER`, which reaches only your own crates, so your dependencies
  aren't built again.
- **Lints** are capped, so a `#![deny(warnings)]` in your crate doesn't stop a build whose rewritten
  code it was never meant to judge.
