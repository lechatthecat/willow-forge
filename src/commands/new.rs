use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn execute(name: &str) -> Result<()> {
    println!("🌿 Creating new Willow application: {}", name);

    let app_path = Path::new(name);

    if app_path.exists() {
        anyhow::bail!("Directory '{}' already exists", name);
    }

    // Create directory structure
    create_directory_structure(app_path)?;

    // Generate files
    generate_files(app_path, name)?;

    println!("✓ Application created successfully!");
    println!("\nNext steps:");
    println!("  cd {}", name);
    println!("  cargo build");
    println!("  cargo run -- willow serve");

    Ok(())
}

fn create_directory_structure(base: &Path) -> Result<()> {
    let dirs = vec![
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
        "resources/views",
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

fn generate_files(base: &Path, name: &str) -> Result<()> {
    use crate::templates::app_files;

    // Cargo.toml
    fs::write(
        base.join("Cargo.toml"),
        app_files::cargo_toml(name),
    )?;

    // .env
    fs::write(base.join(".env"), app_files::env_file())?;

    // main.rs
    fs::write(base.join("src/main.rs"), app_files::main_rs())?;

    // bootstrap/lib.rs (the library root)
    fs::write(base.join("bootstrap/lib.rs"), app_files::bootstrap_lib_rs())?;

    // bootstrap/app_state.rs
    fs::write(base.join("bootstrap/app_state.rs"), app_files::app_state_rs())?;

    // bootstrap/context.rs
    fs::write(base.join("bootstrap/context.rs"), app_files::context_rs())?;

    // bootstrap/validated_json.rs
    fs::write(base.join("bootstrap/validated_json.rs"), app_files::validated_json_rs())?;

    // app/Providers/AppServiceProvider.rs
    fs::write(
        base.join("app/Providers/AppServiceProvider.rs"),
        app_files::app_service_provider(),
    )?;

    // routes/api.rs
    fs::write(base.join("routes/api.rs"), app_files::routes_api())?;

    // routes/web.rs
    fs::write(base.join("routes/web.rs"), app_files::routes_web())?;

    // app/Http/Controllers/UserController.rs
    fs::write(
        base.join("app/Http/Controllers/UserController.rs"),
        app_files::user_controller(),
    )?;

    // app/Http/Requests/StoreUserRequest.rs
    fs::write(
        base.join("app/Http/Requests/StoreUserRequest.rs"),
        app_files::store_user_request(),
    )?;

    // config files
    fs::write(base.join("config/app.toml"), app_files::config_app())?;
    fs::write(base.join("config/database.toml"), app_files::config_database())?;

    // .gitignore
    fs::write(base.join(".gitignore"), app_files::gitignore())?;

    Ok(())
}
