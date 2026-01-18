mod commands;
mod templates;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "willow")]
#[command(about = "Willow Framework - Laravel-like web framework for Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a new Willow application")]
    New {
        #[arg(help = "Name of the application")]
        name: String,
    },
    #[command(about = "Start the development server")]
    Serve,
    #[command(about = "Run database migrations")]
    Migrate,
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => commands::new::execute(&name)?,
        Commands::Serve => commands::serve::execute().await?,
        Commands::Migrate => commands::migrate::execute().await?,
        Commands::MakeController { name } => commands::make::controller(&name)?,
        Commands::MakeRequest { name } => commands::make::request(&name)?,
        Commands::MakeModel { name } => commands::make::model(&name)?,
        Commands::MakeMigration { name } => commands::make::migration(&name)?,
    }

    Ok(())
}
