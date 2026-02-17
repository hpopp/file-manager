use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Three-state patch value for partial updates that survives serialization round-trips.
/// Unlike `Option<Option<T>>`, each variant has a distinct wire representation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum Patch<T> {
    /// Field was not included in the request (no change).
    #[default]
    Absent,
    /// Field was explicitly set to null (clear it).
    Null,
    /// Field was set to a new value.
    Value(T),
}

impl<T> From<Option<Option<T>>> for Patch<T> {
    fn from(v: Option<Option<T>>) -> Self {
        match v {
            None => Patch::Absent,
            Some(None) => Patch::Null,
            Some(Some(v)) => Patch::Value(v),
        }
    }
}

impl<T> Patch<T> {
    /// Convert to the `Option<Option<&T>>` form that storage operations expect.
    pub fn as_option(&self) -> Option<Option<&T>> {
        match self {
            Patch::Absent => None,
            Patch::Null => Some(None),
            Patch::Value(v) => Some(Some(v)),
        }
    }

    pub fn is_absent(&self) -> bool {
        matches!(self, Patch::Absent)
    }
}

/// Classification of a file derived from its MIME type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Audio,
    Binary,
    Document,
    Image,
    Video,
}

impl FileType {
    /// Derive a file type classification from a MIME type string.
    pub fn from_mime(mime_type: &str) -> Self {
        let primary = mime_type.split('/').next().unwrap_or("");
        match primary {
            "audio" => FileType::Audio,
            "image" => FileType::Image,
            "video" => FileType::Video,
            "text" | "application" => {
                let sub = mime_type.split('/').nth(1).unwrap_or("");
                match sub {
                    "pdf"
                    | "msword"
                    | "rtf"
                    | "csv"
                    | "vnd.openxmlformats-officedocument.wordprocessingml.document"
                    | "vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                    | "vnd.openxmlformats-officedocument.presentationml.presentation"
                    | "vnd.ms-excel"
                    | "vnd.ms-powerpoint" => FileType::Document,
                    _ if primary == "text" => FileType::Document,
                    _ => FileType::Binary,
                }
            }
            _ => FileType::Binary,
        }
    }
}

/// A file record stored in redb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    // System fields
    pub id: String,
    pub mime_type: String,
    pub file_type: FileType,
    pub byte_size: u64,
    pub permalink: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // CMS fields (all optional)
    #[serde(default)]
    pub alt: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
}

/// Types of write operations (replicated via muster)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteOp {
    CreateFile(FileRecord),
    DeleteFile {
        id: String,
    },
    UpdateFile {
        id: String,
        #[serde(default)]
        alt: Patch<String>,
        #[serde(default)]
        description: Patch<String>,
        #[serde(default)]
        metadata: Patch<HashMap<String, serde_json::Value>>,
        #[serde(default)]
        name: Patch<String>,
        permalink: Option<String>,
        #[serde(default)]
        subject_id: Patch<String>,
    },
}
