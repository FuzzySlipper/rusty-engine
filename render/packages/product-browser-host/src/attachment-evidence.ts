import type {
  ProductBrowserAttachmentBaseline,
  ProductBrowserAttachmentEvidence,
} from './product-browser-host.js';

interface AttachmentStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Delivery correlation only. A confirmed renderer baseline never proves that
 * an uncertain product callback ran, or authorizes replaying it. */
export function createBrowserAttachmentEvidence(options: {
  readonly key: string;
  readonly storage?: AttachmentStorage;
  readonly reload: boolean;
  readonly newId: () => string;
}) {
  let id = options.newId();
  let replaces: string | undefined;
  let epoch: number | null = null;
  let candidate: ProductBrowserAttachmentBaseline | undefined;
  let baseline: ProductBrowserAttachmentBaseline | undefined;
  try {
    // A new navigation/duplicated tab cannot claim the prior page's recovery.
    const previous = options.reload ? options.storage?.getItem(options.key) : null;
    if (previous && /^[a-zA-Z0-9_-]{1,128}$/u.test(previous)) replaces = previous;
  } catch { /* Missing browser storage leaves reload correlation unavailable. */ }
  return {
    begin(nextEpoch: number): void {
      if (epoch === nextEpoch) return;
      if (epoch !== null) {
        if (baseline !== undefined) replaces = id;
        id = options.newId();
      }
      epoch = nextEpoch;
      baseline = undefined;
      candidate = undefined;
    },
    stage(nextEpoch: number, value: ProductBrowserAttachmentBaseline): void {
      if (epoch === nextEpoch) candidate = value;
    },
    confirm(nextEpoch: number): void {
      if (epoch !== nextEpoch || candidate === undefined) return;
      baseline = candidate;
      try { options.storage?.setItem(options.key, id); } catch { /* Report remains valid in this page. */ }
    },
    read(): ProductBrowserAttachmentEvidence {
      return Object.freeze({ id, ...(replaces === undefined ? {} : { replaces }),
        ...(baseline === undefined ? {} : { baseline }) });
    },
  };
}

export function browserAttachmentEvidence(basePath: string) {
  let storage: Storage | undefined;
  let reload = false;
  try {
    storage = globalThis.sessionStorage;
    reload = (globalThis.performance?.getEntriesByType('navigation')[0] as PerformanceNavigationTiming | undefined)?.type === 'reload';
  } catch { /* Browser restrictions must not prevent attachment. */ }
  return createBrowserAttachmentEvidence({
    key: `rusty.attachment:${basePath}`,
    ...(storage === undefined ? {} : { storage }),
    reload,
    // getRandomValues is available on trusted LAN HTTP origins too; unlike
    // randomUUID it does not require a secure context.
    newId: () => {
      const values = new Uint32Array(4);
      globalThis.crypto.getRandomValues(values);
      return `browser-${[...values].map((value) => value.toString(16).padStart(8, '0')).join('')}`;
    },
  });
}
