# Red Team Findings -- Open

Open findings from red team reviews, newest first.
Fixed findings are moved to
[redteam-resolved.md](redteam-resolved.md).

**Next ID:** RT-117

**Threshold:** when 10+ findings are open, a full-codebase
red team review is required before continuing feature work.

---

### RT-113 — Unauthenticated `/preview.bmp` leaks dashboard contents and is a bandwidth amplifier
**Category:** Security / information disclosure
**Logged:** 2026-04-19 (v0.16.1 landing-page preview fix)
**Description:** `/preview.bmp` is intentionally exempt from the access-token middleware so the landing-page `<img>` works in a browser. Consequence: anyone who can reach the server can pull the latest rendered dashboard BMP (Home Assistant state, temperatures, calendar-ish content depending on widgets) and hammer the endpoint for bandwidth. Deliberately accepted for now because the production deploy target (`malina`) is LAN/Tailscale-only, but worth revisiting if the server is ever exposed beyond the trusted network.
**Trigger:** `while true; do curl http://host:3100/preview.bmp; done` from any host that can reach the server.
**Suggested fix:** Either (a) bind the web server to a non-public interface only and document the constraint, (b) move `/preview.bmp` inside `require_access_token` and accept that the landing-page preview only works with the header set (use a cookie-based token handshake on `/`), or (c) add a lightweight per-IP rate limiter in front of the unauthenticated routes.

### RT-112 — Cloning `Layout::embedded_default` on every `--dev` start / test setup
**Category:** Correctness / efficiency
**Logged:** 2026-04-19 (deferred from v0.15.0 review)
**Description:** `Layout::embedded_default()` returns `&'static Layout`, but `PublishLoop` (and `Startup` in the web crate) own a `Layout` by value. Every startup and every one of the 7 `PublishLoop` tests therefore clones the embedded layout. Safe (no interior mutability) but wasteful — an `Arc<Layout>` or `&'static Layout` in `PublishLoop` would avoid the clones.
**Trigger:** Any construction path that routes the embedded default through.
**Suggested fix:** Store `Arc<Layout>` in `PublishLoop` and in the web crate's `Startup`. Lets config-loaded layouts share the same cheap-clone representation; embedded default becomes `Arc::new(Layout::embedded_default().clone())` once, then cheap clones on every use.
