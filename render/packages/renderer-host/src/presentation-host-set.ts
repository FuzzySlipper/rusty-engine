import type {
  PresentationFrameDiff,
  PresentationHostDiagnostic,
  PresentationOp,
} from '@rusty-engine/render-contracts';
import { ContractDecodeError, decodePresentationFrameDiff } from '@rusty-engine/render-contracts';
import type {
  RendererAudioListenerPose,
  RendererAudioRealizedFactsReadout,
} from './audio-host.js';
import type { RendererAnimationRealizedFactsReadout } from './animation-host.js';
import type { AudioProjectionDiagnostic } from './host-types.js';
import type { RendererGhostPlateReadout } from './ghost-plate-host.js';

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
  /**
   * Reports whether this mechanism currently needs display-clock advancement.
   *
   * Custom advancing hosts that omit this method retain the conservative
   * always-advance behavior.
   */
  readonly requiresAnimationFrame?: () => boolean;
}

interface RendererAudioListenerPresentationHost extends RendererPresentationDomainHost {
  readonly updateListener?: (
    pose: RendererAudioListenerPose,
  ) => readonly AudioProjectionDiagnostic[];
  readonly realizedFacts?: () => RendererAudioRealizedFactsReadout;
  readonly acknowledgeRealizedFacts?: (throughFactId: number) => void;
  readonly reset?: () => void;
}

interface RendererAnimationPresentationHost extends RendererAdvancingPresentationDomainHost {
  readonly realizedFacts?: () => RendererAnimationRealizedFactsReadout;
  readonly acknowledgeRealizedFacts?: (throughFactId: number) => void;
  readonly reset?: () => void;
}

export interface RendererPresentationHosts {
  readonly animation?: RendererAnimationPresentationHost;
  readonly audio?: RendererAudioListenerPresentationHost;
  readonly billboard?: RendererAdvancingPresentationDomainHost;
  readonly particle?: RendererAdvancingPresentationDomainHost;
  readonly telemetryOverlay?: RendererPresentationDomainHost;
  readonly ghostPlate?: RendererPresentationDomainHost & { readonly readout?: () => RendererGhostPlateReadout; readonly dispose?: () => void };
}

/** Typed receipt for the Engine-owned camera-to-audio listener handoff. */
export interface RendererPresentationListenerSyncReceipt {
  readonly schemaVersion: 1;
  /** Whether an audio presentation host is attached. */
  readonly configured: boolean;
  /** Whether the attached audio host accepts listener synchronization. */
  readonly applied: boolean;
  /** Diagnostics are retained by the audio host and surfaced here for callers that need them. */
  readonly diagnostics: readonly AudioProjectionDiagnostic[];
}

export interface RendererPresentationDomainReceipt {
  readonly domain: RendererPresentationDomain;
  readonly configured: boolean;
  readonly requested: number;
  readonly applied: number;
  /** `rejected_atomic` describes only this domain's rejected operations. */
  readonly outcome: 'applied' | 'partial' | 'rejected_atomic' | 'terminal';
  readonly diagnostics: readonly PresentationHostDiagnostic[];
}

export interface RendererPresentationFrameReceipt {
  readonly schemaVersion: 1;
  readonly applied: number;
  /** A mixed frame may have applied other independent presentation domains. */
  readonly outcome: 'applied' | 'partial' | 'rejected_atomic' | 'terminal';
  readonly domains: readonly RendererPresentationDomainReceipt[];
  readonly diagnostics: readonly PresentationHostDiagnostic[];
}

export interface RendererPresentationAdvanceReceipt {
  readonly schemaVersion: 1;
  readonly advancedDomains: readonly RendererPresentationDomain[];
  readonly applied: number;
  readonly diagnostics: readonly PresentationHostDiagnostic[];
}

/** A bounded, coalesced record of an optional presentation host failure. */
export interface RendererPresentationHostFailure {
  readonly domain: RendererPresentationDomain;
  readonly stage: 'apply' | 'advance' | 'listenerSync' | 'dispose';
  readonly message: string;
  readonly occurrences: number;
}

export interface RendererPresentationHostFailureReadout {
  readonly retainedFailureCount: number;
  readonly evictedFailureCount: number;
  readonly failures: readonly RendererPresentationHostFailure[];
}

const MAX_RETAINED_HOST_FAILURES = 64;

/**
 * A small, typed fan-out over optional presentation mechanisms.
 *
 * The set never accepts gameplay state and does not infer missing mechanisms.
 * Every requested domain without a configured host produces an explicit
 * `unavailableHost` diagnostic.
 */
export class RendererPresentationHostSet {
  readonly #hosts: RendererPresentationHosts;
  readonly #degradedDomains = new Set<RendererPresentationDomain>();
  readonly #failures: RendererPresentationHostFailure[] = [];
  #evictedFailureCount = 0;

