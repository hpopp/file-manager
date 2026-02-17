use std::collections::HashMap;

use redb::ReadableTable;

use super::db::{Database, DatabaseError};
use super::models::FileRecord;
use super::tables::*;

impl Database {
    // ========================================================================
    // File operations
    // ========================================================================

    /// Store a file record and update the permalink and subject indexes
    pub fn put_file(&self, file: &FileRecord) -> Result<(), DatabaseError> {
        debug_assert!(!file.id.is_empty(), "file id must not be empty");
        debug_assert!(
            !file.permalink.is_empty(),
            "file permalink must not be empty"
        );

        let write_txn = self.begin_write()?;
        {
            let mut table = write_txn.open_table(FILES)?;
            let data = rmp_serde::to_vec_named(file)?;
            table.insert(file.id.as_str(), data.as_slice())?;

            let mut permalink_table = write_txn.open_table(FILE_PERMALINKS)?;
            permalink_table.insert(file.permalink.as_str(), file.id.as_str())?;

            // Maintain subject index
            if let Some(ref subject_id) = file.subject_id {
                let mut subject_table = write_txn.open_table(SUBJECT_FILES)?;
                let mut file_ids: Vec<String> = subject_table
                    .get(subject_id.as_str())?
                    .map(|v| rmp_serde::from_slice(v.value()).unwrap_or_default())
                    .unwrap_or_default();

                if !file_ids.contains(&file.id) {
                    file_ids.push(file.id.clone());
                    let index_data = rmp_serde::to_vec_named(&file_ids)?;
                    subject_table.insert(subject_id.as_str(), index_data.as_slice())?;
                }
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a file by its UUID
    pub fn get_file(&self, id: &str) -> Result<Option<FileRecord>, DatabaseError> {
        let read_txn = self.begin_read()?;
        let table = read_txn.open_table(FILES)?;

        match table.get(id)? {
            Some(data) => {
                let file: FileRecord = rmp_serde::from_slice(data.value())?;
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }

    /// Get a file by its permalink (resolves permalink -> uuid -> file)
    pub fn get_file_by_permalink(
        &self,
        permalink: &str,
    ) -> Result<Option<FileRecord>, DatabaseError> {
        let read_txn = self.begin_read()?;
        let permalink_table = read_txn.open_table(FILE_PERMALINKS)?;

        let id = match permalink_table.get(permalink)? {
            Some(data) => data.value().to_string(),
            None => return Ok(None),
        };

        let files_table = read_txn.open_table(FILES)?;
        match files_table.get(id.as_str())? {
            Some(data) => {
                let file: FileRecord = rmp_serde::from_slice(data.value())?;
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }

    /// Get all files for a subject
    pub fn get_files_by_subject(&self, subject_id: &str) -> Result<Vec<FileRecord>, DatabaseError> {
        let read_txn = self.begin_read()?;
        let subject_table = read_txn.open_table(SUBJECT_FILES)?;
        let files_table = read_txn.open_table(FILES)?;

        let file_ids: Vec<String> = match subject_table.get(subject_id)? {
            Some(data) => rmp_serde::from_slice(data.value())?,
            None => return Ok(Vec::new()),
        };

        let mut files = Vec::new();
        for file_id in file_ids {
            if let Some(data) = files_table.get(file_id.as_str())? {
                let file: FileRecord = rmp_serde::from_slice(data.value())?;
                files.push(file);
            }
        }

        Ok(files)
    }

    /// Delete a file by its UUID and clean up the permalink and subject indexes
    pub fn delete_file(&self, id: &str) -> Result<bool, DatabaseError> {
        let write_txn = self.begin_write()?;

        // Get the file for index cleanup
        let file_info: Option<(String, Option<String>)> = {
            let table = write_txn.open_table(FILES)?;
            let result = match table.get(id)? {
                Some(data) => {
                    let file: FileRecord = rmp_serde::from_slice(data.value())?;
                    Some((file.permalink, file.subject_id))
                }
                None => None,
            };
            result
        };

        let deleted = match file_info {
            Some((permalink, subject_id)) => {
                // Remove from files table
                {
                    let mut table = write_txn.open_table(FILES)?;
                    table.remove(id)?;
                }
                // Remove from permalink index
                {
                    let mut permalink_table = write_txn.open_table(FILE_PERMALINKS)?;
                    permalink_table.remove(permalink.as_str())?;
                }
                // Remove from subject index
                if let Some(ref subject_id) = subject_id {
                    let file_ids: Option<Vec<String>> = {
                        let subject_table = write_txn.open_table(SUBJECT_FILES)?;
                        let result = subject_table.get(subject_id.as_str())?;
                        match result {
                            Some(data) => Some(rmp_serde::from_slice(data.value())?),
                            None => None,
                        }
                    };

                    if let Some(mut ids) = file_ids {
                        ids.retain(|fid| fid != id);
                        let mut subject_table = write_txn.open_table(SUBJECT_FILES)?;
                        if ids.is_empty() {
                            subject_table.remove(subject_id.as_str())?;
                        } else {
                            let new_data = rmp_serde::to_vec_named(&ids)?;
                            subject_table.insert(subject_id.as_str(), new_data.as_slice())?;
                        }
                    }
                }
                true
            }
            None => false,
        };

        write_txn.commit()?;
        Ok(deleted)
    }

    /// Update a file's mutable fields
    #[allow(clippy::too_many_arguments)]
    pub fn update_file(
        &self,
        id: &str,
        alt: Option<Option<&str>>,
        description: Option<Option<&str>>,
        metadata: Option<Option<&HashMap<String, serde_json::Value>>>,
        name: Option<Option<&str>>,
        permalink: Option<&str>,
        subject_id: Option<Option<&str>>,
    ) -> Result<bool, DatabaseError> {
        let write_txn = self.begin_write()?;

        let existing = {
            let table = write_txn.open_table(FILES)?;
            let result = match table.get(id)? {
                Some(data) => {
                    let file: FileRecord = rmp_serde::from_slice(data.value())?;
                    Some(file)
                }
                None => None,
            };
            result
        };

        let updated = match existing {
            Some(mut file) => {
                if let Some(a) = alt {
                    file.alt = a.map(|s| s.to_string());
                }
                if let Some(d) = description {
                    file.description = d.map(|s| s.to_string());
                }
                if let Some(m) = metadata {
                    file.metadata = m.cloned();
                }
                if let Some(n) = name {
                    file.name = n.map(|s| s.to_string());
                }
                if let Some(new_permalink) = permalink {
                    // Remove old permalink index entry
                    {
                        let mut permalink_table = write_txn.open_table(FILE_PERMALINKS)?;
                        permalink_table.remove(file.permalink.as_str())?;
                    }
                    file.permalink = new_permalink.to_string();
                    // Add new permalink index entry
                    {
                        let mut permalink_table = write_txn.open_table(FILE_PERMALINKS)?;
                        permalink_table.insert(new_permalink, id)?;
                    }
                }
                // Handle subject_id change with index maintenance
                if let Some(new_subject) = subject_id {
                    let old_subject = file.subject_id.clone();

                    // Remove from old subject index
                    if let Some(ref old_sid) = old_subject {
                        let old_ids: Option<Vec<String>> = {
                            let subject_table = write_txn.open_table(SUBJECT_FILES)?;
                            let result = match subject_table.get(old_sid.as_str())? {
                                Some(data) => Some(rmp_serde::from_slice(data.value())?),
                                None => None,
                            };
                            result
                        };
                        if let Some(mut ids) = old_ids {
                            ids.retain(|fid| fid != id);
                            let mut subject_table = write_txn.open_table(SUBJECT_FILES)?;
                            if ids.is_empty() {
                                subject_table.remove(old_sid.as_str())?;
                            } else {
                                let data = rmp_serde::to_vec_named(&ids)?;
                                subject_table.insert(old_sid.as_str(), data.as_slice())?;
                            }
                        }
                    }

                    // Add to new subject index
                    file.subject_id = new_subject.map(|s| s.to_string());
                    if let Some(ref new_sid) = file.subject_id {
                        let mut subject_table = write_txn.open_table(SUBJECT_FILES)?;
                        let mut file_ids: Vec<String> = subject_table
                            .get(new_sid.as_str())?
                            .map(|v| rmp_serde::from_slice(v.value()).unwrap_or_default())
                            .unwrap_or_default();

                        if !file_ids.contains(&id.to_string()) {
                            file_ids.push(id.to_string());
                            let data = rmp_serde::to_vec_named(&file_ids)?;
                            subject_table.insert(new_sid.as_str(), data.as_slice())?;
                        }
                    }
                }

                file.updated_at = chrono::Utc::now();

                let serialized = rmp_serde::to_vec_named(&file)?;
                let mut table = write_txn.open_table(FILES)?;
                table.insert(id, serialized.as_slice())?;
                true
            }
            None => false,
        };

        write_txn.commit()?;
        Ok(updated)
    }

    /// Get all files (for snapshot/restore)
    pub fn get_all_files(&self) -> Result<Vec<FileRecord>, DatabaseError> {
        let read_txn = self.begin_read()?;
        let table = read_txn.open_table(FILES)?;

        let mut files = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let file: FileRecord = rmp_serde::from_slice(value.value())?;
            files.push(file);
        }

        Ok(files)
    }

    /// List files with optional file_type and subject_id filters
    pub fn list_files(
        &self,
        file_type: Option<&str>,
        subject_id: Option<&str>,
    ) -> Result<Vec<FileRecord>, DatabaseError> {
        // Use subject index when subject_id is provided
        let all = match subject_id {
            Some(sid) => self.get_files_by_subject(sid)?,
            None => self.get_all_files()?,
        };

        if let Some(ft) = file_type {
            Ok(all
                .into_iter()
                .filter(|f| {
                    let type_str = serde_json::to_string(&f.file_type)
                        .unwrap_or_default()
                        .trim_matches('"')
                        .to_string();
                    type_str == ft
                })
                .collect())
        } else {
            Ok(all)
        }
    }

    /// Check if a permalink is already in use
    pub fn permalink_exists(&self, permalink: &str) -> Result<bool, DatabaseError> {
        let read_txn = self.begin_read()?;
        let table = read_txn.open_table(FILE_PERMALINKS)?;
        Ok(table.get(permalink)?.is_some())
    }
}
