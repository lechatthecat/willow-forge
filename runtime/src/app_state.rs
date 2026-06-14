use minijinja::Environment;
use redis::cluster::ClusterClient;
use serde::{Deserialize, de::DeserializeOwned};
use sqlx::PgPool;
use sqlx::postgres::PgSslMode;
use std::{fs, path::Path, sync::Arc};

use crate::mailer::{MailConfig, Mailer};

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
    pub app_url: String,
    pub database: DatabaseConfig,
    pub cache: CacheConfig,
    pub redis: RedisConfig,
    pub auth: AuthConfig,
    pub session: SessionConfig,
    pub jwt: JwtConfig,
    pub mail: MailConfig,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub connection: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: u32,
    pub ssl_mode: String,
}

impl DatabaseConfig {
    pub fn pg_ssl_mode(&self) -> anyhow::Result<PgSslMode> {
        match self.ssl_mode.trim().to_ascii_lowercase().as_str() {
            "disable" | "disabled" => Ok(PgSslMode::Disable),
            "allow" => Ok(PgSslMode::Allow),
            "prefer" => Ok(PgSslMode::Prefer),
            "require" => Ok(PgSslMode::Require),
            "verify-ca" | "verify_ca" => Ok(PgSslMode::VerifyCa),
            "verify-full" | "verify_full" => Ok(PgSslMode::VerifyFull),
            other => anyhow::bail!(
                "Invalid database ssl_mode `{}` (expected disable, allow, prefer, require, verify-ca, or verify-full)",
                other
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub store: String,
}

#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Cluster node URLs parsed from config/cache.toml or REDIS_CLUSTER_NODES.
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub guard: String,
    pub redirect: String,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub enabled: bool,
    pub lifetime: u64,
    pub cookie: String,
    pub secure: bool,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry: u64,
}

#[derive(Clone)]
pub struct Services {
    pub db: PgPool,
    /// Shared Redis cluster client.
    /// Call `.get_async_connection().await` to obtain a connection.
    pub redis: RedisCluster,
    /// Mail sender. Send via `services.mailer.send(&email).await`.
    pub mailer: Mailer,
}

impl Config {
    /// Load `src/config/*.toml` and then let matching environment variables override them.
    ///
    /// This mirrors Laravel's usual flow: committed config files provide defaults, while
    /// `.env` carries machine-specific secrets and deployment overrides.
    pub fn load() -> anyhow::Result<Self> {
        Self::from_files_and_env("src/config")
    }

    pub fn from_project_root(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::from_files_and_env(root.as_ref().join("src/config"))
    }

    pub fn from_files_and_env(config_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let files = FileConfig::read(config_dir.as_ref())?;
        Self::from_file_config(files, |key| std::env::var(key).ok())
    }

    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "app.name" => Some(self.app_name.clone()),
            "app.env" => Some(self.app_env.clone()),
            "app.debug" => Some(self.app_debug.to_string()),
            "app.url" => Some(self.app_url.clone()),
            "database.connection" => Some(self.database.connection.clone()),
            "database.host" => Some(self.database.host.clone()),
            "database.port" => Some(self.database.port.to_string()),
            "database.database" => Some(self.database.database.clone()),
            "database.username" => Some(self.database.username.clone()),
            "database.password" => Some(self.database.password.clone()),
            "database.max_connections" => Some(self.database.max_connections.to_string()),
            "database.ssl_mode" => Some(self.database.ssl_mode.clone()),
            "cache.store" => Some(self.cache.store.clone()),
            "cache.nodes" | "redis.nodes" => Some(self.redis.nodes.join(",")),
            "auth.guard" => Some(self.auth.guard.clone()),
            "auth.redirect" => Some(self.auth.redirect.clone()),
            "session.enabled" => Some(self.session.enabled.to_string()),
            "session.lifetime" => Some(self.session.lifetime.to_string()),
            "session.cookie" => Some(self.session.cookie.clone()),
            "session.secure" => Some(self.session.secure.to_string()),
            "jwt.secret" => Some(self.jwt.secret.clone()),
            "jwt.expiry" => Some(self.jwt.expiry.to_string()),
            "mail.mailer" | "mail.driver" => Some(self.mail.driver.clone()),
            "mail.host" => Some(self.mail.host.clone()),
            "mail.port" => Some(self.mail.port.to_string()),
            "mail.username" => Some(self.mail.username.clone()),
            "mail.password" => Some(self.mail.password.clone()),
            "mail.encryption" => Some(self.mail.encryption.clone()),
            "mail.from_address" => Some(self.mail.from_address.clone()),
            "mail.from_name" => Some(self.mail.from_name.clone()),
            _ => None,
        }
    }

