use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use file_manager::{
    api,
    config::{Config, StorageBackend},
    object_store as obj,
    state_machine::FileStateMachine,
    storage::Database,
    AppState,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    match log_format.to_lowercase().as_str() {
        "gcp" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_stackdriver::layer())
                .init();
        }
        "json" => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_target(true)
                        .with_span_list(false),
                )
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
        }
    }

    info!(version = env!("CARGO_PKG_VERSION"), "file-manager starting");

    // Load configuration
    let config = Config::load()?;
    info!("Loaded configuration for node: {}", config.node.id);

    // Initialize database
    let db = Database::open(&config.node.data_dir)?;
    info!("Database opened at: {}", config.node.data_dir);

    // Initialize object store backend
    let object_store: Arc<dyn obj::ObjectStore> = match config.storage.backend {
        StorageBackend::Local => {
            let store = obj::LocalStore::new(&config.storage.local_storage_path)?;
            info!(
                "Using local storage backend at: {}",
                config.storage.local_storage_path
            );
            Arc::new(store)
        }
        StorageBackend::Gcs => {
            let bucket = config
                .storage
                .gcs_bucket
                .as_deref()
                .expect("GCS_BUCKET validated in config");
            let store =
                obj::GcsStore::new(bucket, config.storage.gcs_credentials_file.as_deref()).await?;
            info!("Using GCS storage backend, bucket: {}", bucket);
            Arc::new(store)
        }
    };

    // Build muster configuration
    let cluster_port = config.cluster.cluster_port;
    let cluster_peers: Vec<String> = config
        .cluster
        .peers
        .iter()
        .map(|peer| {
            if let Some((host, _)) = peer.rsplit_once(':') {
                format!("{host}:{cluster_port}")
            } else {
                format!("{peer}:{cluster_port}")
            }
        })
        .collect();

    let muster_config = muster::Config {
        node_id: config.node.id.clone(),
        cluster_port,
        heartbeat_interval_ms: config.cluster.heartbeat_interval_ms,
        election_timeout_ms: config.cluster.election_timeout_ms,
        discovery: muster::DiscoveryConfig {
            dns_name: config.cluster.discovery.dns_name.clone(),
            peers: cluster_peers,
            poll_interval_secs: config.cluster.discovery.poll_interval_seconds,
        },
    };

    // Create muster storage (shares the redb instance with file-manager)
    let muster_storage = muster::RedbStorage::new(db.inner())?;

    // Create the state machine
    let state_machine = FileStateMachine::new(db.clone());

    // Create the cluster node
    let node = muster::MusterNode::new(muster_config, muster_storage, state_machine)?;

    // Start cluster background tasks (heartbeat, election, discovery, TCP server)
    let cluster_handles = node.start();

    // Create shared state
    let state = Arc::new(AppState {
        config: config.clone(),
        db,
        node: Arc::clone(&node),
        object_store,
    });

    // Build and start the HTTP server
    let app = api::create_router(Arc::clone(&state));
    let listener = tokio::net::TcpListener::bind(&config.node.bind_address).await?;
    info!("Listening on: {}", config.node.bind_address);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Cleanup: abort background tasks
    info!("Shutting down background tasks");
    for handle in cluster_handles {
        handle.abort();
    }

    // Persist cluster state to disk
    if let Err(e) = node.persist_state().await {
        tracing::error!(error = %e, "Failed to persist cluster state during shutdown");
    }

    info!("Shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, draining connections");
}
