use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use crate::api::response::ApiError;
use crate::AppState;

/// Serve file content by permalink.
/// Route: GET /static/*permalink
pub async fn serve_static(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(permalink): axum::extract::Path<String>,
) -> Result<Response, ApiError> {
    // Look up file metadata by permalink
    let file = state
        .db
        .get_file_by_permalink(&permalink)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("File not found"))?;

    // Fetch content from object storage
    let data = state
        .object_store
        .get(&file.id)
        .await
        .map_err(|e| match e {
            crate::object_store::ObjectStoreError::NotFound(_) => {
                ApiError::not_found("File content not found")
            }
            _ => ApiError::internal(format!("Failed to retrieve file: {e}")),
        })?;

    // Build response with appropriate headers
    let mut response = (StatusCode::OK, data).into_response();
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_TYPE,
        file.mime_type
            .parse()
            .unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
    );

    headers.insert(
        header::CONTENT_LENGTH,
        header::HeaderValue::from(file.byte_size),
    );

    // Set Content-Disposition with filename from the permalink's last segment
    let filename = permalink.rsplit('/').next().unwrap_or(&permalink);
    if let Ok(value) = format!("inline; filename=\"{filename}\"").parse() {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }

    // Cache for 1 hour (files are immutable once uploaded, only metadata changes)
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("public, max-age=3600"),
    );

    Ok(response)
}
