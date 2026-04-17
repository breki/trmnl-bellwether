use std::path::Path;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

pub mod trmnl;

pub use trmnl::{RefreshInterval, TrmnlState};

/// Create the application router.
///
/// - `/health` — health check endpoint
/// - `/api/*` — scaffold API (`/status`, `/greeting`)
///   plus TRMNL BYOS endpoints (`/api/display`,
///   `/api/log`)
/// - `/images/{filename}` — rendered BMPs for the
///   TRMNL device to fetch
/// - Everything else — serves static files from
///   `frontend_path`, falling back to `index.html` for
///   SPA client-side routing.
pub fn create_router(frontend_path: &Path, trmnl: TrmnlState) -> Router {
    let index_path = frontend_path.join("index.html");
    let serve_dir = ServeDir::new(frontend_path)
        .not_found_service(ServeFile::new(&index_path));

    Router::new()
        .route("/health", get(health))
        .nest("/api", scaffold_api())
        .merge(trmnl::router(trmnl))
        .fallback_service(serve_dir)
        .layer(TraceLayer::new_for_http())
}

fn scaffold_api() -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/greeting", get(greeting))
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

#[derive(Serialize)]
struct GreetingResponse {
    message: &'static str,
}

async fn greeting() -> Json<GreetingResponse> {
    Json(GreetingResponse {
        message: "Hello from bellwether!",
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    fn test_router() -> Router {
        create_router(
            Path::new("nonexistent"),
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
    async fn greeting_returns_message() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/greeting")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // Sanity check that the nested routers don't collide
    // at startup — if scaffold_api and trmnl::router
    // grow overlapping paths, this test fails rather than
    // the binary panicking at first request.
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
