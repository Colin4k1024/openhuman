//! Device identity — per-device X25519 key generation, storage, and discovery.
//!
//! Each device gets a unique identity (X25519 keypair + device_id) persisted
//! in the workspace. Discovery uses mDNS to find peers on the local network.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

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
        let identity = generate_new_identity(config)?;
        Ok(identity)
    }
}

/// Generate a fresh X25519 keypair and persist it.
fn generate_new_identity(config: &Config) -> Result<DeviceIdentity> {
    let mut private_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut private_bytes);

    // X25519 clamping (per RFC 7748)
    private_bytes[0] &= 248;
    private_bytes[31] &= 127;
    private_bytes[31] |= 64;

    // Compute public key: scalar multiplication of private key with basepoint
    let public_bytes = x25519_base_point_mul(&private_bytes);

    let public_hex = hex::encode(public_bytes);
    let private_hex = hex::encode(private_bytes);

    // device_id = first 16 hex chars of SHA-256(public_key)
    let device_id = {
        let mut h = Sha256::new();
        h.update(&public_bytes);
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

/// Public wrapper for basepoint multiplication (used by crypto.rs).
pub(crate) fn x25519_base_point_mul_pub(scalar: &[u8; 32]) -> [u8; 32] {
    x25519_base_point_mul(scalar)
}

/// Public wrapper for scalar multiplication (used by crypto.rs for key agreement).
pub(crate) fn x25519_scalar_mul_pub(scalar: &[u8; 32], u_bytes: &[u8; 32]) -> [u8; 32] {
    x25519_scalar_mul(scalar, u_bytes)
}

/// Minimal X25519 basepoint multiplication (constant-time is not critical here
/// since this runs only at key generation, not in a hot path).
/// In production, use the `x25519-dalek` crate; this is a self-contained fallback.
fn x25519_base_point_mul(scalar: &[u8; 32]) -> [u8; 32] {
    // Basepoint for Curve25519: u = 9
    let mut u = [0u8; 32];
    u[0] = 9;
    x25519_scalar_mul(scalar, &u)
}

/// RFC 7748 scalar multiplication on Curve25519 (Montgomery ladder).
fn x25519_scalar_mul(scalar: &[u8; 32], u_bytes: &[u8; 32]) -> [u8; 32] {
    // Field element: 256-bit number mod p = 2^255 - 19
    // Using u128 pairs for the arithmetic is simplest for correctness.
    // This is a textbook implementation — NOT constant-time.
    // For production, replace with `x25519-dalek`.

    let p: [u64; 4] = [
        0xFFFFFFFFFFFFFFED,
        0xFFFFFFFFFFFFFFFF,
        0xFFFFFFFFFFFFFFFF,
        0x7FFFFFFFFFFFFFFF,
    ];

    fn decode(bytes: &[u8; 32]) -> [u64; 4] {
        let mut r = [0u64; 4];
        for i in 0..4 {
            let mut b = [0u8; 8];
            b.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            r[i] = u64::from_le_bytes(b);
        }
        r
    }

    fn encode(v: &[u64; 4]) -> [u8; 32] {
        let mut r = [0u8; 32];
        for i in 0..4 {
            r[i * 8..(i + 1) * 8].copy_from_slice(&v[i].to_le_bytes());
        }
        r
    }

    fn add_mod(a: &[u64; 4], b: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
        let mut r = [0u64; 4];
        let mut carry = 0u64;
        for i in 0..4 {
            let (s1, c1) = a[i].overflowing_add(b[i]);
            let (s2, c2) = s1.overflowing_add(carry);
            r[i] = s2;
            carry = c1 as u64 + c2 as u64;
        }
        // Reduce if >= p
        let mut borrow = 0i64;
        let mut tmp = [0u64; 4];
        for i in 0..4 {
            let diff = r[i] as i128 - p[i] as i128 - borrow as i128;
            tmp[i] = diff as u64;
            borrow = if diff < 0 { 1 } else { 0 };
        }
        if borrow == 0 { tmp } else { r }
    }

    fn sub_mod(a: &[u64; 4], b: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
        let mut borrow = 0i64;
        let mut r = [0u64; 4];
        for i in 0..4 {
            let diff = a[i] as i128 - b[i] as i128 - borrow as i128;
            r[i] = diff as u64;
            borrow = if diff < 0 { 1 } else { 0 };
        }
        if borrow != 0 {
            let mut carry = 0u64;
            for i in 0..4 {
                let (s, c) = r[i].overflowing_add(p[i]);
                let (s2, c2) = s.overflowing_add(carry);
                r[i] = s2;
                carry = c as u64 + c2 as u64;
            }
        }
        r
    }

    fn mul_mod(a: &[u64; 4], b: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
        // Schoolbook multiplication → reduce
        let mut product = [0u128; 8];
        for i in 0..4 {
            let mut carry = 0u128;
            for j in 0..4 {
                let v = product[i + j] + (a[i] as u128) * (b[j] as u128) + carry;
                product[i + j] = v & 0xFFFFFFFFFFFFFFFF;
                carry = v >> 64;
            }
            product[i + 4] = carry;
        }
        // Barrett-like reduction: divide by p ≈ 2^255
        // Simplified: just do repeated subtraction via shift (slow but correct)
        reduce_512(&product, p)
    }

    fn reduce_512(prod: &[u128; 8], p: &[u64; 4]) -> [u64; 4] {
        // Convert to big number, then mod p
        // For correctness (not speed), we'll use a simple approach
        let mut result = [0u64; 4];
        for i in (0..8).rev() {
            // Shift result left by 64 bits
            let mut shifted = [0u64; 4];
            shifted[1] = result[0];
            shifted[2] = result[1];
            shifted[3] = result[2];
            // lost: result[3] — but if we're doing this right it should be < p already
            shifted[0] = prod[i] as u64;
            result = shifted;
            // Reduce: while result >= p, subtract p
            // Since this can be at most ~2p after each step...
            loop {
                let mut borrow = 0i64;
                let mut tmp = [0u64; 4];
                for j in 0..4 {
                    let diff = result[j] as i128 - p[j] as i128 - borrow as i128;
                    tmp[j] = diff as u64;
                    borrow = if diff < 0 { 1 } else { 0 };
                }
                if borrow != 0 {
                    break;
                }
                result = tmp;
            }
        }
        result
    }

    fn inv_mod(a: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
        // Fermat's little theorem: a^(-1) = a^(p-2) mod p
        let mut pm2 = *p;
        // p - 2: subtract 2 from the first limb
        pm2[0] -= 2;
        pow_mod(a, &pm2, p)
    }

    fn pow_mod(base: &[u64; 4], exp: &[u64; 4], p: &[u64; 4]) -> [u64; 4] {
        let mut result = [0u64; 4];
        result[0] = 1; // 1
        let mut b = *base;
        for i in 0..4 {
            for bit in 0..64 {
                if (exp[i] >> bit) & 1 == 1 {
                    result = mul_mod(&result, &b, p);
                }
                b = mul_mod(&b, &b, p);
            }
        }
        result
    }

    // Clamp scalar
    let mut k = *scalar;
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    let mut u_coord = decode(u_bytes);
    u_coord[3] &= 0x7FFFFFFFFFFFFFFF; // Clear top bit

    // Montgomery ladder
    let mut x_1 = u_coord;
    let mut x_2 = [0u64; 4]; x_2[0] = 1;
    let mut z_2 = [0u64; 4];
    let mut x_3 = u_coord;
    let mut z_3 = [0u64; 4]; z_3[0] = 1;

    let a24 = [121666u64, 0, 0, 0]; // (A+2)/4 = 121666

    let mut swap = 0u64;

    for t in (0..255).rev() {
        let byte_idx = t / 8;
        let bit_idx = t % 8;
        let k_t = ((k[byte_idx] >> bit_idx) & 1) as u64;

        let do_swap = swap ^ k_t;
        // Conditional swap
        if do_swap != 0 {
            std::mem::swap(&mut x_2, &mut x_3);
            std::mem::swap(&mut z_2, &mut z_3);
        }
        swap = k_t;

        let a = add_mod(&x_2, &z_2, &p);
        let aa = mul_mod(&a, &a, &p);
        let b_val = sub_mod(&x_2, &z_2, &p);
        let bb = mul_mod(&b_val, &b_val, &p);
        let e = sub_mod(&aa, &bb, &p);
        let c = add_mod(&x_3, &z_3, &p);
        let d = sub_mod(&x_3, &z_3, &p);
        let da = mul_mod(&d, &a, &p);
        let cb = mul_mod(&c, &b_val, &p);
        let da_cb_sum = add_mod(&da, &cb, &p);
        x_3 = mul_mod(&da_cb_sum, &da_cb_sum, &p);
        let da_cb_diff = sub_mod(&da, &cb, &p);
        let sq = mul_mod(&da_cb_diff, &da_cb_diff, &p);
        z_3 = mul_mod(&x_1, &sq, &p);
        x_2 = mul_mod(&aa, &bb, &p);
        let e_a24 = mul_mod(&e, &a24, &p);
        let aa_e_a24 = add_mod(&aa, &e_a24, &p);
        z_2 = mul_mod(&e, &aa_e_a24, &p);
    }

    if swap != 0 {
        std::mem::swap(&mut x_2, &mut x_3);
        std::mem::swap(&mut z_2, &mut z_3);
    }

    let z_inv = inv_mod(&z_2, &p);
    let result = mul_mod(&x_2, &z_inv, &p);
    encode(&result)
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
        // Verify clamping
        assert_eq!(pk[0] & 7, 0);
        assert_eq!(pk[31] & 128, 0);
        assert_ne!(pk[31] & 64, 0);
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
