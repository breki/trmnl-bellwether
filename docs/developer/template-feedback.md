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
