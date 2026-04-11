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

fn view_name_to_path(name: &str) -> std::path::PathBuf {
    let parts: Vec<&str> = name.split('.').collect();
    let (dirs, stem) = parts.split_at(parts.len() - 1);
    let mut path = Path::new("resources/views").to_path_buf();
    for dir in dirs {
        path = path.join(dir);
    }
    path.join(format!("{}.jinja.html", stem[0]))
}

pub fn view_file(name: &str) -> Result<()> {
    let file_path = view_name_to_path(name);
    let path = file_path.parent().unwrap();

    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;

    if file_path.exists() {
        anyhow::bail!("View already exists: {}", file_path.display());
    }

    let content = format!(
        "{{% extends \"layouts.app\" %}}\n\n{{% block title %}}{name}{{% endblock %}}\n\n{{% block content %}}\n<h1>{name}</h1>\n{{% endblock %}}\n",
        name = name
    );

    fs::write(&file_path, content)
        .with_context(|| format!("Failed to create view: {}", file_path.display()))?;

    println!("✓ View created: {}", file_path.display());
    Ok(())
}

pub fn middleware(name: &str) -> Result<()> {
    let path = Path::new("app/Http/Middleware").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Middleware already exists: {}", path.display());
    }

    let content = crate::templates::app_files::make_middleware_template(name);

    fs::write(&path, &content)
        .with_context(|| format!("Failed to create middleware: {}", path.display()))?;

    println!("✓ Middleware created: {}", path.display());
    Ok(())
}

pub fn migration(name: &str) -> Result<()> {
    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d%H%M%S");
    let created = now.format("%Y-%m-%d %H:%M:%S");
    let base = Path::new("database/migrations");

    let up_path   = base.join(format!("{}_{}.up.sql", timestamp, name));
    let down_path = base.join(format!("{}_{}.down.sql", timestamp, name));

    fs::write(&up_path, format!(
        "-- Migration: {name}\n-- Created:   {created}\n\n-- Write your UP migration SQL here.\n",
        name = name, created = created,
    )).with_context(|| format!("Failed to write {}", up_path.display()))?;

    fs::write(&down_path, format!(
        "-- Migration: {name} (rollback)\n-- Created:   {created}\n\n-- Write your DOWN migration SQL here.\n",
        name = name, created = created,
    )).with_context(|| format!("Failed to write {}", down_path.display()))?;

    println!("✓ Created: {}", up_path.display());
    println!("✓ Created: {}", down_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::view_name_to_path;
    use std::path::PathBuf;

    #[test]
    fn single_segment() {
        assert_eq!(
            view_name_to_path("welcome"),
            PathBuf::from("resources/views/welcome.jinja.html")
        );
    }

    #[test]
    fn two_segments() {
        assert_eq!(
            view_name_to_path("users.index"),
            PathBuf::from("resources/views/users/index.jinja.html")
        );
    }

    #[test]
    fn three_segments() {
        assert_eq!(
            view_name_to_path("admin.users.show"),
            PathBuf::from("resources/views/admin/users/show.jinja.html")
        );
    }
}
