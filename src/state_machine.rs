//! file-manager's state machine for muster cluster replication.

use serde::{Deserialize, Serialize};

use crate::storage::models::{FileRecord, WriteOp};
use crate::storage::Database;

/// The file-manager state machine, replicated by muster.
pub struct FileStateMachine {
    db: Database,
}

impl FileStateMachine {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

/// Full state snapshot for syncing lagging followers.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub files: Vec<FileRecord>,
}

impl muster::StateMachine for FileStateMachine {
    type WriteOp = WriteOp;
    type Snapshot = FileSnapshot;

    fn apply(&self, op: &WriteOp) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match op {
            WriteOp::CreateFile(file) => {
                self.db.put_file(file)?;
            }
            WriteOp::DeleteFile { id } => {
                self.db.delete_file(id)?;
            }
            WriteOp::UpdateFile {
                id,
                alt,
                description,
                metadata,
                name,
                permalink,
                subject_id,
            } => {
                self.db.update_file(
                    id,
                    alt.as_option().map(|o| o.map(String::as_str)),
                    description.as_option().map(|o| o.map(String::as_str)),
                    metadata.as_option(),
                    name.as_option().map(|o| o.map(String::as_str)),
                    permalink.as_deref(),
                    subject_id.as_option().map(|o| o.map(String::as_str)),
                )?;
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<FileSnapshot, Box<dyn std::error::Error + Send + Sync>> {
        let files = self.db.get_all_files()?;
        Ok(FileSnapshot { files })
    }

    fn restore(
        &self,
        snapshot: FileSnapshot,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for file in &snapshot.files {
            self.db.put_file(file)?;
        }
        Ok(())
    }
}
