use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn execute(name: &str) -> Result<()> {
    println!("🌿 Creating new Willow Forge application: {}", name);

    let app_path = Path::new(name);

    if app_path.exists() {
        eprintln!();
        eprintln!("  ✗ A directory named '{}' already exists.", name);
        eprintln!();
        eprintln!("  To create a new app, either:");
        eprintln!("    • Choose a different name:  willow new my-app");
        eprintln!("    • Remove the existing directory first");
        eprintln!();
        std::process::exit(1);
    }

    create_directory_structure(app_path)?;
    generate_files(app_path, name)?;

    println!("✓ Application created successfully!");
    println!("\nNext steps:");
    println!("  cd {}", name);
    println!("  cargo run");

    Ok(())
}

fn create_directory_structure(base: &Path) -> Result<()> {
    let dirs = vec![
        "app/Exceptions",
        "app/Http/Controllers",
        "app/Http/Middleware",
        "app/Http/Requests",
        "app/Models",
        "app/Providers",
        "app/Policies",
        "bootstrap",
        "config",
        "database/migrations",
        "database/seeders",
        "database/factories",
        "routes",
        "resources/views/layouts",
        "resources/views/errors",
        "resources/views/partials",
        "resources/lang",
        "storage/logs",
        "storage/cache",
        "tests/Feature",
        "tests/Unit",
        "src",
    ];

    for dir in dirs {
        let path = base.join(dir);
        fs::create_dir_all(&path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    }

    Ok(())
}

fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

fn generate_files(base: &Path, name: &str) -> Result<()> {
    use crate::templates::app_files;

    let crate_name = normalize_crate_name(name);

    // Cargo.toml
    fs::write(base.join("Cargo.toml"), app_files::cargo_toml(name))?;

    // .env
    fs::write(base.join(".env"), app_files::env_file())?;

    // src/main.rs
    fs::write(base.join("src/main.rs"), app_files::main_rs(&crate_name))?;

    // bootstrap/
    fs::write(base.join("bootstrap/lib.rs"), app_files::bootstrap_lib_rs())?;
    fs::write(base.join("bootstrap/app_state.rs"), app_files::app_state_rs())?;
    fs::write(base.join("bootstrap/cache.rs"), app_files::cache_rs())?;
    fs::write(base.join("bootstrap/context.rs"), app_files::context_rs())?;
    fs::write(base.join("bootstrap/validated_json.rs"), app_files::validated_json_rs())?;
    fs::write(base.join("bootstrap/view.rs"), app_files::view_rs())?;
    fs::write(base.join("bootstrap/middleware.rs"), app_files::bootstrap_middleware_rs(&crate_name))?;

    // app/
    fs::write(base.join("app/errors.rs"), app_files::app_errors_rs())?;
    fs::write(base.join("app/Exceptions/Handler.rs"), app_files::exception_handler_rs(&crate_name))?;
    fs::write(base.join("app/Http/Middleware/LogRequest.rs"), app_files::middleware_log_request_rs())?;
    fs::write(base.join("app/Providers/AppServiceProvider.rs"), app_files::app_service_provider())?;
    fs::write(base.join("app/Http/Controllers/HomeController.rs"), app_files::home_controller(&crate_name))?;
    fs::write(base.join("app/Http/Controllers/UserController.rs"), app_files::user_controller(&crate_name))?;
    fs::write(base.join("app/Http/Controllers/StatusController.rs"), app_files::status_controller(&crate_name))?;
    fs::write(base.join("app/Http/Requests/StoreUserRequest.rs"), app_files::store_user_request())?;

    // routes/
    fs::write(base.join("routes/web.rs"), app_files::routes_web(&crate_name))?;
    fs::write(base.join("routes/api.rs"), app_files::routes_api(&crate_name))?;

    // resources/views/
    fs::write(base.join("resources/views/layouts/app.jinja.html"), app_files::view_layout_app())?;
    fs::write(base.join("resources/views/welcome.jinja.html"), app_files::view_welcome())?;
    fs::write(base.join("resources/views/errors/404.jinja.html"), app_files::view_error_404_html())?;
    fs::write(base.join("resources/views/errors/500.jinja.html"), app_files::view_error_500_html())?;

    // app/Models/
    fs::write(base.join("app/Models/User.rs"), app_files::user_model_rs())?;

    // database/migrations/
    fs::write(
        base.join("database/migrations/0001_create_users_table.up.sql"),
        app_files::initial_migration_up_sql(),
    )?;
    fs::write(
        base.join("database/migrations/0001_create_users_table.down.sql"),
        app_files::initial_migration_down_sql(),
    )?;

    // config/
    fs::write(base.join("config/app.toml"), app_files::config_app())?;
    fs::write(base.join("config/database.toml"), app_files::config_database())?;
    fs::write(base.join("config/cache.toml"), app_files::config_cache())?;

    // .gitignore
    fs::write(base.join(".gitignore"), app_files::gitignore())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_crate_name;

    #[test]
    fn hyphen_becomes_underscore() {
        assert_eq!(normalize_crate_name("my-app"), "my_app");
    }

    #[test]
    fn already_normalized_unchanged() {
        assert_eq!(normalize_crate_name("my_app"), "my_app");
    }

    #[test]
    fn no_hyphens_unchanged() {
        assert_eq!(normalize_crate_name("hello"), "hello");
    }

    #[test]
    fn multiple_hyphens() {
        assert_eq!(normalize_crate_name("a-b-c"), "a_b_c");
    }
}
