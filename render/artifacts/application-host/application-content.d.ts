import { type RendererMeshResourceDescriptor, type RendererMeshResourceManifest, type RendererAudioResourceResolver, type RendererAnimatedMeshResourceManifest, type RendererAnimatedMeshResourceResolver, type RendererTextureResourceDescriptor, type RendererTextureResourceManifest, RendererMutableAnimatedMeshResourceSource, RendererMutableMeshResourceSource, RendererMutableTextureResourceSource } from '@rusty-engine/renderer-host';
import type { RustyApplicationFrame } from './application-host.js';
import type { RenderPublicationFrontier } from '@rusty-engine/render-contracts';
export type RustyApplicationResourceKind = 'animatedMesh' | 'audio' | 'mesh' | 'clipPack' | 'texture' | 'font';
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
/** Renderer resolvers borrow prepared bytes. Consumers must not mutate or detach
 * them; renderer resource loaders snapshot them when admitting resources. */
export interface RustyApplicationSurfaceResourceOptions {
    readonly animatedMeshManifest?: RendererAnimatedMeshResourceManifest;
    readonly resolveAnimatedMeshResource?: RendererAnimatedMeshResourceResolver;
    readonly meshResourceManifest?: RendererMeshResourceManifest;
    readonly resolveMeshResource?: (descriptor: RendererMeshResourceDescriptor) => Promise<ArrayBuffer>;
    readonly textureResourceManifest?: RendererTextureResourceManifest;
    readonly resolveTextureResource?: (descriptor: RendererTextureResourceDescriptor) => Promise<ArrayBuffer>;
}
export declare function prepareRustyApplicationContent(content: RustyApplicationContent): PreparedRustyApplicationContent;
/** One mutable Engine-owned resource catalog shared by a mounted surface and
 * its presentation hosts. Product Browser admits immutable bytes here before
 * applying the output group that names them. */
export declare class RustyApplicationResourceCatalog {
    #private;
    readonly meshSource: RendererMutableMeshResourceSource;
    readonly textureSource: RendererMutableTextureResourceSource;
    readonly animatedSource: RendererMutableAnimatedMeshResourceSource;
    admit(resources: readonly (RustyApplicationResource | PreparedRustyApplicationResource)[], frame?: RustyApplicationFrame): Promise<void>;
    resource(identity: string, hash?: string): PreparedRustyApplicationResource | undefined;
    snapshot(): readonly PreparedRustyApplicationResource[];
    retainOnly(identities: ReadonlySet<string>): void;
    readout(): {
        readonly resources: number;
        readonly animated: number;
        readonly clipPacks: number;
    };
    clear(): void;
    audioResolver(): RendererAudioResourceResolver;
}
export declare function rustyApplicationAudioResourceResolver(content: PreparedRustyApplicationContent): RendererAudioResourceResolver;
export declare function rustyApplicationSurfaceResourceOptions(content: PreparedRustyApplicationContent): RustyApplicationSurfaceResourceOptions;
