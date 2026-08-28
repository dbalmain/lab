# Plan: shared knowledge base + agent project management

Status: rev 8 · 2026-08-29 · authors: Claude (Opus 5, Fable 5), with Dave. Rev 8
makes schema migration a tool rather than a chore: the schema is expected to stay
fluid through P8, so `kb lint --fix` and `kb migrate` are P1 scope and a schema
change is applied by one command across every note (decision 41). Rev 7
cleared the decks for implementation: the aven capability question is answered
against the source rather than the roadmap (decision 34 — grants are already
per-invocation, scoping within a grant is aven-side work that precedes P10b), and
the six calls standing between rev 6 and the first line of code are settled as
decisions 35–40. The largest of them reshapes §6 — the note namespace is one per
registered source rather than one per vault, because every project keeps its own
tool-specific knowledge alongside what it promotes. Rev 6 added
the module layer (§22.1): modules contribute search targets, tools and console
forms, a form submission creates a ticket, and aven is the module language. Rev
5 added the cost dimension the plan had ignored and measured it rather than
assuming it: context depth costs money, caching changes the shape, and which
pattern wins is empirical (§16.3). Rev 4 settled the two open naming questions
and renamed throughout to plain terminology (§2 principle 8). Rev 3 folded in
the second adversarial pass (leak-gate architecture, pm-repo concurrency,
curation snapshotting, index scaling, provenance laundering) and Dave's
responses; decisions are recorded in §25.

## 0. How to review this

This is one plan covering two systems that share a substrate: a **knowledge
base** (KB) shared across projects and agents, and a **project management** (PM)
layer that dispatches work to agents and routes results back to a human. They
are planned together because a ticket, a review finding and a durable lesson are
all the same kind of object with different `type` fields.

Nothing here is built yet. Four repos are named but do not exist.

**The decisions taken before implementation** — the calls that were cheap then
and expensive later — are held on their own page with the options and tradeoffs
each was weighed against, so the reasoning survives the summary of it here:
<https://claude.ai/code/artifact/bf550305-cb81-42e4-b8b5-06b628bf3c53>. It is
updated in place; answered questions stay on it with their answers.

What I would most value being challenged is listed in §25. If a diagnosis in
here is wrong, say so and propose the better shape rather than working around it
— a correct "this is actually X" is worth more than a refinement of a wrong
frame.

Throughout: **evidence** means something observed in the filesystem or stated by
Dave. **Guess** means my inference. They are marked where it matters.

---

## 1. Situation

### 1.1 What exists today (evidence)

Dave works with generative AI roughly 60–80 hours a week across several
projects, orchestrating other agents from Claude Code and reviewing their
output.

Harnesses in use: **Claude Code**, **codex** (Sol for complex work), **grok
build** (cheap, plentiful, credit consistently unused), **opencode**.

Relevant directories:

| Path            | Contents                                                                                                                                                                                   | Remote                 |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------- |
| `~/w/clex`      | Language project. 356 markdown notes in `.ai/`, 138 in `aven-lang/.ai/` — offload briefs and done-notes named `codex-*`, `grok-*`, `claude-*`. Plus `docs/`, `DECISIONS.md`.               | —                      |
| `~/w/aic-edit`  | Client-platform tooling. 49 markdown files, incl. a well-structured `docs/api/` set.                                                                                                       | —                      |
| `~/style-guide` | 5 files: `common.md`, `rust.md`, `haskell.md`, `typescript.md`, `README.md`. Pure public reference.                                                                                        | `dbalmain/style-guide` |
| `~/w/ai-tools`  | Code: `bin/html-page`, `bin/notify`, `bin/sync-style`, `bin/check-email-tokens`; a CSS token system (`style/web/tokens.css`, `components.css`); `skills/html-review`, `skills/send-email`. | `dbalmain/ai-tools`    |
| `~/.claude`     | Harness config. `CLAUDE.md`, `agent-playbook.md`, 7 skills, `settings.json`, and 65 memory files across 6 projects under `projects/*/memory/`. Allowlist `.gitignore`.                     | `dbalmain/claude`      |

`~/.claude/agent-playbook.md` is a hand-written set of durable lessons about
offloading work to agents. It is the highest-quality knowledge artefact in the
set and is the model for what durable notes should look like.

### 1.2 What works today

- Orchestrate-then-review with a **different** agent beats single-agent
  self-review. This is Dave's observation and it matches the mechanism: a model
  reviewing its own work checks its reasoning with the same reasoning.
- Accumulating project knowledge in-repo (`aic-edit`, `clex`) has been
  "invaluable".
- Offload done-notes keep compile/fix cycles out of the main context.

### 1.3 Pain points (Dave's words, condensed)

1. **Losing track of project state.** No clear indicator of where each project
   is up to, or when work has finished on a project he isn't currently looking
   at. _Stated as the main pain point._
2. Roadmaps exist per project but are hard to review and prioritise.
3. Scoping new work clutters the context of in-flight work; jumping between
   follow-up and scoping degrades quality.
4. Roadmap items occasionally get lost (rare now).
5. Terminal-only interface; diagrams and linked documentation would help scoping
   and review.
6. Round tables with other agents must be requested manually and consume
   context.
7. No agent specialisation configured.
8. Wants a mode where he builds and agents assist (the `aven-lang` rebuild).
9. Knowledge is not shared between parallel agents.
10. Duplicate information across projects; general knowledge goes unsaved
    because it belongs to no project.

### 1.4 Diagnosis

Points 2, 3, 4, 6, 9 are one problem: **system state lives in conversation
context** — volatile, single-threaded, and shared between unrelated concerns.
Point 1 is the same problem viewed from outside: state that lives in a context
window is invisible when you aren't in that window.

The fix is to move state into durable, structured artefacts, and let every agent
invocation start cold with exactly the context it needs.

Point 10 is a **write-path** failure, not a search failure. Better search over
an undistilled corpus returns sludge, ranked.

---

## 2. Governing principles

These are load-bearing; most specific decisions below follow from them.

1. **Determinism belongs in scripts, not agents.** If the output is fully
   determined by the input, it must be a script the agent calls. Agents are for
   judgment. (§21)
2. **State belongs in artefacts, not context.** If losing a session would lose
   it, it is in the wrong place.
3. **Propose aggressively, accept conservatively.** Agents generate proposals
   without asking; promotion to durable state is deliberate and reviewed. This
   single rule governs curation, ticket triage, and public promotion.
4. **One copy of anything.** Duplication is the disease being treated; the cure
   must not reintroduce it. Prefer a pointer to a copy, always.
5. **Review capacity is the bottleneck, not throughput.** Dave is the constraint
   and will remain so. Any change that increases agent output without increasing
   review throughput makes things worse.
6. **Evidence over assertion.** Every durable claim carries provenance and a
   date. Half of what is "known" about a tool is dated observation.
7. **The process is an experiment.** No workflow in this plan is final. Run
   deliberate experiments — cheap A/B tests on small process changes, and
   occasional larger ones designed to break out of local maxima — and record the
   results as KB lessons. Every experiment is scoped before it runs — a stated
   hypothesis and a pre-registered evaluation method — so the result is read
   against criteria, not vibes; the designs themselves are future work, not
   fixed in this plan. A process that has never been tested against an
   alternative is a guess with seniority.
8. **Names state their referent.** No term in this system may require a
   glossary, and no component is named after a person-role. This is a rule about
   agents, not taste: a metaphor name arrives pre-loaded with the wrong
   associations, which a model must spend tokens suppressing, and a person-role
   leaks entailments into behaviour — "gardener" imports that growth is good,
   when the job is mostly rejection (§24). A glossary does not fix it, because a
   glossary is optional context: when it fails to load the agent does not fail
   loudly, it falls back on the metaphor's wrong prior and proceeds confidently.
   Dead metaphors with settled technical meanings (`queue`, `index`, `daemon`,
   `blast_radius`, `watermark`, `quarantine`) are plain words and are fine. The
   opposite failure is equally banned: `manager`, `handler`, `service`, `util`
   are plain English and state nothing. The test is whether the name states the
   operation. (§25 decision 24)
9. **Cost compounds with context depth, and caching matters.** Every turn
   re-sends the accumulated conversation; caching discounts that but does not
   eliminate it. This creates two levers: within a working session (keep it warm
   for iterative work to reuse context via cache), and across dispatches
   (serialize short sessions with artefact handoff rather than embedding one
   session inside another). How effective these levers are is not obvious —
   cache hit rates, model efficiency, and the nature of the work all vary — so
   the system measures both, recording per-dispatch costs into the ledger
   (§16.3), and experiments on the trade-offs (warm vs. cold, single-session vs.
   multi-dispatch) as work arises. (§16.2, §16.3, §23 P10)

---

# Part I — Knowledge base

## 3. Shape

Two independent axes. Conflating them is the main way this design goes wrong.

**Tier** — how curated. Tiers are named for what they are; there is no T0/T1/T2
shorthand to memorise (§2 principle 8):

| Tier         | What                                           | Curation                           | Reaches an agent by                |
| ------------ | ---------------------------------------------- | ---------------------------------- | ---------------------------------- |
| **index**    | One line per durable note                      | Generated from durable frontmatter | Always in context                  |
| **durable**  | One concept per note, deduped, linked, sourced | Hand-reviewed, every change        | Retrieval or hook injection        |
| **episodic** | Done-notes, briefs, session memory, ledgers    | None; append-only                  | Indexed only, never read wholesale |

**Visibility** — `public` or `private`, orthogonal to tier.

The episodic tier already exists by accident: ~500 done-notes plus 65 memory
files. The index and a disciplined durable tier are missing. That gap is pain
point 9 and 10.

