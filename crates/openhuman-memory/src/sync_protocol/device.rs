//! Device identity — per-device X25519 key generation, storage, and discovery.
//!
//! Each device gets a unique identity (X25519 keypair + device_id) persisted
//! in the workspace. Discovery uses mDNS to find peers on the local network.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use rand::rngs::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::config::Config;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A device's identity for sync pairing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceIdentity {
    /// Unique device identifier (hex-encoded hash of public key).
    pub device_id: String,
    /// X25519 public key (32 bytes, hex-encoded).
    pub public_key: String,
    /// Human-readable name for this device.
    pub display_name: String,
    /// Last time this device was active.
    pub last_seen: DateTime<Utc>,
}

/// Full keypair stored locally (private key never leaves the device).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct DeviceKeyFile {
    device_id: String,
    public_key: String,
    /// X25519 private key (32 bytes, hex-encoded). NEVER transmitted.
    private_key: String,
    display_name: String,
    created_at: DateTime<Utc>,
}

/// mDNS service advertisement parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// mDNS service type (default: `_openhuman-memory._tcp`).
    pub service_type: String,
    /// Port the memory sync server listens on.
    pub port: u16,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            service_type: "_openhuman-memory._tcp".into(),
            port: 7891,
        }
    }
}

/// A discovered peer on the network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub device_id: String,
    pub public_key: String,
    pub display_name: String,
    pub address: String,
    pub port: u16,
}

// ─── Key Management ──────────────────────────────────────────────────────────

/// Get or create the device identity for this workspace.
///
/// Reads from `{workspace_dir}/device.key`; generates a new keypair if missing.
pub fn get_or_create_identity(config: &Config) -> Result<DeviceIdentity> {
    let key_path = device_key_path(config);

    if key_path.exists() {
        let content = fs::read_to_string(&key_path)
            .with_context(|| format!("read device key at {}", key_path.display()))?;
        let key_file: DeviceKeyFile = serde_json::from_str(&content)
            .context("parse device key file")?;
        Ok(DeviceIdentity {
            device_id: key_file.device_id,
            public_key: key_file.public_key,
            display_name: key_file.display_name,
            last_seen: Utc::now(),
        })
    } else {
        generate_new_identity(config)
    }
}

/// Generate a fresh X25519 keypair and persist it.
fn generate_new_identity(config: &Config) -> Result<DeviceIdentity> {
    // x25519-dalek handles clamping internally via StaticSecret
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);

    let public_bytes: [u8; 32] = *public.as_bytes();
    let private_bytes: [u8; 32] = *secret.as_bytes();

    let public_hex = hex::encode(public_bytes);
    let private_hex = hex::encode(private_bytes);

    // device_id = first 16 hex chars of SHA-256(public_key)
    let device_id = {
        let mut h = Sha256::new();
        h.update(public_bytes);
        hex::encode(h.finalize())[..16].to_string()
    };

    let display_name = default_display_name();
    let now = Utc::now();

    let key_file = DeviceKeyFile {
        device_id: device_id.clone(),
        public_key: public_hex.clone(),
        private_key: private_hex,
        display_name: display_name.clone(),
        created_at: now,
    };

    let key_path = device_key_path(config);
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&key_file)?;
    fs::write(&key_path, content)
        .with_context(|| format!("write device key to {}", key_path.display()))?;

    Ok(DeviceIdentity {
        device_id,
        public_key: public_hex,
        display_name,
        last_seen: now,
    })
}

/// Update the display name for this device.
pub fn set_display_name(config: &Config, name: &str) -> Result<()> {
    let key_path = device_key_path(config);
    let content = fs::read_to_string(&key_path).context("read device key")?;
    let mut key_file: DeviceKeyFile = serde_json::from_str(&content)?;
    key_file.display_name = name.to_string();
    let updated = serde_json::to_string_pretty(&key_file)?;
    fs::write(&key_path, updated)?;
    Ok(())
}

