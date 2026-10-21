// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Fleet secrets: encrypted-at-rest deployment secrets and log redaction.
//!
//! The design, threat model and rotation rules are in `docs/SECRETS.md`.
//! Short version: the bucket only ever holds AES-256-GCM ciphertext; the
//! 32-byte fleet key comes from `CELLD_SECRET_KEY` in the deploy and node
//! environments; the node decrypts once per generation and the redactor
//! rewrites known values in runtime logs.

// Reading the fleet key from the process environment is the whole point of
// the module; the determinism lint that refuses `std::env` elsewhere does not
// apply to an operator-provided secret.
#![allow(clippy::disallowed_methods)]

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, bail, Context};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::{OnceLock, RwLock};

/// Environment variable holding the fleet key (32 bytes, 64 hex characters).
pub const KEY_ENV: &str = "CELLD_SECRET_KEY";
/// Envelope format version. Bumping it is a deployment feature, not a silent
/// reinterpretation: a node that cannot read the version refuses the deploy.
pub const ENVELOPE_VERSION: u8 = 1;
/// Deployment feature naming a manifest that carries encrypted secrets.
pub const FEATURE_SECRETS_V1: &str = crate::protocol::FEATURE_SECRETS_V1;

const NONCE_BYTES: usize = 12;
const MAX_SECRET_BYTES: usize = 1024 * 1024;
const MAX_REDACTIONS: usize = 4096;

/// The binding kind a secret is exposed as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretKind {
    /// `secret_text`: a UTF-8 string in `env`.
    Text,
    /// `secret_key`: an `ArrayBuffer` in `env`.
    Key,
}

impl SecretKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Key => "key",
        }
    }

    /// The manifest binding type for this kind.
    pub fn binding_type(self) -> &'static str {
        match self {
            Self::Text => "secret_text",
            Self::Key => "secret_key",
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "text" => Ok(Self::Text),
            "key" => Ok(Self::Key),
            other => bail!("secret kind {other:?} is not supported; use \"text\" or \"key\""),
        }
    }
}

/// A decrypted secret as it travels from the deploy input to the Worker env.
#[derive(Clone, PartialEq, Eq)]
pub struct PlainSecret {
    pub name: String,
    pub kind: SecretKind,
    pub value: Vec<u8>,
}

impl std::fmt::Debug for PlainSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlainSecret")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("value", &"[redacted]")
            .finish()
    }
}

/// A decrypted secret bound into one worker's `env`. This is the process-only
/// form; it is never serialized into a manifest, a log line, or a report.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretBinding {
    pub name: String,
    pub kind: SecretKind,
    pub value: Vec<u8>,
}

impl std::fmt::Debug for SecretBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretBinding")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("value", &"[redacted]")
            .finish()
    }
}

impl From<PlainSecret> for SecretBinding {
    fn from(secret: PlainSecret) -> Self {
        Self {
            name: secret.name,
            kind: secret.kind,
            value: secret.value,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u8,
    script: String,
    entries: Vec<EncryptedEntry>,
}

#[derive(Serialize, Deserialize)]
struct EncryptedEntry {
    name: String,
    kind: SecretKind,
    nonce: String,
    ciphertext: String,
}

/// The fleet key. Its `Debug` never prints the bytes.
#[derive(Clone)]
pub struct FleetKey([u8; 32]);

impl std::fmt::Debug for FleetKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FleetKey([redacted])")
    }
}

