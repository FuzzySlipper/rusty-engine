export declare const GENERATED_STANDARD_HOST_WIRE: {
    readonly commands: {
        readonly "standard.admin.effect.apply": {
            readonly error: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 8192;
                readonly maximumNodes: 128;
            };
            readonly request: {
                readonly fields: {
                    readonly definition: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly entity: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly expectedRevision: {
                        readonly required: false;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly instance: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly operation: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly provenance: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "taggedUnion";
                            readonly tag: "kind";
                            readonly variants: {
                                readonly effect: {
                                    readonly fields: {
                                        readonly effect: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["effect"];
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly stack: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "integer";
                                                readonly maximum: 65535;
                                                readonly minimum: 0;
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly equippedItem: {
                                    readonly fields: {
                                        readonly item: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["equippedItem"];
                                            };
                                        };
                                        readonly owner: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly intrinsic: {
                                    readonly fields: {
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["intrinsic"];
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly request: {
                                    readonly fields: {
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["request"];
                                            };
                                        };
                                        readonly operation: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                            };
                        };
                    };
                    readonly stacks: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 65535;
                            readonly minimum: 1;
                        };
                    };
                };
                readonly kind: "object";
            };
            readonly result: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 16384;
                readonly maximumNodes: 256;
            };
        };
        readonly "standard.admin.effect.remove": {
            readonly error: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 8192;
                readonly maximumNodes: 128;
            };
            readonly request: {
                readonly fields: {
                    readonly entity: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly expectedRevision: {
                        readonly required: false;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly instance: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly operation: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                };
                readonly kind: "object";
            };
            readonly result: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 16384;
                readonly maximumNodes: 256;
            };
        };
        readonly "standard.admin.stat.set-base": {
            readonly error: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 8192;
                readonly maximumNodes: 128;
            };
            readonly request: {
                readonly fields: {
                    readonly base: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                    readonly entity: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly expectedRevision: {
                        readonly required: false;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly operation: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly source: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "taggedUnion";
                            readonly tag: "kind";
                            readonly variants: {
                                readonly effect: {
                                    readonly fields: {
                                        readonly effect: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["effect"];
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly stack: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "integer";
                                                readonly maximum: 65535;
                                                readonly minimum: 0;
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly equippedItem: {
                                    readonly fields: {
                                        readonly item: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["equippedItem"];
                                            };
                                        };
                                        readonly owner: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly intrinsic: {
                                    readonly fields: {
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["intrinsic"];
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly request: {
                                    readonly fields: {
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["request"];
                                            };
                                        };
                                        readonly operation: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                            };
                        };
                    };
                    readonly stat: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                };
                readonly kind: "object";
            };
            readonly result: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 16384;
                readonly maximumNodes: 256;
            };
        };
        readonly "standard.admin.track.set": {
            readonly error: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 8192;
                readonly maximumNodes: 128;
            };
            readonly request: {
                readonly fields: {
                    readonly entity: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly expectedRevision: {
                        readonly required: false;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly operation: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly policy: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "enum";
                            readonly values: readonly ["rejectOutOfBounds", "clampToBounds"];
                        };
                    };
                    readonly source: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "taggedUnion";
                            readonly tag: "kind";
                            readonly variants: {
                                readonly effect: {
                                    readonly fields: {
                                        readonly effect: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["effect"];
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly stack: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "integer";
                                                readonly maximum: 65535;
                                                readonly minimum: 0;
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly equippedItem: {
                                    readonly fields: {
                                        readonly item: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["equippedItem"];
                                            };
                                        };
                                        readonly owner: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly intrinsic: {
                                    readonly fields: {
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["intrinsic"];
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly request: {
                                    readonly fields: {
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["request"];
                                            };
                                        };
                                        readonly operation: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                            };
                        };
                    };
                    readonly track: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly value: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                };
                readonly kind: "object";
            };
            readonly result: {
                readonly fields: {
                    readonly after: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                    readonly before: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                    readonly catalogFingerprint: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 256;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly catalogVersion: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly committedTracksRevision: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly decision: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "enum";
                            readonly values: readonly ["applied", "clampedToBounds"];
                        };
                    };
                    readonly entity: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly maximum: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                    readonly minimum: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                    readonly observedRevisions: {
                        readonly required: true;
                        readonly value: {
                            readonly items: {
                                readonly fields: {
                                    readonly component: {
                                        readonly required: true;
                                        readonly value: {
                                            readonly kind: "string";
                                            readonly maximumBytes: 128;
                                            readonly pattern: "identifier";
                                        };
                                    };
                                    readonly entity: {
                                        readonly required: true;
                                        readonly value: {
                                            readonly kind: "decimalU64";
                                        };
                                    };
                                    readonly revision: {
                                        readonly required: true;
                                        readonly value: {
                                            readonly kind: "decimalU64";
                                        };
                                    };
                                };
                                readonly kind: "object";
                            };
                            readonly kind: "array";
                            readonly maximumItems: 32;
                        };
                    };
                    readonly observedTracksRevision: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                    readonly operation: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                    readonly policy: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "enum";
                            readonly values: readonly ["rejectOutOfBounds", "clampToBounds"];
                        };
                    };
                    readonly requested: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "integer";
                            readonly maximum: 1000000000000;
                            readonly minimum: -1000000000000;
                        };
                    };
                    readonly source: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "taggedUnion";
                            readonly tag: "kind";
                            readonly variants: {
                                readonly effect: {
                                    readonly fields: {
                                        readonly effect: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["effect"];
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly stack: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "integer";
                                                readonly maximum: 65535;
                                                readonly minimum: 0;
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly equippedItem: {
                                    readonly fields: {
                                        readonly item: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["equippedItem"];
                                            };
                                        };
                                        readonly owner: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly source: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly intrinsic: {
                                    readonly fields: {
                                        readonly entity: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "decimalU64";
                                            };
                                        };
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["intrinsic"];
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                                readonly request: {
                                    readonly fields: {
                                        readonly instance: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                        readonly kind: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "enum";
                                                readonly values: readonly ["request"];
                                            };
                                        };
                                        readonly operation: {
                                            readonly required: true;
                                            readonly value: {
                                                readonly kind: "string";
                                                readonly maximumBytes: 96;
                                                readonly pattern: "identifier";
                                            };
                                        };
                                    };
                                    readonly kind: "object";
                                };
                            };
                        };
                    };
                    readonly sourceCost: {
                        readonly required: true;
                        readonly value: {
                            readonly fields: {
                                readonly effectEntriesVisited: {
                                    readonly required: true;
                                    readonly value: {
                                        readonly kind: "integer";
                                        readonly minimum: 0;
                                    };
                                };
                                readonly effectSourceActivationsVisited: {
                                    readonly required: true;
                                    readonly value: {
                                        readonly kind: "integer";
                                        readonly minimum: 0;
                                    };
                                };
                                readonly equipmentEntriesVisited: {
                                    readonly required: true;
                                    readonly value: {
                                        readonly kind: "integer";
                                        readonly minimum: 0;
                                    };
                                };
                                readonly intrinsicEntriesVisited: {
                                    readonly required: true;
                                    readonly value: {
                                        readonly kind: "integer";
                                        readonly minimum: 0;
                                    };
                                };
                                readonly itemComponentsRead: {
                                    readonly required: true;
                                    readonly value: {
                                        readonly kind: "integer";
                                        readonly minimum: 0;
                                    };
                                };
                                readonly requestEntriesVisited: {
                                    readonly required: true;
                                    readonly value: {
                                        readonly kind: "integer";
                                        readonly minimum: 0;
                                    };
                                };
                            };
                            readonly kind: "object";
                        };
                    };
                    readonly track: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "string";
                            readonly maximumBytes: 96;
                            readonly pattern: "identifier";
                        };
                    };
                };
                readonly kind: "object";
            };
        };
        readonly "standard.inspect.entity": {
            readonly error: {
                readonly fields: {};
                readonly kind: "object";
            };
            readonly request: {
                readonly fields: {
                    readonly entity: {
                        readonly required: true;
                        readonly value: {
                            readonly kind: "decimalU64";
                        };
                    };
                };
                readonly kind: "object";
            };
            readonly result: {
                readonly kind: "opaqueJson";
                readonly maximumBytes: 65536;
                readonly maximumNodes: 2048;
            };
        };
    };
    readonly kind: "rusty-developer-command-standard-host-wire.v1";
};
