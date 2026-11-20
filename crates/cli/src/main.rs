//! mango-cli: headless launcher front end over the Mango crates.

mod account;
mod args;
mod commands;
mod error;
mod launch;

use args::Args;
use error::{usage, Res};
use mango_core::Paths;

const HELP: &str = "mango-cli: headless MangoClient

usage:
  mango-cli profile create <name> --mc <ver> [--loader vanilla|fabric|quilt|forge|neoforge] [--loader-version <v>]
  mango-cli profile list
  mango-cli account add-offline <name>
  mango-cli account add-microsoft     (prints a sign-in URL, paste the final redirect URL)
  mango-cli account list
  mango-cli launch <profile-name-or-id> [--stop-when-running] [--timeout-secs N] [--dry-run]
                   [--perf-mods] [--auto-fabric] [--quick-join host[:port]] [--account <name|uuid>]
  mango-cli versions [--snapshots]
  mango-cli java list

Data lives in $MANGOCLIENT_HOME (default: the launcher data directory).
MANGO_BUNDLE_DIR overrides the MangoConfig jar directory.";

async fn run(raw: Vec<String>) -> Res<()> {
    let mut args = Args::new(raw);
    if args.flag("--help") || args.flag("-h") {
        println!("{HELP}");
        return Ok(());
    }
    let Some(cmd) = args.positional() else {
        return usage(HELP);
    };
    let paths = Paths::from_env();
    match cmd.as_str() {
        "profile" => commands::run_profile(&paths, args),
        "account" => account::run(&paths, args).await,
        "versions" => commands::run_versions(&paths, args).await,
        "java" => commands::run_java(&paths, args).await,
        "launch" => launch::run(&paths, args).await,
        "help" => {
            println!("{HELP}");
            Ok(())
        }
        other => usage(format!("unknown command {other:?}\n\n{HELP}")),
    }
}

#[tokio::main]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(raw).await {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code());
    }
}
