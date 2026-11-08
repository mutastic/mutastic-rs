# Mutant ids

Every mutant has an id, `m` and eight hex digits, such as `mb2f8ae57`. It's how you name a mutant to
mutastic: `mutastic verify mb2f8ae57 fixtures/pricing` checks that one, and `mutastic run
--only=mb2f8ae57 fixtures/pricing` runs only it. Scripts, CI jobs and coding agents keep ids, so an
id has to keep naming the same mutant while you work.

## What an id is made of

An id is the start of a SHA-256 over what the mutant is:

- the crate's name;
- the path of the function it's in, such as `pricing::Pricing::discount`;
- the operator, such as `mutastic/boundary`;
- the code it changes, and what it changes it to, with each run of whitespace and comments
  squeezed to one space;
- and, when the same change can be made more than once in that function, which one it is, counted
  from the top.

There's no line number and no file name in it. The `fingerprint` in `--format=jsonl` is the same
hash, longer.

## What keeps an id

- **Code moving.** Lines added or removed above it, or the function moved within its file.
- **Line breaks and comments.** A long condition broken over two lines, a comment added inside
  it, or more spaces where there were some: each run of them counts as one space. Spaces added
  where there were none (`a<b` to `a < b`) are a change, though `rustfmt`'d code rarely has any to
  add.
- **Other changes in the function,** unless they add or remove the same change earlier in it.
- **Other operators.** An id doesn't depend on which operators a run uses, or which mutants it
  shows: an id shown by an `--operators=extended` run works with `verify` on its own, without
  naming the set again, and so does one from inside unsafe code.
- **A new version of mutastic.** Ids are a promise: the Rust edition's tests hold every mutant of
  every fixture to a kept list, and fail if an id goes or comes to name another mutant. A new
  operator only adds ids.

## What changes an id

- **The change itself.** Edit the line so the code at the site differs (`total >= 100` to
  `total >= 120`), and its mutants are new ones, with new ids.
- **The function's path.** Renaming the function, moving it to another module, or renaming the
  type of an `impl` it's in.
- **The crate's name.**
- **The same change made earlier in the function.** A second `total >= 100` added above the first
  takes the first one's place in the count: the new one gets the old one's ids, and the old one
  gets new ones.

When an id you kept stops working, `verify` says so, and names the command that shows the mutants
as they are now:

<!-- output: concepts/unknown-id -->
```text
mutastic 0.0.1 · reading the Cargo package…
mutastic: m00000000 isn't a mutant of pricing: run `mutastic run fixtures/pricing` to see its mutants
```

## The Java edition's ids

The two editions make ids the same way, from what the mutant is, and each hashes its own name in
first, so a Rust id never names a Java mutant, or the other way round. The operator ids are shared:
`mutastic/boundary` means the same change in both. See [Operators](../operators.md).
