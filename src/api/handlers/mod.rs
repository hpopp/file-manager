mod admin;
mod files;
mod static_files;

use crate::api::response::ApiError;

pub use admin::{admin_purge, cluster_status, health};
pub use files::{create_file, delete_file, get_file, list_files, update_file};
pub use static_files::serve_static;

/// Map a MusterError to an ApiError
fn replication_error(e: muster::MusterError) -> ApiError {
    match e {
        muster::MusterError::NotLeader { .. } => {
            ApiError::unavailable("No leader available — retry shortly")
        }
        muster::MusterError::NoQuorum => {
            ApiError::unavailable("Failed to reach quorum for replication")
        }
        _ => ApiError::internal(e.to_string()),
    }
}