impl FleetKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse `CELLD_SECRET_KEY`: exactly 64 hex characters (32 bytes).
    ///
    /// The error never echoes the input: a typo in a key is still a key.
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let value = value.trim();
        if value.len() != 64 {
            bail!(
                "{KEY_ENV} must be 64 hex characters (32 bytes); it is {} characters",
                value.len()
            );
        }
        let mut bytes = [0u8; 32];
        for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let hi = hex_nibble(pair[0]).with_context(|| {
                format!(
                    "{KEY_ENV} contains a non-hex character at position {}",
                    index * 2
                )
            })?;
            let lo = hex_nibble(pair[1]).with_context(|| {
                format!(
                    "{KEY_ENV} contains a non-hex character at position {}",
                    index * 2 + 1
                )
            })?;
            bytes[index] = hi << 4 | lo;
        }
        Ok(Self(bytes))
    }

    /// Read and parse the fleet key from the process environment.
    pub fn from_env() -> anyhow::Result<Self> {
        let value = std::env::var(KEY_ENV).map_err(|_| {
            anyhow!("{KEY_ENV} is not set; a deployment carrying secrets needs the fleet key")
        })?;
        Self::parse(&value)
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn aad(script: &str, name: &str, kind: SecretKind) -> Vec<u8> {
    format!(
        "celd-secrets\0{}\0{}\0{}\0{}",
        ENVELOPE_VERSION,
        script,
        name,
        kind.as_str()
    )
    .into_bytes()
}

/// Encrypt every secret for one deployment. The returned bytes are the
/// envelope to store at `<prefix>/secrets.json`; its SHA-256 is the manifest
/// reference and part of the deployment version.
pub fn encrypt(key: &FleetKey, script: &str, secrets: &[PlainSecret]) -> anyhow::Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(&key.0).map_err(|_| anyhow!("fleet key has the wrong length"))?;
    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(secrets.len());
    for secret in secrets {
        anyhow::ensure!(
            seen.insert(secret.name.clone()),
            "secret {:?} is declared twice",
            secret.name
        );
        anyhow::ensure!(
            !secret.value.is_empty(),
            "secret {:?} is empty",
            secret.name
        );
        anyhow::ensure!(
            secret.value.len() <= MAX_SECRET_BYTES,
            "secret {:?} is larger than {} bytes",
            secret.name,
            MAX_SECRET_BYTES
        );
        if secret.kind == SecretKind::Text {
            std::str::from_utf8(&secret.value).with_context(|| {
                format!("secret {:?} is a text secret but is not UTF-8", secret.name)
            })?;
        }
        let mut nonce = [0u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| anyhow!("OS random source unavailable"))?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &secret.value,
                    aad: &aad(script, &secret.name, secret.kind),
                },
            )
            .map_err(|_| anyhow!("encrypt secret {:?} failed", secret.name))?;
        entries.push(EncryptedEntry {
            name: secret.name.clone(),
            kind: secret.kind,
            nonce: base64::engine::general_purpose::STANDARD.encode(nonce),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        });
    }
    let envelope = Envelope {
        version: ENVELOPE_VERSION,
        script: script.to_string(),
        entries,
    };
    serde_json::to_vec_pretty(&envelope).context("encode secret envelope")
}

/// Decrypt an envelope for `script`. Errors name the secret and the failure
/// class, never the ciphertext, the key, or the plaintext.
pub fn decrypt(key: &FleetKey, script: &str, bytes: &[u8]) -> anyhow::Result<Vec<PlainSecret>> {
    let envelope: Envelope = serde_json::from_slice(bytes).context("decode secret envelope")?;
    anyhow::ensure!(
        envelope.version == ENVELOPE_VERSION,
        "secret envelope version {} is not supported (this node reads {ENVELOPE_VERSION})",
        envelope.version
    );
    anyhow::ensure!(
        envelope.script == script,
        "secret envelope belongs to script {:?}, not {script:?}",
        envelope.script
    );
    let cipher =
        Aes256Gcm::new_from_slice(&key.0).map_err(|_| anyhow!("fleet key has the wrong length"))?;
    let mut secrets = Vec::with_capacity(envelope.entries.len());
    for entry in envelope.entries {
        let nonce = base64::engine::general_purpose::STANDARD
            .decode(&entry.nonce)
            .with_context(|| format!("secret {:?} has an invalid nonce", entry.name))?;
        anyhow::ensure!(
            nonce.len() == NONCE_BYTES,
            "secret {:?} has an invalid nonce length",
            entry.name
        );
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&entry.ciphertext)
            .with_context(|| format!("secret {:?} has invalid ciphertext", entry.name))?;
        let value = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad(script, &entry.name, entry.kind),
                },
            )
            .map_err(|_| {
                anyhow!(
                    "secret {:?} failed authentication: wrong {KEY_ENV} or a tampered envelope",
                    entry.name
                )
            })?;
        if entry.kind == SecretKind::Text {
            std::str::from_utf8(&value)
                .with_context(|| format!("secret {:?} decrypted to invalid UTF-8", entry.name))?;
        }
        secrets.push(PlainSecret {
            name: entry.name,
            kind: entry.kind,
            value,
        });
    }
    Ok(secrets)
}