This tier is the **index slice** — the generated lines that sit in a prompt. It
is a different artefact from the **search index** qmd builds (§8), which is
never in a prompt; the two update on different schedules for reasons in §16.3.
The index slice is generated **per scope** (§9): one global slice plus one per
project, each independently capped. A single flat index has a scaling ceiling
the plan's own growth projections would hit within weeks of P9 — ~15 playbook
notes plus 65 memory seeds plus up to 10 promotions per curation batch across
five-plus sources — after which it is either incomplete or unread.

## 4. Repositories

Four repos, sibling clones, no submodules. kb-priv's registry (§5) is the only
thing that ties the knowledge repos together; the fourth holds the tools that
operate on them — public code, private vaults (§22).

```text
~/w/kb/                          PUBLIC, self-contained
├── INDEX.md                     the index, public
├── SCHEMA.md                    the note schema (§6)
├── lessons/                     durable cross-project lessons
├── reference/                   tool / API / protocol facts
├── decisions/                   rationale that outlived its project
├── style/                       absorbs ~/style-guide
└── skills/                      canonical cross-harness skills (§10)

~/w/kb-priv/                     PRIVATE
├── clients/                     per-client notes, PII, engagement context
├── people/                      non-public figures
├── policy/denylist.txt          leak-scan terms (itself private)
├── registry.toml                what is indexed and curated -- incl. kb itself
├── evals/                       retrieval eval fixtures
├── curation/                    prompts, runner, watermarks
└── INDEX.md                     the index, private

~/w/pm/                          PRIVATE
├── tickets/<project>/           one markdown file per ticket
├── ledgers/<ticket-id>.md       review-round records (episodic)
├── profiles/                    review profile definitions and checklists
└── projects/<name>.toml         per-project config: slots, autonomy, triage owner

~/w/lab/                         PUBLIC -- the tools
├── crates/                      shared schema, frontmatter, lint, retrieval
├── kb/                          the kb binary
├── pm/                          the pm binary
└── lab-daemon/                  the daemon (§21.1) -- lands at P10
```

**Dependencies point one way: kb-priv and pm may reference kb; kb references
nothing.** A client machine runs `git clone github.com/dbalmain/kb` and gets
everything it is permitted to have — no submodule to init, no advertised private
URL, nothing to configure.

