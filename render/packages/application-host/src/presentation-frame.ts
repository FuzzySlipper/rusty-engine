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
export function validatePresentationAspectBounds(
  value: RustyApplicationPresentationAspectBounds | undefined,
): RustyApplicationPresentationAspectBounds | undefined {
  if (value === undefined) return undefined;
  if (!Number.isFinite(value.minimum) || !Number.isFinite(value.maximum)
    || value.minimum <= 0 || value.maximum <= 0 || value.minimum > value.maximum) {
    throw new RangeError(
      'presentationAspectBounds minimum and maximum must be finite positive numbers with minimum <= maximum',
    );
  }
  return Object.freeze({ minimum: value.minimum, maximum: value.maximum });
}

/**
 * Derive the largest frame that fits the actual mount container. A zero-sized
 * transient container is represented as a valid zero-sized frame rather than
 * producing invalid CSS or inventing a viewport.
 */
export function resolvePresentationFrameGeometry(
  containerWidth: number,
  containerHeight: number,
  bounds: RustyApplicationPresentationAspectBounds,
): RustyApplicationPresentationFrameGeometry {
  if (!Number.isFinite(containerWidth) || !Number.isFinite(containerHeight)
    || containerWidth <= 0 || containerHeight <= 0) {
    return Object.freeze({ width: 0, height: 0 });
  }
  const containerAspect = containerWidth / containerHeight;
  if (containerAspect < bounds.minimum) {
    return Object.freeze({ width: containerWidth, height: containerWidth / bounds.minimum });
  }
  if (containerAspect > bounds.maximum) {
    return Object.freeze({ width: containerHeight * bounds.maximum, height: containerHeight });
  }
  return Object.freeze({ width: containerWidth, height: containerHeight });
}
