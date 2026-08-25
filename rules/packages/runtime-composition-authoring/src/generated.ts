// Generated from Rust product-model contract descriptor. Do not edit.
// Runtime standard capability constants are generated from their Rust descriptor.

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
    },
    {
      "access": {
        "reads": [
          "entity-state.components",
          "entity-state.transforms",
          "engine-spatial.occlusion"
        ],
        "writes": [
          "runtime-mutation.operations"
        ]
      },
      "availability": "linkable",
      "budget": {
        "maximumCompactJsonPayloadBytes": 16384
      },
      "kind": "system",
      "provenance": {
        "logicalPath": "ObservePairsPlan::compile",
        "owner": "rusty-engine.runtime-standard-capabilities",
        "source": "rust/crates/runtime-standard-capabilities/src/lib.rs"
      },
      "target": "engine.runtime.observe-pairs",
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
export const RUNTIME_STANDARD_CAPABILITIES = {
  "artifact": "runtime-standard-capabilities",
  "observePairs": {
    "access": {
      "reads": [
        "entity-state.components",
        "entity-state.transforms",
        "engine-spatial.occlusion"
      ],
      "writes": [
        "runtime-mutation.operations"
      ]
    },
    "kind": "system",
    "maximumCompactJsonPayloadBytes": 16384,
    "payload": {
      "fields": [
        "kind",
        "observerRole",
        "targetRole",
        "operationBinding",
        "operationType",
        "quotas"
      ],
      "kind": "engine.runtime.observe-pairs.v1",
      "quotaFields": [
        "observers",
        "targets",
        "pairs",
        "aggregates"
      ],
      "resultKind": "engine.runtime.observe-pairs.result.v1",
      "visibility": "center-ray"
    },
    "quotas": {
      "aggregates": 256,
      "observers": 64,
      "pairs": 1024,
      "targets": 256
    },
    "target": "engine.runtime.observe-pairs"
  }
} as const;
export const PRODUCT_MODEL_FIELDS = {
  "capabilityBinding": [
    "id",
    "target"
  ],
  "compiledComposition": [
    "product",
    "intentDescriptors",
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
    "trigger"
  ],
  "intentDescriptor": [
    "id",
    "valueKind",
    "payloadContract",
    "capability",
    "payload"
  ],
  "schedule": [
    "phase",
    "mode",
    "systems",
    "before",
    "after"
  ],
  "scheduleCadence": [
    "everySteps",
    "offsetSteps"
  ],
  "scheduleSystem": [
    "id",
    "capability",
    "definition",
    "after",
    "reads",
    "writes",
    "cadence",
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
export const PRODUCT_MODEL_INPUT = {
  "axes": [
    "x",
    "y"
  ],
  "controllerAxes": [
    "axis-0",
    "axis-1",
    "axis-2",
    "axis-3"
  ],
  "controllerButtons": [
    "button-0",
    "button-1",
    "button-2",
    "button-3",
    "button-4",
    "button-5",
    "button-6",
    "button-7",
    "button-8",
    "button-9",
    "button-10",
    "button-11",
    "button-12",
    "button-13",
    "button-14",
    "button-15"
  ],
  "edges": [
    "held",
    "pressed",
    "released"
  ],
  "intentValueKinds": [
    "digital",
    "axis",
    "product-payload"
  ],
  "keyboardControls": [
    "key-a",
    "key-b",
    "key-c",
    "key-d",
    "key-e",
    "key-f",
    "key-g",
    "key-h",
    "key-i",
    "key-j",
    "key-k",
    "key-l",
    "key-m",
    "key-n",
    "key-o",
    "key-p",
    "key-q",
    "key-r",
    "key-s",
    "key-t",
    "key-u",
    "key-v",
    "key-w",
    "key-x",
    "key-y",
    "key-z",
    "digit-0",
    "digit-1",
    "digit-2",
    "digit-3",
    "digit-4",
    "digit-5",
    "digit-6",
    "digit-7",
    "digit-8",
    "digit-9",
    "space",
    "enter",
    "escape",
    "shift-left",
    "shift-right",
    "control-left",
    "control-right",
    "alt-left",
    "alt-right"
  ],
  "pointerButtons": [
    "primary",
    "secondary",
    "middle"
  ],
  "triggerKinds": [
    "key",
    "pointer-button",
    "pointer-axis",
    "wheel",
    "controller-button",
    "controller-axis"
  ]
} as const;
export const PRODUCT_MODEL_LIMITS = {
  "maximumCapabilityBindings": 512,
  "maximumDirectIntentProductPayloadBytes": 65536,
  "maximumEncodedBytes": 1048576,
  "maximumGameplayDefinitions": 512,
  "maximumInputChordControls": 8,
  "maximumInputMapEntries": 256,
  "maximumIntentDescriptors": 256,
  "maximumOpaqueJsonArrayEntries": 1024,
  "maximumOpaqueJsonDepth": 32,
  "maximumOpaqueJsonNodes": 4096,
  "maximumOpaqueJsonObjectEntries": 1024,
  "maximumOpaqueJsonStringBytes": 16384,
  "maximumSafeJsonInteger": 9007199254740991,
  "maximumScheduleAccessDeclarations": 64,
  "maximumScheduleDependencies": 64,
  "maximumScheduleEntries": 512,
  "maximumTimelineSteps": 256,
  "maximumTimelines": 256,
  "schedulePhaseCount": 5
} as const;
export const PRODUCT_MODEL_NUMBER_ENCODING = {
  "finiteBinary64": "ecmascript-number-to-string",
  "integer": "base10",
  "negativeZero": "0"
} as const;
export const PRODUCT_MODEL_OPTIONAL_FIELDS = {
  "intentDescriptor": [
    "payloadContract",
    "capability"
  ],
  "scheduleSystem": [
    "definition"
  ]
} as const;
export const PRODUCT_MODEL_ORDERING = {
  "capabilityBindings": "authored",
  "gameplayDefinitions": "authored",
  "inputMap": "authored",
  "intentDescriptors": "authored",
  "opaqueArrays": "authored",
  "opaqueObjectKeys": "canonical-bytewise",
  "schedule": "canonical-phases",
  "scheduleAfter": "authored",
  "scheduleReads": "authored",
  "scheduleSystems": "authored",
  "scheduleWrites": "authored",
  "timelineSteps": "authored",
  "timelines": "authored"
} as const;
export const PRODUCT_MODEL_SCHEDULE = {
  "defaultCadence": {
    "everySteps": 1,
    "offsetSteps": 0
  },
  "modes": [
    "append",
    "prepend",
    "extend",
    "replace"
  ],
  "phases": [
    "input",
    "simulation",
    "consequences",
    "commit",
    "projection"
  ],
  "placements": [
    "append",
    "prepend",
    "extend-before",
    "extend-after",
    "replace"
  ]
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
