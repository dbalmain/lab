# lab

Tools and infrastructure for orchestrating generative AI agents across projects.

## Contents

- **`plan.md`** — System design: a knowledge base (KB) shared across projects,
  and a project management (PM) layer that dispatches work to agents. Documents
  governing principles, lifecycle, cost structure, and implementation phases.
- **`token-report.py`** — Utility to read and report token usage from Claude and
  Codex session logs. Retroactive instrumentation for cost accounting (no
  configuration needed).

## Building

Phases P0–P13 are defined in `plan.md` §23. The system is not yet built; this
repo will hold the Rust tools as they are implemented.

## Principles

- Determinism belongs in scripts, not agents
- State belongs in artefacts, not context
- Evidence over assertion
- The process is an experiment — measure and test assumptions

See `plan.md` §2 for the full set.

## Measurement

The system is designed to measure its own cost. Each dispatch records tokens and
turns into a ledger. This repo includes `token-report.py` as the seed; grok
token accounting is the instrumentation gap identified at P10.

## License

Public. See `plan.md` for copyright and attribution.
