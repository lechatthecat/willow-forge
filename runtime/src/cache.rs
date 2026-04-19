//! Laravel-like Cache facade backed by Redis.
//!
//! All methods take `&Context` as their first argument to access the cache Redis pool.
//! Values are serialized as JSON, so any `Serialize + DeserializeOwned` type is supported.
//!
//! # Example
//! ```rust,ignore
//! use std::time::Duration;
//!
//! // Get or compute and store for 5 minutes
//! let users = Cache::remember(&ctx, "users.all", Duration::from_secs(300), || async {
//!     sqlx::query_as::<_, User>("SELECT * FROM users")
//!         .fetch_all(&ctx.state.services.db)
//!         .await
//!         .map_err(AppError::from)
//! }).await?;
//!
//! // Direct put / get
//! Cache::put(&ctx, "greeting", &"hello", Duration::from_secs(60)).await?;
//! let val: Option<String> = Cache::get(&ctx, "greeting").await?;
//!
//! // Remove a key
//! Cache::forget(&ctx, "greeting").await?;
//! ```

use std::time::Duration;

use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};

use crate::app_errors::AppError;
use crate::context::Context;

pub struct Cache;

impl Cache {
    /// Retrieve a cached value. Returns `None` on a cache miss.
    pub async fn get<T: DeserializeOwned>(
        ctx: &Context,
        key: &str,
    ) -> Result<Option<T>, AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let raw: Option<String> = conn.get(key).await?;
        match raw {
            None => Ok(None),
            Some(json) => {
                let value = serde_json::from_str(&json)
                    .map_err(|e| redis::RedisError::from((
                        redis::ErrorKind::TypeError,
                        "JSON deserialization failed",
                        e.to_string(),
                    )))?;
                Ok(Some(value))
            }
        }
    }

    /// Store a value with a TTL.
    pub async fn put<T: Serialize>(
        ctx: &Context,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(value)
            .map_err(|e| redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "JSON serialization failed",
                e.to_string(),
            )))?;
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let secs = ttl.as_secs().max(1);
        let _: () = conn.set_ex(key, json, secs).await?;
        Ok(())
    }

    /// Store a value with no expiry.
    pub async fn put_forever<T: Serialize>(
        ctx: &Context,
        key: &str,
        value: &T,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(value)
            .map_err(|e| redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "JSON serialization failed",
                e.to_string(),
            )))?;
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let _: () = conn.set(key, json).await?;
        Ok(())
    }

    /// Get a cached value, or compute it with `f`, store it, and return it.
    ///
    /// The closure is only called on a cache miss.
    pub async fn remember<T, F, Fut>(
        ctx: &Context,
        key: &str,
        ttl: Duration,
        f: F,
    ) -> Result<T, AppError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, AppError>>,
    {
        if let Some(cached) = Self::get::<T>(ctx, key).await? {
            return Ok(cached);
        }
        let value = f().await?;
        Self::put(ctx, key, &value, ttl).await?;
        Ok(value)
    }

    /// Like `remember` but stores the value with no expiry.
    pub async fn remember_forever<T, F, Fut>(
        ctx: &Context,
        key: &str,
        f: F,
    ) -> Result<T, AppError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, AppError>>,
    {
        if let Some(cached) = Self::get::<T>(ctx, key).await? {
            return Ok(cached);
        }
        let value = f().await?;
        Self::put_forever(ctx, key, &value).await?;
        Ok(value)
    }

    /// Delete a cached key. Returns `Ok(())` whether or not the key existed.
    pub async fn forget(ctx: &Context, key: &str) -> Result<(), AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    /// Flush all keys in the cache database (FLUSHDB).
    pub async fn flush(ctx: &Context) -> Result<(), AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let _: () = redis::cmd("FLUSHDB").query_async(&mut conn).await?;
        Ok(())
    }

    /// Check whether a key exists in the cache.
    pub async fn has(ctx: &Context, key: &str) -> Result<bool, AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }

    /// Increment an integer counter by 1. Creates the key at 0 if absent.
    pub async fn increment(ctx: &Context, key: &str) -> Result<i64, AppError> {
        Self::increment_by(ctx, key, 1).await
    }

    /// Increment an integer counter by `delta`. Creates the key at 0 if absent.
    pub async fn increment_by(ctx: &Context, key: &str, delta: i64) -> Result<i64, AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let result: i64 = conn.incr(key, delta).await?;
        Ok(result)
    }

    /// Decrement an integer counter by 1.
    pub async fn decrement(ctx: &Context, key: &str) -> Result<i64, AppError> {
        Self::decrement_by(ctx, key, 1).await
    }

    /// Decrement an integer counter by `delta`.
    pub async fn decrement_by(ctx: &Context, key: &str, delta: i64) -> Result<i64, AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let result: i64 = conn.decr(key, delta).await?;
        Ok(result)
    }
}

#[cfg(test)]
fn to_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

