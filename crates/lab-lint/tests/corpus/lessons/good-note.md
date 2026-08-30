---
description: A well-formed note, so a test can tell a real finding from noise.
type: lesson
scope: global
confidence: high
asserted: 2026-08-30
source:
  - conversation 2026-08-30
triggers:
  - "check the linter finds nothing wrong"
---

The fact.

**Why:** a corpus of only broken notes cannot show that the linter is quiet when
it should be. See [[linked-note]].

**How to apply:** keep one note here that must always pass. This note also links
to every other fixture — [[bad-enum]], [[bad-scope]], [[stores-derivable]],
[[missing-field]], [[stale]], [[broken-link]] — so that each of them carries
exactly one defect. Without that, every fixture is also an orphan, and a test
about one rule fails because of another. `[[orphan]]` is deliberately absent, and
`[[colon-in-scalar]]` cannot be linked because it does not parse — and writing
both in code spans is itself the masking rule at work.
