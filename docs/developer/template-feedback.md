# Template Feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template
discovered during development of this project.

Use this log to feed improvements back to the template.
Newest entries first. Prefix each entry with a status
marker such as `[Deferred]`, `[Fixed locally]`, or
`[N/A for template]`.

---

## 2026-04-17

- **Six-field finding format not sticky.** The `/commit`
  skill step 5 prescribes six labeled fields per
  review finding (ID, Source, Category, Description,
  Impact / Why it matters, Suggested fix) when
  presenting to the user. In practice agents drift
  to prose paragraphs ("**RT-NNN** one-sentence
  description. Fix: …"), dropping Source and Category
  entirely and merging Description/Impact/Fix into
  flowing text. Happened on every `/commit` in this
  project until the user caught it. Fix: tighten the
  skill text to "Render each finding as a bullet
  list with the six labeled fields below. Do not
  compress into prose." Include a one-line example
  of the expected format. **Status:** logged for
  upstream; in-project agent prompts now remind
  explicitly.

- **CHANGELOG-for-chore rule is too blunt.** Skill
  step 7 says "Skip for: chore, ci, style, docs-only
  changes." But chores can contain user-visible
  behavior changes (e.g., this project's 3000→3100
  port default change was committed as `chore:` and
  red-team correctly caught the missing CHANGELOG
  entry). The rule should depend on user-observable
  effect, not commit type: "Skip for commits with no
  user-observable change; otherwise add an
  `[Unreleased]` entry under `Changed`/`Added`/etc.
  even if the commit type is `chore`." **Status:**
  logged for upstream; in-project this chore's
  CHANGELOG entry was added after review caught it.

- **Resolved-log entry format unspecified.** Skill
  step 5 says "For findings the user chose to fix,
  remove from the open log and insert at the top of
  the resolved log with the fix date and resolution."
  Ambiguous about whether the resolved entry
  preserves the original Description/Impact/
  Suggested-fix or replaces them with just the
  resolution. Different agents pick different
  formats across sessions, causing cross-PR
  inconsistency in the same project's resolved log.
  Fix: prescribe a format — e.g., "Preserve the
  original finding body verbatim and append a
  `**Resolution:**` block with the fix date + how it
  was fixed." Or: "Replace with a terse entry: `###
  ID — title` + `**Category:**` + `**Resolution:**`."
  Either choice, but make one. **Status:** logged
  for upstream; in-project I've used the terse
  format throughout PRs 1-3c.

- **Review-subagent prompt construction is ad-hoc.**
  Skill step 5 says "Pass the full `git diff` output
  to both agents and tell them to read the relevant
  source files." In practice agents write ~600–1000
  word prompts per subagent with context (what the
  PR does, what specifically to examine, reminders
  about the six-field format, how to fetch the diff).
  Every agent invents their own template; drift is
  wide, and forgetting to tell the subagent to run
  `git diff --cached` is how the `/tmp` issue below
  started. Fix: ship a prompt skeleton with the
  skill — ideally a short reusable template the
  agent fills in with PR-specific context, including
  "first step: run `git diff --cached`", a reminder
  about the six-field output format, and the list of
  categories to examine. **Status:** logged for
  upstream; in-project agents have been copy-pasting
  a personal template.

- **`/commit` skill ambiguous about how to hand the
  diff to review subagents.** The current text
  ("Pass the full `git diff` output to both agents
  and tell them to read the relevant source files.")
  doesn't prescribe a mechanism, so models reflexively
  reach for `tokio::fs::write("/tmp/foo-diff.txt", ...)`
  or `git diff --cached > /tmp/foo-diff.txt`. On
  Windows with Git Bash, `/tmp` maps to
  `C:\Users\<user>\AppData\Local\Temp\...` — outside
  the workspace, not git-ignored, invisible to the
  user. I did this four times in this project
  (PRs 1, 2, 3a, 3b) before the user flagged it. Fix:
  change the `/commit` skill step 5 to say "have the
  review subagents run `git diff --cached` themselves
  as their first step — they have Bash. If output
  must be captured for some other reason, write to a
  workspace-local git-ignored path like
  `target/review-diff.txt`, never to `/tmp`." Also
  worth generalizing: any time the skill/agent pattern
  involves "capture tool output and pass it to a
  subagent," prefer subagent-runs-the-command or
  `target/`-local files. **Status:** logged for
  upstream; worked around locally by updating the
  review-subagent prompts in this project to run
  `git diff --cached` themselves.

<!-- Add new entries above this line -->
