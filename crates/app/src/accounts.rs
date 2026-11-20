//! Accounts: persistence through the hardened `Vault`, Microsoft sign-in and
//! token refresh.
//!
//! The store (`accounts.json`) only ever holds the sanitized records the vault
//! produces (no plaintext tokens, at most an encrypted refresh-token
//! envelope). Live tokens sit in `Inner::sessions`, in memory, zeroized on
//! drop. Nothing here logs or emits a token.

use crate::error::{AppError, Res};
use crate::events::{AccountsChangedEvent, Event};
use crate::host::LoginOutcome;
use crate::service::{lock, now_ms, AppService, Inner};
use crate::types::{CredState, PublicAccount};
use mango_auth::{
    needs_migration, Account as AuthAccount, AccountType as AuthKind, AuthError, Redirect,
    StoredAccount, SECRET_FIELDS,
};
use mango_core::Account as CoreAccount;
use mango_minecraft::LaunchAccount;
use serde_json::{Map, Value};
use std::fmt;
use std::sync::atomic::Ordering;
use zeroize::Zeroize;

/// Runtime state of one account.
pub(crate) struct Session {
    pub account: AuthAccount,
    /// The refresh token is in the stored envelope.
    pub persisted: bool,
    /// Microsoft rejected the refresh token: sign in again.
    pub rejected: bool,
}

impl Session {
    pub(crate) fn cred_state(&self) -> CredState {
        match self.account.kind {
            AuthKind::Offline => CredState {
                persisted: true,
                locked: false,
                needs_relogin: false,
            },
            AuthKind::Microsoft => {
                let has_token = self
                    .account
                    .msa_refresh_token
                    .as_deref()
                    .is_some_and(|t| !t.is_empty());
                CredState {
                    persisted: self.persisted && has_token,
                    locked: self.account.credentials_locked,
                    needs_relogin: !has_token || self.rejected,
                }
            }
        }
    }
}

/// What a launch needs from an account. Redacted in `Debug`, zeroized on drop.
pub struct LaunchCredentials {
    pub name: String,
    pub uuid: String,
    pub offline: bool,
    access_token: String,
}

impl LaunchCredentials {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// For `build_launch_command` (the target type prints the token in
    /// `Debug`: never format it).
    pub fn to_launch_account(&self) -> LaunchAccount {
        LaunchAccount {
            name: self.name.clone(),
            uuid: self.uuid.clone(),
            access_token: self.access_token.clone(),
            xuid: None,
            offline: self.offline,
        }
    }
}

impl Drop for LaunchCredentials {
    fn drop(&mut self) {
        self.access_token.zeroize();
    }
}

impl fmt::Debug for LaunchCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchCredentials")
            .field("name", &self.name)
            .field("uuid", &self.uuid)
            .field("offline", &self.offline)
            .field("access_token", &"<redacted>")
            .finish()
    }
}

struct LoginGuard<'a>(&'a Inner);

impl Drop for LoginGuard<'_> {
    fn drop(&mut self) {
        self.0.signing_in.store(false, Ordering::SeqCst);
    }
}

fn account_not_found() -> AppError {
    AppError::new("account_not_found", "Account not found")
}