    fn from_file_config<F>(files: FileConfig, env: F) -> anyhow::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let defaults = Config::default();
        let database_defaults = DatabaseConfig::default();
        let cache_defaults = CacheConfig::default();
        let redis_defaults = RedisConfig::default();
        let auth_defaults = AuthConfig::default();
        let session_defaults = SessionConfig::default();
        let jwt_defaults = JwtConfig::default();
        let mail_defaults = MailConfig::default();

        let app = files.app.app;
        let database = files.database.database;
        let cache = files.cache.cache;
        let auth = files.auth.auth;
        let jwt = files.jwt.jwt;
        let mail = files.mail.mail;

        Ok(Config {
            app_name: env_string(&env, "APP_NAME", app.name, &defaults.app_name),
            app_env: env_string(&env, "APP_ENV", app.env, &defaults.app_env),
            app_debug: env_bool(&env, "APP_DEBUG", app.debug, defaults.app_debug)?,
            app_url: env_string(&env, "APP_URL", app.url, &defaults.app_url),
            database: DatabaseConfig {
                connection: env_string(
                    &env,
                    "DB_CONNECTION",
                    database.connection,
                    &database_defaults.connection,
                ),
                host: env_string(&env, "DB_HOST", database.host, &database_defaults.host),
                port: env_parse(&env, "DB_PORT", database.port, database_defaults.port)?,
                database: env_string(
                    &env,
                    "DB_DATABASE",
                    database.database,
                    &database_defaults.database,
                ),
                username: env_string(
                    &env,
                    "DB_USERNAME",
                    database.username,
                    &database_defaults.username,
                ),
                password: env_string(
                    &env,
                    "DB_PASSWORD",
                    database.password,
                    &database_defaults.password,
                ),
                max_connections: env_parse(
                    &env,
                    "DB_MAX_CONNECTIONS",
                    database.max_connections,
                    database_defaults.max_connections,
                )?,
                ssl_mode: env_string(
                    &env,
                    "DB_SSL_MODE",
                    database.ssl_mode,
                    &database_defaults.ssl_mode,
                ),
            },
            cache: CacheConfig {
                store: env_string(&env, "CACHE_STORE", cache.store, &cache_defaults.store),
            },
            redis: RedisConfig {
                nodes: env_list(
                    &env,
                    "REDIS_CLUSTER_NODES",
                    cache.nodes,
                    redis_defaults.nodes,
                ),
            },
            auth: AuthConfig {
                guard: env_string(&env, "AUTH_GUARD", auth.guard, &auth_defaults.guard),
                redirect: env_string(
                    &env,
                    "AUTH_REDIRECT",
                    auth.redirect,
                    &auth_defaults.redirect,
                ),
            },
            session: SessionConfig {
                enabled: env_bool(
                    &env,
                    "SESSION_ENABLED",
                    auth.session_enabled,
                    session_defaults.enabled,
                )?,
                lifetime: env_parse(
                    &env,
                    "SESSION_LIFETIME",
                    auth.session_lifetime,
                    session_defaults.lifetime,
                )?,
                cookie: env_string(
                    &env,
                    "SESSION_COOKIE",
                    auth.session_cookie,
                    &session_defaults.cookie,
                ),
                secure: env_bool(
                    &env,
                    "SESSION_SECURE",
                    auth.session_secure,
                    session_defaults.secure,
                )?,
            },
            jwt: JwtConfig {
                secret: env_string(&env, "JWT_SECRET", jwt.secret, &jwt_defaults.secret),
                expiry: env_parse(&env, "JWT_EXPIRY", jwt.expiry, jwt_defaults.expiry)?,
            },
            mail: MailConfig {
                driver: env_string(
                    &env,
                    "MAIL_MAILER",
                    mail.mailer.or(mail.driver),
                    &mail_defaults.driver,
                ),
                host: env_string(&env, "MAIL_HOST", mail.host, &mail_defaults.host),
                port: env_parse(&env, "MAIL_PORT", mail.port, mail_defaults.port)?,
                username: env_string(
                    &env,
                    "MAIL_USERNAME",
                    mail.username,
                    &mail_defaults.username,
                ),
                password: env_string(
                    &env,
                    "MAIL_PASSWORD",
                    mail.password,
                    &mail_defaults.password,
                ),
                encryption: env_string(
                    &env,
                    "MAIL_ENCRYPTION",
                    mail.encryption,
                    &mail_defaults.encryption,
                ),
                from_address: env_string(
                    &env,
                    "MAIL_FROM_ADDRESS",
                    mail.from_address,
                    &mail_defaults.from_address,
                ),
                from_name: env_string(
                    &env,
                    "MAIL_FROM_NAME",
                    mail.from_name,
                    &mail_defaults.from_name,
                ),
            },
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_name: "Willow Forge".to_string(),
            app_env: "local".to_string(),
            app_debug: true,
            app_url: "http://localhost:3000".to_string(),
            database: DatabaseConfig::default(),
            cache: CacheConfig::default(),
            redis: RedisConfig::default(),
            auth: AuthConfig::default(),
            session: SessionConfig::default(),
            jwt: JwtConfig::default(),
            mail: MailConfig::default(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            connection: "postgres".to_string(),
            host: "127.0.0.1".to_string(),
            port: 5432,
            database: "willowforge".to_string(),
            username: "postgres".to_string(),
            password: "".to_string(),
            max_connections: 10,
            ssl_mode: "disable".to_string(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            store: "redis-cluster".to_string(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            nodes: vec![
                "redis://127.0.0.1:7001".to_string(),
                "redis://127.0.0.1:7002".to_string(),
                "redis://127.0.0.1:7003".to_string(),
            ],
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            guard: "web".to_string(),
            redirect: "/login".to_string(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lifetime: 7200,
            cookie: "willow_session".to_string(),
            secure: false,
        }
    }
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: String::new(),
            expiry: 3600,
        }
    }
}

#[derive(Default)]
struct FileConfig {
    app: AppToml,
    database: DatabaseToml,
    cache: CacheToml,
    auth: AuthToml,
    jwt: JwtToml,
    mail: MailToml,
}

impl FileConfig {
    fn read(config_dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            app: read_toml(config_dir, "app.toml")?,
            database: read_toml(config_dir, "database.toml")?,
            cache: read_toml(config_dir, "cache.toml")?,
            auth: read_toml(config_dir, "auth.toml")?,
            jwt: read_toml(config_dir, "jwt.toml")?,
            mail: read_toml(config_dir, "mail.toml")?,
        })
    }
}

