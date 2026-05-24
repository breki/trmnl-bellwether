---
description: Log feedback for the rustbase template
allowed-tools: Read, Edit, AskUserQuestion
---

Log an observation about the rustbase template in
`docs/developer/template-feedback.md`.

## Instructions

1. If the user provided a specific observation, use it.
   Otherwise, use `AskUserQuestion` to ask what they
   noticed.

2. Read `docs/developer/template-feedback.md` to see
   the format and existing entries. The file is
   organised into three lifecycle sections:

   - **Open divergences** -- things known to be
     suboptimal, missing, or differently-shaped than
     the ideal template baseline. New observations
     about current template issues land here.
   - **Resolved** -- entries closed out by a retrofit
     or fix. Move entries here (with a one-line
     `Resolution:` note and the fix date) once the
     issue is addressed.
   - **Suggestions to flow back to the template** --
     in a derived project, ideas the project wants
     pushed upstream. In the template repo itself
     this section is normally empty; it exists for
     structural symmetry with derived projects.

3. Use `AskUserQuestion` (or infer from context) to
   decide which section the new entry belongs in.
   Default: **Open divergences** for new observations
   about current template state. If the user
   describes something they've already fixed, route
   to **Resolved**. If the user is a derived project
   logging a suggestion for the upstream template,
   route to **Suggestions to flow back**.

4. Insert the entry at the **top** of the chosen
   section (newest first within each section).
   Format:

   ```
   ### <YYYY-MM-DD or short topic> -- <Short title>

   <Description of the issue, why it matters, and
   suggested fix for the template.>
   ```

   For entries in **Resolved**, end with a one-line
   summary of the fix.

5. Do NOT commit -- just edit the file and let the
   user review. It will be included in the next
   `/commit`.
