use std::collections::HashMap;

use chrono::Utc;
use file_manager::storage::models::{FileRecord, FileType};
use file_manager::storage::Database;

fn test_db() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path().join("data")).unwrap();
    (dir, db)
}

fn sample_file(id: &str, permalink: &str) -> FileRecord {
    let now = Utc::now();
    FileRecord {
        id: id.to_string(),
        mime_type: "image/png".to_string(),
        file_type: FileType::Image,
        byte_size: 1024,
        permalink: permalink.to_string(),
        created_at: now,
        updated_at: now,
        alt: Some("test alt".to_string()),
        description: None,
        metadata: None,
        name: Some("Test File".to_string()),
        subject_id: None,
    }
}

fn sample_file_with_subject(id: &str, permalink: &str, subject_id: &str) -> FileRecord {
    let mut file = sample_file(id, permalink);
    file.subject_id = Some(subject_id.to_string());
    file
}

#[test]
fn test_put_and_get_file() {
    let (_dir, db) = test_db();
    let file = sample_file("file-1", "images/test.png");

    db.put_file(&file).unwrap();

    let retrieved = db.get_file("file-1").unwrap().expect("file should exist");
    assert_eq!(retrieved.id, "file-1");
    assert_eq!(retrieved.permalink, "images/test.png");
    assert_eq!(retrieved.name, Some("Test File".to_string()));
    assert_eq!(retrieved.alt, Some("test alt".to_string()));
    assert_eq!(retrieved.mime_type, "image/png");
    assert_eq!(retrieved.file_type, FileType::Image);
    assert_eq!(retrieved.subject_id, None);
    assert_eq!(retrieved.metadata, None);
}

#[test]
fn test_get_file_by_permalink() {
    let (_dir, db) = test_db();
    let file = sample_file("file-2", "docs/readme.txt");
    db.put_file(&file).unwrap();

    let retrieved = db
        .get_file_by_permalink("docs/readme.txt")
        .unwrap()
        .expect("file should exist");
    assert_eq!(retrieved.id, "file-2");
}

#[test]
fn test_get_file_not_found() {
    let (_dir, db) = test_db();
    assert!(db.get_file("nonexistent").unwrap().is_none());
}

#[test]
fn test_get_file_by_permalink_not_found() {
    let (_dir, db) = test_db();
    assert!(db.get_file_by_permalink("no/such/path").unwrap().is_none());
}

#[test]
fn test_delete_file() {
    let (_dir, db) = test_db();
    let file = sample_file("file-3", "to-delete.png");
    db.put_file(&file).unwrap();

    assert!(db.delete_file("file-3").unwrap());
    assert!(db.get_file("file-3").unwrap().is_none());
    assert!(db.get_file_by_permalink("to-delete.png").unwrap().is_none());
}

#[test]
fn test_delete_file_not_found() {
    let (_dir, db) = test_db();
    assert!(!db.delete_file("nonexistent").unwrap());
}

#[test]
fn test_update_file_metadata() {
    let (_dir, db) = test_db();
    let file = sample_file("file-4", "original.png");
    db.put_file(&file).unwrap();

    let updated = db
        .update_file(
            "file-4",
            Some(Some("new alt")),
            Some(None), // clear description
            None,       // keep metadata
            Some(Some("New Name")),
            None, // keep permalink
            None, // keep subject_id
        )
        .unwrap();
    assert!(updated);

    let file = db.get_file("file-4").unwrap().unwrap();
    assert_eq!(file.alt, Some("new alt".to_string()));
    assert_eq!(file.description, None);
    assert_eq!(file.name, Some("New Name".to_string()));
    assert_eq!(file.permalink, "original.png");
}

#[test]
fn test_update_file_permalink() {
    let (_dir, db) = test_db();
    let file = sample_file("file-5", "old-path.png");
    db.put_file(&file).unwrap();

    db.update_file("file-5", None, None, None, None, Some("new-path.png"), None)
        .unwrap();

    // Old permalink should not resolve
    assert!(db.get_file_by_permalink("old-path.png").unwrap().is_none());

    // New permalink should resolve
    let file = db
        .get_file_by_permalink("new-path.png")
        .unwrap()
        .expect("should resolve new permalink");
    assert_eq!(file.id, "file-5");
}

#[test]
fn test_update_file_not_found() {
    let (_dir, db) = test_db();
    assert!(!db
        .update_file(
            "nonexistent",
            Some(Some("alt")),
            None,
            None,
            None,
            None,
            None
        )
        .unwrap());
}