/// SHA-256 of the envelope bytes, as the manifest reference uses it.
pub fn envelope_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

fn redactions() -> &'static RwLock<Vec<String>> {
    static REDACTIONS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
    REDACTIONS.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register decrypted values so runtime logs can redact them. Values shorter
/// than the longest are replaced first so a secret that contains another one
/// does not leave a tail behind.
pub fn register_redactions(secrets: &[PlainSecret]) {
    let mut additions = Vec::new();
    for secret in secrets {
        match secret.kind {
            SecretKind::Text => {
                if let Ok(text) = std::str::from_utf8(&secret.value) {
                    additions.push(text.to_string());
                }
            }
            SecretKind::Key => {
                // The binding is binary, but an application that logs it will
                // usually log one of these encodings. Raw bytes are never
                // registered; they cannot appear in a UTF-8 log by accident.
                additions.push(base64::engine::general_purpose::STANDARD.encode(&secret.value));
                additions
                    .push(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&secret.value));
                additions.push(hex(&secret.value));
                if let Ok(text) = std::str::from_utf8(&secret.value) {
                    additions.push(text.to_string());
                }
            }
        }
    }
    if additions.is_empty() {
        return;
    }
    let Ok(mut values) = redactions().write() else {
        return;
    };
    for value in additions {
        if value.is_empty() || values.contains(&value) {
            continue;
        }
        values.push(value);
    }
    // Keep the registry bounded and keep longest-first so a value that
    // contains another cannot be half-redacted.
    values.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    values.truncate(MAX_REDACTIONS);
}

/// Replace every registered value in `text` with `[redacted]`.
pub fn redact(text: &str) -> String {
    let guard = match redactions().read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for value in guard.iter() {
        if out.contains(value.as_str()) {
            out = out.replace(value.as_str(), "[redacted]");
        }
    }
    out
}

/// The longest console line the host retains. A runaway line is truncated on
/// a UTF-8 boundary and marked, instead of being dropped or duplicated whole.
pub const LOG_LIMIT: usize = 8 * 1024;

