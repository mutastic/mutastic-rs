# Suggested tests

Each missed mutant shows the whole numbers its covering tests passed to its function (`inputs seen:
total = 50, 250`), so what they never tried is plain. For many, mutastic also writes the test that
would catch it, found by running your code:

<!-- output: concepts/suggested-test -->
```text
  ✗ MISSED  no test checks late_fee() at days_late == 7
   ┌─ src/lib.rs:7
   │
 6 │     pub fn late_fee(days_late: i64) -> i64 {
 7 │         if days_late <= 7 {
   │                      ^^ <= changed to <
 8 │             return 0;
   │
   = mutant  - if days_late <= 7 {
             + if days_late < 7 {
   = note: every test calls late_fee() away from the boundary
           inputs seen: 2, 20 (none is 7)
           covered by rates::tests::late_fees_accrue_after_a_week (its 1 covering test still passes)
   = help: test late_fee() with days_late = 7 and assert the result

           // in mod rates::tests
           #[test]
           fn late_fee_at_the_days_late_edge() {
               assert_eq!(late_fee(7), 0); // today 0 · with this mutant 5
           }
           check today's value is the one you intend: the test pins what the code does now

   done when  mutastic verify m416bb0e1 fixtures/suggestions passes
   reproduce  mutastic run --only=m416bb0e1 fixtures/suggestions
```

## How it's found

mutastic calls the function with candidate inputs, first as it is and then with the mutant in, and
keeps the first input where the two give different results. The constants on the changed line, and
one either side of each, come first, then the values the tests passed, then a few small numbers. The
value the test asserts is what the call gave today, and the comment says what it gave with the
mutant: a value, `panics`, or `doesn't return`. Neither is guessed. Both come from running the code.

The calls run in mutastic's copy of your crate, in a test it builds beside the function, in a
process of its own with a time limit. `--no-suggestions` turns the search off, so nothing of yours
runs outside your tests.

## Which functions get one

A suggestion is written only where mutastic can be sure it compiles and means what it says:

- **The call:** a free function, an associated function, or a method of an inherent `impl`, whose
  parameters are all whole numbers (`i32`, `u64`, `usize` and the rest).
- **The receiver,** for a method: made with whole numbers too, by the type's own `new`, or, when
  the type has no `new`, a struct literal of its fields, which must all be whole numbers.
- **The result:** something a test can write as a literal: a whole number, a `bool`, a `char`, a
  `String` or a `&str`.

Generic functions, trait methods, `impl Trait` and `dyn` parameters, `async` and `unsafe`
functions, and code near unsafe code get none: a guess would be worse than nothing. Nor does a
function that answers the same call two ways, because it reads the clock, a random number or state
another call changed. Each call is made twice, and any difference means no suggestion.

## Where it goes

The test goes in the test module beside the function, `mod tests` with `use super::*;` as `cargo
new` writes it, with a number added to its name if the module already has a test by that name.
The name says where the input sits, not its value, which goes stale when the spec moves: the number
stays in the call. For a call of one argument, a zero or negative input says so,
`fee_when_amount_is_zero`; otherwise, at the edge of the literal the condition compares it with,
or one either side, by the parameter's name: `discount_at_the_total_edge` for 100 in
`total >= 100`, `discount_just_below_the_total_edge` for 99. Anything else is named for its
arguments, `postage_with_kilos_3`, as is a call of more than one. When the module has no `use super::*;`, the call is written
with `super::`. When there's no test module, the card says where a new one goes, and the call is
written with `super::` too.

A value the source names is said its way, and names the test. A `const` in the function's file
that the comparison reads (whose value is among the inputs tried) is passed by name, its value in
the comment:

```text
#[test]
fn discount_at_free_from() {
    assert_eq!(discount(FREE_FROM), 10); // FREE_FROM = 100 · today 10 · with this mutant 0
}
```

A field the receiver is made with, of the parameter's type and passed the same value, is one
`let`, passed to both (`is_free_at_free_from`). Otherwise a receiver's values are named by its
fields or `new`'s parameters: `open_on_with_day_5_for_start_5_and_end_10`.

The test pins what the code does today. Check that's what it should do before you keep it: when
today's value is wrong, the test is still the one to write, with the value it should be.

`--format=agent` gives the same test under `test to add in`, and `--format=jsonl` gives it as the
event's `suggestedTest`: where it goes, its name, its code, the call, and both values.
