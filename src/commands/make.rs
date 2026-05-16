use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

fn inject_mod_decl(mod_file: &str, mod_name: &str) -> Result<()> {
    let path = Path::new(mod_file);
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", mod_file))?;
    let decl = format!("pub mod {};", mod_name);
    if content.contains(&decl) {
        return Ok(());
    }
    let updated = if content.trim_end().is_empty() {
        format!("{}\n", decl)
    } else {
        format!("{}\n{}\n", content.trim_end(), decl)
    };
    fs::write(path, updated)
        .with_context(|| format!("Could not write {}", mod_file))?;
    Ok(())
}

pub fn controller(name: &str) -> Result<()> {
    let path = Path::new("src/app/Http/Controllers").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Controller already exists: {}", path.display());
    }

    let content = format!(
        r#"use axum::{{Json, response::IntoResponse}};
use serde_json::json;

pub async fn index() -> impl IntoResponse {{
    Json(json!({{ "message": "{} index" }}))
}}

pub async fn show() -> impl IntoResponse {{
    Json(json!({{ "message": "{} show" }}))
}}

pub async fn store() -> impl IntoResponse {{
    Json(json!({{ "message": "{} store" }}))
}}

pub async fn update() -> impl IntoResponse {{
    Json(json!({{ "message": "{} update" }}))
}}

pub async fn destroy() -> impl IntoResponse {{
    Json(json!({{ "message": "{} destroy" }}))
}}
"#,
        name, name, name, name, name
    );

    fs::write(&path, content)
        .with_context(|| format!("Failed to create controller: {}", path.display()))?;

    inject_mod_decl("src/app/Http/Controllers/mod.rs", name)?;

    println!("✓ Controller created: {}", path.display());
    Ok(())
}

pub fn request(name: &str) -> Result<()> {
    let path = Path::new("src/app/Http/Requests").join(format!("{}.rs", name));

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

    inject_mod_decl("src/app/Http/Requests/mod.rs", name)?;

    println!("✓ Request created: {}", path.display());
    Ok(())
}

pub fn model(name: &str) -> Result<()> {
    let path = Path::new("src/app/Models").join(format!("{}.rs", name));

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

    inject_mod_decl("src/app/Models/mod.rs", name)?;

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
    let path = Path::new("src/app/Http/Middleware").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Middleware already exists: {}", path.display());
    }

    let content = crate::templates::app_files::make_middleware_template(name);

    fs::write(&path, &content)
        .with_context(|| format!("Failed to create middleware: {}", path.display()))?;

    inject_mod_decl("src/app/Http/Middleware/mod.rs", name)?;

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

fn read_crate_name() -> Result<String> {
    let raw = fs::read_to_string("Cargo.toml")
        .with_context(|| "Could not read Cargo.toml — run this command from your app root")?;
    for line in raw.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim();
            let name = rest.trim_matches('"');
            if !name.is_empty() {
                return Ok(name.replace('-', "_"));
            }
        }
    }
    anyhow::bail!("Could not find package.name in Cargo.toml")
}

fn ensure_users_migration() -> Result<()> {
    let base = Path::new("database/migrations");
    if !base.exists() {
        fs::create_dir_all(base).with_context(|| "Failed to create database/migrations")?;
    }

    let already_exists = fs::read_dir(base)
        .with_context(|| "Failed to read database/migrations")?
        .filter_map(|e| e.ok())
        .any(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.contains("create_users") && s.ends_with(".up.sql")
        });

    if already_exists {
        return Ok(());
    }

    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d%H%M%S");

    let up_path = base.join(format!("{}_create_users_table.up.sql", timestamp));
    let down_path = base.join(format!("{}_create_users_table.down.sql", timestamp));

    fs::write(&up_path, crate::templates::app_files::initial_migration_up_sql())
        .with_context(|| format!("Failed to write {}", up_path.display()))?;
    fs::write(&down_path, crate::templates::app_files::initial_migration_down_sql())
        .with_context(|| format!("Failed to write {}", down_path.display()))?;

    println!("✓ Created: {}", up_path.display());
    println!("✓ Created: {}", down_path.display());
    Ok(())
}

