use minijinja::Environment;
use redis::cluster::ClusterClient;
use sqlx::PgPool;
use std::sync::Arc;

pub type ViewEngine = Environment<'static>;
pub type RedisCluster = Arc<ClusterClient>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub services: Services,
    pub views: ViewEngine,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: String,
    pub app_env: String,
    pub app_debug: bool,
    pub redis: RedisConfig,
}

#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Cluster node URLs parsed from REDIS_CLUSTER_NODES (comma-separated).
    pub nodes: Vec<String>,
}

#[derive(Clone)]
pub struct Services {
    pub db: PgPool,
    /// Shared Redis cluster client.
    /// Call `.get_async_connection().await` to obtain a connection.
    pub redis: RedisCluster,
}
