- **`library/swap-ends`, in `stronger`: one end of a collection for the other.** `first` and
  `last`, `front` and `back` (and their `_mut` forms), and a `BTreeMap`'s and a `BTreeSet`'s
  `first_key_value` and `last_key_value`, `first_entry` and `last_entry`, and `pop_first` and
  `pop_last` swap. It finds the newest taken where the oldest was meant, which tests with one
  element can't see. An iterator's `last` is left alone, as it has no `first`. The Java edition has
  it too, for `getFirst` and `getLast` and a `Deque`'s and a sorted collection's ends. See [Swap
  ends](docs/operators/library.md#swap-ends).
