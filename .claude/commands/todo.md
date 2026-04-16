---
description: Add a TODO item, or implement the next pending one
allowed-tools: Bash(cargo xtask*), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Bash(scripts/e2e.sh), Read, Write, Edit, Glob, Grep, Agent, AskUserQuestion, Skill(commit)
---

Manage the project TODO list (`TODO.md`).

## Behaviour

- **With arguments** (e.g. `/todo fix the search bar`):
  add the text as a new pending item to `TODO.md` under
  `## Pending`. Do NOT implement it -- just add it and
  confirm.

- **Without arguments** (just `/todo`): read `TODO.md`,
  pick the first pending item, and implement it using
  the steps below.

## Adding an item

1. Read `TODO.md`
2. Append a new bullet under `## Pending` with the
   user's text (wrap at 80 chars, use markdown bullet)
3. Confirm the item was added

## Implementing the next item

1. Read `TODO.md` and identify the first pending item
   (items under the `## Done` heading are completed)

2. If the item is ambiguous or has multiple possible
   interpretations, use `AskUserQuestion` to clarify

3. Implement the item following all project rules in
   `CLAUDE.md`

4. Run `cargo xtask validate` to ensure all checks pass

5. Move the completed item to the `## Done` section of
   `TODO.md` with today's date in parentheses

6. Commit using `/commit`
