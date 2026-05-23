# Template Feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template
discovered during development of this project.

Use this log to feed improvements back to the template.
Newest entries first. Prefix each entry with a status
marker such as `[Deferred]`, `[Fixed locally]`, or
`[N/A for template]`.

---

## 2026-05-23

- **Deploy should auto-sync the systemd unit file
  (drift detection).** The template's
  `cargo xtask deploy` ships the binary and config
  but not the systemd unit. If the unit file in the
  repo drifts from what's installed on the device
  (e.g. the project removes a CLI flag like
  `--frontend` but the device's unit still passes
  it), the next deploy crash-loops the service
  because the new binary rejects the stale arg.
  This bit us hard during a real outage scenario —
  the v0.16.0 `--frontend` regression silently
  lived on the deployed unit for ~7 versions and
  only surfaced on the first deploy after the
  binary's clap parser tightened. Fixed in this
  project in commit `f73945b` (v0.23.1): added a
  `sync_service_unit` step to `cargo xtask deploy`
  that hashes the local `deploy/<service>.service`
  against the installed one, scps + `mv` +
  `daemon-reload`s when they differ, and no-ops
  when identical. Implementation is ~50 lines of
  Rust in `xtask/src/deploy.rs` + tests; the
  `unit_contents_match` helper is the trickiest
  bit (trailing-newline tolerance because scp /
  sudo cat round-trips can flip the terminal `\n`).
  Worth porting to the template so every project
  gets self-healing unit-file deploys without
  having to discover the same outage. **Status:**
  fixed locally; logged for upstream sync.

- **`/commit` skill's E2E step should be
  conditional on `scripts/e2e.sh` existing, not
  conditional on diff content.** Step 8 says
  "Run scripts/e2e.sh … Skip if no frontend or
  API changes in the diff." When the project has
  no `scripts/e2e.sh` at all (because the Svelte
  frontend was dropped in v0.16.0 and the script
  went with it), the skill's instruction to "run"
  the script is meaningless and just confuses
  agents into trying. The diff-based skip is
  also too narrow: any change touching backend
  HTTP routes is "an API change" but doesn't
  necessarily warrant E2E when no end-to-end
  harness exists. Fix: change step 8 to "Run
  `scripts/e2e.sh` if it exists and the diff
  touches frontend / API surface. Skip with no
  warning if the script is absent (project may
  have dropped its E2E layer)." **Status:**
  worked around in tonight's `/commit` invocation
  by skipping E2E entirely; logged for upstream.

- **Domain-expert skill pattern is genuinely
  useful — recommend it in the template.** This
  project hit a real bug tonight where the
  bellwether-web `/api/log` parser silently
  drifted from the actual TRMNL firmware schema
  for 34 days (`battery_voltage=None` across all
  811 log lines). Recovery required reading the
  upstream firmware source on GitHub and
  reconciling. To prevent the same gap from
  reopening, I created `.claude/skills/trmnl-expert/SKILL.md`
  capturing the protocol/schema, all four API
  endpoints with header/body shapes, the firmware
  source-file map, and a re-verification cadence
  (a `gh api repos/<owner>/<repo>/commits/HEAD`
  one-liner that returns the current upstream
  SHA + date so the next reader knows whether the
  schema is fresh). The pattern transfers cleanly
  to any project with a significant external API
  / device protocol dependency where the schema
  isn't versioned formally. The template could
  ship a `.claude/skills/external-protocols/EXAMPLE.md`
  scaffold or document the pattern in the CLAUDE.md
  skills section as "When the project depends on
  an unversioned external schema (firmware,
  third-party API, internal microservice), create
  a `<domain>-expert` skill capturing schema +
  source pin + verification cadence." Companion
  pattern: `docs/developer/RUNBOOK.md` for
  symptom-keyed operational playbooks, kept
  separate from the protocol-reference skill so
  the skill stays focused (skill ≈ "what does
  this look like", runbook ≈ "I see X, do Y").
  **Status:** implemented locally; logged for
  upstream as a recommended-practice addition.

- **Template's initial `TODO.md` content
  encourages staleness.** The template ships a
  `## Pending` section pre-populated with
  example PRs ("PR 1 (in progress): Config
  loading", "PR 2: Windy client", "PR 3: First
  render", "PR N: Wire the loop"). On this
  project those entries lingered in `## Pending`
  for ~50 commits past their actual ship date —
  they're useful as scaffolding on day 1 but
  rapidly become misleading, because no one
  thinks to move them when they finish the work
  ("the diary already captures it"). At the
  point we noticed, the file had 6 stale entries
  and 0 real ones until we added today's two
  battery-related items. Fix options: (a) ship
  an empty `## Pending` section with a `<!-- Add
  pending items here -->` comment; (b) ship the
  examples but auto-stamp them with "Example,
  delete when claiming first real item"; (c)
  add a note to the `/todo` skill that fresh
  projects should clear template examples on
  first use. **Status:** noticed during tonight's
  `/todo` invocation; logged for upstream; not
  fixed in this project (the stale entries are a
  useful artifact of how the project evolved).

## 2026-04-19

- **`.gitignore` only hides `/target/` at the workspace
  root.** The root-anchored leading slash means any
  crate-local `target/` directory (e.g.
  `crates/bellwether/target/` when cargo tests run
  from inside the crate dir) shows as untracked on
  every `git status`. I had to manually avoid it in
  every `git add` argument list across ~5 commits this
  project — easy to forget, easy to accidentally
  `git add .` it into a real commit. Fix: replace
  `/target/` with an un-anchored `target/` in the
  template's `.gitignore` so any `target/` directory
  at any depth is ignored. Also worth adding
  `**/target/` for belt-and-braces. **Status:** not
  fixed in-project (would touch an infra file mid-
  feature-PR); logged here to batch upstream.

- **`cargo xtask test` missed `--ignored` forwarding.**
  The template's `xtask test` wrapper took a filter
  and a `--verbose` flag, but had no way to run
  `#[ignore]`-tagged tests (the standard Rust idiom
  for "manual tool" tests that shouldn't run in
  `xtask validate`). CLAUDE.md explicitly forbids raw
  `cargo test` in favour of the wrapper, so any
  project using `#[ignore]` for manual tools (e.g.
  this project's `generate_dashboard_sample_bmp`)
  hits a dead end. Fixed in-project in commit
  `5bcf286` — added an `--ignored: bool` flag to
  `XCommand::Test`, introduced `TestOptions<'a> {
  filter, verbose, ignored }` so the signature stays
  readable when a fourth flag lands, and threaded
  `--ignored` into the harness args list via an
  explicit `--` separator. The patch is ~60 lines in
  `xtask/src/test_cmd.rs` + `xtask/src/main.rs` plus
  5 `build_args` unit tests. Worth porting to the
  template so every project gets `#[ignore]`
  support. **Status:** fixed locally; logged for
  upstream sync.

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
