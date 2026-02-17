use async_trait::async_trait;
use bytes::Bytes;
use std::path::{Path, PathBuf};

use super::{ObjectStore, ObjectStoreError};

/// Local filesystem object store for development and testing.
pub struct LocalStore {
    base_path: PathBuf,
}

impl LocalStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self, std::io::Error> {
        let base_path = base_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }

    fn object_path(&self, key: &str) -> PathBuf {
        self.base_path.join(key)
    }
}

#[async_trait]
impl ObjectStore for LocalStore {
    async fn put(&self, key: &str, data: Bytes) -> Result<(), ObjectStoreError> {
        let path = self.object_path(key);
        tokio::fs::write(&path, &data).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes, ObjectStoreError> {
        let path = self.object_path(key);
        if !path.exists() {
            return Err(ObjectStoreError::NotFound(key.to_string()));
        }
        let data = tokio::fs::read(&path).await?;
        Ok(Bytes::from(data))
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        let path = self.object_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError> {
        let path = self.object_path(key);
        Ok(path.exists())
    }
}
