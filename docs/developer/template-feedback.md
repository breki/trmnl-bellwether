# Template Feedback

Issues, improvements, and observations about the
[rustbase](https://github.com/breki/rustbase) template
discovered during development of this project.

This file is organised into three lifecycle sections.
Within each section, entries are in reverse
chronological order (newest first).

- **Open divergences** -- things known to be
  suboptimal, missing, or differently-shaped than
  the ideal template baseline that have not yet
  been addressed in either this project or the
  template.
- **Resolved** -- entries closed out by a fix.
  Includes a `Resolution:` line with the fix date.
- **Suggestions to flow back to the template** --
  ideas this project wants pushed upstream
  (typically fixed locally, awaiting backfeed).

---

## Open divergences

(none currently outstanding -- all observations have
been either resolved upstream or staged in
Suggestions for backfeed.)

---

## Resolved

### 2026-04-17 -- Six-field finding format not sticky

The `/commit` skill step 5 prescribes six labeled
fields per review finding (ID, Source, Category,
Description, Impact / Why it matters, Suggested fix)
when presenting to the user. In practice agents
drift to prose paragraphs ("**RT-NNN** one-sentence
description. Fix: ..."), dropping Source and
Category entirely and merging Description / Impact /
Fix into flowing text. Happened on every `/commit`
in this project until the user caught it.

**Resolution:** 2026-05-23 -- upstream template
v0.10.0 `/commit` skill now includes an explicit
six-field reminder in the subagent prompt
construction step (synced into this project on
2026-05-23). Some prose-drift risk remains; if it
recurs, tighten the rendering rule too.

### 2026-04-17 -- CHANGELOG-for-chore rule is too blunt

Skill step 7 said "Skip for: chore, ci, style,
docs-only changes." But chores can contain
user-visible behavior changes (e.g., this project's
3000->3100 port default change was committed as
`chore:` and red-team correctly caught the missing
CHANGELOG entry). The rule should depend on
user-observable effect, not commit type.

**Resolution:** 2026-05-23 -- upstream template
v0.10.0 `/commit` skill step 7 was rewritten to
trigger on observable effect, explicitly noting
that `chore:` commits with user-observable changes
must still get a `[Unreleased]` entry. Synced into
this project on 2026-05-23.

### 2026-04-17 -- Resolved-log entry format unspecified

Skill step 5 said "For findings the user chose to
fix, remove from the open log and insert at the top
of the resolved log with the fix date and
resolution." Ambiguous about whether the resolved
entry preserves the original Description / Impact /
Suggested-fix or replaces them with just the
resolution. Different agents picked different
formats across sessions.

**Resolution:** 2026-05-23 -- upstream template
v0.10.0 `/commit` skill now prescribes the terse
format (ID -- title, Category, Resolution date +
1-3 sentences) and explicitly says not to preserve
the original body. Synced into this project on
2026-05-23.

### 2026-04-17 -- /commit skill ambiguous about diff handoff to subagents

The current text ("Pass the full `git diff` output
to both agents and tell them to read the relevant
source files.") didn't prescribe a mechanism, so
models reflexively reached for `tokio::fs::write(
"/tmp/foo-diff.txt", ...)` or
`git diff --cached > /tmp/foo-diff.txt`. On Windows
with Git Bash, `/tmp` maps outside the workspace
and is invisible to the user.

**Resolution:** 2026-05-23 -- upstream template
v0.10.0 `/commit` skill now includes a "How to hand
the diff to the subagents" section telling each
subagent to run `git diff --cached` itself, with a
`target/` fallback explicitly forbidding `/tmp`.
Synced into this project on 2026-05-23.

---

## Suggestions to flow back to the template

### 2026-05-24 -- `clean_cache` folds `dir_size` warnings into deletion-error exit (RT-052)

`xtask/src/clean_cache.rs:123-131` pushes every
`DirSizeWarning` returned by `dir_size` into the same
`errors` vec that gates the function's final exit
status (lines 70-78 treat any non-empty error list as
a hard `Err("N deletion error(s)")`). Result: a
transient `symlink_metadata` / `read_dir` failure
during sizing (e.g. Defender briefly locking a
fingerprint file) is reported as a "deletion error"
and produces a non-zero exit even when every entry
was deleted successfully. This is exactly the
AV/rust-analyzer scenario the module's doc-comment
cites as motivation for the tool. Fix: track sizing
warnings in a separate vec (or downgrade to a
`println!` info line), or only fold a sizing warning
into `errors` when the matching `delete_entry` also
failed. **Status:** not yet fixed locally (template
sync 2026-05-24); logged for upstream.

### 2026-05-24 -- `clean_cache` does not respect `CARGO_TARGET_DIR` (RT-053)

`xtask/src/clean_cache.rs:38-39` hardcodes
`target/{debug,release}/incremental` under
`workspace_root()`. Doesn't consult
`CARGO_TARGET_DIR`, the `CARGO_BUILD_TARGET_DIR` env
var, or a `[build] target-dir` entry in
`.cargo/config.toml`. Developers (and CI jobs) that
redirect the target directory -- common when
`target/` is on an SSD scratch path or shared across
worktrees -- silently get "not present, skipping" /
"Total: 0 B" with no signal that the real cache went
untouched. These are exactly the power users most
likely to invoke the tool. Fix: resolve the target
directory at runtime via
`cargo metadata --format-version 1 --no-deps`
(reading `.target_directory`) or the `cargo_metadata`
crate; at minimum, check `$CARGO_TARGET_DIR` and fall
back to `workspace_root().join("target")`. Document
the resolution order in the module comment.
**Status:** not yet fixed locally (template sync
2026-05-24); logged for upstream.

### 2026-05-24 -- Public coverage types missing `Debug` / `Clone` derives (AQ-048)

`xtask/src/coverage.rs` exposes `CoverageResult`
(line 26), `CoverageFailure` (line 43), and
`FailingModule` (line 56) as public types but
derives neither `Debug` nor implements `Display`.
The existing test workaround
(`format_failure_overall` / `format_failure_modules`
matching on string output of `format_failure(&f)`
rather than asserting on structured values) is a
tell-tale sign the missing derives are felt. Add
`#[derive(Debug)]` to all three and consider
`#[derive(Clone)]` on `FailingModule` (matches the
treatment of `DirSizeWarning`). **Status:** not
fixed locally; logged for upstream.

### 2026-05-24 -- `format_failure` should be `impl Display for CoverageFailure` (AQ-049)

`xtask/src/coverage.rs:178` exposes
`pub fn format_failure(failure: &CoverageFailure)
-> String` as a free function alongside the public
`CoverageFailure` type, instead of `impl Display for
CoverageFailure`. The module-doc on
`CoverageFailure` (line 38) even calls this out
("Render via `format_failure`") -- which is exactly
the documentation a `Display` impl would replace.
Free-function formatters don't compose with `{}`,
`to_string()`, `?`-propagation, or `anyhow::Error`;
every caller has to remember the namespaced helper.
Fix: replace with `impl fmt::Display for
CoverageFailure`, move the existing function body
verbatim into `fn fmt`, update `validate.rs` call
site to `Err(failure.to_string())` (or
`format!("{failure}")`). **Status:** not fixed
locally; logged for upstream.

### 2026-05-24 -- `DirSizeWarning` fields exposed `pub` unnecessarily (AQ-050)

`xtask/src/helpers.rs` `DirSizeWarning` (around
line 78) declares both fields `pub`: `pub path:
PathBuf, pub message: String`. The module already
provides a `Display` impl that renders the canonical
form, and the only documented consumer
(`clean_cache.rs:130`, `errors.push(w.to_string())`)
only needs `Display`. Public fields lock in the wire
format -- any future change (e.g. switching
`message` to a structured `enum DirSizeError`
variant, or splitting `path` into `dir + entry`)
becomes a breaking change to every caller that did
field access. Fix: make fields `pub(crate)` (or
private) and expose `pub fn path(&self) -> &Path` /
`pub fn message(&self) -> &str` accessors only if
needed. **Status:** not fixed locally; logged for
upstream.

### 2026-05-24 -- `scan_failing_modules` mixes typed and untyped JSON access (AQ-051)

`xtask/src/coverage.rs:117` defines
`fn scan_failing_modules(files: &serde_json::Value)
-> Result<Vec<FailingModule>, String>`, taking an
untyped `serde_json::Value` and using
`file["filename"].as_str().unwrap_or("?")` /
`file["summary"]["lines"]["percent"].as_f64()
.unwrap_or(0.0)`, then re-parses `file["segments"]`
with a typed `Deserialize`. Worst of both worlds:
typed inner shape catches segment-schema drift, but
the untyped outer envelope silently masks drift in
filename / summary (missing `percent` becomes 0.0,
which then falls below `MODULE_THRESHOLD` and looks
like a real coverage failure). Fix: define a
`#[derive(Deserialize)] struct FileEntry { filename:
String, summary: FileSummary, segments: Vec<Segment>
}` (plus nested `FileSummary` / `LineSummary`), parse
the files array once at the top of `coverage_check`,
and have `scan_failing_modules` take `&[FileEntry]`.
Body collapses to a straight iterator without
`unwrap_or` fallbacks. **Status:** not fixed
locally; logged for upstream.

### 2026-05-24 -- `.gitignore` `target/` un-anchoring is too broad (RT-054)

The template's previous `/target/` was root-anchored
and narrow; the post-sync `target/` (un-anchored)
plus `**/target/` (redundant given the un-anchored
form) matches *any* directory named `target` at any
depth. Future doc pages, asset folders, fixture
dirs, or sample data named `target` (e.g.,
`docs/target-audience/`,
`crates/foo/tests/fixtures/target/`) will be
silently untracked. The 2026-04-19 feedback entry
(now Resolved) requested the un-anchoring because
crate-local `target/` directories were showing as
untracked; that need is real but the current pattern
is broader than required. Better template default:
`/target/` plus an explicit
`/crates/*/target/` line (or whatever the canonical
nested-target locations are). **Status:** logged
during template sync 2026-05-24 against a change
that already shipped to this project; flagged for
the next template revision.

### 2026-05-23 -- Deploy should auto-sync the systemd unit file (drift detection)

The template's `cargo xtask deploy` ships the
binary and config but not the systemd unit. If the
unit file in the repo drifts from what's installed
on the device (e.g. the project removes a CLI flag
like `--frontend` but the device's unit still
passes it), the next deploy crash-loops the
service because the new binary rejects the stale
arg. This bit us hard during a real outage
scenario -- the v0.16.0 `--frontend` regression
silently lived on the deployed unit for ~7 versions
and only surfaced on the first deploy after the
binary's clap parser tightened. Fixed in this
project in commit `f73945b` (v0.23.1): added a
`sync_service_unit` step to `cargo xtask deploy`
that hashes the local `deploy/<service>.service`
against the installed one, scps + `mv` +
`daemon-reload`s when they differ, and no-ops when
identical. Implementation is ~50 lines of Rust in
`xtask/src/deploy.rs` + tests; the
`unit_contents_match` helper is the trickiest bit
(trailing-newline tolerance because scp / sudo cat
round-trips can flip the terminal `\n`). Worth
porting to the template so every project gets
self-healing unit-file deploys without having to
discover the same outage. **Status:** fixed
locally; logged for upstream sync.

### 2026-05-23 -- /commit skill's E2E step should be conditional on scripts/e2e.sh existing

Step 8 says "Run scripts/e2e.sh ... Skip if no
frontend or API changes in the diff." When the
project has no `scripts/e2e.sh` at all (because the
Svelte frontend was dropped in v0.16.0 and the
script went with it), the skill's instruction to
"run" the script is meaningless and just confuses
agents into trying. The diff-based skip is also
too narrow: any change touching backend HTTP routes
is "an API change" but doesn't necessarily warrant
E2E when no end-to-end harness exists. Fix: change
step 8 to "Run `scripts/e2e.sh` if it exists and
the diff touches frontend / API surface. Skip with
no warning if the script is absent (project may
have dropped its E2E layer)." **Status:** worked
around in tonight's `/commit` invocation by
skipping E2E entirely; logged for upstream.

### 2026-05-23 -- Domain-expert skill pattern is useful -- recommend it in the template

This project hit a real bug tonight where the
bellwether-web `/api/log` parser silently drifted
from the actual TRMNL firmware schema for 34 days
(`battery_voltage=None` across all 811 log lines).
Recovery required reading the upstream firmware
source on GitHub and reconciling. To prevent the
same gap from reopening, I created
`.claude/skills/trmnl-expert/SKILL.md` capturing
the protocol/schema, all four API endpoints with
header/body shapes, the firmware source-file map,
and a re-verification cadence (a
`gh api repos/<owner>/<repo>/commits/HEAD`
one-liner that returns the current upstream SHA +
date so the next reader knows whether the schema
is fresh). The pattern transfers cleanly to any
project with a significant external API / device
protocol dependency where the schema isn't
versioned formally. The template could ship a
`.claude/skills/external-protocols/EXAMPLE.md`
scaffold or document the pattern in the CLAUDE.md
skills section as "When the project depends on an
unversioned external schema (firmware, third-party
API, internal microservice), create a
`<domain>-expert` skill capturing schema + source
pin + verification cadence." Companion pattern:
`docs/developer/RUNBOOK.md` for symptom-keyed
operational playbooks, kept separate from the
protocol-reference skill so the skill stays
focused (skill ~= "what does this look like",
runbook ~= "I see X, do Y"). **Status:**
implemented locally; logged for upstream as a
recommended-practice addition.

### 2026-05-23 -- Template's initial TODO.md content encourages staleness

The template ships a `## Pending` section
pre-populated with example PRs ("PR 1 (in
progress): Config loading", "PR 2: Windy client",
"PR 3: First render", "PR N: Wire the loop"). On
this project those entries lingered in `## Pending`
for ~50 commits past their actual ship date --
they're useful as scaffolding on day 1 but rapidly
become misleading, because no one thinks to move
them when they finish the work ("the diary already
captures it"). At the point we noticed, the file
had 6 stale entries and 0 real ones until we added
today's two battery-related items. Fix options:
(a) ship an empty `## Pending` section with a
`<!-- Add pending items here -->` comment; (b)
ship the examples but auto-stamp them with
"Example, delete when claiming first real item";
(c) add a note to the `/todo` skill that fresh
projects should clear template examples on first
use. **Status:** noticed during tonight's `/todo`
invocation; logged for upstream; not fixed in this
project (the stale entries are a useful artifact
of how the project evolved).

### 2026-04-19 -- `.gitignore` only hides `/target/` at the workspace root

The root-anchored leading slash means any
crate-local `target/` directory (e.g.
`crates/bellwether/target/` when cargo tests run
from inside the crate dir) shows as untracked on
every `git status`. I had to manually avoid it in
every `git add` argument list across ~5 commits
this project -- easy to forget, easy to
accidentally `git add .` it into a real commit.
Fix: replace `/target/` with an un-anchored
`target/` in the template's `.gitignore` so any
`target/` directory at any depth is ignored. Also
worth adding `**/target/` for belt-and-braces.
**Status:** not fixed in-project (would touch an
infra file mid-feature-PR); logged here to batch
upstream.

### 2026-04-19 -- `cargo xtask test` missed `--ignored` forwarding

The template's `xtask test` wrapper took a filter
and a `--verbose` flag, but had no way to run
`#[ignore]`-tagged tests (the standard Rust idiom
for "manual tool" tests that shouldn't run in
`xtask validate`). CLAUDE.md explicitly forbids
raw `cargo test` in favour of the wrapper, so any
project using `#[ignore]` for manual tools (e.g.
this project's `generate_dashboard_sample_bmp`)
hits a dead end. Fixed in-project in commit
`5bcf286` -- added an `--ignored: bool` flag to
`XCommand::Test`, introduced `TestOptions<'a> {
filter, verbose, ignored }` so the signature
stays readable when a fourth flag lands, and
threaded `--ignored` into the harness args list
via an explicit `--` separator. The patch is ~60
lines in `xtask/src/test_cmd.rs` +
`xtask/src/main.rs` plus 5 `build_args` unit
tests. Worth porting to the template so every
project gets `#[ignore]` support. **Status:**
fixed locally; logged for upstream sync.

### 2026-04-17 -- Review-subagent prompt construction is ad-hoc

Skill step 5 said "Pass the full `git diff` output
to both agents and tell them to read the relevant
source files." In practice agents write ~600-1000
word prompts per subagent with context (what the
PR does, what specifically to examine, reminders
about the six-field format, how to fetch the
diff). Every agent invents their own template;
drift is wide, and forgetting to tell the subagent
to run `git diff --cached` is how the `/tmp`
issue started. Fix: ship a prompt skeleton with
the skill -- ideally a short reusable template
the agent fills in with PR-specific context,
including "first step: run `git diff --cached`",
a reminder about the six-field output format, and
the list of categories to examine. **Status:**
partially addressed in upstream template v0.10.0
(subagent prompts now include a one-line
description, six-field reminder, and category
list per reviewer) -- but no reusable skeleton
yet. In-project agents have been copy-pasting a
personal template.

<!-- Add new entries above this line, inside the
relevant section. -->
