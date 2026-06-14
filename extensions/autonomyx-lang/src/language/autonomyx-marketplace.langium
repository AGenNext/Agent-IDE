// Autonomyx Marketplace Language Extension
//
// Hierarchy:
//   Marketplace (root)
//     └── Provider (e.g. "openai", "anthropic", "ollama", "cloudstack", "oci")
//           └── Operator (a configured instance of a provider — 1 provider : N operators)
//                 └── Profile (a named configuration within an operator — 1 operator : N profiles)
//
// Schema.org grounding:
//   Provider  → schema:Organization
//   Operator  → schema:Service / schema:LocalBusiness
//   Profile   → schema:Product / schema:Offer
//
// This lets you do things like:
//
//   provider openai {
//     operator prod-openai {
//       profile gpt4o  { model: "gpt-4o",    rpm: 500,  tier: "paid" }
//       profile gpt35  { model: "gpt-3.5",   rpm: 3500, tier: "paid" }
//     }
//     operator azure-openai {
//       base_url: "https://myorg.openai.azure.com"
//       profile gpt4o  { model: "gpt-4o", deployment: "my-gpt4o-deployment" }
//     }
//   }
//
//   provider ollama {
//     operator local {
//       base_url: "http://localhost:11434"
//       profile llama3    { model: "llama3",     context: 8192  }
//       profile codellama { model: "codellama",  context: 16384 }
//     }
//     operator remote {
//       base_url: "$REMOTE_OLLAMA_URL"
//       profile phi3 { model: "phi3" }
//     }
//   }

// ── Marketplace (root container) ──────────────────────────────────────────────
Marketplace:
    (schema='@schema' '(' schemaType=STRING ')')?
    'marketplace' name=ID '{'
        providers+=Provider*
    '}';

// ── Provider (schema:Organization) ───────────────────────────────────────────
Provider:
    (schema='@schema' '(' schemaType=STRING ')')?
    'provider' name=ProviderName '{'
        ('description:' description=STRING)?
        ('category:' category=ProviderCategory)?
        operators+=Operator*
    '}';

ProviderName returns string:
    // LLM providers
    'openai'     | 'anthropic'  | 'ollama'    | 'vllm'     |
    'groq'       | 'mistral'    | 'together'  | 'cohere'    |
    'google'     | 'bedrock'    | 'vertex'    | 'huggingface' |
    // Infra / cloud providers
    'kubernetes' | 'oci'        | 'aws'       | 'gcp'       |
    'azure'      | 'hetzner'    | 'fly'       | 'cloudstack' |
    'docker'     | 'bare_metal' | 'nomad'     |
    // Tool providers
    'github'     | 'gitlab'     | 'slack'     | 'jira'      |
    'stripe'     | 'twilio'     | 'sendgrid'  |
    // Custom
    ID;

ProviderCategory returns string:
    'llm' | 'infra' | 'tool' | 'storage' | 'observability' | 'security' | 'data';

// ── Operator (schema:Service — a configured instance of a provider) ───────────
Operator:
    (schema='@schema' '(' schemaType=STRING ')')?
    'operator' name=ID '{'
        ('base_url:'    baseUrl=STRING)?
        ('region:'      region=STRING)?
        ('account_id:'  accountId=STRING)?
        ('api_key_env:' apiKeyEnv=STRING)?
        ('auth:'        auth=OperatorAuth)?
        ('description:' description=STRING)?
        ('enabled:'     enabled=BOOLEAN)?
        profiles+=Profile*
    '}';

OperatorAuth:
    '{' 'type:' type=OperatorAuthKind
        ('key_env:'    keyEnv=STRING)?
        ('secret_env:' secretEnv=STRING)?
        ('cert_path:'  certPath=STRING)?
    '}';

OperatorAuthKind returns string:
    'bearer_env' | 'api_key_env' | 'basic_env' | 'mtls' | 'aws_iam' | 'gcp_sa' | 'none';

// ── Profile (schema:Product — a named configuration within an operator) ────────
Profile:
    (schema='@schema' '(' schemaType=STRING ')')?
    'profile' name=ID '{'
        // LLM profile fields
        ('model:'       model=STRING)?
        ('deployment:'  deployment=STRING)?   // Azure OpenAI deployment name
        ('context:'     context=NUMBER)?      // context window tokens
        ('max_tokens:'  maxTokens=NUMBER)?
        ('rpm:'         rpm=NUMBER)?           // rate: requests per minute
        ('tpm:'         tpm=NUMBER)?           // rate: tokens per minute
        ('tier:'        tier=ProfileTier)?
        ('cost_per_1m_input:'  costIn=NUMBER)?
        ('cost_per_1m_output:' costOut=NUMBER)?

        // Infra profile fields
        ('size:'        size=STRING)?          // e.g. "t3.medium", "VM.Standard.E4.Flex"
        ('cpu:'         cpu=STRING)?
        ('memory:'      memory=STRING)?
        ('gpu:'         gpu=STRING)?
        ('region:'      region=STRING)?
        ('zone:'        zone=STRING)?
        ('os:'          os=STRING)?

        // Tool profile fields
        ('endpoint:'    endpoint=STRING)?
        ('version:'     version=STRING)?
        ('scopes:'      '[' scopes+=STRING (',' scopes+=STRING)* ']')?

        // Common
        ('tags:'        '[' tags+=STRING (',' tags+=STRING)* ']')?
        ('default:'     isDefault=BOOLEAN)?
    '}';

ProfileTier returns string:
    'free' | 'paid' | 'enterprise' | 'preview' | 'beta';

terminal ID:      /[_a-zA-Z][\w-]*/;
terminal STRING:  /"[^"]*"|'[^']*'/;
terminal NUMBER:  /[0-9]+(\.[0-9]+)?/;
terminal BOOLEAN: /true|false/;
