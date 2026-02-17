use std::collections::HashMap;

use axum::extract::{Multipart, Path, State};
use axum::Json;
use bytes::BytesMut;
use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;

use super::replication_error;
use crate::api::response::{ApiError, AppJson, AppQuery, JSend, JSendPaginated, Pagination};
use crate::storage::models::{FileRecord, FileType, Patch, WriteOp};
use crate::AppState;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct FileResponse {
    pub alt: Option<String>,
    pub byte_size: u64,
    pub created_at: String,
    pub description: Option<String>,
    pub file_type: FileType,
    pub id: String,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub mime_type: String,
    pub name: Option<String>,
    pub permalink: String,
    pub subject_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateFileRequest {
    #[serde(default, deserialize_with = "nullable")]
    pub alt: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub metadata: Option<Option<HashMap<String, serde_json::Value>>>,
    #[serde(default, deserialize_with = "nullable")]
    pub name: Option<Option<String>>,
    #[serde(default)]
    pub permalink: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub subject_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ListFilesParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub file_type: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
}

fn default_limit() -> u32 {
    20
}

/// Distinguishes between a missing field (`None`) and an explicit `null` (`Some(None)`).
fn nullable<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

// ============================================================================
// Handlers
// ============================================================================

pub async fn create_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<JSend<FileResponse>>, ApiError> {
    let mut file_data: Option<BytesMut> = None;
    let mut file_name: Option<String> = None;
    let mut file_content_type: Option<String> = None;
    let mut permalink: Option<String> = None;
    let mut name: Option<String> = None;
    let mut alt: Option<String> = None;
    let mut description: Option<String> = None;
    let mut metadata: Option<HashMap<String, serde_json::Value>> = None;
    let mut subject_id: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("Invalid multipart data: {e}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                file_content_type = field.content_type().map(|s| s.to_string());

                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::bad_request(format!("Failed to read file: {e}")))?;

                if data.len() as u64 > state.config.max_upload_size {
                    return Err(ApiError::payload_too_large(format!(
                        "File exceeds maximum upload size of {} bytes",
                        state.config.max_upload_size
                    )));
                }

                let mut buf = BytesMut::with_capacity(data.len());
                buf.extend_from_slice(&data);
                file_data = Some(buf);
            }
            "permalink" => {
                permalink = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::bad_request(format!("Invalid permalink: {e}")))?,
                );
            }
            "name" => {
                name = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::bad_request(format!("Invalid name: {e}")))?,
                );
            }
            "alt" => {
                alt = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::bad_request(format!("Invalid alt: {e}")))?,
                );
            }
            "description" => {
                description = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::bad_request(format!("Invalid description: {e}")))?,
                );
            }
            "subject_id" => {
                subject_id = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::bad_request(format!("Invalid subject_id: {e}")))?,
                );
            }
            "metadata" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::bad_request(format!("Invalid metadata: {e}")))?;
                let parsed: HashMap<String, serde_json::Value> = serde_json::from_str(&text)
                    .map_err(|e| {
                        ApiError::bad_request(format!("metadata must be a JSON object: {e}"))
                    })?;
                metadata = Some(parsed);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    let file_data = file_data.ok_or_else(|| ApiError::bad_request("file field is required"))?;
    let permalink =
        permalink.ok_or_else(|| ApiError::bad_request("permalink field is required"))?;

    if permalink.trim().is_empty() {
        return Err(ApiError::bad_request("permalink must not be empty"));
    }

    // Check permalink uniqueness
    if state
        .db
        .permalink_exists(&permalink)
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        return Err(ApiError::conflict(format!(
            "permalink '{permalink}' is already in use"
        )));
    }

    // Determine MIME type: from multipart Content-Type, or guess from filename, or fallback
    let mime_type = file_content_type
        .filter(|ct| ct != "application/octet-stream")
        .or_else(|| {
            file_name
                .as_deref()
                .and_then(|n| mime_guess::from_path(n).first())
                .map(|m| m.to_string())
        })
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let file_type = FileType::from_mime(&mime_type);
    let byte_size = file_data.len() as u64;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    // Phase 1: Upload bytes to object storage (keyed by UUID)
    state
        .object_store
        .put(&id, file_data.freeze())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to store file: {e}")))?;

    // Phase 2: Write metadata to redb via muster
    let file_record = FileRecord {
        id: id.clone(),
        mime_type,
        file_type,
        byte_size,
        permalink: permalink.clone(),
        created_at: now,
        updated_at: now,
        alt: alt.clone(),
        description: description.clone(),
        metadata: metadata.clone(),
        name: name.clone(),
        subject_id: subject_id.clone(),
    };

    let operation = WriteOp::CreateFile(file_record.clone());
    if let Err(e) = state.node.replicate(operation).await {
        // Best-effort cleanup of the uploaded blob
        let _ = state.object_store.delete(&id).await;
        return Err(replication_error(e));
    }

    tracing::debug!(file_id = %id, permalink = %permalink, "Created file");

    Ok(JSend::success(file_to_response(&file_record)))
}

