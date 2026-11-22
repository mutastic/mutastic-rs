- **A `Display` impl is counted, and `new` has its body replaced.** A `fmt`'s `Ok(())`, and with
  `extended` a `fmt`'s or a `hash`'s or a `new`'s body replaced, had no mutant. Outside a `Debug`
  or `Hash` impl they're mutants now: what a `Display` writes is often what a person or a program
  reads. New ids appear; none moves.
