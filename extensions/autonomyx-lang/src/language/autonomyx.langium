// Autonomyx Language Grammar
// Semantic types are grounded in Schema.org vocabulary:
//   Agent      → schema:SoftwareApplication / schema:Person
//   Workflow   → schema:Action / schema:ItemList
//   Tool       → schema:SoftwareApplication
//   Memory     → schema:Dataset / schema:DataFeed
//   Peer       → schema:Place / schema:Organization
//   Infra      → schema:DataCenter / schema:HostingBusiness
//   Task       → schema:EntryPoint / schema:Action
//   Pipeline   → schema:Workflow (pending schema.org proposal)

grammar Autonomyx

entry Model:
    declarations+=Declaration*;

Declaration:
    Agent | Workflow | Tool | Peer | Infra | Pipeline | SchemaType;

// ── Agent (schema:SoftwareApplication) ───────────────────────────────────────
Agent:
    (schema='@schema' '(' schemaType=STRING ')')?
    'agent' name=ID ('extends' base=[Agent:ID])?
    '{'
        ('model:'     model=STRING)?
        ('provider:'  provider=ProviderKind)?
        ('api_key:'   apiKey=STRING)?
        ('base_url:'  baseUrl=STRING)?
        ('tools:'     '[' tools+=[Tool:ID] (',' tools+=[Tool:ID])* ']')?
        ('memory:'    memory=MemoryBlock)?
        ('reasoning:' reasoning=ReasoningBlock)?
        ('perception:'perception=PerceptionBlock)?
        ('peers:'     '[' peers+=[Peer:ID] (',' peers+=[Peer:ID])* ']')?
        ('tags:'      '[' tags+=STRING (',' tags+=STRING)* ']')?
    '}';

ProviderKind returns string:
    'anthropic' | 'openai' | 'ollama' | 'vllm' | 'groq' | 'mistral' |
    'together' | 'custom';

// ── Tool (schema:SoftwareApplication / schema:EntryPoint) ─────────────────────
Tool:
    (schema='@schema' '(' schemaType=STRING ')')?
    'tool' name=ID
    '{'
        ('description:' description=STRING)?
        ('input:'  '{' inputs+=ParamDef (',' inputs+=ParamDef)* '}')?
        ('output:' outputType=TypeRef)?
        ('impl:'   impl=ImplKind)?
        ('url:'    url=STRING)?
        ('auth:'   auth=AuthBlock)?
    '}';

ImplKind returns string:
    'http' | 'shell' | 'python' | 'wasm' | 'native' | 'mcp';

ParamDef:
    name=ID ':' type=TypeRef ('=' default=Literal)?;

TypeRef returns string:
    'string' | 'number' | 'boolean' | 'json' | 'file' | 'image' | 'buffer';

Literal:
    value=STRING | value=NUMBER | value=BOOLEAN;

AuthBlock:
    '{' 'type:' type=AuthKind ('key:' key=STRING)? '}';

AuthKind returns string:
    'bearer' | 'api_key' | 'basic' | 'none';

// ── Memory (schema:Dataset / schema:DataFeed) ─────────────────────────────────
MemoryBlock:
    kind=MemoryKind
    ('{' options+=MemoryOption* '}')?;

MemoryKind returns string:
    'episodic' | 'semantic' | 'procedural' | 'working' | 'rag' | 'sql';

MemoryOption:
    'storage:'    storage=StorageKind     |
    'window:'     window=NUMBER           |
    'index:'      index=STRING            |
    'ttl:'        ttl=NUMBER              |
    'embeddings:' embeddings=STRING;

StorageKind returns string:
    'in_memory' | 'rag' | 'redis' | 'postgres' | 'sqlite' | 'vector';

// ── Reasoning (implements ReAct / CoT / ToT / DPPM from the architecture) ────
ReasoningBlock:
    strategy=ReasoningStrategy
    ('{' options+=ReasoningOption* '}')?;

ReasoningStrategy returns string:
    'react' | 'chain_of_thought' | 'tree_of_thought' | 'dppm' | 'rewoo' | 'reflexion';

ReasoningOption:
    'max_iter:'   maxIter=NUMBER      |
    'decompose:'  decompose=STRING    |
    'parallel:'   parallel=BOOLEAN    |
    'reflection:' reflection=BOOLEAN  |
    'anticipate:' anticipate=BOOLEAN;

// ── Perception (schema:Observation / multimodal inputs) ───────────────────────
PerceptionBlock:
    modality=PerceptionKind
    ('{' options+=PerceptionOption* '}')?;

