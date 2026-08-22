import type { RustyDeveloperCommandClient } from './developer-command-client.js';
export interface RustyDeveloperCommandShellOptions {
    readonly client: RustyDeveloperCommandClient;
    readonly label?: string;
    /** Application host supplies this small UI-arbitration seam; the shell owns no input policy. */
    readonly enterInterface?: () => () => void;
}
export interface RustyDeveloperCommandShell {
    readonly dispose: () => void;
}
/** A small Engine-owned UI over an injected, transport-neutral command client. */
export declare function mountRustyDeveloperCommandShell(root: HTMLElement, options: RustyDeveloperCommandShellOptions): RustyDeveloperCommandShell;
