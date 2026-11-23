- **A package whose name a dependency also has, from crates.io, is built.** When a dev-dependency
  pulls the package itself from crates.io, as itoa's and serde_json's do through criterion and
  trybuild, Cargo has two packages of that name, and the bare `-p itoa` that mutastic built with
  was ambiguous: the run stopped with `build-failed` before any mutant ran. mutastic now names the
  package by its spec, `path+file:///…/itoa#itoa@1.0.19`, and so does a command it shows when the
  name alone would be ambiguous.