  constructor(hosts: RendererPresentationHosts) {
    this.#hosts = { ...hosts };
  }

  async apply(frame: PresentationFrameDiff): Promise<RendererPresentationFrameReceipt> {
    try {
      decodePresentationFrameDiff(frame);
    } catch (cause) {
      if (cause instanceof ContractDecodeError) {
        throw new RendererPresentationFrameValidationError(cause);
      }
      throw cause;
    }

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
          outcome: operations.length === 0 ? 'applied' : 'rejected_atomic',
          diagnostics,
        });
        continue;
      }
      if (operations.length === 0) {
        domains.push({
          domain, configured: true, requested: 0, applied: 0, outcome: 'applied', diagnostics: [],
        });
        continue;
      }
      if (this.#degradedDomains.has(domain)) {
        domains.push(degradedReceipt(domain, operations));
        continue;
      }
      let receipt: PresentationDomainReceipt;
      try {
        receipt = await host.applyPresentation({ schemaVersion: 1, ops: operations });
      } catch (cause) {
        this.#degrade(domain, 'apply', cause);
        domains.push(degradedReceipt(domain, operations));
        continue;
      }
      if (receipt.diagnostics.some((diagnostic) => diagnostic.code === 'hostFailure')) {
        this.#degrade(domain, 'apply', receipt.diagnostics[0]?.message ?? 'presentation host failure');
      }
      domains.push({
        domain,
        configured: true,
        requested: operations.length,
        applied: receipt.applied,
        outcome: presentationDomainOutcome(receipt),
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
      if (this.#degradedDomains.has(domain)) continue;
      let receipt: PresentationDomainReceipt;
      try {
        receipt = host.advance(deltaSeconds);
      } catch (cause) {
        this.#degrade(domain, 'advance', cause);
        continue;
      }
      if (receipt.diagnostics.some((diagnostic) => diagnostic.code === 'hostFailure')) {
        this.#degrade(domain, 'advance', receipt.diagnostics[0]?.message ?? 'presentation host failure');
      }
      advancedDomains.push(domain);
      applied += receipt.applied;
      diagnostics.push(...receipt.diagnostics.map((diagnostic) => ({ domain, ...diagnostic })));
    }
    return { schemaVersion: 1, advancedDomains, applied, diagnostics };
  }

  syncListener(pose: RendererAudioListenerPose): RendererPresentationListenerSyncReceipt {
    const host = this.#hosts.audio;
    if (host === undefined) {
      return { schemaVersion: 1, configured: false, applied: false, diagnostics: [] };
    }
    if (!hasListenerSynchronization(host)) {
      return { schemaVersion: 1, configured: true, applied: false, diagnostics: [] };
    }
    if (this.#degradedDomains.has('audio')) {
      return listenerFailureReceipt('audio presentation host is degraded after an earlier failure');
    }
    let diagnostics: readonly AudioProjectionDiagnostic[];
    try {
      diagnostics = host.updateListener(pose);
    } catch (cause) {
      this.#degrade('audio', 'listenerSync', cause);
      return listenerFailureReceipt(errorMessage(cause));
    }
    return {
      schemaVersion: 1,
      configured: true,
      applied: diagnostics.length === 0,
      diagnostics,
    };
  }

  /** Renderer-realized audio feedback, never projection/admission state. */
  readAudioRealizedFacts(): RendererAudioRealizedFactsReadout | null {
    const host = this.#hosts.audio;
    return host?.realizedFacts?.() ?? null;
  }

  /** Acknowledge audio facts through the submitted ID while preserving later arrivals. */
  acknowledgeAudioRealizedFacts(throughFactId: number): boolean {
    const host = this.#hosts.audio;
    if (host?.acknowledgeRealizedFacts === undefined) return false;
    host.acknowledgeRealizedFacts(throughFactId);
    return true;
  }

  /** Replace the current audio realization owner and invalidate its callbacks. */
  resetAudioRealizationOwner(): boolean {
    const host = this.#hosts.audio;
    if (host?.reset === undefined) return false;
    host.reset();
    return true;
  }

  /** Renderer-observed animation facts; status observations are not terminal claims. */
  readAnimationRealizedFacts(): RendererAnimationRealizedFactsReadout | null {
    return this.#hosts.animation?.realizedFacts?.() ?? null;
  }

  readGhostPlate(): RendererGhostPlateReadout | null {
    return this.#hosts.ghostPlate?.readout?.() ?? null;
  }

  /** Acknowledge only the exact submitted animation feedback boundary. */
  acknowledgeAnimationRealizedFacts(throughFactId: number): boolean {
    const host = this.#hosts.animation;
    if (host?.acknowledgeRealizedFacts === undefined) return false;
    host.acknowledgeRealizedFacts(throughFactId);
    return true;
  }

  /** Replace the animation feedback owner without recycling fact identifiers. */
  resetAnimationRealizationOwner(): boolean {
    const host = this.#hosts.animation;
    if (host?.reset === undefined) return false;
    host.reset();
    return true;
  }

  requiresAnimationFrame(): boolean {
    return ADVANCING_DOMAIN_ORDER.some((domain) => {
      const host = this.#hosts[domain];
      return !this.#degradedDomains.has(domain)
        && host !== undefined
        && (host.requiresAnimationFrame?.() ?? true);
    });
  }

  /** Optional host failures are coalesced so a stuck mechanism cannot grow diagnostics per frame. */
  failureReadout(): RendererPresentationHostFailureReadout {
    return Object.freeze({
      retainedFailureCount: this.#failures.length,
      evictedFailureCount: this.#evictedFailureCount,
      failures: Object.freeze(this.#failures.map((failure) => Object.freeze({ ...failure }))),
    });
  }

  dispose(): void {
    try {
      this.#hosts.ghostPlate?.dispose?.();
    } catch (cause) {
      this.#degrade('ghostPlate', 'dispose', cause);
    }
  }

  #degrade(
    domain: RendererPresentationDomain,
    stage: RendererPresentationHostFailure['stage'],
    cause: unknown,
  ): void {
    this.#degradedDomains.add(domain);
    const message = errorMessage(cause);
    const existing = this.#failures.find((failure) => (
      failure.domain === domain && failure.stage === stage && failure.message === message
    ));
    if (existing !== undefined) {
      const index = this.#failures.indexOf(existing);
      this.#failures[index] = { ...existing, occurrences: existing.occurrences + 1 };
      return;
    }
    if (this.#failures.length === MAX_RETAINED_HOST_FAILURES) {
      this.#failures.shift();
      this.#evictedFailureCount += 1;
    }
    this.#failures.push({ domain, stage, message, occurrences: 1 });
  }
}

