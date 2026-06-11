//! Sync transport — trait and implementations for pushing/pulling changes.
//!
//! Three transport strategies:
//! - `LanTransport`: HTTP over mDNS-discovered LAN peer.
//! - `RelayTransport`: via backend socket.io relay (E2E encrypted).
//! - `DirectTransport`: direct TCP on the same subnet.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::changelog::ChangeEntry;

// ─── Trait ───────────────────────────────────────────────────────────────────

/// A transport capable of exchanging change entries with a remote peer.
#[async_trait]
pub trait SyncTransport: Send + Sync {
    /// Push local changes to the remote peer.
    async fn push_changes(&self, entries: &[ChangeEntry]) -> Result<PushResult>;

    /// Pull changes from the remote peer since the given Lamport timestamp.
    async fn pull_changes(&self, since_ts: u64) -> Result<Vec<ChangeEntry>>;

    /// Check if the remote peer is reachable.
    async fn is_available(&self) -> bool;

    /// Human-readable name of this transport.
    fn name(&self) -> &str;
}

/// Result of a push operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushResult {
    /// Number of entries accepted by the remote.
    pub accepted: usize,
    /// Number of entries rejected (e.g., duplicates).
    pub rejected: usize,
    /// Remote's current Lamport clock after merge.
    pub remote_clock: u64,
}

// ─── LAN Transport ──────────────────────────────────────────────────────────

/// HTTP transport over local network (mDNS-discovered peer).
pub struct LanTransport {
    /// Base URL of the peer's memory server (e.g., `http://192.168.1.50:7891`).
    pub peer_url: String,
    /// Our device ID for identification in requests.
    pub device_id: String,
}

#[async_trait]
impl SyncTransport for LanTransport {
    async fn push_changes(&self, entries: &[ChangeEntry]) -> Result<PushResult> {
        let client = reqwest::Client::new();
        let url = format!("{}/sync/push", self.peer_url);
        let body = serde_json::json!({
            "device_id": self.device_id,
            "entries": entries,
        });

        let resp = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        let result: PushResult = resp.json().await?;
        Ok(result)
    }

    async fn pull_changes(&self, since_ts: u64) -> Result<Vec<ChangeEntry>> {
        let client = reqwest::Client::new();
        let url = format!("{}/sync/pull?since_ts={}&device_id={}", self.peer_url, since_ts, self.device_id);

        let resp = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        let entries: Vec<ChangeEntry> = resp.json().await?;
        Ok(entries)
    }

    async fn is_available(&self) -> bool {
        let client = reqwest::Client::new();
        let url = format!("{}/sync/ping", self.peer_url);
        client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .is_ok()
    }

    fn name(&self) -> &str {
        "lan"
    }
}

// ─── Relay Transport ────────────────────────────────────────────────────────

/// Transport via backend relay (for when peers are on different networks).
/// Entries are E2E encrypted before transmission.
pub struct RelayTransport {
    /// Backend relay URL.
    pub relay_url: String,
    /// Our device ID.
    pub device_id: String,
    /// Target device ID to sync with.
    pub target_device_id: String,
    /// Shared secret (derived from X25519 key agreement). Used for E2E encryption.
    pub shared_secret: [u8; 32],
}

#[async_trait]
impl SyncTransport for RelayTransport {
    async fn push_changes(&self, entries: &[ChangeEntry]) -> Result<PushResult> {
        let payload = serde_json::to_vec(entries)?;
        let encrypted = encrypt_payload(&self.shared_secret, &payload);

        let client = reqwest::Client::new();
        let url = format!("{}/relay/memory/push", self.relay_url);
        let body = serde_json::json!({
            "from_device": self.device_id,
            "to_device": self.target_device_id,
            "payload": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &encrypted),
        });

        let resp = client.post(&url).json(&body).send().await?;
        let result: PushResult = resp.json().await?;
        Ok(result)
    }

    async fn pull_changes(&self, since_ts: u64) -> Result<Vec<ChangeEntry>> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/relay/memory/pull?from_device={}&to_device={}&since_ts={}",
            self.relay_url, self.target_device_id, self.device_id, since_ts
        );

        let resp = client.get(&url).send().await?;
        let envelope: RelayEnvelope = resp.json().await?;

        let encrypted = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &envelope.payload)?;
        let decrypted = decrypt_payload(&self.shared_secret, &encrypted)?;
        let entries: Vec<ChangeEntry> = serde_json::from_slice(&decrypted)?;
        Ok(entries)
    }

    async fn is_available(&self) -> bool {
        let client = reqwest::Client::new();
        let url = format!("{}/relay/health", self.relay_url);
        client.get(&url).send().await.is_ok()
    }

    fn name(&self) -> &str {
        "relay"
    }
}

#[derive(Deserialize)]
struct RelayEnvelope {
    payload: String,
}

// ─── Direct Transport ───────────────────────────────────────────────────────

/// Direct TCP transport for two devices on the same subnet.
pub struct DirectTransport {
    /// Peer address (e.g., `192.168.1.50:7892`).
    pub peer_address: String,
    /// Our device ID.
    pub device_id: String,
}

#[async_trait]
impl SyncTransport for DirectTransport {
    async fn push_changes(&self, entries: &[ChangeEntry]) -> Result<PushResult> {
        // Direct transport uses the same HTTP protocol as LAN but on a fixed port
        let lan = LanTransport {
            peer_url: format!("http://{}", self.peer_address),
            device_id: self.device_id.clone(),
        };
        lan.push_changes(entries).await
    }

    async fn pull_changes(&self, since_ts: u64) -> Result<Vec<ChangeEntry>> {
        let lan = LanTransport {
            peer_url: format!("http://{}", self.peer_address),
            device_id: self.device_id.clone(),
        };
        lan.pull_changes(since_ts).await
    }

    async fn is_available(&self) -> bool {
        let lan = LanTransport {
            peer_url: format!("http://{}", self.peer_address),
            device_id: self.device_id.clone(),
        };
        lan.is_available().await
    }

    fn name(&self) -> &str {
        "direct"
    }
}

// ─── Encryption helpers ─────────────────────────────────────────────────────

/// Simple XOR-based encryption placeholder.
/// In production, use XChaCha20-Poly1305 (already in the project's tunnel crypto).
fn encrypt_payload(key: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    // Prepend a 24-byte random nonce
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce);

    let mut output = Vec::with_capacity(24 + plaintext.len());
    output.extend_from_slice(&nonce);

    // XOR with key-derived stream (placeholder — replace with XChaCha20-Poly1305)
    for (i, byte) in plaintext.iter().enumerate() {
        output.push(byte ^ key[i % 32] ^ nonce[i % 24]);
    }
    output
}

fn decrypt_payload(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < 24 {
        anyhow::bail!("ciphertext too short");
    }
    let nonce = &ciphertext[..24];
    let encrypted = &ciphertext[24..];

    let mut output = Vec::with_capacity(encrypted.len());
    for (i, byte) in encrypted.iter().enumerate() {
        output.push(byte ^ key[i % 32] ^ nonce[i % 24]);
    }
    Ok(output)
}

use rand::RngCore;

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let plaintext = b"hello world, this is a sync payload";

        let encrypted = encrypt_payload(&key, plaintext);
        let decrypted = decrypt_payload(&key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_rejects_short_input() {
        let key = [0u8; 32];
        let result = decrypt_payload(&key, &[1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn lan_transport_has_correct_name() {
        let t = LanTransport {
            peer_url: "http://localhost:7891".into(),
            device_id: "test".into(),
        };
        assert_eq!(t.name(), "lan");
    }
}
