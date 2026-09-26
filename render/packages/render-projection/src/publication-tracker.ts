import type { RenderFramePublication, RenderPublicationFrontier } from '@rusty-engine/render-contracts';

export class RenderPublicationError extends Error {}

/** Continuation state only; no retained graphics mirror. */
export class RenderPublicationTracker {
  #publishedRevisions = new Map<string, number>();
  /** Read-only continuation check for graphics and other presentation domains. */
  validatePublication(publication: RenderFramePublication | undefined, operationCount: number): void {
    if (publication !== undefined) {
      if (publication.operationCount !== operationCount) {
        throw new RenderPublicationError(
          `publication ${publication.stream} operationCount does not match frame`,
        );
      }
      if (publication.revision !== publication.baseRevision + 1) {
        throw new RenderPublicationError(
          `publication gap for ${publication.stream}; revision ${String(publication.revision)} must immediately follow base ${String(publication.baseRevision)}`,
        );
      }
      const previous = this.#publishedRevisions.get(publication.stream);
      if (previous !== undefined && publication.revision <= previous) {
        throw new RenderPublicationError(
          `stale publication ${publication.stream} revision ${String(publication.revision)}; latest is ${String(previous)}`,
        );
      }
      const expectedBase = previous ?? 0;
      if (publication.baseRevision !== expectedBase) {
        throw new RenderPublicationError(
          `publication gap for ${publication.stream}; expected base ${String(expectedBase)}, received ${String(publication.baseRevision)}`,
        );
      }
    }
  }

  /** Recheck after realization, which may have yielded to another publication. */
  commitPublication(publication: RenderFramePublication | undefined, operationCount: number): void {
    this.validatePublication(publication, operationCount);
    if (publication !== undefined) {
      this.#publishedRevisions.set(publication.stream, publication.revision);
    }
  }

  /** Atomically install all active stream continuation points for a complete retained replacement. */
  replacePublicationFrontiers(frontiers: readonly RenderPublicationFrontier[]): void {
    const next = new Map<string, number>();
    for (const frontier of frontiers) {
      if (typeof frontier.stream !== 'string' || frontier.stream.trim().length === 0
        || frontier.stream.length > 256) {
        throw new RenderPublicationError('publication frontier stream must contain 1..=256 characters');
      }
      if (!Number.isSafeInteger(frontier.revision) || frontier.revision < 0) {
        throw new RenderPublicationError(`publication frontier ${frontier.stream} revision must be a JSON-safe unsigned integer`);
      }
      if (next.has(frontier.stream)) {
        throw new RenderPublicationError(`duplicate publication frontier ${frontier.stream}`);
      }
      next.set(frontier.stream, frontier.revision);
    }
    this.#publishedRevisions = next;
  }


  publicationFrontiers(): readonly RenderPublicationFrontier[] {
    return [...this.#publishedRevisions].map(([stream, revision]) => ({ stream, revision }));
  }
}
