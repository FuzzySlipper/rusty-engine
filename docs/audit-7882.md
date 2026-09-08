# Test and CI audit follow-through (#7881–7882)

The workflow/script survey found no obsolete verification lane to delete. Rust
invariants, generated C# bindings/lifecycle, renderer artifacts, and browser
behavior remain distinct useful checks. The assertion/call-path survey did find
retired policy still enforced in production:

| Path | Removed | Evidence retained |
| --- | --- | --- |
| Browser timeline completion | Serialize-only 4 KiB preflight and JSON shape quotas | Actual completion posting with larger/deeper detached data |
| Browser UI projection | Depth, array, key, and string quotas | Runtime-output delivery with larger/deeper immutable data |
| Rust input wire decoder | Duplicate 512 KiB host byte policy | Large direct decoding succeeds; ProductDev HTTP framing still rejects oversized bodies |

Timeline/UI JSON keeps finite-number semantics, including numbers outside the
safe-integer range. Direct-input payloads retain their existing safe-integer
contract. Both use one iterative immutable plain-JSON snapshot implementation;
Unicode scalar strings, cycle detection, and copied ownership remain relevant.

These tests do not promise arbitrary nesting through every codec. Rust's
serde_json recursion limit remains a parser stack constraint; it is not another
serialize-to-measure policy. HTTP framing, event-page limits, typed correlation,
renderer diagnostic response budgets, and device limits were retained.

#7881 was a fixture deadline error: 120 display callbacks elapsed before the
renderer pacing policy's next admission. Recorded failure: current callback
7.51 s, next admission 8.27 s, cadence ready, no renderer failures. The fixture
now uses the browser test's existing 60 s elapsed readiness deadline and still
requires actual render-sequence advancement. Terminal fixture failures end the
wait promptly. The full browser suite passed twice consecutively after this fix.
