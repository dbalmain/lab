---
description: A guide, which is exempt from the inbound-link rule as an entry point.
type: guide
scope: global
confidence: high
asserted: 2026-08-30
source:
  - conversation 2026-08-30
triggers:
  - "prove the guide exemption works"
---

Guides are reached by trigger, not by link, so nothing pointing here is fine.

Bash's `[[ -f foo ]]` and a fenced block:

```sh
if [[ -n "$x" ]]; then echo "[[not-a-link]]"; fi
```

Neither of those is a link, and a linter that thinks otherwise is unusable in a
repo that documents shell.
