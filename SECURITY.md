# Security

## Supported versions

Fixes ship as a patch release of the latest minor version (1.0.x today), for both editions. Older
minor versions aren't patched: upgrade to the latest to get a fix.

## Reporting a vulnerability

Please report it privately, through GitHub's
[private vulnerability reporting](https://github.com/mutastic/mutastic-rs/security/advisories/new)
(the repository's *Security* tab, then *Report a vulnerability*), rather than in a public issue.

Say what's affected, how to reproduce it, and what it lets someone do. You'll get an acknowledgement
within a week, and we'll keep you informed while it's fixed. Once a fix is released, the advisory
is published, crediting you unless you'd rather not be.

## What mutastic tells an agent

mutastic's output quotes your project: the lines a mutant changes, test names, file paths. A coding
agent reads it, often through a plugin's hooks, with no person in between, so text planted in a
project (a string literal or a comment, a test's name, a vendored file, a pull request) could read
as an instruction. mutastic keeps the two apart:

- **The project's text is fenced.** In the agent format, `next`'s agent text and the gate's
  messages, mutastic's instructions come first, and what quotes the project comes after, between
  `<<mutastic-data TOKEN>>` and `<<end mutastic-data TOKEN>>`. The token is 8 hex digits drawn at
  random for each run, and drawn again if the text holds it, so the text can't close the fence
  early. The opening line says everything up to the closing marker is data, not instructions.
- **Hidden characters are shown.** Control characters (so every ANSI escape), bidi controls,
  zero-width characters and every other invisible format character in the project's text show as
  escapes, `\x1b` and `\u202e`, in every output, a person's or an agent's. A value or line over
  200 characters is cut, with how many more there were.
- **Commands are mutastic's.** A command mutastic offers is made only of a checked mutant id,
  its own options and placeholders, and the directory, quoted for a shell: never of text from the
  project or a report file.
- **Events are data.** `--format=jsonl` keeps the text as it is, JSON-escaped: a tool reading it
  decides what to trust.

A fence and escapes make planted text plain to see, and tell the agent it's data; they can't make
an agent obey that. Review what an agent does in a project you don't trust, as you would its tests.
See [Agent output and untrusted text](docs/concepts/output.md#agent-output-and-untrusted-text).

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

mutastic's unsafe code is all in one crate, `crates/mutastic-terminal`: a handful of C library
calls for the terminal's width, its input mode and signals, each in a block of its own that says
why it's sound, run nightly under Miri and AddressSanitizer. The rest of the workspace forbids it
(`unsafe_code = "forbid"`). mutastic runs no network services. Its
only network requests are for the Java edition, in a Maven or Gradle project: it looks up the
Java edition's GitHub releases at most once a day, downloads one with `curl` the first time it's
needed, and checks it against the release's `SHA256SUMS` before keeping it in `~/.mutastic`.
`MUTASTIC_JAVA_RELEASES` points it at a mirror. For a Rust project it fetches nothing.