#[cfg(test)]
fn from_json<T: DeserializeOwned>(s: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct User {
        id: i32,
        name: String,
    }

    // ── JSON serialization (no Redis needed) ─────────────────────────────────

    #[test]
    fn serialize_string_roundtrip() {
        let original = "hello, cache".to_string();
        let json = to_json(&original).unwrap();
        let decoded: String = from_json(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn serialize_struct_roundtrip() {
        let user = User { id: 1, name: "Alice".to_string() };
        let json = to_json(&user).unwrap();
        let decoded: User = from_json(&json).unwrap();
        assert_eq!(user, decoded);
    }

    #[test]
    fn serialize_vec_roundtrip() {
        let items = vec![1i64, 2, 3, 4, 5];
        let json = to_json(&items).unwrap();
        let decoded: Vec<i64> = from_json(&json).unwrap();
        assert_eq!(items, decoded);
    }

    #[test]
    fn deserialize_invalid_json_returns_error() {
        let result = from_json::<String>("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn ttl_min_one_second() {
        // Duration::from_millis(500).as_secs() == 0, .max(1) clamps to 1
        let secs = Duration::from_millis(500).as_secs().max(1);
        assert_eq!(secs, 1);
    }

    #[test]
    fn ttl_sixty_seconds() {
        let secs = Duration::from_secs(60).as_secs().max(1);
        assert_eq!(secs, 60);
    }

    // ── Redis integration tests (require running cluster) ────────────────────

    fn cluster_nodes() -> Vec<String> {
        std::env::var("REDIS_CLUSTER_NODES")
            .unwrap_or_else(|_| {
                "redis://127.0.0.1:7001,redis://127.0.0.1:7002,redis://127.0.0.1:7003".to_string()
            })
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    }

    async fn make_ctx() -> crate::context::Context {
        use crate::app_state::{AppState, Config, RedisConfig, Services};
        use redis::cluster::ClusterClient;
        use std::sync::Arc;

        let nodes = cluster_nodes();
        let redis = Arc::new(
            ClusterClient::new(nodes.iter().map(|s| s.as_str()).collect::<Vec<_>>()).unwrap(),
        );

        // Minimal PgPool: connect_lazy so it doesn't actually connect at test time.
        let db = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@127.0.0.1:5432/test")
            .unwrap();

        let config = Config {
            app_name: "test".to_string(),
            app_env: "test".to_string(),
            app_debug: true,
            redis: RedisConfig { nodes: cluster_nodes() },
        };

        let services = Services { db, redis };
        let views = minijinja::Environment::new();

        let state = Arc::new(AppState { config, services, views });
        crate::context::Context { state }
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn put_and_get_roundtrip() {
        let ctx = make_ctx().await;
        let key = "test:put_get";
        Cache::put(&ctx, key, &"world".to_string(), Duration::from_secs(10)).await.unwrap();
        let val: Option<String> = Cache::get(&ctx, key).await.unwrap();
        assert_eq!(val, Some("world".to_string()));
        Cache::forget(&ctx, key).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn get_returns_none_on_miss() {
        let ctx = make_ctx().await;
        let val: Option<String> = Cache::get(&ctx, "test:nonexistent_key_xyz").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn remember_calls_closure_on_miss() {
        let ctx = make_ctx().await;
        let key = "test:remember_miss";
        Cache::forget(&ctx, key).await.unwrap();

        let mut called = false;
        let val = Cache::remember(&ctx, key, Duration::from_secs(10), || async {
            called = true;
            Ok::<_, AppError>("computed".to_string())
        })
        .await
        .unwrap();

        assert!(called);
        assert_eq!(val, "computed");
        Cache::forget(&ctx, key).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn remember_skips_closure_on_hit() {
        let ctx = make_ctx().await;
        let key = "test:remember_hit";
        Cache::put(&ctx, key, &"cached".to_string(), Duration::from_secs(10)).await.unwrap();

        let mut called = false;
        let val = Cache::remember(&ctx, key, Duration::from_secs(10), || async {
            called = true;
            Ok::<_, AppError>("fresh".to_string())
        })
        .await
        .unwrap();

        assert!(!called);
        assert_eq!(val, "cached");
        Cache::forget(&ctx, key).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn forget_removes_key() {
        let ctx = make_ctx().await;
        let key = "test:forget";
        Cache::put(&ctx, key, &42i64, Duration::from_secs(10)).await.unwrap();
        Cache::forget(&ctx, key).await.unwrap();
        let val: Option<i64> = Cache::get(&ctx, key).await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn has_returns_true_when_key_exists() {
        let ctx = make_ctx().await;
        let key = "test:has";
        Cache::put(&ctx, key, &true, Duration::from_secs(10)).await.unwrap();
        assert!(Cache::has(&ctx, key).await.unwrap());
        Cache::forget(&ctx, key).await.unwrap();
        assert!(!Cache::has(&ctx, key).await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires Redis cluster on localhost:7001-7003"]
    async fn increment_and_decrement() {
        let ctx = make_ctx().await;
        let key = "test:counter";
        Cache::forget(&ctx, key).await.unwrap();

        let v1 = Cache::increment(&ctx, key).await.unwrap();
        let v2 = Cache::increment_by(&ctx, key, 4).await.unwrap();
        let v3 = Cache::decrement(&ctx, key).await.unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 5);
        assert_eq!(v3, 4);
        Cache::forget(&ctx, key).await.unwrap();
    }
}
