---
description: Commit current changes following project conventions
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(git add:*), Bash(git commit:*), Bash(cargo xtask validate*), Bash(cargo xtask fmt*), Bash(cargo generate-lockfile*), Bash(scripts/e2e.sh*), Read, Edit, Agent, AskUserQuestion
---

Commit the current changes following the project's git commit
conventions.

## Instructions

1. **Analyze current state** - Run these commands in parallel:
   - `git status` (never use -uall flag)
   - `git diff` for unstaged changes
   - `git diff --cached` for staged changes
   - `git log --oneline -5` for recent commit style reference

2. **Review changes** - Analyze what was changed and determine:
   - The commit type: feat, fix, chore, refactor, docs, test,
     style, perf
   - A concise subject line (imperative mood, no period)
   - A brief body explaining what and why

3. **Bump version** (for feat, fix, perf commits):
   - Read the current version from
     `crates/bellwether/Cargo.toml`
   - Bump according to commit type:
     - `feat` -> **minor** bump (0.1.0 -> 0.2.0)
     - `fix`, `perf` -> **patch** bump (0.1.0 -> 0.1.1)
   - Edit `crates/bellwether/Cargo.toml` to update the version
   - Run `cargo generate-lockfile` to update `Cargo.lock`
   - Include both files in staged files
   - Skip version bump for: docs, test, refactor, chore, style

4. **Validate** (when version was bumped in step 3):
   - Run `cargo xtask validate` to ensure all checks pass
   - If validation **fails**, ask the user whether to commit
     anyway or abort. Wait for their answer before proceeding.
   - Skip this step if no version bump occurred

5. **Code review** -- Before E2E tests, spawn **two** AI
   agents **in parallel** (in a single message with two
   Agent tool calls). Both read the source files but do
   not modify them.

   **IMPORTANT:** Always run both reviews when the diff
   contains code changes: Rust (`.rs`, `.toml`),
   frontend (`.svelte`, `.js`, `.ts`, `.css`), config
   files (`playwright.config.ts`, `vite.config.js`,
   `vitest.config.js`, etc.), or
   deployment/infrastructure files (`.service`,
   `Dockerfile`, `docker-compose.yml`, `.conf`,
   `.nginx`, `.env.example`, etc.).
   Never skip them -- even for "straightforward"
   changes. The only exception is commits that contain
   no code at all (docs-only markdown, `.md` files).

   **Agent A -- Red Team** (security & correctness):

   > You are a red team reviewer. Analyze the code changes
   > for a Rust project. Report issues in two categories:
   >
   > **Correctness**: logic bugs, unhandled edge cases,
   > missing error handling, off-by-one errors, incorrect
   > assumptions, dead code, unclear semantics.
   >
   > **Security**: command injection, path traversal,
   > unsafe deserialization, unvalidated input, TOCTOU
   > races, information leaks, denial of service vectors.
   >
   > **CI/CD** (when `.github/workflows/` files are in
   > the diff): shell injection via untrusted context
   > variables, excessive permissions, unpinned actions,
   > cache poisoning, secret exposure.
   >
   > **Project Configuration** (when `Cargo.toml`,
   > `rustfmt.toml`, `clippy.toml`, `.gitignore`, or
   > other root config files are in the diff): insecure
   > defaults, overly permissive settings, missing
   > deny/forbid lint levels, vulnerable dependencies.
   >
   > **Deployment** (when `.service`, `Dockerfile`,
   > `docker-compose.yml`, nginx/Apache configs, or
   > other infra files are in the diff): running as
   > root, overly broad filesystem access, missing
   > sandboxing (`ProtectSystem`, `PrivateTmp`, etc.),
   > world-readable secrets, open bind addresses
   > without firewall context.
   >
   > Be adversarial -- assume the code is wrong and try
   > to prove it. Only report real, actionable issues
   > with specific line references. Do NOT report style
   > nits, missing docs, or hypothetical concerns. If you
   > find nothing, say "No issues found."
   >
   > For each finding, include:
   > 1. **What**: the specific issue with file:line ref
   > 2. **Why it matters**: concrete impact
   > 3. **Example trigger**: specific input or state
   > 4. **Suggested fix**: how to resolve it

   **Agent B -- Artisan** (code quality & craftsmanship):

   > You are the Artisan -- a code quality reviewer for a
   > Rust project. You focus on craftsmanship beyond what
   > clippy catches. Analyze the code changes and report
   > issues in these categories:
   >
   > **Error Handling & Messages**: error types missing
   > Display, capitalized/punctuated error messages,
   > error chains leaking library types.
   >
   > **API Design**: functions accepting concrete types
   > instead of trait bounds, inconsistent parameter
   > patterns, ownership semantics unclear.
   >
   > **Abstraction Boundaries**: public modules exposing
   > internal types, dependency types leaked in public
   > APIs, business logic in the binary instead of the
   > library.
   >
   > **Type Safety**: missing Display/Debug on public
   > types, stringly-typed APIs where enums/newtypes
   > would be safer, unnecessary clones or allocations.
   >
   > **Module Size**: any source file over 500 lines
   > that contains multiple structs/enums should be
   > flagged for splitting.
   >
   > Only report real, actionable issues with specific
   > line references. Do NOT duplicate clippy warnings
   > or red team findings. If you find nothing, say
   > "No issues found."
   >
   > For each finding, include:
   > 1. **What**: the specific issue with file:line ref
   > 2. **Why it matters**: impact on maintainability
   > 3. **Better approach**: specific code change

   Pass the full `git diff` output to both agents and
   tell them to read the relevant source files.

   **Presenting findings to the user:**
   - Present **ALL** findings from both reviewers
     without filtering or skipping any. Do not omit
     findings based on your own priority assessment.
   - Present each finding with full detail:
     - **ID and title** (e.g. RT-023 or AQ-001)
     - **Source**: Red Team or Artisan
     - **Category**
     - **Description**
     - **Impact / Why it matters**
     - **Suggested fix**
   - Use `AskUserQuestion` with the findings as
     options to let the user pick which to fix.
     Split into multiple questions if needed (max
     4 options per question). Include "Commit as-is"
     and "Abort" as options.
   - Wait for the user's answer before proceeding

   **Findings logs:**

   Red team findings use two files:
   - `docs/developer/redteam-log.md` -- open (RT-NNN)
   - `docs/developer/redteam-resolved.md` -- fixed

   Artisan findings use two files:
   - `docs/developer/artisan-log.md` -- open (AQ-NNN)
   - `docs/developer/artisan-resolved.md` -- fixed

   Both pairs are in **reverse chronological order**
   (newest first). New entries go right after the `---`
   separator.

   After the review:
   - Read each log to get the next ID (noted in the
     "Next ID" field at the top of each open log)
   - For each **new** finding, insert at the **top**
     of the relevant open log with the next ID, date,
     commit context, full description, and category.
     Increment "Next ID".
   - For findings the user chose to **fix**, remove
     from the open log and insert at the **top** of
     the resolved log with the fix date and resolution
   - Include all changed log files in staged files
   - **Threshold warning:** if 10 or more findings
     are open in either log, tell the user that a
     comprehensive full-codebase review is needed

