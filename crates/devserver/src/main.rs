//! `mango-dev`: see the library docs. Usage:
//!
//! ```text
//! MANGOCLIENT_HOME=/path/to/scratch/dir mango-dev [--port N] [--allow-origin URL]...
//! mango-dev --home DIR [--port N] [--allow-origin URL]... [--background]
//!           [--i-know-this-uses-real-data]
//! ```

use mango_app::ServiceOptions;
use mango_core::Paths;
use mango_devserver::{start, validate_origin, ServerConfig};
use std::path::{Path, PathBuf};

const USAGE: &str = "\
mango-dev: localhost HTTP/SSE bridge to the MangoClient service (development only)

  --home <dir>                  data dir (or set MANGOCLIENT_HOME); REQUIRED
  --port <n>                    listen port on 127.0.0.1 (default: random)
  --allow-origin <origin>       exact CORS origin, repeatable
                                (default: http://127.0.0.1:5173 and http://localhost:5173)
  --background                  also run telemetry, update checks and the mods watcher
  --i-know-this-uses-real-data  allow the real per-user data dir
";

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn parse() -> Result<ServerConfig, String> {
    let mut home: Option<PathBuf> = None;
    let mut port = 0u16;
    let mut origins: Vec<String> = Vec::new();
    let mut background = false;
    let mut real = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--home" => home = Some(PathBuf::from(value("--home")?)),
            "--port" => {
                port = value("--port")?
                    .parse()
                    .map_err(|_| "--port must be 0..65535".to_string())?;
            }
            "--allow-origin" => origins.push(validate_origin(&value("--allow-origin")?)?),
            "--background" => background = true,
            "--i-know-this-uses-real-data" => real = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    let home = home
        .or_else(|| {
            std::env::var_os("MANGOCLIENT_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        })
        .ok_or("no data dir: pass --home <dir> or set MANGOCLIENT_HOME (a scratch dir)")?;
    if !real && canonical(&home) == canonical(&Paths::platform_default_root()) {
        return Err(format!(
            "{} is the real MangoClient data dir; use a scratch dir, or pass --i-know-this-uses-real-data",
            home.display()
        ));
    }
    if origins.is_empty() {
        origins = vec![
            "http://127.0.0.1:5173".into(),
            "http://localhost:5173".into(),
        ];
    }
    Ok(ServerConfig {
        home,
        allow_origins: origins,
        token: None,
        port,
        options: ServiceOptions::default(),
        host: None,
        background,
    })
}

#[tokio::main]
async fn main() {
    let cfg = match parse() {
        Ok(c) => c,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("mango-dev: {e}\n");
            }
            eprint!("{USAGE}");
            std::process::exit(if e.is_empty() { 0 } else { 2 });
        }
    };
    let (home, origins) = (cfg.home.clone(), cfg.allow_origins.clone());
    let server = match start(cfg).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mango-dev: {e}");
            std::process::exit(1);
        }
    };
    println!("mango-dev listening on {}", server.base_url());
    println!("token: {}", server.token);
    println!("data dir: {}", home.display());
    println!("allowed origins: {}", origins.join(", "));
    let _ = tokio::signal::ctrl_c().await;
    server.shutdown().await;
}
