use std::path::Path;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

/// Create the application router.
///
/// - `/health` -- health check endpoint
/// - `/api/*` -- API routes
/// - Everything else -- serves static files from
///   `frontend_path`, falling back to `index.html`
///   for SPA client-side routing.
pub fn create_router(frontend_path: &Path) -> Router {
    let index_path = frontend_path.join("index.html");
    let serve_dir = ServeDir::new(frontend_path)
        .not_found_service(ServeFile::new(&index_path));

    Router::new()
        .route("/health", get(health))
        .nest("/api", api_routes())
        .fallback_service(serve_dir)
        .layer(TraceLayer::new_for_http())
}

fn api_routes() -> Router {
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
        version: rustbase::version().into(),
    })
}

#[derive(Serialize)]
struct GreetingResponse {
    message: &'static str,
}

async fn greeting() -> Json<GreetingResponse> {
    Json(GreetingResponse {
        message: "Hello from rustbase!",
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let app = create_router(Path::new("nonexistent"));
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
        let app = create_router(Path::new("nonexistent"));
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
        let app = create_router(Path::new("nonexistent"));
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
}
