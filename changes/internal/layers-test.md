- **Each module keeps to its layer, and a test holds it there.** ARCHITECTURE.md's Layers puts
  every module of `crates/mutastic` in one of six layers, and `tests/layers.rs` fails on a
  `crate::` path that reaches a layer above its own. `engine` and `compile` no longer reach into
  `report`: the helpers they used for paths, shell words, counts and durations moved below it.
