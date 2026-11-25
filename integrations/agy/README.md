# loops for Antigravity (agy)

One plugin does it all: it registers loops' MCP server and carries the one
rule agy needs to see the server's instructions.

- `mcp_config.json`: the server, named `loops`, run as `loops mcp` over
  stdio. agy names a plugin's server `<plugin>_<server>`, so it shows as
  `loops_loops`, with the tools `loops_loops/list`, `add`, `done` and
  `clear`.
- `rules/AGENTS.md`: a one-line rule telling the model to read that
  server's instructions before its first reply. agy writes MCP server
  instructions to a file rather than into the prompt, so without the rule
  the model may never see them. The instructions themselves stay in loops.

## Install

Install `loops` on your `PATH` first (see the [main README](../../README.md)).
Then, from a loops checkout:

```sh
agy plugin install ./integrations/agy
agy plugin enable loops
```

agy copies the plugin into `~/.gemini/config/plugins/loops/`, so reinstall
after changing it. Restart agy to pick it up. The plugin registers the
server itself, so drop any `loops` entry from
`~/.gemini/config/mcp_config.json`, or agy runs it twice.

The rule can be project-scoped instead: place this plugin's contents under
`.agents/plugins/loops` in a repository and agy loads it there without
`plugin install` or `enable`.

## Test

```sh
agy plugin validate integrations/agy
```