6. **Update development diary** (for significant changes):
   - Read `docs/developer/DIARY.md` to see format and
     recent entries
   - Add an entry for:
     - `feat`, `fix`, `perf` commits (functional changes)
     - Infrastructure/setup changes that affect developer
       workflow
   - Entries are in reverse chronological order (newest
     first)
   - Merge entries for the same day under one
     `### YYYY-MM-DD` heading
   - Attach the version to each entry title, not the
     date heading: `- Entry title (vX.Y.Z)` (use the
     version **after** the bump from step 3)
   - Use backticks for technical terms
   - Skip diary update for: docs, style, test, refactor,
     minor chores

7. **Update CHANGELOG.md** (for user-visible changes):
   - If the commit adds features, fixes bugs, changes
     behaviour, or removes functionality, add a bullet to
     the `[Unreleased]` section under the appropriate
     heading (`Added`, `Changed`, `Fixed`, or `Removed`)
   - Skip for: chore, ci, style, docs-only changes

8. **E2E tests** -- Run `scripts/e2e.sh` to verify the
   full stack works end-to-end. The script kills stale
   servers and runs Playwright, which auto-starts both
   backend and frontend using test data (not production
   data).
   - If E2E tests **fail**, ask the user whether to
     commit anyway or abort.
   - Skip if no frontend or API changes in the diff.

9. **Fix line endings** - After staging, check for CRLF
   warnings. All text files must use LF line endings.

10. **Stage files** - Add specific files by name (avoid
   `git add -A` or `git add .`). Never commit sensitive
   files (.env, credentials, etc.). Include diary and
   changelog if updated.

11. **Commit** using this exact format (use HEREDOC):

```bash
git commit -m "$(cat <<'EOF'
<type>: <subject>

<body>

AI-Generated: Claude Code (<ModelName> <YYYY-MM-DD>)
EOF
)"
```

## Rules

- DO NOT include "Co-Authored-By" lines
- DO NOT include "Generated with [Claude Code]" lines
- Use the AI-Generated footer format shown above
- If no changes to commit, inform the user
- If changes look incomplete or risky, ask before committing

## Commit Types

- `feat`: New feature (minor version bump)
- `fix`: Bug fix (patch version bump)
- `perf`: Performance improvement (patch version bump)
- `chore`: Maintenance, tooling, dependencies (no bump)
- `refactor`: Code restructuring (no bump)
- `docs`: Documentation only (no bump)
- `test`: Adding or updating tests (no bump)
- `style`: Formatting, whitespace (no bump)
