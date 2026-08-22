export declare const GENERATED_DEVELOPER_COMMAND_CONTRACT: Readonly<{
    readonly kind: "rusty-developer-command.v1";
    readonly protocolVersion: 1;
    readonly identity: {
        readonly commandBytes: 128;
        readonly correlationBytes: 128;
        readonly runtimeBytes: 128;
        readonly profileBytes: 128;
        readonly charset: "lowercase-ascii-alnum-dot-dash-underscore-colon";
    };
    readonly limits: {
        readonly commandAliases: 8;
        readonly summaryBytes: 256;
        readonly historyEntries: 256;
    };
    readonly lanes: readonly ["inspect", "preview", "play", "admin", "session", "author", "fault"];
    readonly discoveryFields: readonly ["protocolVersion", "runtime", "profile", "permittedLanes", "revision", "catalogEpoch", "contractFingerprint", "commands"];
    readonly requestFields: readonly ["protocolVersion", "command", "correlation", "runtime", "expected", "payload"];
    readonly responseFields: readonly ["correlation", "runtime", "profile", "revision", "catalogEpoch", "outcome"];
    readonly outcomes: readonly ["success", "error"];
    readonly sequence: {
        readonly kind: "rusty_developer_command.sequence.v1";
        readonly deterministicReplay: false;
        readonly requiredEntryFacts: readonly ["runtime", "profile", "revision", "catalogEpoch", "outcome", "receiptRefs"];
    };
}>;
