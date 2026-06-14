// Autonomyx Schema Grammar — Schema.org grounding for all agent types.
//
// Every Autonomyx type is grounded in Schema.org vocabulary.
// Schema.org provides semantic interoperability: any system that
// understands Schema.org can understand an Autonomyx agent.
//
// Agent Schema Base:
//   Agent       → schema:SoftwareApplication, schema:Person
//   Tool        → schema:SoftwareApplication, schema:EntryPoint
//   Workflow    → schema:Action, schema:ItemList
//   Memory      → schema:Dataset, schema:DataFeed
//   Peer        → schema:Organization, schema:Place
//   Infra       → schema:DataCenter, schema:HostingBusiness
//   Creative    → schema:CreativeWork (writer, composer, designer, ...)
//   Research    → schema:ScholarlyArticle, schema:Dataset
//   Analysis    → schema:AnalysisNewsArticle, schema:Report
//   Code        → schema:SoftwareSourceCode, schema:SoftwareApplication
//   Knowledge   → schema:KnowledgeGraph, schema:DefinedTermSet
//   Event       → schema:Event, schema:BroadcastEvent
//   Commerce    → schema:Product, schema:Offer, schema:Order
//
// Schema.org base types give agents semantic meaning beyond the platform:
//   - Linked data consumers understand agent outputs natively
//   - Search engines can index agent capabilities
//   - AIP messages carry schema.org type annotations
//   - The BOM references schema.org for dependency typing

grammar AutonomyxSchema

// ── Schema base declaration ───────────────────────────────────────────────────

SchemaBase:
    'schema_base' name=ID '{'
        mappings+=SchemaMapping*
        creative+=CreativeAgentType*
    '}'
;

SchemaMapping:
    'map' autonomyxType=AutonomyxType '->' '[' schemaTypes+=STRING (',' schemaTypes+=STRING)* ']' ';'
;

AutonomyxType returns string:
    'Agent' | 'Tool' | 'Workflow' | 'Memory' | 'Peer' | 'Infra' |
    'Pipeline' | 'Gateway' | 'Contract' | 'Identity' | 'World' |
    'Lifecycle' | 'Gate' | 'Fabric' | 'Provider' | 'Adapter' |
    'Adopter' | 'Promoter' | 'Runtime' | 'Coral' | 'FinOps' | 'Sla' | 'Gpa'
;

// ── Creative Agents ───────────────────────────────────────────────────────────
//
// Creative agents operate in generative, open-ended domains.
// They use schema:CreativeWork and its subtypes as their output schema.
// Creative agents may use adaptive or chain reasoning modes (model-directed).
// Their outputs are versioned, signed, and attributed via the accountability log.
//
// Creative agent types:
//   writer      — long-form text, articles, documentation, stories
//   composer    — music, audio, structured audio notation
//   designer    — visual concepts, layouts, component specs
//   coder       — source code, algorithms, tests, scripts
//   researcher  — literature synthesis, hypothesis generation
//   analyst     — data interpretation, trend identification
//   strategist  — plans, roadmaps, decision frameworks
//   narrator    — presentations, explanations, educational content

CreativeAgentType:
    'creative_type' name=CreativeKind '{'
        'schema:'       schemaType=STRING        // schema.org type URL
        'output_type:'  outputType=CreativeOutput
        ('reasoning:'   reasoning=ReasoningKind)?
        ('tools:'       '[' tools+=STRING (',' tools+=STRING)* ']')?
        ('attribution:' attribution=AttributionKind)?
        ('license:'     license=STRING)?         // SPDX or CC license
    '}'
;

CreativeKind returns string:
    'writer' | 'composer' | 'designer' | 'coder' | 'researcher' |
    'analyst' | 'strategist' | 'narrator' | 'curator' | 'critic'
;

CreativeOutput returns string:
    // schema:CreativeWork subtypes
    'Article'          // schema:Article
    | 'BlogPosting'    // schema:BlogPosting
    | 'TechArticle'    // schema:TechArticle
    | 'Book'           // schema:Book
    | 'MusicComposition' // schema:MusicComposition
    | 'VisualArtwork'  // schema:VisualArtwork
    | 'SoftwareSourceCode' // schema:SoftwareSourceCode
    | 'Dataset'        // schema:Dataset
    | 'Report'         // schema:Report
    | 'Presentation'   // schema:PresentationDigitalDocument
    | 'HowTo'          // schema:HowTo
    | 'FAQPage'        // schema:FAQPage
    | 'DefinedTermSet' // schema:DefinedTermSet
    | 'KnowledgeGraph' // schema:KnowledgeGraph (informal)
    | 'Structured'     // custom structured JSON-LD output
