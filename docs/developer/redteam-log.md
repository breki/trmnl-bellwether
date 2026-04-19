# Red Team Findings -- Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-113

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

---

### RT-112 — Cloning `Layout::embedded_default` on every `--dev` start / test setup
**Category:** Correctness / efficiency
**Logged:** 2026-04-19 (deferred from v0.15.0 review)
**Description:** `Layout::embedded_default()` returns `&'static Layout`, but `PublishLoop` (and `Startup` in the web crate) own a `Layout` by value. Every startup and every one of the 7 `PublishLoop` tests therefore clones the embedded layout. Safe (no interior mutability) but wasteful — an `Arc<Layout>` or `&'static Layout` in `PublishLoop` would avoid the clones.
**Trigger:** Any construction path that routes the embedded default through.
**Suggested fix:** Store `Arc<Layout>` in `PublishLoop` and in the web crate's `Startup`. Lets config-loaded layouts share the same cheap-clone representation; embedded default becomes `Arc::new(Layout::embedded_default().clone())` once, then cheap clones on every use.
