# Autonomyx Autonomous Agent Specification

**Platform:** Autonomyx — openautonomyx.com
**Version:** 0.1.0
**Status:** Living specification

---

## 1. Definition

An **Autonomous Agent** in the Autonomyx platform is an entity that is:

| Property | Definition |
|---|---|
| **Real** | Hardware-backed Ed25519 identity (HSM/TPM). Not a UUID. Not a session token. A cryptographic person. |
| **Unique** | One DID per agent instance: `did:autonomyx:<base58-pubkey>`. Globally unique. Cannot be cloned. |
| **Identifiable** | Resolvable DID Document (W3C DID Core 1.1). Any peer can look up any agent without a central server. |
| **Governed** | GovernancePolicy in the DID Document: max TTL, allowed capabilities, allowed operators. Enforced at every gate. |
| **Autonomous** | Self-sovereign. Holds its own keys. Signs its own claims. Requests JIT access — no standing permissions. |
| **Federal** | Federated across peer nodes. DID resolution: local → peer broadcast → fail. No central authority. |
| **Accountable** | Signed accountability log. Every action recorded, signed by the agent's key. Non-repudiable. |
| **Intelligent** | Intelligence bound in the DID Document: provider + operator + profile + reasoning mode. |

---

## 2. Identity

```
DID:     did:autonomyx:<base58(ed25519_pubkey)>
Keys:    Ed25519 keypair — HSM in production, /dev/urandom in dev
Doc:     W3C DID Document 1.1 — stored in SurrealDB, replicated to peers
Version: bumped on every key rotation or policy change
```

The DID Document is the agent's constitution. It declares:
- Who the agent is (verification methods)
- What it is allowed to do (governance policy)
- How it reasons (intelligence binding)
- Where it runs (service endpoints)

---

## 3. Lifecycle

Every agent artifact moves through eight gates. Each gate is idempotent.
Each gate has an oath — the invariant that must hold before it opens.
The fabric fills the gaps between gates.

```
build → sign → push → sync → deploy → run → observe → feedback
  ↑                                                        |
  └──────────────────── iterate ───────────────────────────┘
```

| Gate | Oath | BOM | Produces |
|---|---|---|---|
| build | artifact_has_digest_and_bom | Generated | OCI image + CycloneDX BOM |
| sign | cosign_image_and_bom_attested | Required | cosign bundle + BOM attestation |
| push | registry_ref_and_bom_stored | Required | Zot registry ref |
| sync | argocd_app_healthy | Verified | GitOps sync confirmed |
| deploy | rollout_ready | Verified | k8s Deployment ready |
| run | run_has_agent | Verified | k8s Job spawned |
| observe | telemetry_emitted | Verified | OTel trace + span |
| feedback | signal_has_source | Verified | Loop closed, iterate |

---

## 4. Access Control

Access is **JIT — Just In Time**. No standing permissions.

```
Agent needs capability
  → requests AccessGrant from issuing identity
  → grant is scoped: identity + operation + resource + TTL
  → grant is Ed25519-signed by the issuer
  → governance policy checked: TTL ≤ max, capability ∈ allowed_caps
  → gate verifies grant at time of use
  → grant expires — never revoked, just runs out
```

Maximum grant TTL: 300 seconds (configurable per DID in governance policy).

---

## 5. Federation

```
Node A registers agent A → local DID registry
Node B registers agent B → local DID registry
A needs to verify B → resolve: local miss → push to peer B → receive doc
B needs to invoke A → transfer push (egress-push only, no inbound ports)
```

Federation is egress-push. No node exposes inbound ports to peers.
The network holds the contract (Istio mTLS STRICT between pods).
The metal box delivers via ArgoCD to N clusters — each cluster is a peer.

---

## 6. Intelligence

An agent's intelligence is declared in its DID Document:

```json
{
  "intelligence": {
    "provider":  "anthropic",
    "operator":  "direct",
    "profile":   "claude-opus-4-8",
    "reasoning": "react"
  }
}
```

Reasoning modes:
- `react` — Reason + Act loop (default). Tool calls interleaved with reasoning.
- `chain` — Chain-of-thought. No external tools.
- `adaptive` — Model decides (Claude extended thinking / o1).
- `direct` — Single-shot. No loop.

The marketplace resolves `provider/operator/profile` → API endpoint + key env var.
The agent never holds the API key — it holds its DID. The platform holds the key.

---

## 7. Accountability

Every action is recorded in a signed log:

```
AccountabilityRecord {
  did:         "did:autonomyx:...",    // who acted
  action:      "tool:web_search",      // what was done
  resource:    "https://...",          // what it was done to
  grant_id:    "uuid",                 // the JIT grant that authorised it
  outcome:     "success",             // success | denied | failed | partial
  evidence:    { gate_record, trace }, // proof
  signature:   "ed25519_hex",         // signed by the agent — non-repudiable
  recorded_at: "2026-06-14T...",
}
```

The log is replicated to SurrealDB. Live queries alert on anomalies.
Every record is signed by the agent's own key — it cannot be forged.

---

## 8. Process Excellence

Pursuing process excellence means:
- Every gate is idempotent — running twice yields the same state
- Every transition is atomic — no partial state leaks
- Every action is signed — accountability at every step
- Every artifact carries a BOM — provenance is always known
- Every config change pushes to agents via SurrealDB live queries — no restart
- Every peer exchange is egress-push — no inbound attack surface
- Every capability is JIT — no standing permissions that can be stolen

---

## 9. Ecosystem Balance

The platform maintains ecosystem balance by:
- Supporting all LLM providers equally (OpenAI, Anthropic, Ollama, any OpenAI-compatible)
- Governance policy per agent — not per platform — operators set their own rules
- CNCF-mature stack — no vendor lock-in at any layer
- Self-hosted Zot registry — the device is the host
- Open specification — this document is the contract

---

## 10. DSL — `.ayx` source of truth

Agents, tools, workflows, gateways, contracts, identities, worlds, and lifecycle
gates are all declared in `.ayx` files (Autonomyx Language — Langium DSL).

The `.ayx` file is:
- Compiled by the Autonomyx Language Server
- Validated by the validator (no API keys in source — hard error)
- Stored in ConfigDB (SurrealDB) as a `config:agent` record
- Pushed to all running agents via SurrealDB live query
- Versioned in git — ArgoCD delivers changes without restart

```ayx
agent MyAgent {
  @schema(SoftwareApplication)
  model "claude-opus-4-8"
  provider anthropic
  reasoning react
  tools [web_search, code_execution]
  worlds [server, edge, k8s]
}
```

---

*Autonomyx is the platform. openautonomyx.com*
*Code is the contract. Gates are the keepers. Oaths are their twins.*
