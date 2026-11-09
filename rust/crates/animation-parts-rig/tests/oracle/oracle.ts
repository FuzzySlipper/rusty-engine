// Poses a plan through rig-c's own core (the installed @rig-c/core), for the
// Engine's parts-rig port to be compared against.
//
// bun oracle.ts <core-entry.ts> <plan.json>  → JSON on stdout
//
// plan: { model, skin: string|null, mode: "track", animation, loop, steps: [dt…], drive?: [[bone, channel, value]…],
//          before?: { "<step>": { wind?: [x, y], gravity?: [x, y], move?: [x, y] } } }
//    (`before` acts on the live track before step n, counted from 1: the physics environment, x right and y down,
//     and a physicsTranslate, y up)
//    or { model, skin, mode: "tracks", ops: [{play, loop, mix} | {stop} | {step}] }
//    (the first step of "tracks" is the start, at dt 0)
import { readFileSync } from 'node:fs';

const [entryPath, planPath] = process.argv.slice(2);
const core = await import(entryPath);
const plan = JSON.parse(readFileSync(planPath, 'utf8'));
const text = readFileSync(plan.model, 'utf8');
const read = core.readModel(text);
const doc = plan.skin === null ? core.underNoSkin(read) : core.underSkin(read, plan.skin);
const lookup = core.documentPageLookup(doc.pages);
const sequences = core.readUvSequences(JSON.parse(text));

function row(pose: any) {
  const active = new Map(pose.bones.map((b: any) => [b.name, b.active]));
  const slotRows = new Map(pose.slots.map((s: any) => [s[0], s]));
  const { drawn: regions } = core.drawnRegions(doc, pose.shown, pose.drawOrder, { lookup, sequences });
  const bySlot = new Map((regions ?? []).map((r: any) => [r.slot, r]));
  return {
    animationTime: pose.animationTime,
    bones: pose.bones.map((b: any) => [b.a, b.b, b.c, b.d, b.worldX, b.worldY]),
    drawOrder: pose.drawOrder,
    drawn: pose.drawn.map((d: any) => {
      const s: any = slotRows.get(d.slot);
      const slotBone = doc.slots.find((x: any) => x.name === d.slot).bone;
      const r: any = bySlot.get(d.slot);
      const uvs = r.art === null ? core.regionPageUvs(r.found.region, r.found.page) : core.meshPageUvs(r.found.region, r.found.page, r.art);
      return {
        slot: d.slot,
        attachment: d.attachment,
        active: active.get(slotBone),
        vertices: d.vertices,
        triangles: [...d.triangles],
        page: r.found.page.name,
        region: r.region,
        uvs,
        color: [s[2] * d.colour[0], s[3] * d.colour[1], s[4] * d.colour[2], s[5] * d.colour[3]],
        dark: s[6],
        blend: s[8],
      };
    }),
    events: pose.events.map((e: any) => [e[0], e[1], e[2], e[3], e[4]]),
  };
}

const poses: any[] = [];
if (plan.mode === 'track') {
  // A driven value is written over the posed locals each step, as the core's `adjust`.
  const drive = plan.drive ?? [];
  const adjust = drive.length === 0 ? undefined : (locals: any) => {
    for (const [bone, channel, value] of drive) locals.bone(bone)[channel] = value;
  };
  const track = core.openTrack(doc, plan.animation, { loop: plan.loop, reset: 'animation', adjust });
  const before = plan.before ?? {};
  poses.push(row(track.first));
  plan.steps.forEach((dt: number, i: number) => {
    const b = before[String(i + 1)];
    if (b !== undefined) {
      const env: any = {};
      if (b.wind !== undefined) env.wind = b.wind;
      if (b.gravity !== undefined) env.gravity = b.gravity;
      if (Object.keys(env).length > 0) track.setEnvironment(env);
      if (b.move !== undefined) track.physicsTranslate(b.move[0], b.move[1]);
    }
    poses.push(row(track.step(dt)));
  });
} else {
  const tracks = core.openTracks(doc, { reset: 'animation' });
  let started = false;
  for (const op of plan.ops) {
    if (op.play !== undefined) tracks.setAnimation(0, op.play, { loop: op.loop, mixDuration: op.mix });
    else if (op.stop !== undefined) tracks.setEmptyAnimation(0, op.stop);
    else if (!started) {
      started = true;
      poses.push(row(tracks.start()));
    } else poses.push(row(tracks.step(op.step)));
  }
}
process.stdout.write(JSON.stringify({ poses }));
