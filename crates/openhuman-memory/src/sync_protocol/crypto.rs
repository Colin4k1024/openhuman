//! E2E encryption for memory sync — XChaCha20-Poly1305 over X25519.
//!
//! Protocol:
//! 1. Each device has a persistent X25519 keypair (see `device.rs`).
//! 2. On sync session start, both sides perform X25519 key agreement.
//! 3. The shared secret is used to derive a symmetric key via HKDF-SHA256.
//! 4. All change entries are encrypted with XChaCha20-Poly1305 before transit.
//! 5. The relay server sees only ciphertext — cannot read memory content.

use anyhow::{bail, Result};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Nonce size for XChaCha20-Poly1305 (24 bytes).
pub const NONCE_SIZE: usize = 24;
/// Authentication tag size for Poly1305 (16 bytes).
pub const TAG_SIZE: usize = 16;

// Fixed salt for HKDF
static HKDF_SALT: &[u8] = b"openhuman-memory-sync-v1-salt";

// ─── Key Agreement ────────────────────────────────────────────────────────────

/// Perform X25519 Diffie-Hellman key agreement.
///
/// Given our private key and the peer's public key, produce a 32-byte shared secret.
pub fn x25519_key_agreement(our_private: &[u8; 32], their_public: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*our_private);
    let public = PublicKey::from(*their_public);
    let shared = secret.diffie_hellman(&public);
    *shared.as_bytes()
}

/// Derive a symmetric encryption key from the shared secret using HKDF-SHA256.
///
/// `context` is used as the `info` parameter in HKDF-Expand.
pub fn derive_session_key(shared_secret: &[u8; 32], context: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), shared_secret);
    let mut okm = [0u8; 32];
    hk.expand(context, &mut okm)
        .expect("HKDF expand: output length is always valid for SHA-256");
    okm
}

// ─── Authenticated Encryption ─────────────────────────────────────────────────

/// Encrypt plaintext with the given key.
///
/// Output format: `nonce (24 bytes) || ciphertext || tag (16 bytes)`.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    // chacha20poly1305 appends the tag to the ciphertext
    let ciphertext_with_tag = cipher
        .encrypt(&nonce, plaintext)
        .expect("XChaCha20Poly1305 encrypt cannot fail with valid key/nonce");

    let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext_with_tag.len());
    output.extend_from_slice(nonce.as_slice());
    output.extend_from_slice(&ciphertext_with_tag);
    output
}

/// Decrypt ciphertext produced by `encrypt`.
///
/// Verifies the authentication tag; returns error on tampering.
pub fn decrypt(key: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < NONCE_SIZE + TAG_SIZE {
        bail!("ciphertext too short: {} bytes", sealed.len());
    }

    let (nonce_bytes, ciphertext_with_tag) = sealed.split_at(NONCE_SIZE);
    let nonce = XNonce::from_slice(nonce_bytes);
    let cipher = XChaCha20Poly1305::new(key.into());

    cipher
        .decrypt(nonce, ciphertext_with_tag)
        .map_err(|_| anyhow::anyhow!("authentication failed: tag mismatch or corrupt ciphertext"))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [0x42u8; 32];
        let plaintext = b"Hello, this is a secret memory sync payload!";

        let sealed = encrypt(&key, plaintext);
        let decrypted = decrypt(&key, &sealed).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_detects_tampering() {
        let key = [0x42u8; 32];
        let plaintext = b"secret data";

        let mut sealed = encrypt(&key, plaintext);
        // Flip a byte in the ciphertext
        if sealed.len() > NONCE_SIZE + 2 {
            sealed[NONCE_SIZE + 1] ^= 0xFF;
        }

        let result = decrypt(&key, &sealed);
        assert!(result.is_err());
    }

    #[test]
    fn different_keys_cannot_decrypt() {
        let key1 = [0x01u8; 32];
        let key2 = [0x02u8; 32];
        let plaintext = b"only for key1";

        let sealed = encrypt(&key1, plaintext);
        let result = decrypt(&key2, &sealed);
        assert!(result.is_err());
    }

    #[test]
    fn key_agreement_produces_shared_secret() {
        let alice_private = [1u8; 32];
        let bob_private = [2u8; 32];

        let alice_public = super::super::device::x25519_base_point_mul_pub(&alice_private);
        let bob_public = super::super::device::x25519_base_point_mul_pub(&bob_private);

        let shared_ab = x25519_key_agreement(&alice_private, &bob_public);
        let shared_ba = x25519_key_agreement(&bob_private, &alice_public);

        // Both sides must derive the same shared secret
        assert_eq!(shared_ab, shared_ba);
    }

    #[test]
    fn derive_session_key_is_deterministic() {
        let secret = [0x99u8; 32];
        let context = b"test-session-1";

        let k1 = derive_session_key(&secret, context);
        let k2 = derive_session_key(&secret, context);
        assert_eq!(k1, k2);
    }

    #[test]
    fn derive_session_key_differs_by_context() {
        let secret = [0x99u8; 32];
        let k1 = derive_session_key(&secret, b"context-a");
        let k2 = derive_session_key(&secret, b"context-b");
        assert_ne!(k1, k2);
    }

    #[test]
    fn short_ciphertext_is_rejected() {
        let key = [0u8; 32];
        let result = decrypt(&key, &[1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn large_payload_roundtrip() {
        let key = [0xABu8; 32];
        let plaintext: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();

        let sealed = encrypt(&key, &plaintext);
        let decrypted = decrypt(&key, &sealed).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
