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

    let dirs: &[&str] = &[
        "src/app/Http/Controllers/Auth",
        "src/app/Http/Requests",
        if api { "" } else { "resources/views/auth" },
    ];
    for dir in dirs.iter().filter(|d| !d.is_empty()) {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", dir))?;
    }

    let auth_mod = "src/app/Http/Controllers/Auth/mod.rs";
    let auth_mod_decls = if api {
        "pub mod ApiLoginController;\npub mod ApiRegisterController;\n"
    } else {
        "pub mod LoginController;\npub mod RegisterController;\n"
    };
    if !Path::new(auth_mod).exists() {
        fs::write(auth_mod, auth_mod_decls)
            .with_context(|| format!("Failed to write {}", auth_mod))?;
        println!("✓ Created: {}", auth_mod);
    } else {
        let existing = fs::read_to_string(auth_mod)
            .with_context(|| format!("Could not read {}", auth_mod))?;
        let decl_a = if api { "pub mod ApiLoginController;" } else { "pub mod LoginController;" };
        let decl_b = if api { "pub mod ApiRegisterController;" } else { "pub mod RegisterController;" };
        let mut updated = existing.clone();
        if !updated.contains(decl_a) {
            updated = format!("{}\n{}", updated.trim_end(), format!("\n{}", decl_a));
        }
        if !updated.contains(decl_b) {
            updated = format!("{}\n{}", updated.trim_end(), format!("\n{}", decl_b));
        }
        if updated != existing {
            fs::write(auth_mod, updated)
                .with_context(|| format!("Could not write {}", auth_mod))?;
        }
    }

    let mut files: Vec<(&str, String)> = vec![
        (
            "src/app/Http/Requests/login_request.rs",
            crate::templates::app_files::make_auth_login_request().to_string(),
        ),
        (
            "src/app/Http/Requests/register_request.rs",
            crate::templates::app_files::make_auth_register_request().to_string(),
        ),
    ];

    if api {
        files.push((
            "src/app/Http/Controllers/Auth/ApiLoginController.rs",
            crate::templates::app_files::make_auth_api_login_controller(&crate_name),
        ));
        files.push((
            "src/app/Http/Controllers/Auth/ApiRegisterController.rs",
            crate::templates::app_files::make_auth_api_register_controller(&crate_name),
        ));
    } else {
        files.push((
            "src/app/Http/Controllers/Auth/LoginController.rs",
            crate::templates::app_files::make_auth_login_controller(&crate_name),
        ));
        files.push((
            "src/app/Http/Controllers/Auth/RegisterController.rs",
            crate::templates::app_files::make_auth_register_controller(&crate_name),
        ));
        files.push((
            "src/app/Http/Controllers/DashboardController.rs",
            crate::templates::app_files::make_auth_dashboard_controller(&crate_name),
        ));
        files.push((
            "resources/views/auth/login.jinja.html",
            crate::templates::app_files::view_auth_login().to_string(),
        ));
        files.push((
            "resources/views/auth/register.jinja.html",
            crate::templates::app_files::view_auth_register().to_string(),
        ));
        files.push((
            "resources/views/dashboard.jinja.html",
            crate::templates::app_files::view_auth_dashboard().to_string(),
        ));
    }

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
    inject_mod_decl("src/app/Http/Requests/mod.rs", "login_request")?;
    inject_mod_decl("src/app/Http/Requests/mod.rs", "register_request")?;

    if api {
        let use_decl =
            "use crate::app::Http::Controllers::Auth::{ApiLoginController, ApiRegisterController};";
        let route_lines = "\n        \
.route(\"/api/auth/login\",    post(ApiLoginController::store))\n        \
.route(\"/api/auth/refresh\",  post(ApiLoginController::refresh))\n        \
.route(\"/api/auth/logout\",   post(ApiLoginController::destroy))\n        \
.route(\"/api/auth/register\", post(ApiRegisterController::store))\n        \
.route(\"/api/me\",            get(ApiLoginController::me))";
        inject_auth_into_routes("src/routes/api.rs", use_decl, route_lines)?;
    } else {
        inject_mod_decl("src/app/Http/Controllers/mod.rs", "DashboardController")?;
        let use_decl = "use crate::app::Http::Controllers::Auth::{LoginController, RegisterController};\nuse crate::app::Http::Controllers::DashboardController;";
        let route_lines = "\n        \
.route(\"/login\",    get(LoginController::show).post(LoginController::store))\n        \
.route(\"/logout\",   post(LoginController::destroy))\n        \
.route(\"/register\", get(RegisterController::show).post(RegisterController::store))\n        \
.route(\"/dashboard\", get(DashboardController::index))";
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
    use super::*;
    use std::path::PathBuf;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir { tempfile::tempdir().unwrap() }

    fn api_content() -> String {
        "use axum::{routing::get, Router};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/api/status\", get(status))\n}\n".into()
    }

    fn web_content() -> String {
        "use axum::{routing::get, Router};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/\", get(home))\n}\n".into()
    }

    fn api_use() -> &'static str {
        "use crate::app::Http::Controllers::Auth::{ApiLoginController, ApiRegisterController};"
    }
    fn api_routes() -> &'static str {
        "\n        .route(\"/api/auth/login\",    post(ApiLoginController::store))\n        .route(\"/api/auth/refresh\",  post(ApiLoginController::refresh))\n        .route(\"/api/auth/logout\",   post(ApiLoginController::destroy))\n        .route(\"/api/auth/register\", post(ApiRegisterController::store))"
    }
    fn web_use() -> &'static str {
        "use crate::app::Http::Controllers::Auth::{LoginController, RegisterController};\nuse crate::app::Http::Controllers::DashboardController;"
    }
    fn web_routes() -> &'static str {
        "\n        .route(\"/login\",    get(LoginController::show).post(LoginController::store))\n        .route(\"/logout\",   post(LoginController::destroy))\n        .route(\"/register\", get(RegisterController::show).post(RegisterController::store))\n        .route(\"/dashboard\", get(DashboardController::index))"
    }

    fn inject_api(f: &std::path::Path) {
        inject_auth_into_routes(f.to_str().unwrap(), api_use(), api_routes()).unwrap();
    }
    fn inject_web(f: &std::path::Path) {
        inject_auth_into_routes(f.to_str().unwrap(), web_use(), web_routes()).unwrap();
    }

    // ── view_name_to_path ──────────────────────────────────────────────────────

    #[test]
    fn single_segment() {
        assert_eq!(view_name_to_path("welcome"), PathBuf::from("resources/views/welcome.jinja.html"));
    }
    #[test]
    fn two_segments() {
        assert_eq!(view_name_to_path("users.index"), PathBuf::from("resources/views/users/index.jinja.html"));
    }
    #[test]
    fn three_segments() {
        assert_eq!(view_name_to_path("admin.users.show"), PathBuf::from("resources/views/admin/users/show.jinja.html"));
    }

    // ── inject_mod_decl (25) ───────────────────────────────────────────────────

    // 1. empty file → decl added
    #[test]
    fn imd_01_empty_file_adds_decl() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod foo;"));
    }
    // 2. non-empty file, absent → appended
    #[test]
    fn imd_02_appends_when_absent() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod bar;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod foo;") && c.contains("pub mod bar;"));
    }
    // 3. already present → no change
    #[test]
    fn imd_03_no_op_when_present() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod foo;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap().matches("pub mod foo;").count(), 1);
    }
    // 4. missing file → Ok, no file created
    #[test]
    fn imd_04_missing_file_ok() {
        let d = tmp(); let f = d.path().join("nope.rs");
        assert!(inject_mod_decl(f.to_str().unwrap(), "foo").is_ok());
        assert!(!f.exists());
    }
    // 5. two calls same module → exactly one occurrence
    #[test]
    fn imd_05_two_calls_no_duplicate() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap().matches("pub mod foo;").count(), 1);
    }
    // 6. five calls same module → still one
    #[test]
    fn imd_06_five_calls_no_duplicate() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for _ in 0..5 { inject_mod_decl(f.to_str().unwrap(), "foo").unwrap(); }
        assert_eq!(fs::read_to_string(&f).unwrap().matches("pub mod foo;").count(), 1);
    }
    // 7. two different modules → both present
    #[test]
    fn imd_07_two_different_both_added() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "bar").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod foo;") && c.contains("pub mod bar;"));
    }
    // 8. existing content preserved
    #[test]
    fn imd_08_existing_content_preserved() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod existing;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "new_mod").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod existing;") && c.contains("pub mod new_mod;"));
    }
    // 9. snake_case name (login_request)
    #[test]
    fn imd_09_snake_case_login_request() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "login_request").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod login_request;"));
    }
    // 10. snake_case name (register_request)
    #[test]
    fn imd_10_snake_case_register_request() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod login_request;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "register_request").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod login_request;") && c.contains("pub mod register_request;"));
    }
    // 11. PascalCase (Auth)
    #[test]
    fn imd_11_pascal_case_auth() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Auth").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod Auth;"));
    }
    // 12. PascalCase (ApiLoginController)
    #[test]
    fn imd_12_pascal_case_api_login_controller() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "ApiLoginController").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod ApiLoginController;"));
    }
    // 13. PascalCase (ApiRegisterController)
    #[test]
    fn imd_13_pascal_case_api_register_controller() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "ApiRegisterController").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod ApiRegisterController;"));
    }
    // 14. name is prefix of existing → both distinct
    #[test]
    fn imd_14_prefix_of_existing_both_present() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod log_request;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "log").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod log_request;") && c.contains("pub mod log;"));
    }
    // 15. existing is prefix of new → both distinct
    #[test]
    fn imd_15_existing_is_prefix_of_new() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod log;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "log_request").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod log;") && c.contains("pub mod log_request;"));
    }
    // 16. multiple existing → new appended
    #[test]
    fn imd_16_multiple_existing_appends_new() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod a;\npub mod b;\npub mod c;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "d").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        for m in &["a", "b", "c", "d"] { assert!(c.contains(&format!("pub mod {};", m))); }
    }
    // 17. duplicate of first in multiple → not duplicated
    #[test]
    fn imd_17_no_dup_first_in_multiple() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod a;\npub mod b;\npub mod c;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "a").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap().matches("pub mod a;").count(), 1);
    }
    // 18. duplicate of last in multiple → not duplicated
    #[test]
    fn imd_18_no_dup_last_in_multiple() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod a;\npub mod b;\npub mod c;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "c").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap().matches("pub mod c;").count(), 1);
    }
    // 19. file without trailing newline → injection still works
    #[test]
    fn imd_19_no_trailing_newline() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod foo;").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "bar").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod bar;"));
    }
    // 20. output ends with newline
    #[test]
    fn imd_20_output_ends_with_newline() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        assert!(fs::read_to_string(&f).unwrap().ends_with('\n'));
    }
    // 21. whitespace-only file → decl added
    #[test]
    fn imd_21_whitespace_only_file() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "   \n  \n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "foo").unwrap();
        assert!(fs::read_to_string(&f).unwrap().contains("pub mod foo;"));
    }
    // 22. returns Ok on success
    #[test]
    fn imd_22_returns_ok() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        assert!(inject_mod_decl(f.to_str().unwrap(), "foo").is_ok());
    }
    // 23. three modules in sequence → each exactly once
    #[test]
    fn imd_23_three_modules_each_once() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for m in &["a", "b", "c"] { inject_mod_decl(f.to_str().unwrap(), m).unwrap(); }
        let c = fs::read_to_string(&f).unwrap();
        for m in &["a", "b", "c"] { assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1); }
    }
    // 24. realistic Controllers/mod.rs: HomeController, UserController, then Auth
    #[test]
    fn imd_24_realistic_controllers_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod HomeController;\npub mod UserController;\npub mod StatusController;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Auth").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        for m in &["HomeController", "UserController", "StatusController", "Auth"] {
            assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1, "{} must appear once", m);
        }
    }
    // 25. inject login_request then register_request → both in Requests/mod.rs
    #[test]
    fn imd_25_login_and_register_request_both_present() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "login_request").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "register_request").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod login_request;") && c.contains("pub mod register_request;"));
    }

    // ── inject_auth_into_routes — API (35) ────────────────────────────────────

    // 26. /api/auth/login injected
    #[test]
    fn api_26_login_injected() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/auth/login"));
    }
    // 27. /api/auth/refresh injected
    #[test]
    fn api_27_refresh_injected() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/auth/refresh"));
    }
    // 28. /api/auth/logout injected
    #[test]
    fn api_28_logout_injected() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/auth/logout"));
    }
    // 29. /api/auth/register injected
    #[test]
    fn api_29_register_injected() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/auth/register"));
    }
    // 30. use_decl contains ApiLoginController
    #[test]
    fn api_30_use_decl_has_api_login_controller() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("ApiLoginController"));
    }
    // 31. use_decl contains ApiRegisterController
    #[test]
    fn api_31_use_decl_has_api_register_controller() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("ApiRegisterController"));
    }
    // 32. use_decl placed before pub fn routes
    #[test]
    fn api_32_use_decl_before_pub_fn_routes() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.find("ApiLoginController").unwrap() < c.find("pub fn routes").unwrap());
    }
    // 33. route_lines before closing brace
    #[test]
    fn api_33_routes_before_closing_brace() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.rfind("/api/auth/logout").unwrap() < c.rfind('}').unwrap());
    }
    // 34. existing route preserved
    #[test]
    fn api_34_existing_route_preserved() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/status"));
    }
    // 35. no #[path] in result
    #[test]
    fn api_35_no_path_attribute() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(!fs::read_to_string(&f).unwrap().contains("#[path"));
    }
    // 36. routing::get, → routing::{get, post},
    #[test]
    fn api_36_routing_get_comma_replaced() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("routing::{get, post}"));
    }
    // 37. routing::get, no longer present after replacement
    #[test]
    fn api_37_routing_get_comma_removed() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(!fs::read_to_string(&f).unwrap().contains("routing::get,"));
    }
    // 38. already has routing::{get, post} → not doubled
    #[test]
    fn api_38_already_has_get_post_not_doubled() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = api_content().replace("routing::get,", "routing::{get, post},");
        fs::write(&f, &c).unwrap(); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("routing::{get, post}").count(), 1);
    }
    // 39. routing::get} → routing::{get, post}}
    #[test]
    fn api_39_routing_get_brace_replaced() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = "use axum::{routing::get};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/api/status\", get(status))\n}\n";
        fs::write(&f, c).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("routing::{get, post}"));
    }
    // 40. idempotent: /api/auth/login appears once after two calls
    #[test]
    fn api_40_idempotent_login() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("/api/auth/login").count(), 1);
    }
    // 41. idempotent: /api/auth/refresh once after two calls
    #[test]
    fn api_41_idempotent_refresh() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("/api/auth/refresh").count(), 1);
    }
    // 42. idempotent: /api/auth/logout once after two calls
    #[test]
    fn api_42_idempotent_logout() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("/api/auth/logout").count(), 1);
    }
    // 43. idempotent: /api/auth/register once after two calls
    #[test]
    fn api_43_idempotent_register() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("/api/auth/register").count(), 1);
    }
    // 44. idempotent: use_decl once after two calls
    #[test]
    fn api_44_idempotent_use_decl() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("ApiLoginController, ApiRegisterController").count(), 1);
    }
    // 45. five calls → each route still exactly once
    #[test]
    fn api_45_five_calls_each_once() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        for _ in 0..5 { inject_api(&f); }
        let c = fs::read_to_string(&f).unwrap();
        for route in &["/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register"] {
            assert_eq!(c.matches(route).count(), 1, "{} must appear exactly once", route);
        }
    }
    // 46. missing file → Ok, not created
    #[test]
    fn api_46_missing_file_ok() {
        let d = tmp(); let f = d.path().join("nope.rs");
        assert!(inject_auth_into_routes(f.to_str().unwrap(), api_use(), api_routes()).is_ok());
        assert!(!f.exists());
    }
    // 47. no pub fn routes → Err
    #[test]
    fn api_47_no_pub_fn_routes_err() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, "use axum::Router;\nfn not_routes() {}\n").unwrap();
        assert!(inject_auth_into_routes(f.to_str().unwrap(), api_use(), api_routes()).is_err());
    }
    // 48. no closing brace → Err
    #[test]
    fn api_48_no_closing_brace_err() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, "use axum::Router;\n\npub fn routes() -> Router {\n    Router::new()").unwrap();
        assert!(inject_auth_into_routes(f.to_str().unwrap(), api_use(), api_routes()).is_err());
    }
    // 49. uses crate:: path in result
    #[test]
    fn api_49_uses_crate_path() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("use crate::app::Http::Controllers::Auth::"));
    }
    // 50. ApiLoginController::store present
    #[test]
    fn api_50_login_store_present() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("ApiLoginController::store"));
    }
    // 51. ApiLoginController::refresh present
    #[test]
    fn api_51_login_refresh_present() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("ApiLoginController::refresh"));
    }
    // 52. ApiLoginController::destroy present
    #[test]
    fn api_52_login_destroy_present() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("ApiLoginController::destroy"));
    }
    // 53. ApiRegisterController::store present
    #[test]
    fn api_53_register_store_present() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("ApiRegisterController::store"));
    }
    // 54. multiple existing routes all preserved
    #[test]
    fn api_54_multiple_existing_preserved() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = "use axum::{routing::get, Router};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/api/status\", get(status))\n        .route(\"/api/users\", get(users))\n        .route(\"/api/items\", get(items))\n}\n";
        fs::write(&f, c).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        for r in &["/api/status", "/api/users", "/api/items", "/api/auth/login"] {
            assert!(c.contains(r), "missing {}", r);
        }
    }
    // 55. already has Controllers::Auth:: marker → skip, Ok returned
    #[test]
    fn api_55_skip_when_marker_present() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = format!("{}\n// Controllers::Auth:: here", api_content());
        fs::write(&f, &c).unwrap();
        assert!(inject_auth_into_routes(f.to_str().unwrap(), api_use(), api_routes()).is_ok());
        assert!(!fs::read_to_string(&f).unwrap().contains("/api/auth/login"));
    }
    // 56. web routes NOT present in api.rs result
    #[test]
    fn api_56_no_web_routes_in_api() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(!c.contains(".route(\"/login\"") && !c.contains(".route(\"/register\""));
    }
    // 57. login route uses post()
    #[test]
    fn api_57_login_uses_post() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("post(ApiLoginController::store)"));
    }
    // 58. no routing import at all → no crash, routes added
    #[test]
    fn api_58_no_routing_import_no_crash() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = "use axum::Router;\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n}\n";
        fs::write(&f, c).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/auth/login"));
    }
    // 59. empty routes body → routes added
    #[test]
    fn api_59_empty_routes_body() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = "use axum::{routing::{get, post}, Router};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n}\n";
        fs::write(&f, c).unwrap(); inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/auth/login"));
    }
    // 60. realistic api.rs (like demo) → all auth routes injected, all existing preserved
    #[test]
    fn api_60_realistic_api_rs() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = "use axum::{routing::{get, post}, Router};\nuse std::sync::Arc;\nuse my_app::AppState;\nuse crate::app::Http::Controllers::{UserController, StatusController};\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/api/users\", get(UserController::index).post(UserController::store))\n        .route(\"/api/status\", get(StatusController::index))\n        .route(\"/api/users/mock\", get(UserController::mock))\n}\n";
        fs::write(&f, c).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        for r in &["/api/users", "/api/status", "/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register"] {
            assert!(c.contains(r), "missing {}", r);
        }
    }

    // ── inject_auth_into_routes — web (25) ────────────────────────────────────

    // 61. /login GET injected
    #[test]
    fn web_61_login_get_injected() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("\"/login\"") && c.contains("get(LoginController::show)"));
    }
    // 62. /login POST injected
    #[test]
    fn web_62_login_post_injected() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("post(LoginController::store)"));
    }
    // 63. /logout injected
    #[test]
    fn web_63_logout_injected() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("\"/logout\""));
    }
    // 64. /register GET injected
    #[test]
    fn web_64_register_get_injected() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("\"/register\"") && c.contains("get(RegisterController::show)"));
    }
    // 65. /register POST injected
    #[test]
    fn web_65_register_post_injected() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("post(RegisterController::store)"));
    }
    // 66. use_decl contains LoginController
    #[test]
    fn web_66_use_decl_has_login_controller() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("LoginController"));
    }
    // 67. use_decl contains RegisterController
    #[test]
    fn web_67_use_decl_has_register_controller() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("RegisterController"));
    }
    // 68. routing::get, → routing::{get, post},
    #[test]
    fn web_68_routing_replaced() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("routing::{get, post}"));
    }
    // 69. existing home route preserved
    #[test]
    fn web_69_existing_route_preserved() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("\"/\""));
    }
    // 70. use_decl before pub fn routes
    #[test]
    fn web_70_use_decl_before_pub_fn_routes() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.find("LoginController").unwrap() < c.find("pub fn routes").unwrap());
    }
    // 71. idempotent: /login once after two calls
    #[test]
    fn web_71_idempotent_login() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f); inject_web(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("\"/login\"").count(), 1);
    }
    // 72. idempotent: /register once after two calls
    #[test]
    fn web_72_idempotent_register() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f); inject_web(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("\"/register\"").count(), 1);
    }
    // 73. idempotent: /logout once after two calls
    #[test]
    fn web_73_idempotent_logout() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f); inject_web(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("\"/logout\"").count(), 1);
    }
    // 74. idempotent: use_decl once after two calls
    #[test]
    fn web_74_idempotent_use_decl() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f); inject_web(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("LoginController, RegisterController").count(), 1);
    }
    // 75. no #[path] in result
    #[test]
    fn web_75_no_path_attribute() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(!fs::read_to_string(&f).unwrap().contains("#[path"));
    }
    // 76. missing file → Ok
    #[test]
    fn web_76_missing_file_ok() {
        let d = tmp(); let f = d.path().join("web.rs");
        assert!(inject_auth_into_routes(f.to_str().unwrap(), web_use(), web_routes()).is_ok());
    }
    // 77. no pub fn routes → Err
    #[test]
    fn web_77_no_pub_fn_routes_err() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, "use axum::Router;\nfn setup() {}\n").unwrap();
        assert!(inject_auth_into_routes(f.to_str().unwrap(), web_use(), web_routes()).is_err());
    }
    // 78. API routes NOT in web.rs result
    #[test]
    fn web_78_no_api_routes_in_web() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(!c.contains("/api/auth/login") && !c.contains("/api/auth/refresh"));
    }
    // 79. LoginController::destroy present for logout
    #[test]
    fn web_79_logout_uses_destroy() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("LoginController::destroy"));
    }
    // 80. five calls → /login exactly once
    #[test]
    fn web_80_five_calls_login_once() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap();
        for _ in 0..5 { inject_web(&f); }
        assert_eq!(fs::read_to_string(&f).unwrap().matches("\"/login\"").count(), 1);
    }
    // 81. crate:: path in use_decl
    #[test]
    fn web_81_uses_crate_path() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("use crate::app::Http::Controllers::Auth::"));
    }
    // 82. pub fn routes preserved
    #[test]
    fn web_82_pub_fn_routes_preserved() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("pub fn routes"));
    }
    // 83. already has Controllers::Auth:: → skip, Ok
    #[test]
    fn web_83_skip_when_marker_present() {
        let d = tmp(); let f = d.path().join("web.rs");
        let c = format!("{}\n// Controllers::Auth:: here", web_content());
        fs::write(&f, &c).unwrap();
        assert!(inject_auth_into_routes(f.to_str().unwrap(), web_use(), web_routes()).is_ok());
        assert!(!fs::read_to_string(&f).unwrap().contains("\"/login\""));
    }
    // 84. multiple existing routes all preserved
    #[test]
    fn web_84_multiple_existing_preserved() {
        let d = tmp(); let f = d.path().join("web.rs");
        let c = "use axum::{routing::get, Router};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/\", get(home))\n        .route(\"/about\", get(about))\n        .route(\"/contact\", get(contact))\n}\n";
        fs::write(&f, c).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        for r in &["\"/\"", "\"/about\"", "\"/contact\"", "\"/login\""] {
            assert!(c.contains(r), "missing {}", r);
        }
    }
    // 85. login uses get().post() chained
    #[test]
    fn web_85_login_get_post_chained() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("get(LoginController::show).post(LoginController::store)"));
    }
    // 86. /dashboard GET injected
    #[test]
    fn web_86_dashboard_get_injected() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("\"/dashboard\"") && c.contains("get(DashboardController::index)"));
    }
    // 87. DashboardController present in use_decl after web inject
    #[test]
    fn web_87_dashboard_controller_in_use_decl() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("DashboardController"));
    }

    // ── combined / edge cases (15) ─────────────────────────────────────────────

    // 86. inject_mod_decl + inject_auth_into_routes both succeed
    #[test]
    fn cmb_86_mod_decl_and_route_injection() {
        let d = tmp();
        let mod_f = d.path().join("mod.rs"); let routes_f = d.path().join("api.rs");
        fs::write(&mod_f, "").unwrap(); fs::write(&routes_f, api_content()).unwrap();
        inject_mod_decl(mod_f.to_str().unwrap(), "Auth").unwrap();
        inject_api(&routes_f);
        assert!(fs::read_to_string(&mod_f).unwrap().contains("pub mod Auth;"));
        assert!(fs::read_to_string(&routes_f).unwrap().contains("/api/auth/login"));
    }
    // 87. API injection does not modify web.rs
    #[test]
    fn cmb_87_api_inject_leaves_web_unchanged() {
        let d = tmp();
        let api_f = d.path().join("api.rs"); let web_f = d.path().join("web.rs");
        fs::write(&api_f, api_content()).unwrap(); fs::write(&web_f, web_content()).unwrap();
        inject_api(&api_f);
        assert_eq!(fs::read_to_string(&web_f).unwrap(), web_content());
    }
    // 88. web injection does not modify api.rs
    #[test]
    fn cmb_88_web_inject_leaves_api_unchanged() {
        let d = tmp();
        let api_f = d.path().join("api.rs"); let web_f = d.path().join("web.rs");
        fs::write(&api_f, api_content()).unwrap(); fs::write(&web_f, web_content()).unwrap();
        inject_web(&web_f);
        assert_eq!(fs::read_to_string(&api_f).unwrap(), api_content());
    }
    // 89. both API and web injected into separate files
    #[test]
    fn cmb_89_both_api_and_web_injected() {
        let d = tmp();
        let api_f = d.path().join("api.rs"); let web_f = d.path().join("web.rs");
        fs::write(&api_f, api_content()).unwrap(); fs::write(&web_f, web_content()).unwrap();
        inject_api(&api_f); inject_web(&web_f);
        assert!(fs::read_to_string(&api_f).unwrap().contains("/api/auth/refresh"));
        assert!(fs::read_to_string(&web_f).unwrap().contains("\"/login\""));
    }
    // 90. three mod decls in sequence each appear exactly once
    #[test]
    fn cmb_90_three_mod_decls_each_once() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for m in &["Auth", "login_request", "register_request"] {
            inject_mod_decl(f.to_str().unwrap(), m).unwrap();
        }
        let c = fs::read_to_string(&f).unwrap();
        for m in &["Auth", "login_request", "register_request"] {
            assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1, "{} must appear once", m);
        }
    }
    // 91. all four API routes present after single inject_api call
    #[test]
    fn cmb_91_all_four_api_routes_at_once() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        for r in &["/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register"] {
            assert!(c.contains(r), "missing {}", r);
        }
    }
    // 92. all four web auth routes present after single inject_web call
    #[test]
    fn cmb_92_all_web_auth_routes_at_once() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        for r in &["\"/login\"", "\"/logout\"", "\"/register\"", "\"/dashboard\""] {
            assert!(c.contains(r), "missing {}", r);
        }
    }
    // 93. register uses get().post() chained
    #[test]
    fn cmb_93_register_get_post_chained() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("get(RegisterController::show).post(RegisterController::store)"));
    }
    // 94. rfind finds last closing brace (inline closure in route)
    #[test]
    fn cmb_94_rfind_last_brace_with_inline_closure() {
        let d = tmp(); let f = d.path().join("api.rs");
        let c = "use axum::{routing::{get, post}, Router};\nuse my_app::AppState;\nuse std::sync::Arc;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/api/status\", get(|| async { \"ok\" }))\n}\n";
        fs::write(&f, c).unwrap(); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("/api/auth/login"));
        assert!(c.trim_end().ends_with('}'));
    }
    // 95. inject Auth mod twice → still once
    #[test]
    fn cmb_95_inject_auth_mod_twice_once() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod UserController;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Auth").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Auth").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap().matches("pub mod Auth;").count(), 1);
    }
    // 96. realistic web.rs: existing routes preserved, auth injected, routing replaced
    #[test]
    fn cmb_96_realistic_web_rs() {
        let d = tmp(); let f = d.path().join("web.rs");
        let c = "use axum::{routing::get, Router};\nuse std::sync::Arc;\nuse my_app::AppState;\nuse crate::app::Http::Controllers::HomeController;\n\npub fn routes() -> Router<Arc<AppState>> {\n    Router::new()\n        .route(\"/\", get(HomeController::index))\n        .route(\"/dashboard\", get(HomeController::dashboard))\n}\n";
        fs::write(&f, c).unwrap(); inject_web(&f);
        let c = fs::read_to_string(&f).unwrap();
        for r in &["\"/\"", "\"/dashboard\"", "\"/login\"", "\"/logout\"", "\"/register\""] {
            assert!(c.contains(r), "missing {}", r);
        }
        assert!(c.contains("routing::{get, post}"));
    }
    // 97. API inject: use_decl appears exactly once
    #[test]
    fn cmb_97_api_use_decl_exactly_once() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap(); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches(api_use()).count(), 1);
    }
    // 98. web inject: use_decl appears exactly once
    #[test]
    fn cmb_98_web_use_decl_exactly_once() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap(); inject_web(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches(web_use()).count(), 1);
    }
    // 99. inject_mod_decl + inject_auth_into_routes for web: mod.rs and web.rs both correct
    #[test]
    fn cmb_99_web_mod_and_routes_combined() {
        let d = tmp();
        let mod_f = d.path().join("mod.rs"); let web_f = d.path().join("web.rs");
        fs::write(&mod_f, "pub mod HomeController;\n").unwrap();
        fs::write(&web_f, web_content()).unwrap();
        inject_mod_decl(mod_f.to_str().unwrap(), "Auth").unwrap();
        inject_web(&web_f);
        assert!(fs::read_to_string(&mod_f).unwrap().contains("pub mod Auth;"));
        assert!(fs::read_to_string(&web_f).unwrap().contains("\"/login\""));
    }
    // 100. full round-trip: fresh files → inject API → inject again → each element exactly once
    #[test]
    fn cmb_100_full_round_trip_api_idempotent() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        inject_api(&f); inject_api(&f);
        let c = fs::read_to_string(&f).unwrap();
        for route in &["/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register"] {
            assert_eq!(c.matches(route).count(), 1, "{} must appear exactly once", route);
        }
        assert_eq!(c.matches(api_use()).count(), 1, "use_decl must appear exactly once");
        assert_eq!(c.matches("routing::{get, post}").count(), 1, "routing import must appear exactly once");
    }
}