/** A malformed frame was rejected before any optional presentation host ran. */
export class RendererPresentationFrameValidationError extends Error {
  constructor(override readonly cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause));
    this.name = 'RendererPresentationFrameValidationError';
  }
}

function hasListenerSynchronization(
  host: RendererAudioListenerPresentationHost,
): host is RendererAudioListenerPresentationHost & Required<Pick<
  RendererAudioListenerPresentationHost,
  'updateListener'
>> {
  return typeof host.updateListener === 'function';
}

const PRESENTATION_DOMAIN_ORDER: readonly RendererPresentationDomain[] = [
  'animation',
  'audio',
  'billboard',
  'particle',
  'telemetryOverlay',
  'ghostPlate',
];

const ADVANCING_DOMAIN_ORDER: readonly (
  'animation' | 'billboard' | 'particle'
)[] = ['animation', 'billboard', 'particle'];

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

function degradedReceipt(
  domain: RendererPresentationDomain,
  operations: readonly PresentationOp[],
): RendererPresentationDomainReceipt {
  const operation = operations[0];
  return {
    domain,
    configured: true,
    requested: operations.length,
    applied: 0,
    outcome: 'terminal',
    diagnostics: operation === undefined ? [] : [{
      domain,
      code: 'hostFailure',
      sequence: operation.meta.sequence,
      handle: operationHandle(operation),
      message: `${domain} presentation host is degraded after an earlier failure`,
    }],
  };
}

function listenerFailureReceipt(message: string): RendererPresentationListenerSyncReceipt {
  return {
    schemaVersion: 1,
    configured: true,
    applied: false,
    diagnostics: [{ code: 'hostFailure', sequence: 0, handle: null, message }],
  };
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

function frameReceipt(
  domains: readonly RendererPresentationDomainReceipt[],
): RendererPresentationFrameReceipt {
  return {
    schemaVersion: 1,
    applied: domains.reduce((total, domain) => total + domain.applied, 0),
    outcome: presentationFrameOutcome(domains),
    domains,
    diagnostics: domains.flatMap((domain) => domain.diagnostics),
  };
}

function presentationDomainOutcome(
  receipt: PresentationDomainReceipt,
): RendererPresentationDomainReceipt['outcome'] {
  if (receipt.diagnostics.some((diagnostic) => diagnostic.code === 'hostFailure')) return 'terminal';
  if (receipt.diagnostics.length === 0) return 'applied';
  return receipt.applied === 0 ? 'rejected_atomic' : 'partial';
}

function presentationFrameOutcome(
  domains: readonly RendererPresentationDomainReceipt[],
): RendererPresentationFrameReceipt['outcome'] {
  if (domains.some((domain) => domain.outcome === 'terminal')) return 'terminal';
  if (domains.some((domain) => domain.outcome === 'partial')) return 'partial';
  const hasRejection = domains.some((domain) => domain.outcome === 'rejected_atomic');
  if (!hasRejection) return 'applied';
  return domains.some((domain) => domain.outcome === 'applied' && domain.applied > 0)
    ? 'partial'
    : 'rejected_atomic';
}
