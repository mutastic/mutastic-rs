- **The contract checks read this edition's binding.** The editions' contract now gives each
  edition a binding block (its selector, error names, arid reasons and examples, capture masks)
  and names on a difference only the editions that deviate. `tests/contract.rs` reads the rules
  this edition has from `editions.rust`, and counts every slot (`each`) and each deviation naming
  `rust` as its own. `scripts/pin-contract.sh` runs the contract's shared tooling, fetched from
  mutastic, and `scripts/release-check.sh` refuses a release while a pending entry for Rust has no
  issue or target, or a target gone by. Nothing this edition does has changed.
