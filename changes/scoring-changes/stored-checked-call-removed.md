- **A call stored and then only checked is removed: `let removed = remove_file(&p);
  removed.unwrap();` is a mutant.** The check was skipped as arid, and the call that made the value
  got no mutant at all, though `remove_file(&p).unwrap();` written in one statement has one. Now
  both statements go, by `mutastic/remove-discarded-call`, when the check comes straight after and
  nothing else uses the value. `x.unwrap();` on a value it's given stays skipped. New ids appear;
  none moves. See [Remove discarded call](docs/operators/calls.md#remove-discarded-call).