PerceptionKind returns string:
    'text' | 'multimodal' | 'vision' | 'a11y_tree' | 'html' | 'tool_augmented';

PerceptionOption:
    'encoder:'     encoder=STRING   |
    'som:'         som=BOOLEAN      |
    'depth_maps:'  depthMaps=BOOLEAN|
    'screenshot:'  screenshot=BOOLEAN;

// ── Workflow (schema:Action / schema:ItemList) ────────────────────────────────
Workflow:
    (schema='@schema' '(' schemaType=STRING ')')?
    'workflow' name=ID
    '{'
        ('agent:' agent=[Agent:ID])?
        ('input:' '{' inputs+=ParamDef (',' inputs+=ParamDef)* '}')?
        ('output:' '{' outputs+=ParamDef (',' outputs+=ParamDef)* '}')?
        steps+=Step*
        ('on_error:' onError=ErrorPolicy)?
    '}';

Step:
    'step' name=ID ('→' | '->')? '{'
        ('tool:'    tool=[Tool:ID])?
        ('input:'   '{' bindings+=Binding (',' bindings+=Binding)* '}')?
        ('output:'  outputVar=ID)?
        ('when:'    condition=STRING)?
        ('parallel:'parallel=BOOLEAN)?
    '}';

Binding:
    name=ID ':' source=STRING;

ErrorPolicy returns string:
    'retry' | 'fail' | 'skip' | 'escalate';

// ── Pipeline (CI / build — schema:SoftwareSourceCode) ─────────────────────────
Pipeline:
    'pipeline' name=ID
    '{'
        ('trigger:' trigger=TriggerKind)?
        stages+=Stage*
    '}';

TriggerKind returns string:
    'push' | 'pr' | 'schedule' | 'manual';

Stage:
    'stage' name=ID '{'
        ('image:'   image=STRING)?
        ('runs_on:' runsOn=STRING)?
        commands+=Command*
        ('needs:'   '[' needs+=STRING (',' needs+=STRING)* ']')?
    '}';

Command:
    'run:' cmd=STRING;

// ── Peer (schema:Organization / network peer) ─────────────────────────────────
Peer:
    (schema='@schema' '(' schemaType=STRING ')')?
    'peer' name=ID
    '{'
        ('url:'      url=STRING)?
        ('protocol:' protocol=PeerProtocol)?
        ('auth:'     auth=AuthBlock)?
        ('region:'   region=STRING)?
    '}';

PeerProtocol returns string:
    'transfer' | 'grpc' | 'http' | 'ws';

// ── Infra (schema:DataCenter / HostingBusiness) ───────────────────────────────
Infra:
    (schema='@schema' '(' schemaType=STRING ')')?
    'infra' name=ID
    '{'
        ('provider:' provider=InfraProvider)?
        ('region:'   region=STRING)?
        ('runtime:'  runtime=RuntimeKind)?
        ('image:'    image=STRING)?
        ('domain:'   domain=STRING)?
        ('tls:'      tls=BOOLEAN)?
        resources+=Resource*
    '}';

InfraProvider returns string:
    'oci' | 'cloudstack' | 'aws' | 'gcp' | 'azure' | 'hetzner' | 'fly' |
    'kubernetes' | 'docker' | 'bare_metal';

RuntimeKind returns string:
    'autonomyx_runner' | 'node' | 'docker' | 'k8s' | 'wasm';

Resource:
    'resource' kind=ResourceKind name=ID
    ('{' ('size:' size=STRING)? ('count:' count=NUMBER)? '}')?;

ResourceKind returns string:
    'cpu' | 'memory' | 'storage' | 'gpu' | 'network';

// ── SchemaType annotation block (explicit schema.org binding) ─────────────────
SchemaType:
    '@type' name=ID 'as' uri=STRING
    ('{' (properties+=SchemaProperty)* '}')?;

SchemaProperty:
    propName=ID ':' propType=STRING;

// ── Terminals ─────────────────────────────────────────────────────────────────
hidden terminal WS:         /\s+/;
hidden terminal BLOCK_COMMENT: /\/\*[\s\S]*?\*\//;
hidden terminal LINE_COMMENT:  /\/\/[^\n]*/;

terminal ID:      /[_a-zA-Z][\w-]*/;
terminal STRING:  /"[^"]*"|'[^']*'/;
terminal NUMBER:  /[0-9]+(\.[0-9]+)?/;
terminal BOOLEAN: /true|false/;
