- **A removed release's card says what to look at.** A removed `remove_file` or `remove_dir_all`
  asks for a test that the path is gone, and a statement removed from a `Drop` impl's `drop` for a
  test that lets a value go and checks what that changes. A removed `drop(x)` says that `x` still
  goes at the end of its scope, so the mutant matters only where code after it takes the lock
  again or waits on a thread that does. Each card's test is run against its mutant. The Java
  edition's cards do the same for `close()`, a pool's `shutdown()`, a shutdown hook and
  `clean()`. See [Calls](https://docs.mutastic.io/rust/operators/calls/).
