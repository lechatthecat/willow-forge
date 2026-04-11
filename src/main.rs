mod commands;
mod templates;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "willow-forge")]
#[command(about = "Willow Forge - Laravel-like web framework for Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a new Willow Forge application")]
    New {
        #[arg(help = "Name of the application")]
        name: String,
    },
    #[command(about = "Run all pending migrations")]
    Migrate,
    #[command(name = "migrate:rollback", about = "Roll back the last applied migration")]
    MigrateRollback,
    #[command(name = "migrate:status", about = "Show applied and pending migrations")]
    MigrateStatus,
    #[command(name = "migrate:fresh", about = "Drop all tables and re-run all migrations")]
    MigrateFresh,
    #[command(name = "migrate:reset", about = "Roll back all applied migrations")]
    MigrateReset,
    #[command(name = "make:controller", about = "Create a new controller")]
    MakeController {
        #[arg(help = "Name of the controller")]
        name: String,
    },
    #[command(name = "make:request", about = "Create a new form request")]
    MakeRequest {
        #[arg(help = "Name of the request")]
        name: String,
    },
    #[command(name = "make:model", about = "Create a new model")]
    MakeModel {
        #[arg(help = "Name of the model")]
        name: String,
    },
    #[command(name = "make:migration", about = "Create a new migration")]
    MakeMigration {
        #[arg(help = "Name of the migration")]
        name: String,
    },
    #[command(name = "make:view", about = "Create a new view template")]
    MakeView {
        #[arg(help = "View name in dot notation (e.g. welcome, users.index)")]
        name: String,
    },
    #[command(name = "make:middleware", about = "Create a new middleware")]
    MakeMiddleware {
        #[arg(help = "Name of the middleware")]
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => commands::new::execute(&name)?,
        Commands::Migrate         => commands::migrate::execute().await?,
        Commands::MigrateRollback => commands::migrate::rollback().await?,
        Commands::MigrateStatus   => commands::migrate::status().await?,
        Commands::MigrateFresh    => commands::migrate::fresh().await?,
        Commands::MigrateReset    => commands::migrate::reset().await?,
        Commands::MakeController { name } => commands::make::controller(&name)?,
        Commands::MakeRequest { name } => commands::make::request(&name)?,
        Commands::MakeModel { name } => commands::make::model(&name)?,
        Commands::MakeMigration { name } => commands::make::migration(&name)?,
        Commands::MakeView { name } => commands::make::view_file(&name)?,
        Commands::MakeMiddleware { name } => commands::make::middleware(&name)?,
    }

    Ok(())
}
