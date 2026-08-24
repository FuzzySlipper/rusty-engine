// Generated from Rust product-model contract descriptor. Do not edit.

export const PRODUCT_MODEL_ARTIFACT = "compiled-composition" as const;
export const PRODUCT_MODEL_CAPABILITY_TARGETS = {
  "namespaces": [
    "engine",
    "kernel"
  ],
  "separator": "."
} as const;
export const PRODUCT_MODEL_CAPABILITY_CATALOG = {
  "engine": [
    {
      "access": {
        "reads": [
          "entity-state.projection"
        ],
        "writes": [
          "render-frame.diff"
        ]
      },
      "availability": "linkable",
      "budget": {
        "maximumCompactJsonPayloadBytes": 1024
      },
      "kind": "projection",
      "provenance": {
        "logicalPath": "EntityRenderProjector::project",
        "owner": "rusty-engine.render-projection",
        "source": "rust/crates/render-projection/src/entity.rs"
      },
      "target": "engine.render.entity-project",
      "uses": [
        "schedule"
      ]
    }
  ],
  "kinds": [
    "system",
    "operation",
    "query",
    "projection",
    "migration"
  ]
} as const;
export type EngineCapabilityTarget = typeof PRODUCT_MODEL_CAPABILITY_CATALOG.engine[number]['target'];
export type EngineCapabilityName = EngineCapabilityTarget extends `engine.${infer Name}` ? Name : never;
export const PRODUCT_MODEL_FIELDS = {
  "capabilityBinding": [
    "id",
    "target"
  ],
  "compiledComposition": [
    "product",
    "inputMap",
    "schedule",
    "gameplayDefinitions",
    "timelines",
    "capabilityBindings"
  ],
  "gameplayDefinition": [
    "id",
    "payload"
  ],
  "inputMap": [
    "id",
    "intent",
    "capability",
    "payload"
  ],
  "schedule": [
    "id",
    "phase",
    "capability",
    "definition",
    "reads",
    "writes",
    "payload"
  ],
  "timeline": [
    "id",
    "steps"
  ],
  "timelineStep": [
    "id",
    "capability",
    "payload"
  ]
} as const;
export const PRODUCT_MODEL_IDENTITY = {
  "alphabet": "lowercase-ascii-alphanumeric-dot-underscore-hyphen",
  "forbidAdjacentSeparators": true,
  "maximumBytes": 128,
  "startsAndEndsAlphanumeric": true
} as const;
export const PRODUCT_MODEL_LIMITS = {
  "maximumCapabilityBindings": 512,
  "maximumEncodedBytes": 1048576,
  "maximumGameplayDefinitions": 512,
  "maximumInputMapEntries": 256,
  "maximumOpaqueJsonArrayEntries": 1024,
  "maximumOpaqueJsonDepth": 32,
  "maximumOpaqueJsonNodes": 4096,
  "maximumOpaqueJsonObjectEntries": 1024,
  "maximumOpaqueJsonStringBytes": 16384,
  "maximumSafeJsonInteger": 9007199254740991,
  "maximumScheduleAccessDeclarations": 64,
  "maximumScheduleEntries": 512,
  "maximumTimelineSteps": 256,
  "maximumTimelines": 256
} as const;
export const PRODUCT_MODEL_NUMBER_ENCODING = {
  "finiteBinary64": "ecmascript-number-to-string",
  "integer": "base10",
  "negativeZero": "0"
} as const;
export const PRODUCT_MODEL_OPTIONAL_FIELDS = {
  "schedule": [
    "definition"
  ]
} as const;
export const PRODUCT_MODEL_ORDERING = {
  "capabilityBindings": "authored",
  "gameplayDefinitions": "authored",
  "inputMap": "authored",
  "opaqueArrays": "authored",
  "opaqueObjectKeys": "canonical-bytewise",
  "schedule": "authored",
  "scheduleReads": "authored",
  "scheduleWrites": "authored",
  "timelineSteps": "authored",
  "timelines": "authored"
} as const;
export const PRODUCT_MODEL_FAILURES = [
  "unknown-field",
  "missing-field",
  "duplicate-json-key",
  "duplicate-identity",
  "unknown-capability",
  "unknown-definition",
  "invalid-capability-target",
  "invalid-identity",
  "access-declaration-limit",
  "opaque-json-limit"
] as const;