#[test]
fn test_list_files() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file("a", "a.png")).unwrap();
    db.put_file(&sample_file("b", "b.png")).unwrap();

    let files = db.list_files(None, None).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_list_files_with_filter() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file("img", "img.png")).unwrap();

    let now = Utc::now();
    let doc = FileRecord {
        id: "doc".to_string(),
        mime_type: "application/pdf".to_string(),
        file_type: FileType::Document,
        byte_size: 2048,
        permalink: "doc.pdf".to_string(),
        created_at: now,
        updated_at: now,
        alt: None,
        description: None,
        metadata: None,
        name: None,
        subject_id: None,
    };
    db.put_file(&doc).unwrap();

    let images = db.list_files(Some("image"), None).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].id, "img");

    let documents = db.list_files(Some("document"), None).unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].id, "doc");
}

#[test]
fn test_permalink_exists() {
    let (_dir, db) = test_db();
    let file = sample_file("file-6", "check-me.png");
    db.put_file(&file).unwrap();

    assert!(db.permalink_exists("check-me.png").unwrap());
    assert!(!db.permalink_exists("not-here.png").unwrap());
}

#[test]
fn test_purge_all() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file("p1", "p1.png")).unwrap();
    db.put_file(&sample_file("p2", "p2.png")).unwrap();

    let stats = db.purge_all().unwrap();
    assert_eq!(stats.files, 2);

    assert!(db.get_all_files().unwrap().is_empty());
    assert!(!db.permalink_exists("p1.png").unwrap());
    assert!(!db.permalink_exists("p2.png").unwrap());
}

#[test]
fn test_file_type_from_mime() {
    assert_eq!(FileType::from_mime("image/png"), FileType::Image);
    assert_eq!(FileType::from_mime("image/jpeg"), FileType::Image);
    assert_eq!(FileType::from_mime("video/mp4"), FileType::Video);
    assert_eq!(FileType::from_mime("audio/mpeg"), FileType::Audio);
    assert_eq!(FileType::from_mime("application/pdf"), FileType::Document);
    assert_eq!(FileType::from_mime("text/plain"), FileType::Document);
    assert_eq!(FileType::from_mime("text/csv"), FileType::Document);
    assert_eq!(
        FileType::from_mime("application/octet-stream"),
        FileType::Binary
    );
    assert_eq!(FileType::from_mime("unknown/type"), FileType::Binary);
}

// ============================================================================
// subject_id tests
// ============================================================================

#[test]
fn test_put_file_with_subject_id() {
    let (_dir, db) = test_db();
    let file = sample_file_with_subject("s1", "s1.png", "user-123");
    db.put_file(&file).unwrap();

    let retrieved = db.get_file("s1").unwrap().unwrap();
    assert_eq!(retrieved.subject_id, Some("user-123".to_string()));
}

#[test]
fn test_get_files_by_subject() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file_with_subject("s-a", "sa.png", "org-1"))
        .unwrap();
    db.put_file(&sample_file_with_subject("s-b", "sb.png", "org-1"))
        .unwrap();
    db.put_file(&sample_file_with_subject("s-c", "sc.png", "org-2"))
        .unwrap();
    db.put_file(&sample_file("no-sub", "nosub.png")).unwrap();

    let org1_files = db.get_files_by_subject("org-1").unwrap();
    assert_eq!(org1_files.len(), 2);

    let org2_files = db.get_files_by_subject("org-2").unwrap();
    assert_eq!(org2_files.len(), 1);
    assert_eq!(org2_files[0].id, "s-c");

    let empty = db.get_files_by_subject("nonexistent").unwrap();
    assert!(empty.is_empty());
}

#[test]
fn test_list_files_by_subject() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file_with_subject("ls-a", "lsa.png", "user-1"))
        .unwrap();
    db.put_file(&sample_file_with_subject("ls-b", "lsb.png", "user-2"))
        .unwrap();
    db.put_file(&sample_file("ls-c", "lsc.png")).unwrap();

    let user1_files = db.list_files(None, Some("user-1")).unwrap();
    assert_eq!(user1_files.len(), 1);
    assert_eq!(user1_files[0].id, "ls-a");

    let all_files = db.list_files(None, None).unwrap();
    assert_eq!(all_files.len(), 3);
}

#[test]
fn test_delete_file_cleans_subject_index() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file_with_subject("del-s", "dels.png", "user-x"))
        .unwrap();
    db.put_file(&sample_file_with_subject("keep-s", "keeps.png", "user-x"))
        .unwrap();

    db.delete_file("del-s").unwrap();

    let remaining = db.get_files_by_subject("user-x").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "keep-s");
}

