- **A thread's name, its stack size and how long a runtime's idle threads live are skipped as
  arid.** None of them changes whether, when or how often work runs. A name given to
  `thread::Builder`'s `name`, or to a tokio runtime builder's `thread_name` or `thread_name_fn`, is
  `arid: only names a thread`; a stack size, `stack_size` or `thread_stack_size`, is `arid: only
  sizes a thread's stack` while the source shows it stays above zero; and tokio's
  `thread_keep_alive` is `arid: only sets how long idle threads live` on the same terms. The call
  is known by the builder its chain starts at, so any other `.name(..)` is mutated as before. A
  crate that reads names (`thread::current().name()`) keeps its name mutants. A spawn, a `join`
  and `worker_threads` still run. Runs on code that sets threads up get fewer mutants, and scores
  can go up; a `mutate:` rule brings them back. The Java edition skips them too, and a priority.
