# Troubleshooting

What to do when a run doesn't go as you expect: a mutant skipped, a run slower than it should be, a
package mutastic won't run, code it doesn't mutate. Each section starts from what you see, shows it
on one of the repository's fixtures where it can, then says what to do. What each exit status
means is on [Environment and exit codes](reference/commands.md#exit-codes).

## A mutant is skipped as unviable

A few operators make changes the compiler may reject: a call left out whose result is another type,
arguments swapped whose types differ, a `?` turned into `.unwrap()` where the error type matters.
Each mutant and your code are arms of one `match`, so they must have one type. When one doesn't
compile, the build withdraws it, builds again, and the mutant is skipped as unviable, with the
compiler's error as its reason. It's left out of the score, and you don't need a test for it.

The summary counts them (`1 skipped (1 unviable)`). To see which, and why, use `--format=jsonl`:
each has `"status": "Ignored"`, `"skipKind": "unviable"` and the compiler's error code and message.
This is `.to_lowercase()` left out of `input.trim().to_lowercase()`, which would make a `&str` where
the function returns a `String`:

<!-- output: troubleshooting/unviable-event -->
```text
{
  "schema": "mutastic.events/v1",
  "edition": "rust",
  "reason": "mutant-finished",
  "id": "m4db96a57",
  "fingerprint": "4db96a5721e0ee95",
  "status": "Ignored",
  "operator": "mutastic/omit-call",
  "docs": "https://docs.mutastic.io/rust/operators/arguments/#omit-call",
  "location": {
    "file": "src/arguments.rs",
    "line": 4,
    "column": 18,
    "class": "arguments",
    "method": "username"
  },
  "mutation": {
    "original": ".to_lowercase()",
    "replacement": "",
    "description": "`.to_lowercase()` is left out",
    "applied": {
      "level": "source",
      "text": "input.trim().to_lowercase() -> input.trim()"
    }
  },
  "statusReason": "unviable: doesn't compile (E0308: mismatched types)",
  "skipKind": "unviable",
  "coveredBy": [],
  "testsRun": 0,
  "durationMs": 0
}
```

Unviable mutants cost a round of the build each time some are found, never a test run. If a run
has many, they come from the operators that lean on the compiler, which are in `stronger` and
`extended`. The defaults make changes that compile, but for a `?` turned into `.unwrap()` where the
error has no `Debug` or the type isn't an `Option` or a `Result`, which is rare.

**When the build fails and no mutant explains it,** mutastic stops with the compiler's error:
`the build fails, and not because of a mutant`. Check that `cargo build --tests` passes in the
package as it is. If it does, and mutastic's build still fails, that's a bug in mutastic: please
report it with the error.

## A loop counter takes a long time

A mutant that stops a loop's counter from moving makes the loop run forever. mutastic stops each
such run when it has taken several times as long as its tests did without a mutant, and counts the
mutant as timed out, which counts as caught: the tests did notice, by never finishing. A loop
counter can have several such mutants (`n /= 10` to `n %= 10`, `10` to `1`), and each waits for its
time limit, so a run with many takes noticeably longer. This is a crate whose only function counts
digits, run with `--operators=stronger`:

<!-- from: fixtures/loops/src/lib.rs -->
```rust
pub fn digits(mut n: u64) -> u64 {
    let mut count = 1;
    while n >= 10 {
        n /= 10;
        count += 1;
    }
    count
}
```

<!-- output: troubleshooting/loops -->
```text
   Mutants  ✓ 14 caught · ✗ 6 missed · ⏱ 5 timed out (25)
  Worst at  digits() 6 ✗
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  25 (25 without coverage selection)
  Start at  11:03:14
  Duration  10.1s (setup 19ms, build 20ms, coverage 2ms, mutants 10.0s, checking 8ms)
     Tests  1 on rustc 1.99.1 · 5 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  76.0%   FAIL  Below the threshold of 80: 6 missed, 0 not covered.
```

Nothing needs fixing: those mutants are caught. If the time matters more than the extra mutants,
run the defaults, which make few of them; `stronger` and `extended` add the constants and the
arithmetic that hang loops.

## A slow test suite

Each mutant runs only the tests that reach it, and stops at the first that fails, so a run costs
far less than one suite run per mutant. When it's still slow:

- **Look at what's slow in your tests first.** The coverage pass runs every test once, on its own,
  and a mutant's time limit follows its tests' time. A test that sleeps, or waits on the network,
  slows every mutant it reaches.
- **Use more workers** if the machine has the cores: `--workers` sets how many mutants run side by
  side.
- **Run fewer mutants.** `--operators=defaults` is the smallest set, and `--only` runs just the
  mutants you name, such as the ones a card listed.
- **Keep `target/mutastic`.** The copy's build is kept warm between runs; in CI, cache it with the
  rest of `target/`.
- **Split it across CI jobs.** `--shard` runs a share of the mutants in each job, and
  `--from-shards` puts them back together as the whole run: see
  [One run across CI jobs](concepts/how-a-run-works.md#one-run-across-ci-jobs).

A test that starts its own threads still has its work credited to it: mutastic records what each
test reaches by running it alone.

## A workspace

mutastic runs one package at a time. Pointed at a workspace's root, it stops and gives the command
for each member, from where you are:

<!-- output: troubleshooting/workspace -->
```text
mutastic: fixtures/workspace is a workspace's root, and mutastic tests one package at a time.
It has 2 packages. To test one:
  mutastic run fixtures/workspace/billing
  mutastic run fixtures/workspace/shop
```

Run one of them, or `cargo mutastic run` inside the member. A workspace of more than ten packages
is listed by name, with the command to fill in. Anywhere inside a member, such as its `src`,
mutastic runs that member, as Cargo would, and reads its `mutastic.yaml`, ignores and baseline from
the member's directory. A member's package name typed where there's no directory of that name,
`mutastic run shop` with `shop` in `crates/shop`, runs that member, and a note says which
directory it used. A root package that is also the workspace's root runs as itself. Today:

- **Only the package's own tests count.** A test in another member that calls the package's code
  doesn't reach its mutants, so that code may show as not covered. Move or copy such tests into the
  package, or run mutastic on the member whose tests you want to judge.
- **The whole workspace is copied,** so path dependencies on other members build as usual, but
  only the package named is mutated.
- **A proc-macro crate** can't be mutated yet: mutastic says so and stops.

## mutastic says there's no directory, or no project

A directory mutastic is pointed at is checked before anything runs there. One that isn't there is
named as you typed it, with where mutastic looked. A package or directory whose name is close to it
is offered, never run, and the message ends in the command to run instead:

<!-- output: troubleshooting/no-directory -->
```text
mutastic: there's no directory fixtures/workspace/shpo in /home/you/project.
There's a package called shop, in fixtures/workspace/shop. To test it:
  mutastic run fixtures/workspace/shop
```

With no build file in the directory or any above it, `Cargo.toml` for Rust or `pom.xml`,
`build.gradle(.kts)` or `settings.gradle(.kts)` for Java, there's no project to test. Go to your
project, or name its directory. mutastic's options are all long ones, as `--format=full`: a word
such as `-v` is an option it doesn't have, and `mutastic run --help` lists run's. When there's no
`cargo` to run, mutastic says where it looked: the `CARGO` variable, when it's set, or the PATH.

## A word mutastic doesn't take

A command, option or value mutastic doesn't have is never guessed at and run. When one it has is
close, the error says so and ends in your line again with only that word changed; a value goes
after an `=`, never a space:

<!-- output: troubleshooting/mistyped -->
```text
mutastic: run has no option --formt. There's one called --format.
  mutastic run fixtures/pricing --format=agent
```

With nothing close, it gives the command's usage, and `mutastic run --help` for its options. A
`mutastic.yaml`, a rules file and `.mutastic/ignores.yaml` are read the same way: an error names
the file and its line, and what that line could hold.

## Test code isn't mutated

mutastic never mutates your tests, only the code they test. Test code is known by its attributes:

- a function marked `#[test]`, or a test attribute from a crate, such as `#[tokio::test]`,
  `#[rstest]` or `#[test_case(..)]`, and `#[bench]`;
- anything under a `cfg` that needs `test`: `#[cfg(test)]`, and combinations that hold only when
  testing, such as `#[cfg(all(test, unix))]`, or also when testing, such as
  `#[cfg(any(test, feature = "fuzzing"))]`.

`#[cfg(not(test))]` code is yours, and is mutated. Integration tests in `tests/`, examples and
benches aren't crate roots of your library or binaries, so nothing in them is mutated either. In the
unsafety fixture, neither of these is mutated:

<!-- from: fixtures/unsafety/src/lib.rs -->
```rust
/// Test code by its `cfg`, however it's combined.
#[cfg(all(test, unix))]
mod unix_tests {
    pub fn helper(x: u32) -> u32 {
        x + 1
    }

    #[test]
    fn helps() {
        assert_eq!(helper(1), 2);
    }
}

#[cfg(any(test, feature = "fuzzing"))]
pub fn test_helper(x: u32) -> u32 {
    x * 2
}
```

## Code behind other `cfg`s

mutastic builds what Cargo builds by default, on your machine: the default features, for the
platform you're on. Code under a `cfg` that's off, such as `#[cfg(windows)]` on macOS, or a feature
that isn't on by default, isn't in that build, so no test could reach a mutant there. mutastic
doesn't make those mutants: the summary counts them by the `cfg` that's off, and they're out of the
score. This crate has a function for each platform and one behind a feature that isn't on by
default:

<!-- from: fixtures/platforms/src/lib.rs -->
```rust
/// Whether a path starts at a drive, on Windows.
#[cfg(windows)]
pub fn is_absolute(path: &str) -> bool {
    path.len() > 2 && path.as_bytes()[1] == b':'
}
```

<!-- output: troubleshooting/platforms -->
```text
   Mutants  ✓ 9 caught (9)
  Left out  not built here: `cfg(feature = "simd")` (2), `cfg(windows)` (15)
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  9 (18 without coverage selection)
  Start at  11:04:59
  Duration  62ms (setup 20ms, build 18ms, coverage 2ms, mutants 6ms)
     Tests  2 on rustc 1.99.1 · 4 workers
   Reports  target/mutastic/mutation.json · target/mutastic/mutation.html
     Score  100.0%   PASS  Every mutant was caught.
```

What's off is decided as the build decides it: the platform's settings come from `rustc --print cfg`
for the target Cargo builds for (`CARGO_BUILD_TARGET`, and the flags in `RUSTFLAGS`), the features
from your `Cargo.toml` (the default ones, and what they turn on), and `debug_assertions` is on,
because mutastic's build turns it on. A `cfg` mutastic can't judge, such as one a build script or
`RUSTFLAGS` sets (`docsrs`, `tokio_unstable`), counts as built, so its code is still mutated: code
left out by mistake would hide a gap, where code kept by mistake only shows as not covered. A
`.cargo/config.toml` that sets the target or the flags isn't read: set them in the environment
instead.

To judge the tests of code for another platform, run mutastic on that platform. For a feature,
make it a default one while you run, or run on a package that turns it on.

## The package doesn't build

mutastic builds a copy of the package with its mutants in. When that build fails and no mutant is to
blame, mutastic checks the package as you wrote it, and names each error where it is in your files,
with the command that shows them in full:

<!-- output: troubleshooting/broken -->
```text
mutastic: pricing doesn't build, before any mutant is in it.
The error is in src/lib.rs, line 27: cannot divide `u32` by `&str`.
To see it in full:
  cargo test --no-run --manifest-path fixtures/pricing/Cargo.toml -p pricing
```

If your package builds and only mutastic's copy doesn't, that's a bug in mutastic, and it says so,
with the copy's first error to report.

## A test fails before any mutant

mutastic runs every test once, alone, before any mutant, and every test has to pass. One that
fails runs again on its own; one that fails every time stops the run, with how it failed, where its
output is, and the command that runs it alone:

<!-- output: troubleshooting/red -->
```text
mutastic: tests::shipping_is_free_from_the_threshold fails without any mutant, so mutastic can't judge one.
It panicked: assertion `left == right` failed: left: 5, right: 4
mutastic runs each test alone: a test that passes with the others, but fails alone, leans on one of them.
Its output is in fixtures/pricing/target/mutastic/tests.log. To run it alone:
  cargo test --manifest-path fixtures/pricing/Cargo.toml -p pricing --lib -- --exact tests::shipping_is_free_from_the_threshold
```

A test that passes in `cargo test` but fails here usually depends on another test having run
first, or on running in parallel with others: mutastic runs each one alone, in a process of its
own. Make it stand alone.

## No tests

A package with no tests has nothing to catch a mutant: every one is not covered, and the run says
`Found no tests` in place of a score below the threshold. mutastic runs what `cargo test` runs:
`#[test]` functions in the package's library, binaries and `tests/`. Doc tests don't count; see
[Doc tests](#doc-tests).

## A test is flaky

A test that failed, then passed when run again, is left out of the run: `! flaky: … failed, then
passed when run again; left out of this run`. Its catches don't count, so code only it reaches
shows as not covered. When mutastic sees what varies, it says: `it reaches SystemTime::now` (the
code the test runs) or `its own code reaches Instant::now` (the test itself). Making that the same
every run, with a seed or a fixed clock through the code's own seam, is usually the fix. Don't
remove the test, turn it off or loosen what it checks to get a run through.

## A kept id names another line, or nothing

A mutant's id comes from what it is and its place in its function, counted from the top among
identical changes, not from its line. So lines added or removed above it never move it. But when a
function has the same line more than once, four `i = i + 1;` say, deleting one of them moves the
ones below it up the count: each takes the ids of the one that was above it, and the last one's ids
are gone. An id kept in a script, a baseline or an ignore then names the next identical line, or
nothing, and `verify` says so for an id that's gone.

This is by design: an id that followed line numbers would change whenever anything above it moved.
After deleting one of several identical lines, run again to see their ids as they are now. See
[Mutant ids](concepts/ids.md#what-changes-an-id).

## Doc tests

Doc tests don't run under mutastic: it runs your test binaries, and rustdoc builds doc tests apart.
Code only a doc test calls shows as not covered. If that code matters, give it a unit test too.
