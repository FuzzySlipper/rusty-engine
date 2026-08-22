// This file is generated from the Rust-owned developer-command wire contract.
// Regenerate with: cargo run -p developer-command --bin export-wire-contract > render/contracts/developer-command-contract.json
const CONTRACT = {
  "kind": "rusty-developer-command.v1",
  "protocolVersion": 1,
  "identity": {
    "commandBytes": 128,
    "correlationBytes": 128,
    "runtimeBytes": 128,
    "profileBytes": 128,
    "charset": "lowercase-ascii-alnum-dot-dash-underscore-colon"
  },
  "limits": {
    "commandAliases": 8,
    "summaryBytes": 256,
    "historyEntries": 256
  },
  "lanes": [
    "inspect",
    "preview",
    "play",
    "admin",
    "session",
    "author",
    "fault"
  ],
  "discoveryFields": [
    "protocolVersion",
    "runtime",
    "profile",
    "permittedLanes",
    "revision",
    "catalogEpoch",
    "contractFingerprint",
    "commands"
  ],
  "requestFields": [
    "protocolVersion",
    "command",
    "correlation",
    "runtime",
    "expected",
    "payload"
  ],
  "responseFields": [
    "correlation",
    "runtime",
    "profile",
    "revision",
    "catalogEpoch",
    "outcome"
  ],
  "outcomes": [
    "success",
    "error"
  ],
  "sequence": {
    "kind": "rusty_developer_command.sequence.v1",
    "deterministicReplay": false,
    "requiredEntryFacts": [
      "runtime",
      "profile",
      "revision",
      "catalogEpoch",
      "outcome",
      "receiptRefs"
    ]
  }
} as const;
export const GENERATED_DEVELOPER_COMMAND_CONTRACT = Object.freeze(CONTRACT);
