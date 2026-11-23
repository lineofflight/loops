#!/bin/sh
# The installers and the Homebrew formula, offline: docs/install.sh
# against a file:// release, integrations/herdr/install.sh against a
# throwaway Herdr home with a mock herdr-app, and
# scripts/render-formula.sh against that release's checksums. Runs with
# its own HOME, so nothing of the user's is read or written.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/loops-packaging.XXXXXX")
work=$(cd "$work" && pwd -P)
trap 'rm -rf "$work"' EXIT
trap 'exit 130' INT TERM

# Ruby, to check the formula, found while HOME is still the user's, as a
# version manager's shim may need it.
ruby=$(ruby -e 'print RbConfig.ruby' 2>/dev/null || true)

HOME=$work/home
export HOME
mkdir -p "$HOME"
unset XDG_BIN_HOME XDG_CONFIG_HOME XDG_DATA_HOME XDG_STATE_HOME XDG_CACHE_HOME \
  HERDR_APP_HOME HERDR_APP_BIN LOOPS_VERSION LOOPS_INSTALL_DIR LOOPS_RELEASE_BASE_URL

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# Every script parses.
for script in docs/install.sh integrations/herdr/install.sh scripts/render-formula.sh \
  scripts/eval.sh tests/packaging.sh; do
  sh -n "$root/$script" || fail "$script does not parse"
done

# A release, v0.1.0, of all four targets, each a script for loops that
# names its target, as the release workflow lays them out.
release=$work/release
targets="aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl"
mkdir -p "$release/v0.1.0"
for target in $targets; do
  stage=$work/stage/$target
  mkdir -p "$stage"
  printf '#!/bin/sh\necho "loops %s"\n' "$target" >"$stage/loops"
  cp "$root/LICENSE" "$root/README.md" "$stage/"
  tar -czf "$release/v0.1.0/loops-v0.1.0-$target.tar.gz" -C "$stage" loops LICENSE README.md
done
(
  cd "$release/v0.1.0"
  for f in loops-*.tar.gz; do
    echo "$(sha256 "$f")  $f"
  done
) >"$release/v0.1.0/SHA256SUMS"

# docs/install.sh installs the host's binary, runnable.
bin=$work/bin
out=$(LOOPS_RELEASE_BASE_URL=file://$release LOOPS_VERSION=v0.1.0 LOOPS_INSTALL_DIR=$bin \
  sh "$root/docs/install.sh") || fail "install.sh failed: $out"
[ -x "$bin/loops" ] || fail "install.sh left no executable loops"
case $("$bin/loops") in
  "loops "*) ;;
  *) fail "install.sh installed the wrong file" ;;
esac
case $out in
  *"not on your PATH"*) ;;
  *) fail "install.sh did not say $bin is not on PATH: $out" ;;
esac
[ -z "$(find "$bin" -name '.loops.*')" ] || fail "install.sh left a staged file"

# A version without its v is the same release.
LOOPS_RELEASE_BASE_URL=file://$release LOOPS_VERSION=0.1.0 LOOPS_INSTALL_DIR=$work/bin2 \
  sh "$root/docs/install.sh" >/dev/null || fail "install.sh did not take 0.1.0"
[ -x "$work/bin2/loops" ] || fail "install.sh did not install 0.1.0"

# A checksum mismatch fails and leaves the installed binary alone.
mkdir -p "$release/v0.1.1"
for target in $targets; do
  cp "$release/v0.1.0/loops-v0.1.0-$target.tar.gz" "$release/v0.1.1/loops-v0.1.1-$target.tar.gz"
  echo "0000000000000000000000000000000000000000000000000000000000000000  loops-v0.1.1-$target.tar.gz"
done >"$release/v0.1.1/SHA256SUMS"
printf 'old\n' >"$bin/loops"
if LOOPS_RELEASE_BASE_URL=file://$release LOOPS_VERSION=v0.1.1 LOOPS_INSTALL_DIR=$bin \
  sh "$root/docs/install.sh" >/dev/null 2>"$work/err"; then
  fail "install.sh accepted a checksum mismatch"
fi
grep -q "checksum mismatch" "$work/err" || fail "install.sh did not report the mismatch"
[ "$(cat "$bin/loops")" = old ] || fail "install.sh replaced loops on a mismatch"
[ -z "$(find "$bin" -name '.loops.*')" ] || fail "install.sh left a staged file"

# Nor does a missing release leave anything behind.
if LOOPS_RELEASE_BASE_URL=file://$release LOOPS_VERSION=v9.9.9 LOOPS_INSTALL_DIR=$work/bin3 \
  sh "$root/docs/install.sh" >/dev/null 2>&1; then
  fail "install.sh installed a missing release"
fi
[ ! -e "$work/bin3/loops" ] || fail "install.sh installed a missing release"

