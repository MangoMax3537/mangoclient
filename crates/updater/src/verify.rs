//! Detached Ed25519 verification over the exact bytes of the downloaded file.
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

/// Verify a file; on any failure the file is deleted before returning.
pub async fn verify_file_or_delete(
    public_key_b64: &str,
    signature_b64: &str,
    file: &Path,
) -> Result<()> {
    let result = async {
        let bytes = tokio::fs::read(file).await?;
        verify_bytes(public_key_b64, signature_b64, &bytes)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(file).await;
    }
    result
}
