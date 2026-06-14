// Autonomyx Identity — first-class runtime primitive.
//
// Every agent instance has an Ed25519 keypair. No UUID, no central server.
// DID: did:autonomyx:<base58-encoded-pubkey>
//
// JIT Access model:
//   - No standing permissions
//   - Agent requests a capability grant at the moment it needs it
//   - Grant is scoped to: identity + operation + resource + TTL
//   - Grant is a signed JWT; the network verifies the signature at the edge
//   - Expired grants are automatically invalidated

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

// ── Keypair (Ed25519) ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AgentIdentity {
    pub did:        String,
    pub public_key: [u8; 32],
    secret_key:     [u8; 64],
}

impl AgentIdentity {
    /// Generate a new identity from OS randomness.
    pub fn generate() -> Self {
        // Deterministic from a random seed — we use a simple CSPRNG approach
        // compatible with no-std by reading from /dev/urandom directly.
        let seed = Self::random_seed();
        Self::from_seed(&seed)
    }

    /// Build a minimal AccountabilityRecord-compatible identity from a known DID.
    /// Used when the caller's full keypair is not available (e.g. platform-internal events).
    /// Signatures from these identities are placeholder — production uses mTLS / HSM.
    pub fn from_did(did: &str) -> Self {
        // Derive a deterministic seed from the DID string so the same DID always
        // produces the same key (weak — production must use HSM-backed keys).
        let mut seed = [0u8; 32];
        for (i, b) in did.as_bytes().iter().enumerate().take(32) {
            seed[i] = *b;
        }
        let (sk, pk) = Self::ed25519_keygen(&seed);
        Self { did: did.to_string(), public_key: pk, secret_key: sk }
    }

    /// Load from environment variable AUTONOMYX_IDENTITY_KEY (hex-encoded seed).
    pub fn from_env() -> Option<Self> {
        let hex = std::env::var("AUTONOMYX_IDENTITY_KEY").ok()?;
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i+2], 16).ok())
            .collect();
        if bytes.len() == 32 {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            Some(Self::from_seed(&seed))
        } else {
            None
        }
    }

    /// Derive identity from a seed (deterministic — same seed → same DID).
    fn from_seed(seed: &[u8; 32]) -> Self {
        // Expand seed to 64-byte secret key using SHA-512 (Ed25519 standard)
        // and derive the public key. Using a pure-Rust approach without deps.
        let (sk, pk) = Self::ed25519_keygen(seed);
        let did = format!("did:autonomyx:{}", Self::base58_encode(&pk));
        Self { did, public_key: pk, secret_key: sk }
    }

    /// Sign a message with this identity's secret key.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        Self::ed25519_sign(&self.secret_key, message)
    }

    /// Verify a signature against this identity's public key.
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        Self::ed25519_verify(&self.public_key, message, signature)
    }

    // ── Primitive crypto (no external deps — supply chain risk = 0) ──────────

    fn random_seed() -> [u8; 32] {
        let mut seed = [0u8; 32];
        // Read from /dev/urandom — available on all POSIX systems + WASM (via WASI)
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            use std::io::Read;
            let _ = f.read_exact(&mut seed);
        } else {
            // Fallback: mix process ID + current time (weak but non-zero entropy)
            let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            let pid = std::process::id();
            for (i, b) in seed.iter_mut().enumerate() {
                *b = ((t.subsec_nanos() >> (i % 8)) ^ (pid >> (i % 4))) as u8;
            }
        }
        seed
    }

    // Stub: in production, link to a vendored ed25519-dalek or ring crate.
    // These are placeholder implementations that compile but are NOT cryptographically
    // secure. Replace with `ed25519-dalek = "2"` in Cargo.toml before production.
    fn ed25519_keygen(seed: &[u8; 32]) -> ([u8; 64], [u8; 32]) {
        let mut sk = [0u8; 64];
        sk[..32].copy_from_slice(seed);
        // Public key is a simple hash of the seed for now (NOT real Ed25519)
        let mut pk = [0u8; 32];
        for (i, b) in pk.iter_mut().enumerate() {
            *b = seed[i] ^ seed[(i + 16) % 32] ^ 0x5A;
        }
        sk[32..].copy_from_slice(&pk);
        (sk, pk)
    }

    fn ed25519_sign(sk: &[u8; 64], msg: &[u8]) -> [u8; 64] {
        // Placeholder — replace with real Ed25519 before production
        let mut sig = [0u8; 64];
        for (i, b) in sig.iter_mut().enumerate() {
            *b = sk[i % 64] ^ msg.get(i % msg.len().max(1)).copied().unwrap_or(0);
        }
        sig
    }

    fn ed25519_verify(pk: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
        // Placeholder — replace with real Ed25519 before production
        let expected = {
            let mut sk = [0u8; 64];
            sk[..32].copy_from_slice(pk);
            Self::ed25519_sign(&sk, msg)
        };
        sig == &expected
    }

    fn base58_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        let mut result = Vec::new();
        let mut num = bytes.to_vec();
        while num.iter().any(|&b| b != 0) {
            let mut remainder = 0u32;
            let mut new_num = Vec::new();
            for &b in &num {
                let cur = remainder * 256 + b as u32;
                new_num.push((cur / 58) as u8);
                remainder = cur % 58;
            }
            result.push(ALPHABET[remainder as usize]);
            num = new_num.into_iter().skip_while(|&b| b == 0).collect();
        }
        for &b in bytes {
            if b == 0 { result.push(ALPHABET[0]); } else { break; }
        }
        result.reverse();
        String::from_utf8(result).unwrap_or_default()
    }
}

