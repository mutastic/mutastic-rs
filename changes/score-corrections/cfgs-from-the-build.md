- **Code the build doesn't compile is not built here, whatever its `cfg`.** mutastic judged a `cfg`
  from its text, and took any name it didn't know as on: code under `cfg(docsrs)`, `cfg(loom)`, a
  build script's `cfg` it didn't set, or `cfg(not(..))` of one it did, was mutated, and counted as
  not covered though it never compiles, so anyhow scored 60.4% where 72.5% was right. Now the build
  says: as it compiles your package, mutastic asks the same rustc which `cfg`s it has, the features,
  the build script's `cargo::rustc-cfg` and the `--cfg`s from `RUSTFLAGS` and `.cargo/config.toml`
  included, and a name nothing set is off, as rustc has it. Only mutants that could never run go,
  so a score can only rise. See `docs/troubleshooting.md`.
