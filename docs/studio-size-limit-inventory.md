# Studio Size-Limit Inventory

Rusty Engine Studio is a trusted local/LAN authoring product, not an
internet-distributed web application. A local deployment still needs finite
memory, parsing, projection, and filesystem-input boundaries. This inventory
separates those liveness limits from stale defaults and distribution budgets.

## Classification

- **Liveness:** retained because removing it can wedge the Node host, browser
  main thread, renderer, or filesystem projection. The owning source includes
  the local reason.
- **Calibrated control:** retained because the adapter operation is control
  data, with a regression at the declared boundary or a checked real corpus.
- **Distribution fossil:** removed because it constrained download-oriented
  packaging rather than local correctness or liveness.

## Inventory

| Surface | Limit | Class | Rationale and evidence |
| --- | ---: | --- | --- |
| Adapter request JSON | 256 KiB | Calibrated control | Requests contain operation metadata and bounded edits, never assets or retained frames. `HttpStudioAdapterTransport` rejects an oversized request before fetch; the editor-shell transport test pins that behavior. |
| Adapter response JSONL | 64 MiB | Liveness and calibrated control | The host retains one complete response and the browser parses one JSON document. The prior 32 MiB protocol-v1 default rejected the checked 54.5 MB `rusty-engine-voxels` high-fidelity project. The 64 MiB ceiling admits that corpus, remains finite, and has exchange-local overflow coverage. A complete oversized line is drained and rejected without killing the adapter. |
| Adapter stderr tail | 64 KiB | Liveness | Only the newest diagnostic tail is useful after failure. Bounding it prevents a noisy adapter from becoming unbounded host state. |
| Render-resource file | 64 MiB | Liveness | The trusted host reads one file into a `Buffer` and the browser reads one `ArrayBuffer`. Host and client use the same ceiling. This is a memory-allocation bound, not a network-transfer budget. |
| Render-resource and host-file paths | 4096 bytes | Liveness | Rejects malformed path input before normalization, symlink checks, and traversal. It is well above ordinary host path sizes. |
| Host-file directory projection | 512 entries | Liveness | The browser needs a predictable single directory readout. Results declare `truncated`; navigation remains available instead of recursively inventorying the host. |
| Host-file extension filters | 16 filters of at most 16 characters | Liveness | Prevents an unbounded request-controlled filter set while covering ordinary editor file selectors. |
| User-settings artifact | 64 KiB | Liveness | Settings are small human-editable JSON, not project content. The bound covers reads, writes, parsing, and preserved invalid text. HTTP writes allow at most twice this size so malformed/multibyte input can be diagnosed before canonical validation. |
| Startup root and project selectors | 4096 and 1024 characters | Liveness | Rejects malformed startup URLs before adapter dispatch. These values select host paths; they do not carry project content. |
| Settings project key and keyboard binding | 160 and 64 UTF-8 bytes | Liveness | Bounds persisted identifiers and individual input bindings. Validation measures bytes rather than JavaScript UTF-16 code units. |
| Retained voxel-object frame patches | 120 patches | Liveness | Long playback periodically returns to a complete owner frame instead of growing an unbounded browser patch chain. |
| Conversion preview nodes | 512 nodes | Liveness | Bounds disposable diagnostic render geometry. It does not truncate the owner conversion plan or result. |
| Angular initial/component-style budgets | Removed | Distribution fossil | The default warning/error budgets measured web download packaging. Studio is host-served on a trusted LAN; build correctness remains covered by compile, lint, tests, and browser gates. |

## Maintenance rule

New Studio limits must be added here and documented at the owning constant.
Limits must name the local allocation, parsing, projection, or filesystem
failure they prevent. Do not add bundle, transfer, CDN, compression, or
metered-network budgets to this local product.

The adapter response ceiling is not permission to keep expanding control-plane
readouts indefinitely. If a checked consumer approaches 64 MiB, move its bulk
render data behind an explicit resource or incremental protocol before raising
the ceiling again.
