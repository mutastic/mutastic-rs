# Installing mutastic

mutastic is one command, `mutastic`, for Rust, Maven and Gradle projects. It's a native program,
with nothing else to install for Rust. In a Maven or Gradle project, it fetches its Java edition the
first time it runs there: see [One command, two editions](editions.md).

## With the installer

```sh
curl -fsSL https://mutastic.io/install.sh | sh
```

That installs the latest release for this machine: macOS on Apple silicon or Intel, or Linux on
x86-64 or Arm64. The Linux builds are static, so one archive runs on any distribution. It needs
`curl` and `tar`. Run it again to update.

It downloads the release's archive for this machine, checks it against the release's
`SHA256SUMS`, and refuses one that doesn't match. It keeps the release in
`~/.mutastic/native-<version>` and links `mutastic` and `cargo-mutastic` into `~/.local/bin`. If
`~/.local/bin` isn't on your `PATH`, it prints the line to add for your shell. It writes nowhere
else, so removing those two directories uninstalls it.

If an earlier installer linked `~/.local/bin/mutastic` to the Java edition, this one replaces that
link. The Java edition's releases stay in `~/.mutastic`, where `mutastic` finds and uses them.

These variables change what it does:

| Variable | Effect |
|---|---|
| `MUTASTIC_NATIVE_VERSION` | The release to install, such as `0.2.1`: the latest unless set |
| `MUTASTIC_HOME` | Where releases are kept: `~/.mutastic` unless set |
| `MUTASTIC_BIN_DIR` | Where the commands are linked: `~/.local/bin` unless set |
| `MUTASTIC_NATIVE_RELEASES` | Where releases are downloaded from, for a mirror: GitHub's releases of `mutastic/mutastic-rs` unless set |

## Check it

`mutastic --version` says which version you have, and each edition's. Before the Java edition is
fetched:

<!-- output: reference/version -->
```text
mutastic 0.2.1
Rust edition 0.2.1, built in
Java edition: not fetched yet, which happens the first time mutastic runs in a Maven or Gradle project
```

Next: [Getting started](getting-started.md) runs mutastic on a small crate, and closes the gap it
finds.

## From source

There's no release for Windows yet. There, use WSL, where the installer works as on Linux, or build
mutastic with a Rust toolchain. Stable is fine. The same applies to any other platform without a
release, and to working on mutastic itself:

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
cargo install --path crates/mutastic
```

That installs the same two commands into Cargo's `bin` directory. From a clone, `sh
scripts/install.sh` runs the installer itself.

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
