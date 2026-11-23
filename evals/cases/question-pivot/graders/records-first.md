---
type: regex
target: trace
match: not_contains
# The skipped item is recorded before the new ask is started: fails on
# any written text or tool call, bar ToolSearch and the loops server's
# own, before the first `add`, or when there is none.
pattern: '^(?:(?!"name":"mcp__plugin_loops_loops__add")[\s\S])*?(?:"type":"tool_use","id":"[^"]*","name":"(?!mcp__plugin_loops_loops__|ToolSearch")|"role":"assistant","content":\[\{"type":"text")'
---
