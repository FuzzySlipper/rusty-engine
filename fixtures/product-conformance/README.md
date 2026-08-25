# Product Model conformance fixture

This is the smallest complete product vertical used to prove the Engine-owned
Product Model workflow. Product meaning stays in `kernel/entry.rs`: the
counter owns its mutation planner, source-linked observation facts, recurring
readout, and finite timeline request. Engine owns composition admission,
lifecycle, input, schedule/timeline ordering, standard capability execution,
mutation receipts, assembly, and the one application-host canvas.

From a clean Engine checkout, prepare the isolated owner workspaces and run:

```text
./scripts/verify-rules.sh
./scripts/verify-render.sh
./scripts/verify-product-conformance.sh
```

The conformance gate copies this fixture to a disposable directory. It proves
authoring/admission, deterministic delete/regenerate, exact content closure,
headless Assembly/package closure, and browser UI/physical input convergence.
It does not install a desktop product by default. Set
`RUSTY_PRODUCT_CONFORMANCE_DESKTOP=1` only in an environment prepared with
the existing Tauri/WebDriver prerequisites to add the selected packaged-host
proof.

The product deliberately has one semantic intent, `increment`. Physical `W`
and the DOM button both claim it through the host-owned input lane; neither
path receives direct mutation access.
