import type { RustyApplicationFrame } from './application-host.js';
export type RustyApplicationResourceKind = 'audio' | 'mesh' | 'clipPack' | 'texture';
export declare const RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_BYTES: number;
export declare const RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_COUNT = 64;
export declare const RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_TOTAL_BYTES: number;
export interface RustyApplicationResource {
    readonly identity: string;
    readonly contentHash: string;
    readonly mediaType: string;
    readonly bytes: Uint8Array;
}
export interface RustyApplicationContent {
    readonly frame: RustyApplicationFrame;
    readonly resources?: readonly RustyApplicationResource[];
}
export type RustyApplicationContentDiagnosticCode = 'content_invalid' | 'resource_duplicate' | 'resource_identity_invalid' | 'resource_limit_exceeded' | 'resource_media_type_unsupported';
export declare class RustyApplicationContentError extends Error {
    readonly code: RustyApplicationContentDiagnosticCode;
    readonly resource: string | null;
    constructor(code: RustyApplicationContentDiagnosticCode, resource: string | null, message: string);
}
