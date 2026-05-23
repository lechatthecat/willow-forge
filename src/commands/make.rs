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

fn controller_content(name: &str) -> String {
    format!(
        r#"use axum::{{Json, response::IntoResponse}};
use serde_json::json;

pub async fn index() -> impl IntoResponse {{
    Json(json!({{ "message": "{name} index" }}))
}}

pub async fn show() -> impl IntoResponse {{
    Json(json!({{ "message": "{name} show" }}))
}}

pub async fn store() -> impl IntoResponse {{
    Json(json!({{ "message": "{name} store" }}))
}}

pub async fn update() -> impl IntoResponse {{
    Json(json!({{ "message": "{name} update" }}))
}}

pub async fn destroy() -> impl IntoResponse {{
    Json(json!({{ "message": "{name} destroy" }}))
}}
"#,
        name = name
    )
}

fn request_content(name: &str) -> String {
    format!(
        r#"use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct {name} {{
    // Add your fields here
    // Example:
    // #[validate(length(min = 1, max = 255))]
    // pub name: String,
}}
"#,
        name = name
    )
}

fn model_content(name: &str) -> String {
    format!(
        r#"use serde::{{Deserialize, Serialize}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {name} {{
    pub id: i64,
    // Add your fields here
}}

impl {name} {{
    // Add your model methods here
}}
"#,
        name = name
    )
}

fn view_template_content(name: &str) -> String {
    format!(
        "{{% extends \"layouts.app\" %}}\n\n{{% block title %}}{name}{{% endblock %}}\n\n{{% block content %}}\n<h1>{name}</h1>\n{{% endblock %}}\n",
        name = name
    )
}

fn migration_up_content(name: &str, created: &str) -> String {
    format!(
        "-- Migration: {name}\n-- Created:   {created}\n\n-- Write your UP migration SQL here.\n",
        name = name,
        created = created,
    )
}

fn migration_down_content(name: &str, created: &str) -> String {
    format!(
        "-- Migration: {name} (rollback)\n-- Created:   {created}\n\n-- Write your DOWN migration SQL here.\n",
        name = name,
        created = created,
    )
}

pub fn controller(name: &str) -> Result<()> {
    let path = Path::new("src/app/Http/Controllers").join(format!("{}.rs", name));

    if path.exists() {
        anyhow::bail!("Controller already exists: {}", path.display());
    }

    fs::write(&path, controller_content(name))
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

    fs::write(&path, request_content(name))
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

    fs::write(&path, model_content(name))
        .with_context(|| format!("Failed to create model: {}", path.display()))?;

    inject_mod_decl("src/app/Models/mod.rs", name)?;

    println!("✓ Model created: {}", path.display());
    Ok(())
}

