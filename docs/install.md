# Installing mutastic

mutastic is one command, `mutastic`, for Rust, Maven and Gradle projects. It's a native program,
with nothing else to install for Rust. In a Maven or Gradle project, it fetches its Java edition the
first time it runs there: see [One command, two editions](editions.md).

## With the installer

On macOS or Linux:

```sh
curl -fsSL https://mutastic.io/install.sh | sh
```

On Windows, see [On Windows](#on-windows).

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

## On Windows

In PowerShell, 5.1 or 7:

```powershell
irm https://mutastic.io/install.ps1 | iex
```

That installs the latest release for Windows on x64 or Arm64. Run it again to update.

It downloads the release's zip for this machine, checks it against the release's `SHA256SUMS`, and
refuses one that doesn't match. It keeps the release in `%USERPROFILE%\.mutastic\native-<version>`,
copies `mutastic.exe` and `cargo-mutastic.exe` into `%USERPROFILE%\.mutastic\bin`, and adds that
folder to your user `Path`, so every terminal you open from then on finds them. The terminal you
ran it in finds them straight away; open a new one for the others.

It reads the same variables as the installer above, and `MUTASTIC_BIN_DIR` is
`%USERPROFILE%\.mutastic\bin` unless set. `MUTASTIC_NATIVE_RELEASES` may be a folder laid out as a
release is, with `MUTASTIC_NATIVE_VERSION` set. In a Maven or Gradle project, mutastic keeps its
Java edition in `%USERPROFILE%\.mutastic` too.

To uninstall, delete `%USERPROFILE%\.mutastic`, and take `%USERPROFILE%\.mutastic\bin` out of your
user `Path`: in Start, search for "environment variables", choose "Edit environment variables for
your account", then edit `Path`.

In Git Bash, MSYS2 or Cygwin, `curl -fsSL https://mutastic.io/install.sh | sh` installs the same
Windows release into the same folders, but puts `%USERPROFILE%\.mutastic\bin` on that shell's
`PATH` only: it prints the line to add. The PowerShell line puts it on every terminal's.

### With Scoop

Each release has a [Scoop](https://scoop.sh) manifest, `mutastic.json`, which Scoop installs
from directly:

```powershell
scoop install https://github.com/mutastic/mutastic-rs/releases/latest/download/mutastic.json
```

## With cargo-binstall

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) installs a release's build, on any
platform with one, instead of compiling it:

```sh
cargo binstall --git https://github.com/mutastic/mutastic-rs mutastic
```

That installs the same two commands into Cargo's `bin` directory.

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

On a platform without a release, or to work on mutastic itself, build it with a Rust toolchain.
Stable is fine:

```sh
git clone https://github.com/mutastic/mutastic-rs.git
cd mutastic-rs
cargo install --path crates/mutastic
```

That installs the same two commands into Cargo's `bin` directory. From a clone, `sh
scripts/install.sh` runs the installer itself.

## How a release is laid out

Each release, tagged `v<version>`, holds an archive for each target, named for it, `SHA256SUMS`,
and Scoop's manifest:

```text
mutastic-<version>-aarch64-apple-darwin.tar.gz
mutastic-<version>-x86_64-apple-darwin.tar.gz
mutastic-<version>-x86_64-unknown-linux-musl.tar.gz
mutastic-<version>-aarch64-unknown-linux-musl.tar.gz
mutastic-<version>-x86_64-pc-windows-msvc.zip
mutastic-<version>-aarch64-pc-windows-msvc.zip
SHA256SUMS
mutastic.json
```

Each archive holds one directory, `mutastic-<version>-<target>/`, with `mutastic` and
`cargo-mutastic` (`.exe` on Windows), the licence and the README. The Windows builds need nothing
else installed. The release workflow builds them from the tag, and installs the Windows builds with
both installers on Windows before the release is drafted.
