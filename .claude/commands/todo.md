---
description: Capture an issue or idea into the TODO list (no implementation)
allowed-tools: Read, Write, Edit, Grep
---

Collect an issue or idea into `docs/todo.md`. This
command **only captures** -- it never implements. Use
`/implement` to act on a captured item.

## Behaviour

- **With arguments** (e.g. `/todo search bar is slow`):
  add the text as a new pending item with a generated
  slug.
- **Without arguments** (just `/todo`): list the
  current pending items (slug + first line) so the
  user can see what is queued, then stop.

## Adding an item

1. Read `docs/todo.md`.

2. **Generate a slug** from the user's text:
   - Lowercase, ASCII only, words joined by `-`.
   - Drop filler words (`a`, `the`, `to`, `for`,
     `is`, `of`, `in`, `on`, `and`, `or`).
   - 3-6 words, <= 50 chars total.
   - Should read as a topic, not a sentence:
     `search-bar-perf`, not
     `make-the-search-bar-faster`.
   - If the slug collides with an existing pending or
     done entry in `docs/todo.md`, append `-2`,
     `-3`, etc.

3. Append a bullet under `## Pending` in this exact
   form:

   ```
   - **<slug>** -- <one-line summary, <= 80 chars>
     <optional extra lines, indented 2 spaces, wrapped
     at 80>
   ```

   Keep the user's wording. Do not paraphrase or
   expand.

4. Confirm: print the slug and the line that was
   added. Mention `/implement <slug>` as the next
   step. Do not start implementing.

## Listing pending items

When called with no arguments:

- Read `docs/todo.md`.
- Print each pending entry as `<slug> -- <summary>`,
  one per line. Nothing else.

## Rules

- Never edit the `## Done` section from this command.
- Never create files in `docs/issues/` from this
  command -- that is `/implement`'s job.
- Never run tests, builds, or git commands.
