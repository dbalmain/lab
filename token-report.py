#!/usr/bin/env python3
"""Per-CLI token accounting from local session logs.

Reads what each agent CLI already records on disk. No instrumentation, no
proxy, works retroactively over any past day.

  ./token-report.py              # today
  ./token-report.py 2026-08-23   # a specific day

Sources:
  claude  ~/.claude/projects/*/*.jsonl   per-message `usage` object
  codex   ~/.codex/sessions/Y/M/D/*.jsonl  `token_count` events (cumulative
          per session, so the last one per file is the session total)
  grok    not available -- grok's session store keeps prompt history only,
          with no token accounting. Needs a proxy or an upstream change.

Caveats worth remembering when reading the output:
  - Claude's `input_tokens` EXCLUDES cache reads/writes; codex's includes them.
    The report normalises this.
  - Cache reads are summed per request, so the same prefix is counted on every
    turn. That is correct for billing and wrong for "distinct bytes ingested".
  - Token counts are not costs. Vendors price input, cached input, output and
    reasoning differently, and quota windows are per-plan. Compare ratios and
    hit rates across CLIs, never raw percentages of different quotas.
"""
import json, glob, os, sys, datetime

day = sys.argv[1] if len(sys.argv) > 1 else datetime.date.today().isoformat()


def claude(day):
    ci = cc = cr = co = msgs = 0
    sessions = set()
    for f in glob.glob(os.path.expanduser("~/.claude/projects/*/*.jsonl")):
        for line in open(f, errors="ignore"):
            if day not in line:
                continue
            try:
                d = json.loads(line)
            except ValueError:
                continue
            if not str(d.get("timestamp", "")).startswith(day):
                continue
            u = (d.get("message") or {}).get("usage")
            if not u:
                continue
            ci += u.get("input_tokens", 0)
            cc += u.get("cache_creation_input_tokens", 0)
            cr += u.get("cache_read_input_tokens", 0)
            co += u.get("output_tokens", 0)
            msgs += 1
            sessions.add(f)
    return dict(label="claude", sessions=len(sessions), calls=msgs,
                input_total=ci + cc + cr, cached=cr, cache_write=cc,
                output=co, reasoning=None)


def codex(day):
    y, m, d_ = day.split("-")
    i = c = w = o = r = sessions = 0
    pat = os.path.expanduser(f"~/.codex/sessions/{y}/{m}/{d_}/*.jsonl")
    for f in glob.glob(pat):
        last = None
        for line in open(f, errors="ignore"):
            if '"token_count"' not in line:
                continue
            try:
                t = json.loads(line)["payload"]["info"]["total_token_usage"]
            except (ValueError, KeyError):
                continue
            last = t
        if last:
            sessions += 1
            i += last["input_tokens"]
            c += last["cached_input_tokens"]
            w += last.get("cache_write_input_tokens", 0)
            o += last["output_tokens"]
            r += last.get("reasoning_output_tokens", 0)
    return dict(label="codex", sessions=sessions, calls=None, input_total=i,
                cached=c, cache_write=w, output=o, reasoning=r)


def quota():
    """codex reports its own remaining quota; surface the freshest reading."""
    y, m, d_ = day.split("-")
    latest = None
    for f in glob.glob(os.path.expanduser(f"~/.codex/sessions/{y}/{m}/{d_}/*.jsonl")):
        for line in open(f, errors="ignore"):
            if '"rate_limits"' not in line:
                continue
            try:
                rl = json.loads(line)["payload"]["rate_limits"]
            except (ValueError, KeyError):
                continue
            if rl and rl.get("primary"):
                latest = rl
    return latest


def pct(a, b):
    return f"{100 * a / b:.1f}%" if b else "n/a"


print(f"token report for {day}\n")
for rep in (claude(day), codex(day)):
    if not rep["input_total"] and not rep["output"]:
        print(f"{rep['label']:<8} no activity\n")
        continue
    print(f"{rep['label']:<8} {rep['sessions']} sessions"
          + (f", {rep['calls']} calls" if rep["calls"] else ""))
    print(f"  input           {rep['input_total']:>12,}")
    print(f"    cached        {rep['cached']:>12,}   hit rate {pct(rep['cached'], rep['input_total'])}")
    print(f"    cache write   {rep['cache_write']:>12,}")
    print(f"  output          {rep['output']:>12,}")
    if rep["reasoning"] is not None:
        print(f"    reasoning     {rep['reasoning']:>12,}   {pct(rep['reasoning'], rep['output'])} of output")
    print()

q = quota()
if q:
    p = q["primary"]
    hrs = p["window_minutes"] // 60
    print(f"codex quota     {p['used_percent']}% of a {hrs}h window used "
          f"(plan: {q.get('plan_type')})")
print("grok            no token accounting in ~/.grok/sessions -- gap")
