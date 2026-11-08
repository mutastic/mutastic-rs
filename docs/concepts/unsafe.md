# Unsafe code

A mutant is a bug put in on purpose. In safe Rust the worst a bug can do is panic, loop or give a
wrong answer, and a test can catch each of those. In `unsafe` code a bug can be undefined
behaviour: the program may crash, carry on with corrupted memory, or seem fine. A test can't be
said to catch or miss that, so mutastic treats code in and around `unsafe` by how close it is.

## Four tiers, from syntax alone

mutastic decides where code stands from the source, without type information:

| Where the code is | What mutastic does | Example in `fixtures/unsafety` |
|---|---|---|
| Inside an `unsafe` block, an `unsafe fn`, or a method of an `unsafe impl` | Leaves it alone | `raw::first`, an `unsafe fn` |
| A safe function whose body holds an `unsafe` block | Leaves it alone | `raw::halve` |
| A function called from inside an `unsafe` block anywhere in the crate | Mutates it, marked near unsafe | `raw::positive`, called in `halve`'s `unsafe` block |
| Anything else in a module that has unsafe code, or a method of a type that's `unsafe impl Send` or `Sync` | Mutates it, marked near unsafe | `raw::checked` |

Everything else is ordinary safe code, such as `plain::guarded`.

<!-- from: fixtures/unsafety/src/raw.rs -->
```rust
/// Called from inside an `unsafe` block: mutated, and marked.
pub fn positive(x: i32) -> bool {
    x > 0
}

/// Holds an `unsafe` block: left alone.
pub fn halve(x: i32) -> i32 {
    unsafe { std::hint::assert_unchecked(positive(x)) };
    x / 2
}

/// Unsafe itself: left alone.
///
/// # Safety
/// `values` isn't empty.
pub unsafe fn first(values: &[u8]) -> u8 {
    *values.get_unchecked(0) + 1
}
```

`mutastic mutants` shows what's made, which mutants are near unsafe and why, and how many were left
alone:

<!-- output: concepts/unsafe-mutants -->
```text
m2fcb19d0 src/raw.rs:5:5 mutastic/return-value returns true instead in raw::positive (near unsafe: called from unsafe code)
m71ecfa0f src/raw.rs:5:5 mutastic/return-value returns false instead in raw::positive (near unsafe: called from unsafe code)
m97f387f6 src/raw.rs:5:7 mutastic/boundary > becomes >= in raw::positive (near unsafe: called from unsafe code)
mbff60c78 src/raw.rs:5:5 mutastic/remove-conditional x > 0 is always true in raw::positive (near unsafe: called from unsafe code)
mcea3b45a src/raw.rs:5:5 mutastic/remove-conditional x > 0 is always false in raw::positive (near unsafe: called from unsafe code)
mf3e66daa src/raw.rs:27:5 mutastic/return-value returns 0 instead in raw::checked (near unsafe: in a module with unsafe code)
m68f8c8e3 src/raw.rs:24:10 mutastic/boundary < becomes <= in raw::checked (near unsafe: in a module with unsafe code)
m9ff28261 src/raw.rs:24:8 mutastic/remove-conditional x < 0 is always true in raw::checked (near unsafe: in a module with unsafe code)
mbb05698b src/raw.rs:24:8 mutastic/remove-conditional x < 0 is always false in raw::checked (near unsafe: in a module with unsafe code)
m8d677769 src/raw.rs:25:9 mutastic/remove-discarded-call std::process::abort(); is removed in raw::checked (near unsafe: in a module with unsafe code)
mad5f43d6 src/plain.rs:8:5 mutastic/return-value returns 0 instead in plain::guarded
mdb89abdf src/plain.rs:5:10 mutastic/boundary < becomes <= in plain::guarded
m418fd2e4 src/plain.rs:5:8 mutastic/remove-conditional x < 0 is always true in plain::guarded
m476f7baf src/plain.rs:5:8 mutastic/remove-conditional x < 0 is always false in plain::guarded
m3ec82c9d src/plain.rs:6:9 mutastic/remove-discarded-call std::process::abort(); is removed in plain::guarded
m92b6cfd0 src/plain.rs:13:5 mutastic/return-value returns 0 instead in plain::total
m24a65ba1 src/plain.rs:13:7 mutastic/math + becomes - in plain::total
17 mutants
not mutated: unsafe (4): --unsafe=enclosing or --unsafe=all includes them
```

## What's left alone, and how to include it

Code inside `unsafe`, and safe functions that hold an `unsafe` block, get no mutants by default.
The summary counts what was left out, so a run never looks more thorough than it was. This is the
end of `mutastic run fixtures/unsafety`:

<!-- output: concepts/unsafe-summary -->
```text
   Mutants  ✓ 5 caught · ✗ 7 missed · ○ 2 not covered (14) · 3 crashed where a crash proves nothing
  Left out  not mutated: unsafe (4), in or around unsafe blocks; --unsafe=enclosing or --unsafe=all includes them
   Crashes  1 caught only because the test process crashed, not by a test's check
     Build  one build of every mutant, 2 rounds (check and build)
 Test runs  21
  Duration  1.1s (setup 948ms, coverage 18ms, mutants 116ms)
     Tests  6 on rustc 1.94.0 · 4 workers
     Score  35.7%  · 41.7% where tests reach  threshold 80

  [FAIL]  Score below 80: 7 missed, 2 not covered.
 Next: mutastic verify <id> fixtures/unsafety · mutastic run --only=<id> fixtures/unsafety
```

