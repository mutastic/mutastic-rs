#!/bin/sh
# Installs the `mutastic` command, for Rust, Maven and Gradle projects alike:
#   curl -fsSL https://mutastic.io/install.sh | sh
# or, from a clone of mutastic's repository:
#   sh scripts/install.sh
#
# Downloads the native mutastic for this machine from a GitHub release into ~/.mutastic, checks it
# against the release's SHA256SUMS, and links `mutastic` and `cargo-mutastic` into ~/.local/bin.
# Run it again to update. In a Cargo project mutastic runs itself; the first time it runs in a Maven
# or Gradle project, it fetches its Java edition into ~/.mutastic too, which needs JDK 25. Nothing
# is installed outside those two directories, so removing them uninstalls it.
#
# In Git Bash, MSYS2 or Cygwin, which run on Windows, it installs the Windows build, as
# scripts/install.ps1 does from PowerShell: the same release and folders, so either updates the
# other's install. The commands are copied into ~/.mutastic/bin, as Windows has no links every
# program follows.
#
# Settings, each optional:
#   MUTASTIC_NATIVE_VERSION  the release of the native command to install; the latest unless set
#   MUTASTIC_HOME            where releases go, ~/.mutastic unless set
#   MUTASTIC_BIN_DIR         where the commands are linked, ~/.local/bin (on Windows, the bin
#                            folder in MUTASTIC_HOME) unless set
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

# Whether this is Windows underneath: Git Bash and MSYS2 say MINGW64_NT-10.0 or MSYS_NT-10.0, and
# Cygwin CYGWIN_NT-10.0.
windows() {
  case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*) return 0 ;;
    *) return 1 ;;
  esac
}

# The release target for this machine, as Rust names it. On Windows, the processor is Windows' own
# word for it: a shell built for x86-64 runs on Arm64 too, and says x86_64.
target() {
  if windows; then
    case "${PROCESSOR_ARCHITEW6432:-${PROCESSOR_ARCHITECTURE:-}}" in
      ARM64) echo aarch64-pc-windows-msvc ;;
      AMD64) echo x86_64-pc-windows-msvc ;;
      *) fail "there's no mutastic release for Windows on ${PROCESSOR_ARCHITECTURE:-this processor}: build it from source, as the install page says" ;;
    esac
    return
  fi
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

# Unpacks a Windows release's zip into a directory: with unzip, as Git Bash has, or else with
# PowerShell, which every Windows has.
unzip_into() {
  if command -v unzip > /dev/null 2>&1; then
    unzip -q "$1" -d "$2"
  else
    need powershell.exe
    powershell.exe -NoProfile -Command "Expand-Archive -LiteralPath '$(windows_path "$1")' -DestinationPath '$(windows_path "$2")'" ||
      fail "couldn't unpack $1"
  fi
}

# A path as Windows programs take it, C:\Users\...: cygpath is in Git Bash, MSYS2 and Cygwin.
windows_path() {
  cygpath -w "$1"
}

# The person's home: on Windows, Windows' own, where mutastic keeps its Java edition too (Cygwin's
# HOME is a folder of its own).
user_home() {
  if windows && [ -n "${USERPROFILE:-}" ]; then
    cygpath -u "$USERPROFILE"
  else
    printf '%s\n' "$HOME"
  fi
}

main() {
  need curl
  TARGET=$(target)
  VERSION=${MUTASTIC_NATIVE_VERSION:-$(latest)}
  home=${MUTASTIC_HOME:-$(user_home)/.mutastic}
  name="mutastic-$VERSION-$TARGET"
  if windows; then
    bin=${MUTASTIC_BIN_DIR:-$home/bin}
    archive="$name.zip" exe=.exe
  else
    need tar
    bin=${MUTASTIC_BIN_DIR:-$HOME/.local/bin}
    archive="$name.tar.gz" exe=
  fi

  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  say "downloading $VERSION for $TARGET"
  curl -fsSL -o "$work/$archive" "$RELEASES/download/v$VERSION/$archive" ||
    fail "couldn't download $VERSION for $TARGET: is it a release? ($RELEASES)"
  verify "$work/$archive" "$archive" "$work/SHA256SUMS"

  if windows; then
    unzip_into "$work/$archive" "$work"
  else
    tar -xzf "$work/$archive" -C "$work"
  fi
  [ -x "$work/$name/mutastic$exe" ] || fail "the download had no mutastic command in it"
  mkdir -p "$home" "$bin"
  rm -rf "$home/native-$VERSION"
  mv "$work/$name" "$home/native-$VERSION"
  for command in mutastic cargo-mutastic; do
    if windows; then
      cp -f "$home/native-$VERSION/$command.exe" "$bin/$command.exe" ||
        fail "couldn't copy $command.exe into $bin: is it running?"
    else
      ln -sf "$home/native-$VERSION/$command" "$bin/$command"
    fi
  done

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
  if windows; then
    say "for PowerShell and cmd as well, this puts it on your Windows PATH:"
    say "  powershell -c \"irm https://mutastic.io/install.ps1 | iex\""
  fi
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
