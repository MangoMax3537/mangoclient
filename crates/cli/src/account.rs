//! `account` subcommands and the account stage of `launch`.
//!
//! Everything that touches tokens goes through `mango_app::AppService`
//! (vault persistence, refresh). Nothing here prints a token.

use crate::args::Args;
use crate::error::{usage, CliError, Res};
use async_trait::async_trait;
use mango_app::{
    AppError, AppService, EventSink, Host, LaunchCredentials, LoginOutcome, LoginWindow,
    PublicAccount, ServiceOptions,
};
use mango_core::Paths;
use std::io::BufRead;
use std::sync::{Arc, Mutex};

/// Events have no UI here.
struct NoSink;

impl EventSink for NoSink {
    fn emit(&self, _event: mango_app::Event) {}
}

fn service(paths: &Paths, host: Host) -> Res<AppService> {
    AppService::with_host(
        paths.clone(),
        Arc::new(NoSink),
        ServiceOptions {
            open_folders: false,
            auto_update_check: false,
            ..ServiceOptions::default()
        },
        host,
    )
    .map_err(app_err)
}

fn app_err(e: AppError) -> CliError {
    let hint = if e.needs_relogin {
        "; sign in again with `mango-cli account add-microsoft`"
    } else {
        ""
    };
    CliError::Failure(format!("{} ({}){hint}", e.message, e.code))
}

/// The sign-in "window" of a terminal: print the URL, read the pasted
/// redirect URL from the input.
pub struct PasteLogin<R> {
    input: Arc<Mutex<R>>,
}

impl<R: BufRead + Send + 'static> PasteLogin<R> {
    pub fn new(input: R) -> Self {
        Self {
            input: Arc::new(Mutex::new(input)),
        }
    }
}

#[async_trait]
impl<R: BufRead + Send + 'static> LoginWindow for PasteLogin<R> {
    async fn authorize(&self, authorize_url: &str, redirect_prefix: &str) -> LoginOutcome {
        println!("Open this URL in a browser and sign in:\n\n{authorize_url}\n");
        println!(
            "After signing in the browser lands on a blank page whose address starts with\n{redirect_prefix}\nPaste that full address here (empty line cancels):"
        );
        let input = self.input.clone();
        let line = tokio::task::spawn_blocking(move || {
            let mut line = String::new();
            let mut guard = input.lock().unwrap_or_else(|e| e.into_inner());
            guard.read_line(&mut line).map(|_| line)
        })
        .await;
        match line {
            Ok(Ok(text)) if !text.trim().is_empty() => LoginOutcome::Code(text.trim().to_string()),
            Ok(Ok(_)) => LoginOutcome::Cancelled,
            Ok(Err(e)) => LoginOutcome::Error(e.to_string()),
            Err(e) => LoginOutcome::Error(e.to_string()),
        }
    }
}

/// One line per account; credential state only, never tokens.
pub fn list_lines(svc: &AppService) -> Res<Vec<String>> {
    let st = svc.app_state().map_err(app_err)?;
    let selected = st.config.selected_account.clone();
    Ok(st
        .accounts
        .iter()
        .map(|a| account_line(a, selected.as_deref() == Some(a.uuid.as_str())))
        .collect())
}

