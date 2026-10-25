#!/bin/sh
# Install the rsi-ralph binary on macOS or Linux from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version v0.4.0 --dir /usr/local/bin
#
# It picks the archive for this machine, checks it against the release's
# .sha256 file, and copies `rsi-ralph` into the install folder. Nothing else on
# the system is changed: if the folder is not on PATH, it says so and stops.
#
# Environment (each flag wins over its variable):
#   RSI_RALPH_VERSION        release tag to install, such as v0.4.0 (default: latest)
#   RSI_RALPH_INSTALL_DIR    install folder (default: ~/.local/bin)
#   RSI_RALPH_TARGET         Rust target triple, overriding detection
#   RSI_RALPH_DOWNLOAD_BASE  releases URL, for mirrors and tests
#                            (default: https://github.com/tinyhumansai/rsi-ralph/releases)

set -eu

version="${RSI_RALPH_VERSION:-latest}"
install_dir="${RSI_RALPH_INSTALL_DIR:-${HOME:-}/.local/bin}"
target="${RSI_RALPH_TARGET:-}"
base="${RSI_RALPH_DOWNLOAD_BASE:-https://github.com/tinyhumansai/rsi-ralph/releases}"
dry_run=0

say() { printf '%s\n' "rsi-ralph-install: $*"; }
fail() {
  printf '%s\n' "rsi-ralph-install: error: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Install the rsi-ralph binary from a GitHub release.

Usage: install.sh [--version TAG] [--dir FOLDER] [--target TRIPLE] [--dry-run]

  --version TAG     release to install, such as v0.4.0 (default: latest)
  --dir FOLDER      where to put rsi-ralph (default: ~/.local/bin)
  --target TRIPLE   Rust target triple instead of the detected one
  --dry-run         print what would be downloaded and installed, then stop
  -h, --help        show this help
EOF
}

value() {
  [ "$#" -ge 2 ] && [ -n "$2" ] || fail "$1 needs a value"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) value "$@"; version="$2"; shift 2 ;;
    --dir) value "$@"; install_dir="$2"; shift 2 ;;
    --target) value "$@"; target="$2"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; fail "unknown option $1" ;;
  esac
done

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) fail "no prebuilt binary for $arch; build from source with cargo install" ;;
  esac
  # A shell under Rosetta reports x86_64 on Apple silicon; prefer the native build.
  if [ "$os" = Darwin ] && [ "$arch" = x86_64 ] &&
    [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
    arch=aarch64
  fi
  case "$os" in
    Linux) printf '%s\n' "$arch-unknown-linux-gnu" ;;
    Darwin) printf '%s\n' "$arch-apple-darwin" ;;
    MINGW* | MSYS* | CYGWIN*)
      fail "on Windows run: irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1 | iex" ;;
    *) fail "no prebuilt binary for $os; build from source with cargo install" ;;
  esac
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    fail "need sha256sum or shasum to verify the download"
  fi
}

[ -n "$target" ] || target="$(detect_target)"
[ -n "$install_dir" ] || fail "set --dir or HOME"
archive="rsi-ralph-$target.tar.gz"
if [ "$version" = latest ]; then
  url="$base/latest/download/$archive"
else
  url="$base/download/$version/$archive"
fi

say "target  $target"
say "release $version"
say "from    $url"
say "into    $install_dir"
[ "$dry_run" -eq 0 ] || exit 0

command -v curl >/dev/null 2>&1 || fail "need curl"
command -v tar >/dev/null 2>&1 || fail "need tar"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT INT TERM

curl -fsSL --retry 3 -o "$work/$archive" "$url" ||
  fail "could not download $url (is there a $target build in that release?)"
curl -fsSL --retry 3 -o "$work/$archive.sha256" "$url.sha256" ||
  fail "could not download $url.sha256"

expected="$(cut -d ' ' -f 1 <"$work/$archive.sha256")"
actual="$(sha256 "$work/$archive")"
[ -n "$expected" ] && [ "$expected" = "$actual" ] ||
  fail "checksum mismatch for $archive: expected $expected, got $actual"

tar -xzf "$work/$archive" -C "$work"
binary="$work/rsi-ralph-$target/rsi-ralph"
[ -f "$binary" ] || fail "$archive holds no rsi-ralph-$target/rsi-ralph"

mkdir -p "$install_dir"
cp "$binary" "$install_dir/rsi-ralph.tmp"
chmod 755 "$install_dir/rsi-ralph.tmp"
mv "$install_dir/rsi-ralph.tmp" "$install_dir/rsi-ralph"
say "installed $("$install_dir/rsi-ralph" --version 2>/dev/null || echo rsi-ralph) to $install_dir/rsi-ralph"

case ":${PATH:-}:" in
  *":$install_dir:"*) say "next: rsi-ralph doctor" ;;
  *)
    say "$install_dir is not on PATH; add this to your shell profile:"
    say "  export PATH=\"$install_dir:\$PATH\""
    ;;
esac
