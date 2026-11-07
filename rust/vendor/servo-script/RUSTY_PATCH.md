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

Engine #10397 changes `dom/element/focus.rs`: an element inside an inline
`<svg>` counts as rendered for focus when the `<svg>` that draws it has a
layout box and neither it nor an SVG ancestor has a `display="none"`
attribute. Its own computed `display` is `none` (nothing inside the picture
gets a box), so 0.7.0 refused it as a focusable area: a `<g tabindex="0">`
took focus neither from `focus()` nor from Tab.

Engine #10401 changes `dom/document/document_event_handler.rs`: a gamepad
input event queues its gamepad task when it arrives. 0.7.0 held it with the
other input events until the next "update the rendering", so a page's
`navigator.getGamepads()` could show a pad press after a network message the
press had already brought about (in the desktop window, the UI projection of
the panel the press opened), and the page then took the held button as a new
press.

Engine #10661 changes `dom/clipboard/clipboard.rs`: `navigator.clipboard.readText()`
rejects with `NotAllowedError`, as a browser does when its read prompt is
denied. Servo asks the embedder for the clipboard's text the same way for a
page's read and for the person's own paste, so with the read refused there
the desktop window can answer every request it gets with the system
clipboard, and `Ctrl+V` pastes into a focused field.

Remove this vendored copy once a Servo release carries these fixes (and the
read refusal can be told apart by the embedder).
