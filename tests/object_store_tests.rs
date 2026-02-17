use bytes::Bytes;
use file_manager::object_store::{LocalStore, ObjectStore};

#[tokio::test]
async fn test_local_store_put_get() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::new(dir.path()).unwrap();

    let data = Bytes::from("hello world");
    store.put("test-key", data.clone()).await.unwrap();

    let retrieved = store.get("test-key").await.unwrap();
    assert_eq!(retrieved, data);
}

#[tokio::test]
async fn test_local_store_exists() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::new(dir.path()).unwrap();

    assert!(!store.exists("missing").await.unwrap());

    store.put("present", Bytes::from("data")).await.unwrap();
    assert!(store.exists("present").await.unwrap());
}

#[tokio::test]
async fn test_local_store_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::new(dir.path()).unwrap();

    store.put("to-delete", Bytes::from("data")).await.unwrap();
    assert!(store.exists("to-delete").await.unwrap());

    store.delete("to-delete").await.unwrap();
    assert!(!store.exists("to-delete").await.unwrap());
}

#[tokio::test]
async fn test_local_store_delete_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::new(dir.path()).unwrap();

    // Deleting a nonexistent key should not error
    store.delete("nonexistent").await.unwrap();
}

#[tokio::test]
async fn test_local_store_get_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::new(dir.path()).unwrap();

    let result = store.get("missing").await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        file_manager::object_store::ObjectStoreError::NotFound(_)
    ));
}

#[tokio::test]
async fn test_local_store_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::new(dir.path()).unwrap();

    store.put("key", Bytes::from("first")).await.unwrap();
    store.put("key", Bytes::from("second")).await.unwrap();

    let data = store.get("key").await.unwrap();
    assert_eq!(data, Bytes::from("second"));
}
