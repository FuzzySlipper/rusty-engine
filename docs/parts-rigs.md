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
engine.PartsRigs.DriveBone(PartsRigDriveRequest.Hold(PersonId, "turn", PartsRigBoneChannel.X, 18));
```

See `fixtures/csharp-parts-rigs` for a townsperson of four views walking in
place with an orbiting camera, and a gallery rig whose clip fires events.

## Loading

`Load` reads a model document from product content (loose content or a
bundle) and each page it names from the file beside it. `Filter` is how the
pages are sampled: `Nearest` for pixel art. It returns the rig with its bone,
slot, clip, skin and page counts. A document of another `rigc-compiled`
version is refused by name, and so is a rig declaring a path or slider
constraint, which the Engine does not evaluate yet. IK, transform and
physics constraints and clipping attachments are evaluated as rig-c's core
does.

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
play together: one clip, one clip time, one speed and one set of driven
values, so a stride carries across a change of view. Their rigs need clips
of the same names, cut to the same length.

- `Play`: a clip by name from its start, looping or once, fading in over
  `CrossfadeSeconds` from what played, at `Speed` (1 as authored). Every
  view's rig must have the clip.
- `Stop`: fade back to the setup pose.
- `SetSkin`: a skin by name in every view, or empty for the default skin
  alone.
- `DriveBone`: hold one of a bone's local values (rotation, x, y, scale or
  shear) over what the clips pose and before the constraints read it, in
  every view whose rig has the bone: a product's driving value such as
  Dagger's `turn` bone, which transform constraints read for the head turn.
  `Release` lets the clips pose it again.

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

- `Read`: the views, the playing clip and its time, the speed, and whether
  the object has been posed.
- `ReadEvents`: the events rig objects' clips passed since the previous
  update, object by object in time order: name (cut to 96 bytes), key time,
  int and float values, volume and balance. A view set fires its first
  view's events. Events' string values are not carried.
- `ReadBone`: a named bone of one view: its position, rotation and scale in
  rig units as authored, and its offset in the object's space on the rig's
  plane facing the object's +z, with the view's mirror. Attach something to
  a hand by that offset.

## Cost

Each view is posed every call that moves time, so a figure of four views
costs four poses. A rusty-dagger prototype townsperson rig (122 bones, 26
transform constraints) poses in about 8 µs on den-agents. An unchanged pose
is not sent again, and each reading draws one view per figure.
