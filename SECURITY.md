# Security

## Supported versions

mutastic for Rust has no releases yet, so fixes go into `main`. Once there are releases, fixes go
into the latest: mutastic is before 1.0, so older releases won't be patched.

## Reporting a vulnerability

Please report it privately, through GitHub's
[private vulnerability reporting](https://github.com/mutastic/mutastic-rs/security/advisories/new)
(the repository's *Security* tab, then *Report a vulnerability*), rather than in a public issue.

Say what's affected, how to reproduce it, and what it lets someone do. You'll get an acknowledgement
within a week, and we'll keep you informed while it's fixed. Once a fix is released, the advisory
is published, crediting you unless you'd rather not be.

## What mutastic runs

mutastic builds your crate and runs your tests, as `cargo test` does: treat running it on a project
as you would running that project's tests.

- **It builds a copy of your crate.** The copy lives in `target/mutastic`, and your source,
  `Cargo.toml` and `target/` are left alone. Building it runs what any build of your crate runs:
  build scripts and procedural macros, yours and your dependencies'. It builds with your own `cargo`
  and `rustc`, which fetch dependencies as your build does.
- **It runs your test binaries,** many times, with a mutant live in each run. A mutant is a bug put
  in on purpose, so a test may loop, panic, abort, or do what its code does on a wrong value, such
  as write a file somewhere else. Each run has a time limit, and is stopped when it's over.
- **It never mutates unsafe code by default.** Code in an `unsafe` block, an `unsafe fn` or an
  `unsafe impl`, and safe functions that hold an `unsafe` block, get no mutants: a mutant there can
  cause undefined behaviour. `--unsafe=enclosing` mutates those safe functions, and `--unsafe=all`
  the unsafe code too. With either, a test process can corrupt its own memory, so use them only on
  code whose tests you'd run under the same risk. See
  [Unsafe code](docs/concepts/unsafe.md).

mutastic itself has no unsafe code (`unsafe_code = "forbid"`), runs no network services, and makes
no network requests of its own.