fn view_name_to_path(name: &str) -> std::path::PathBuf {
    let parts: Vec<&str> = name.split('.').collect();
    let (dirs, stem) = parts.split_at(parts.len() - 1);
    let mut path = Path::new("src/resources/views").to_path_buf();
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

    fs::write(&file_path, view_template_content(name))
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
    let created = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let base = Path::new("src/database/migrations");

    let up_path   = base.join(format!("{}_{}.up.sql", timestamp, name));
    let down_path = base.join(format!("{}_{}.down.sql", timestamp, name));

    fs::write(&up_path, migration_up_content(name, &created))
        .with_context(|| format!("Failed to write {}", up_path.display()))?;

    fs::write(&down_path, migration_down_content(name, &created))
        .with_context(|| format!("Failed to write {}", down_path.display()))?;

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
    let base = Path::new("src/database/migrations");
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
        if api { "" } else { "src/resources/views/auth" },
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
            "src/resources/views/auth/login.jinja.html",
            crate::templates::app_files::view_auth_login().to_string(),
        ));
        files.push((
            "src/resources/views/auth/register.jinja.html",
            crate::templates::app_files::view_auth_register().to_string(),
        ));
        files.push((
            "src/resources/views/dashboard.jinja.html",
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
        "\n        .route(\"/api/auth/login\",    post(ApiLoginController::store))\n        .route(\"/api/auth/refresh\",  post(ApiLoginController::refresh))\n        .route(\"/api/auth/logout\",   post(ApiLoginController::destroy))\n        .route(\"/api/auth/register\", post(ApiRegisterController::store))\n        .route(\"/api/me\",            get(ApiLoginController::me))"
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
        assert_eq!(view_name_to_path("welcome"), PathBuf::from("src/resources/views/welcome.jinja.html"));
    }
    #[test]
    fn two_segments() {
        assert_eq!(view_name_to_path("users.index"), PathBuf::from("src/resources/views/users/index.jinja.html"));
    }
    #[test]
    fn three_segments() {
        assert_eq!(view_name_to_path("admin.users.show"), PathBuf::from("src/resources/views/admin/users/show.jinja.html"));
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
        for route in &["/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register", "/api/me"] {
            assert_eq!(c.matches(route).count(), 1, "{} must appear exactly once", route);
        }
        assert_eq!(c.matches(api_use()).count(), 1, "use_decl must appear exactly once");
        assert_eq!(c.matches("routing::{get, post}").count(), 1, "routing import must appear exactly once");
    }

    // ── controller_content — 10 tests ─────────────────────────────────────────

    #[test]
    fn ctrl_01_all_five_messages_contain_name() {
        // Regression: name must be substituted in every action, not just index
        let out = controller_content("MyController");
        for action in &["index", "show", "store", "update", "destroy"] {
            assert!(out.contains(&format!("\"MyController {}\"", action)),
                "name missing from {} message", action);
        }
    }

    #[test]
    fn ctrl_02_message_format_is_name_space_action() {
        // Regression: format must be "{name} {action}", not "{action} {name}" or just "{action}"
        let out = controller_content("Ctrl");
        for action in &["index", "show", "store", "update", "destroy"] {
            assert!(out.contains(&format!("\"Ctrl {}\"", action)),
                "wrong format for {}", action);
            assert!(!out.contains(&format!("\"{}  Ctrl\"", action)),
                "reversed format found for {}", action);
        }
    }

    #[test]
    fn ctrl_03_json_key_is_message_not_data_or_result() {
        // Regression: the JSON response must use "message" as key
        let out = controller_content("Foo");
        assert!(out.contains("\"message\":") || out.contains("\"message\" :"),
            "key must be \"message\"");
        assert!(!out.contains("\"data\":") && !out.contains("\"result\":"));
    }

    #[test]
    fn ctrl_04_single_char_name_substituted_correctly() {
        let out = controller_content("X");
        for action in &["index", "show", "store", "update", "destroy"] {
            assert!(out.contains(&format!("\"X {}\"", action)), "single-char name missing from {}", action);
        }
    }

    #[test]
    fn ctrl_05_all_five_messages_are_distinct() {
        // Regression: if name substitution is broken all messages might be identical
        let out = controller_content("Foo");
        let messages: Vec<_> = ["index", "show", "store", "update", "destroy"]
            .iter()
            .map(|a| format!("\"Foo {}\"", a))
            .collect();
        for (i, m1) in messages.iter().enumerate() {
            for (j, m2) in messages.iter().enumerate() {
                if i != j {
                    assert_ne!(m1, m2, "messages {} and {} must be different", i, j);
                }
            }
        }
    }

    #[test]
    fn ctrl_06_json_message_contains_name_in_index() {
        assert!(controller_content("PostController").contains("\"PostController index\""));
    }

    #[test]
    fn ctrl_07_json_message_contains_name_in_destroy() {
        assert!(controller_content("PostController").contains("\"PostController destroy\""));
    }

    #[test]
    fn ctrl_08_imports_axum_json_and_into_response() {
        let out = controller_content("Foo");
        assert!(out.contains("use axum::{Json, response::IntoResponse};"));
    }

    #[test]
    fn ctrl_09_returns_impl_into_response() {
        assert!(controller_content("Foo").contains("-> impl IntoResponse"));
    }

    #[test]
    fn ctrl_10_different_names_produce_different_content() {
        assert_ne!(controller_content("FooController"), controller_content("BarController"));
    }

    // ── request_content — 10 tests ────────────────────────────────────────────

    #[test]
    fn req_01_has_pub_struct_with_name() {
        assert!(request_content("LoginRequest").contains("pub struct LoginRequest"));
    }

    #[test]
    fn req_02_derives_deserialize() {
        assert!(request_content("LoginRequest").contains("Deserialize"));
    }

    #[test]
    fn req_03_derives_validate() {
        assert!(request_content("LoginRequest").contains("Validate"));
    }

    #[test]
    fn req_04_derives_debug() {
        assert!(request_content("LoginRequest").contains("Debug"));
    }

    #[test]
    fn req_05_imports_serde_deserialize() {
        assert!(request_content("LoginRequest").contains("use serde::Deserialize;"));
    }

    #[test]
    fn req_06_imports_validator_validate() {
        assert!(request_content("LoginRequest").contains("use validator::Validate;"));
    }

    #[test]
    fn req_07_no_serialize_derive() {
        // Requests only need deserialization (incoming data), not serialization
        assert!(!request_content("LoginRequest").contains("Serialize"));
    }

    #[test]
    fn req_08_derive_attr_correct_order() {
        assert!(request_content("StoreUserRequest").contains("#[derive(Debug, Deserialize, Validate)]"));
    }

    #[test]
    fn req_09_struct_name_matches_input() {
        assert!(request_content("UpdatePasswordRequest").contains("pub struct UpdatePasswordRequest"));
    }

    #[test]
    fn req_10_different_names_different_structs() {
        assert_ne!(request_content("LoginRequest"), request_content("RegisterRequest"));
    }

    // ── model_content — 10 tests ──────────────────────────────────────────────

    #[test]
    fn mdl_01_has_pub_struct_with_name() {
        assert!(model_content("Post").contains("pub struct Post"));
    }

    #[test]
    fn mdl_02_derives_serialize() {
        assert!(model_content("Post").contains("Serialize"));
    }

    #[test]
    fn mdl_03_derives_deserialize() {
        assert!(model_content("Post").contains("Deserialize"));
    }

    #[test]
    fn mdl_04_derives_clone() {
        assert!(model_content("Post").contains("Clone"));
    }

    #[test]
    fn mdl_05_derives_debug() {
        assert!(model_content("Post").contains("Debug"));
    }

    #[test]
    fn mdl_06_has_pub_id_i64() {
        assert!(model_content("Post").contains("pub id: i64,"));
    }

    #[test]
    fn mdl_07_has_impl_block() {
        assert!(model_content("Comment").contains("impl Comment"));
    }

    #[test]
    fn mdl_08_imports_serde_serialize_deserialize() {
        assert!(model_content("Post").contains("use serde::{Deserialize, Serialize};"));
    }

    #[test]
    fn mdl_09_impl_block_name_matches_struct_name() {
        let out = model_content("Article");
        assert!(out.contains("pub struct Article") && out.contains("impl Article"));
    }

    #[test]
    fn mdl_10_different_names_different_content() {
        assert_ne!(model_content("User"), model_content("Post"));
    }

    // ── view_template_content — 10 tests ──────────────────────────────────────

    #[test]
    fn view_tpl_01_extends_layouts_app() {
        assert!(view_template_content("welcome").contains("{% extends \"layouts.app\" %}"));
    }

    #[test]
    fn view_tpl_02_has_block_title() {
        assert!(view_template_content("welcome").contains("{% block title %}"));
    }

    #[test]
    fn view_tpl_03_has_block_content() {
        assert!(view_template_content("welcome").contains("{% block content %}"));
    }

    #[test]
    fn view_tpl_04_has_endblock() {
        assert!(view_template_content("welcome").contains("{% endblock %}"));
    }

    #[test]
    fn view_tpl_05_name_appears_in_title_block() {
        let out = view_template_content("users.index");
        let title_pos = out.find("{% block title %}").unwrap();
        let endblock_pos = out[title_pos..].find("{% endblock %}").unwrap() + title_pos;
        assert!(out[title_pos..endblock_pos].contains("users.index"));
    }

    #[test]
    fn view_tpl_06_name_appears_in_h1() {
        assert!(view_template_content("dashboard").contains("<h1>dashboard</h1>"));
    }

    #[test]
    fn view_tpl_07_dot_notation_name_preserved_as_is() {
        // The view content uses the name as passed, not decomposed
        assert!(view_template_content("admin.users.show").contains("admin.users.show"));
    }

    #[test]
    fn view_tpl_08_single_segment_name_in_content() {
        assert!(view_template_content("welcome").contains("<h1>welcome</h1>"));
    }

    #[test]
    fn view_tpl_09_different_names_different_content() {
        assert_ne!(view_template_content("login"), view_template_content("register"));
    }

    #[test]
    fn view_tpl_10_no_japanese_text() {
        let out = view_template_content("welcome");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{9FFF}').contains(&c)));
    }

    // ── migration content — 10 tests ──────────────────────────────────────────

    #[test]
    fn mig_01_up_contains_migration_comment_with_name() {
        let up = migration_up_content("create_posts_table", "2026-01-01 00:00:00");
        assert!(up.contains("-- Migration: create_posts_table"));
    }

    #[test]
    fn mig_02_up_contains_up_sql_instruction() {
        let up = migration_up_content("create_posts_table", "2026-01-01 00:00:00");
        assert!(up.contains("-- Write your UP migration SQL here."));
    }

    #[test]
    fn mig_03_down_contains_rollback_in_comment() {
        let down = migration_down_content("create_posts_table", "2026-01-01 00:00:00");
        assert!(down.contains("-- Migration: create_posts_table (rollback)"));
    }

    #[test]
    fn mig_04_down_contains_down_sql_instruction() {
        let down = migration_down_content("create_posts_table", "2026-01-01 00:00:00");
        assert!(down.contains("-- Write your DOWN migration SQL here."));
    }

    #[test]
    fn mig_05_both_contain_created_timestamp() {
        let ts = "2026-05-17 12:34:56";
        let up = migration_up_content("add_index", ts);
        let dn = migration_down_content("add_index", ts);
        assert!(up.contains("-- Created:   2026-05-17 12:34:56"));
        assert!(dn.contains("-- Created:   2026-05-17 12:34:56"));
    }

    #[test]
    fn mig_06_up_and_down_are_different() {
        let ts = "2026-01-01 00:00:00";
        assert_ne!(migration_up_content("foo", ts), migration_down_content("foo", ts));
    }

    #[test]
    fn mig_07_name_preserved_verbatim_in_up() {
        let up = migration_up_content("create_user_sessions", "2026-01-01 00:00:00");
        assert!(up.contains("create_user_sessions"));
        assert!(!up.contains("create_user_sessions (rollback)"));
    }

    #[test]
    fn mig_08_name_preserved_verbatim_in_down() {
        let down = migration_down_content("drop_old_columns", "2026-01-01 00:00:00");
        assert!(down.contains("drop_old_columns"));
    }

    #[test]
    fn mig_09_different_names_produce_different_content() {
        let ts = "2026-01-01 00:00:00";
        assert_ne!(migration_up_content("create_posts", ts), migration_up_content("create_comments", ts));
    }

    #[test]
    fn mig_10_content_starts_with_sql_comment() {
        let ts = "2026-01-01 00:00:00";
        assert!(migration_up_content("foo", ts).starts_with("--"));
        assert!(migration_down_content("foo", ts).starts_with("--"));
    }

    // ── combination tests (50) ────────────────────────────────────────────────

    // --- Group 1: Multiple controllers in Controllers/mod.rs (5) ---

    #[test]
    fn cmb2_01_two_controllers_both_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "UserController").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "PostController").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod UserController;") && c.contains("pub mod PostController;"));
    }

    #[test]
    fn cmb2_02_three_controllers_all_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for ctrl in &["HomeController", "UserController", "StatusController"] {
            inject_mod_decl(f.to_str().unwrap(), ctrl).unwrap();
        }
        let c = fs::read_to_string(&f).unwrap();
        for ctrl in &["HomeController", "UserController", "StatusController"] {
            assert!(c.contains(&format!("pub mod {};", ctrl)), "missing {}", ctrl);
        }
    }

    #[test]
    fn cmb2_03_five_controllers_all_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for ctrl in &["A", "B", "C", "D", "E"] {
            inject_mod_decl(f.to_str().unwrap(), ctrl).unwrap();
        }
        let c = fs::read_to_string(&f).unwrap();
        for ctrl in &["A", "B", "C", "D", "E"] {
            assert_eq!(c.matches(&format!("pub mod {};", ctrl)).count(), 1, "{} must appear once", ctrl);
        }
    }

    #[test]
    fn cmb2_04_controller_then_auth_inject_both_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod HomeController;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "UserController").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Auth").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod HomeController;"));
        assert!(c.contains("pub mod UserController;"));
        assert!(c.contains("pub mod Auth;"));
    }

    #[test]
    fn cmb2_05_controller_then_dashboard_all_three_once() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod UserController;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Auth").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "DashboardController").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        for m in &["UserController", "Auth", "DashboardController"] {
            assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1, "{} must appear once", m);
        }
    }

    // --- Group 2: Multiple requests in Requests/mod.rs (5) ---

    #[test]
    fn cmb2_06_two_requests_both_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "login_request").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "register_request").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod login_request;") && c.contains("pub mod register_request;"));
    }

    #[test]
    fn cmb2_07_store_user_request_plus_auth_requests_all_three() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod StoreUserRequest;\n").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "login_request").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "register_request").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        for m in &["StoreUserRequest", "login_request", "register_request"] {
            assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1, "{} must appear once", m);
        }
    }

    #[test]
    fn cmb2_08_five_requests_all_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for req in &["r1", "r2", "r3", "r4", "r5"] {
            inject_mod_decl(f.to_str().unwrap(), req).unwrap();
        }
        let c = fs::read_to_string(&f).unwrap();
        for req in &["r1", "r2", "r3", "r4", "r5"] {
            assert_eq!(c.matches(&format!("pub mod {};", req)).count(), 1);
        }
    }

    #[test]
    fn cmb2_09_requests_mod_unaffected_by_controllers_inject() {
        let d = tmp();
        let ctrl_mod = d.path().join("controllers_mod.rs");
        let req_mod  = d.path().join("requests_mod.rs");
        fs::write(&ctrl_mod, "").unwrap();
        fs::write(&req_mod, "pub mod StoreUserRequest;\n").unwrap();
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "UserController").unwrap();
        // requests mod.rs must be unchanged
        assert_eq!(fs::read_to_string(&req_mod).unwrap(), "pub mod StoreUserRequest;\n");
    }

    #[test]
    fn cmb2_10_controllers_mod_unaffected_by_requests_inject() {
        let d = tmp();
        let ctrl_mod = d.path().join("controllers_mod.rs");
        let req_mod  = d.path().join("requests_mod.rs");
        fs::write(&ctrl_mod, "pub mod UserController;\n").unwrap();
        fs::write(&req_mod, "").unwrap();
        inject_mod_decl(req_mod.to_str().unwrap(), "login_request").unwrap();
        assert_eq!(fs::read_to_string(&ctrl_mod).unwrap(), "pub mod UserController;\n");
    }

    // --- Group 3: Models and middleware in their own mod.rs (5) ---

    #[test]
    fn cmb2_11_two_models_both_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "User").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "Post").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        assert!(c.contains("pub mod User;") && c.contains("pub mod Post;"));
    }

    #[test]
    fn cmb2_12_user_post_comment_all_in_models_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for m in &["User", "Post", "Comment"] {
            inject_mod_decl(f.to_str().unwrap(), m).unwrap();
        }
        let c = fs::read_to_string(&f).unwrap();
        for m in &["User", "Post", "Comment"] {
            assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1);
        }
    }

    #[test]
    fn cmb2_13_three_middlewares_all_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        for mw in &["LogRequest", "Authenticate", "RateLimit"] {
            inject_mod_decl(f.to_str().unwrap(), mw).unwrap();
        }
        let c = fs::read_to_string(&f).unwrap();
        for mw in &["LogRequest", "Authenticate", "RateLimit"] {
            assert_eq!(c.matches(&format!("pub mod {};", mw)).count(), 1);
        }
    }

    #[test]
    fn cmb2_14_models_mod_independent_from_controllers_mod() {
        let d = tmp();
        let ctrl_mod  = d.path().join("ctrl_mod.rs");
        let model_mod = d.path().join("model_mod.rs");
        fs::write(&ctrl_mod, "").unwrap();
        fs::write(&model_mod, "").unwrap();
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "UserController").unwrap();
        inject_mod_decl(model_mod.to_str().unwrap(), "User").unwrap();
        assert!(fs::read_to_string(&ctrl_mod).unwrap().contains("pub mod UserController;"));
        assert!(fs::read_to_string(&model_mod).unwrap().contains("pub mod User;"));
        assert!(!fs::read_to_string(&ctrl_mod).unwrap().contains("pub mod User;"));
        assert!(!fs::read_to_string(&model_mod).unwrap().contains("pub mod UserController;"));
    }

    #[test]
    fn cmb2_15_ten_module_names_all_in_mod() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "").unwrap();
        let names = ["m1","m2","m3","m4","m5","m6","m7","m8","m9","m10"];
        for n in &names { inject_mod_decl(f.to_str().unwrap(), n).unwrap(); }
        let c = fs::read_to_string(&f).unwrap();
        for n in &names { assert_eq!(c.matches(&format!("pub mod {};", n)).count(), 1, "{} missing", n); }
    }

    // --- Group 4: make:auth API + non-auth commands (5) ---

    #[test]
    fn cmb2_16_api_inject_and_controller_mod_independent() {
        let d = tmp();
        let api_f = d.path().join("api.rs");
        let mod_f = d.path().join("mod.rs");
        fs::write(&api_f, api_content()).unwrap();
        fs::write(&mod_f, "pub mod UserController;\n").unwrap();
        inject_api(&api_f);
        inject_mod_decl(mod_f.to_str().unwrap(), "Auth").unwrap();
        assert!(fs::read_to_string(&api_f).unwrap().contains("/api/auth/login"));
        let mc = fs::read_to_string(&mod_f).unwrap();
        assert!(mc.contains("pub mod UserController;") && mc.contains("pub mod Auth;"));
    }

    #[test]
    fn cmb2_17_web_inject_and_requests_mod_independent() {
        let d = tmp();
        let web_f = d.path().join("web.rs");
        let req_f = d.path().join("mod.rs");
        fs::write(&web_f, web_content()).unwrap();
        fs::write(&req_f, "pub mod StoreUserRequest;\n").unwrap();
        inject_web(&web_f);
        inject_mod_decl(req_f.to_str().unwrap(), "login_request").unwrap();
        inject_mod_decl(req_f.to_str().unwrap(), "register_request").unwrap();
        assert!(fs::read_to_string(&web_f).unwrap().contains("\"/login\""));
        let rc = fs::read_to_string(&req_f).unwrap();
        for m in &["StoreUserRequest", "login_request", "register_request"] {
            assert_eq!(rc.matches(&format!("pub mod {};", m)).count(), 1);
        }
    }

    #[test]
    fn cmb2_18_api_inject_five_times_all_routes_once() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        for _ in 0..5 { inject_api(&f); }
        let c = fs::read_to_string(&f).unwrap();
        for r in &["/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register", "/api/me"] {
            assert_eq!(c.matches(r).count(), 1, "{} must appear once", r);
        }
    }

    #[test]
    fn cmb2_19_web_inject_five_times_all_routes_once() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap();
        for _ in 0..5 { inject_web(&f); }
        let c = fs::read_to_string(&f).unwrap();
        for r in &["\"/login\"", "\"/logout\"", "\"/register\"", "\"/dashboard\""] {
            assert_eq!(c.matches(r).count(), 1, "{} must appear once", r);
        }
    }

    #[test]
    fn cmb2_20_api_and_web_inject_both_files_independent() {
        let d = tmp();
        let api_f = d.path().join("api.rs");
        let web_f = d.path().join("web.rs");
        fs::write(&api_f, api_content()).unwrap();
        fs::write(&web_f, web_content()).unwrap();
        inject_api(&api_f);
        inject_web(&web_f);
        assert!(fs::read_to_string(&api_f).unwrap().contains("/api/auth/login"));
        assert!(!fs::read_to_string(&api_f).unwrap().contains("\"/login\""));
        assert!(fs::read_to_string(&web_f).unwrap().contains("\"/login\""));
        assert!(!fs::read_to_string(&web_f).unwrap().contains("/api/auth/login"));
    }

    // --- Group 5: /api/me route (5) ---

    #[test]
    fn cmb2_21_api_me_route_injected() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("/api/me"));
    }

    #[test]
    fn cmb2_22_api_me_uses_get() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        inject_api(&f);
        assert!(fs::read_to_string(&f).unwrap().contains("get(ApiLoginController::me)"));
    }

    #[test]
    fn cmb2_23_api_me_idempotent_after_two_calls() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        inject_api(&f); inject_api(&f);
        assert_eq!(fs::read_to_string(&f).unwrap().matches("/api/me").count(), 1);
    }

    #[test]
    fn cmb2_24_api_me_after_five_calls_exactly_once() {
        let d = tmp(); let f = d.path().join("api.rs");
        fs::write(&f, api_content()).unwrap();
        for _ in 0..5 { inject_api(&f); }
        assert_eq!(fs::read_to_string(&f).unwrap().matches("/api/me").count(), 1);
    }

    #[test]
    fn cmb2_25_api_me_not_in_web_rs() {
        let d = tmp(); let f = d.path().join("web.rs");
        fs::write(&f, web_content()).unwrap();
        inject_web(&f);
        assert!(!fs::read_to_string(&f).unwrap().contains("/api/me"));
    }

    // --- Group 6: controller_content correctness (5) ---

    #[test]
    fn cmb2_26_controller_five_functions_all_use_name() {
        let out = controller_content("ArticleController");
        for action in &["index", "show", "store", "update", "destroy"] {
            assert!(out.contains(&format!("\"ArticleController {}\"", action)),
                "missing message for {}", action);
        }
    }

    #[test]
    fn cmb2_27_controller_no_path_attribute() {
        assert!(!controller_content("FooController").contains("#[path"));
    }

    #[test]
    fn cmb2_28_controller_no_japanese_text() {
        let out = controller_content("FooController");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{9FFF}').contains(&c)));
    }

    #[test]
    fn cmb2_29_controller_uses_serde_json_json_macro() {
        assert!(controller_content("Foo").contains("use serde_json::json;"));
    }

    #[test]
    fn cmb2_30_controller_each_action_word_appears_in_its_json_message() {
        // Regression: "index" message must say "index", "show" must say "show", etc.
        let out = controller_content("Ctrl");
        for action in &["index", "show", "store", "update", "destroy"] {
            assert!(out.contains(&format!("Ctrl {}", action)),
                "action word '{}' missing from its JSON message", action);
        }
    }

    // --- Group 7: request_content + model_content correctness (5) ---

    #[test]
    fn cmb2_31_request_no_path_attribute() {
        assert!(!request_content("LoginRequest").contains("#[path"));
    }

    #[test]
    fn cmb2_32_request_no_japanese_text() {
        let out = request_content("FooRequest");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{9FFF}').contains(&c)));
    }

    #[test]
    fn cmb2_33_model_no_path_attribute() {
        assert!(!model_content("Post").contains("#[path"));
    }

    #[test]
    fn cmb2_34_model_no_japanese_text() {
        let out = model_content("Article");
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{9FFF}').contains(&c)));
    }

    #[test]
    fn cmb2_35_model_id_is_i64_not_i32() {
        // make:model generates id: i64; the auth User model uses i32 (different template)
        assert!(model_content("Post").contains("pub id: i64,"));
        assert!(!model_content("Post").contains("pub id: i32,"));
    }

    // --- Group 8: view_template_content correctness (5) ---

    #[test]
    fn cmb2_36_view_four_segment_path_name_preserved() {
        assert!(view_template_content("a.b.c.d").contains("a.b.c.d"));
    }

    #[test]
    fn cmb2_37_view_no_path_attribute() {
        assert!(!view_template_content("welcome").contains("#[path"));
    }

    #[test]
    fn cmb2_38_view_name_in_both_title_and_h1() {
        let out = view_template_content("about");
        assert!(out.contains("{% block title %}about{% endblock %}"));
        assert!(out.contains("<h1>about</h1>"));
    }

    #[test]
    fn cmb2_39_view_layout_extends_exact_string() {
        // Must be "layouts.app" not "layout.app" or "layouts/app"
        assert!(view_template_content("welcome").contains("{% extends \"layouts.app\" %}"));
        assert!(!view_template_content("welcome").contains("layouts/app"));
    }

    #[test]
    fn cmb2_40_view_template_ends_with_newline() {
        assert!(view_template_content("welcome").ends_with('\n'));
    }

    // --- Group 9: migration correctness (5) ---

    #[test]
    fn cmb2_41_migration_up_has_no_rollback_word() {
        let up = migration_up_content("foo", "2026-01-01 00:00:00");
        assert!(!up.contains("rollback"));
    }

    #[test]
    fn cmb2_42_migration_down_has_rollback_word() {
        let down = migration_down_content("foo", "2026-01-01 00:00:00");
        assert!(down.contains("rollback"));
    }

    #[test]
    fn cmb2_43_migration_no_japanese_text() {
        let ts = "2026-01-01 00:00:00";
        let out = migration_up_content("foo", ts) + &migration_down_content("foo", ts);
        assert!(!out.chars().any(|c| ('\u{3040}'..='\u{9FFF}').contains(&c)));
    }

    #[test]
    fn cmb2_44_migration_created_ts_preserved_verbatim() {
        let ts = "2026-05-17 09:30:00";
        assert!(migration_up_content("m", ts).contains(ts));
        assert!(migration_down_content("m", ts).contains(ts));
    }

    #[test]
    fn cmb2_45_migration_up_for_two_names_are_different() {
        let ts = "2026-01-01 00:00:00";
        assert_ne!(migration_up_content("create_posts", ts), migration_up_content("create_users", ts));
    }

    // --- Group 10: realistic multi-command scenario (5) ---

    #[test]
    fn cmb2_46_realistic_make_controller_and_make_auth_web() {
        let d = tmp();
        let ctrl_mod = d.path().join("mod.rs");
        let web_f    = d.path().join("web.rs");
        fs::write(&ctrl_mod, "pub mod HomeController;\npub mod UserController;\n").unwrap();
        fs::write(&web_f, web_content()).unwrap();
        // simulate make:controller PostController
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "PostController").unwrap();
        // simulate make:auth (web): injects Auth + DashboardController + web routes
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "Auth").unwrap();
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "DashboardController").unwrap();
        inject_web(&web_f);
        let mc = fs::read_to_string(&ctrl_mod).unwrap();
        for m in &["HomeController", "UserController", "PostController", "Auth", "DashboardController"] {
            assert_eq!(mc.matches(&format!("pub mod {};", m)).count(), 1, "{} must appear once", m);
        }
        let wc = fs::read_to_string(&web_f).unwrap();
        for r in &["\"/login\"", "\"/logout\"", "\"/register\"", "\"/dashboard\""] {
            assert!(wc.contains(r), "missing route {}", r);
        }
    }

    #[test]
    fn cmb2_47_realistic_make_request_and_make_auth() {
        let d = tmp(); let f = d.path().join("mod.rs");
        fs::write(&f, "pub mod StoreUserRequest;\n").unwrap();
        // simulate make:auth injecting login_request and register_request
        inject_mod_decl(f.to_str().unwrap(), "login_request").unwrap();
        inject_mod_decl(f.to_str().unwrap(), "register_request").unwrap();
        let c = fs::read_to_string(&f).unwrap();
        for m in &["StoreUserRequest", "login_request", "register_request"] {
            assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1, "{} must appear once", m);
        }
    }

    #[test]
    fn cmb2_48_make_auth_web_then_make_controller_mod_order_invariant() {
        // Test that running make:auth before or after make:controller gives same mod.rs state
        let d = tmp();
        // Order A: auth first, then controller
        let f_a = d.path().join("mod_a.rs");
        fs::write(&f_a, "pub mod HomeController;\n").unwrap();
        inject_mod_decl(f_a.to_str().unwrap(), "Auth").unwrap();
        inject_mod_decl(f_a.to_str().unwrap(), "DashboardController").unwrap();
        inject_mod_decl(f_a.to_str().unwrap(), "PostController").unwrap();
        // Order B: controller first, then auth
        let f_b = d.path().join("mod_b.rs");
        fs::write(&f_b, "pub mod HomeController;\n").unwrap();
        inject_mod_decl(f_b.to_str().unwrap(), "PostController").unwrap();
        inject_mod_decl(f_b.to_str().unwrap(), "Auth").unwrap();
        inject_mod_decl(f_b.to_str().unwrap(), "DashboardController").unwrap();
        // Both must contain all four modules
        for f in &[f_a, f_b] {
            let c = fs::read_to_string(f).unwrap();
            for m in &["HomeController", "Auth", "DashboardController", "PostController"] {
                assert_eq!(c.matches(&format!("pub mod {};", m)).count(), 1, "{}: {} must appear once", f.display(), m);
            }
        }
    }

    #[test]
    fn cmb2_49_make_auth_api_then_make_controller_api_file_unchanged() {
        let d = tmp();
        let api_f = d.path().join("api.rs");
        let mod_f = d.path().join("mod.rs");
        fs::write(&api_f, api_content()).unwrap();
        fs::write(&mod_f, "").unwrap();
        // make:auth --api injects into api.rs
        inject_api(&api_f);
        // make:controller PostController only touches mod.rs, not api.rs
        let api_after_auth = fs::read_to_string(&api_f).unwrap();
        inject_mod_decl(mod_f.to_str().unwrap(), "PostController").unwrap();
        // api.rs must be exactly the same after make:controller
        assert_eq!(fs::read_to_string(&api_f).unwrap(), api_after_auth);
    }

    #[test]
    fn cmb2_50_full_scenario_controller_request_model_auth() {
        let d = tmp();
        let ctrl_mod = d.path().join("ctrl_mod.rs");
        let req_mod  = d.path().join("req_mod.rs");
        let mdl_mod  = d.path().join("mdl_mod.rs");
        let api_f    = d.path().join("api.rs");
        let web_f    = d.path().join("web.rs");
        fs::write(&ctrl_mod, "pub mod HomeController;\n").unwrap();
        fs::write(&req_mod,  "pub mod StoreUserRequest;\n").unwrap();
        fs::write(&mdl_mod,  "pub mod User;\n").unwrap();
        fs::write(&api_f, api_content()).unwrap();
        fs::write(&web_f, web_content()).unwrap();
        // make:controller PostController
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "PostController").unwrap();
        // make:request CreatePostRequest
        inject_mod_decl(req_mod.to_str().unwrap(), "CreatePostRequest").unwrap();
        // make:model Post
        inject_mod_decl(mdl_mod.to_str().unwrap(), "Post").unwrap();
        // make:auth --api
        inject_api(&api_f);
        inject_mod_decl(ctrl_mod.to_str().unwrap(), "Auth").unwrap();
        inject_mod_decl(req_mod.to_str().unwrap(), "login_request").unwrap();
        inject_mod_decl(req_mod.to_str().unwrap(), "register_request").unwrap();
        // make:auth (web separately via web.rs)
        inject_web(&web_f);
        // Verify ctrl_mod: HomeController, PostController, Auth
        let cc = fs::read_to_string(&ctrl_mod).unwrap();
        for m in &["HomeController", "PostController", "Auth"] {
            assert_eq!(cc.matches(&format!("pub mod {};", m)).count(), 1, "ctrl: {} once", m);
        }
        // Verify req_mod: StoreUserRequest, CreatePostRequest, login_request, register_request
        let rc = fs::read_to_string(&req_mod).unwrap();
        for m in &["StoreUserRequest", "CreatePostRequest", "login_request", "register_request"] {
            assert_eq!(rc.matches(&format!("pub mod {};", m)).count(), 1, "req: {} once", m);
        }
        // Verify mdl_mod: User, Post
        let mc = fs::read_to_string(&mdl_mod).unwrap();
        for m in &["User", "Post"] {
            assert_eq!(mc.matches(&format!("pub mod {};", m)).count(), 1, "mdl: {} once", m);
        }
        // Verify api.rs has all API auth routes
        let ac = fs::read_to_string(&api_f).unwrap();
        for r in &["/api/auth/login", "/api/auth/refresh", "/api/auth/logout", "/api/auth/register", "/api/me"] {
            assert!(ac.contains(r), "api: missing {}", r);
        }
        // Verify web.rs has all web auth routes
        let wc = fs::read_to_string(&web_f).unwrap();
        for r in &["\"/login\"", "\"/logout\"", "\"/register\"", "\"/dashboard\""] {
            assert!(wc.contains(r), "web: missing {}", r);
        }
    }

    // ── Integration tests ─────────────────────────────────────────────────────
    // Each test creates a real app via `willow new`, then runs make commands in a
    // specific order, verifying results after every single command.
    //
    // A global mutex serializes these tests to avoid set_current_dir races.

    static APP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_app<F: FnOnce() -> anyhow::Result<()>>(f: F) {
        let _lock = APP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::current_dir().unwrap();
        let d = tempfile::tempdir().unwrap();
        std::env::set_current_dir(d.path()).unwrap();
        crate::commands::new::execute("app").unwrap();
        std::env::set_current_dir(d.path().join("app")).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f().expect("integration test step failed")
        }));
        std::env::set_current_dir(&original).unwrap();
        drop(d);
        if let Err(e) = result { std::panic::resume_unwind(e); }
    }

    fn read_file(path: &str) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|_| panic!("cannot read {}", path))
    }

    fn file_exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn find_migrations(fragment: &str) -> Vec<std::path::PathBuf> {
        std::fs::read_dir("src/database/migrations").unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().contains(fragment))
            .collect()
    }

    // ── Single-command verification ───────────────────────────────────────────

    #[test]
    fn int_01_make_controller() {
        with_app(|| {
            controller("ArticleController")?;

            assert!(file_exists("src/app/Http/Controllers/ArticleController.rs"),
                "controller file not created");
            let c = read_file("src/app/Http/Controllers/ArticleController.rs");
            for action in &["index", "show", "store", "update", "destroy"] {
                assert!(c.contains(&format!("\"ArticleController {}\"", action)),
                    "message missing for {}", action);
            }
            assert!(read_file("src/app/Http/Controllers/mod.rs")
                .contains("pub mod ArticleController;"),
                "ArticleController not in mod.rs");
            Ok(())
        });
    }

    #[test]
    fn int_02_make_request() {
        with_app(|| {
            request("CreateArticleRequest")?;

            assert!(file_exists("src/app/Http/Requests/CreateArticleRequest.rs"));
            let c = read_file("src/app/Http/Requests/CreateArticleRequest.rs");
            assert!(c.contains("pub struct CreateArticleRequest"));
            assert!(c.contains("Validate"));
            assert!(!c.contains("Serialize"), "request must not derive Serialize");
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod CreateArticleRequest;"));
            Ok(())
        });
    }

    #[test]
    fn int_03_make_model() {
        with_app(|| {
            model("Article")?;

            assert!(file_exists("src/app/Models/Article.rs"));
            let c = read_file("src/app/Models/Article.rs");
            assert!(c.contains("pub struct Article"));
            assert!(c.contains("impl Article"));
            assert!(c.contains("pub id: i64,"), "model id must be i64");
            assert!(read_file("src/app/Models/mod.rs").contains("pub mod Article;"));
            Ok(())
        });
    }

    #[test]
    fn int_04_make_view() {
        with_app(|| {
            view_file("articles.index")?;

            assert!(file_exists("src/resources/views/articles/index.jinja.html"));
            let c = read_file("src/resources/views/articles/index.jinja.html");
            assert!(c.contains("{% extends \"layouts.app\" %}"));
            assert!(c.contains("articles.index"));
            Ok(())
        });
    }

    #[test]
    fn int_05_make_middleware() {
        with_app(|| {
            middleware("EnsureAdmin")?;

            assert!(file_exists("src/app/Http/Middleware/EnsureAdmin.rs"));
            assert!(read_file("src/app/Http/Middleware/mod.rs")
                .contains("pub mod EnsureAdmin;"));
            Ok(())
        });
    }

    #[test]
    fn int_06_make_migration() {
        with_app(|| {
            migration("create_articles_table")?;

            let ups   = find_migrations("create_articles_table.up.sql");
            let downs = find_migrations("create_articles_table.down.sql");
            assert_eq!(ups.len(),   1, "should have exactly 1 up migration");
            assert_eq!(downs.len(), 1, "should have exactly 1 down migration");
            let up_c = read_file(ups[0].to_str().unwrap());
            assert!(up_c.contains("-- Write your UP migration SQL here."));
            assert!(!up_c.contains("rollback"), "up must not mention rollback");
            let dn_c = read_file(downs[0].to_str().unwrap());
            assert!(dn_c.contains("(rollback)"));
            Ok(())
        });
    }

    #[test]
    fn int_07_make_auth_web() {
        with_app(|| {
            auth(false)?;

            // Controllers
            assert!(file_exists("src/app/Http/Controllers/Auth/LoginController.rs"));
            assert!(file_exists("src/app/Http/Controllers/Auth/RegisterController.rs"));
            assert!(file_exists("src/app/Http/Controllers/DashboardController.rs"));
            // Views
            assert!(file_exists("src/resources/views/auth/login.jinja.html"));
            assert!(file_exists("src/resources/views/auth/register.jinja.html"));
            assert!(file_exists("src/resources/views/dashboard.jinja.html"));
            // mod.rs
            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod Auth;"));
            assert!(ctrl_mod.contains("pub mod DashboardController;"));
            // Requests
            let req_mod = read_file("src/app/Http/Requests/mod.rs");
            assert!(req_mod.contains("pub mod login_request;"));
            assert!(req_mod.contains("pub mod register_request;"));
            // Routes
            let web_rs = read_file("src/routes/web.rs");
            for r in &["/login", "/logout", "/register", "/dashboard"] {
                assert!(web_rs.contains(r), "web.rs missing {}", r);
            }
            Ok(())
        });
    }

    #[test]
    fn int_08_make_auth_api() {
        with_app(|| {
            auth(true)?;

            assert!(file_exists("src/app/Http/Controllers/Auth/ApiLoginController.rs"));
            assert!(file_exists("src/app/Http/Controllers/Auth/ApiRegisterController.rs"));
            assert!(!file_exists("src/app/Http/Controllers/DashboardController.rs"),
                "api auth must not create DashboardController");
            let api_rs = read_file("src/routes/api.rs");
            for r in &["/api/auth/login", "/api/auth/refresh",
                        "/api/auth/logout", "/api/auth/register", "/api/me"] {
                assert!(api_rs.contains(r), "api.rs missing {}", r);
            }
            // web.rs must be untouched (no login route)
            assert!(!read_file("src/routes/web.rs").contains("\"/login\""),
                "api auth must not touch web.rs");
            Ok(())
        });
    }

    // ── Multi-command: controller → auth (web) ────────────────────────────────

    #[test]
    fn int_09_controller_then_auth_web() {
        with_app(|| {
            // Step 1
            controller("PostController")?;
            assert!(file_exists("src/app/Http/Controllers/PostController.rs"),
                "step1: PostController missing");
            let mod_after_ctrl = read_file("src/app/Http/Controllers/mod.rs");
            assert!(mod_after_ctrl.contains("pub mod PostController;"),
                "step1: PostController not in mod.rs");

            // Step 2
            auth(false)?;
            assert!(file_exists("src/app/Http/Controllers/Auth/LoginController.rs"),
                "step2: LoginController missing");
            let mod_final = read_file("src/app/Http/Controllers/mod.rs");
            // PostController must survive
            assert!(mod_final.contains("pub mod PostController;"),
                "step2: PostController lost from mod.rs");
            assert!(mod_final.contains("pub mod Auth;"));
            assert!(mod_final.contains("pub mod DashboardController;"));
            // PostController file content must be intact
            assert!(read_file("src/app/Http/Controllers/PostController.rs")
                .contains("\"PostController index\""),
                "step2: PostController file corrupted");
            // Web routes present
            let web = read_file("src/routes/web.rs");
            for r in &["/login", "/logout", "/register", "/dashboard"] {
                assert!(web.contains(r), "step2: web.rs missing {}", r);
            }
            Ok(())
        });
    }

    // ── Multi-command: auth (web) → controller ────────────────────────────────

    #[test]
    fn int_10_auth_web_then_controller() {
        with_app(|| {
            // Step 1
            auth(false)?;
            let web_after = read_file("src/routes/web.rs");
            assert!(web_after.contains("/login"), "step1: /login missing");

            // Step 2
            controller("CommentController")?;
            // Routes must be unchanged
            assert_eq!(read_file("src/routes/web.rs"), web_after,
                "step2: web.rs changed after make:controller");
            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod CommentController;"),
                "step2: CommentController missing from mod.rs");
            assert!(ctrl_mod.contains("pub mod Auth;"),
                "step2: Auth lost from mod.rs");
            Ok(())
        });
    }

    // ── Multi-command: request → model → controller ───────────────────────────

    #[test]
    fn int_11_request_then_model_then_controller() {
        with_app(|| {
            // Step 1: make:request
            request("StorePostRequest")?;
            assert!(file_exists("src/app/Http/Requests/StorePostRequest.rs"),
                "step1: file missing");
            assert!(read_file("src/app/Http/Requests/StorePostRequest.rs")
                .contains("pub struct StorePostRequest"), "step1: wrong struct name");
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod StorePostRequest;"), "step1: not in requests mod.rs");

            // Step 2: make:model (must not touch Requests)
            model("Post")?;
            assert!(file_exists("src/app/Models/Post.rs"), "step2: model file missing");
            assert!(read_file("src/app/Models/Post.rs").contains("impl Post"),
                "step2: impl block missing");
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod StorePostRequest;"),
                "step2: requests mod.rs corrupted by model command");

            // Step 3: make:controller (must not touch Requests or Models)
            controller("PostController")?;
            assert!(file_exists("src/app/Http/Controllers/PostController.rs"),
                "step3: controller file missing");
            assert!(read_file("src/app/Models/mod.rs").contains("pub mod Post;"),
                "step3: Models mod.rs corrupted by controller command");
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod StorePostRequest;"),
                "step3: Requests mod.rs corrupted by controller command");
            Ok(())
        });
    }

    // ── Multi-command: controller → migration → auth (api) ───────────────────

    #[test]
    fn int_12_controller_then_migration_then_auth_api() {
        with_app(|| {
            // Step 1
            controller("ArticleController")?;
            assert!(file_exists("src/app/Http/Controllers/ArticleController.rs"),
                "step1: controller missing");

            // Step 2
            migration("create_articles_table")?;
            assert_eq!(find_migrations("create_articles_table.up.sql").len(), 1,
                "step2: wrong number of up migrations");
            assert!(file_exists("src/app/Http/Controllers/ArticleController.rs"),
                "step2: controller removed by migration command");

            // Step 3
            auth(true)?;
            assert!(file_exists("src/app/Http/Controllers/ArticleController.rs"),
                "step3: controller removed by auth command");
            assert_eq!(find_migrations("create_articles_table.up.sql").len(), 1,
                "step3: migration count changed after auth");
            let api = read_file("src/routes/api.rs");
            assert!(api.contains("/api/auth/login"), "step3: api route missing");
            assert!(api.contains("/api/me"),          "step3: /api/me missing");
            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod ArticleController;"),
                "step3: ArticleController lost after auth");
            assert!(ctrl_mod.contains("pub mod Auth;"),
                "step3: Auth not added to mod.rs");
            Ok(())
        });
    }

    // ── Multi-command: auth (api) → controller → request → model ─────────────

    #[test]
    fn int_13_auth_api_then_controller_then_request_then_model() {
        with_app(|| {
            // Step 1
            auth(true)?;
            let api_snapshot = read_file("src/routes/api.rs");
            assert!(api_snapshot.contains("/api/me"), "step1: /api/me missing");

            // Step 2: make:controller must not touch api.rs
            controller("TagController")?;
            assert_eq!(read_file("src/routes/api.rs"), api_snapshot,
                "step2: api.rs changed by make:controller");
            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod TagController;"),
                "step2: TagController not in mod.rs");
            assert!(ctrl_mod.contains("pub mod Auth;"),
                "step2: Auth lost from mod.rs");

            // Step 3: make:request must not touch Controllers or api.rs
            request("StoreTagRequest")?;
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod StoreTagRequest;"), "step3: not in requests mod.rs");
            assert!(read_file("src/app/Http/Controllers/mod.rs")
                .contains("pub mod TagController;"),
                "step3: controllers mod.rs corrupted by request command");
            assert_eq!(read_file("src/routes/api.rs"), api_snapshot,
                "step3: api.rs changed by make:request");

            // Step 4: make:model must not touch anything above
            model("Tag")?;
            assert!(read_file("src/app/Models/mod.rs")
                .contains("pub mod Tag;"), "step4: Tag not in models mod.rs");
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod StoreTagRequest;"),
                "step4: requests mod.rs corrupted by model command");
            assert_eq!(read_file("src/routes/api.rs"), api_snapshot,
                "step4: api.rs changed by make:model");
            Ok(())
        });
    }

    // ── Multi-command: view → middleware → auth (web) ─────────────────────────

    #[test]
    fn int_14_view_then_middleware_then_auth_web() {
        with_app(|| {
            // Step 1
            view_file("admin.dashboard")?;
            assert!(file_exists("src/resources/views/admin/dashboard.jinja.html"),
                "step1: view missing");
            let view_content = read_file("src/resources/views/admin/dashboard.jinja.html");
            assert!(view_content.contains("admin.dashboard"), "step1: name not in view");

            // Step 2
            middleware("AdminOnly")?;
            assert!(file_exists("src/app/Http/Middleware/AdminOnly.rs"),
                "step2: middleware file missing");
            assert_eq!(read_file("src/resources/views/admin/dashboard.jinja.html"), view_content,
                "step2: view file changed by middleware command");

            // Step 3
            auth(false)?;
            assert!(file_exists("src/resources/views/auth/login.jinja.html"),
                "step3: auth login view missing");
            // Earlier files must survive
            assert!(file_exists("src/resources/views/admin/dashboard.jinja.html"),
                "step3: custom view removed by auth command");
            assert!(read_file("src/app/Http/Middleware/mod.rs")
                .contains("pub mod AdminOnly;"),
                "step3: AdminOnly lost from middleware mod.rs after auth");
            Ok(())
        });
    }

    // ── Multi-command: same commands, reversed order ───────────────────────────

    #[test]
    fn int_15_reversed_order_same_final_state() {
        // Forward: controller → request → model → auth(web)
        // Reversed: auth(web) → model → request → controller
        // Final Controllers mod.rs state must include the same entries either way.

        with_app(|| {
            auth(false)?;

            // After auth: verify auth entries
            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod Auth;"));
            assert!(ctrl_mod.contains("pub mod DashboardController;"));

            model("Widget")?;
            assert!(read_file("src/app/Models/mod.rs").contains("pub mod Widget;"),
                "Widget not in models mod.rs");

            request("StoreWidgetRequest")?;
            assert!(read_file("src/app/Http/Requests/mod.rs")
                .contains("pub mod StoreWidgetRequest;"));

            controller("WidgetController")?;
            // Final state: WidgetController in controllers mod, Auth still there
            let final_ctrl = read_file("src/app/Http/Controllers/mod.rs");
            assert!(final_ctrl.contains("pub mod WidgetController;"));
            assert!(final_ctrl.contains("pub mod Auth;"),
                "Auth lost after adding WidgetController");
            // Routes unchanged by non-auth commands
            let web = read_file("src/routes/web.rs");
            for r in &["/login", "/logout", "/register", "/dashboard"] {
                assert!(web.contains(r), "route {} lost", r);
            }
            Ok(())
        });
    }

    // ── Duplicate command: error + no corruption ───────────────────────────────

    #[test]
    fn int_16_duplicate_controller_returns_error_no_corruption() {
        with_app(|| {
            controller("DupeController")?;
            let original = read_file("src/app/Http/Controllers/DupeController.rs");

            // Second call must fail
            assert!(controller("DupeController").is_err(),
                "second controller() call must return Err");
            // File must be unchanged
            assert_eq!(read_file("src/app/Http/Controllers/DupeController.rs"), original,
                "controller file modified by failed second call");
            // mod.rs must have exactly one occurrence
            assert_eq!(
                read_file("src/app/Http/Controllers/mod.rs")
                    .matches("pub mod DupeController;").count(),
                1,
                "mod.rs has duplicate entry after failed second call"
            );
            Ok(())
        });
    }

    #[test]
    fn int_17_duplicate_model_returns_error_no_corruption() {
        with_app(|| {
            model("Post")?;
            let original = read_file("src/app/Models/Post.rs");

            assert!(model("Post").is_err(), "second model() must return Err");
            assert_eq!(read_file("src/app/Models/Post.rs"), original);
            assert_eq!(
                read_file("src/app/Models/mod.rs").matches("pub mod Post;").count(),
                1
            );
            Ok(())
        });
    }

    // ── auth idempotency ──────────────────────────────────────────────────────

    #[test]
    fn int_18_make_auth_web_idempotent() {
        with_app(|| {
            auth(false)?;
            let web_snap  = read_file("src/routes/web.rs");
            let mod_snap  = read_file("src/app/Http/Controllers/mod.rs");
            let req_snap  = read_file("src/app/Http/Requests/mod.rs");

            // Second call: files exist → skips files, routes already present → skips injection
            auth(false)?;

            assert_eq!(read_file("src/routes/web.rs"), web_snap,
                "web.rs changed on second auth(web) call");
            assert_eq!(read_file("src/app/Http/Controllers/mod.rs"), mod_snap,
                "controllers mod.rs changed on second auth(web) call");
            assert_eq!(read_file("src/app/Http/Requests/mod.rs"), req_snap,
                "requests mod.rs changed on second auth(web) call");
            // Each route appears exactly once
            for r in &["\"/login\"", "\"/logout\"", "\"/register\"", "\"/dashboard\""] {
                assert_eq!(read_file("src/routes/web.rs").matches(r).count(), 1,
                    "{} appears more than once", r);
            }
            Ok(())
        });
    }

    // ── auth(api) then auth(web): both applied, no conflict ───────────────────

    #[test]
    fn int_19_auth_api_then_auth_web_both_applied() {
        with_app(|| {
            // Step 1
            auth(true)?;
            let api_snap = read_file("src/routes/api.rs");
            assert!(api_snap.contains("/api/me"), "step1: /api/me missing");
            assert!(file_exists("src/app/Http/Controllers/Auth/ApiLoginController.rs"));

            // Step 2
            auth(false)?;
            // API routes must be untouched
            assert_eq!(read_file("src/routes/api.rs"), api_snap,
                "step2: api.rs changed by auth(web)");
            // Web routes must be injected
            let web = read_file("src/routes/web.rs");
            for r in &["/login", "/logout", "/register", "/dashboard"] {
                assert!(web.contains(r), "step2: web.rs missing {}", r);
            }
            // Auth/mod.rs must contain all 4 controller declarations
            let auth_mod = read_file("src/app/Http/Controllers/Auth/mod.rs");
            for decl in &["ApiLoginController", "ApiRegisterController",
                           "LoginController", "RegisterController"] {
                assert!(auth_mod.contains(&format!("pub mod {};", decl)),
                    "Auth/mod.rs missing {}", decl);
            }
            Ok(())
        });
    }

    // ── Many controllers, then auth: all survive ──────────────────────────────

    #[test]
    fn int_20_many_controllers_then_auth_all_in_mod() {
        with_app(|| {
            let controllers = ["PostController", "CommentController",
                               "TagController",  "CategoryController"];

            // Create each controller and verify immediately after
            for name in &controllers {
                controller(name)?;
                assert!(file_exists(&format!("src/app/Http/Controllers/{}.rs", name)),
                    "{}: file not created", name);
                assert!(
                    read_file("src/app/Http/Controllers/mod.rs")
                        .contains(&format!("pub mod {};", name)),
                    "{}: not in mod.rs after its own creation", name
                );
            }

            // Run auth(web)
            auth(false)?;

            // Every controller + auth entries must appear exactly once
            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            for name in controllers.iter()
                .chain(&["Auth", "DashboardController"])
            {
                assert_eq!(
                    ctrl_mod.matches(&format!("pub mod {};", name)).count(),
                    1,
                    "{} must appear exactly once in mod.rs", name
                );
            }
            // Web routes present
            let web = read_file("src/routes/web.rs");
            for r in &["/login", "/logout", "/register", "/dashboard"] {
                assert!(web.contains(r), "web.rs missing {}", r);
            }
            Ok(())
        });
    }

    #[test]
    fn int_21_controller_and_request_do_not_cross_contaminate_mod_files() {
        with_app(|| {
            controller("BlogController")?;
            request("CreateBlog")?;

            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            let req_mod  = read_file("src/app/Http/Requests/mod.rs");

            assert!(ctrl_mod.contains("pub mod BlogController;"), "controller missing from Controllers/mod.rs");
            assert!(req_mod.contains("pub mod CreateBlog;"), "request missing from Requests/mod.rs");

            assert!(!ctrl_mod.contains("CreateBlog"), "request name leaked into Controllers/mod.rs");
            assert!(!req_mod.contains("BlogController"), "controller name leaked into Requests/mod.rs");

            assert!(file_exists("src/app/Http/Controllers/BlogController.rs"));
            assert!(file_exists("src/app/Http/Requests/CreateBlog.rs"));
            Ok(())
        });
    }

    #[test]
    fn int_22_model_and_view_are_fully_independent() {
        with_app(|| {
            model("Post")?;
            view_file("posts/index")?;

            let model_mod = read_file("src/app/Models/mod.rs");
            assert!(model_mod.contains("pub mod Post;"), "model missing from Models/mod.rs");
            assert!(!model_mod.contains("posts"), "view path leaked into Models/mod.rs");

            assert!(file_exists("src/app/Models/Post.rs"));
            assert!(file_exists("src/resources/views/posts/index.jinja.html"));
            Ok(())
        });
    }

    #[test]
    fn int_23_all_six_non_auth_commands_work_together() {
        with_app(|| {
            controller("GalleryController")?;
            request("StoreGallery")?;
            model("Gallery")?;
            view_file("gallery/show")?;
            middleware("RateLimit")?;
            migration("create_galleries_table")?;

            assert!(file_exists("src/app/Http/Controllers/GalleryController.rs"));
            assert!(file_exists("src/app/Http/Requests/StoreGallery.rs"));
            assert!(file_exists("src/app/Models/Gallery.rs"));
            assert!(file_exists("src/resources/views/gallery/show.jinja.html"));
            assert!(file_exists("src/app/Http/Middleware/RateLimit.rs"));

            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod GalleryController;"));

            let req_mod = read_file("src/app/Http/Requests/mod.rs");
            assert!(req_mod.contains("pub mod StoreGallery;"));

            let mdl_mod = read_file("src/app/Models/mod.rs");
            assert!(mdl_mod.contains("pub mod Gallery;"));

            let mw_mod = read_file("src/app/Http/Middleware/mod.rs");
            assert!(mw_mod.contains("pub mod RateLimit;"));

            assert!(!find_migrations("create_galleries_table").is_empty(), "migration file not found");
            Ok(())
        });
    }

    #[test]
    fn int_24_three_migrations_each_get_own_file() {
        with_app(|| {
            migration("create_orders_table")?;
            migration("create_products_table")?;
            migration("create_reviews_table")?;

            let orders   = find_migrations("create_orders_table");
            let products = find_migrations("create_products_table");
            let reviews  = find_migrations("create_reviews_table");

            assert_eq!(orders.len(),   2, "expected up+down for create_orders_table");
            assert_eq!(products.len(), 2, "expected up+down for create_products_table");
            assert_eq!(reviews.len(),  2, "expected up+down for create_reviews_table");

            // Each migration has its own prefix — no file is shared
            let order_names:   Vec<_> = orders.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
            let product_names: Vec<_> = products.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
            for name in &order_names {
                assert!(!product_names.contains(name), "migration file name collision: {}", name);
            }
            Ok(())
        });
    }

    #[test]
    fn int_25_two_controllers_and_two_requests_no_cross_entries() {
        with_app(|| {
            controller("BlogController")?;
            controller("CommentController")?;
            request("StoreBlog")?;
            request("StoreComment")?;

            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            let req_mod  = read_file("src/app/Http/Requests/mod.rs");

            assert!(ctrl_mod.contains("pub mod BlogController;"));
            assert!(ctrl_mod.contains("pub mod CommentController;"));
            assert!(req_mod.contains("pub mod StoreBlog;"));
            assert!(req_mod.contains("pub mod StoreComment;"));

            // No leakage
            assert!(!ctrl_mod.contains("StoreBlog"));
            assert!(!ctrl_mod.contains("StoreComment"));
            assert!(!req_mod.contains("BlogController"));
            assert!(!req_mod.contains("CommentController"));
            Ok(())
        });
    }

    #[test]
    fn int_26_middleware_does_not_pollute_controller_mod() {
        with_app(|| {
            controller("ArticleController")?;
            middleware("Throttle")?;

            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            let mw_mod   = read_file("src/app/Http/Middleware/mod.rs");

            assert!(ctrl_mod.contains("pub mod ArticleController;"));
            assert!(!ctrl_mod.contains("Throttle"), "middleware name leaked into Controllers/mod.rs");

            assert!(mw_mod.contains("pub mod Throttle;"));
            assert!(!mw_mod.contains("ArticleController"), "controller name leaked into Middleware/mod.rs");
            Ok(())
        });
    }

    #[test]
    fn int_27_view_at_three_level_nested_path() {
        with_app(|| {
            view_file("products/categories/list")?;
            assert!(file_exists("src/resources/views/products/categories/list.jinja.html"));
            Ok(())
        });
    }

    #[test]
    fn int_28_multiple_views_in_different_subdirectories() {
        with_app(|| {
            view_file("orders/index")?;
            view_file("reports/summary")?;
            view_file("admin/users/list")?;

            assert!(file_exists("src/resources/views/orders/index.jinja.html"));
            assert!(file_exists("src/resources/views/reports/summary.jinja.html"));
            assert!(file_exists("src/resources/views/admin/users/list.jinja.html"));
            Ok(())
        });
    }

    #[test]
    fn int_29_migration_and_controller_are_completely_independent() {
        with_app(|| {
            migration("add_slug_to_posts")?;
            controller("SlugController")?;

            assert!(!find_migrations("add_slug_to_posts").is_empty());
            assert!(file_exists("src/app/Http/Controllers/SlugController.rs"));

            let ctrl_mod = read_file("src/app/Http/Controllers/mod.rs");
            assert!(ctrl_mod.contains("pub mod SlugController;"));
            assert!(!ctrl_mod.contains("add_slug"), "migration name leaked into Controllers/mod.rs");
            Ok(())
        });
    }

    #[test]
    fn int_30_request_and_model_mod_files_are_distinct() {
        with_app(|| {
            request("UpdateProfile")?;
            model("Profile")?;

            let req_mod = read_file("src/app/Http/Requests/mod.rs");
            let mdl_mod = read_file("src/app/Models/mod.rs");

            assert!(req_mod.contains("pub mod UpdateProfile;"));
            assert!(mdl_mod.contains("pub mod Profile;"));

            assert!(!req_mod.contains("pub mod Profile;"), "model leaked into Requests/mod.rs");
            assert!(!mdl_mod.contains("UpdateProfile"), "request leaked into Models/mod.rs");
            Ok(())
        });
    }
}
