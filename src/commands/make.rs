use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn controller(name: &str) -> Result<()> {
    let path = Path::new("app/Http/Controllers").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Controller already exists: {}", path.display());
    }

    let content = format!(
        r#"use axum::{{Json, response::IntoResponse}};
use serde_json::json;

use crate::bootstrap::context::Context;

pub async fn index(ctx: Context) -> impl IntoResponse {{
    Json(json!({{ "message": "{} index" }}))
}}

pub async fn show(ctx: Context) -> impl IntoResponse {{
    Json(json!({{ "message": "{} show" }}))
}}

pub async fn store(ctx: Context) -> impl IntoResponse {{
    Json(json!({{ "message": "{} store" }}))
}}

pub async fn update(ctx: Context) -> impl IntoResponse {{
    Json(json!({{ "message": "{} update" }}))
}}

pub async fn destroy(ctx: Context) -> impl IntoResponse {{
    Json(json!({{ "message": "{} destroy" }}))
}}
"#,
        name, name, name, name, name
    );

    fs::write(&path, content)
        .with_context(|| format!("Failed to create controller: {}", path.display()))?;

    println!("✓ Controller created: {}", path.display());
    Ok(())
}

pub fn request(name: &str) -> Result<()> {
    let path = Path::new("app/Http/Requests").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Request already exists: {}", path.display());
    }

    let content = format!(
        r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct {} {{
    // Add your fields here
    // Example:
    // #[validate(length(min = 1, max = 255))]
    // pub name: String,
}}
"#,
        name
    );

    fs::write(&path, content)
        .with_context(|| format!("Failed to create request: {}", path.display()))?;

    println!("✓ Request created: {}", path.display());
    Ok(())
}

pub fn model(name: &str) -> Result<()> {
    let path = Path::new("app/Models").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Model already exists: {}", path.display());
    }

    let content = format!(
        r#"use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {} {{
    pub id: i64,
    // Add your fields here
}}

impl {} {{
    // Add your model methods here
}}
"#,
        name, name
    );

    fs::write(&path, content)
        .with_context(|| format!("Failed to create model: {}", path.display()))?;

    println!("✓ Model created: {}", path.display());
    Ok(())
}

pub fn migration(name: &str) -> Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("{}_{}.sql", timestamp, name);
    let path = Path::new("database/migrations").join(&filename);

    let content = format!(
        r#"-- Migration: {}
-- Created at: {}

-- Add your SQL here
"#,
        name,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
    );

    fs::write(&path, content)
        .with_context(|| format!("Failed to create migration: {}", path.display()))?;

    println!("✓ Migration created: {}", path.display());
    Ok(())
}
