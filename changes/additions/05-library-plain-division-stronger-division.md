- **`library/plain-division`, in `stronger`: division that rounds towards zero.** `a.div_euclid(b)`
  becomes `a / b` and `rem_euclid` becomes `%`, so a remainder that's wrong for negative numbers,
  `-1 % 3` being `-1` where `rem_euclid` gives `2`, shows unless a test has one. A value the source
  proves unsigned is skipped as equivalent. The Java edition has it too, for `Math.floorDiv` and
  `floorMod`. See [Plain division](docs/operators/arithmetic.md#plain-division).
