mod cli;
mod commands;
mod config;

use clap::Parser;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth => commands::auth::run().await?,
        Commands::Scan { by } => commands::scan::run(by).await?,
        Commands::FuzzyScan { threshold } => commands::fuzzy_scan::run(threshold).await?,
        Commands::SyncFlickr {
            directory,
            max_images,
        } => commands::sync_flickr::run(directory, max_images).await?,
        Commands::AiDedupe {
            directory,
            method,
            model,
            max_images,
            similarity_threshold,
        } => commands::ai_dedupe::run(directory, method, model, max_images, similarity_threshold)?,
        Commands::BenchmarkMethods {
            directory,
            num_images,
            model,
        } => commands::benchmark::run(directory, num_images, model)?,
    }

    Ok(())
}
