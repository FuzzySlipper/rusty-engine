import { type RendererMeshResourceDescriptor, type RendererMeshResourceManifest, type RendererAudioResourceResolver, type RendererAnimatedMeshResourceManifest, type RendererAnimatedMeshResourceResolver, type RendererTextureResourceDescriptor, type RendererTextureResourceManifest } from '@rusty-engine/renderer-host';
import type { RustyApplicationFrame } from './application-host.js';
import type { RenderPublicationFrontier } from '@rusty-engine/render-contracts';
export type RustyApplicationResourceKind = 'animatedMesh' | 'audio' | 'mesh' | 'clipPack' | 'texture';
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
    readonly publicationFrontiers?: readonly RenderPublicationFrontier[];
}
export type RustyApplicationContentDiagnosticCode = 'content_invalid' | 'resource_duplicate' | 'resource_identity_invalid' | 'resource_limit_exceeded' | 'resource_media_type_unsupported';
export declare class RustyApplicationContentError extends Error {
    readonly code: RustyApplicationContentDiagnosticCode;
    readonly resource: string | null;
    constructor(code: RustyApplicationContentDiagnosticCode, resource: string | null, message: string);
}
export interface PreparedRustyApplicationResource {
    readonly identity: string;
    readonly contentHash: string;
    readonly mediaType: string;
    readonly bytes: ArrayBuffer;
    readonly kind: RustyApplicationResourceKind;
}
export interface PreparedRustyApplicationContent {
    readonly frame: RustyApplicationFrame;
    readonly resources: readonly PreparedRustyApplicationResource[];
    readonly resourceBytes: number;
    readonly publicationFrontiers: readonly RenderPublicationFrontier[];
}
export interface RustyApplicationSurfaceResourceOptions {
    readonly animatedMeshManifest?: RendererAnimatedMeshResourceManifest;
    readonly resolveAnimatedMeshResource?: RendererAnimatedMeshResourceResolver;
    readonly meshResourceManifest?: RendererMeshResourceManifest;
    readonly resolveMeshResource?: (descriptor: RendererMeshResourceDescriptor) => Promise<ArrayBuffer>;
    readonly textureResourceManifest?: RendererTextureResourceManifest;
    readonly resolveTextureResource?: (descriptor: RendererTextureResourceDescriptor) => Promise<ArrayBuffer>;
}
export declare function prepareRustyApplicationContent(content: RustyApplicationContent): PreparedRustyApplicationContent;
export declare function rustyApplicationAudioResourceResolver(content: PreparedRustyApplicationContent): RendererAudioResourceResolver;
export declare function rustyApplicationSurfaceResourceOptions(content: PreparedRustyApplicationContent): RustyApplicationSurfaceResourceOptions;
