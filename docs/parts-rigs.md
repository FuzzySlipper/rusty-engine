# Parts rigs

`IEngineContext.PartsRigs` plays 2D parts rigs built by
[rig-c](https://github.com/rig-c/core) and draws them as their posed pieces
on one upright plane in the world, the way a sprite stands. The Engine reads
rig-c's own model document (`skeleton.model.json`, `rigc-compiled/3`) and
the pages it names, and poses each rig once per call. It reads no Spine JSON
or atlas and links no Spine runtime.

The product loads rigs, makes appearances that show them, publishes those as
objects, and chooses clips, skins and driving values per object. The Engine
evaluates the poses, keeps the pages as textures and draws the pieces in
every reading of the scene.

```csharp
PartsRig front = engine.PartsRigs.Load(new("people/townsperson/front/skeleton.model.json", TextureFilter.Linear)).Rig;
PartsRig side = engine.PartsRigs.Load(new("people/townsperson/side/skeleton.model.json", TextureFilter.Linear)).Rig;
PartsRig back = engine.PartsRigs.Load(new("people/townsperson/back/skeleton.model.json", TextureFilter.Linear)).Rig;
Appearance person = engine.PartsRigs.CreateAppearance(new PartsRigAppearanceRequest(new PartsRigView[]
{
    new(front, 0, false),
    new(side, -MathF.PI / 2, false),
    new(back, MathF.PI, false),
    new(side, MathF.PI / 2, true),
}, metresPerUnit: 1.75f / 896));
engine.Graphics.PublishSnapshot([new(PersonId, false, 0, transform, person, true, RenderLayer.Scene)]);
engine.PartsRigs.Play(new PartsRigPlayRequest(PersonId, "walk", crossfadeSeconds: .2f));
// The idle's breathing added over the walk on track 1.
engine.PartsRigs.Play(PartsRigPlayRequest.Layer(PersonId, 1, "idle", alpha: 1, additive: true));
engine.PartsRigs.DriveBone(PartsRigDriveRequest.Hold(PersonId, "turn", PartsRigBoneChannel.X, 18));
```

See `fixtures/csharp-parts-rigs` for a townsperson of four views walking in
place with its breathing added over the walk and an orbiting camera, and a
gallery rig whose clip fires events with a second clip added over it.

## Loading

`Load` reads a model document from product content (loose content or a
bundle) and each page it names from the file beside it. `Filter` is how the
pages are sampled: `Nearest` for pixel art. It returns the rig with its bone,
slot, clip, skin and page counts. A document of another `rigc-compiled`
version is refused by name. IK, transform, path, physics and slider
constraints and clipping attachments are evaluated as rig-c's core does. A
rig whose slider animation keys another constraint's timelines, or deforms a
path a path constraint walks, is refused by name: rig-c's core does not pose
those either.

Disposing a `PartsRig` frees its pages; it is refused while an appearance
shows the rig.

## Appearances and views

`CreateAppearance` makes an appearance from one or more `PartsRigView`s, each
a rig with the heading it is seen from and a mirror flag. The other fields
are a sprite's ([upright sprites](csharp-capabilities.md#upright-sprites)):
`Scale` in metres per rig unit (CSS pixels for a `Pixel`-sized rig),
billboard mode, render order, depth policy, linear tint, and a sprite
material for lighting, alpha, shadow policy, emission and
`AuthoredFacing`. `TexelSnap`, in rig units per texel, draws a pixel-art
figure on its texel grid. The convenience constructor makes a world-sized
`Cylindrical` rig, unlit and masked at half alpha.

A heading is the bearing, about the object's up axis, of a camera that sees
that view: radians from the object's +z toward its +x, so 0 is a camera in
front. Each reading of the scene (the window, the stream, a capture, a
snapshot, an offline render) draws the view whose heading is nearest the
bearing its own camera sees the object from: toward the eye, or against the
view's heading for an orthographic or `CylindricalView` camera. A mirrored
view is drawn flipped across the rig's vertical axis, so one side's rig shows
the other side.

One view shows from every side.

## Playing

Every call that plays names a published object. All of an object's views
play together: the same clips on the same tracks at the same clip times,
one speed and one set of driven values, so a stride carries across a change
of view. Their rigs need clips of the same names, cut to the same length.

An object plays its clips on layered tracks, as rig-c's live tracks do.
Track 0 is the base; each track above applies over what the tracks below
posed, lowest first. A clip on a track moves the pose toward its own by its
`Alpha` (0 to 1). An `Additive` clip instead adds its keyed values (times
its alpha) to what the tracks below posed: bone rotations, translations,
scales and shears, mesh deforms, transform constraint mixes, and physics
wind and gravity. Its colours, attachments and other values replace, as a
replacing clip's do. That is an effect layer: a flinch, a glow pulse or a
breathing layer over a walk. Each track crossfades on its own, and every
track's clip fires its events. Draw order and sequence frames follow track
0's clip.

- `Play`: a clip by name on a `Track` from its start, looping or once,
  fading in over `CrossfadeSeconds` from what that track played, at `Alpha`,
  `Additive` or replacing. A play on track 0 sets the object's `Speed` (1 as
  authored), which all its tracks share; a play on a higher track leaves it.
  Every view's rig must have the clip. `new PartsRigPlayRequest(id, clip)`
  loops a clip on track 0; `PartsRigPlayRequest.Layer(id, track, clip,
  alpha, additive)` layers one.
