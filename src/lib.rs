//! file-manager - A unified internal API for file storage and CMS-like file management
//!
//! This crate provides file upload, metadata management, and content serving with:
//! - Swappable object storage backends (local filesystem, GCS)
//! - File metadata replicated via muster (Raft-like clustering)
//! - redb embedded database for metadata (ACID, MVCC, crash-safe)
//! - REST API with multipart upload support

pub mod api;
pub mod config;
pub mod object_store;
pub mod state_machine;
pub mod storage;
#[cfg(test)]
pub mod testutil;

use std::sync::Arc;

use config::Config;
use state_machine::FileStateMachine;
use storage::Database;

/// Shared application state
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub node: Arc<muster::RedbNode<FileStateMachine>>,
    pub object_store: Arc<dyn object_store::ObjectStore>,
}
