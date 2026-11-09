# Installing mutastic

mutastic is one command, `mutastic`, for Rust, Maven and Gradle projects. It's a native program
with nothing else to install for Rust. In a Maven or Gradle project, it fetches its Java edition the
first time it runs there: see [One command, two editions](editions.md).

## With the installer

The installer, `scripts/install.sh` in mutastic's repository, installs a published release:

```sh
sh scripts/install.sh
```

It finds this machine's target, downloads that release's archive, checks it against the release's
`SHA256SUMS`, and refuses one that doesn't match. It keeps the release in
`~/.mutastic/native-<version>`, and links `mutastic` and `cargo-mutastic` into `~/.local/bin`. Run
it again to update. If `~/.local/bin` isn't on your `PATH`, it says the line to add for your shell.
Nothing goes anywhere else, so removing those two directories uninstalls it.

Releases are built for macOS on Apple silicon and Intel, and for Linux on x86-64 and Arm64, static,
so one archive runs on any distribution. Elsewhere, or before a release is out, build it from
source.

| Variable | Effect |
|---|---|
| `MUTASTIC_NATIVE_VERSION` | The release to install, such as `0.1.0`: the latest unless set |
| `MUTASTIC_HOME` | Where releases are kept: `~/.mutastic` unless set |
| `MUTASTIC_BIN_DIR` | Where the commands are linked: `~/.local/bin` unless set |
| `MUTASTIC_NATIVE_RELEASES` | Where releases are downloaded from, for a mirror: GitHub's releases of `mutastic/mutastic-rs` unless set |

An earlier installer linked `~/.local/bin/mutastic` to the Java edition. This one replaces that
link, and keeps the Java edition's releases in `~/.mutastic`, where `mutastic` finds and uses them.

## From source

With a Rust toolchain, stable is fine:

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
cargo install --path crates/mutastic
```

That installs the same two commands into Cargo's `bin` directory.

## Check it

`mutastic --version` says which version you have, and each edition's. Before the Java edition is
fetched:

<!-- output: reference/version -->
```text
mutastic 0.1.0
Rust edition 0.1.0, built in
Java edition: not fetched yet, which happens the first time mutastic runs in a Maven or Gradle project
```

## How a release is laid out

Each release, tagged `v<version>`, holds an archive for each target, named for it, and
`SHA256SUMS`:

```text
mutastic-<version>-aarch64-apple-darwin.tar.gz
mutastic-<version>-x86_64-apple-darwin.tar.gz
mutastic-<version>-x86_64-unknown-linux-musl.tar.gz
mutastic-<version>-aarch64-unknown-linux-musl.tar.gz
SHA256SUMS
```

Each archive holds one directory, `mutastic-<version>-<target>/`, with `mutastic`,
`cargo-mutastic`, the licence and the README. The release workflow builds them from the tag.

Next: [Getting started](getting-started.md) runs mutastic on a small crate, and closes the gap it
finds.
