/** Numeric host policy for the shared, frame-local presentation viewport. */
export interface RustyApplicationPresentationAspectBounds {
    readonly minimum: number;
    readonly maximum: number;
}
export interface RustyApplicationPresentationFrameGeometry {
    readonly height: number;
    readonly width: number;
}
/**
 * Reject malformed host policy before any mount DOM is published.  The bounds
 * deliberately carry no product, platform, or responsive-layout vocabulary.
 */
export declare function validatePresentationAspectBounds(value: RustyApplicationPresentationAspectBounds | undefined): RustyApplicationPresentationAspectBounds | undefined;
/**
 * Derive the largest frame that fits the actual mount container. A zero-sized
 * transient container is represented as a valid zero-sized frame rather than
 * producing invalid CSS or inventing a viewport.
 */
export declare function resolvePresentationFrameGeometry(containerWidth: number, containerHeight: number, bounds: RustyApplicationPresentationAspectBounds): RustyApplicationPresentationFrameGeometry;
