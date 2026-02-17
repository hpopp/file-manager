use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use serde::Deserialize;

use super::{ObjectStore, ObjectStoreError};

/// Google Cloud Storage object store backend.
pub struct GcsStore {
    bucket: String,
    client: Client,
    access_token: tokio::sync::RwLock<String>,
    credentials_file: Option<String>,
}

#[derive(Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    token_uri: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

impl GcsStore {
    pub async fn new(bucket: &str, credentials_file: Option<&str>) -> Result<Self, anyhow::Error> {
        let client = Client::builder().build()?;

        let store = Self {
            bucket: bucket.to_string(),
            client,
            access_token: tokio::sync::RwLock::new(String::new()),
            credentials_file: credentials_file.map(|s| s.to_string()),
        };

        store.refresh_token().await?;
        Ok(store)
    }

    async fn refresh_token(&self) -> Result<(), anyhow::Error> {
        let token = if let Some(ref creds_path) = self.credentials_file {
            self.token_from_service_account(creds_path).await?
        } else {
            self.token_from_metadata_server().await?
        };

        let mut lock = self.access_token.write().await;
        *lock = token;
        Ok(())
    }

    async fn token_from_service_account(&self, path: &str) -> Result<String, anyhow::Error> {
        let key_json = tokio::fs::read_to_string(path).await?;
        let key: ServiceAccountKey = serde_json::from_str(&key_json)?;

        let now = chrono::Utc::now().timestamp();
        let claims = serde_json::json!({
            "iss": key.client_email,
            "scope": "https://www.googleapis.com/auth/devstorage.read_write",
            "aud": key.token_uri,
            "iat": now,
            "exp": now + 3600,
        });

        // Build JWT (header.claims.signature)
        let header = base64_url_encode(&serde_json::to_vec(&serde_json::json!({
            "alg": "RS256",
            "typ": "JWT"
        }))?);
        let payload = base64_url_encode(&serde_json::to_vec(&claims)?);
        let unsigned = format!("{header}.{payload}");

        let signature = sign_rs256(unsigned.as_bytes(), &key.private_key)?;
        let jwt = format!("{unsigned}.{}", base64_url_encode(&signature));

        let resp: TokenResponse = self
            .client
            .post(&key.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.access_token)
    }

    async fn token_from_metadata_server(&self) -> Result<String, anyhow::Error> {
        let resp: TokenResponse = self
            .client
            .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor", "Google")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.access_token)
    }

    fn upload_url(&self, key: &str) -> String {
        format!(
            "https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&name={}",
            self.bucket, key
        )
    }

    fn object_url(&self, key: &str) -> String {
        format!(
            "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
            self.bucket, key
        )
    }

    fn delete_url(&self, key: &str) -> String {
        format!(
            "https://storage.googleapis.com/storage/v1/b/{}/o/{}",
            self.bucket, key
        )
    }

    fn metadata_url(&self, key: &str) -> String {
        format!(
            "https://storage.googleapis.com/storage/v1/b/{}/o/{}",
            self.bucket, key
        )
    }
}

#[async_trait]
impl ObjectStore for GcsStore {
    async fn put(&self, key: &str, data: Bytes) -> Result<(), ObjectStoreError> {
        let token = self.access_token.read().await.clone();

        let resp = self
            .client
            .post(self.upload_url(key))
            .bearer_auth(&token)
            .header("Content-Type", "application/octet-stream")
            .body(data)
            .send()
            .await
            .map_err(|e| ObjectStoreError::Backend(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ObjectStoreError::Backend(format!(
                "GCS upload failed ({status}): {body}"
            )));
        }

        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes, ObjectStoreError> {
        let token = self.access_token.read().await.clone();

        let resp = self
            .client
            .get(self.object_url(key))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| ObjectStoreError::Backend(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ObjectStoreError::NotFound(key.to_string()));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ObjectStoreError::Backend(format!(
                "GCS download failed ({status}): {body}"
            )));
        }

        let data = resp
            .bytes()
            .await
            .map_err(|e| ObjectStoreError::Backend(e.to_string()))?;

        Ok(data)
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        let token = self.access_token.read().await.clone();

        let resp = self
            .client
            .delete(self.delete_url(key))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| ObjectStoreError::Backend(e.to_string()))?;

        // 404 is fine -- object already gone
        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ObjectStoreError::Backend(format!(
                "GCS delete failed ({status}): {body}"
            )));
        }

        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError> {
        let token = self.access_token.read().await.clone();

        let resp = self
            .client
            .get(self.metadata_url(key))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| ObjectStoreError::Backend(e.to_string()))?;

        Ok(resp.status().is_success())
    }
}

fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn sign_rs256(data: &[u8], private_key_pem: &str) -> Result<Vec<u8>, anyhow::Error> {
    use std::io::Read;

    // Parse PEM to DER
    let pem_bytes = private_key_pem.as_bytes();
    let mut reader = std::io::Cursor::new(pem_bytes);
    let mut pem_content = String::new();
    reader.read_to_string(&mut pem_content)?;

    // Strip PEM headers and decode base64
    let der_b64: String = pem_content
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    let der = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &der_b64)?;

    // Use ring for RSA signing
    let key_pair = ring::signature::RsaKeyPair::from_pkcs8(&der)
        .map_err(|e| anyhow::anyhow!("Failed to parse RSA key: {e}"))?;

    let mut signature = vec![0u8; key_pair.public().modulus_len()];
    key_pair
        .sign(
            &ring::signature::RSA_PKCS1_SHA256,
            &ring::rand::SystemRandom::new(),
            data,
            &mut signature,
        )
        .map_err(|e| anyhow::anyhow!("Failed to sign: {e}"))?;

    Ok(signature)
}
