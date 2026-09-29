# Demand-lifecycle products mount in the runtime-pack shell (#8824)

## Cause

A demand-lifecycle product never mounted in the browser. There were two
failures in a row.

1. **Owner check.** `runtime-pack-shell/main.js` passes
   `realtimeAdvanceOwner: 'rust-host'` for every product.
   `mountProductBrowserHost` rejected that for any lifecycle other than
   realtime: `Product Browser Host rust-host realtime advance ownership requires
   realtime lifecycle mode`. The owner is read only when the product is realtime
   (`realtime-cadence.ts`), so the check rejected a harmless combination.
2. **Inspection time.** With the check gone, the stream failed with `invalid
   output: invalid inspection time`. `CsharpProductRuntime::readout` attached
   playtest inspection time to every readout, using a rate of 0 when the product
   has no fixed step (`["realtime",0]` on the wire). The browser rejects a rate
   of 0. It divides steps by that rate to get the inspection clock, which only
   exists for realtime products.

## Change

- **Removed** the owner/lifecycle check in both places it appeared:
  `validateOptions` and `productBrowserBundleAssets`. Also removed the bundle
  test that asserted the rejection. The option's doc now says only realtime
  products read it. `main.js` is unchanged.
- **Runtime:** the readout carries `inspectionTime` only for a realtime
  lifecycle. The field was already optional on the wire and in TS.
- **Evidence product:** `docs/evidence/source-root-reload-8743/product/ui/main.ts`
  now exports `mountProductUi`. Before, no browser could mount it: the shell
  requires that export.

## Evidence

The committed #8743 exercise product (`"lifecycle":{"mode":"demand"}`) ran under
`rusty dev --live-debug`, with a runtime pack and SDK built from this change.
`probe.mjs` loaded the page in Chromium and recorded each `EventSource` opened:

```
{"failure":null,"exercise":"ui v1","mountedText":true,"streams":["/__rusty/product/runtime/outputs/fresh"]}
```

The first fix alone gave `failure: "…invalid inspection time"`. Before either
fix, the failure was the owner-check error, as recorded in #8802.

- `product-browser-host`: 106 tests pass. The same-incarnation rebind test now
  mounts a demand product with `realtimeAdvanceOwner: 'rust-host'`, exactly what
  the shell passes; it fails without the fix.
- `csharp-product-runtime`: tests pass, including the new
  `only_a_realtime_readout_carries_inspection_time`. Clippy is clean.
- `pnpm run typecheck` passes, and `pnpm run test:browser` has 57 passed.
