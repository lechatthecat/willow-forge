use anyhow::{Context, Result};
use sqlx::migrate::{Migrate, Migrator};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::Path;

// ── shared helpers ────────────────────────────────────────────────────────────

async fn build_pool() -> Result<PgPool> {
    let host     = std::env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port     = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let database = std::env::var("DB_DATABASE")
        .context("DB_DATABASE is not set — check your .env file")?;
    let username = std::env::var("DB_USERNAME").unwrap_or_else(|_| "postgres".to_string());
    let password = std::env::var("DB_PASSWORD").unwrap_or_default();

    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        username, password, host, port, database
    );

    println!("🌿 Connecting to {}:{}/{} ...", host, port, database);

    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .with_context(|| format!("Cannot connect to database at {}:{}/{}", host, port, database))
}

async fn build_migrator() -> Result<Migrator> {
    Migrator::new(Path::new("database/migrations"))
        .await
        .context("Failed to load migrations from database/migrations/")
}

// ── willow migrate ────────────────────────────────────────────────────────────

pub async fn execute() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool     = build_pool().await?;
    let migrator = build_migrator().await?;

    println!("🌿 Running migrations ...");
    migrator.run(&pool).await.context("Migration failed")?;
    println!("✓ Migrations complete.");
    Ok(())
}

// ── willow migrate:rollback ───────────────────────────────────────────────────

pub async fn rollback() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool     = build_pool().await?;
    let migrator = build_migrator().await?;

    let mut conn = pool.acquire().await?;
    conn.ensure_migrations_table().await
        .context("Failed to ensure migrations table")?;
    let applied = conn.list_applied_migrations().await
        .context("Failed to list applied migrations")?;
    drop(conn);

    if applied.is_empty() {
        println!("Nothing to roll back — no migrations have been applied.");
        return Ok(());
    }

    let last = applied.last().unwrap();

    let has_down = migrator
        .iter()
        .any(|m| m.version == last.version && m.migration_type.is_down_migration());

    if !has_down {
        anyhow::bail!(
            "Migration {} has no .down.sql — cannot roll back.",
            last.version
        );
    }

    let target = applied.iter().rev().nth(1).map(|m| m.version).unwrap_or(0);

    println!("Rolling back migration {} ...", last.version);
    migrator.undo(&pool, target).await.context("Rollback failed")?;
    println!("✓ Rolled back.");
    Ok(())
}

// ── willow migrate:status ─────────────────────────────────────────────────────

pub async fn status() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool     = build_pool().await?;
    let migrator = build_migrator().await?;

    let mut conn = pool.acquire().await?;
    conn.ensure_migrations_table().await
        .context("Failed to ensure migrations table")?;
    let applied = conn.list_applied_migrations().await
        .context("Failed to list applied migrations")?;
    drop(conn);

    let applied_versions: std::collections::HashSet<i64> =
        applied.iter().map(|m| m.version).collect();

    println!("\n{:<20} {:<45} {}", "Version", "Description", "Status");
    println!("{}", "─".repeat(75));

    for m in migrator.iter().filter(|m| m.migration_type.is_up_migration()) {
        let state = if applied_versions.contains(&m.version) {
            "Applied ✓"
        } else {
            "Pending"
        };
        println!("{:<20} {:<45} {}", m.version, m.description, state);
    }
    println!();
    Ok(())
}

// ── willow migrate:fresh ──────────────────────────────────────────────────────

pub async fn fresh() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = build_pool().await?;

    println!("🌿 Dropping all tables ...");
    sqlx::query("DROP SCHEMA public CASCADE").execute(&pool).await
        .context("Failed to drop schema")?;
    sqlx::query("CREATE SCHEMA public").execute(&pool).await
        .context("Failed to recreate schema")?;
    sqlx::query("GRANT ALL ON SCHEMA public TO public").execute(&pool).await
        .context("Failed to grant schema privileges")?;

    println!("🌿 Running all migrations ...");
    let migrator = build_migrator().await?;
    migrator.run(&pool).await.context("Migration failed")?;

    println!("✓ Database refreshed.");
    Ok(())
}

// ── willow migrate:reset ──────────────────────────────────────────────────────

pub async fn reset() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool     = build_pool().await?;
    let migrator = build_migrator().await?;

    let mut conn = pool.acquire().await?;
    conn.ensure_migrations_table().await
        .context("Failed to ensure migrations table")?;
    let applied = conn.list_applied_migrations().await
        .context("Failed to list applied migrations")?;
    drop(conn);

    if applied.is_empty() {
        println!("Nothing to reset — no migrations are applied.");
        return Ok(());
    }

    let down_versions: std::collections::HashSet<i64> = migrator
        .iter()
        .filter(|m| m.migration_type.is_down_migration())
        .map(|m| m.version)
        .collect();

    let missing: Vec<i64> = applied
        .iter()
        .map(|m| m.version)
        .filter(|v| !down_versions.contains(v))
        .collect();

    if !missing.is_empty() {
        anyhow::bail!(
            "Cannot reset: missing .down.sql for migration(s): {:?}",
            missing
        );
    }

    println!("Rolling back {} migration(s) ...", applied.len());
    migrator.undo(&pool, 0).await.context("Reset failed")?;
    println!("✓ All migrations rolled back.");
    Ok(())
}
