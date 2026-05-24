---
description: Workflow retrospective on the session so far -- Efficiency, Quality, Speed
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git log:*), Read, Grep, Glob, AskUserQuestion, Edit
---

Reflect on the *process* of the work just completed
(or in progress) and surface concrete improvements
to how the session was run. This complements code
reviews (Red Team / Artisan in `/commit`) which
critique the diff -- the retrospective critiques the
*way* the diff was produced.

Invoke this skill either:
- **Automatically** at the end of `/commit` step 12
  (which delegates to this skill).
- **Manually** at any time -- mid-session, after a
  failed attempt, before a hand-off -- when you want
  a process-level reflection that is not tied to a
  commit.

## When to skip

**Recursive workflow-only skip.** When invoked
automatically from `/commit` and the just-committed
diff is entirely under `.claude/**` or `CLAUDE.md`,
skip silently. Those sessions ARE the workflow
authoring -- any "improvements" loop back to the
files just shipped. `docs/**` and `README.md`
commits still run the retro because those sessions
usually involve real research / tool work worth
reflecting on.

The skip does not apply when the user invokes
`/retrospect` directly -- the user asked for it
explicitly, so run it even on a workflow-only
session.

## Surfacing findings

Walk the session (or the just-committed work) and
collect findings in three buckets. Aim for 0-3
findings per bucket; do not invent filler.

1. **Efficiency** -- tool calls that wasted budget.
   Examples:
   - Reading the same file twice when the first read
     was still in context.
   - Running full `cargo xtask validate` when
     `cargo xtask check` or `cargo xtask test`
     would have caught the same thing in a fraction
     of the time.
   - Repeated round-trips on a pattern that could
     have been extracted into a helper.
   - `cd subdir && ...` constructions that lose
     working-directory context across calls.
   - Sequential Agent calls that had no dependency
     and could have run in parallel.

2. **Quality** -- process shortcomings the code
   reviewers do not catch. Examples:
   - Committed before running a verification step
     (e.g. browser-checked a UI change only after
     the commit, not before).
   - Skipped a cross-reference (e.g. did not check
     `docs/developer/template-feedback.md` before
     applying a template change).
   - Undocumented decision -- chose between two
     approaches without recording why.
   - Did not capture a finding the reviewers raised
     into the long-lived RT/AQ logs.

3. **Speed** -- wall-time delays caused by ordering.
   Examples:
   - Slow step ran in the foreground when
     `run_in_background: true` would have unblocked
     other work.
   - Reviewer agents ran serially when they could
     have been one parallel message.
   - The /commit reviewers could have been
     pre-launched during `/implement` Phase 3 so
     `/commit` had results waiting.

## Tagging findings

For each finding, assign:

- **ID:** `<N>-<slug>` (e.g. `1-redundant-reads`,
  `2-serial-agents`). Single digit per session; the
  N resets each retro.
- **Tag:**
  - `[trivial]` if the fix is a single tool call
    right now (append a clause to a doc, add a
    permission to settings, rename a constant).
  - `[propose]` if the fix needs user input,
    cross-cuts files, implies a policy change, or
    requires architectural judgement.

## Presenting

Output a short report like:

```
Workflow retrospective

Efficiency:
  1-redundant-reads [trivial]
    Read coverage.rs twice (turns 3 and 7). The
    first read was still in context. Suggestion:
    note already-read files before reading again.

Quality:
  (none surfaced)

Speed:
  2-serial-reviewers [propose]
    Red Team and Artisan ran in one parallel
    message in /commit, but the second iteration
    of fixes ran serial cargo xtask validate then
    fmt then validate. Could fold fmt into the
    validate wrapper.
```

End the report with one of:

- "Apply trivial findings now?" -- if any
  `[trivial]` items exist, offer to apply them via
  `AskUserQuestion`. Apply only the selected ones.
- "No trivial findings to auto-apply." -- if every
  finding is `[propose]` or there are no findings.

## What stays ephemeral

`[propose]` findings are surfaced for awareness and
discarded unless the user asks to escalate them. The
escalation paths:

- **Real RT/AQ finding:** append to
  `docs/developer/redteam-log.md` or
  `artisan-log.md` (only when the finding describes
  a defect in shipped code, not a process gap).
- **TODO item:** capture via `/todo <text>` for a
  follow-up implementation pass.
- **Doc edit:** small process rule changes (e.g. "add
  this to CLAUDE.md") land directly via `Edit`.

Without explicit user direction, do not write the
findings anywhere. The transcript is the record.

## Rules

- Be specific. "Could have been faster" is not a
  finding; "the two Agent calls in turn N had no
  dependency and could have run in one message" is.
- Cite turn numbers or tool names when possible so
  the user can verify.
- Do not duplicate Red Team / Artisan findings.
  Those critique the code; this critiques the
  process.
- Cap output at ~15 lines of finding text. Beyond
  that, prioritise the highest-impact items.
- One retrospective per session is usually enough.
  Repeated invocations within the same session
  should focus on work since the last retro.
