// Autonomyx Identity — first-class language primitive.
//
// Identity is NOT an afterthought. Every agent, peer, and infra node
// has a cryptographic identity. No central identity server. No UUID.
//
// Model:
//   - Ed25519 keypair generated on first start (stored in /workspace/.identity)
//   - DID: did:autonomyx:<base58-pubkey>
//   - All inter-agent messages are signed by the sender's identity
//   - Peer trust is established by presenting and verifying DIDs
//   - mTLS certificates are derived from the agent's keypair
//
// Schema.org grounding:
//   Identity → schema:Person (for agents) / schema:Organization (for peers)
//   Credential → schema:DigitalDocument / schema:QualitativeValue
//   Trust → schema:Offer (what capabilities this identity grants)

// ── Identity block (first-class primitive) ────────────────────────────────────
Identity:
    (schema='@schema' '(' schemaType=STRING ')')?
    'identity' name=ID '{'
        ('did:'        did=STRING)?          // did:autonomyx:<pubkey> — auto-generated if omitted
        ('key_type:'   keyType=KeyType)?
        ('storage:'    storage=IdentityStorage)?
        ('rotate:'     rotatePolicy=RotatePolicy)?
        credentials+=Credential*
        trust+=TrustRule*
    '}';

KeyType returns string:
    'ed25519'    // default — fast, small signatures, widely supported
    | 'p256'     // NIST P-256 — required for some enterprise PKI chains
    | 'rsa2048'; // legacy compat only — avoid for new agents

IdentityStorage returns string:
    'filesystem'   // /workspace/.identity/<name>.key (default for server/desktop)
    | 'env'        // IDENTITY_<NAME>_KEY env var (containers, GitOps)
    | 'k8s_secret' // Kubernetes Secret (GitOps — never committed to git)
    | 'hsm'        // Hardware Security Module (production edge nodes)
    | 'tee';       // Trusted Execution Environment

RotatePolicy:
    '{' 'interval:' interval=Duration ('on_peer_request:' onPeer=BOOLEAN)? '}';

// ── Verifiable Credential ─────────────────────────────────────────────────────
Credential:
    'credential' name=ID '{'
        'type:'    credType=CredentialType
        ('issuer:' issuer=STRING)?    // DID of the issuing identity (self-issued if omitted)
        ('scope:'  '[' scopes+=STRING (',' scopes+=STRING)* ']')?
        ('ttl:'    ttl=Duration)?
    '}';

CredentialType returns string:
    'agent_capability'    // this agent can use specific tools/workflows
    | 'operator_access'   // access to a specific operator's profiles
    | 'peer_trust'        // bilateral trust with another peer
    | 'infra_access'      // permission to deploy to an infra target
    | 'data_access';      // access to a shared dataset/vector store

// ── Trust rules — who this identity trusts ────────────────────────────────────
TrustRule:
    'trust' peer=STRING  // DID of the trusted peer
    ('{'
        ('capabilities:' '[' caps+=STRING (',' caps+=STRING)* ']')?
        ('expires:'      expires=Duration)?
        ('require_mtls:' mtls=BOOLEAN)?
    '}')?;

// ── Duration ──────────────────────────────────────────────────────────────────
Duration returns string:
    NUMBER ('s' | 'ms' | 'm' | 'h' | 'd');

terminal ID:      /[_a-zA-Z][\w-]*/;
terminal STRING:  /"[^"]*"|'[^']*'/;
terminal NUMBER:  /[0-9]+(\.[0-9]+)?/;
terminal BOOLEAN: /true|false/;
