#!/bin/sh
# Installs the `mutastic` command, for Rust, Maven and Gradle projects alike:
#   curl -fsSL https://raw.githubusercontent.com/mutastic/mutastic-rs/main/scripts/install.sh | sh
# or, from a clone of mutastic's repository:
#   sh scripts/install.sh
#
# Downloads the native mutastic for this machine from a GitHub release into ~/.mutastic, checks it
# against the release's SHA256SUMS, and links `mutastic` and `cargo-mutastic` into ~/.local/bin.
# Run it again to update. In a Cargo project mutastic runs itself; the first time it runs in a Maven
# or Gradle project, it fetches its Java edition into ~/.mutastic too, which needs JDK 25. Nothing
# is installed outside those two directories, so removing them uninstalls it.
#
# Settings, each optional:
#   MUTASTIC_NATIVE_VERSION  the release of the native command to install; the latest unless set
#   MUTASTIC_HOME            where releases go, ~/.mutastic unless set
#   MUTASTIC_BIN_DIR         where the commands are linked, ~/.local/bin unless set
#   MUTASTIC_NATIVE_RELEASES where releases are downloaded from, for a mirror
#
# Everything is in functions, called on the last line, so a download cut short runs nothing.

set -eu

RELEASES=${MUTASTIC_NATIVE_RELEASES:-https://github.com/mutastic/mutastic-rs/releases}

say() {
  printf 'mutastic: %s\n' "$*"
}

fail() {
  printf 'mutastic: %s\n' "$*" >&2
  exit 1
}

need() {
  command -v "$1" > /dev/null 2>&1 || fail "the installer needs $1, which isn't on the PATH"
}

# The release target for this machine, as Rust names it.
target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os/$arch" in
    Darwin/arm64 | Darwin/aarch64) echo aarch64-apple-darwin ;;
    Darwin/x86_64) echo x86_64-apple-darwin ;;
    Linux/x86_64 | Linux/amd64) echo x86_64-unknown-linux-musl ;;
    Linux/aarch64 | Linux/arm64) echo aarch64-unknown-linux-musl ;;
    *) fail "there's no mutastic release for $os on $arch yet: build it from source, as the install page says" ;;
  esac
}

# The newest release's version, from where GitHub's "latest release" page redirects to.
latest() {
  url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$RELEASES/latest") ||
    fail "couldn't reach $RELEASES to find the latest release"
  version=${url##*/v}
  case "$version" in
    [0-9]*.[0-9]*.[0-9]*) printf '%s\n' "$version" ;;
    *) fail "couldn't tell the latest release from $url" ;;
  esac
}

# Checks the download against the release's SHA256SUMS: every release has one. sh has no local
# variables, so these are named apart from main's.
verify() {
  file=$1 listed=$2 sums=$3
  curl -fsSL -o "$sums" "$RELEASES/download/v$VERSION/SHA256SUMS" ||
    fail "couldn't download the release's SHA256SUMS: not installed"
  expected=$(grep " \*\{0,1\}$listed\$" "$sums" | cut -d ' ' -f 1)
  [ -n "$expected" ] || fail "SHA256SUMS doesn't list $listed: not installed"
  if command -v sha256sum > /dev/null 2>&1; then
    actual=$(sha256sum "$file" | cut -d ' ' -f 1)
  else
    actual=$(shasum -a 256 "$file" | cut -d ' ' -f 1)
  fi
  [ "$actual" = "$expected" ] || fail "the download doesn't match its checksum: not installed"
}

main() {
  need curl
  need tar
  TARGET=$(target)
  VERSION=${MUTASTIC_NATIVE_VERSION:-$(latest)}
  home=${MUTASTIC_HOME:-$HOME/.mutastic}
  bin=${MUTASTIC_BIN_DIR:-$HOME/.local/bin}
  name="mutastic-$VERSION-$TARGET"

  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  say "downloading $VERSION for $TARGET"
  curl -fsSL -o "$work/$name.tar.gz" "$RELEASES/download/v$VERSION/$name.tar.gz" ||
    fail "couldn't download $VERSION for $TARGET: is it a release? ($RELEASES)"
  verify "$work/$name.tar.gz" "$name.tar.gz" "$work/SHA256SUMS"

  tar -xzf "$work/$name.tar.gz" -C "$work"
  [ -x "$work/$name/mutastic" ] || fail "the download had no mutastic command in it"
  mkdir -p "$home" "$bin"
  rm -rf "$home/native-$VERSION"
  mv "$work/$name" "$home/native-$VERSION"
  ln -sf "$home/native-$VERSION/mutastic" "$bin/mutastic"
  ln -sf "$home/native-$VERSION/cargo-mutastic" "$bin/cargo-mutastic"

  say "installed $VERSION: $bin/mutastic"
  say "in a Maven or Gradle project, it fetches its Java edition the first time, which runs on JDK 25"
  case ":$PATH:" in
    *":$bin:"*) say "next, in a Cargo, Maven or Gradle project: mutastic run" ;;
    *)
      say "$bin isn't on your PATH: add it, then open a new terminal"
      path_line "$bin"
      say "then, in a Cargo, Maven or Gradle project: mutastic run"
      ;;
  esac
}

# The line that puts `bin` on the PATH, for the shell the person uses: zsh, as macOS starts with,
# reads ~/.zshrc; bash ~/.bashrc; fish has its own command.
path_line() {
  case "${SHELL##*/}" in
    zsh) printf "  echo 'export PATH=\"%s:\$PATH\"' >> ~/.zshrc\n" "$1" ;;
    bash) printf "  echo 'export PATH=\"%s:\$PATH\"' >> ~/.bashrc\n" "$1" ;;
    fish) printf '  fish_add_path %s\n' "$1" ;;
    *) printf '  export PATH="%s:$PATH"   (in your shell'"'"'s startup file)\n' "$1" ;;
  esac
}

main "$@"
