#!/bin/sh
# Render the Homebrew formula for a release from packaging/homebrew/loops.rb
# to stdout, with the checksums of its four tarballs.
#
# Usage: scripts/render-formula.sh <tag> <SHA256SUMS>
set -eu

fail() {
  echo "render-formula: $*" >&2
  exit 1
}

[ $# -eq 2 ] || fail "usage: render-formula.sh <tag> <SHA256SUMS>"
tag=$1
sums=$2
case $tag in
  *[!A-Za-z0-9.+-]*) fail "not a release tag: $tag" ;;
  v[0-9]*) ;;
  *) fail "not a release tag: $tag" ;;
esac
[ -f "$sums" ] || fail "no such file: $sums"

# The checksum of a target's tarball, in either of sha256sum's formats.
sum() {
  file=loops-$tag-$1.tar.gz
  s=$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1; exit }' "$sums")
  case $s in
    "" | *[!0-9a-f]*) fail "no checksum for $file in $sums" ;;
  esac
  [ ${#s} -eq 64 ] || fail "no checksum for $file in $sums"
  echo "$s"
}

mac_arm=$(sum aarch64-apple-darwin)
mac_intel=$(sum x86_64-apple-darwin)
linux_arm=$(sum aarch64-unknown-linux-musl)
linux_intel=$(sum x86_64-unknown-linux-musl)

root=$(cd "$(dirname "$0")/.." && pwd)
sed \
  -e "s/@VERSION@/${tag#v}/g" \
  -e "s/@TAG@/$tag/g" \
  -e "s/@SHA256_AARCH64_APPLE_DARWIN@/$mac_arm/" \
  -e "s/@SHA256_X86_64_APPLE_DARWIN@/$mac_intel/" \
  -e "s/@SHA256_AARCH64_UNKNOWN_LINUX_MUSL@/$linux_arm/" \
  -e "s/@SHA256_X86_64_UNKNOWN_LINUX_MUSL@/$linux_intel/" \
  "$root/packaging/homebrew/loops.rb"
