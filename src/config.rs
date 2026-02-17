use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub cluster: ClusterConfig,
    pub node: NodeConfig,
    pub storage: StorageConfig,
    /// Enables dangerous operations like purge. Must never be true in production.
    pub test_mode: bool,
    /// Maximum upload size in bytes
    pub max_upload_size: u64,
}

#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub bind_address: String,
    pub data_dir: String,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// TCP port for inter-node cluster communication
    pub cluster_port: u16,
    pub discovery: DiscoveryConfig,
    pub election_timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub peers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// DNS name to resolve for peer discovery (e.g., a Kubernetes headless service).
    pub dns_name: Option<String>,
    /// How often to poll for peer changes (seconds)
    pub poll_interval_seconds: u64,
}

#[derive(Debug, Clone)]
pub enum StorageBackend {
    Gcs,
    Local,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub backend: StorageBackend,
    /// Directory for local storage backend
    pub local_storage_path: String,
    /// GCS bucket name (required when backend is gcs)
    pub gcs_bucket: Option<String>,
    /// Path to GCS service account JSON (optional, defaults to ADC)
    pub gcs_credentials_file: Option<String>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            dns_name: None,
            poll_interval_seconds: 5,
        }
    }
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            cluster_port: 9993,
            discovery: DiscoveryConfig::default(),
            election_timeout_ms: 3000,
            heartbeat_interval_ms: 300,
            peers: Vec::new(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Local,
            local_storage_path: "./files".to_string(),
            gcs_bucket: None,
            gcs_credentials_file: None,
        }
    }
}

impl Config {
    /// Load configuration from environment variables.
    pub fn load() -> Result<Self, ConfigError> {
        let node_id = std::env::var("NODE_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        let bind_address =
            std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());

        let peers: Vec<String> = std::env::var("PEERS")
            .map(|p| {
                p.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .filter(|s| !s.starts_with(&format!("{node_id}:")) && s != &node_id)
                    .collect()
            })
            .unwrap_or_default();

        let dns_name = std::env::var("DISCOVERY_DNS_NAME").ok();
        let poll_interval = std::env::var("DISCOVERY_POLL_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let cluster_port = std::env::var("CLUSTER_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(9993);

        let test_mode = std::env::var("TEST_MODE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let max_upload_size = std::env::var("MAX_UPLOAD_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50 * 1024 * 1024); // 50MB

        let storage_backend = match std::env::var("STORAGE_BACKEND")
            .unwrap_or_else(|_| "local".to_string())
            .to_lowercase()
            .as_str()
        {
            "gcs" => StorageBackend::Gcs,
            _ => StorageBackend::Local,
        };

        let local_storage_path =
            std::env::var("LOCAL_STORAGE_PATH").unwrap_or_else(|_| "./files".to_string());

        let gcs_bucket = std::env::var("GCS_BUCKET").ok();
        let gcs_credentials_file = std::env::var("GCS_CREDENTIALS_FILE").ok();

        let config = Config {
            node: NodeConfig {
                id: node_id,
                bind_address,
                data_dir,
            },
            cluster: ClusterConfig {
                cluster_port,
                peers,
                discovery: DiscoveryConfig {
                    dns_name,
                    poll_interval_seconds: poll_interval,
                },
                ..Default::default()
            },
            storage: StorageConfig {
                backend: storage_backend,
                local_storage_path,
                gcs_bucket,
                gcs_credentials_file,
            },
            test_mode,
            max_upload_size,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.node.id.is_empty() {
            return Err(ConfigError::ValidationError(
                "NODE_ID cannot be empty".to_string(),
            ));
        }

        if matches!(self.storage.backend, StorageBackend::Gcs) && self.storage.gcs_bucket.is_none()
        {
            return Err(ConfigError::ValidationError(
                "GCS_BUCKET is required when STORAGE_BACKEND=gcs".to_string(),
            ));
        }

        let cluster_size = self.cluster.peers.len() + 1;
        if cluster_size > 1 && cluster_size.is_multiple_of(2) {
            tracing::warn!(
                "Cluster size {} is even. This may lead to split-brain scenarios. \
                 Consider using an odd number of nodes.",
                cluster_size
            );
        }

        Ok(())
    }

    /// Check if running in single-node mode.
    pub fn is_single_node(&self) -> bool {
        self.cluster.peers.is_empty() && self.cluster.discovery.dns_name.is_none()
    }
}
