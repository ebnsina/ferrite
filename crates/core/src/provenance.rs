//! Content provenance: a signed, tamper-evident manifest describing how an
//! asset was produced (tool, operation, source lineage, content hash). Signed
//! with Ed25519 so anyone with the public key can verify authenticity, and the
//! embedded SHA-256 detects any later modification of the file.
//!
//! This is a self-contained, C2PA-inspired scheme; embedding a standards
//! compliant C2PA manifest into the media is a future interop upgrade.

use ed25519_dalek::{Signer, SigningKey, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A provenance claim over one produced asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u8,
    /// Producing tool, e.g. "Ferrite Stream".
    pub tool: String,
    pub tenant_id: Uuid,
    pub asset_id: Uuid,
    pub filename: String,
    /// Hex SHA-256 of the produced file at signing time.
    pub sha256: String,
    /// How it was made: "clip", "shorts", "live-clip", "transcode", "upload".
    pub operation: String,
    /// Lineage: the source asset this was derived from, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_asset_id: Option<Uuid>,
    pub created_at: String,
}

impl Manifest {
    /// Canonical bytes that get signed (stable serde field order).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("manifest serializes")
    }
}

/// Deterministic signing key derived from a deployment secret (SHA-256 seed).
fn signing_key(secret: &str) -> SigningKey {
    let seed: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
    SigningKey::from_bytes(&seed)
}

/// The deployment's public verification key (hex).
pub fn public_key_hex(secret: &str) -> String {
    hex::encode(signing_key(secret).verifying_key().to_bytes())
}

/// Sign a manifest's canonical JSON; returns the hex signature.
pub fn sign(secret: &str, manifest_json: &str) -> String {
    hex::encode(
        signing_key(secret)
            .sign(manifest_json.as_bytes())
            .to_bytes(),
    )
}

/// Verify a hex signature over the exact manifest JSON that was signed.
pub fn verify(secret: &str, manifest_json: &str, sig_hex: &str) -> bool {
    let Ok(bytes) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(sig) = ed25519_dalek::Signature::from_slice(&bytes) else {
        return false;
    };
    signing_key(secret)
        .verifying_key()
        .verify(manifest_json.as_bytes(), &sig)
        .is_ok()
}

/// Hex SHA-256 of arbitrary bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        Manifest {
            version: 1,
            tool: "Ferrite Stream".into(),
            tenant_id: Uuid::nil(),
            asset_id: Uuid::nil(),
            filename: "clip.mp4".into(),
            sha256: "abc".into(),
            operation: "clip".into(),
            source_asset_id: Some(Uuid::nil()),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let json = sample().to_json();
        let sig = sign("secret", &json);
        assert!(verify("secret", &json, &sig));
    }

    #[test]
    fn tampered_manifest_fails() {
        let sig = sign("secret", &sample().to_json());
        let mut m = sample();
        m.sha256 = "different".into();
        assert!(!verify("secret", &m.to_json(), &sig));
    }

    #[test]
    fn wrong_key_fails() {
        let json = sample().to_json();
        let sig = sign("secret", &json);
        assert!(!verify("other-secret", &json, &sig));
    }
}