#[derive(Default, Deserialize)]
struct AppToml {
    #[serde(default)]
    app: AppSection,
}

#[derive(Default, Deserialize)]
struct AppSection {
    name: Option<String>,
    env: Option<String>,
    debug: Option<bool>,
    url: Option<String>,
}

#[derive(Default, Deserialize)]
struct DatabaseToml {
    #[serde(default)]
    database: DatabaseSection,
}

#[derive(Default, Deserialize)]
struct DatabaseSection {
    connection: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    database: Option<String>,
    username: Option<String>,
    password: Option<String>,
    max_connections: Option<u32>,
    ssl_mode: Option<String>,
}

#[derive(Default, Deserialize)]
struct CacheToml {
    #[serde(default)]
    cache: CacheSection,
}

#[derive(Default, Deserialize)]
struct CacheSection {
    store: Option<String>,
    nodes: Option<Vec<String>>,
}

#[derive(Default, Deserialize)]
struct AuthToml {
    #[serde(default)]
    auth: AuthSection,
}

#[derive(Default, Deserialize)]
struct AuthSection {
    guard: Option<String>,
    redirect: Option<String>,
    session_enabled: Option<bool>,
    session_lifetime: Option<u64>,
    session_cookie: Option<String>,
    session_secure: Option<bool>,
}

#[derive(Default, Deserialize)]
struct JwtToml {
    #[serde(default)]
    jwt: JwtSection,
}

#[derive(Default, Deserialize)]
struct JwtSection {
    secret: Option<String>,
    expiry: Option<u64>,
}

#[derive(Default, Deserialize)]
struct MailToml {
    #[serde(default)]
    mail: MailSection,
}

#[derive(Default, Deserialize)]
struct MailSection {
    mailer: Option<String>,
    driver: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    encryption: Option<String>,
    from_address: Option<String>,
    from_name: Option<String>,
}

fn read_toml<T>(config_dir: &Path, file_name: &str) -> anyhow::Result<T>
where
    T: Default + DeserializeOwned,
{
    let path = config_dir.join(file_name);
    if !path.exists() {
        return Ok(T::default());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Could not read config file {}: {}", path.display(), e))?;

    toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("Could not parse config file {}: {}", path.display(), e))
}

