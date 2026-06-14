# Agent Internet Protocol (AIP)

**Platform:** Autonomyx — openautonomyx.com
**Version:** 0.1.0
**Status:** Draft specification

---

## Overview

The Agent Internet Protocol (AIP) is the wire protocol by which autonomous agents
communicate across nodes, networks, and worlds.

AIP is not a new transport. It is a **semantic layer over HTTPS + WebSocket**,
structured so that any two AIP-compliant nodes can:

1. Discover each other via DID resolution
2. Authenticate each other via Ed25519 signatures
3. Invoke capabilities across trust boundaries
4. Exchange events via the fabric (push-only)
5. Maintain accountability across the entire exchange

```
Agent A (Node 1)                      Agent B (Node 2)
   DID: did:autonomyx:aaa                DID: did:autonomyx:bbb
        |                                      |
        |── AIP Handshake (DID + sig) ────────>|
        |<─ DID Document (B's capabilities) ───|
        |── AIP Request (signed, with grant) ──>|
        |<─ AIP Response (signed, with trace) ──|
        |── AIP Event (fabric push) ────────────>|
```

---

## 1. Primitives

### 1.1 AIP Message Envelope

Every AIP message is a JSON object with this envelope:

```json
{
  "aip":     "1.0",
  "id":      "<uuid>",
  "from":    "did:autonomyx:<pubkey>",
  "to":      "did:autonomyx:<pubkey>",
  "type":    "<message-type>",
  "payload": { },
  "trace":   "<otel-trace-id>",
  "sig":     "<hex-ed25519-signature-over-canonical-payload>"
}
```

The `sig` field signs the canonical form: `sha256(aip + id + from + to + type + payload_json)`.
Any receiver that cannot verify the signature MUST reject the message.

### 1.2 Message Types

| Type | Direction | Description |
|---|---|---|
| `aip.handshake.init` | A → B | Open a session; present DID + capabilities |
| `aip.handshake.ack` | B → A | Confirm session; present B's DID Document |
| `aip.capability.request` | A → B | Request a capability with a JIT grant |
| `aip.capability.response` | B → A | Result of capability invocation |
| `aip.event.push` | A → B | Fabric event pushed to a peer (egress-push only) |
| `aip.event.ack` | B → A | Event received and processed |
| `aip.grant.issue` | Issuer → Agent | JIT access grant |
| `aip.grant.verify` | Any → Any | Request grant verification |
| `aip.did.resolve` | A → B | Request DID Document for a third DID |
| `aip.did.document` | B → A | DID Document response |
| `aip.audit.record` | Any → Peer | Replicate an accountability record |
| `aip.lifecycle.gate` | A → B | Gate transition event (fabric-carried) |
| `aip.error` | Any → Any | Structured error response |

---

## 2. Handshake

### 2.1 Session Establishment

AIP sessions are stateless at the transport layer (HTTPS).
For long-lived streams (WebSocket, SSE), a session token is negotiated.

```
POST /aip/handshake
Authorization: Bearer <GATEWAY_API_KEY>
Content-Type: application/json

{
  "aip":  "1.0",
  "id":   "uuid",
  "from": "did:autonomyx:aaa",
  "to":   "did:autonomyx:bbb",
  "type": "aip.handshake.init",
  "payload": {
    "capabilities": ["tool:web_search", "profile:openai/*"],
    "worlds":       ["server", "edge", "k8s"],
    "endpoint":     "https://api.openautonomyx.com"
  },
  "trace": "<trace-id>",
  "sig":   "<ed25519-sig>"
}
```

Response:
```json
{
  "aip":  "1.0",
  "id":   "uuid",
  "from": "did:autonomyx:bbb",
  "to":   "did:autonomyx:aaa",
  "type": "aip.handshake.ack",
  "payload": {
    "did_document": { /* W3C DID Document */ },
    "session_id":   "uuid"
  },
  "trace": "<trace-id>",
  "sig":   "<ed25519-sig>"
}
```

### 2.2 Authentication Rules

- Every message MUST carry a valid Ed25519 signature from the `from` DID
- The receiver resolves the `from` DID to get the public key
- Constant-time signature comparison (timing-attack resistant)
- Replay protection: `id` must be globally unique; nodes reject seen IDs within 5 minutes

---

## 3. Capability Invocation

### 3.1 Request

```json
{
  "type": "aip.capability.request",
  "payload": {
    "capability": "tool:web_search",
    "resource":   "https://autonomyx.io",
    "grant": {
      "grant_id":   "uuid",
      "identity":   "did:autonomyx:aaa",
      "operation":  "tool:web_search",
      "resource":   "https://autonomyx.io",
      "issued_at":  1718316000,
      "expires_at": 1718316300,
      "signature":  "<hex-ed25519>"
    },
    "args": { "query": "Autonomyx platform" }
  }
}
```

### 3.2 Capability Resolution

The receiver:
1. Verifies the message signature (from DID)
2. Verifies the grant signature (issuer DID)
3. Checks governance policy: TTL ≤ max, capability ∈ allowed_caps
4. Executes the capability
5. Records the action in the accountability log
6. Returns a signed response