Hosting: the tools repo is public on GitHub from the start. **kb is created
private and flips to public at P8**, when the denylist, the pre-push gate and
the tripwire CI exist; the flip runs the same one-time full-tree-and-history
scan §5 specifies for any repo opening late. Notes are written as though public
from note one — the private window is an undo, not a licence. kb-priv and pm are
GitHub private repos permanently — acceptable for their current sensitivity
(Dave's call; revisit if genuinely sensitive client material ever lands).
Redundancy comes from clones on two always-on tailscale-reachable machines at
different sites, not from a second forge.

Rejected: submodules in either direction. Private-inside-public advertises the
private URL in `.gitmodules` and breaks every client clone;
public-inside-private makes each kb commit a two-step (commit, push, bump
pointer) an agent will eventually half-do, and pins SHAs for no information
gain. The registry does the pointer's job without the ceremony: kb is registered
as a source like any project.

Tickets get their own repo rather than living in each project because the
dashboard must render from one checkout, `depends_on` crosses projects, and
tickets touching client work cannot sit in a public project repo. They are not a
directory inside kb-priv because they are a different kind of thing — work in
flight, rewritten many times an hour by CLI-mediated state transitions — and
mixing them into the knowledge vault would drown its history and its reviews in
ticket churn. (Rev 2 also cited the curation pass's dirty-tree refusal here;
that rationale dissolved when it moved to snapshot reads, §11.) The one loss — a
code commit cannot atomically close its ticket — does not matter, because the
CLI owns every state transition anyway (§21).

## 5. What goes where, and the registry

Adding `~/style-guide`, `~/.claude` and `~/w/ai-tools` forces a taxonomy. Those
directories are not uniformly "knowledge".

| Kind      | Test                                                         | Destination                          |
| --------- | ------------------------------------------------------------ | ------------------------------------ |
| Knowledge | True independent of any machine                              | kb / kb-priv, durable tier           |
| Code      | Executable; has tests or a runtime                           | Stays in its own repo                |
| Config    | A harness reads it from a fixed path to change its behaviour | Stays put, private                   |
| Generated | Derived from the KB; safe to delete and rebuild              | Emitted wherever the harness demands |

Disposition of existing repos:

- **`dbalmain/style-guide` — absorb.** Five files, all public reference. Becomes
  `kb/style/`; archive the repo. Migration cost is one grep: offload prompts
  reference it by path.
- **`dbalmain/ai-tools` — keep.** It is code. Only `skills/` moves out.
- **`dbalmain/claude` — shrink.** Loses `agent-playbook.md` and the general half
  of `CLAUDE.md` to kb; `skills/` become generated stubs. What remains is
  genuinely config.

Applied to `~/.claude`:

- Knowledge → `agent-playbook.md` becomes ~15 atomic lessons; shell/doc
  conventions in `CLAUDE.md` become public reference notes.
- Config → `settings.json`, `keybindings.json`, `.credentials.json`. Untouched.
- Generated → `skills/*` become adapter stubs; `CLAUDE.md` becomes a thin
  pointer at kb.
- Episodic → `projects/*/memory/` and `plans/` are registered and indexed **in
  place**.
- Excluded → `history.jsonl`, `sessions/`, `file-history/`, `shell-snapshots/`.
  Raw transcripts with client material; excluded explicitly, not by omission.

Projects are **registered, not submoduled.** A submodule would make kb-priv pin
project SHAs, so every project commit dirties the KB for no information gain,
and it would put a scheduled curation run and Dave in the same working tree.

```toml
# kb-priv/registry.toml

[[source]]
name        = "kb"
path        = "~/w/kb"
durable     = ["lessons", "reference", "decisions", "style"]
publication = "public"      # already published
curate      = { min_backlog = 20 }

[[source]]
name        = "clex"
path        = "~/w/clex"
durable     = ["docs", "DECISIONS.md"]
episodic    = [".ai"]
publication = "public"      # repo private today, intended to publish
curate      = { min_backlog = 20 }

[[source]]
name        = "aven-lang"
path        = "~/w/clex/aven-lang"
episodic    = [".ai"]
publication = "public"      # repo is public
curate      = { min_backlog = 20 }

[[source]]
name        = "aic-edit"
path        = "~/w/aic-edit"
durable     = ["docs", "docs/api"]
episodic    = ["tmp"]
publication = "public"
curate      = { min_backlog = 20 }

[[source]]
name        = "claude-memory"
path        = "~/.claude/projects"
episodic    = ["*/memory"]
exclude     = ["history.jsonl", "sessions", "file-history", "shell-snapshots"]
publication = "private"     # raw session material; never published
curate      = { min_backlog = 20 }

[[source]]
name        = "pm"
path        = "~/w/pm"
episodic    = ["ledgers"]
publication = "private"
curate      = { min_backlog = 20 }
```

`curate` is demand-driven, not scheduled: a source is curated when its episodic
backlog since the watermark crosses the threshold (§11). A timer out-runs the
reviewer — five weekly sources at the 10-promotion batch cap is ~50 curation
decisions a week landing on the stated bottleneck — where a threshold tracks
supply. The 20 is a placeholder to tune with use.

**`publication` is a guard, not a gate-opener.** It records whether a source's
content is published or intended to be (`public`) or must never be (`private`).
Counter-intuitively, `public` _adds_ safeguards: capture and curation into a
`public` source run the denylist scan at write time, because everything written
there is on a path to publication — including repos like clex that are private
today but meant to open later. Flipping such a repo to actually-public requires
a one-time full-tree-and-history scan, because content predating the safeguards
is in it. `private` sources may hold client material and reach kb only through
the generalization verb (§7), never wholesale. This is a different axis from a
note's `visibility` (§6): `visibility` decides which KB repo a promoted note
lands in; `publication` describes the fate of a registered source tree.

Project-durable knowledge **stays in the project**. `aic-edit/docs/api` does not
move. It gets the same schema, curation and index. Only knowledge that has
escaped its project — true of more than one codebase, or of a tool rather than a
program — gets promoted into kb/kb-priv.

## 6. Note schema

The schema is the one place field names are written down, and it is written and
proven by hand on real content before any machinery assumes it.

```yaml
---
name: monitors-self-match
description:
  A watcher that greps for a pattern present in its own command line matches
  itself and spins forever.
type: lesson # lesson | reference | decision | howto | client | task | finding | position
scope: global # global | project:clex | client:acme
visibility: public # public | private -- DEFAULT private
confidence: high # high | medium | low
asserted: 2026-07-22 # when this was last known true
source: # provenance -- no note without it
  - clex/.ai/grok-monitor-loop.md
  - agent-playbook.md
triggers: # when this note matters, in Dave's words
  - "writing a process monitor"
  - "pgrep"
---
```

Body: the fact, then `**Why:**`, then `**How to apply:**`. Link with `[[name]]`.

Rules:

- **One concept per note.** If the description needs an "and", it is two notes.
- **Edit over create.** Before writing, search for the note this should have
  been an edit to. Enforced on the write path (§9), not by good intentions.
- **No note without provenance.**
- **Private is the default.** Publication is an explicit, reviewed act on one
  note. There is no bulk promotion.
- **`asserted` is not decoration.** A lesson asserting tool behaviour older than
  ~90 days is flagged for recheck. Rechecking is grok breadth work (§20): it
  gathers the evidence and escalates to needs-you only on contradiction — the
  flag must not become another queue for Dave.
- **Names are unique per source, and cross-source links are prefixed.** The
  namespace is not two vaults but one per registered source (§5): kb, kb-priv,
  pm and every project repo that declares `durable`. `[[name]]` means within
  this source; `[[source:name]]` crosses, where `source` is the registry name —
  `[[kb:monitors-self-match]]` from a project note, `[[aic-edit:token-refresh]]`
  the other way. Links genuinely run in both directions: a project's general
  knowledge is promoted to kb while its tool-specific knowledge stays put, and
  each half wants to reference the other.
- **Within-source links must resolve; cross-source links resolve where the
  target is checked out.** The first is a hard lint error — it is the guarantee
  a lone clone needs, and `kb lint` on a bare public kb must still pass. The
  second warns when the registry names a source that is absent and never fails,
  because kb cannot depend on any other repo existing.
- **A public source may link only public sources.** Unchanged in force from the
  visibility rule it replaces: a private note's name inside a public link is
  itself a leak, whether the private thing is kb-priv or a client's repo.
  Private sources may link both ways.

## 7. The public/private boundary

The one thing that must not fail. Public git history is permanent; the remedy
for a leak is a rewrite and a disclosure, not a follow-up commit.

Five layers, each assuming the one above will occasionally fail:

1. **Default-private.** Curation writes into kb-priv unless a note is explicitly
   marked otherwise.
2. **The denylist lives in private.** `kb-priv/policy/denylist.txt` — client
   names, domains, codenames, people. It cannot live in kb; the list is itself
   the disclosure.
3. **The pre-push gate is the real gate.** Push is publication for a public
   repo, so the last thing standing before it is a **full-tree** denylist scan
   in a pre-push hook (pre-commit runs the same scan as early warning). The
   denylist is read from the sibling kb-priv checkout at a configured path;
   where it is absent — a client machine with kb cloned alone — the hook
   **refuses the push** rather than passing. Hooks are client-side and
   `--no-verify` exists, so every agent-facing write path (`kb promote`,
   `kb curate`) runs the same scan in-process, where deleting a hook cannot
   bypass it.
4. **kb-priv CI is the tripwire, not the gate.** It has both trees and rescans
   the _full_ public tree on every kb-priv push — i.e. whenever a term joins the
   denylist — catching content that arrived before its term existed. By the time
   it fires, the content is published; what it buys is fast detection, and the
   remedy is the rewrite-and-disclose path, so it must page, not log.
5. **Public CI does what it can without the list:** gitleaks, email/phone/IP
   regexes, entropy checks.

Deferred hardening: advance kb mainline only via PR whose required check fetches
the denylist over a deploy key — the list stays private; the public repo's CI
merely gets read access to it. Server-side and unbypassable, but PR ceremony on
every note edit is heavy for one person. Revisit if any agent ever gets push
access to kb.

Copy the hygiene already in `~/.claude/.gitignore`: allowlist (`/*`, then
un-ignore named files). It fails closed on anything unanticipated, which is
stronger than a denylist scan.

**Generalization is a first-class operation.** The most valuable public content
is the sanitised form of private experience. It is also the highest-risk
operation, so it gets its own verb, its own digest section, and never happens
without human review of the specific diff. Curation proposes; it does not
promote.

## 8. Retrieval

qmd's architecture is right and does not need replacing: FTS5 + sqlite-vec,
reciprocal rank fusion, reranker, all local. Collections come from the registry
and are indexed where they live.

Two additions worth the effort on this corpus:

- **Contextual chunk augmentation.** Prepend a generated situating sentence to
  each chunk before embedding. Chunk 7 of a 4,000-word codex brief has no idea
  which project it belongs to; a one-line preamble makes it retrievable.
- **Link-graph expansion in reranking.** Retrieve, follow `[[links]]` one hop,
  rerank the union. Free once lint guarantees links resolve.

**Build the eval fixture before tuning anything.** `kb-priv/evals/` — 20+ real
questions from real work, each with the note that should have answered it.
`qmd bench` for precision@k and MRR across keyword-only, vector-only, hybrid.
Without it, fusion weights get tuned on vibes and the "is grep enough?" question
stays unanswerable for this corpus.

**Web-harvested content is quarantined.** A planned daily debrief will pull
internet material on AI and active projects, growing the corpus quickly. It gets
its own collection, excluded from the default search scope, and can never reach
the durable tier except through the same per-note deliberate promotion as
everything else — Dave reads the briefs and chooses what enters. An
undifferentiated stream of secondhand notes is the fastest way to poison
retrieval (§24).

## 9. Surfacing and the write path

"Information that only gets discovered once the agent digs into a problem" is a
**surfacing** failure, not a retrieval failure. The note was findable; nothing
told the agent to look.

1. **The index in context** via the global instruction files, generated **per
   scope**: the global slice is always in context; a project slice
   (`scope: project:clex`) loads only in that project's sessions. The ~200-line
   ceiling — past which an index stops being read — applies per slice, which is
   what lets the corpus grow past 200 notes without breaking the safety net
   (§3).
2. **Prompt-submit hook** running a qmd query and injecting hits above a
   relevance floor. Highest-leverage item in the plan: it converts retrieval
   from something an agent must choose to do into something already done. The
   relevance floor is essential — three mediocre notes on every prompt trains
   you to ignore it. Claude Code has `UserPromptSubmit`; whether the other three
   harnesses have an equivalent is the **first thing P3 checks**, because this
   claim is load-bearing. Fallback where no hook exists: a generated per-project
   context file (a KB slice the harness already reads, the way it reads
   AGENTS.md), refreshed post-commit.
3. **`triggers:` matching** for cases where semantic similarity is weak but the
   situation is sharply identifiable.
4. **A `kb` skill** for explicit deep search.

**Assembly order is stable-first** — profile prompt, then index slice, then
retrieval hits, then volatile ticket material. Free to obey and worth obeying,
but measurement puts the prize at 0.3% of spend (§16.3), so it is hygiene rather
than a design driver, and nothing else should be contorted to serve it.

**Write path:** query for the note this should be an edit to → show candidates →
edit if one fits, create only if none does → fill provenance → lint → regenerate
index. Retrieval and writing exercise each other, so neither rots quietly.

## 10. Cross-harness skills

Four harnesses, four discovery mechanisms, four capability ceilings. Writing
each skill four times is the disease. There is already a live case: `send-email`
exists in both `~/.claude/skills/` and `~/w/ai-tools/skills/`.

```text
kb/skills/<name>/
├── skill.md        canonical body -- tool-neutral prose, the only real copy
├── meta.yaml       name, description, triggers, invocation, harness support
├── scripts/        optional, POSIX sh
└── reference/      optional progressive-disclosure files

generated, never hand-edited:
  ~/.claude/skills/<name>/SKILL.md      frontmatter + pointer
  ~/.codex/prompts/<name>.md            pointer
  <opencode command path>/<name>.md     pointer
  <grok equivalent>                     pointer
```

Each stub carries the harness-specific frontmatter needed for discovery, a
`DO NOT EDIT — generated from kb/skills/<name>` header, and a body that says:
read the canonical file at this path and follow it. CI regenerates and diffs, so
drift fails the build.

Authoring rules:

- **Describe intent, not tool names.** "Read the file at X", never "use the Read
  tool" — tool vocabularies differ across all four.
- **Executables go in `scripts/` as POSIX `sh`.**
- **Declare harness support, tested rather than assumed.**
- **The `*-rust` skills are a special case.** They are skills _about_ driving
  other harnesses; they stay Claude-only and are exempt.

**MCP is deliberately not used here.** All four speak it, but it adds a running
dependency, breaks on client machines and offline, and does not solve
_discovery_ — each harness still needs a local file to know a skill exists. Use
stubs for discovery and qmd's existing MCP server for content search.

## 11. The curation pass

Curation reads episodic material and proposes durable notes. It is named for the
operation, not for an actor — there is no "gardener" persona, because a
person-role name would import that growth is good when the job is mostly
rejection (§2 principle 8, §24).

grok runs the pass, one source per batch, triggered by the registry's backlog
threshold (§5). It never opens a live working tree: it reads an rsync
**snapshot** of the registered paths — episodic notes are often uncommitted, so
a SHA pin would miss exactly the newest material — and writes to `curate/<date>`
in its **own checkout** of kb/kb-priv. Dave's trees are untouched, so there is
no lock, no dirty-tree refusal, and no starvation of the busiest sources — the
rev-2 design refused dirty trees, which at 60–80 hours a week meant the most
productive sources would rarely be curated, and pm's constantly-written ledgers
never would.

| #   | Step                                                   | Constraint                                                    |
| --- | ------------------------------------------------------ | ------------------------------------------------------------- |
| 1   | Snapshot the source's registered paths                 | rsync copy; the pass never opens a live working tree          |
| 2   | Read episodic notes since the watermark                | Never re-reads the whole corpus                               |
| 3   | Propose _promote_, _merge_, _flag stale_, _generalize_ | Those four change kinds and nothing else                      |
| 4   | Commit to `curate/<date>` + write `DIGEST.md`          | No merge, no mainline push, no force-push, no history rewrite |
| 5   | Stop at ~10 promotions                                 | A forty-file PR does not get reviewed                         |

Constraints inherited from `agent-playbook.md`:

- One source per batch, one batch at a time — for review sanity; the two-writer
  rule is satisfied by construction (snapshot reads, own-checkout writes).
- Completion detected by **artefact** (branch SHA + `DIGEST.md`), never by
  process liveness. Capture the PID at launch and poll `/proc/<pid>`; never arm
  a `pgrep` pattern that has not been watched matching real output.
- Invite pushback: a lesson that does not survive its own evidence should be
  reported as not worth keeping, and that is a successful batch.
- Redirect output to a log at launch.

**Review passes.** The digest is agent-written prose _about_ a diff and can be
confidently wrong about what the diff contains. Claude reviews first,
mechanically and adversarially: every claim checked against its cited source;
lessons resting on a single weak observation killed; corroboration audited for
**independence** — a source counts only if it predates the note or adds
observational detail the note lacks, because agents restate injected notes (§9)
and an echo is not evidence (§24); boundary check that nothing private crossed
into public; link and schema lint. Then the PR opens with the digest as its
description. Dave reads the digest and drills into diffs only where it looks
wrong.

**The drafting pipeline is the first process experiment (§2, principle 7).**
Start cheapest: grok drafts, Claude reviews. Measure the correction rate over
two batches. If it stays high, escalate the process — grok extracts candidates
and a stronger model writes, or two blind drafts are synthesised — and measure
again. The expensive variants are earned by data, not assumed; whichever
pipeline wins, keep A/B-ing it occasionally so the process does not fossilise at
a local maximum.

---

# Part II — Project management

## 12. Model

- **Project** — a named unit of work. Links to one or more git repos. Carries
  guidelines, diagrams and docs so a human can understand how it is configured.
  Per-project config (`pm/projects/<name>.toml`) sets `triage_owner` (Dave, or a
  dedicated agent), dispatch `slots`, and orchestrator `autonomy`.
- **Ticket** — a unit of work with a lifecycle. Stored as a markdown file with
  frontmatter in the central private `pm` repo (§4), one file per ticket.
- **Ledger** — the record of a ticket's review rounds. Episodic, indexed, and an
  input to curation. It also carries per-dispatch token and cost accounting
  (§16.3), which is what makes the plan's cost claims checkable.
- **Profile** — a bundle of (system prompt, KB slice, tool permissions, review
  checklist, model). Not a personality.
- **Orchestrator** — each project has an orchestrating agent with a per-project
  autonomy level: `suggest` (proposes dispatches and priorities; Dave acts),
  `approve` (dispatches after Dave OKs), or `auto` (dispatches ready tickets,
  decides what can safely run in parallel, and owns merge conflict resolution).
  The blog and the aven rebuild run at `suggest` — Dave fully at the wheel;
  "just make it work" projects run at `auto`. At dispatch the orchestrator also
  resolves `model: any` — from remaining quota on each model, ticket complexity
  and impact — and records the rationale on the ticket. Quota is a real reading
  where it exists — Sol reports its remaining weekly window in its session log —
  and a known gap where it does not, since neither grok nor Claude records one
  (§16.3). At scoping, the model choice is deliberate: grok where breadth and
  verification dominate, Sol for design-sensitive work, Claude for judgment
  (§20). Whether to keep a session warm or close it between dispatches depends
  on whether the work is iterative (warm sessions reuse context via cache,
  keeping you out of a re-read cost) or serial (each dispatch is independent,
  and artefact handoff is cleaner). Reasoning effort is set per ticket, being
  the one knob that controls output cost once a model is already chosen.

## 13. Ticket schema

```yaml
---
id: aven-lang-0042
title: Reference-counted mutable data structures
project: aven-lang
repos: ["~/w/clex/aven-lang"]
type: build # build | assisted | investigate | chore | bug
state: building # see §14
model: sol # sol | grok | claude | any -- set at scoping; any = orchestrator picks at dispatch (§12)
profile: [rust, memory-model]
blast_radius: design-bearing # bounded | cross-cutting | design-bearing
reversibility: costly # cheap | costly
priority: p1
depends_on: []
evidence: [] # required for agent-filed tickets
owner: sol
worktree: ~/w/.worktrees/aven-lang-0042
rounds: 0 # review rounds elapsed
created: 2026-08-09
updated: 2026-08-09
---
```

Body sections, fixed order, lint-enforced: **Goal**, **Scope** (in/out),
**Success criteria**, **Notes**.

`repos` is a list for forward-compatibility only: lint forbids more than one
entry, because `worktree`, the dispatch slot and `pm merge` are all singular.
The first real multi-repo ticket forces that design; until then it is illegal
rather than undefined.

## 14. State machine

Enforced by the CLI. Invalid transitions exit non-zero.

```text
triage ──► scoping ──► ready ──► building ──► review ──► done
   │          │                      ▲           │
   │          │                      └───────────┘   findings, rounds++
   ▼          ▼                              │
dropped    dropped                           ▼
                                          blocked ──► (back to prior state)
```

Transition rules:

- `triage → scoping` requires the project's `triage_owner`. Agent-filed tickets
  enter at `triage` and never skip it.
- `scoping → ready` requires `model`, `profile`, `blast_radius`, `reversibility`
  and all four body sections to be non-empty. This is what makes dequeue
  thoughtless-but-safe.
- `ready → building` requires a free worktree and the repo's dispatch slot.
- `building → review` requires a commit on the ticket branch plus a done-note.
- `review → done` requires zero open findings, and the transition runs the merge
  script: the ticket's worktree branch merges to mainline. On conflict the
  transition fails and the ticket returns to `building` with a rebase
  instruction, `rounds` unchanged — resolving a conflict is judgment work and
  belongs to the builder or, at `auto`, the orchestrator; never to the merge
  script (§21).
- `review → building` increments `rounds`. At `rounds == 2` with `model: grok`,
  the CLI **escalates** `model` to `sol` and resets `rounds` rather than
  looping. At `rounds == 5` with any non-grok model, the ticket moves to
  `blocked` with a needs-you reason — no model loops forever. An escalated grok
  ticket can therefore see seven rounds in total — accepted: these are caps, and
  every model in use is decent at breaking to `blocked` early when human input
  is what is actually needed.

## 15. Lifecycle

1. **Capture.** Dave or an agent files a ticket. Agent-filed requires `evidence`
   — a failing test, a review finding, a TODO with a location — and lands in
   `triage`.
2. **Recon (optional, cheap).** grok reads the repo and produces a brief:
   relevant files, prior art, existing patterns, related KB notes. This moves
   expensive context-gathering _out of_ the scoping conversation, which
   addresses pain point 3 directly.
3. **Scoping.** A conversation — with the recon brief and KB hits preloaded —
   produces the four body sections and sets model, profile, blast radius,
   reversibility. Scoping happens in its **own session**, never in the session
   following up in-flight work.
4. **Prioritise.** `triage_owner` sets priority. Because tickets are structured,
   the roadmap is a query, not a document (§17).
5. **Dispatch.** Dispatcher claims a `ready` ticket, creates a worktree, invokes
   the specified model. **One in-flight build ticket per project by default**;
   the project config can raise the slot count (§16).
6. **Build.** Agent works in its worktree, commits, writes a done-note.
7. **Adversarial review.** N independent reviewers with different profiles, each
   **cold** — given the diff, the spec, and the findings ledger, never the
   builder's session. Findings appended to the ledger.
8. **Fix.** Builder starts fresh with diff + spec + ledger. Context is dropped
   between rounds deliberately (§16.2).
9. **Merge and done.** The merge script folds the worktree branch into mainline
   (§14); the ledger becomes episodic material, and curation eventually extracts
   durable lessons from it.

## 16. Parallelism, isolation and context

### 16.1 Isolation

"Never two writers in one repo" is a documented, already-paid-for failure. It is
a hard constraint, not a preference.

- **A git worktree per ticket.** The agent gets its own tree; the dispatcher
  merges. Repos where worktrees do not work get a hard per-repo lock and
  serialised dispatch.
- **Parallelism defaults to across projects, not within.** One in-flight build
  ticket per project unless the project's config raises the slot count. The
  invariant being protected is per-**repo**, not per-project: even with
  worktrees, two agents in one repo still collide at merge time. A project
  running multiple slots therefore needs its orchestrator (§12) deciding what is
  disjoint enough to parallelise, and owning conflict resolution when that bet
  is wrong. Dave is the bottleneck regardless, so multi-slot is reserved for
  `auto`-autonomy projects where he reviews outcomes, not code.
- **The pm repo is itself a shared-write hotspot** — dispatcher, builders moving
  state, N parallel reviewers, Dave — and gets the same discipline, not an
  exemption: every pm write goes through the CLI, and the CLI serialises commits
  under a repo-level lock. Reviewers never append to a ledger directly; they
  return findings to `pm review`, which appends serially. `lab-daemon` (§21.1)
  is a client of the same lock, never a second write path.

### 16.2 Context: within rounds, between rounds, and warm iteration

**Within a single round:** the builder keeps its session warm across turns so it
can iterate without re-reading content. If a preview reviewer is present, it
keeps its own separate warm session — passing artefacts (diffs, feedback) back
and forth with the builder, each reusing its own context via cache. After the
pair converges, a fresh adversarial reviewer enters cold, given only diff + spec

- findings ledger.

**Between rounds:** after adversarial review, context is dropped. Round N+1
starts cold with diff + spec + ledger.

Rejected: a builder or reviewer holding rounds 1–2 becomes invested and patches
instead of reconsidering; a reviewer holding its earlier findings anchors on
them and confirms rather than re-examines, which destroys what makes adversarial
review work. The cold start for adversarial reviewers prevents this.

**The artefact carries the history, not the model.** The findings ledger records
what was claimed, what changed, what was rejected and why. Models can be swapped
between rounds, a human can read it, it feeds the KB.

Whether warm iteration (builder and preview reviewer each keeping sessions open,
passing artefacts) actually beats serializing them with cold starts is the
plan's most load-bearing untested claim. It gets the first formal experiment
(§2, principle 7): before P11 hardens the ledger schema, run real multi-round
builds both ways and instrument both for cost (§16.3, decision 31). The result
either confirms the pattern or points to a better one.

### 16.3 The cost of context: measurement and experiments

Context compounds with depth. Every turn re-sends the accumulated conversation;
caching discounts that but does not eliminate it. This means:

- **Warm sessions are cheaper within a round**, because iteration reuses context
  via cache, avoiding re-reads. This applies to builders and to reviewers with
  their own separate sessions.
- **Cold dispatches are cheaper across rounds**, because each round starts fresh
  at diff-plus-spec-plus-ledger depth, before the conversation compounds. A warm
  continuation drags every prior round's turns through every later turn's
  prefix.
- **Which actually wins** — warm iteration vs. cold serialization, within a
  single agent or across a build-and-review pair — depends on cache hit rates,
  model efficiency, the nature of the work, and the size of diffs relative to
  stable context. This is empirical, not a priori.

**The system measures both.** At P10, per-dispatch accounting lands in the
ledger (§23 P10), recording tokens, turns, and depth per dispatch. Both Claude
and Sol record per-turn usage in their session logs retroactively; Grok does
not, which is the only real instrumentation work. `token-report.py` is the seed
of that accounting.

**The experiments on §16.2's assumptions** — whether warm iteration beats cold
serialization, whether the ledger carries enough for cold fixes — are run on
real work, not simulated. They are scoped before they run (§2 principle 7), with
pre-registered hypotheses and evaluation methods. The cost instrumenting feeds
into deciding which pattern to standardise.

## 17. The dashboard

This addresses the stated main pain point and is the earliest PM deliverable,
because it derives entirely from the pm repo — one checkout renders everything.
It ships read-only at P5 and gains a write path once modules exist (§22.1).

One page, three elements, in order:

1. **The needs-you queue, first.** Every ticket waiting on Dave — a completed
   review awaiting his decision, a blocked ticket with a question, a `triage`
   queue over threshold — sorted by priority. This is the routing signal.
2. **One row per project** — a summary of what is currently happening plus the
   last thing completed:

   | Field        | Example                                            |
   | ------------ | -------------------------------------------------- |
   | Project      | `aven-lang`                                        |
   | Activity     | "coding reference-counted mutable data structures" |
   | Phase        | `building`, round 0                                |
   | Agent        | Sol — last artefact 4m ago                         |
   | Last done    | "parser error recovery" (yesterday)                |
   | Status class | `in-flight`                                        |

   Rows sort by status class: **ready-to-start** (a scoped queue with no agent
   in flight; switching here is productive), then **in-flight** (an agent is
   working; nothing to do), then **idle** (no scoped work; needs scoping).

3. **Nothing needs clearing.** The last-done cell is simply replaced when the
   next ticket finishes, and the full completion history stays a query away
   (`pm list --state done --project X`). No acknowledged/unacknowledged state to
   maintain, no notification debt to accumulate.

"Last artefact 4m ago" is deliberate: elapsed time is measured from the agent's
most recent commit or log write, never from process start — a live process is
not a running agent, and the playbook already paid for that lesson. A **stall
threshold** acts on that number: in-flight with last-artefact age past the
threshold flips the ticket into the needs-you queue. A live-but-silent agent is
exactly the nine-hour-corpse failure, and a board that merely displays the age
still requires Dave to notice it.

The board is pull; needs-you is also **push**: a ticket entering the needs-you
queue fires `bin/notify`. "When work has finished on a project he isn't
currently looking at" is half of the stated main pain point, and a page Dave has
to open does not solve the isn't-looking half.

Generated by `pm board` → HTML through the existing `bin/html-page`, plus
`pm board --text` for the terminal. Regenerated by a post-commit hook and a
timer. It should subsume the reason Dave adopted herdr.

**Forms are the write path, and they create tickets.** A module (§22.1) declares
a field schema, the console renders it, and submitting it creates a ticket
rather than executing anything inline. That keeps the console from growing its
own execution semantics — the ticket is already the unit of work, dispatch
already runs it, results already land as artefacts — and a fast check simply
completes before Dave looks at it. It also means every lookup leaves a dated
record, which is what §6's `asserted` model wants: names get taken, and "was
this free in August?" becomes a question the pm repo can answer.

Roadmap review (pain point 2) falls out of the same mechanism: once tickets are
structured files, the roadmap is
`pm list --project X --state ready,scoping --sort priority`, and prioritising is
a sort, not a re-read of prose.

## 18. Review profiles and parallel investigation

### 18.1 Profiles, not personalities

Telling a model it is a Rust expert changes its confidence, not its output. What
changes output is different context, tools and success criteria. A profile is
therefore a bundle:

```yaml
name: rust
model: any
kb_scope: [scope:global, project:clex, type:reference]
prompt: profiles/rust.md
checklist: profiles/rust-checklist.md
tools: [read, grep, bash:cargo]
```

Profiles earn their keep in **review** and **investigation**, where decorrelated
opinions are the product. They buy little during building.

### 18.2 Parallel investigation replaces the round table

A round table where N agents share one context is expensive, and later speakers
anchor on earlier ones — destroying the decorrelation that was the point.

Instead: the same brief goes to N agents, each writes a position **without
seeing the others**, and a synthesis pass produces a comparison naming
agreements, disagreements and cruxes. Cheaper, actually parallel, decorrelated,
and it leaves a reviewable artefact. This is `agent-playbook.md`'s "A/B on
diagnoses, not implementations" generalised.

Triggered by a `type: investigate` ticket entering scoping, not by asking. The N
dispatches go out as one burst rather than sequentially — it is the simplest way
to guarantee none of them can see another's position, which is the whole point
of the design.

**This process is itself a subject for the KB.** Which elicitation shapes
produce the most valuable information is an empirical question worth recording:
positions as episodic material, conclusions about _what worked_ as durable
`type: lesson` notes.

## 19. Assisted mode

`type: assisted` — Dave builds; agents research, produce options, write tests
and review his code. A first-class type, not an exception, because a system that
only knows how to dispatch build work will fight him on `aven-lang`, which is
the project he most wants to build himself.

## 20. Making grok valuable

Unused grok credit is real capacity. A cheap, plentiful model is most valuable
where work is **parallel, repetitive and cheaply verifiable** — a false positive
costs a moment to dismiss, whereas a false-positive _code change_ costs a review
cycle.

Fitting jobs: the curation pass; adversarial review at volume (N profiles on one
diff, union of findings, Claude triages); pre-scoping recon; eval runs;
doc-drift and link-integrity checks; TODO harvesting; `asserted`-staleness
rechecks (§6), escalating only on contradiction.

**grok does breadth, Sol does depth, Claude does judgment.** Route by task
_kind_, not only by ticket size. Breadth work goes to grok rather than being
absorbed by a more capable model just because it is open. Model choice is set
during scoping (§14), so this is a human decision with a recorded rationale, not
an autonomous router guessing at dequeue time. Whether a given pattern actually
saves cost compared to the alternative is measured and recorded (§16.3).

---

# Part III — Tooling

## 21. The determinism rule

Dave's critique, and it is correct: agents are too keen to do work better left
to a dumb script. Every deterministic operation must be a script the agent
calls.

> **If the output is fully determined by the input, it is a script. Agents are
> for judgment.**

Consequences:

- **The CLI owns all frontmatter writes.** Agents never hand-edit structured
  fields. Marking a ticket done is `pm move <id> done`, which validates the
  transition, stamps `updated`, and rewrites the file deterministically.
- **Body prose is free-form**; frontmatter is not.
- **One schema file** drives the linter, the CLI's validation, and the generated
  documentation. There is no second place where field names are written down.
- **Deterministic output**: stable key order, stable sort, idempotent writes, so
  diffs are clean and re-running changes nothing.
- **`--json` on every read command** for agent consumption; human-readable by
  default.
- **Exit codes are the contract.** Agents branch on them.
- **Merging is a script; conflict resolution is judgment.** `pm merge` performs
  the mechanical merge and exits non-zero on conflict; deciding how to resolve
  belongs to the builder or the orchestrator, never to the script.
- **Lint runs in pre-commit and CI**, both repos.
- **Anything an agent does twice becomes a script.** Recurring manual agent work
  is a bug report against the tooling.

What agents may **not** do: choose ticket IDs, compute priority or sort order,
mark a ticket done without a ledger entry, edit another agent's worktree, write
another agent's ledger entries, or promote anything to public.

### 21.1 Serialisation and `lab-daemon`

The serialisation invariant is a **lock in the CLI**, not a service: every kb
and pm write path takes a repo-level flock before committing. That holds with no
process running, costs nothing, and keeps the CLI dependency-free — nothing in
the system requires anything to be up.

On top of it — at P10, not before — sits **`lab-daemon`**: a local daemon in the
psst/aic-edit mould — a long-running process behind a Unix socket that the CLI
and hooks can talk to. It owns what needs a clock or a persistent view: watching
registry backlogs and launching curation batches, dispatching ready tickets on
`auto` projects, the stall-threshold alarms (§17), firing `bin/notify`, serving
the web board. Unlike psst it holds no secrets; its socket is a convenience, not
a security boundary.

The generic name is deliberate and is the correct application of §2 principle 8,
not an exception to it: the process does five unrelated jobs, so naming it for
any one of them would be a lie, and "daemon" states exactly what it is.

`lab-daemon` is a **client** of the CLI and its locks, never a second write path
and never the serialisation mechanism. When it is down, every operation still
works by hand, and nothing can corrupt state by racing it.

Console forms (§17) make the daemon the **browser's write path**, which does not
change that rule but raises its stakes. A form POST is validated against the
module's field schema and then executed as `pm new`, taking the same flock as
every other write; the daemon never edits a ticket file itself. With the daemon
down, forms are unavailable and the CLI is untouched — degraded, not broken.

## 22. Command surface

Two binaries, `kb` and `pm`, in one public cargo workspace (§4) with shared
crates for schema, frontmatter, lint and retrieval — revised from rev 2's one
binary: the tools are public code while the vaults they operate on stay private,
and the two lifecycles deserve separate releases. Written in Rust (decided —
§25) to feed on the Rust knowledge accumulating in other projects. The schema
lives in a data file — originally to keep P0–P4's churn out of recompiles, and
now for a second and larger reason: **schema handling is built as a general
mechanism, not a ticket-specific validator.** A schema describes fields; a
validator checks a record against one; a renderer draws a form from one. Built
that way at P4, console ticket creation and module forms (§22.1) both come free;
built as "validate a ticket", each needs parallel machinery later. It is the
same work either way, and only cheap if it is decided before P4 is written.

**The schema will move for the whole of P0–P8, so migration is a tool rather
than a chore.** Fields will be renamed, added, split and dropped as the first
hundred notes teach what the schema got wrong, and every one of those changes
would otherwise mean opening every note by hand — which is exactly the work §2
principle 1 says belongs in a script. So `kb lint` ships with `--fix` from P1,
and a schema change is expressed as a migration the tool applies across every
registered source at once. Two consequences worth stating: a rule is only worth
adding to the schema if its violation can be described precisely enough to
detect, which is a useful discipline on the schema itself; and rules divide into
the mechanically fixable (a missing default, a renamed field, a link that needs
its source prefix) and the ones needing judgment (a missing `source:`), so lint
reports the second kind and repairs the first rather than pretending both are
errors of the same kind. This is the other half of the argument for the schema
being data (decision 37): autofix against hardcoded rules means writing a
bespoke migration each time, where autofix against a described schema can derive
most of them.

```sh
# knowledge base
kb lint [--fix] [--explain]        # schema, links, orphans, index sync
kb migrate [--dry-run]             # apply a schema change across every note
kb index [--incremental]           # rebuild qmd collections from registry
kb search QUERY [--json]           # hybrid retrieval
kb capture [--from FILE]           # write path: dedup gate, provenance, lint
kb promote NOTE --to public        # boundary-checked; refuses without denylist
kb curate SOURCE                   # dispatch a curation batch
kb bench                           # eval fixture -> precision@k, MRR

# project management
pm new PROJECT TITLE               # allocates id, writes skeleton, state=triage
pm show ID [--json]
pm set ID FIELD=VALUE              # validated against schema
pm move ID STATE                   # validated against the state machine
pm list [--project P] [--state S] [--sort priority] [--json]
pm board [--text]                  # dashboard -> HTML via bin/html-page
pm next [--project P]              # highest-priority ready ticket
pm dispatch ID                     # worktree + lock + invoke the ticket's model
pm merge ID                        # merge ticket branch; non-zero on conflict
pm review ID [--profile P ...]     # fan out N cold reviewers, collect findings
pm ledger ID add                   # append a round record
pm investigate ID --agents N       # parallel positions + synthesis
pm lint                            # schema, states, dependency cycles, orphans
```

Everything above is a script. The agent's job is to decide _which_ to call and
to write the prose that goes in the bodies.

### 22.1 Modules

A module teaches the system somewhere to look and something to ask. The
motivating case is name selection: propose candidates, then check whether the
name is free on npm, crates.io, PyPI, the domain registries, GitHub orgs. Today
that means an agent inventing endpoints, guessing at response semantics, and
cheerfully misreading a 200-with-empty-body as "taken". A module makes it
declarative, which is §2 principle 1 pointed outward: the judgment — is this a
good name? — stays with the agent, and the lookup stops being judgment at all.

A module contributes three things, and the load-bearing property is that most of
it is **declaration rather than code**:

| Contribution | What it is                                           |
| ------------ | ---------------------------------------------------- |
| **Targets**  | Where to look, and how to read the answer            |
| **Tools**    | Verbs an agent calls, which execute those targets    |
| **Forms**    | A field schema the console renders and submits (§17) |

**A form submission creates a ticket.** It does not execute inline. The ticket
is already the unit of work, `pm dispatch` already runs it, and results already
land as artefacts that feed curation — so the console needs no execution
semantics of its own, and a fast check simply completes before anyone looks at
it. Forms are therefore not a special case: they are structured ticket creation,
which the console needs anyway, generalised.

**Namespacing is settled now**, because collisions cost nothing to prevent and a
lot to unwind: every module-contributed target, tool, form and note type is
qualified by its module's name.

**A module is where the leak gate leaks (§7).** It runs with Dave's credentials,
can see kb-priv, and can talk to the network — so a module that reads a private
note and posts it to an availability API walks straight around the in-process
denylist scan every other write path takes. `cargo-*`-level trust — you
installed it, you own the consequences — is defensible for modules Dave writes
and not for modules other people write. That is the constraint that decides the
module language.

#### The module language is aven

Modules must express targets, predicates over responses, and field schemas. The
cheap answer is TOML plus a small expression string
(`available_when = "status == 404"`), which means inventing an expression
language badly — the road that produced Starlark, CUE, Rego, Dhall and HCL, each
because a config format grew a brain one feature at a time.

**aven fits this slot better than a home-grown mini-language, and better than it
first appears.** It describes itself as a typed glue language with a type-safe
host/script boundary, which is the role exactly. It is written in Rust, so
embedding it in the `lab` workspace is a path dependency rather than an FFI
project. `aven-host` already registers HTTP and JSON capabilities, so a target
declaration is directly expressible today. Row-polymorphic records are the right
shape for schemas a module extends without the host knowing them in advance. And
the tooling grew ahead of the runtime, so an agent writing a module gets a
formatter, an LSP, `aven explain <code>` and machine-readable diagnostics — an
unusually good story for agent-authored code in a language with no training data
behind it.

The security argument is the strongest one. A host-provided capability boundary
is precisely the answer to the trust problem above: a module receives the
capabilities the host hands it and nothing else, so "may this module reach
kb-priv, and may it reach the network" becomes a host decision rather than an
act of faith. A subprocess module cannot offer that; a sandboxed evaluator can.

**Sequencing keeps aven off the critical path.** P1–P4 use a closed, built-in
set of validation rules and no expression language at all, so §23's "P1 before
everything" holds and lint never waits on a language under reconstruction. aven
arrives at P10b with modules — the first point at which anything genuinely needs
open-ended third-party expression. If it is not ready then, TOML plus a fixed
predicate set remains available and nothing upstream has been blocked.

What the slot demands, offered as a checklist against aven's roadmap rather than
a claim about its current state:

- **Per-invocation capability scoping** — the host granting one module
  HTTP-to-crates.io and another no network at all, rather than one global
  platform. This decides whether third-party modules are safe. _Checked against
  the source; see decision 34._ Granting is already per-`Host`, so the
  per-invocation half holds; scoping **within** a capability is the gap, and
  closing it is aven-side work that must precede P10b.
- **Bounded evaluation** — a module expression that loops forever is a denial of
  service against the daemon. The existing recursion-depth-guard work is the
  right shape.
- **Deterministic evaluation** in the schema and validation role, because §21
  requires idempotent writes and clean diffs.
- **A stable embedding API** across aven's rebuild, since `lab` would depend on
  `aven-eval` and `aven-host` by path.

Inadequacies found here are worth finding rather than worth avoiding: a module
system is a glue problem and aven is a glue language, so if it cannot yet
express "declare some HTTP targets, read the status, return a record", that is a
small and well-aimed gap.

## 23. Phases

Two tracks. The KB track is a prerequisite for the PM track's quality but not
for its existence; the dashboard is deliberately pulled early because it attacks
the top pain point and depends only on ticket files existing.

| Phase | Track | Work                                                                                                                                                                                                                                                      | Done when                                                                                      |
| ----- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| P0    | KB    | Create the vault repos (kb **private until P8**, kb-priv, pm), write `SCHEMA.md`, hand-migrate `agent-playbook.md` into ~15 notes, absorb `~/style-guide`, seed `kb-priv/policy/denylist.txt` from terms seen while migrating. No automation. | The index reads usefully and the denylist has real terms in it; the notes become P1's lint fixture, checked retroactively |
| P1    | KB    | The tools workspace repo; `kb lint --fix`, `kb migrate` and index generation, with the schema as a data file and a closed built-in predicate set (§22) — `SCHEMA.md` becomes generated. Autofix and migration are P1 scope, not later polish. | A schema change is applied across every note by one command, not by hand |
| P2    | KB    | qmd, registry, collections, eval fixture.                                                                                                                                                                                                                 | `kb bench` runs; hybrid measured against keyword-only                                          |
| P3    | KB    | Surfacing: index pointers, prompt-submit hook with relevance floor, `kb` skill.                                                                                                                                                                           | A session surfaces a forgotten note                                                            |
| P4    | PM    | Ticket schema, `pm new/show/set/move/list/lint`, state machine. Schema handling built as a general mechanism — validator plus form renderer — not a ticket-specific validator (§22). Tickets by hand only.                                                | A ticket cannot enter an invalid state, and the same schema code validates a non-ticket record |
| P5    | PM    | **Dashboard**, read-only. `pm board` → HTML.                                                                                                                                                                                                              | Dave says the board answers "where am I up to?"                                                |
| P6    | KB    | Cross-harness skills: canonical bodies, generator, stubs, `send-email` dedup, drift CI.                                                                                                                                                                   | One edit is live in all four harnesses                                                         |
| P7    | KB    | Write path: `kb capture` with dedup gate.                                                                                                                                                                                                                 | Capture reliably offers the right note to edit                                                 |
| P8    | KB    | **Leak defense — blocks all public writes, and the phase kb goes public in.** Denylist (seeded at P0), allowlist gitignore, pre-push gate + in-process scan in `kb promote`, kb-priv tripwire CI, public generic CI, then the full-history scan and flip. | A canary term is refused at push; one predating its term trips kb-priv CI; kb is public        |
| P9    | KB    | `kb curate` on clex. Manual runs before scheduling.                                                                                                                                                                                                       | Two consecutive batches need no correction                                                     |
| P10   | PM    | `pm dispatch` + `pm merge`: worktrees, per-project slots, orchestrator autonomy levels, model invocation. `lab-daemon` (§21.1): scheduling, stall alarms, notifications, board serving. Per-dispatch token and cost accounting into the ledger (§16.3).   | A ticket builds end-to-end unattended                                                          |
| P10b  | Both  | **Modules** (§22.1): manifest format, namespacing, target/tool/form declarations, aven as the module language with host capability scoping, console form rendering, and submit-creates-ticket.                                                            | The name module runs end-to-end from a console form                                            |
| P11   | PM    | `pm review` with profiles + findings ledger; escalation rules. Preceded by the warm-vs-cold ledger experiment (§16.2).                                                                                                                                    | A grok ticket escalates to Sol on its own                                                      |
| P12   | PM    | `pm investigate`: parallel positions + synthesis.                                                                                                                                                                                                         | An investigation ticket produces a crux document                                               |
| P13   | Both  | Register remaining sources; schedule curation; project guidelines and diagrams.                                                                                                                                                                           | Unattended for a month                                                                         |

Hard ordering constraints:

- **P8 before any public promotion, and P8 is where kb becomes public at all.**
  Until it lands, curation runs with public promotion disabled entirely and
  every write to kb is a hand-reviewed human commit — no agent touches kb. kb
  being private for that window makes the constraint recoverable rather than
  merely stated: a mistake before P8 is a rewrite of a private history, which is
  a chore, not a disclosure.
- **P4 before P5.** The dashboard is a view; it needs something to view.
- **P10 before P10b.** Forms need the daemon to serve them and dispatch to run
  what they create. The module layer is separable: it can slip past P11–P13
  without blocking them.
- **P1 before everything.** The lint is what prevents drift, and drift is
  unrecoverable once 500 notes exist — but it is `--fix` and `migrate` that make
  the schema safe to keep changing while the corpus grows, which is why they are
  in P1 and not after it (decision 41).

## 24. Risks

- **Sibling-clone coupling.** kb's pre-push gate depends on the kb-priv checkout
  for the denylist. _Mitigate:_ the gate fails closed when the denylist is
  absent; the path to it is configuration, not convention.
- **Digest and ledger fabrication.** Both are agent-written prose _about_ a diff
  and can be confidently wrong. _Mitigate:_ Claude's review checks claim against
  cited source, never the summary alone; 100% spot-check until two clean
  batches.
- **Curation promotes noise.** 500 done-notes contain much restated obvious, and
  a KB of mediocre lessons is worse than a small one because it poisons
  retrieval. _Mitigate:_ batch cap; multi-source provenance required for
  `type: lesson`; eval fixture re-run per batch as a regression test.
- **Provenance laundering.** Surfacing (§9) manufactures corroboration: an agent
  shown a note restates it in its done-note, and the restatement then poses as
  an independent second source — the better surfacing works, the more echoes.
  _Mitigate:_ the review pass audits independence (§11): a source counts only if
  it predates the note or adds observational detail; `asserted` bumps require
  new evidence, not restatement.
- **Stub drift across four harnesses.** _Mitigate:_ DO-NOT-EDIT header, CI
  regenerate-and-diff, stubs short enough that editing one is obviously wrong.
- **Index staleness.** A lagging index gives confidently outdated answers.
  _Mitigate:_ post-commit incremental reindex of the **search** index; the
  **index slice** refreshes at curation-batch boundaries instead, since it is
  read at session start and a mid-session change reaches nobody;
  `kb index --status` surfaces both on the dashboard, because the slice is the
  one that can lag by days.
- **Sessions compound, and the tradeoff between warm and cold is not obvious.**
  Caching makes warm sessions cheaper within a round; cold starts make
  independent rounds cheaper. Which pattern actually costs less depends on cache
  efficiency, model capabilities, and work characteristics — all of which vary.
  _Mitigate:_ per-dispatch accounting in the ledger from P10; parallel
  experiments on warm-vs-cold and build-and-preview patterns before deciding
  which to standardise (§16.2).
- **P5 grows a write path.** The dashboard was the cheap early win — generated
  HTML, read-only, one checkout. Forms (§17, §22.1) turn it into a form renderer
  with a POST handler, which is meaningfully more, and the
  tooling-becomes-a-project risk below applies to it directly. _Mitigate:_ the
  renderer is generic and schema-driven, built once, so modules add declarations
  rather than UI code; P5 ships read-only and the write path lands with the
  daemon at P10.
- **A module is a hole in the leak gate.** It runs with Dave's credentials, can
  read kb-priv, and can reach the network, walking around the in-process scan §7
  relies on. _Mitigate:_ the host capability boundary (§22.1) is the mechanism
  rather than trust in module authors — a module gets what the host grants and
  nothing else, and one asking for kb-priv reads plus network is refused rather
  than reviewed.
- **`lab` gains a dependency on a language under reconstruction.** aven is
  experimental and its embedding API will move. _Mitigate:_ sequencing — P1–P4
  use a closed built-in rule set, so aven gates only the module layer (P10b) and
  never lint; TOML with a fixed predicate set remains the fallback if aven is
  not ready.
- **Public history is permanent.** _Mitigate:_ kb stays private until P8, so the
  phases with no leak gate are recoverable; small reviewed promotion batches
  after that; curation may never force-push or rewrite public history.
- **Promotion breaks inbound cross-source links.** Moving a note from a project
  into kb invalidates every `[[project:name]]` pointing at it, in repos the
  promoting checkout may not have. _Mitigate:_ promotion leaves a tombstone in
  the origin — a stub whose frontmatter carries `moved_to: kb:<name>` and no
  body — so old links resolve through it and lint can rewrite lazily wherever it
  next sees both sources. A tombstone is cheap; a cross-repo rewrite of repos
  you do not have is not possible.
- **The system raises output without raising review capacity.** The stated
  bottleneck is Dave. _Mitigate:_ the digest, the ledger and Claude-as-first-
  reviewer are load-bearing, not conveniences; the dashboard sorts by needs-you;
  per-project single-slot dispatch caps in-flight work; curation is
  demand-driven by backlog (§5), not a timer that out-runs the reviewer.
- **Tooling becomes a project of its own.** Fourteen phases is a lot of
  scaffolding for one person. _Mitigate:_ P0–P5 is a complete, useful system on
  its own; stall there if momentum goes.

## 25. Decisions and remaining open questions

Resolved during the rev-2 review (Dave + adversarial pass), with rationale in
the sections cited:

1. **The three-tier split stays.** The index is generated and episodic material
   stays in place; the only ceremony is durable-tier discipline, which is the
   product, not overhead. _The T0/T1/T2 labels are superseded by rev-4
   decision 24._
2. **No submodules.** kb is a sibling clone registered in the registry like any
   project source (§4).
3. **The curation pass's drafting pipeline is an experiment, not a decision.**
   Start grok-drafts + Claude-reviews, measure the correction rate over two
   batches, and escalate to candidate-extraction or blind-draft synthesis only
   if the data demands it (§11). Constant process experimentation is now a
   governing principle (§2, principle 7).
4. **Findings-ledger sufficiency gets measured, not assumed.** Warm-vs-cold A/B
   on the next real multi-round fix, before P11 (§16.2).
5. **Web-harvested content is quarantined** in its own collection, outside the
   default search scope; Dave promotes deliberately (§8).
6. **Written in Rust** (§22). _The one-binary half is superseded by rev-3
   decision 17._
7. **The dashboard shows the needs-you queue, current activity and last-done per
   project.** Nothing needs acknowledging or clearing (§17).
8. **Dispatch slots and orchestrator autonomy are per-project config** —
   `suggest` for projects Dave builds himself, `auto` ("go wild") for projects
   where he reviews outcomes rather than code (§12, §16.1).
9. **Tickets live in a central private `pm` repo**, not in project repos and not
   inside kb-priv (§4).
10. **`publication` is a per-source guard**: marking a source `public` adds
    write-time safeguards because its content is destined for publication (§5).
11. **Escalation:** grok hands to Sol at 2 rounds; any non-grok model blocks
    with needs-you at 5 (§14). Seven total rounds for an escalated grok ticket
    is accepted; models are trusted to break to `blocked` early.

Resolved during the rev-3 review:

12. **The leak gate moves to push time.** Pre-push full-tree scan plus the same
    scan in-process in `kb promote` and `kb curate`; kb-priv CI is renamed what
    it is — a post-publication tripwire that pages. The PR-with-required-check
    hardening is deferred until any agent has push access to kb (§7).
13. **Curation reads snapshots and writes in its own checkout.** No locks, no
    dirty-tree refusal, no starvation of active repos; pm's ledgers can be
    curated. Cadence is demand-driven by per-source backlog threshold
    (placeholder 20), not a timer (§5, §11).
14. **The index is generated per scope**; the ~200-line cap applies per slice
    (§3, §9).
15. **pm writes are CLI-owned and flock-serialised**; reviewers return findings
    to the CLI, which appends to ledgers serially (§16.1).
16. **A daemon lands at P10** as a client of the same CLI and locks, owning
    scheduling, dispatch on `auto` projects, stall alarms, notifications and
    board serving. The flock is the invariant; the daemon is additive and
    optional (§21.1). _Named by rev-4 decision 26._
17. **Two public binaries, `kb` and `pm`, in one cargo workspace repo**; the
    vaults they operate on stay private. kb-priv and pm are hosted as GitHub
    private repos — acceptable sensitivity today, revisit if that changes — with
    redundancy via clones on two always-on tailscale-reachable machines (§4,
    §22). _The repo is named by rev-4 decision 25._
18. **`model: any` is resolved by the orchestrator at dispatch** from quota and
    ticket complexity/impact, rationale recorded on the ticket (§12).
19. **Public notes link only to public notes**; lint enforces within-boundary
    resolution (§6).
20. **Multi-repo tickets are lint-forbidden** until a real one forces the design
    (§13).
21. **Experiments are scoped before they run** — stated hypothesis,
    pre-registered evaluation; individual designs are future work (§2 principle
    7).
22. **Staleness rechecks are grok work**, escalating to needs-you only on
    contradiction (§6, §20).
23. **The dashboard pushes**: entering needs-you fires `bin/notify`; a stall
    threshold on last-artefact age routes silent in-flight tickets into
    needs-you (§17).

Resolved during the rev-4 review:

24. **Names state their referent; no term requires a glossary** (§2 principle
    8). Dave's call, and the reasoning is about agents rather than taste: a
    metaphor name arrives with the wrong priors pre-loaded, a person-role name
    leaks entailments into behaviour, and a glossary cannot repair either
    because it is optional context — when it fails to load, the agent falls back
    on the wrong prior silently. Renames forced by this: the tiers are `index` /
    `durable` / `episodic` rather than T0/T1/T2 (§3); `lens` becomes `profile`
    (§12, §13, §18); "the gardener" becomes the curation pass, verb `kb curate`,
    branch `curate/<date>` (§11); §6's "constitution" is just the schema. Terms
    deliberately kept because they are dead metaphors with settled technical
    meanings: `blast_radius`, `watermark`, `orchestrator`, `quarantine`,
    `promote`, `gate`.
25. **The tools workspace repo is `lab`** (§4, §22) — a place where experiments
    are run, knowledge accumulates and things get built, which is what the
    system is. `dbalmain/lab`. The name is deliberately unexciting while the
    whole thing is an experiment; a more distinctive one can be earned later if
    it works out and is worth promoting to others.
26. **The daemon is `lab-daemon`** (§21.1). Generic on purpose: it schedules,
    dispatches, alarms, notifies and serves, so naming it for any single job
    would misdescribe it.

Resolved during the rev-5 review:

27. **Caching matters more than cold-start costs.** When the same content is
    read repeatedly across short sessions, it stays cached; when context
    compounds within a long session, every turn pays the prefix cost. The
    tradeoff between warm and cold is empirical, not a priori, and depends on
    cache efficiency and work characteristics (§2 principle 9, §16.2, §16.3).
28. **Builder and preview reviewer keep separate warm sessions.** Each reuses
    context via cache, avoiding re-reads on their own turns. They pass artefacts
    (diffs, feedback) to iterate. After convergence, a fresh adversarial
    reviewer enters cold. Whether this beats serializing them all cold is the
    first experiment (§16.2).
29. **Per-dispatch accounting lands at P10**, into the ledger and onto the
    board. Claude and Sol record it retroactively in their session logs; **grok
    records nothing**, which is the only real instrumentation work. Cost is
    measured by dispatch so experiments on warm-vs-cold and build-and-review
    patterns can be compared (§16.3, §23 P10).

Resolved during the rev-6 review:

30. **A module contributes targets, tools and forms, and is mostly declaration
    rather than code** (§22.1). The lookup half of name-checking stops being
    agent judgment, which is §2 principle 1 pointed outward. The escape hatch
    for awkward targets is the module language, not an arbitrary subprocess.
31. **A form submission creates a ticket** rather than executing inline (§17,
    §22.1). The console grows no execution semantics of its own, results land as
    artefacts that feed curation, and every lookup leaves a dated record for
    §6's `asserted` model.
32. **Schema handling is general at P4, not a ticket-specific validator** (§22,
    §23 P4). A schema describes fields, a validator checks a record, a renderer
    draws a form. The same work if decided before P4 is written; parallel
    machinery if not.
33. **aven is the module language, arriving at P10b and not before** (§22.1). A
    typed glue language with a host capability boundary answers both the
    expression problem and the module-trust problem at once, and embedding is a
    path dependency since both are Rust. P1–P4 use a closed built-in rule set so
    aven never gates lint, and TOML with fixed predicates stays the fallback.
    Module namespacing is settled now to avoid an unwind later.

Still open — the plan proceeds on these assumptions and tests them:

1. **Does warm iteration (builder and preview reviewer keeping sessions open)
   cost less than serializing them with cold starts?** First formal experiment
   (§16.2). Run the next real multi-round build both ways and compare costs from
   the ledger.
   > Dave: Run it twice and evaluate the output.
2. **Is hybrid retrieval worth it at this corpus size?** P2's eval fixture
   answers this empirically. The corpus is expected to grow quickly (daily
   debrief, curated projects), which is why the plan assumes hybrid — but the
   assumption is checked before tuning anything (§8).
   > Dave: I'm happy to try without hybrid retrieval and introduce when it
   > becomes a pain-point.
3. **How much do the patterns chosen (warm iteration, session length, cache
   loading) actually affect cost?** No amount of reading session logs answers
   counterfactuals like tighter briefs or smaller scope per dispatch. Parallel
   experiments are needed, waiting for `pm dispatch` (P10).
   > Dave: We'll need parallel experiments — wait until we have things built and
   > run the experiments then. _(Question 4, on aven capability scoping, was
   > answered by inspection and is now decision 34.)_

Resolved during the rev-7 review:

34. **Capability grants are per-invocation already; scoping within a capability
    is aven-side work that precedes P10b** (§22.1). `Host::new()` is empty and
    every capability is opt-in, so a module invocation gets its own `Host`
    carrying only its grants — and a module importing an ungranted capability is
    refused with a check-time diagnostic naming the missing capability, before
    it runs. What does not exist is scoping _within_ a grant: `register_http`
    allows every URL and `register_files` the whole filesystem, so "crates.io
    only, no filesystem" is inexpressible. Both are one interception point each
    and additive — scoped registration variants taking a policy, touching
    neither the checker nor the language. Since aven is ours to improve, the
    answer is to build it rather than weaken the trust model; the path allowlist
    must canonicalise before checking, or `..` and symlinks walk straight out of
    it.
35. **kb is created private and flips to public at P8** (§4, §23). The
    asymmetry decides it: public history is permanent and the remedy is a rewrite
    plus a disclosure, where a late flip costs one full-tree-and-history scan the
    plan already specifies for repos opening late. P0–P7 is exactly the window
    with no denylist, no pre-push gate and no tripwire CI, and the first content
    in is the playbook, which is the most incident-dense text in the corpus.
    Notes are written as though public throughout; the private window is an undo,
    not a relaxation.
36. **The denylist is seeded at P0, not P8** (§23). P8 builds the scanning
    machinery; the list itself is a text file, and its terms are visible exactly
    once — while a human reads the playbook and the style guide line by line to
    migrate them. Reconstructing them seven phases later means recalling what was
    in a migration long finished. P8 then starts from a real list and its canary
    test has something to test against.
37. **The note schema is a data file from P1, and `SCHEMA.md` is generated from
    it** (§22, §23 P1). Decision 32's argument applies one phase earlier than it
    was written: `kb lint` is the first schema consumer, so hardcoding rules at P1
    buys a rewrite at P4 plus a drift window where the prose and the code
    disagree. A closed built-in predicate set keeps aven out of P1 as decision 33
    requires. P0 is unaffected — prose and fifteen hand-written notes, no
    machinery.
38. **Names are unique per source, and cross-source links carry the source
    prefix** (§6). The namespace was never two vaults: every project keeps its own
    tool-specific knowledge while its general knowledge is promoted to kb, so the
    registry's source list _is_ the namespace list and it grows with every
    project. `[[name]]` is within-source and must resolve; `[[source:name]]`
    crosses and resolves only where that source is checked out. Public sources
    may link only public sources, which is the old visibility rule surviving
    intact. The cost is that promotion breaks inbound links, paid with a tombstone
    (§24).
39. **`INDEX.md` is committed, and refreshed off the commit path** (§9, §23 P1).
    Committing it is what lets a fresh clone and a harness without the binary read
    the tier at all, which is the tier's entire purpose. The churn is real, so
    regeneration happens at curation-batch boundaries and on demand — §24's
    position already, since a mid-session refresh reaches nobody — and CI
    _checks_ freshness rather than pre-commit _writing_ it. Auto-writing on commit
    is what manufactures the conflicts.
40. **YAML frontmatter with TOML for configuration, deliberately** (§5, §6).
    YAML frontmatter is what the existing memory files carry and what the harness
    itself writes, so switching costs a migration and breaks writes the system
    does not control; TOML is right for hand-edited config with no document body.
    Two parsers is not a burden worth a migration to avoid.
41. **The schema is expected to stay fluid through P8, so `kb lint --fix` and
    `kb migrate` are P1 scope rather than later polish** (§22, §23 P1). Dave's
    call, and it inverts how the phase reads: P1's product is not a checker but a
    schema-change tool that happens to check. The first hundred notes will teach
    what the schema got wrong, and a rename that means opening every note by hand
    is the tax that stops the schema improving — the fluidity is only affordable
    if migration is a command. Two things follow. A rule earns its place in the
    schema only if its violation is describable precisely enough to detect, which
    disciplines the schema itself; and lint sorts rules into the mechanically
    repairable and the ones needing judgment, fixing the first and reporting the
    second rather than treating them alike. This is also the second and stronger
    argument for decision 37: autofix against hardcoded rules means hand-writing
    each migration, where autofix against a described schema derives most of
    them.

---

## Appendix A — Findings from inspecting the current setup

Independent of the plan; worth acting on regardless.

- **`send-email` exists twice** — `~/.claude/skills/` and
  `~/w/ai-tools/skills/`. Verified byte-identical on 2026-08-10: dedup is free
  today and stops being free at the first divergent edit.
- **`~/w/ai-tools/nohup.out`** — 33 KB of captured agent output in a repo with a
  GitHub remote, matched by no ignore rule. Untracked today, one `git add -A`
  from committed, which is the playbook's own warning.
- **The 65 memory files** under `~/.claude/projects/*/memory/` already use
  name/description/type frontmatter and `[[links]]`. They are the closest thing
  to the target schema and the best durable-tier seed after the playbook — but
  they are Claude-only today, which is the problem §10 solves.