/// Get the raw private key bytes (for establishing shared secrets). Use with care.
pub fn get_private_key(config: &Config) -> Result<[u8; 32]> {
    let key_path = device_key_path(config);
    let content = fs::read_to_string(&key_path).context("read device key")?;
    let key_file: DeviceKeyFile = serde_json::from_str(&content)?;
    let bytes = hex::decode(&key_file.private_key).context("decode private key hex")?;
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

// ─── Discovery (types only — actual mDNS requires optional dep) ─────────────

/// Build the TXT record properties for mDNS advertisement.
pub fn mdns_txt_properties(identity: &DeviceIdentity) -> Vec<(String, String)> {
    vec![
        ("device_id".into(), identity.device_id.clone()),
        ("public_key".into(), identity.public_key.clone()),
        ("display_name".into(), identity.display_name.clone()),
    ]
}

/// Parse a discovered mDNS service into a DiscoveredPeer.
pub fn parse_discovered_peer(
    txt_records: &[(String, String)],
    address: &str,
    port: u16,
) -> Option<DiscoveredPeer> {
    let mut device_id = None;
    let mut public_key = None;
    let mut display_name = None;

    for (k, v) in txt_records {
        match k.as_str() {
            "device_id" => device_id = Some(v.clone()),
            "public_key" => public_key = Some(v.clone()),
            "display_name" => display_name = Some(v.clone()),
            _ => {}
        }
    }

    Some(DiscoveredPeer {
        device_id: device_id?,
        public_key: public_key?,
        display_name: display_name.unwrap_or_else(|| "Unknown".into()),
        address: address.to_string(),
        port,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn device_key_path(config: &Config) -> PathBuf {
    config.workspace_dir.join("device.key")
}

fn default_display_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown Device".into())
}

/// Public wrapper: compute the X25519 public key for a given private scalar.
///
/// Used by `crypto.rs` tests to set up test keypairs.
pub(crate) fn x25519_base_point_mul_pub(scalar: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*scalar);
    *PublicKey::from(&secret).as_bytes()
}

/// Public wrapper: X25519 scalar multiplication (key agreement).
///
/// Used by `crypto.rs`.
pub(crate) fn x25519_scalar_mul_pub(scalar: &[u8; 32], u_bytes: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*scalar);
    let public = PublicKey::from(*u_bytes);
    *secret.diffie_hellman(&public).as_bytes()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> (TempDir, Config) {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.workspace_dir = tmp.path().to_path_buf();
        (tmp, cfg)
    }

    #[test]
    fn creates_identity_on_first_call() {
        let (_tmp, cfg) = test_config();
        let id = get_or_create_identity(&cfg).unwrap();

        assert_eq!(id.device_id.len(), 16);
        assert_eq!(id.public_key.len(), 64); // 32 bytes hex
        assert!(!id.display_name.is_empty());
    }

    #[test]
    fn returns_same_identity_on_second_call() {
        let (_tmp, cfg) = test_config();
        let id1 = get_or_create_identity(&cfg).unwrap();
        let id2 = get_or_create_identity(&cfg).unwrap();
        assert_eq!(id1.device_id, id2.device_id);
        assert_eq!(id1.public_key, id2.public_key);
    }

    #[test]
    fn update_display_name() {
        let (_tmp, cfg) = test_config();
        get_or_create_identity(&cfg).unwrap();
        set_display_name(&cfg, "My Laptop").unwrap();
        let id = get_or_create_identity(&cfg).unwrap();
        assert_eq!(id.display_name, "My Laptop");
    }

    #[test]
    fn private_key_is_32_bytes() {
        let (_tmp, cfg) = test_config();
        get_or_create_identity(&cfg).unwrap();
        let pk = get_private_key(&cfg).unwrap();
        assert_eq!(pk.len(), 32);
    }

    #[test]
    fn mdns_txt_roundtrip() {
        let (_tmp, cfg) = test_config();
        let id = get_or_create_identity(&cfg).unwrap();
        let txt = mdns_txt_properties(&id);
        let peer = parse_discovered_peer(&txt, "192.168.1.100", 7891).unwrap();
        assert_eq!(peer.device_id, id.device_id);
        assert_eq!(peer.public_key, id.public_key);
    }
}
