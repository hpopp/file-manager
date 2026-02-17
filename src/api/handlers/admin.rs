use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;

use crate::api::response::{ApiError, JSend};
use crate::AppState;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ClusterStatusResponse {
    pub cluster_info: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct PurgeResponse {
    pub files_deleted: u64,
}

// ============================================================================
// Handlers
// ============================================================================

pub async fn health() -> Json<JSend<HealthResponse>> {
    JSend::success(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn cluster_status(
    State(state): State<Arc<AppState>>,
) -> Json<JSend<ClusterStatusResponse>> {
    let info = state.node.cluster_info().await;
    let peers: Vec<serde_json::Value> = info
        .peers
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "address": p.address,
                "status": format!("{:?}", p.status),
                "sequence": p.sequence,
            })
        })
        .collect();

    JSend::success(ClusterStatusResponse {
        cluster_info: serde_json::json!({
            "node_id": info.node_id,
            "role": format!("{:?}", info.role),
            "term": info.term,
            "leader_id": info.leader_id,
            "peers": peers,
            "sequence": info.sequence,
        }),
    })
}

pub async fn admin_purge(
    State(state): State<Arc<AppState>>,
) -> Result<Json<JSend<PurgeResponse>>, ApiError> {
    let stats = state
        .db
        .purge_all()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    tracing::warn!(files = stats.files, "Purged all data");

    Ok(JSend::success(PurgeResponse {
        files_deleted: stats.files,
    }))
}