fn account_line(a: &PublicAccount, selected: bool) -> String {
    let kind = serde_json::to_value(a.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let mut flags = Vec::new();
    if kind != "offline" {
        flags.push(if a.credentials_persisted {
            "persisted"
        } else {
            "memory-only"
        });
        if a.credentials_locked {
            flags.push("locked");
        }
        if a.needs_relogin {
            flags.push("needs-relogin");
        }
    }
    format!(
        "{}{}  {}  {}  {}",
        if selected { "* " } else { "  " },
        a.uuid,
        a.name,
        kind,
        flags.join(",")
    )
}

pub async fn run(paths: &Paths, mut args: Args) -> Res<()> {
    match args.positional().as_deref() {
        Some("add-offline") => {
            let name = args.positional();
            args.finish()?;
            let Some(name) = name else {
                return usage("usage: account add-offline <name>");
            };
            let svc = service(paths, Host::system(false))?;
            let a = svc.account_add_offline(name).map_err(app_err)?;
            println!("Added offline account {} ({})", a.name, a.uuid);
            println!("RESULT account uuid={}", a.uuid);
            Ok(())
        }
        Some("add-microsoft") => {
            args.finish()?;
            let login = Arc::new(PasteLogin::new(std::io::BufReader::new(std::io::stdin())));
            let svc = service(paths, Host::system(false).with_login(login))?;
            add_microsoft(&svc).await
        }
        Some("list") => {
            args.finish()?;
            let svc = service(paths, Host::system(false))?;
            for l in list_lines(&svc)? {
                println!("{l}");
            }
            Ok(())
        }
        _ => usage("usage: account <add-offline <name>|add-microsoft|list>"),
    }
}

pub async fn add_microsoft(svc: &AppService) -> Res<()> {
    match svc.account_sign_in_microsoft().await.map_err(app_err)? {
        None => println!("Sign-in cancelled"),
        Some(a) => {
            println!("Signed in as {} ({})", a.name, a.uuid);
            if !a.credentials_persisted {
                eprintln!("warning: no usable OS keystore; this login will not survive a restart");
            }
            println!("RESULT account uuid={}", a.uuid);
        }
    }
    Ok(())
}

/// Resolve `--account <name|uuid>` (default: the selected account), refresh
/// it when needed and return what the launch needs.
pub async fn launch_credentials(paths: &Paths, wanted: Option<&str>) -> Res<LaunchCredentials> {
    let svc = service(paths, Host::system(false))?;
    credentials_of(&svc, wanted).await
}

pub async fn credentials_of(svc: &AppService, wanted: Option<&str>) -> Res<LaunchCredentials> {
    let st = svc.app_state().map_err(app_err)?;
    let uuid = match wanted {
        Some(w) => st
            .accounts
            .iter()
            .find(|a| a.uuid == w)
            .or_else(|| st.accounts.iter().find(|a| a.name.eq_ignore_ascii_case(w)))
            .map(|a| a.uuid.clone())
            .ok_or_else(|| CliError::Failure(format!("no account named or numbered {w:?}")))?,
        None => st
            .config
            .selected_account
            .clone()
            .filter(|u| st.accounts.iter().any(|a| &a.uuid == u))
            .or_else(|| st.accounts.first().map(|a| a.uuid.clone()))
            .ok_or_else(|| {
                CliError::Failure(
                    "no account; run `account add-offline <name>` or `account add-microsoft` first"
                        .into(),
                )
            })?,
    };
    svc.launch_credentials(&uuid).await.map_err(app_err)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use mango_auth::MemoryDeviceKey;
    use std::io::Cursor;

    fn svc(dir: &std::path::Path, login: Arc<dyn LoginWindow>) -> AppService {
        let host = Host::system(false)
            .with_login(login)
            .with_device_keys(Arc::new(MemoryDeviceKey::new()));
        service(&Paths::new(dir), host).unwrap()
    }

    #[tokio::test]
    async fn paste_login_returns_the_pasted_url_or_cancels() {
        let l = PasteLogin::new(Cursor::new(
            b"  https://login.live.com/oauth20_desktop.srf?code=abc \n".to_vec(),
        ));
        assert_eq!(
            l.authorize("https://x", "https://login.live.com/oauth20_desktop.srf")
                .await,
            LoginOutcome::Code("https://login.live.com/oauth20_desktop.srf?code=abc".into())
        );
        let empty = PasteLogin::new(Cursor::new(b"\n".to_vec()));
        assert_eq!(empty.authorize("u", "p").await, LoginOutcome::Cancelled);
        let eof = PasteLogin::new(Cursor::new(Vec::new()));
        assert_eq!(eof.authorize("u", "p").await, LoginOutcome::Cancelled);
    }

    #[tokio::test]
    async fn pasted_cancel_adds_nothing_and_list_shows_credential_state() {
        let tmp = tempfile::tempdir().unwrap();
        let s = svc(
            tmp.path(),
            Arc::new(PasteLogin::new(Cursor::new(b"\n".to_vec()))),
        );
        add_microsoft(&s).await.unwrap();
        assert!(list_lines(&s).unwrap().is_empty());

        let off = s.account_add_offline("Steve_1".into()).unwrap();
        let lines = list_lines(&s).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with(&format!("* {}", off.uuid)));
        assert!(lines[0].ends_with("offline  "));
    }

    #[tokio::test]
    async fn account_selection_by_name_or_uuid() {
        let tmp = tempfile::tempdir().unwrap();
        let s = svc(
            tmp.path(),
            Arc::new(PasteLogin::new(Cursor::new(Vec::new()))),
        );
        let a = s.account_add_offline("Alpha".into()).unwrap();
        let b = s.account_add_offline("Beta".into()).unwrap();
        assert_eq!(credentials_of(&s, None).await.unwrap().uuid, a.uuid);
        assert_eq!(credentials_of(&s, Some("beta")).await.unwrap().uuid, b.uuid);
        assert_eq!(
            credentials_of(&s, Some(&a.uuid)).await.unwrap().name,
            "Alpha"
        );
        let c = credentials_of(&s, Some("Beta")).await.unwrap();
        assert!(c.offline && c.access_token() == "0");
        assert!(credentials_of(&s, Some("nobody")).await.is_err());
    }
}
