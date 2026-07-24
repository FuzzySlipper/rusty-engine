import type {
  PresentationFrameDiff,
  PresentationHostDiagnostic,
  PresentationOp,
} from '@rusty-engine/render-contracts';
import { decodePresentationFrameDiff } from '@rusty-engine/render-contracts';

export type RendererPresentationDomain = PresentationOp['domain'];

interface PresentationDomainReceipt {
  readonly applied: number;
  readonly diagnostics: readonly {
    readonly code: string;
    readonly sequence: number;
    readonly handle: number | null;
    readonly message: string;
  }[];
}

export interface RendererPresentationDomainHost {
  readonly applyPresentation: (
    frame: PresentationFrameDiff,
  ) => PresentationDomainReceipt | Promise<PresentationDomainReceipt>;
}

export interface RendererAdvancingPresentationDomainHost
  extends RendererPresentationDomainHost {
  readonly advance: (deltaSeconds: number) => PresentationDomainReceipt;
}

export interface RendererPresentationHosts {
  readonly animation?: RendererAdvancingPresentationDomainHost;
  readonly audio?: RendererPresentationDomainHost;
  readonly billboard?: RendererPresentationDomainHost;
  readonly particle?: RendererAdvancingPresentationDomainHost;
  readonly telemetryOverlay?: RendererPresentationDomainHost;
}

export interface RendererPresentationDomainReceipt {
  readonly domain: RendererPresentationDomain;
  readonly configured: boolean;
  readonly requested: number;
  readonly applied: number;
  readonly diagnostics: readonly PresentationHostDiagnostic[];
}

export interface RendererPresentationFrameReceipt {
  readonly schemaVersion: 1;
  readonly applied: number;
  readonly domains: readonly RendererPresentationDomainReceipt[];
  readonly diagnostics: readonly PresentationHostDiagnostic[];
}

export interface RendererPresentationAdvanceReceipt {
  readonly schemaVersion: 1;
  readonly advancedDomains: readonly RendererPresentationDomain[];
  readonly applied: number;
  readonly diagnostics: readonly PresentationHostDiagnostic[];
}

/**
 * A small, typed fan-out over optional presentation mechanisms.
 *
 * The set never accepts gameplay state and does not infer missing mechanisms.
 * Every requested domain without a configured host produces an explicit
 * `unavailableHost` diagnostic.
 */
export class RendererPresentationHostSet {
  readonly #hosts: RendererPresentationHosts;

  constructor(hosts: RendererPresentationHosts) {
    this.#hosts = { ...hosts };
  }

  async apply(frame: PresentationFrameDiff): Promise<RendererPresentationFrameReceipt> {
    decodePresentationFrameDiff(frame);

    const domains: RendererPresentationDomainReceipt[] = [];
    for (const domain of PRESENTATION_DOMAIN_ORDER) {
      const operations = frame.ops.filter((operation) => operation.domain === domain);
      const host = this.#hosts[domain];
      if (host === undefined) {
        const diagnostics = operations.map((operation) => unavailableDiagnostic(operation));
        domains.push({
          domain,
          configured: false,
          requested: operations.length,
          applied: 0,
          diagnostics,
        });
        continue;
      }
      if (operations.length === 0) {
        domains.push({ domain, configured: true, requested: 0, applied: 0, diagnostics: [] });
        continue;
      }
      const receipt = await host.applyPresentation({ schemaVersion: 1, ops: operations });
      domains.push({
        domain,
        configured: true,
        requested: operations.length,
        applied: receipt.applied,
        diagnostics: receipt.diagnostics.map((diagnostic) => ({ domain, ...diagnostic })),
      });
    }

    return frameReceipt(domains);
  }

  advance(deltaSeconds: number): RendererPresentationAdvanceReceipt {
    if (!Number.isFinite(deltaSeconds) || deltaSeconds < 0) {
      throw new RangeError('presentation deltaSeconds must be finite and non-negative');
    }
    const advancedDomains: RendererPresentationDomain[] = [];
    const diagnostics: PresentationHostDiagnostic[] = [];
    let applied = 0;
    for (const domain of ADVANCING_DOMAIN_ORDER) {
      const host = this.#hosts[domain];
      if (host === undefined) {
        continue;
      }
      const receipt = host.advance(deltaSeconds);
      advancedDomains.push(domain);
      applied += receipt.applied;
      diagnostics.push(...receipt.diagnostics.map((diagnostic) => ({ domain, ...diagnostic })));
    }
    return { schemaVersion: 1, advancedDomains, applied, diagnostics };
  }
}

const PRESENTATION_DOMAIN_ORDER: readonly RendererPresentationDomain[] = [
  'animation',
  'audio',
  'billboard',
  'particle',
  'telemetryOverlay',
];

const ADVANCING_DOMAIN_ORDER: readonly (
  'animation' | 'particle'
)[] = ['animation', 'particle'];

function unavailableDiagnostic(operation: PresentationOp): PresentationHostDiagnostic {
  return {
    domain: operation.domain,
    code: 'unavailableHost',
    sequence: operation.meta.sequence,
    handle: operationHandle(operation),
    message: `${operation.domain} presentation was requested without a configured host`,
  };
}

function operationHandle(operation: PresentationOp): number | null {
  const op = operation.op;
  return 'handle' in op ? op.handle as number : null;
}

function frameReceipt(
  domains: readonly RendererPresentationDomainReceipt[],
): RendererPresentationFrameReceipt {
  return {
    schemaVersion: 1,
    applied: domains.reduce((total, domain) => total + domain.applied, 0),
    domains,
    diagnostics: domains.flatMap((domain) => domain.diagnostics),
  };
}
