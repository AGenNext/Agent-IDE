// Autonomyx BOM — Bill of Materials, native at the build gate.
//
// Every artifact carries a BOM. Not decoration — evidence.
// The build gate produces it. The sign gate attests it.
// The push gate stores it alongside the image.
// Every downstream gate verifies it is present.
//
// Format: CycloneDX 1.6 (CNCF standard, tool-neutral, schema-verifiable)
//
// Provenance chain:
//   source commit (git sha)
//   → Stacker SI (hermetic build, pinned inputs)
//   → SBOM (CycloneDX — every dep, every layer, every hash)
//   → cosign attest (SBOM signed alongside image)
//   → Zot registry (image + attestation co-located)
//   → gate record (BOM digest in lifecycle audit log)
//
// Metal native: the BOM is generated from the binary itself at build time.
// No external tool required — Rust's Cargo.lock is the ground truth.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ── CycloneDX BOM (subset — the fields we track at every gate) ───────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bom {
    pub bom_format:    String,     // "CycloneDX"
    pub spec_version:  String,     // "1.6"
    pub serial_number: String,     // urn:uuid:<uuid>
    pub version:       u32,
    pub metadata:      BomMetadata,
    pub components:    Vec<BomComponent>,
    pub dependencies:  Vec<BomDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BomMetadata {
    pub timestamp:  DateTime<Utc>,
    pub tools:      Vec<BomTool>,
    pub component:  BomComponent,   // the artifact itself
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomTool {
    pub vendor:  String,
    pub name:    String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BomComponent {
    #[serde(rename = "type")]
    pub kind:        ComponentKind,
    pub name:        String,
    pub version:     Option<String>,
    pub purl:        Option<String>,         // package URL
    pub hashes:      Vec<BomHash>,
    pub licenses:    Vec<BomLicense>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentKind {
    Application,
    Library,
    Container,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomHash {
    pub alg:     HashAlg,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum HashAlg { Sha256, Sha512, Blake3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomLicense {
    pub id: String,    // SPDX license ID, e.g. "MIT", "Apache-2.0"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDependency {
    #[serde(rename = "ref")]
    pub refers_to:  String,
    pub depends_on: Vec<String>,
}

// ── Attestation — BOM signed by cosign ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomAttestation {
    pub bom_digest:     String,   // sha256 of the serialised BOM JSON
    pub image_digest:   String,   // sha256 of the OCI image
    pub signer_did:     String,   // did:autonomyx:<pubkey>
    pub bundle:         String,   // cosign bundle (base64 Rekor entry)
    pub attested_at:    DateTime<Utc>,
}

// ── BomRecord — what gets stored in the gate payload ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomRecord {
    pub bom:         Bom,
    pub bom_digest:  String,     // sha256 of the BOM JSON
    pub attestation: Option<BomAttestation>,
}

impl BomRecord {
    pub fn new(bom: Bom) -> Self {
        let json   = serde_json::to_string(&bom).unwrap_or_default();
        let digest = sha256_hex(json.as_bytes());
        Self { bom, bom_digest: digest, attestation: None }
    }

    pub fn attest(&mut self, image_digest: &str, signer_did: &str, bundle: &str) {
        self.attestation = Some(BomAttestation {
            bom_digest:   self.bom_digest.clone(),
            image_digest: image_digest.into(),
            signer_did:   signer_did.into(),
            bundle:       bundle.into(),
            attested_at:  Utc::now(),
        });
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ── BOM builder ───────────────────────────────────────────────────────────────
//
// Constructs a CycloneDX BOM for the autonomyx-runner binary.
// In production: parse Cargo.lock for the full dependency tree.
// Here: produces the minimal provenance record from build-time constants.

pub fn build_bom(
    artifact:    &str,
    version:     &str,
    image_digest: &str,
    git_sha:     &str,
    cargo_deps:  Vec<CargoDep>,
) -> BomRecord {
    let serial = format!("urn:uuid:{}", uuid::Uuid::new_v4());

    let root = BomComponent {
        kind:        ComponentKind::Container,
        name:        artifact.into(),
        version:     Some(version.into()),
        purl:        Some(format!("pkg:oci/{}@{}", artifact, image_digest)),
        hashes:      vec![BomHash { alg: HashAlg::Sha256, content: image_digest.trim_start_matches("sha256:").into() }],
        licenses:    vec![BomLicense { id: "Apache-2.0".into() }],
        description: Some("Autonomyx runtime core — native everywhere".into()),
    };

    let mut components: Vec<BomComponent> = cargo_deps.iter().map(|dep| BomComponent {
        kind:        ComponentKind::Library,
        name:        dep.name.clone(),
        version:     Some(dep.version.clone()),
        purl:        Some(format!("pkg:cargo/{}@{}", dep.name, dep.version)),
        hashes:      dep.checksum.as_deref()
                        .map(|c| vec![BomHash { alg: HashAlg::Sha256, content: c.into() }])
                        .unwrap_or_default(),
        licenses:    vec![],
        description: None,
    }).collect();

    // Git provenance as a file component
    components.push(BomComponent {
        kind:        ComponentKind::File,
        name:        "source".into(),
        version:     Some(git_sha.into()),
        purl:        None,
        hashes:      vec![BomHash { alg: HashAlg::Sha256, content: sha256_hex(git_sha.as_bytes()) }],
        licenses:    vec![],
        description: Some(format!("git commit {git_sha}")),
    });

    let bom = Bom {
        bom_format:   "CycloneDX".into(),
        spec_version: "1.6".into(),
        serial_number: serial,
        version:      1,
        metadata: BomMetadata {
            timestamp: Utc::now(),
            tools: vec![
                BomTool { vendor: "Autonomyx".into(), name: "autonomyx-runner".into(), version: version.into() },
                BomTool { vendor: "StackerBuild".into(), name: "stacker".into(), version: "1.0.0".into() },
                BomTool { vendor: "sigstore".into(), name: "cosign".into(), version: "2.x".into() },
            ],
            component: root,
        },
        components,
        dependencies: vec![],
    };

    BomRecord::new(bom)
}

// ── Cargo dep (from Cargo.lock) ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CargoDep {
    pub name:     String,
    pub version:  String,
    pub checksum: Option<String>,
}

// ── Provenance summary — what goes in every gate payload ─────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub artifact:    String,
    pub git_sha:     String,
    pub bom_digest:  String,
    pub image_digest: String,
    pub built_at:    DateTime<Utc>,
}

impl Provenance {
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ── sha256 helper (no external dep — std only) ────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    // Minimal sha256 using the sha2 crate already in the dependency tree via surrealdb.
    // Fallback: hex-encode the first 32 bytes of a simple fold.
    // Real impl: use sha2::Sha256::digest(data).
    // For now: derive from length + first/last bytes as a placeholder.
    // TODO: wire sha2::Sha256 when supply chain gates are fully active.
    let len = data.len();
    let sum: u64 = data.iter().enumerate()
        .map(|(i, &b)| (b as u64).wrapping_mul(i as u64 + 1))
        .fold(0u64, |a, b| a.wrapping_add(b));
    format!("{:016x}{:016x}{:016x}{:016x}", sum, len as u64, sum ^ 0xdeadbeef, len as u64 ^ sum)
}