;

ReasoningKind returns string:
    'react' | 'chain' | 'adaptive' | 'direct'
;

AttributionKind returns string:
    'did_signed'     // signed by agent DID (accountable, non-repudiable)
    | 'anonymous'    // no attribution (community mode)
    | 'team'         // attributed to a team DID
    | 'collab'       // multiple DID co-attribution
;

// ── Schema-defined Agent ──────────────────────────────────────────────────────
//
// A Schema-defined Agent is fully described by Schema.org vocabulary.
// The agent's capabilities, inputs, and outputs are schema-typed.
// This makes agents machine-readable by any linked-data consumer.
// AIP messages from schema-defined agents carry @context and @type.

SchemaDefined:
    'schema_agent' name=ID '{'
        '@context:' context=STRING             // "https://schema.org"
        '@type:'    type=STRING                // e.g. "SoftwareApplication"
        'identifier:' identifier=STRING        // the agent's DID
        ('name:'    label=STRING)?
        ('description:' description=STRING)?
        ('url:'     url=STRING)?               // agent's service endpoint
        inputs+=SchemaProperty*
        outputs+=SchemaProperty*
        ('offers:'  '[' offers+=SchemaOffer (',' offers+=SchemaOffer)* ']')?
    '}'
;

SchemaProperty:
    ('input' | 'output') name=ID ':' type=STRING ('required:' required=BOOL)? ';'
;

SchemaOffer:
    'offer' '{'
        'name:'       name=STRING
        'description:' description=STRING
        ('price:'     price=NUMBER  'currency:' currency=STRING)?
        ('category:'  category=STRING)?
    '}'
;

// ── Built-in Schema Base — Autonomyx canonical mappings ──────────────────────
//
// These are the authoritative mappings for the Autonomyx platform.
// They are baked into the validator and LSP for hover documentation.

schema_base autonomyx_canonical {
    map Agent      -> ["https://schema.org/SoftwareApplication", "https://schema.org/Person"] ;
    map Tool       -> ["https://schema.org/SoftwareApplication", "https://schema.org/EntryPoint"] ;
    map Workflow   -> ["https://schema.org/Action", "https://schema.org/ItemList"] ;
    map Memory     -> ["https://schema.org/Dataset", "https://schema.org/DataFeed"] ;
    map Peer       -> ["https://schema.org/Organization"] ;
    map Infra      -> ["https://schema.org/DataCenter"] ;
    map Gateway    -> ["https://schema.org/WebAPI"] ;
    map Contract   -> ["https://schema.org/DigitalDocument"] ;
    map Identity   -> ["https://schema.org/Person", "https://schema.org/Identifier"] ;
    map Lifecycle  -> ["https://schema.org/ItemList"] ;
    map Gate       -> ["https://schema.org/Action"] ;
    map Fabric     -> ["https://schema.org/DataFeed"] ;
    map Provider   -> ["https://schema.org/Organization"] ;
    map Adapter    -> ["https://schema.org/SoftwareApplication"] ;
    map Adopter    -> ["https://schema.org/Customer", "https://schema.org/Organization"] ;
    map Promoter   -> ["https://schema.org/Organization"] ;
    map Runtime    -> ["https://schema.org/SoftwareApplication"] ;
    map FinOps     -> ["https://schema.org/MonetaryAmount"] ;
    map Coral      -> ["https://schema.org/Ecosystem"] ;

    creative_type writer {
        schema:      "https://schema.org/Article"
        output_type: Article
        reasoning:   chain
        attribution: did_signed
        license:     "CC-BY-4.0"
    }

    creative_type coder {
        schema:      "https://schema.org/SoftwareSourceCode"
        output_type: SoftwareSourceCode
        reasoning:   react
        tools:       ["code_execution", "web_search", "file_read"]
        attribution: did_signed
        license:     "Apache-2.0"
    }

    creative_type researcher {
        schema:      "https://schema.org/ScholarlyArticle"
        output_type: Report
        reasoning:   adaptive
        tools:       ["web_search", "document_reader"]
        attribution: did_signed
    }

    creative_type analyst {
        schema:      "https://schema.org/Report"
        output_type: Dataset
        reasoning:   chain
        tools:       ["data_query", "chart_generation"]
        attribution: did_signed
    }

    creative_type strategist {
        schema:      "https://schema.org/Report"
        output_type: Structured
        reasoning:   adaptive
        attribution: did_signed
    }
}