A safe function with an `unsafe` block is left alone because it usually exists to uphold what the
block relies on: the bounds check before `get_unchecked`, the length before `set_len`. A mutant
there breaks the very thing that keeps the block sound.

- `--unsafe=enclosing` mutates those safe functions too, but still not the code inside the block.
  Their mutants are marked near unsafe.
- `--unsafe=all` mutates everything, the code inside `unsafe` included, marked the same way.

Use either when you want to know whether your tests pin down what your unsafe code relies on, and
read every missed mutant and every crash among them with care: see [Miri](#when-to-use-miri).

## Near unsafe: marked, and run on their own

A function called from unsafe code, or anything else in a module with unsafe code, is mutated, and
its mutants are marked with why (`near unsafe: called from unsafe code`). The module counts because
it's the boundary the Rustonomicon draws: safe code in the same module can break what the unsafe code
assumes, as a field's invariant.

Near-unsafe mutants run differently:

- **One test at a time.** A mutant that corrupts memory mustn't make another test fail and take the
  credit, so their tests run with `--test-threads=1`.
- **A crash proves nothing.** If the test process dies by a signal, the mutant may have caused
  undefined behaviour, so it isn't counted as caught. It's a `RuntimeError`, left out of the score,
  with the reason:

<!-- output: concepts/runtime-error-event -->
```text
{
  "schema": "mutastic.events/v1",
  "reason": "mutant-finished",
  "id": "m9ff28261",
  "fingerprint": "9ff2826156a229af",
  "status": "RuntimeError",
  "operator": "mutastic/remove-conditional",
  "location": {
    "file": "src/raw.rs",
    "line": 24,
    "column": 8,
    "class": "raw",
    "method": "checked"
  },
  "mutation": {
    "replacement": "true",
    "description": "`x < 0` is always true",
    "bytecode": "x < 0 -> true"
  },
  "statusReason": "crash: SIGABRT in a module with unsafe code, which may be undefined behaviour",
  "nearUnsafe": "in a module with unsafe code",
  "coveredBy": [
    "run::checks"
  ],
  "inputs": {
    "x": [
      3
    ]
  },
  "testsRun": 1,
  "durationMs": 70
}
```

- **A miss says to look further.** A missed near-unsafe mutant's card adds a note: the change no
  test noticed can break what the unsafe code relies on, so check that code with Miri too.

## When std stops a mutant

With debug assertions on, which mutastic always has (see [Profiles](profiles.md)), std checks some
unsafe preconditions itself and aborts with `unsafe precondition(s) violated`. In the fixture,
`positive` turned to always return `false` makes `halve` call `assert_unchecked(false)`, and std
stops it. That's std's catch, not a test's, so it's a `RuntimeError` too, wherever the mutant is:

<!-- output: concepts/precondition-event -->
```text
{
  "schema": "mutastic.events/v1",
  "reason": "mutant-finished",
  "id": "m71ecfa0f",
  "fingerprint": "71ecfa0fc15036f8",
  "status": "RuntimeError",
  "operator": "mutastic/return-value",
  "location": {
    "file": "src/raw.rs",
    "line": 5,
    "column": 5,
    "class": "raw",
    "method": "positive"
  },
  "mutation": {
    "replacement": "false",
    "description": "returns `false` instead",
    "bytecode": "x > 0 -> false"
  },
  "statusReason": "crash: std's debug check of an unsafe precondition stopped it (SIGABRT), not a test",
  "nearUnsafe": "called from unsafe code",
  "coveredBy": [
    "run::halves"
  ],
  "inputs": {
    "x": [
      4
    ]
  },
  "testsRun": 1,
  "durationMs": 46
}
```

## A crash in safe code

Safe code can crash too: `std::process::abort()`, a panic that can't unwind, a stack overflow.
There the crash is the mutant's doing and nothing else's, so the mutant is caught, but by the
crash, not by a test's check. No test is named, and the result says so:

<!-- output: concepts/verify-crash -->
```text
mutastic: m418fd2e4 is caught only because the test process crashed (SIGABRT): no test's check caught it
```

See [Results](results.md#caught-only-because-the-test-process-crashed) for what to do with one.

## When to use Miri

[Miri](https://github.com/rust-lang/miri) runs your tests in an interpreter that checks for
undefined behaviour. It needs a nightly toolchain and runs far slower than a normal build, so
mutastic doesn't run it for you; use it on the code mutastic points you to:

- **A missed mutant near unsafe code.** Write the test the card asks for, then run it under Miri, so
  the value the unsafe code relies on is both checked and sound.
- **A `RuntimeError`.** The mutant crashed where a crash may be undefined behaviour. The tests that
  reach it are the ones to run under Miri, on your code as it is: a crash there that a small change
  provokes is often a precondition the unsafe code doesn't check.
- **After `--unsafe=enclosing` or `--unsafe=all`.** A missed mutant inside unsafe code is one no
  test noticed, but it may also be undefined behaviour that happened not to show. Miri tells the
  two apart.

To run a package's tests under Miri:

```sh
rustup toolchain install nightly --component miri
cargo +nightly miri test
```

`cargo +nightly miri test <name>` runs only the tests whose names contain `<name>`, which is how to
run just the ones a card lists as covering the mutant.

## What mutastic never mutates

Some places can't hold a switch at all, or shouldn't: `const fn` bodies, `const` and `static`
initialisers, array lengths and const generic arguments, patterns, `#[naked]` functions, and the
tokens of a macro call. These aren't counted as left out: there's nothing a test could catch
there that a run could switch on.