pub async fn get_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JSend<FileResponse>>, ApiError> {
    let file = state
        .db
        .get_file(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("File not found"))?;

    Ok(JSend::success(file_to_response(&file)))
}

pub async fn update_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AppJson(req): AppJson<UpdateFileRequest>,
) -> Result<Json<JSend<FileResponse>>, ApiError> {
    // Validate at least one field is provided
    if req.alt.is_none()
        && req.description.is_none()
        && req.metadata.is_none()
        && req.name.is_none()
        && req.permalink.is_none()
        && req.subject_id.is_none()
    {
        return Err(ApiError::bad_request(
            "at least one field (alt, description, metadata, name, permalink, subject_id) must be provided",
        ));
    }

    // Verify the file exists
    let existing = state
        .db
        .get_file(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("File not found"))?;

    // If changing permalink, check uniqueness (allow keeping the same permalink)
    if let Some(ref new_permalink) = req.permalink {
        if new_permalink.trim().is_empty() {
            return Err(ApiError::bad_request("permalink must not be empty"));
        }
        if *new_permalink != existing.permalink
            && state
                .db
                .permalink_exists(new_permalink)
                .map_err(|e| ApiError::internal(e.to_string()))?
        {
            return Err(ApiError::conflict(format!(
                "permalink '{new_permalink}' is already in use"
            )));
        }
    }

    let operation = WriteOp::UpdateFile {
        id: id.clone(),
        alt: Patch::from(req.alt.clone()),
        description: Patch::from(req.description.clone()),
        metadata: Patch::from(req.metadata.clone()),
        name: Patch::from(req.name.clone()),
        permalink: req.permalink.clone(),
        subject_id: Patch::from(req.subject_id.clone()),
    };
    state
        .node
        .replicate(operation)
        .await
        .map_err(replication_error)?;

    let file = state
        .db
        .get_file(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("File not found after update"))?;

    tracing::debug!(file_id = %id, "Updated file");
    Ok(JSend::success(file_to_response(&file)))
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JSend<()>>, ApiError> {
    // Verify the file exists
    state
        .db
        .get_file(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("File not found"))?;

    // Phase 1: Remove metadata via muster
    let operation = WriteOp::DeleteFile { id: id.clone() };
    state
        .node
        .replicate(operation)
        .await
        .map_err(replication_error)?;

    // Phase 2: Delete blob from object storage (best-effort)
    if let Err(e) = state.object_store.delete(&id).await {
        tracing::warn!(file_id = %id, error = %e, "Failed to delete file from object storage");
    }

    tracing::debug!(file_id = %id, "Deleted file");
    Ok(JSend::success(()))
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    AppQuery(params): AppQuery<ListFilesParams>,
) -> Result<Json<JSendPaginated<FileResponse>>, ApiError> {
    if params.limit == 0 {
        return Err(ApiError::bad_request("limit must be greater than 0"));
    }

    match state
        .db
        .list_files(params.file_type.as_deref(), params.subject_id.as_deref())
    {
        Ok(files) => {
            let total = files.len() as u64;
            let items: Vec<FileResponse> = files
                .iter()
                .skip(params.offset as usize)
                .take(params.limit as usize)
                .map(file_to_response)
                .collect();

            Ok(JSendPaginated::success(
                items,
                Pagination {
                    limit: params.limit,
                    offset: params.offset,
                    total,
                },
            ))
        }
        Err(e) => Err(ApiError::internal(e.to_string())),
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn file_to_response(file: &FileRecord) -> FileResponse {
    FileResponse {
        alt: file.alt.clone(),
        byte_size: file.byte_size,
        created_at: file.created_at.to_rfc3339(),
        description: file.description.clone(),
        file_type: file.file_type,
        id: file.id.clone(),
        metadata: file.metadata.clone(),
        mime_type: file.mime_type.clone(),
        name: file.name.clone(),
        permalink: file.permalink.clone(),
        subject_id: file.subject_id.clone(),
        updated_at: file.updated_at.to_rfc3339(),
    }
}