# integrations/herdr/install.sh copies the mod's runtime files and the
# layout, and checks the mod. Its copy here has the files an editor
# setup leaves beside the mod, which stay behind.
herdr=$work/herdr
cp -R "$root/integrations/herdr" "$herdr"
touch "$herdr/mods/loops/gpui-kit.d.ts" "$herdr/mods/loops/jsconfig.json"
mock=$work/herdr-app
printf '#!/bin/sh\necho "$*" >>"%s"\n' "$work/herdr-app.log" >"$mock"
chmod +x "$mock"

app=$work/app
HERDR_APP_HOME=$app HERDR_APP_BIN=$mock sh "$herdr/install.sh" >/dev/null ||
  fail "herdr install.sh failed"
expected=$(cd "$root/integrations/herdr/mods/loops" &&
  printf '%s\n' main.js tabs.js logic.js mod.json *.svg | sort)
copied=$(cd "$app/mods/loops" && printf '%s\n' * | sort)
[ "$copied" = "$expected" ] || fail "herdr install.sh copied $(echo "$copied" | tr "\n" " ")"
for f in $expected; do
  cmp -s "$root/integrations/herdr/mods/loops/$f" "$app/mods/loops/$f" ||
    fail "herdr install.sh changed $f"
done
cmp -s "$root/integrations/herdr/layouts/loops.json" "$app/layouts/loops.json" ||
  fail "herdr install.sh did not copy the layout"
[ "$(cat "$work/herdr-app.log")" = "check $app/mods/loops" ] ||
  fail "herdr install.sh did not check the mod: $(cat "$work/herdr-app.log")"

# Run again, it updates the copy in place.
HERDR_APP_HOME=$app HERDR_APP_BIN=$mock sh "$herdr/install.sh" >/dev/null ||
  fail "herdr install.sh failed over an earlier copy"

# Without herdr-app, it skips the check.
out=$(HERDR_APP_HOME=$work/app2 PATH=/usr/bin:/bin sh "$herdr/install.sh") ||
  fail "herdr install.sh failed without herdr-app"
case $out in
  *"not checked"*) ;;
  *) fail "herdr install.sh did not say it skipped the check: $out" ;;
esac

# It never writes through a symlink, as a checkout linked in would be.
linked=$work/linked
mkdir -p "$linked/mods" "$work/checkout"
ln -s "$work/checkout" "$linked/mods/loops"
if HERDR_APP_HOME=$linked HERDR_APP_BIN=$mock sh "$herdr/install.sh" >/dev/null 2>"$work/err"; then
  fail "herdr install.sh wrote through a linked mod"
fi
grep -q "symlink" "$work/err" || fail "herdr install.sh did not say why it stopped"
[ -z "$(ls "$work/checkout")" ] || fail "herdr install.sh wrote into the linked mod"

linked=$work/linked2
mkdir -p "$linked/layouts"
echo '{}' >"$work/layout.json"
ln -s "$work/layout.json" "$linked/layouts/loops.json"
if HERDR_APP_HOME=$linked HERDR_APP_BIN=$mock sh "$herdr/install.sh" >/dev/null 2>&1; then
  fail "herdr install.sh wrote through a linked layout"
fi
[ "$(cat "$work/layout.json")" = '{}' ] || fail "herdr install.sh wrote into the linked layout"
[ ! -e "$linked/mods/loops" ] || fail "herdr install.sh copied the mod before refusing"

# scripts/render-formula.sh fills in every target's URL and checksum.
formula=$work/loops.rb
sh "$root/scripts/render-formula.sh" v0.1.0 "$release/v0.1.0/SHA256SUMS" >"$formula" ||
  fail "render-formula.sh failed"
for target in $targets; do
  grep -qF "url \"https://github.com/lineofflight/loops/releases/download/v0.1.0/loops-v0.1.0-$target.tar.gz\"" "$formula" ||
    fail "the formula has no URL for $target"
  grep -qF "sha256 \"$(sha256 "$release/v0.1.0/loops-v0.1.0-$target.tar.gz")\"" "$formula" ||
    fail "the formula has no checksum for $target"
done
grep -qF 'version "0.1.0"' "$formula" || fail "the formula has no version"
! grep -q '@[A-Z0-9_]*@' "$formula" || fail "the formula has a placeholder left"
if [ -n "$ruby" ]; then
  "$ruby" -c "$formula" >/dev/null || fail "the formula is not valid Ruby"
fi

# It refuses checksums that miss a target.
grep -v x86_64-apple-darwin "$release/v0.1.0/SHA256SUMS" >"$work/partial"
if sh "$root/scripts/render-formula.sh" v0.1.0 "$work/partial" >/dev/null 2>&1; then
  fail "render-formula.sh rendered without every checksum"
fi

echo "PASS: packaging"