#[test]
fn test_delete_last_file_removes_subject_entry() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file_with_subject("only", "only.png", "user-solo"))
        .unwrap();

    db.delete_file("only").unwrap();

    let empty = db.get_files_by_subject("user-solo").unwrap();
    assert!(empty.is_empty());
}

#[test]
fn test_update_file_subject_id() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file_with_subject("mv", "mv.png", "old-owner"))
        .unwrap();

    db.update_file("mv", None, None, None, None, None, Some(Some("new-owner")))
        .unwrap();

    let file = db.get_file("mv").unwrap().unwrap();
    assert_eq!(file.subject_id, Some("new-owner".to_string()));

    // Old owner should have no files
    let old = db.get_files_by_subject("old-owner").unwrap();
    assert!(old.is_empty());

    // New owner should have the file
    let new = db.get_files_by_subject("new-owner").unwrap();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].id, "mv");
}

#[test]
fn test_update_file_clear_subject_id() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file_with_subject("clr", "clr.png", "owner"))
        .unwrap();

    db.update_file("clr", None, None, None, None, None, Some(None))
        .unwrap();

    let file = db.get_file("clr").unwrap().unwrap();
    assert_eq!(file.subject_id, None);

    let owner_files = db.get_files_by_subject("owner").unwrap();
    assert!(owner_files.is_empty());
}

// ============================================================================
// metadata tests
// ============================================================================

#[test]
fn test_put_file_with_metadata() {
    let (_dir, db) = test_db();
    let mut file = sample_file("meta-1", "meta1.png");
    let mut meta = HashMap::new();
    meta.insert("width".to_string(), serde_json::Value::Number(1920.into()));
    meta.insert("height".to_string(), serde_json::Value::Number(1080.into()));
    file.metadata = Some(meta);
    db.put_file(&file).unwrap();

    let retrieved = db.get_file("meta-1").unwrap().unwrap();
    let metadata = retrieved.metadata.unwrap();
    assert_eq!(metadata.get("width").unwrap(), &serde_json::json!(1920));
    assert_eq!(metadata.get("height").unwrap(), &serde_json::json!(1080));
}

#[test]
fn test_update_file_metadata_map() {
    let (_dir, db) = test_db();
    db.put_file(&sample_file("meta-2", "meta2.png")).unwrap();

    let mut meta = HashMap::new();
    meta.insert(
        "camera".to_string(),
        serde_json::Value::String("Canon EOS R5".to_string()),
    );

    db.update_file("meta-2", None, None, Some(Some(&meta)), None, None, None)
        .unwrap();

    let file = db.get_file("meta-2").unwrap().unwrap();
    let metadata = file.metadata.unwrap();
    assert_eq!(
        metadata.get("camera").unwrap(),
        &serde_json::json!("Canon EOS R5")
    );
}

#[test]
fn test_update_file_clear_metadata() {
    let (_dir, db) = test_db();
    let mut file = sample_file("meta-3", "meta3.png");
    let mut meta = HashMap::new();
    meta.insert("key".to_string(), serde_json::Value::Bool(true));
    file.metadata = Some(meta);
    db.put_file(&file).unwrap();

    db.update_file("meta-3", None, None, Some(None), None, None, None)
        .unwrap();

    let file = db.get_file("meta-3").unwrap().unwrap();
    assert_eq!(file.metadata, None);
}

#[test]
fn test_metadata_round_trip_complex() {
    let (_dir, db) = test_db();
    let mut file = sample_file("meta-4", "meta4.png");
    let mut meta = HashMap::new();
    meta.insert(
        "tags".to_string(),
        serde_json::json!(["landscape", "nature"]),
    );
    meta.insert("rating".to_string(), serde_json::json!(4.5));
    meta.insert("published".to_string(), serde_json::json!(true));
    meta.insert("author".to_string(), serde_json::json!(null));
    file.metadata = Some(meta);
    db.put_file(&file).unwrap();

    let retrieved = db.get_file("meta-4").unwrap().unwrap();
    let metadata = retrieved.metadata.unwrap();
    assert_eq!(
        metadata.get("tags").unwrap(),
        &serde_json::json!(["landscape", "nature"])
    );
    assert_eq!(metadata.get("rating").unwrap(), &serde_json::json!(4.5));
    assert_eq!(metadata.get("published").unwrap(), &serde_json::json!(true));
    assert_eq!(metadata.get("author").unwrap(), &serde_json::json!(null));
}
