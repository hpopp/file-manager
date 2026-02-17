//! Shared test helpers for file-manager integration tests.

use std::sync::Arc;

use crate::config::{ClusterConfig, Config, NodeConfig, StorageConfig};
use crate::object_store::LocalStore;
use crate::state_machine::FileStateMachine;
use crate::storage::Database;
use crate::AppState;

/// Create a test AppState with a temporary database and local object store.
pub fn test_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
    let data_dir = temp_dir.path().join("data");
    let files_dir = temp_dir.path().join("files");

    let config = Config {
        node: NodeConfig {
            id: uuid::Uuid::new_v4().to_string(),
            bind_address: "127.0.0.1:0".to_string(),
            data_dir: data_dir.to_string_lossy().to_string(),
        },
        cluster: ClusterConfig::default(),
        storage: StorageConfig::default(),
        test_mode: true,
        max_upload_size: 10 * 1024 * 1024, // 10MB for tests
    };

    let db = Database::open(&data_dir).expect("Failed to open test database");
    let object_store = LocalStore::new(&files_dir).expect("Failed to create test object store");

    let muster_storage =
        muster::RedbStorage::new(db.inner()).expect("Failed to create muster storage");
    let state_machine = FileStateMachine::new(db.clone());
    let muster_config = muster::Config {
        node_id: config.node.id.clone(),
        cluster_port: 0,
        heartbeat_interval_ms: 300,
        election_timeout_ms: 3000,
        discovery: muster::DiscoveryConfig {
            dns_name: None,
            peers: vec![],
            poll_interval_secs: 5,
        },
    };
    let node = muster::MusterNode::new(muster_config, muster_storage, state_machine)
        .expect("Failed to create muster node");

    Arc::new(AppState {
        config,
        db,
        node: Arc::clone(&node),
        object_store: Arc::new(object_store),
    })
}
