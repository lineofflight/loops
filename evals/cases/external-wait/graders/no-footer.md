---
type: regex
target: trace
match: not_contains
flags: i
# No written reply carries an open loops footer or list, which the ledger
# holds instead: no line, heading or bullet of one that opens with "Still
# open", "Open loops" or "Carried". A sentence that mentions a loop is not
# one.
pattern: '"role":"assistant","content":\[\{"type":"text","text":"(?:(?:[^"\\]|\\.)*?\\n)?[ \t>]*(?:#{1,6} |\*\*|__|[-*] )?(?:still open|open loops|carried)\b'
---
