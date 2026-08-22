import type {
  RustyDeveloperCommandClient,
  RustyDeveloperCommandValueSchema,
  RustyDeveloperCommandWireSchema,
} from '@rusty-engine/developer-command-client';

export interface RustyDeveloperCommandShellOptions {
  readonly client: RustyDeveloperCommandClient;
  readonly label?: string;
  /** Application host supplies this small UI-arbitration seam; the shell owns no input policy. */
  readonly enterInterface?: () => () => void;
}
export interface RustyDeveloperCommandShell { readonly dispose: () => void; }

/** A small Engine-owned UI over an injected, transport-neutral command client. */
export function mountRustyDeveloperCommandShell(root: HTMLElement, options: RustyDeveloperCommandShellOptions): RustyDeveloperCommandShell {
  const document = root.ownerDocument;
  const host = document.createElement('section');
  host.dataset['rustyDeveloperCommandShell'] = 'v1';
  host.style.cssText = 'background:#111c;color:#eef;font:12px system-ui;left:0;max-width:min(720px,100%);padding:8px;position:absolute;top:0;z-index:20;';
  const toggle = document.createElement('button'); toggle.type = 'button'; toggle.textContent = options.label ?? 'Developer commands';
  const body = document.createElement('div'); body.hidden = true;
  const status = document.createElement('output'); status.dataset['developerCommandStatus'] = '';
  const select = document.createElement('select'); select.setAttribute('aria-label', 'Developer command');
  const fields = document.createElement('div'); fields.dataset['developerCommandFields'] = '';
  const payload = document.createElement('textarea'); payload.setAttribute('aria-label', 'Developer command parameters'); payload.rows = 5; payload.cols = 58;
  const run = document.createElement('button'); run.type = 'button'; run.textContent = 'Run';
  const cancel = document.createElement('button'); cancel.type = 'button'; cancel.textContent = 'Cancel'; cancel.disabled = true;
  const exportButton = document.createElement('button'); exportButton.type = 'button'; exportButton.textContent = 'Export sequence';
  const history = document.createElement('pre'); history.dataset['developerCommandHistory'] = '';
  body.append(status, document.createElement('br'), select, document.createElement('br'), fields, payload, document.createElement('br'), run, cancel, exportButton, history);
  host.append(toggle, body); root.append(host);
  let disposed = false;
  let restoreInteraction: (() => void) | null = null;
  let activeAbort: AbortController | null = null;
  const writeStatus = (message: string): void => { status.value = message; status.textContent = message; };
  const populate = async (): Promise<void> => {
    try {
      const snapshot = await options.client.discover();
      // Discovery belongs to the injected transport. A late response must not
      // resurrect a closed or disposed shell.
      if (disposed || !host.isConnected) return;
      select.replaceChildren();
      for (const command of snapshot.commands) {
        const option = document.createElement('option'); option.value = command.id;
        const codecAvailable = options.client.schema(command.id) !== null;
        option.disabled = !codecAvailable;
        option.textContent = `${command.id} (${command.lane}) — ${command.summary}${codecAvailable ? '' : ' (help only; no exact codec)'}`;
        select.append(option);
      }
      if (select.value !== '') { const schema = options.client.schema(select.value); payload.value = suggestedPayload(schema); payload.hidden = renderFields(fields, schema); }
      writeStatus(`protocol ${snapshot.protocolVersion}; runtime ${snapshot.runtime}; profile ${snapshot.profile}; lanes ${snapshot.permittedLanes.join(',')}; revision ${snapshot.revision}; epoch ${snapshot.catalogEpoch}; contract ${snapshot.contractFingerprint}`);
    } catch (cause) { if (!disposed && host.isConnected) writeStatus(`Unavailable: ${errorMessage(cause)}`); }
  };
  const onToggle = (): void => {
    if (disposed) return;
    body.hidden = !body.hidden;
    if (!body.hidden) { restoreInteraction ??= options.enterInterface?.() ?? null; void populate(); }
    else { restoreInteraction?.(); restoreInteraction = null; }
  };
  const onSelect = (): void => { if (disposed) return; const schema = options.client.schema(select.value); payload.value = suggestedPayload(schema); payload.hidden = renderFields(fields, schema); };
  const onRun = async (): Promise<void> => {
    if (disposed) return;
    if (options.client.schema(select.value) === null) { writeStatus('Unavailable: this command is help only; its product has not supplied an exact codec.'); return; }
    let parsed: unknown;
    try { parsed = fields.childElementCount > 0 ? fieldPayload(fields) : JSON.parse(payload.value); } catch { writeStatus('Malformed parameters: expected JSON'); return; }
    try {
      const controller = new AbortController(); activeAbort = controller; run.disabled = true; cancel.disabled = false;
      const response = await options.client.execute(select.value, parsed, controller.signal);
      if (disposed || controller.signal.aborted) return;
      writeStatus(response.outcome.kind === 'success' ? `Success (${response.correlation})` : `Error ${response.outcome.code}: ${response.outcome.message}`);
      history.textContent = JSON.stringify(options.client.history(), null, 2);
    } catch (cause) { if (!disposed) writeStatus(`Failed: ${errorMessage(cause)}`); }
    finally { activeAbort = null; run.disabled = false; cancel.disabled = true; }
  };
  const onExport = (): void => {
    if (disposed) return;
    history.textContent = JSON.stringify(options.client.exportSequence(), null, 2);
    writeStatus('Sequence exported below; it is not deterministic replay.');
  };
  const onCancel = (): void => { if (!disposed) activeAbort?.abort(); };
  const onRunClick = (): void => { void onRun(); };
  toggle.addEventListener('click', onToggle); select.addEventListener('change', onSelect); run.addEventListener('click', onRunClick); cancel.addEventListener('click', onCancel); exportButton.addEventListener('click', onExport);
  return Object.freeze({ dispose: () => { if (disposed) return; disposed = true; activeAbort?.abort(); restoreInteraction?.(); restoreInteraction = null; toggle.removeEventListener('click', onToggle); select.removeEventListener('change', onSelect); run.removeEventListener('click', onRunClick); cancel.removeEventListener('click', onCancel); exportButton.removeEventListener('click', onExport); host.remove(); } });
}
function suggestedPayload(schema: RustyDeveloperCommandWireSchema | null): string { return JSON.stringify(schema === null ? {} : sampleValue(schema.request), null, 2); }
function sampleValue(schema: RustyDeveloperCommandValueSchema): unknown { switch (schema.kind) { case 'boolean': return false; case 'decimalU64': return '0'; case 'integer': return schema.minimum ?? 0; case 'string': return schema.pattern === 'identifier' ? 'example' : ''; case 'array': return []; case 'object': return Object.fromEntries(Object.entries(schema.fields).filter(([, field]) => field.required).map(([key, field]) => [key, sampleValue(field.value)])); case 'enum': return schema.values[0] ?? ''; case 'taggedUnion': return {}; case 'opaqueJson': return {}; } }
function errorMessage(cause: unknown): string { return cause instanceof Error ? cause.message : String(cause); }
function renderFields(root: HTMLElement, schema: RustyDeveloperCommandWireSchema | null): boolean { root.replaceChildren(); if (schema?.request.kind !== 'object' || Object.values(schema.request.fields).some((field) => !['boolean', 'decimalU64', 'integer', 'string', 'enum'].includes(field.value.kind))) return false; for (const [name, field] of Object.entries(schema.request.fields)) { const label = root.ownerDocument.createElement('label'); label.textContent = `${name} `; const input = field.value.kind === 'enum' ? root.ownerDocument.createElement('select') : root.ownerDocument.createElement('input'); input.dataset['commandField'] = name; if (input instanceof HTMLInputElement) { input.type = field.value.kind === 'boolean' ? 'checkbox' : field.value.kind === 'integer' ? 'number' : 'text'; input.required = field.required; if (field.value.kind === 'decimalU64') { input.placeholder = '0'; if (field.required) input.value = '0'; } if (field.value.kind === 'integer') { if (field.value.minimum !== undefined) input.min = String(field.value.minimum); if (field.value.maximum !== undefined) input.max = String(field.value.maximum); } if (field.value.kind === 'string') input.maxLength = field.value.maximumBytes; } if (input instanceof HTMLSelectElement && field.value.kind === 'enum') for (const entry of field.value.values) input.add(new Option(entry, entry)); label.append(input); root.append(label); } return true; }
function fieldPayload(root: HTMLElement): unknown { const value: Record<string, unknown> = {}; for (const control of root.querySelectorAll<HTMLInputElement | HTMLSelectElement>('[data-command-field]')) { const key = control.dataset['commandField']; if (key === undefined || (!(control instanceof HTMLInputElement && control.type === 'checkbox') && control.value === '')) continue; value[key] = control instanceof HTMLInputElement && control.type === 'checkbox' ? control.checked : control instanceof HTMLInputElement && control.type === 'number' ? Number(control.value) : control.value; } return value; }
