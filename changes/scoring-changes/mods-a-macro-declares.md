- **Modules a macro of the crate's declares are mutated.** A crate that declares its modules from
  a `macro_rules!` of its own, as serde and serde_core do with `crate_root!()`, had none of their
  files read: a run found no mutant, and passed with no score and no word of why. mutastic now
  follows a `mod name;` in the expansion of a macro the crate defines, in the file that calls it or
  in a `#[macro_use] mod` it declares, where the call is an item, and reads the file as rustc
  would. Their mutants are new, so a score can move either way.
