---
name: web-dev
description: >
  Web development patterns for Axum backend, Svelte 5
  frontend, Vite configuration, and Playwright E2E
  testing. Use when debugging frontend/backend issues,
  writing E2E tests, or adding API endpoints.
invocation: >
  Use /web-dev when working on frontend, backend, or
  E2E testing code.
---

# Web Development Guide

## Axum Backend Patterns

### Router Setup

```rust
Router::new()
    .route("/health", get(health))
    .nest("/api", api_routes())
    .fallback_service(serve_dir)
    .layer(TraceLayer::new_for_http())
```

- Health check at `/health` (plain text "OK")
- All API routes nested under `/api`
- Fallback serves static files with SPA fallback
  to `index.html`

### Handler Pattern

```rust
#[derive(Serialize)]
struct MyResponse { field: String }

async fn my_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(MyResponse { ... }))
}
```

Always return `(StatusCode, Json<T>)` for API
endpoints. Use `impl IntoResponse` return type.

### Testing Handlers

```rust
use tower::ServiceExt;
use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn test_endpoint() {
    let app = create_router("nonexistent");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/my-endpoint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

The `"nonexistent"` frontend path is fine for API
tests -- static file fallback won't trigger.

## Svelte 5 Frontend

### Runes (Svelte 5 reactivity)

```svelte
<script>
  let count = $state(0);
  let doubled = $derived(count * 2);

  function increment() { count += 1; }
</script>
```

- `$state()` for reactive state
- `$derived()` for computed values
- `$effect()` for side effects
- `onMount()` for initialization

### Fetching Data

```svelte
<script>
  import { onMount } from "svelte";
  let data = $state(null);

  onMount(async () => {
    const res = await fetch("/api/endpoint");
    data = await res.json();
  });
</script>
```

In dev mode, Vite proxies `/api` to the backend.
In production, both are served from the same origin.

### CSS Custom Properties

Use variables from `app.css`:
- `--bg` -- background
- `--text` -- primary text
- `--muted` -- secondary text
- `--accent` -- highlight color
- `--surface` -- card background
- `--border` -- borders

## Vite Configuration

### Port Configuration

`vite.config.js` reads `.ports` file for backend
port. Default is 3000. The `.ports` file uses
`backend_port=NNNN` format.

### Version Injection

`__APP_VERSION__` is injected at build time from
`crates/rustbase/Cargo.toml`. Access in Svelte:

```svelte
<span>{__APP_VERSION__}</span>
```

## Playwright E2E Testing

### Running Tests

```bash
npx playwright test              # all tests
npx playwright test smoke        # filtered
npx playwright test --ui         # interactive UI
```

Playwright auto-starts both backend and frontend
servers (configured in `playwright.config.js`).

### Test Pattern

```javascript
import { test, expect } from "@playwright/test";

test("page loads", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("h1")).toContainText("text");
});

test("API works", async ({ request }) => {
  const res = await request.get("/api/endpoint");
  expect(res.ok()).toBeTruthy();
  const json = await res.json();
  expect(json.field).toBe("value");
});
```

### Port Configuration

`playwright.config.js` reads `.ports` file (same
format as Vite). It starts:
1. Backend: `cargo run -p rustbase-web -- --port N`
2. Frontend: `cd frontend && npm run dev`

## Dev Workflow

### Start Development

```bash
# Terminal 1: backend
cargo run -p rustbase-web

# Terminal 2: frontend (hot reload)
cd frontend && npm run dev

# Browser: http://localhost:5173
```

### Run E2E Tests

```bash
# Auto-starts both servers:
npx playwright test

# Or with UI mode:
npx playwright test --ui
```

### Production Build

```bash
cd frontend && npm run build
cargo build --release -p rustbase-web
# Serve: ./target/release/rustbase-web -f frontend/dist
```

## Common Issues

### CORS in Development

Not needed -- Vite proxy handles `/api` routing in
dev mode. In production, both are same-origin.

### Port Conflicts

Copy `.ports.sample` to `.ports` and change
`backend_port` to avoid conflicts with other
instances.

### Use 127.0.0.1 not localhost

In PowerShell, `localhost` can resolve to IPv6.
Always use `127.0.0.1` for HTTP requests.
