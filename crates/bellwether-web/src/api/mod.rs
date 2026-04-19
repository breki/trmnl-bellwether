use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::trace::TraceLayer;

pub mod trmnl;

pub use trmnl::{RefreshInterval, TrmnlState};

/// Create the application router.
///
/// - `/` — hand-rolled HTML landing page listing the
///   available endpoints and showing the latest
///   rendered dashboard image.
/// - `/health` — plain health-check endpoint.
/// - `/api/status` — version + status JSON.
/// - `/api/display`, `/api/setup`, `/api/log` — TRMNL
///   BYOS endpoints.
/// - `/images/{filename}` — rendered BMPs for the TRMNL
///   device to fetch.
pub fn create_router(trmnl: TrmnlState) -> Router {
    Router::new()
        .route("/", get(landing_page))
        .route("/health", get(health))
        .route("/api/status", get(status))
        .merge(trmnl::router(trmnl))
        .layer(TraceLayer::new_for_http())
}

/// Health check response.
#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
    version: String,
}

async fn status() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ready",
        version: bellwether::version().into(),
    })
}

/// Landing page: a minimal self-describing HTML page
/// listing the server's endpoints and showing the
/// latest rendered dashboard image. Served at `/`.
#[allow(clippy::unused_async)]
async fn landing_page() -> Html<String> {
    let version = bellwether::version();
    Html(format!(
        "<!doctype html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<title>bellwether — TRMNL dashboard server</title>\n\
<style>\n\
  :root {{\n\
    color-scheme: light dark;\n\
    --mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;\n\
  }}\n\
  body {{ font: 15px/1.5 system-ui, sans-serif; \
          max-width: 48rem; margin: 2rem auto; \
          padding: 0 1rem; }}\n\
  h1 {{ margin-bottom: 0.25rem; }}\n\
  .sub {{ opacity: 0.7; margin-top: 0; }}\n\
  table {{ border-collapse: collapse; width: 100%; \
           margin-top: 1rem; }}\n\
  th, td {{ text-align: left; padding: 0.35rem 0.5rem; \
            border-bottom: 1px solid #8884; }}\n\
  code, pre {{ font-family: var(--mono); font-size: 13px; }}\n\
  code {{ background: #8882; padding: 0.1rem 0.3rem; \
          border-radius: 3px; }}\n\
  .preview {{ margin-top: 1.5rem; text-align: center; }}\n\
  .preview img {{ max-width: 100%; \
                  image-rendering: pixelated; \
                  border: 1px solid #8884; }}\n\
  .preview small {{ display: block; opacity: 0.6; \
                    margin-top: 0.5rem; }}\n\
</style>\n\
</head>\n\
<body>\n\
<h1>bellwether</h1>\n\
<p class=\"sub\">TRMNL dashboard server \
 · v{version}</p>\n\
\n\
<h2>Endpoints</h2>\n\
<table>\n\
<tr><th>Path</th><th>Purpose</th></tr>\n\
<tr><td><code>GET /health</code></td>\n\
    <td>Liveness probe. Returns <code>{{\"status\":\"ok\"}}</code>.</td></tr>\n\
<tr><td><code>GET /api/status</code></td>\n\
    <td>Server status + version.</td></tr>\n\
<tr><td><code>GET /api/display</code></td>\n\
    <td>TRMNL BYOS display poll. Returns the manifest \
        pointing at the latest rendered BMP.</td></tr>\n\
<tr><td><code>GET /api/setup</code></td>\n\
    <td>TRMNL BYOS first-boot registration. Returns \
        <code>api_key</code>, <code>friendly_id</code>, \
        and the current image URL.</td></tr>\n\
<tr><td><code>POST /api/log</code></td>\n\
    <td>TRMNL device log ingest.</td></tr>\n\
<tr><td><code>GET /images/&lt;filename&gt;</code></td>\n\
    <td>Rendered BMPs served to the TRMNL device.</td></tr>\n\
</table>\n\
\n\
<h2>Latest rendered dashboard</h2>\n\
<div class=\"preview\">\n\
  <img src=\"/api/display?preview=1\" alt=\"latest \
       rendered dashboard\"\n\
       onerror=\"this.replaceWith(\
                  Object.assign(\
                    document.createElement('p'),\
                    {{textContent:'No image yet — the \
                      publish loop hasn\\\\'t produced \
                      one.'}}));\">\n\
  <small>Auto-refresh this page to see new renders.</small>\n\
</div>\n\
</body>\n\
</html>\n",
    ))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    fn test_router() -> Router {
        create_router(
            TrmnlState::new(
                "http://host.test/images",
                RefreshInterval::from_secs(900),
            )
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn status_returns_version() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ready");
        assert!(!json["version"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn landing_page_lists_endpoints() {
        let app = test_router();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(html.contains("bellwether"));
        assert!(html.contains("/api/display"));
        assert!(html.contains("/api/setup"));
        assert!(html.contains("/health"));
    }

    // Sanity check that the merged routers don't
    // collide at startup — if the root app and
    // trmnl::router grow overlapping paths, this test
    // fails rather than the binary panicking at first
    // request.
    #[tokio::test]
    async fn trmnl_routes_reachable_through_create_router() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/display")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
