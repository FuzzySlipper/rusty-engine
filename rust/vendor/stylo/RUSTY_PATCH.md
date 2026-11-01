# stylo patch

Source: crates.io `stylo` 0.22.0, https://github.com/servo/stylo
License: MPL-2.0. The desktop pack ships this modified source in
`share/third-party/stylo`.

Engine #10524 changes `properties/longhands.toml` and
`properties/shorthands.toml`: `mask-image`, `mask-mode`, `mask-origin`,
`mask-clip`, `mask-size`, `mask-position-x`, `mask-position-y`,
`mask-repeat`, `mask-composite` and the `mask` and `mask-position`
shorthands, with their `-webkit-` aliases, no longer wait for the
`layout.unimplemented` preference. The Engine's servo-layout draws them
([its patch](../servo-layout/RUSTY_PATCH.md)); the preference would also
turn on every other property Servo does not draw.

Remove this vendored copy once a Servo release draws masks.