fn record_map(rec: &CoreAccount) -> Option<Map<String, Value>> {
    match serde_json::to_value(rec) {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}

impl Inner {
    /// Load the runtime state of a stored record (once), migrating and
    /// scrubbing legacy plaintext tokens on the way. Returns its credential
    /// state.
    pub(crate) fn ingest(&self, rec: &CoreAccount) -> CredState {
        if let Some(s) = lock(&self.sessions).get(&rec.uuid) {
            return s.cred_state();
        }
        let broken = CredState {
            persisted: false,
            locked: false,
            needs_relogin: rec.kind == mango_core::AccountType::Microsoft,
        };
        let Some(map) = record_map(rec) else {
            return broken;
        };
        let mut session = match self.vault.load_account(&map) {
            Ok(acc) => {
                let persisted = acc.kind == AuthKind::Offline
                    || (!acc.credentials_locked && acc.msa_refresh_token.is_some());
                Session {
                    account: acc,
                    persisted,
                    rejected: false,
                }
            }
            Err(AuthError::LegacyEnvelope) => {
                // Electron (Chromium OSCrypt) blob: unreadable, one re-login.
                let Ok(mut acc) = serde_json::from_value::<AuthAccount>(Value::Object(map)) else {
                    return broken;
                };
                acc.zeroize_tokens();
                acc.access_token.clear();
                acc.msa_refresh_token = None;
                acc.credentials_locked = false;
                let session = Session {
                    account: acc,
                    persisted: false,
                    rejected: true,
                };
                let state = session.cred_state();
                lock(&self.sessions).insert(rec.uuid.clone(), session);
                return state;
            }
            Err(_) => return broken,
        };

        let dirty =
            needs_migration(&map) || rec.access_token.is_some() || rec.msa_refresh_token.is_some();
        if dirty {
            let scrubbed = match self.vault.store_account(&session.account) {
                Ok(StoredAccount { value, persisted }) => {
                    session.persisted = persisted && session.account.msa_refresh_token.is_some();
                    value
                }
                Err(_) => {
                    tracing::warn!("could not re-encrypt account credentials");
                    session.persisted = false;
                    let mut m = map.clone();
                    for k in SECRET_FIELDS {
                        m.remove(k);
                    }
                    m
                }
            };
            match serde_json::from_value::<CoreAccount>(Value::Object(scrubbed)) {
                Ok(new_rec) => {
                    let uuid = rec.uuid.clone();
                    let saved = self.with_store(|s| {
                        if let Some(slot) = s.accounts.iter_mut().find(|a| a.uuid == uuid) {
                            *slot = new_rec;
                        }
                        s.save_accounts_scrubbed()
                    });
                    if saved.is_err() {
                        tracing::warn!("could not save the scrubbed accounts file");
                    }
                }
                Err(_) => tracing::warn!("could not convert the scrubbed account"),
            }
        }
        let state = session.cred_state();
        lock(&self.sessions).insert(rec.uuid.clone(), session);
        state
    }

    pub(crate) fn record(&self, uuid: &str) -> Option<CoreAccount> {
        self.with_store(|s| s.accounts.iter().find(|a| a.uuid == uuid).cloned())
    }

    pub(crate) fn public_account(&self, uuid: &str) -> Res<PublicAccount> {
        let rec = self.record(uuid).ok_or_else(account_not_found)?;
        let creds = self.ingest(&rec);
        Ok(PublicAccount::from_account(&rec, now_ms() as i64, creds).versioned(&self.paths))
    }

    /// Write a vault result into the store and remember the runtime account.
    fn commit(&self, acc: AuthAccount, stored: StoredAccount) -> Res<PublicAccount> {
        let uuid = acc.uuid.clone();
        let persisted = stored.persisted;
        let put = |acc: AuthAccount, persisted: bool| {
            lock(&self.sessions).insert(
                uuid.clone(),
                Session {
                    account: acc,
                    persisted,
                    rejected: false,
                },
            );
        };
        let rec: CoreAccount = match serde_json::from_value(Value::Object(stored.value)) {
            Ok(r) => r,
            Err(e) => {
                put(acc, false);
                return Err(e.into());
            }
        };
        if let Err(e) = self.with_store(|s| s.upsert_account(rec.clone())) {
            put(acc, false);
            return Err(e.into());
        }
        put(acc, persisted);
        self.public_account(&uuid)
    }

    fn emit_changed(&self, uuid: &str) {
        self.emit(Event::AccountsChanged(AccountsChangedEvent {
            uuid: uuid.to_string(),
        }));
    }
}

impl AppService {
    /// The runtime account (tokens in memory) of `uuid`.
    pub(crate) fn session_account(&self, uuid: &str) -> Res<AuthAccount> {
        let rec = self.inner.record(uuid).ok_or_else(account_not_found)?;
        self.inner.ingest(&rec);
        lock(&self.inner.sessions)
            .get(uuid)
            .map(|s| s.account.clone())
            .ok_or_else(account_not_found)
    }

    /// Remember fresh skin/cape lists; persists them (they are part of the
    /// stored record, no secrets) only when they changed.
    pub(crate) async fn store_textures(
        &self,
        uuid: &str,
        skins: Vec<mango_auth::Texture>,
        capes: Vec<mango_auth::Texture>,
    ) {
        let changed = {
            let mut sessions = lock(&self.inner.sessions);
            match sessions.get_mut(uuid) {
                Some(s) if s.account.skins != skins || s.account.capes != capes => {
                    s.account.skins = skins;
                    s.account.capes = capes;
                    Some(s.account.clone())
                }
                _ => None,
            }
        };
        if let Some(acc) = changed {
            if let Err(e) = self.persist_account(acc).await {
                tracing::warn!("could not persist the account textures: {}", e.code);
            }
            self.inner.emit_changed(uuid);
        }
    }

    /// Startup: ingest every stored account (migrates and scrubs).
    pub(crate) fn load_accounts(&self) {
        let records = self.inner.with_store(|s| s.accounts.clone());
        for rec in &records {
            self.inner.ingest(rec);
        }
    }

    pub fn account_add_offline(&self, name: String) -> Res<PublicAccount> {
        let acc = mango_auth::offline_account(&name)?;
        // Offline accounts never touch the keystore.
        let stored = self.inner.vault.store_account(&acc)?;
        self.inner.commit(acc, stored)
    }

    pub fn account_remove(&self, uuid: String) -> Res<()> {
        if let Some(map) = self.inner.record(&uuid).as_ref().and_then(record_map) {
            if self.inner.vault.delete_credentials(&map).is_err() {
                tracing::warn!("could not delete a legacy credential entry");
            }
        }
        lock(&self.inner.sessions).remove(&uuid);
        self.inner
            .with_store(|s| s.remove_account(&uuid))
            .map_err(Into::into)
    }

    /// Vault encryption off the async threads (the first call may talk to the
    /// OS keystore), then store + session update.
    pub(crate) async fn persist_account(&self, mut acc: AuthAccount) -> Res<PublicAccount> {
        if acc.encrypted_credentials.is_none() {
            // Keep an existing (unreadable) envelope when the keystore is
            // unavailable right now.
            acc.encrypted_credentials = self
                .inner
                .record(&acc.uuid)
                .and_then(|r| r.encrypted_credentials);
        }
        let vault = self.inner.vault.clone();
        let (acc, stored) = tokio::task::spawn_blocking(move || {
            let r = vault.store_account(&acc);
            (acc, r)
        })
        .await?;
        match stored {
            Ok(stored) => self.inner.commit(acc, stored),
            Err(e) => {
                // Keep the session usable in memory.
                lock(&self.inner.sessions).insert(
                    acc.uuid.clone(),
                    Session {
                        account: acc,
                        persisted: false,
                        rejected: false,
                    },
                );
                Err(e.into())
            }
        }
    }

    /// Opens the sign-in window and runs the full Microsoft -> Xbox -> XSTS ->
    /// Minecraft chain. `None` when the user cancelled.
    pub async fn account_sign_in_microsoft(&self) -> Res<Option<PublicAccount>> {
        let inner = &self.inner;
        if inner
            .signing_in
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AppError::new(
                "auth.login_in_progress",
                "A sign-in window is already open",
            ));
        }
        let _guard = LoginGuard(inner);
        let ep = inner.opts.auth_endpoints.clone();
        // Binds the redirect to this attempt; checked in `parse_redirect`.
        let state = mango_auth::new_oauth_state();
        let url = ep.build_authorize_url(&state);
        let outcome = inner.login.authorize(&url, &ep.redirect_uri).await;
        let code = match outcome {
            LoginOutcome::Cancelled => return Ok(None),
            LoginOutcome::Error(msg) => return Err(AppError::new("auth.login_failed", msg)),
            LoginOutcome::Code(text) => match ep.parse_redirect(&text, &state) {
                Some(Redirect::Code(c)) => c,
                Some(Redirect::Cancelled) => return Ok(None),
                Some(Redirect::Error(msg)) => return Err(AppError::new("auth.login_failed", msg)),
                None if is_bare_code(&text) => text,
                None => {
                    return Err(AppError::new(
                        "auth.login_failed",
                        "The sign-in did not return an authorization code",
                    ))
                }
            },
        };
        let tokens = mango_auth::exchange_code(&ep, &code).await?;
        let acc = mango_auth::login_with_tokens(&ep, &tokens).await?;
        let uuid = acc.uuid.clone();
        let public = self.persist_account(acc).await?;
        inner.emit_changed(&uuid);
        self.spawn_skin_refresh(uuid, true);
        Ok(Some(public))
    }

    /// Refresh the Minecraft access token in memory and persist the rotated
    /// refresh token.
    pub async fn account_refresh(&self, uuid: String) -> Res<PublicAccount> {
        self.refresh_session(&uuid, true).await?;
        self.spawn_skin_refresh(uuid.clone(), true);
        self.inner.public_account(&uuid)
    }

    /// Fresh credentials for launching with `uuid` (refreshes Microsoft
    /// accounts that are about to expire). `needs_relogin` errors when the
    /// user has to sign in again.
    pub async fn launch_credentials(&self, uuid: &str) -> Res<LaunchCredentials> {
        let acc = self.refresh_session(uuid, false).await?;
        Ok(LaunchCredentials {
            name: acc.name.clone(),
            uuid: acc.uuid.clone(),
            offline: acc.kind == AuthKind::Offline,
            access_token: acc.access_token.clone(),
        })
    }

    pub(crate) async fn refresh_session(&self, uuid: &str, force: bool) -> Res<AuthAccount> {
        let inner = &self.inner;
        let rec = inner.record(uuid).ok_or_else(account_not_found)?;
        inner.ingest(&rec);
        let _gate = inner.refresh_gate.lock().await;
        let current = lock(&inner.sessions)
            .get(uuid)
            .map(|s| s.account.clone())
            .ok_or_else(account_not_found)?;
        if current.kind == AuthKind::Offline {
            return Ok(current);
        }
        let mut input = current.clone();
        if force {
            input.access_token.zeroize();
        }
        let ep = inner.opts.auth_endpoints.clone();
        match mango_auth::ensure_fresh_account(&ep, input).await {
            Ok(fresh) => {
                let changed = fresh.access_token != current.access_token
                    || fresh.expires_at != current.expires_at
                    || fresh.msa_refresh_token != current.msa_refresh_token;
                if changed {
                    if let Err(e) = self.persist_account(fresh.clone()).await {
                        tracing::warn!("could not persist the refreshed account: {}", e.code);
                    }
                    inner.emit_changed(uuid);
                }
                Ok(fresh)
            }
            Err(e) => {
                if e.needs_relogin() {
                    if let Some(s) = lock(&inner.sessions).get_mut(uuid) {
                        s.rejected = true;
                    }
                    inner.emit_changed(uuid);
                }
                Err(e.into())
            }
        }
    }
}

/// An authorization code pasted without its URL.
fn is_bare_code(text: &str) -> bool {
    !text.is_empty()
        && text.len() < 2048
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'!' | b'*'))
}

/// Hosts help links may point to (and their subdomains).
const EXTERNAL_HOSTS: [&str; 4] = ["microsoft.com", "xbox.com", "minecraft.net", "live.com"];

fn external_host_allowed(raw: &str) -> bool {
    if !mango_mango::security::is_allowed_external_url(raw) {
        return false;
    }
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    EXTERNAL_HOSTS
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

impl AppService {
    /// Open a help link in the browser: https, no credentials, and only
    /// microsoft.com / xbox.com / minecraft.net / live.com (+ subdomains).
    pub fn open_external(&self, url: String) -> Res<()> {
        if !external_host_allowed(&url) {
            return Err(AppError::new(
                "external_url_forbidden",
                "This link is not allowed to open",
            ));
        }
        self.inner
            .host
            .external
            .open_url(&url)
            .map_err(|e| AppError::new("external_open_failed", e))
    }
}
