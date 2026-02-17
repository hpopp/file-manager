use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use super::handlers;
use crate::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    let upload_limit = state.config.max_upload_size as usize;

    let mut router = Router::new()
        // Files
        .route("/files", get(handlers::list_files))
        .route(
            "/files",
            post(handlers::create_file).layer(DefaultBodyLimit::max(upload_limit)),
        )
        .route("/files/:id", delete(handlers::delete_file))
        .route("/files/:id", get(handlers::get_file))
        .route("/files/:id", put(handlers::update_file))
        // Static content (permalink download)
        .route("/static/*permalink", get(handlers::serve_static))
        // Internal
        .route("/_internal/cluster/status", get(handlers::cluster_status))
        .route("/_internal/health", get(handlers::health));

    // Test-only routes
    if state.config.test_mode {
        tracing::warn!("Test mode enabled — purge route is available.");
        router = router.route("/admin/purge", delete(handlers::admin_purge));
    }

    router.layer(TraceLayer::new_for_http()).with_state(state)
}
