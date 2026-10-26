# servo-script patch

Source: crates.io `servo-script` 0.7.0, https://github.com/servo/servo/tree/v0.7.0/components/script
License: MPL-2.0. The desktop pack ships this modified source in
`share/third-party/servo-script`.

Engine #10389 changes `event_loop/svg_font.rs`: the SVG font resolver caches
each font's face (its data and index) rather than its `fontdb::ID`, and finds
or loads that face in the database it is given.

Servo draws an inline `<svg>` by rasterizing its serialized markup, and every
parse starts from an empty font database. 0.7.0 cached the ID a font got in
the first parse, which in any later parse names no face or another one, so
text in a font that had drawn once was missing when the picture was redrawn
or another `<svg>` used it.

Remove this vendored copy once a Servo release carries a fix.
