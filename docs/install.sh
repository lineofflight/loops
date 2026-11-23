#!/bin/sh
# Install loops from its GitHub releases, for macOS or Linux:
#
#   curl -fsSL https://lineofflight.dev/loops/install.sh | sh
#
# LOOPS_VERSION           the release, like v0.1.0 (default: the latest)
# LOOPS_INSTALL_DIR       where it goes (default: ${XDG_BIN_HOME:-$HOME/.local/bin})
# LOOPS_RELEASE_BASE_URL  where the releases are, for tests
#                         (default: https://github.com/lineofflight/loops/releases/download)
set -eu

fail() {
  echo "loops install: $*" >&2
  exit 1
}

# Everything runs from here, so a download cut short runs nothing.
main() {
  repo=lineofflight/loops
  base=${LOOPS_RELEASE_BASE_URL:-https://github.com/$repo/releases/download}
  dir=${LOOPS_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}

  case $(uname -s) in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-musl ;;
    *) fail "no release for $(uname -s)" ;;
  esac
  case $(uname -m) in
    arm64 | aarch64) arch=aarch64 ;;
    x86_64 | amd64) arch=x86_64 ;;
    *) fail "no release for $(uname -m)" ;;
  esac

  version=${LOOPS_VERSION:-}
  if [ -z "$version" ]; then
    version=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" |
      sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
    [ -n "$version" ] || fail "could not find the latest release"
  fi
  case $version in
    v*) ;;
    *) version=v$version ;;
  esac

  file=loops-$version-$arch-$os.tar.gz
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/loops-install.XXXXXX")
  staged=
  trap 'rm -rf "$tmp"; [ -z "$staged" ] || rm -f "$staged"' EXIT
  trap 'exit 130' INT TERM

  curl -fsSL "$base/$version/$file" -o "$tmp/$file" ||
    fail "could not download $base/$version/$file"
  curl -fsSL "$base/$version/SHA256SUMS" -o "$tmp/SHA256SUMS" ||
    fail "could not download $base/$version/SHA256SUMS"

  expected=$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1; exit }' "$tmp/SHA256SUMS")
  [ -n "$expected" ] || fail "no checksum for $file"
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$file" | cut -d ' ' -f 1)
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$file" | cut -d ' ' -f 1)
  else
    fail "found neither sha256sum nor shasum to check the download"
  fi
  [ "$actual" = "$expected" ] || fail "checksum mismatch for $file"

  tar -xzf "$tmp/$file" -C "$tmp" loops
  mkdir -p "$dir"
  staged=$dir/.loops.$$
  cp "$tmp/loops" "$staged"
  chmod 755 "$staged"
  mv -f "$staged" "$dir/loops"
  staged=

  echo "Installed loops $version to $dir/loops"
  case :$PATH: in
    *:"$dir":*) ;;
    *)
      echo "$dir is not on your PATH. Add it to your shell's profile:"
      echo "  export PATH=\"$dir:\$PATH\""
      ;;
  esac
  echo
  echo "Next, add it to Claude Code:"
  echo "  claude mcp add loops -- loops mcp"
  echo "Other agents take the same command: loops mcp"
}

main "$@"
