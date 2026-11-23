- **A rules file can ask for the Java edition's other rewrites.** Beside `remove` and `rename`, an
  operator's `rewrite:` takes `keep argument` (`set.unwrap_or(8080)` becomes `8080`), `unchecked
  arithmetic` (`a.checked_add(b)` becomes `Some(a.wrapping_add(b))`, and a `saturating_` method the
  `wrapping_` one) and `call name(TYPES) with A, B` (`call take(usize) with usize::MAX`), whose
  arguments are constants and whose types are for the reader. `omit last argument` stays an error,
  now with its reason: it calls a shorter overload, and Rust has none. See
  [Rewrites](docs/reference/configuration.md#rewrites).
