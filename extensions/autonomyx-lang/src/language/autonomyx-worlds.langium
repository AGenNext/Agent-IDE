// Autonomyx Worlds — deploy agents natively into any execution environment.
//
// A "world" is an execution target: where the agent runs, what it can perceive,
// and what actions it can take. The same agent definition works across all worlds
// — the world declaration controls the runtime, perception system, and action space.
//
// Worlds:
//   browser    — WASM in the user's browser, no server round-trip
//   server     — Rust autonomyx-runner (Linux/macOS/Windows)
//   edge       — single binary at CDN edge (Cloudflare Workers, Fly.io)
//   mobile     — native binary on iOS/Android (via UniFFI bindings)
//   embedded   — no_std Rust on microcontrollers (ESP32, RPi Pico)
//   k8s        — Kubernetes pod (CNCF native)
//   desktop    — Theia IDE extension (Electron / native)
//   peer       — another Autonomyx node (distributed multi-agent)
//
// Schema.org grounding:
//   World → schema:Place / schema:VirtualLocation
//   Runtime → schema:SoftwareApplication
//   Deploy → schema:DeployAction

World:
    (schema='@schema' '(' schemaType=STRING ')')?
    'world' name=ID '{'
        'target:'  target=WorldTarget
        ('runtime:' runtime=WorldRuntime)?
        ('agent:'   agent=[AgentRef:ID])?
        ('deploy:'  deploy=DeployBlock)?
        ('perception:' perception=PerceptionRef)?
        ('actions:' '[' actions+=ActionKind (',' actions+=ActionKind)* ']')?
        ('constraints:' constraints=ConstraintBlock)?
        ('replicas:' replicas=NUMBER)?
        ('region:'   region=STRING)?
    '}';

WorldTarget returns string:
    'browser'   // WASM — runs in the user's browser, zero latency
    | 'server'  // Rust autonomyx-runner on any OS
    | 'edge'    // CDN edge (Cloudflare, Fly, Fastly) — global, near-zero latency
    | 'mobile'  // iOS/Android native (UniFFI → Swift/Kotlin)
    | 'embedded'// no_std — ESP32, RPi Pico, RISC-V
    | 'k8s'     // Kubernetes pod
    | 'desktop' // Theia IDE extension
    | 'peer';   // Distributed Autonomyx node

WorldRuntime returns string:
    'autonomyx_wasm'    // browser WASM
    | 'autonomyx_rust'  // native Rust binary
    | 'autonomyx_node'  // Node.js (Phase 1 compat)
    | 'autonomyx_edge'  // edge runtime (Workers-compatible)
    | 'autonomyx_bare'  // no_std bare metal
    | 'autonomyx_ios'   // Swift/UniFFI
    | 'autonomyx_android'; // Kotlin/UniFFI

AgentRef: name=ID;

DeployBlock:
    '{' options+=DeployOption* '}';

DeployOption:
    'image:'   image=STRING     |
    'binary:'  binary=STRING    |
    'wasm:'    wasmPath=STRING  |
    'helm:'    helmChart=STRING |
    'gitops:'  gitopsPath=STRING;

PerceptionRef returns string:
    'dom'          // browser DOM + accessibility tree
    | 'screenshot' // visual (browser + desktop)
    | 'filesystem' // server filesystem
    | 'sensors'    // embedded hardware sensors
    | 'camera'     // mobile camera
    | 'mic'        // mobile/desktop microphone
    | 'api'        // REST/GraphQL tool calls
    | 'network';   // network traffic (edge)

ActionKind returns string:
    'click' | 'type' | 'navigate' | 'scroll'    // browser
    | 'shell' | 'filesystem' | 'process'         // server
    | 'http' | 'grpc' | 'ws'                     // network
    | 'gpio' | 'i2c' | 'spi' | 'uart'            // embedded
    | 'push_notification' | 'haptic' | 'camera'  // mobile
    | 'tool_call' | 'peer_transfer';              // universal

ConstraintBlock:
    '{' options+=ConstraintOption* '}';

ConstraintOption:
    'no_std:'     noStd=BOOLEAN     |
    'max_mem:'    maxMem=STRING     |
    'no_network:' noNetwork=BOOLEAN |
    'offline:'    offline=BOOLEAN   |
    'battery:'    batteryAware=BOOLEAN;

terminal ID:      /[_a-zA-Z][\w-]*/;
terminal STRING:  /"[^"]*"|'[^']*'/;
terminal NUMBER:  /[0-9]+(\.[0-9]+)?/;
terminal BOOLEAN: /true|false/;
