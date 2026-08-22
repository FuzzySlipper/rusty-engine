// Generated from developer-command-standard host DTO schema.
export const GENERATED_STANDARD_HOST_WIRE = {
    "commands": {
        "standard.admin.effect.apply": {
            "error": {
                "kind": "opaqueJson",
                "maximumBytes": 8192,
                "maximumNodes": 128
            },
            "request": {
                "fields": {
                    "definition": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "entity": {
                        "required": true,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "expectedRevision": {
                        "required": false,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "instance": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "operation": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "provenance": {
                        "required": true,
                        "value": {
                            "kind": "taggedUnion",
                            "tag": "kind",
                            "variants": {
                                "effect": {
                                    "fields": {
                                        "effect": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "entity": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "effect"
                                                ]
                                            }
                                        },
                                        "source": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "stack": {
                                            "required": true,
                                            "value": {
                                                "kind": "integer",
                                                "maximum": 65535,
                                                "minimum": 0
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "equippedItem": {
                                    "fields": {
                                        "item": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "equippedItem"
                                                ]
                                            }
                                        },
                                        "owner": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "source": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "intrinsic": {
                                    "fields": {
                                        "entity": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "instance": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "intrinsic"
                                                ]
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "request": {
                                    "fields": {
                                        "instance": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "request"
                                                ]
                                            }
                                        },
                                        "operation": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        }
                                    },
                                    "kind": "object"
                                }
                            }
                        }
                    },
                    "stacks": {
                        "required": true,
                        "value": {
                            "kind": "integer",
                            "maximum": 65535,
                            "minimum": 1
                        }
                    }
                },
                "kind": "object"
            },
            "result": {
                "kind": "opaqueJson",
                "maximumBytes": 16384,
                "maximumNodes": 256
            }
        },
        "standard.admin.effect.remove": {
            "error": {
                "kind": "opaqueJson",
                "maximumBytes": 8192,
                "maximumNodes": 128
            },
            "request": {
                "fields": {
                    "entity": {
                        "required": true,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "expectedRevision": {
                        "required": false,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "instance": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "operation": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    }
                },
                "kind": "object"
            },
            "result": {
                "kind": "opaqueJson",
                "maximumBytes": 16384,
                "maximumNodes": 256
            }
        },
        "standard.admin.stat.set-base": {
            "error": {
                "kind": "opaqueJson",
                "maximumBytes": 8192,
                "maximumNodes": 128
            },
            "request": {
                "fields": {
                    "base": {
                        "required": true,
                        "value": {
                            "kind": "integer",
                            "maximum": 1000000000000,
                            "minimum": -1000000000000
                        }
                    },
                    "entity": {
                        "required": true,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "expectedRevision": {
                        "required": false,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "operation": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "source": {
                        "required": true,
                        "value": {
                            "kind": "taggedUnion",
                            "tag": "kind",
                            "variants": {
                                "effect": {
                                    "fields": {
                                        "effect": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "entity": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "effect"
                                                ]
                                            }
                                        },
                                        "source": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "stack": {
                                            "required": true,
                                            "value": {
                                                "kind": "integer",
                                                "maximum": 65535,
                                                "minimum": 0
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "equippedItem": {
                                    "fields": {
                                        "item": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "equippedItem"
                                                ]
                                            }
                                        },
                                        "owner": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "source": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "intrinsic": {
                                    "fields": {
                                        "entity": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "instance": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "intrinsic"
                                                ]
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "request": {
                                    "fields": {
                                        "instance": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "request"
                                                ]
                                            }
                                        },
                                        "operation": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        }
                                    },
                                    "kind": "object"
                                }
                            }
                        }
                    },
                    "stat": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    }
                },
                "kind": "object"
            },
            "result": {
                "kind": "opaqueJson",
                "maximumBytes": 16384,
                "maximumNodes": 256
            }
        },
        "standard.admin.track.set": {
            "error": {
                "kind": "opaqueJson",
                "maximumBytes": 8192,
                "maximumNodes": 128
            },
            "request": {
                "fields": {
                    "entity": {
                        "required": true,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "expectedRevision": {
                        "required": false,
                        "value": {
                            "kind": "decimalU64"
                        }
                    },
                    "operation": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "policy": {
                        "required": true,
                        "value": {
                            "kind": "enum",
                            "values": [
                                "rejectOutOfBounds",
                                "clampToBounds"
                            ]
                        }
                    },
                    "source": {
                        "required": true,
                        "value": {
                            "kind": "taggedUnion",
                            "tag": "kind",
                            "variants": {
                                "effect": {
                                    "fields": {
                                        "effect": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "entity": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "effect"
                                                ]
                                            }
                                        },
                                        "source": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "stack": {
                                            "required": true,
                                            "value": {
                                                "kind": "integer",
                                                "maximum": 65535,
                                                "minimum": 0
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "equippedItem": {
                                    "fields": {
                                        "item": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "equippedItem"
                                                ]
                                            }
                                        },
                                        "owner": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "source": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "intrinsic": {
                                    "fields": {
                                        "entity": {
                                            "required": true,
                                            "value": {
                                                "kind": "decimalU64"
                                            }
                                        },
                                        "instance": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "intrinsic"
                                                ]
                                            }
                                        }
                                    },
                                    "kind": "object"
                                },
                                "request": {
                                    "fields": {
                                        "instance": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        },
                                        "kind": {
                                            "required": true,
                                            "value": {
                                                "kind": "enum",
                                                "values": [
                                                    "request"
                                                ]
                                            }
                                        },
                                        "operation": {
                                            "required": true,
                                            "value": {
                                                "kind": "string",
                                                "maximumBytes": 96,
                                                "pattern": "identifier"
                                            }
                                        }
                                    },
                                    "kind": "object"
                                }
                            }
                        }
                    },
                    "track": {
                        "required": true,
                        "value": {
                            "kind": "string",
                            "maximumBytes": 96,
                            "pattern": "identifier"
                        }
                    },
                    "value": {
                        "required": true,
                        "value": {
                            "kind": "integer",
                            "maximum": 1000000000000,
                            "minimum": -1000000000000
                        }
                    }
                },
                "kind": "object"
            },
            "result": {
                "kind": "opaqueJson",
                "maximumBytes": 16384,
                "maximumNodes": 256
            }
        }
    },
    "kind": "rusty-developer-command-standard-host-wire.v1"
};
