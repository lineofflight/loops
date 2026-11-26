#!/bin/sh
# Install the Loops mod into Herdr from this checkout: the mod's runtime
# files to $HERDR_APP_HOME/mods/loops, then check the mod with herdr-app.
# It needs no layout of its own: it shows in the status bar of whichever
# layout it is opened on, stock main included.
#
# HERDR_APP_HOME  Herdr's home (default: ~/.config/herdr-app)
# HERDR_APP_BIN   the herdr-app that checks the mod (default: the one on
#                 PATH; with none, the check is skipped)
set -eu

src=$(cd "$(dirname "$0")" && pwd)
home=${HERDR_APP_HOME:-$HOME/.config/herdr-app}
mod=$home/mods/loops

# A symlink here is most likely a checkout linked in for development:
# copying through it would write over the checkout's own files.
if [ -L "$mod" ]; then
  echo "herdr install: $mod is a symlink, so it is left alone. Remove it to install a copy." >&2
  exit 1
fi

mkdir -p "$mod"
cp "$src/mods/loops/main.js" "$src/mods/loops/logic.js" "$src/mods/loops/mod.json" \
  "$src"/mods/loops/*.svg "$mod/"

bin=${HERDR_APP_BIN:-$(command -v herdr-app || true)}
if [ -n "$bin" ]; then
  "$bin" check "$mod"
else
  echo "herdr-app not found, so the mod was not checked."
fi

echo "Installed the Loops mod to $home."
echo "Open it once on each layout you use: herdr-app do mod.open '{\"name\":\"loops\"}'"
echo "(or Loops in the command palette, Cmd+Shift+P). It comes back with that layout from then on."
