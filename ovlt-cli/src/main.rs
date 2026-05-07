mod api;
mod app;
mod components;
mod config_file;
mod events;
mod tui;
mod ui;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "ovlt",
    about = "OVLT — Developer-first Auth Service",
    version,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Launch the admin TUI
    Serve {
        /// OVLT server URL (overrides saved connection)
        #[arg(long, short, env = "OVLT_URL")]
        url: Option<String>,
    },
    /// Save a server URL for future connections
    Connect {
        /// Server URL, e.g. http://my-server:3000
        url: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Connect { url } => {
            let mut cfg = config_file::load();
            cfg.url = Some(url.clone());
            if let Err(e) = config_file::save(&cfg) {
                eprintln!("Failed to save connection: {e}");
                std::process::exit(1);
            }
            println!("Connected to {url}");
            println!("Run `ovlt serve` to open the admin TUI.");
        }

        Command::Serve { url } => {
            let cfg = config_file::load();
            let url = url
                .or(cfg.url)
                .unwrap_or_else(|| "http://localhost:3000".into());

            let client = api::Client::new(url);
            let app_state = app::App::new(client);

            if let Err(e) = tui::run(app_state).await {
                eprintln!("TUI error: {e}");
                std::process::exit(1);
            }
        }
    }
}
