//! DEV-ONLY helper: generate an update key pair or sign a file.
//!
//!   cargo run -p mango-updater --example keygen            # new key pair
//!   MANGO_UPDATE_PRIVATE_KEY=<seed b64> cargo run -p mango-updater --example keygen -- sign <file> <version> <platform key>
//!
//! The signature covers `mango-update-v1|<version>|<platform key>|<sha256 of file>`.
//!
//! Paste the public key into `UPDATE_PUBLIC_KEY_B64`. Keep the private seed in
//! CI secrets only; never commit it. Randomness comes from /dev/urandom, so
//! this runs on Linux/macOS (CI signing happens there).

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use sha2::Digest;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("sign") {
        let usage = "usage: sign <file> <version> <platform key>";
        let file = args.next().ok_or(usage)?;
        let version = args.next().ok_or(usage)?;
        let platform = args.next().ok_or(usage)?;
        let version: semver::Version = version.trim_start_matches('v').parse()?;
        let seed = std::env::var("MANGO_UPDATE_PRIVATE_KEY")?;
        let seed: [u8; 32] = b64
            .decode(seed.trim())?
            .try_into()
            .map_err(|_| "private key must be 32 bytes")?;
        let sha = hex::encode(sha2::Sha256::digest(std::fs::read(file)?));
        let msg = mango_updater::update_message(&version.to_string(), &platform, &sha);
        let sig = SigningKey::from_bytes(&seed).sign(&msg);
        println!("{}", b64.encode(sig.to_bytes()));
        return Ok(());
    }
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
    let key = SigningKey::from_bytes(&seed);
    println!(
        "public  (compile in): {}",
        b64.encode(key.verifying_key().to_bytes())
    );
    println!("private (CI secret):  {}", b64.encode(seed));
    Ok(())
}