- `SetTrackAlpha`: change how strongly a track's clip applies while it
  plays; a clip it fades from keeps its own. Refused when the track plays
  nothing.
- `Stop`: fade a track out: track 0 back to the setup pose, a higher track
  to what the tracks below it pose.
- `SetSkin`: a skin by name in every view, or empty for the default skin
  alone.
- `DriveBone`: hold one of a bone's local values (rotation, x, y, scale or
  shear) over what the clips pose and before the constraints read it, in
  every view whose rig has the bone: a product's driving value such as
  Dagger's `turn` bone, which transform constraints read for the head turn,
  or a slider's dial, whose value picks the time its animation applies at
  (rig-c's value-driven face turn). `Release` lets the clips pose it again.

An object's motion in the world swings its physics (hair, cloth): what it
moved since the last call along its own x and y axes, in rig units, moves
every view's physics before its step, mirrored for a mirrored view. A
`Pixel`-sized rig takes none.

An object keeps its clip when it is published again, even when its renderer
node is made again (a new parent or layer). A new appearance over different
rigs starts from the setup pose.

## Reading back

The poses are evaluated as each call settles, at the Engine's presentation
time, so reads describe the pose of the previous call.

- `Read`: one track's clip as it stands (a clip played this call reads at
  clip time 0), its clip time, whether it loops, its alpha and whether it
  adds; and the object's views, speed, and whether it has been posed. `new
  PartsRigTrackRequest(id)` reads track 0.
- `ReadEvents`: the events rig objects' clips, on every track, passed since
  the previous update, object by object in time order: name (cut to 96
  bytes), key time, int and float values, volume and balance. A view set fires its first
  view's events. Events' string values are not carried.
- `ReadBone`: a named bone as one camera's reading draws it: the view that
  camera draws, the bone's position, rotation and scale in that view's rig
  units as authored, and its world position with the view's mirror and the
  plane turned to that camera by the billboard mode, the object where it
  was last posed. Publish an attachment there each update to keep it on the
  bone in that camera's view (the fixture keeps a marker on the head). The
  camera is taken as the call leaves it. A `Pixel`-sized rig is refused: its
  size follows each reading's viewport.

## Cost

Each view is posed every call that moves time, so a figure of four views
costs four poses. A rusty-dagger prototype townsperson rig (122 bones, 26
transform constraints) poses in about 8 µs on den-agents. On rig-c's
gallery rigs a path constraint on one bone adds about 0.5 µs a pose, and
`look`'s two sliders, each applying a clip of 16 bones and mesh deforms,
about 3 µs. An unchanged pose is not sent again, and each reading draws one
view per figure.
