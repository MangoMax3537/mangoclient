//! Detached Ed25519 verification of a domain-separated message that commits
//! to version, platform and the SHA-256 of the downloaded file.
//!
//! KEY MANAGEMENT: [`UPDATE_PUBLIC_KEY_B64`] below is a PLACEHOLDER and makes
//! every update fail closed until replaced. Generate a pair with
//! `cargo run -p mango-updater --example keygen` (dev-only), compile the
//! public half in here, and store the private half ONLY in CI secrets.
//! Never commit a private key.

use crate::error::{Result, UpdateError};
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use std::path::Path;

/// Base64 of the 32-byte Ed25519 public key. PLACEHOLDER, replace before release.
pub const UPDATE_PUBLIC_KEY_B64: &str = "AyaIbka9P2+y4lEja49+7WBPbxiciTudYXqT9c9BTT8=";

fn b64(s: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .ok()
}

/// Verify `message` against a base64 signature with a base64 public key.
pub fn verify_bytes(public_key_b64: &str, signature_b64: &str, message: &[u8]) -> Result<()> {
    let key_bytes: [u8; 32] = b64(public_key_b64)
        .and_then(|b| b.try_into().ok())
        .ok_or(UpdateError::BadPublicKey)?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| UpdateError::BadPublicKey)?;
    let sig_bytes: [u8; 64] = b64(signature_b64)
        .and_then(|b| b.try_into().ok())
        .ok_or(UpdateError::SignatureEncoding)?;
    let sig = Signature::from_bytes(&sig_bytes);
    key.verify_strict(message, &sig)
        .map_err(|_| UpdateError::SignatureInvalid)
}

/// The exact message the publisher signs. Binding version, platform key and
/// content hash stops a validly signed older (or other-platform) installer
/// from being served under a newer version or another platform entry.
pub fn update_message(version: &str, platform_key: &str, sha256_hex: &str) -> Vec<u8> {
    format!(
        "mango-update-v1|{version}|{platform_key}|{}",
        sha256_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

/// Verify the signature over [`update_message`] using the SHA-256 of the file
/// on disk; on any failure the file is deleted before returning.
pub async fn verify_update_or_delete(
    public_key_b64: &str,
    signature_b64: &str,
    version: &str,
    platform_key: &str,
    file: &Path,
) -> Result<()> {
    let result = async {
        use sha2::{Digest, Sha256};
        let bytes = tokio::fs::read(file).await?;
        let sha = hex::encode(Sha256::digest(&bytes));
        verify_bytes(
            public_key_b64,
            signature_b64,
            &update_message(version, platform_key, &sha),
        )
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(file).await;
    }
    result
}