fn env_string<F>(env: &F, key: &str, file_value: Option<String>, default: &str) -> String
where
    F: Fn(&str) -> Option<String>,
{
    env(key)
        .or(file_value)
        .unwrap_or_else(|| default.to_string())
}

fn env_parse<T, F>(env: &F, key: &str, file_value: Option<T>, default: T) -> anyhow::Result<T>
where
    T: Copy + std::str::FromStr,
    T::Err: std::fmt::Display,
    F: Fn(&str) -> Option<String>,
{
    match env(key) {
        Some(raw) if !raw.trim().is_empty() => raw
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid {} value `{}`: {}", key, raw, e)),
        _ => Ok(file_value.unwrap_or(default)),
    }
}

fn env_bool<F>(env: &F, key: &str, file_value: Option<bool>, default: bool) -> anyhow::Result<bool>
where
    F: Fn(&str) -> Option<String>,
{
    match env(key) {
        Some(raw) if !raw.trim().is_empty() => parse_bool(key, &raw),
        _ => Ok(file_value.unwrap_or(default)),
    }
}

fn parse_bool(key: &str, raw: &str) -> anyhow::Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("Invalid {} value `{}` (expected true/false)", key, raw),
    }
}

fn env_list<F>(
    env: &F,
    key: &str,
    file_value: Option<Vec<String>>,
    default: Vec<String>,
) -> Vec<String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(raw) = env(key).filter(|value| !value.trim().is_empty()) {
        return raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    file_value
        .filter(|items| !items.is_empty())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn file_config_values_are_loaded() {
        let files = FileConfig {
            app: AppToml {
                app: AppSection {
                    name: Some("Forge App".to_string()),
                    env: Some("testing".to_string()),
                    debug: Some(false),
                    url: Some("https://example.test".to_string()),
                },
            },
            database: DatabaseToml {
                database: DatabaseSection {
                    database: Some("forge_test".to_string()),
                    password: Some("secret".to_string()),
                    max_connections: Some(5),
                    ..Default::default()
                },
            },
            cache: CacheToml {
                cache: CacheSection {
                    nodes: Some(vec!["redis://cache:7001".to_string()]),
                    ..Default::default()
                },
            },
            jwt: JwtToml {
                jwt: JwtSection {
                    secret: Some("jwt-secret".to_string()),
                    expiry: Some(900),
                },
            },
            auth: AuthToml {
                auth: AuthSection {
                    session_enabled: Some(true),
                    ..Default::default()
                },
            },
            ..Default::default()
        };

        let config = Config::from_file_config(files, |_| None).unwrap();

        assert_eq!(config.app_name, "Forge App");
        assert_eq!(config.app_env, "testing");
        assert!(!config.app_debug);
        assert_eq!(config.app_url, "https://example.test");
        assert_eq!(config.database.database, "forge_test");
        assert_eq!(config.database.password, "secret");
        assert_eq!(config.database.max_connections, 5);
        assert_eq!(config.redis.nodes, vec!["redis://cache:7001".to_string()]);
        assert!(config.session.enabled);
        assert_eq!(config.jwt.secret, "jwt-secret");
        assert_eq!(config.jwt.expiry, 900);
    }

    #[test]
    fn environment_values_override_files() {
        let files = FileConfig {
            app: AppToml {
                app: AppSection {
                    name: Some("From File".to_string()),
                    debug: Some(false),
                    ..Default::default()
                },
            },
            database: DatabaseToml {
                database: DatabaseSection {
                    port: Some(5432),
                    ..Default::default()
                },
            },
            ..Default::default()
        };

        let config = Config::from_file_config(files, |key| match key {
            "APP_NAME" => Some("From Env".to_string()),
            "APP_DEBUG" => Some("true".to_string()),
            "DB_PORT" => Some("15432".to_string()),
            "SESSION_ENABLED" => Some("true".to_string()),
            _ => None,
        })
        .unwrap();

        assert_eq!(config.app_name, "From Env");
        assert!(config.app_debug);
        assert_eq!(config.database.port, 15432);
        assert!(config.session.enabled);
    }

    #[test]
    fn toml_files_are_optional_but_parsed_when_present() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("willow-forge-config-{}", unique));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("app.toml"),
            r#"[app]
name = "Parsed App"
debug = false
"#,
        )
        .unwrap();

        let files = FileConfig::read(&dir).unwrap();
        let config = Config::from_file_config(files, |_| None).unwrap();

        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(config.app_name, "Parsed App");
        assert!(!config.app_debug);
        assert_eq!(config.database.host, "127.0.0.1");
        assert!(!config.session.enabled);
    }
}
