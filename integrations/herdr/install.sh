#!/bin/sh
# Install the Loops drawer into Herdr from this checkout: the mod's files
# to $HERDR_APP_HOME/mods/loops and its layout to
# $HERDR_APP_HOME/layouts/loops.json, then check the mod with herdr-app.
#
# HERDR_APP_HOME  Herdr's home (default: ~/.config/herdr-app)
# HERDR_APP_BIN   the herdr-app that checks the mod (default: the one on
#                 PATH; with none, the check is skipped)
set -eu

src=$(cd "$(dirname "$0")" && pwd)
home=${HERDR_APP_HOME:-$HOME/.config/herdr-app}
mod=$home/mods/loops
layout=$home/layouts/loops.json

# A symlink here is most likely a checkout linked in for development:
# copying through it would write over the checkout's own files.
for path in "$mod" "$layout"; do
  if [ -L "$path" ]; then
    echo "herdr install: $path is a symlink, so it is left alone. Remove it to install a copy." >&2
    exit 1
  fi
done

mkdir -p "$mod" "$home/layouts"
cp "$src/mods/loops/main.js" "$src/mods/loops/tabs.js" "$src/mods/loops/logic.js" \
  "$src/mods/loops/mod.json" "$src"/mods/loops/*.svg "$mod/"
cp "$src/layouts/loops.json" "$layout"

bin=${HERDR_APP_BIN:-$(command -v herdr-app || true)}
if [ -n "$bin" ]; then
  "$bin" check "$mod"
else
  echo "herdr-app not found, so the mod was not checked."
fi

echo "Installed the Loops drawer to $home."
echo "Show it with: herdr-app do layout.switch --name loops (or Cmd+Shift+L to cycle layouts)"
