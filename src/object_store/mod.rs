mod gcs;
mod local;

pub use gcs::GcsStore;
pub use local::LocalStore;

use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Object not found: {0}")]
    NotFound(String),
    #[error("Backend error: {0}")]
    Backend(String),
}

/// Abstraction over object storage backends.
/// Keys are UUIDs -- the raw blobs are meaningless without the metadata DB.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, key: &str, data: Bytes) -> Result<(), ObjectStoreError>;
    async fn get(&self, key: &str) -> Result<Bytes, ObjectStoreError>;
    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError>;
    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError>;
}
