# lab

Tools for a knowledge base and a project management layer that generative agents
can read from and write to. The vaults they operate on live in separate,
private repos; this repo is the public half.

- `crates/lab-schema` — the schema: a described set of fields and rules, plus the
  renderer that produces `SCHEMA.md`
- `kb` — the knowledge base tool
- `schemas/` — the schema data files themselves

`plan.md` is the design document and the record of what was decided and why.

## Building

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## The schema is data

Field and rule definitions live in `schemas/*.toml`, not in Rust. The schema is
expected to move for a long time, and every consumer — the linter, `kb migrate`,
and later the console's form renderer — reads the same description, so a field
added to the data file is a field all three understand.

`kb/SCHEMA.md` in the knowledge base repo is generated from `schemas/note.toml`:

```sh
cargo run -p kb -- schema render --out ../kb/SCHEMA.md
cargo run -p kb -- schema render --out ../kb/SCHEMA.md --check   # for CI
```