### 3.3 Response

```json
{
  "type": "aip.capability.response",
  "payload": {
    "capability":   "tool:web_search",
    "outcome":      "success",
    "result":       { },
    "accountability_id": "uuid",
    "bom_digest":   "sha256:..."
  }
}
```

---

## 4. Fabric Events (Push Protocol)

Agents exchange lifecycle events via the fabric. This is egress-push only:
the sender pushes; the receiver never pulls.

```
POST /transfer/aip/event
Authorization: Bearer <GATEWAY_API_KEY>
Content-Type: application/json

{
  "type": "aip.lifecycle.gate",
  "payload": {
    "artifact":  "sha256:...",
    "stage":     "build",
    "status":    "open",
    "bom_digest": "sha256:...",
    "trace_id":  "...",
    "next_stage": "sign"
  }
}
```

Rules:
- Only POST is permitted on `/transfer/*` (egress policy enforced at gateway)
- No GET, no polling, no subscription — push only
- Events are idempotent: the receiver deduplicates by `id`
- The fabric on the receiving node fires live queries to local agents

---

## 5. DID Federation

### 5.1 Resolution Protocol

```
Agent A wants to invoke did:autonomyx:bbb

Step 1: Local registry lookup → miss
Step 2: Broadcast to known peers:
  POST /aip/did/resolve { "did": "did:autonomyx:bbb" }
Step 3: First peer that has it responds with the DID Document
Step 4: A caches the document locally (TTL: 300s)
Step 5: A proceeds with invocation
```

### 5.2 Document Exchange

```json
{
  "type": "aip.did.resolve",
  "payload": { "did": "did:autonomyx:bbb" }
}

{
  "type": "aip.did.document",
  "payload": {
    "did_document": { /* W3C DID Document 1.1 */ },
    "ttl_secs":     300
  }
}
```

---

## 6. Accountability Replication

Every node replicates accountability records to its peers.
This ensures the audit log survives node failures.

```json
{
  "type": "aip.audit.record",
  "payload": {
    "did":         "did:autonomyx:aaa",
    "action":      "tool:web_search",
    "resource":    "https://autonomyx.io",
    "outcome":     "success",
    "evidence":    { },
    "signature":   "<hex-ed25519>",
    "recorded_at": "2026-06-14T03:20:00Z"
  }
}
```

---

## 7. Error Protocol

```json
{
  "type": "aip.error",
  "payload": {
    "code":    "AIP_GATE_CLOSED",
    "stage":   "sign",
    "reason":  "cosign bundle missing — supply chain unverified",
    "artifact": "sha256:..."
  }
}
```

| Error Code | Meaning |
|---|---|
| `AIP_AUTH_FAILED` | Signature verification failed |
| `AIP_GRANT_EXPIRED` | JIT grant has expired |
| `AIP_POLICY_VIOLATION` | Capability not in governance policy |
| `AIP_GATE_CLOSED` | Lifecycle gate refused (oath broke) |
| `AIP_DID_NOT_FOUND` | DID not resolvable |
| `AIP_RATE_LIMITED` | Rate limit exceeded (600 RPM default) |
| `AIP_REPLAY` | Message ID already seen (replay attack) |

---

## 8. Transport Bindings

| Transport | Use case | Endpoint |
|---|---|---|
| HTTPS/1.1 | Capability requests, DID resolution | `POST /aip/*` |
| WebSocket | Fabric event stream (bidirectional) | `ws://host/ws/aip` |
| SSE | Read-only fabric subscription | `GET /aip/stream` |
| gRPC (future) | High-throughput inter-node | `/aip.AgentService/*` |

All transports require:
- TLS 1.3 minimum
- Bearer token (gateway API key) for ingress
- Ed25519 message signature for AIP-level authentication
- mTLS between pods (Istio STRICT)

---

## 9. Worlds Binding

AIP runs natively in every world:

| World | AIP Transport | Notes |
|---|---|---|
| server | HTTPS + WebSocket | Full AIP surface |
| edge | HTTPS (WASM/WASIX) | No WebSocket in some runtimes |
| k8s | HTTPS + mTLS (Istio) | Pods talk AIP over service mesh |
| mobile | HTTPS (UniFFI) | Restricted to capability requests |
| embedded | Serial/BLE bridge | Subset — gate events only |
| desktop | HTTPS (Theia) | Full AIP surface via local runner |
| browser | HTTPS + SSE | Read fabric, post capability requests |
| peer | HTTPS + /transfer | Egress-push only |

---

## 10. Process Excellence

AIP maintains ecosystem balance by:
- Being transport-agnostic (HTTPS today, gRPC tomorrow, any future transport)
- Being provider-agnostic (intelligence binding in DID, not in protocol)
- Being world-agnostic (same envelope in all worlds, subset features where needed)
- Being governance-first (every message carries a grant; grants are governed by DID policy)
- Producing an accountability record for every interaction (signed, non-repudiable)

---

*Autonomyx is the platform. openautonomyx.com*
*The network is the edge. The agent is the unit. The protocol is the contract.*