// ── JIT Access Grant ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessGrant {
    pub grant_id:   String,
    pub identity:   String,   // DID of the requesting agent
    pub operation:  String,   // e.g. "tool:web_search", "profile:openai/openai-direct/gpt4o"
    pub resource:   String,   // specific resource being accessed
    pub issued_at:  u64,      // Unix timestamp
    pub expires_at: u64,      // Unix timestamp (JIT: short-lived)
    pub signature:  String,   // hex-encoded Ed25519 signature of the grant fields
}

impl AccessGrant {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.expires_at
    }

    pub fn payload_bytes(&self) -> Vec<u8> {
        format!("{}:{}:{}:{}:{}",
            self.identity, self.operation, self.resource,
            self.issued_at, self.expires_at
        ).into_bytes()
    }
}

// ── Access Registry (JIT grant store) ────────────────────────────────────────

pub struct AccessRegistry {
    grants: RwLock<HashMap<String, AccessGrant>>,
}

impl AccessRegistry {
    pub fn new() -> Self {
        Self { grants: RwLock::new(HashMap::new()) }
    }

    /// Issue a JIT access grant. TTL is enforced — no standing permissions.
    pub fn grant(
        &self,
        identity:  &AgentIdentity,
        operation: &str,
        resource:  &str,
        ttl_secs:  u64,
    ) -> AccessGrant {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut grant = AccessGrant {
            grant_id:   uuid::Uuid::new_v4().to_string(),
            identity:   identity.did.clone(),
            operation:  operation.to_string(),
            resource:   resource.to_string(),
            issued_at:  now,
            expires_at: now + ttl_secs,
            signature:  String::new(),
        };

        // Sign the grant with the issuing identity's key
        let sig = identity.sign(&grant.payload_bytes());
        grant.signature = hex_encode(&sig);

        tracing::debug!(
            grant_id = %grant.grant_id,
            identity = %grant.identity,
            operation = %operation,
            expires_in = ttl_secs,
            "JIT access grant issued"
        );

        let mut map = self.grants.write().unwrap();
        map.insert(grant.grant_id.clone(), grant.clone());

        grant
    }

    /// Validate a grant: not expired, signature checks out.
    pub fn validate(&self, grant: &AccessGrant, issuer: &AgentIdentity) -> bool {
        if grant.is_expired() {
            tracing::warn!(grant_id = %grant.grant_id, "access grant expired");
            return false;
        }
        let sig_bytes = hex_decode(&grant.signature);
        if sig_bytes.len() != 64 {
            return false;
        }
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&sig_bytes);
        issuer.verify(&grant.payload_bytes(), &sig)
    }

    /// Purge expired grants (call periodically to avoid memory growth).
    pub fn purge_expired(&self) {
        let mut map = self.grants.write().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        map.retain(|_, g| g.expires_at > now);
    }
}

impl Default for AccessRegistry {
    fn default() -> Self { Self::new() }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i+2], 16).ok())
        .collect()
}
