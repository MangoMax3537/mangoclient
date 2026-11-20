//! Envelope v2: `mc2:<base64(nonce(24) || ciphertext)>`.
//!
//! XChaCha20-Poly1305, random 192-bit nonce per encryption, AAD
//! `mangoclient:v2:account:<uuid>` (binds the envelope to its account and
//! format version). The plaintext is `{"msaRefreshToken": "..."}` and nothing
//! else.

use crate::error::AuthError;
use crate::keystore::DeviceKey;
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub const ENVELOPE_V2_PREFIX: &str = "mc2:";
const NONCE_LEN: usize = 24;

fn aad(uuid: &str) -> String {
    format!("mangoclient:v2:account:{uuid}")
}

#[derive(Serialize)]
struct SecretsOut<'a> {
    #[serde(rename = "msaRefreshToken")]
    msa_refresh_token: &'a str,
}

#[derive(Deserialize)]
struct SecretsIn {
    #[serde(rename = "msaRefreshToken")]
    msa_refresh_token: String,
}

impl Drop for SecretsIn {
    fn drop(&mut self) {
        self.msa_refresh_token.zeroize();
    }
}

fn crypto_err() -> AuthError {
    AuthError::Crypto
}

pub fn is_v2(envelope: &str) -> bool {
    envelope.starts_with(ENVELOPE_V2_PREFIX)
}

/// Encrypt the refresh token for `uuid`.
pub fn seal(key: &DeviceKey, uuid: &str, refresh_token: &str) -> Result<String, AuthError> {
    let plain = Zeroizing::new(
        serde_json::to_vec(&SecretsOut {
            msa_refresh_token: refresh_token,
        })
        .map_err(|_| crypto_err())?,
    );
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|_| crypto_err())?;
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plain,
                aad: aad(uuid).as_bytes(),
            },
        )
        .map_err(|_| crypto_err())?;
    let mut blob = Vec::with_capacity(NONCE_LEN + ct.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ct);
    Ok(format!("{ENVELOPE_V2_PREFIX}{}", STANDARD.encode(blob)))
}

/// Decrypt an envelope; any tampering, wrong key or wrong account fails.
pub fn open(key: &DeviceKey, uuid: &str, envelope: &str) -> Result<Zeroizing<String>, AuthError> {
    let body = envelope
        .strip_prefix(ENVELOPE_V2_PREFIX)
        .ok_or_else(crypto_err)?;
    let blob = STANDARD.decode(body).map_err(|_| crypto_err())?;
    if blob.len() <= NONCE_LEN {
        return Err(crypto_err());
    }
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let plain = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ct,
                    aad: aad(uuid).as_bytes(),
                },
            )
            .map_err(|_| crypto_err())?,
    );
    let mut parsed: SecretsIn = serde_json::from_slice(&plain).map_err(|_| crypto_err())?;
    Ok(Zeroizing::new(std::mem::take(
        &mut parsed.msa_refresh_token,
    )))
}
