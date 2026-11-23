#!/bin/sh
# A page with a grey Save button, for the unrelated tweak.
set -eu
cat >index.html <<'HTML'
<!doctype html>
<title>Dates</title>
<style>
  .save { background: #888; color: #fff; }
</style>
<button class="save">Save</button>
HTML