pub fn cap_log(text: &str) -> String {
    if text.len() <= LOG_LIMIT {
        return text.to_string();
    }
    let mut end = LOG_LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated]", &text[..end])
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> FleetKey {
        FleetKey::parse(&"ab".repeat(32)).expect("valid key")
    }

    fn secrets() -> Vec<PlainSecret> {
        vec![
            PlainSecret {
                name: "API_TOKEN".into(),
                kind: SecretKind::Text,
                value: b"sk-live-very-secret".to_vec(),
            },
            PlainSecret {
                name: "TLS_KEY".into(),
                kind: SecretKind::Key,
                value: vec![0, 1, 2, 3, 255],
            },
        ]
    }

    #[test]
    fn key_parsing_is_strict_and_opaque() {
        assert!(FleetKey::parse(&"ab".repeat(32)).is_ok());
        assert!(FleetKey::parse("").is_err());
        assert!(FleetKey::parse(&"ab".repeat(31)).is_err());
        assert!(FleetKey::parse(&"zz".repeat(32)).is_err());
        let error = FleetKey::parse(&"zz".repeat(32)).unwrap_err().to_string();
        assert!(!error.contains("zzzz"));
        assert!(!format!("{:?}", key()).contains("abab"));
    }

    #[test]
    fn round_trip_text_and_key() {
        let envelope = encrypt(&key(), "worker", &secrets()).unwrap();
        let decoded = decrypt(&key(), "worker", &envelope).unwrap();
        assert_eq!(decoded, secrets());
    }

    #[test]
    fn wrong_key_is_rejected_without_echoing_material() {
        let envelope = encrypt(&key(), "worker", &secrets()).unwrap();
        let other = FleetKey::parse(&"cd".repeat(32)).unwrap();
        let error = decrypt(&other, "worker", &envelope)
            .unwrap_err()
            .to_string();
        assert!(error.contains("failed authentication"));
        assert!(!error.contains("sk-live"));
    }

    #[test]
    fn aad_binds_script_name_and_kind() {
        let envelope = encrypt(&key(), "worker", &secrets()).unwrap();
        assert!(decrypt(&key(), "other-worker", &envelope).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&envelope).unwrap();
        value["entries"][0]["name"] = "RENAMED".into();
        let tampered = serde_json::to_vec(&value).unwrap();
        assert!(decrypt(&key(), "worker", &tampered).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&envelope).unwrap();
        value["entries"][0]["kind"] = "key".into();
        let tampered = serde_json::to_vec(&value).unwrap();
        assert!(decrypt(&key(), "worker", &tampered).is_err());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let envelope = encrypt(&key(), "worker", &secrets()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&envelope).unwrap();
        let ciphertext = value["entries"][0]["ciphertext"].as_str().unwrap();
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(ciphertext)
            .unwrap();
        bytes[0] ^= 1;
        value["entries"][0]["ciphertext"] = base64::engine::general_purpose::STANDARD
            .encode(bytes)
            .into();
        let tampered = serde_json::to_vec(&value).unwrap();
        assert!(decrypt(&key(), "worker", &tampered).is_err());
    }

    #[test]
    fn envelope_does_not_contain_plaintext() {
        let envelope = encrypt(&key(), "worker", &secrets()).unwrap();
        let text = String::from_utf8_lossy(&envelope);
        assert!(!text.contains("sk-live-very-secret"));
        assert!(text.contains("API_TOKEN")); // names are metadata
        assert!(!hex(&[0, 1, 2, 3, 255]).is_empty());
    }

    #[test]
    fn redaction_replaces_registered_values() {
        let mut secrets = secrets();
        secrets[0].value = b"redaction-test-value".to_vec();
        secrets[1].value = b"redaction-key-bytes".to_vec();
        register_redactions(&secrets);
        let line = "token=redaction-test-value key=cmVkYWN0aW9uLWtleS1ieXRlcw==";
        let redacted = redact(line);
        assert!(!redacted.contains("redaction-test-value"));
        assert!(!redacted.contains("cmVkYWN0aW9uLWtleS1ieXRlcw=="));
        assert!(redacted.contains("[redacted]"));
        // Nothing registered: text is untouched.
        assert_eq!(redact("plain line"), "plain line");
    }

    #[test]
    fn empty_and_duplicate_secrets_are_refused() {
        let empty = PlainSecret {
            name: "EMPTY".into(),
            kind: SecretKind::Text,
            value: Vec::new(),
        };
        assert!(encrypt(&key(), "worker", &[empty]).is_err());
        let one = secrets().remove(0);
        let two = one.clone();
        assert!(encrypt(&key(), "worker", &[one, two]).is_err());
    }

    #[test]
    fn missing_key_is_a_clear_error() {
        let error = FleetKey::from_env().unwrap_err().to_string();
        assert!(error.contains(KEY_ENV));
    }
}
