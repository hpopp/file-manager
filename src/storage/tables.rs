use redb::TableDefinition;

/// File records: uuid -> FileRecord (msgpack)
pub const FILES: TableDefinition<&str, &[u8]> = TableDefinition::new("files");

/// Permalink index: permalink -> uuid (for /static/ route lookups)
pub const FILE_PERMALINKS: TableDefinition<&str, &str> = TableDefinition::new("file_permalinks");

/// Subject index: subject_id -> msgpack Vec of file UUIDs
pub const SUBJECT_FILES: TableDefinition<&str, &[u8]> = TableDefinition::new("subject_files");
