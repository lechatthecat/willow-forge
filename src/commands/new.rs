use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn execute(name: &str) -> Result<()> {
    println!("Creating new Willow Forge application: {}", name);

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

    println!("Application created successfully!");
    println!("\nNext steps:");
    println!("  cd {}", name);
    println!("  cargo run");

    Ok(())
}

fn create_directory_structure(base: &Path) -> Result<()> {
    let dirs = vec![
        "src/app/http/controllers",
        "src/app/http/middleware",
        "src/app/http/requests",
        "src/app/models",
        "src/app/exceptions",
        "src/routes",
        "src/config",
        "src/database/migrations",
        "src/database/seeders",
        "src/database/factories",
        "src/resources/views/auth",
        "src/resources/views/layouts",
        "src/resources/views/errors",
        "src/resources/views/partials",
        "src/resources/lang",
        "src/storage/logs",
        "src/storage/cache",
        "src/docker",
        "tests/Feature",
        "tests/Unit",
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

    // src/middleware.rs
    fs::write(base.join("src/middleware.rs"), app_files::bootstrap_middleware_rs(&crate_name))?;

    // src/app/
    fs::write(base.join("src/app/mod.rs"), app_files::src_app_mod_rs())?;
    fs::write(base.join("src/app/http/mod.rs"), app_files::src_app_http_mod_rs())?;
    fs::write(base.join("src/app/http/controllers/mod.rs"), app_files::src_app_http_controllers_mod_rs())?;
    fs::write(base.join("src/app/http/controllers/home_controller.rs"), app_files::home_controller(&crate_name))?;
    fs::write(base.join("src/app/http/controllers/user_controller.rs"), app_files::user_controller(&crate_name))?;
    fs::write(base.join("src/app/http/controllers/status_controller.rs"), app_files::status_controller(&crate_name))?;
    fs::write(base.join("src/app/http/middleware/mod.rs"), app_files::src_app_http_middleware_mod_rs())?;
    fs::write(base.join("src/app/http/middleware/log_request.rs"), app_files::middleware_log_request_rs())?;
    fs::write(base.join("src/app/http/requests/mod.rs"), app_files::src_app_http_requests_mod_rs())?;
    fs::write(base.join("src/app/http/requests/store_user_request.rs"), app_files::store_user_request())?;
    fs::write(base.join("src/app/models/mod.rs"), app_files::src_app_models_mod_rs())?;
    fs::write(base.join("src/app/models/user.rs"), app_files::user_model_rs())?;
    fs::write(base.join("src/app/exceptions/mod.rs"), app_files::src_app_exceptions_mod_rs())?;
    fs::write(base.join("src/app/exceptions/handler.rs"), app_files::exception_handler_rs(&crate_name))?;

    // src/routes/
    fs::write(base.join("src/routes/mod.rs"), app_files::src_routes_mod_rs())?;
    fs::write(base.join("src/routes/web.rs"), app_files::routes_web(&crate_name))?;
    fs::write(base.join("src/routes/api.rs"), app_files::routes_api(&crate_name))?;

    // src/ (library root + service provider)
    fs::write(base.join("src/lib.rs"), app_files::bootstrap_lib_rs())?;
    fs::write(base.join("src/app_service_provider.rs"), app_files::app_service_provider())?;

    // src/resources/views/
    fs::write(base.join("src/resources/views/layouts/app.jinja.html"), app_files::view_layout_app())?;
    fs::write(base.join("src/resources/views/welcome.jinja.html"), app_files::view_welcome())?;
    fs::write(base.join("src/resources/views/errors/404.jinja.html"), app_files::view_error_404_html())?;
    fs::write(base.join("src/resources/views/errors/500.jinja.html"), app_files::view_error_500_html())?;
    fs::write(base.join("src/resources/views/errors/generic.jinja.html"), app_files::view_error_generic_html())?;

    // src/database/migrations/
    fs::write(
        base.join("src/database/migrations/0001_create_users_table.up.sql"),
        app_files::initial_migration_up_sql(),
    )?;
    fs::write(
        base.join("src/database/migrations/0001_create_users_table.down.sql"),
        app_files::initial_migration_down_sql(),
    )?;

    // src/config/
    fs::write(base.join("src/config/app.toml"), app_files::config_app())?;
    fs::write(base.join("src/config/auth.toml"), app_files::config_auth())?;
    fs::write(base.join("src/config/database.toml"), app_files::config_database())?;
    fs::write(base.join("src/config/cache.toml"), app_files::config_cache())?;

    // src/docker/
    fs::write(base.join("src/docker/docker-compose.yml"), app_files::docker_compose())?;

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