pub fn auth(api: bool) -> Result<()> {
    let crate_name = read_crate_name()?;

    ensure_users_migration()?;

    let dirs: &[&str] = if api {
        &["src/app/Http/Controllers/Auth"]
    } else {
        &[
            "src/app/Http/Controllers/Auth",
            "src/app/Http/Requests",
            "resources/views/auth",
        ]
    };
    for dir in dirs {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", dir))?;
    }

    let auth_mod = "src/app/Http/Controllers/Auth/mod.rs";
    if !Path::new(auth_mod).exists() {
        fs::write(auth_mod, "pub mod LoginController;\npub mod RegisterController;\n")
            .with_context(|| format!("Failed to write {}", auth_mod))?;
        println!("✓ Created: {}", auth_mod);
    }

    let files: Vec<(&str, String)> = if api {
        vec![
            (
                "src/app/Http/Controllers/Auth/LoginController.rs",
                crate::templates::app_files::make_auth_api_login_controller(&crate_name),
            ),
            (
                "src/app/Http/Controllers/Auth/RegisterController.rs",
                crate::templates::app_files::make_auth_api_register_controller(&crate_name),
            ),
        ]
    } else {
        vec![
            (
                "src/app/Http/Controllers/Auth/LoginController.rs",
                crate::templates::app_files::make_auth_login_controller(&crate_name),
            ),
            (
                "src/app/Http/Controllers/Auth/RegisterController.rs",
                crate::templates::app_files::make_auth_register_controller(&crate_name),
            ),
            (
                "src/app/Http/Requests/login_request.rs",
                crate::templates::app_files::make_auth_login_request().to_string(),
            ),
            (
                "src/app/Http/Requests/register_request.rs",
                crate::templates::app_files::make_auth_register_request().to_string(),
            ),
            (
                "resources/views/auth/login.jinja.html",
                crate::templates::app_files::view_auth_login().to_string(),
            ),
            (
                "resources/views/auth/register.jinja.html",
                crate::templates::app_files::view_auth_register().to_string(),
            ),
        ]
    };

    for (path, content) in &files {
        let p = Path::new(path);
        if p.exists() {
            println!("  skip (already exists): {}", path);
            continue;
        }
        fs::write(p, content).with_context(|| format!("Failed to write: {}", path))?;
        println!("✓ Created: {}", path);
    }

    inject_mod_decl("src/app/Http/Controllers/mod.rs", "Auth")?;

    if !api {
        inject_mod_decl("src/app/Http/Requests/mod.rs", "login_request")?;
        inject_mod_decl("src/app/Http/Requests/mod.rs", "register_request")?;
    }

    let use_decl =
        "use crate::app::Http::Controllers::Auth::{LoginController, RegisterController};";

    if api {
        let route_lines = "\n        \
.route(\"/api/auth/login\",    post(LoginController::store))\n        \
.route(\"/api/auth/logout\",   post(LoginController::destroy))\n        \
.route(\"/api/auth/register\", post(RegisterController::store))";
        inject_auth_into_routes("src/routes/api.rs", use_decl, route_lines)?;
    } else {
        let route_lines = "\n        \
.route(\"/login\",    get(LoginController::show).post(LoginController::store))\n        \
.route(\"/logout\",   post(LoginController::destroy))\n        \
.route(\"/register\", get(RegisterController::show).post(RegisterController::store))";
        inject_auth_into_routes("src/routes/web.rs", use_decl, route_lines)?;
    }

    Ok(())
}

fn inject_auth_into_routes(routes_path: &str, use_decl: &str, route_lines: &str) -> Result<()> {
    let path = Path::new(routes_path);
    if !path.exists() {
        println!("  Warning: {} not found — add routes manually.", routes_path);
        return Ok(());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", routes_path))?;

    if content.contains("Controllers::Auth::") {
        println!("  Routes already present in {} — skipping.", routes_path);
        return Ok(());
    }

    let content = if content.contains("routing::get,") && !content.contains("routing::{get, post}") {
        content.replace("routing::get,", "routing::{get, post},")
    } else if content.contains("routing::get}") && !content.contains("routing::{get, post}") {
        content.replace("routing::get}", "routing::{get, post}}")
    } else {
        content
    };

    let fn_marker = "\npub fn routes";
    let fn_pos = content
        .find(fn_marker)
        .ok_or_else(|| anyhow::anyhow!("Could not find `pub fn routes` in {}", routes_path))?;

    let content = format!(
        "{}\n{}\n{}",
        &content[..fn_pos],
        use_decl,
        &content[fn_pos..]
    );

    let close_pos = content
        .rfind("\n}")
        .ok_or_else(|| anyhow::anyhow!("Could not find closing brace in {}", routes_path))?;

    let content = format!(
        "{}{}{}",
        &content[..close_pos],
        route_lines,
        &content[close_pos..]
    );

    fs::write(path, &content)
        .with_context(|| format!("Could not write {}", routes_path))?;

    println!("✓ Routes injected into {}", routes_path);
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
