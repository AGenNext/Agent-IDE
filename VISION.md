# Autonomyx — The Vision

**Multi-ecosystem. Single world model.**
**Everyone and everything is an agent.**
**Manageable, governable, secure at infinite scale.**

---

## The World Model

There is one world. Every entity in it — person, device, service, organisation,
model, process, workflow, sensor, vehicle, building — is an agent.

Each agent is:
- **Real** — hardware-backed identity (HSM/TPM/DID)
- **Unique** — globally unique `did:autonomyx:<pubkey>`. Cannot be cloned.
- **Versioned** — every agent has a version. Changes are explicit, signed, auditable.
- **Manageable** — registered in the federation, reachable via AIP
- **Governable** — GovernancePolicy in the DID Document; JIT access, no standing permissions
- **Secure** — Ed25519 signatures on every action; mTLS between all nodes
- **Accountable** — signed accountability log; non-repudiable

This is not a metaphor. It is a specification.
A temperature sensor has a DID. A CI/CD pipeline has a DID. A customer has a DID.
The agent runtime runs natively on all of them.

---

## Multi-Ecosystem

The platform bridges multiple ecosystems without owning any of them:

```
LLM Ecosystem      Intelligence providers (OpenAI, Anthropic, Ollama, Groq, ...)
Cloud Ecosystem    Compute providers (AWS, GCP, Azure, Hetzner, home server, k3s)
Identity Ecosystem DIDs, W3C Verifiable Credentials, Ed25519, HSM/TPM
Data Ecosystem     SurrealDB, S3, local fs, Redis, Postgres
Observability      OTel, Prometheus, Jaeger, Grafana
GitOps             ArgoCD, Flux — config is code, delivery is declarative
Supply Chain       Stacker SI, Zot registry, cosign — BOM at every gate
Mesh               Istio, Cilium eBPF — network is the contract
IDE                Theia, VS Code, JetBrains — where agents are authored
```

No ecosystem is required. All are supported. Any can be swapped.
The `.ayx` file declares which ecosystems the agent participates in.
The platform routes accordingly.

---

## Single World Model

One meta model. One DID space. One protocol (AIP). One lifecycle.

Every entity, regardless of which ecosystem it lives in,
speaks the same language:
- Same DID format (`did:autonomyx:<pubkey>`)
- Same AIP wire protocol
- Same lifecycle gates (build → sign → push → sync → deploy → run → observe → feedback)
- Same accountability log format
- Same governance model (JIT grants, GPA policy)
- Same usage meter (tokens × price, compute × price, $0 for self-hosted)

The world model is the unifying layer.
It does not replace existing systems. It describes them.

---

## Infinite Scale

Scale is not a size. It is a property of the architecture.

**Horizontal**: every agent is a Kubernetes Job. The cluster IS the runner.
Add nodes, add capacity. No rewrite. No migration. No downtime.

**Federated**: every node maintains its own DID registry.
DID resolution is local-first, peer-broadcast second, never central.
A million nodes = a million independent registries = no single point of failure.

**Idempotent**: every gate transition is idempotent.
The same input always produces the same output.
At infinite scale, retries are safe. Duplicates are harmless.

**Governed at scale**: governance is per-DID, not per-platform.
A million agents means a million governance policies — each independently managed.
The platform enforces all of them simultaneously, at the gate, in real time.

**Observable at scale**: every action emits an OTel span.
Every gate emits a SurrealDB event (live query to all subscribers).
Every spend emits a usage record.
At infinite scale, the signal never disappears — it is always traceable.

---

## The Feedback Loop at Infinite Scale

```
Every agent run produces signal.
Signal flows back through the feedback gate.
The feedback gate fires a SurrealDB live query.
Live query reaches all subscribed agents.
Subscribed agents update their behaviour.
Updated behaviour flows to the next build.
Build produces a new BOM-carrying artifact.
Artifact is signed, pushed, synced, deployed.
New agents run with updated intelligence.
Loop repeats.
```

This is not a pipeline. It is a living system.
It evolves. It learns. It governs itself.
And it does it with every participant's identity, governance, and accountability
intact — at any scale.

---

## The Platform Equation

```
Autonomyx = (
    Agent everywhere            — all 8 worlds, any device
  + Identity as primitive       — DID, Ed25519, HSM
  + Network as contract         — Istio, Cilium, AIP
  + Gates as law                — idempotent, oath-enforced, audited
  + Fabric as nervous system    — fills every gap, no polling
  + Supply chain risk = 0       — BOM at every gate, cosign, Zot
  + Usage-based, freedom not free — fair, transparent, self-hostable
  + Ecosystem balance           — Coral monitors, rebalances, sustains
  + Infinite scale              — k8s, federation, idempotency
)
```

That is Autonomyx.

**openautonomyx.com**
