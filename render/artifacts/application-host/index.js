//#region packages/render-contracts/dist/render.js
function e(e, t) {
	if (!Number.isSafeInteger(e) || e < 0) throw RangeError(`${t} must be an unsigned JSON-safe integer`);
	return e;
}
var t = (t) => e(t, "render handle"), n = 1e4, r = 9007199254740991, i = class extends Error {
	constructor(e) {
		super(e), this.name = "ContractDecodeError";
	}
};
function a(e) {
	let t = M(e, "$", ["schemaVersion", "ops"]);
	return t.schemaVersion !== 1 && V("$.schemaVersion", "must equal 1"), P(t.ops, "$.ops").forEach((e, t) => s(e, `$.ops[${String(t)}]`)), e;
}
function o(e) {
	let t = M(e, "$", ["schemaVersion", "ops"]);
	return t.schemaVersion !== 1 && V("$.schemaVersion", "must equal 1"), P(t.ops, "$.ops").forEach((e, t) => {
		let n = `$.ops[${String(t)}]`, r = M(e, n, [
			"domain",
			"meta",
			"op"
		]), i = M(r.meta, `${n}.meta`, ["sequence"]);
		R(i.sequence, `${n}.meta.sequence`, 0, 4294967295), i.sequence !== t && V(`${n}.meta.sequence`, `must equal ordered index ${String(t)}`), re(B(r.domain, `${n}.domain`, [
			"audio",
			"billboard",
			"particle",
			"telemetryOverlay",
			"animation"
		]), r.op, `${n}.op`);
	}), e;
}
function s(e, t) {
	let n = Re(N(e, t).op, `${t}.op`);
	switch (n) {
		case "create": {
			let n = M(e, t, [
				"op",
				"handle",
				"parent",
				"node"
			]);
			Ae(n.handle, `${t}.handle`), ke(n.parent, `${t}.parent`), c(n.node, `${t}.node`);
			return;
		}
		case "update": {
			let n = M(e, t, [
				"op",
				"handle",
				"transform",
				"material",
				"visible",
				"metadata"
			]);
			Ae(n.handle, `${t}.handle`), L(n.transform, `${t}.transform`, d), L(n.material, `${t}.material`, u), L(n.visible, `${t}.visible`, ze), L(n.metadata, `${t}.metadata`, f);
			return;
		}
		case "destroy":
			Ae(M(e, t, ["op", "handle"]).handle, `${t}.handle`);
			return;
		case "replaceMeshPayload": {
			let n = M(e, t, [
				"op",
				"handle",
				"payload"
			]);
			Ae(n.handle, `${t}.handle`), p(n.payload, `${t}.payload`);
			return;
		}
		case "createLight": {
			let n = M(e, t, [
				"op",
				"handle",
				"parent",
				"light"
			]);
			Ae(n.handle, `${t}.handle`), ke(n.parent, `${t}.parent`), te(n.light, `${t}.light`);
			return;
		}
		case "updateLight": {
			let n = M(e, t, [
				"op",
				"handle",
				"light"
			]);
			Ae(n.handle, `${t}.handle`), te(n.light, `${t}.light`);
			return;
		}
		case "defineMaterial":
			w(M(e, t, ["op", "material"]).material, `${t}.material`);
			return;
		case "setMaterialInstanceParameters": {
			let n = M(e, t, [
				"op",
				"handle",
				"slot",
				"parameters"
			]);
			Ae(n.handle, `${t}.handle`), R(n.slot, `${t}.slot`, 0, 65535), L(n.parameters, `${t}.parameters`, O);
			return;
		}
		case "defineTexture":
			k(M(e, t, ["op", "texture"]).texture, `${t}.texture`);
			return;
		case "defineSpriteAtlas":
			A(M(e, t, ["op", "atlas"]).atlas, `${t}.atlas`);
			return;
		case "defineStaticMesh":
			_(M(e, t, ["op", "asset"]).asset, `${t}.asset`);
			return;
		case "defineAnimatedMesh":
			y(M(e, t, ["op", "asset"]).asset, `${t}.asset`);
			return;
		case "defineVoxelObject":
			x(M(e, t, ["op", "asset"]).asset, `${t}.asset`);
			return;
		case "releaseVoxelObject":
			z(M(e, t, ["op", "asset"]).asset, `${t}.asset`);
			return;
		case "createStaticMeshInstance": {
			let n = M(e, t, [
				"op",
				"handle",
				"parent",
				"instance"
			]);
			Ae(n.handle, `${t}.handle`), ke(n.parent, `${t}.parent`), v(n.instance, `${t}.instance`);
			return;
		}
		case "createAnimatedMeshInstance": {
			let n = M(e, t, [
				"op",
				"handle",
				"parent",
				"instance"
			]);
			Ae(n.handle, `${t}.handle`), ke(n.parent, `${t}.parent`), b(n.instance, `${t}.instance`);
			return;
		}
		case "setAnimatedMeshPlayback": {
			let n = M(e, t, [
				"op",
				"handle",
				"playback"
			]);
			Ae(n.handle, `${t}.handle`), C(n.playback, `${t}.playback`);
			return;
		}
		case "createVoxelObjectInstance": {
			let n = M(e, t, [
				"op",
				"handle",
				"parent",
				"instance"
			]);
			Ae(n.handle, `${t}.handle`), ke(n.parent, `${t}.parent`), S(n.instance, `${t}.instance`);
			return;
		}
		case "setVoxelObjectFrame": {
			let n = M(e, t, [
				"op",
				"handle",
				"frame"
			]);
			Ae(n.handle, `${t}.handle`), R(n.frame, `${t}.frame`, 0, 4294967295);
			return;
		}
		case "createSprite": {
			let n = M(e, t, [
				"op",
				"handle",
				"parent",
				"sprite"
			]);
			Ae(n.handle, `${t}.handle`), ke(n.parent, `${t}.parent`), ee(n.sprite, `${t}.sprite`);
			return;
		}
		case "updateSprite": {
			let n = M(e, t, [
				"op",
				"handle",
				"frame",
				"tint",
				"renderOrder",
				"visible"
			]);
			Ae(n.handle, `${t}.handle`), L(n.frame, `${t}.frame`, Me), L(n.tint, `${t}.tint`, Ee), L(n.renderOrder, `${t}.renderOrder`, Ne), L(n.visible, `${t}.visible`, ze);
			return;
		}
		default: V(`${t}.op`, `unsupported operation ${JSON.stringify(n)}`);
	}
}
function c(e, t) {
	let n = M(e, t, [
		"geometry",
		"material",
		"transform",
		"visible",
		"layer",
		"metadata"
	]);
	l(n.geometry, `${t}.geometry`), u(n.material, `${t}.material`), d(n.transform, `${t}.transform`), ze(n.visible, `${t}.visible`), B(n.layer, `${t}.layer`, [
		"scene",
		"debug",
		"ui",
		"viewmodel"
	]), f(n.metadata, `${t}.metadata`);
}
function l(e, t) {
	if (B(N(e, t).kind, `${t}.kind`, [
		"group",
		"cube",
		"sphere",
		"quad",
		"point",
		"line"
	]) === "line") {
		let n = M(e, t, [
			"kind",
			"a",
			"b"
		]);
		F(n.a, `${t}.a`), F(n.b, `${t}.b`);
	} else M(e, t, ["kind"]);
}
function u(e, t) {
	let n = M(e, t, ["color", "wireframe"]);
	Ee(n.color, `${t}.color`), ze(n.wireframe, `${t}.wireframe`);
}
function d(e, t) {
	let n = M(e, t, [
		"translation",
		"rotation",
		"scale"
	]);
	F(n.translation, `${t}.translation`);
	let r = Te(n.rotation, `${t}.rotation`, 4);
	r.forEach((e, n) => Pe(e, `${t}.rotation[${String(n)}]`)), r.every((e) => e === 0) && V(`${t}.rotation`, "must be non-zero"), F(n.scale, `${t}.scale`);
}
function f(e, t) {
	let n = M(e, t, [
		"sourceEntity",
		"sourceSceneNode",
		"tags",
		"label"
	]);
	L(n.sourceEntity, `${t}.sourceEntity`, je), L(n.sourceSceneNode, `${t}.sourceSceneNode`, je);
	let r = P(n.tags, `${t}.tags`), i;
	r.forEach((e, n) => {
		let r = z(e, `${t}.tags[${String(n)}]`);
		i !== void 0 && i >= r && V(`${t}.tags`, "must be strictly sorted and unique"), i = r;
	}), L(n.label, `${t}.label`, z);
}
function p(e, t) {
	let n = M(e, t, [
		"layout",
		"groups",
		"bounds",
		"source",
		"provenance"
	]), r = M(n.layout, `${t}.layout`, [
		"vertexCount",
		"indexCount",
		"indexWidth",
		"attributes"
	]), i = R(r.vertexCount, `${t}.layout.vertexCount`, 0, 4294967295), a = R(r.indexCount, `${t}.layout.indexCount`, 0, 4294967295);
	B(r.indexWidth, `${t}.layout.indexWidth`, ["u32"]);
	let o = P(r.attributes, `${t}.layout.attributes`), s = /* @__PURE__ */ new Set();
	o.forEach((e, n) => {
		let r = `${t}.layout.attributes[${String(n)}]`, i = M(e, r, [
			"name",
			"components",
			"kind"
		]), a = B(i.name, `${r}.name`, [
			"position",
			"normal",
			"uv",
			"color"
		]);
		s.has(a) && V(`${r}.name`, "is duplicated"), s.add(a);
		let o = a === "uv" ? 2 : a === "color" ? 4 : 3;
		i.components !== o && V(`${r}.components`, `must equal ${String(o)}`), B(i.kind, `${r}.kind`, ["f32"]);
	}), (!s.has("position") || !s.has("normal")) && V(`${t}.layout.attributes`, "must declare position and normal"), m(n.bounds, `${t}.bounds`), B(n.provenance, `${t}.provenance`, [
		"voxelChunk",
		"voxelObject",
		"staticAsset",
		"generated",
		"debug"
	]);
	let c = B(N(n.source, `${t}.source`).kind, `${t}.source.kind`, [
		"inline",
		"sharedBuffer",
		"resource"
	]);
	if (c === "inline") {
		let e = we(n.source, `${t}.source`, [
			"kind",
			"positions",
			"normals",
			"indices"
		], ["uvs"]);
		if (Oe(e.positions, `${t}.source.positions`, i * 3, !1), Oe(e.normals, `${t}.source.normals`, i * 3, !1), s.has("uv") !== Object.hasOwn(e, "uvs") && V(`${t}.source.uvs`, "must be present exactly when the uv attribute is declared"), Object.hasOwn(e, "uvs")) {
			let r = Oe(e.uvs, `${t}.source.uvs`, i * 2, !1);
			(n.provenance === "voxelChunk" || n.provenance === "voxelObject") && r.some((e) => Math.abs(e) > 16777216) && V(`${t}.source.uvs`, "voxel tile coordinate exceeds the exact f32 integer range");
		}
		Oe(e.indices, `${t}.source.indices`, a, !0).forEach((e, n) => {
			e >= i && V(`${t}.source.indices[${String(n)}]`, "is outside vertex range");
		});
	} else if (c === "sharedBuffer") {
		let e = we(n.source, `${t}.source`, [
			"kind",
			"buffer",
			"positionsByteOffset",
			"normalsByteOffset",
			"indicesByteOffset"
		], ["uvsByteOffset"]);
		je(e.buffer, `${t}.source.buffer`), Me(e.positionsByteOffset, `${t}.source.positionsByteOffset`), Me(e.normalsByteOffset, `${t}.source.normalsByteOffset`), s.has("uv") !== Object.hasOwn(e, "uvsByteOffset") && V(`${t}.source.uvsByteOffset`, "must be present exactly when the uv attribute is declared"), Object.hasOwn(e, "uvsByteOffset") && Me(e.uvsByteOffset, `${t}.source.uvsByteOffset`), Me(e.indicesByteOffset, `${t}.source.indicesByteOffset`);
	} else {
		let e = we(n.source, `${t}.source`, [
			"kind",
			"resource",
			"contentHash",
			"byteLength",
			"encoding",
			"positionsByteOffset",
			"normalsByteOffset",
			"indicesByteOffset"
		], ["uvsByteOffset"]), r = z(e.resource, `${t}.source.resource`), o = z(e.contentHash, `${t}.source.contentHash`), c = /^sha256:([0-9a-f]{64})$/u.exec(o)?.[1];
		c === void 0 && V(`${t}.source.contentHash`, "must be a lowercase SHA-256 identity"), r !== `mesh-resource/${c}` && V(`${t}.source.resource`, "must be the content-addressed mesh-resource identity");
		let l = R(e.byteLength, `${t}.source.byteLength`, 16, 64 * 1024 * 1024), u = B(e.encoding, `${t}.source.encoding`, ["packedStreamsLeV1", "packedStreamsLeV2"]);
		(s.has("uv") !== Object.hasOwn(e, "uvsByteOffset") || u === "packedStreamsLeV1" && Object.hasOwn(e, "uvsByteOffset") || u === "packedStreamsLeV2" && !Object.hasOwn(e, "uvsByteOffset")) && V(`${t}.source`, "mesh resource encoding and uv stream must agree");
		let d = R(e.positionsByteOffset, `${t}.source.positionsByteOffset`, 16, 4294967295), f = R(e.normalsByteOffset, `${t}.source.normalsByteOffset`, 16, 4294967295), p = Object.hasOwn(e, "uvsByteOffset") ? R(e.uvsByteOffset, `${t}.source.uvsByteOffset`, 16, 4294967295) : void 0, m = R(e.indicesByteOffset, `${t}.source.indicesByteOffset`, 16, 4294967295);
		for (let [e, n] of [
			["positionsByteOffset", d],
			["normalsByteOffset", f],
			...p === void 0 ? [] : [["uvsByteOffset", p]],
			["indicesByteOffset", m]
		]) n % 4 != 0 && V(`${t}.source.${e}`, "must be four-byte aligned");
		let h = d + i * 3 * 4, g = f + i * 3 * 4, _ = p === void 0 ? g : p + i * 2 * 4, v = m + a * 4;
		(h > l || g > l || _ > l || v > l) && V(`${t}.source`, "declares a mesh stream outside the resource byte length"), (h > f || (p === void 0 ? g : _) > m || p !== void 0 && g > p) && V(`${t}.source`, "mesh resource streams must not overlap");
	}
	let l = P(n.groups, `${t}.groups`), u = 0;
	l.forEach((e, n) => {
		let r = `${t}.groups[${String(n)}]`, i = M(e, r, [
			"materialSlot",
			"start",
			"count"
		]);
		R(i.materialSlot, `${r}.materialSlot`, 0, 65535);
		let o = Me(i.start, `${r}.start`), s = Me(i.count, `${r}.count`);
		o !== u && V(`${r}.start`, `must tile from ${String(u)}`), u += s, u > a && V(r, "extends beyond index count");
	}), u !== a && V(`${t}.groups`, "must cover the complete index buffer");
}
function m(e, t) {
	let n = M(e, t, ["min", "max"]), r = F(n.min, `${t}.min`), i = F(n.max, `${t}.max`);
	r.forEach((e, n) => {
		e > i[n] && V(t, "minimum exceeds maximum");
	});
}
function h(e, t) {
	let n = M(e, t, ["slot", "material"]), r = R(n.slot, `${t}.slot`, 0, 65535);
	return z(n.material, `${t}.material`), r;
}
function g(e, t) {
	let n = /* @__PURE__ */ new Set();
	return P(e, t).forEach((e, r) => {
		let i = h(e, `${t}[${String(r)}]`);
		n.has(i) && V(`${t}[${String(r)}].slot`, "is duplicated"), n.add(i);
	}), n;
}
function _(e, t) {
	let n = M(e, t, [
		"asset",
		"payload",
		"materialSlots",
		"collision"
	]);
	z(n.asset, `${t}.asset`), p(n.payload, `${t}.payload`);
	let r = g(n.materialSlots, `${t}.materialSlots`);
	P(N(n.payload, `${t}.payload`).groups, `${t}.payload.groups`).forEach((e, n) => {
		let i = N(e, `${t}.payload.groups[${String(n)}]`);
		r.has(i.materialSlot) || V(`${t}.payload.groups[${String(n)}].materialSlot`, "is not bound");
	}), B(N(n.collision, `${t}.collision`).kind, `${t}.collision.kind`, [
		"visualOnly",
		"proxy",
		"aabbFallback",
		"trimesh"
	]) === "proxy" ? z(M(n.collision, `${t}.collision`, ["kind", "proxyAsset"]).proxyAsset, `${t}.collision.proxyAsset`) : M(n.collision, `${t}.collision`, ["kind"]);
}
function v(e, t) {
	let n = M(e, t, [
		"asset",
		"transform",
		"visible",
		"materialOverrides",
		"metadata"
	]);
	z(n.asset, `${t}.asset`), d(n.transform, `${t}.transform`), ze(n.visible, `${t}.visible`), g(n.materialOverrides, `${t}.materialOverrides`), f(n.metadata, `${t}.metadata`);
}
function y(e, t) {
	let n = M(e, t, [
		"asset",
		"runtimeFormat",
		"contentHash",
		"clips",
		"defaultClip",
		"materialSlots",
		"bounds"
	]);
	z(n.asset, `${t}.asset`), B(n.runtimeFormat, `${t}.runtimeFormat`, ["glb"]), L(n.contentHash, `${t}.contentHash`, z);
	let r = /* @__PURE__ */ new Set();
	if (P(n.clips, `${t}.clips`).forEach((e, n) => {
		let i = `${t}.clips[${String(n)}]`, a = M(e, i, [
			"id",
			"name",
			"durationSeconds"
		]), o = z(a.id, `${i}.id`);
		r.has(o) && V(`${i}.id`, "is duplicated"), r.add(o), L(a.name, `${i}.name`, z), L(a.durationSeconds, `${i}.durationSeconds`, Fe);
	}), n.defaultClip !== null) {
		let e = z(n.defaultClip, `${t}.defaultClip`);
		r.has(e) || V(`${t}.defaultClip`, "is not declared");
	}
	g(n.materialSlots, `${t}.materialSlots`), m(n.bounds, `${t}.bounds`);
}
function b(e, t) {
	let n = M(e, t, [
		"asset",
		"transform",
		"visible",
		"materialOverrides",
		"playback",
		"metadata"
	]);
	z(n.asset, `${t}.asset`), d(n.transform, `${t}.transform`), ze(n.visible, `${t}.visible`), g(n.materialOverrides, `${t}.materialOverrides`), L(n.playback, `${t}.playback`, C), f(n.metadata, `${t}.metadata`);
}
function x(e, t) {
	let n = M(e, t, [
		"asset",
		"contentHash",
		"meshes",
		"frames",
		"materialSlots"
	]);
	z(n.asset, `${t}.asset`), z(n.contentHash, `${t}.contentHash`);
	let r = g(n.materialSlots, `${t}.materialSlots`), i = P(n.meshes, `${t}.meshes`);
	(i.length === 0 || i.length > 8193) && V(`${t}.meshes`, "must contain 1..=8193 entries");
	let a = 0, o = 0;
	i.forEach((e, n) => {
		let i = `${t}.meshes[${String(n)}]`, s = M(e, i, ["payload"]);
		p(s.payload, `${i}.payload`);
		let c = N(s.payload, `${i}.payload`), l = N(c.layout, `${i}.payload.layout`);
		a += l.vertexCount, o += l.indexCount, P(c.groups, `${i}.payload.groups`).forEach((e, t) => {
			let n = N(e, `${i}.payload.groups[${String(t)}]`);
			r.has(n.materialSlot) || V(`${i}.payload.groups[${String(t)}].materialSlot`, "is not bound");
		});
	}), (a > 8e6 || o > 12e6) && V(`${t}.meshes`, "exceeds aggregate vertex/index work limits");
	let s = P(n.frames, `${t}.frames`);
	(s.length === 0 || s.length > 8193) && V(`${t}.frames`, "must contain 1..=8193 entries");
	let c = /* @__PURE__ */ new Set();
	s.forEach((e, n) => {
		let r = `${t}.frames[${String(n)}]`, a = M(e, r, ["id", "mesh"]), o = z(a.id, `${r}.id`);
		c.has(o) && V(`${r}.id`, "is duplicated"), c.add(o), R(a.mesh, `${r}.mesh`, 0, i.length - 1);
	});
}
function S(e, t) {
	let n = M(e, t, [
		"asset",
		"frame",
		"transform",
		"visible",
		"materialOverrides",
		"metadata"
	]);
	z(n.asset, `${t}.asset`), R(n.frame, `${t}.frame`, 0, 4294967295), d(n.transform, `${t}.transform`), ze(n.visible, `${t}.visible`), g(n.materialOverrides, `${t}.materialOverrides`), f(n.metadata, `${t}.metadata`);
}
function C(e, t) {
	let n = B(N(e, t).kind, `${t}.kind`, [
		"play",
		"stop",
		"pause",
		"resume"
	]);
	if (n === "play") {
		let n = M(e, t, [
			"kind",
			"clip",
			"loop",
			"speed",
			"weight",
			"restart",
			"fadeSeconds"
		]);
		z(n.clip, `${t}.clip`), B(n.loop, `${t}.loop`, [
			"once",
			"repeat",
			"pingPong"
		]), Fe(n.speed, `${t}.speed`), Le(n.weight, `${t}.weight`, 0, 1), ze(n.restart, `${t}.restart`), L(n.fadeSeconds, `${t}.fadeSeconds`, Ie);
	} else n === "stop" ? L(M(e, t, ["kind", "fadeSeconds"]).fadeSeconds, `${t}.fadeSeconds`, Ie) : M(e, t, ["kind"]);
}
function w(e, t) {
	let n = we(e, t, [
		"schemaVersion",
		"id",
		"color",
		"texture",
		"roughness",
		"textureTint",
		"emissionColor",
		"emissionIntensity",
		"uvStrategy"
	], ["voxelSurface"]);
	R(n.schemaVersion, `${t}.schemaVersion`, 1, 4294967295), z(n.id, `${t}.id`), Ee(n.color, `${t}.color`), L(n.texture, `${t}.texture`, z), Le(n.roughness, `${t}.roughness`, 0, 1), Ee(n.textureTint, `${t}.textureTint`), I(n.emissionColor, `${t}.emissionColor`), Ie(n.emissionIntensity, `${t}.emissionIntensity`), B(n.uvStrategy, `${t}.uvStrategy`, [
		"flat",
		"planar",
		"atlas"
	]), Object.hasOwn(n, "voxelSurface") && T(n.voxelSurface, `${t}.voxelSurface`, n.texture);
}
function T(e, t, n) {
	let r = M(e, t, [
		"schemaVersion",
		"filter",
		"wrap",
		"alphaMode",
		"mapping"
	]);
	R(r.schemaVersion, `${t}.schemaVersion`, 1, 1);
	let i = B(r.filter, `${t}.filter`, ["nearest", "linear"]), a = B(r.wrap, `${t}.wrap`, ["clamp", "repeat"]);
	B(N(r.alphaMode, `${t}.alphaMode`).kind, `${t}.alphaMode.kind`, [
		"opaque",
		"mask",
		"blend"
	]) === "mask" ? Le(M(r.alphaMode, `${t}.alphaMode`, ["kind", "cutoff"]).cutoff, `${t}.alphaMode.cutoff`, 0, 1) : M(r.alphaMode, `${t}.alphaMode`, ["kind"]);
	let o = B(N(r.mapping, `${t}.mapping`).kind, `${t}.mapping.kind`, ["repeat", "atlas"]), s = o === "repeat" ? M(r.mapping, `${t}.mapping`, [
		"kind",
		"texture",
		"textureVersion",
		"textureContentHash",
		"tileScaleCells",
		"tileOriginCells"
	]) : M(r.mapping, `${t}.mapping`, [
		"kind",
		"atlas",
		"atlasVersion",
		"atlasContentHash",
		"texture",
		"textureVersion",
		"textureContentHash",
		"region",
		"tileScaleCells",
		"tileOriginCells"
	]);
	if (n !== z(s.texture, `${t}.mapping.texture`) && V(`${t}.mapping.texture`, "must match material texture"), R(s.textureVersion, `${t}.mapping.textureVersion`, 1, 4294967295), z(s.textureContentHash, `${t}.mapping.textureContentHash`), E(s.tileScaleCells, `${t}.mapping.tileScaleCells`, 1 / 256, 4096), E(s.tileOriginCells, `${t}.mapping.tileOriginCells`, -16777216, 16777216), o === "repeat") {
		a !== "repeat" && V(`${t}.wrap`, "repeat mapping requires repeat wrap");
		return;
	}
	a !== "clamp" && V(`${t}.wrap`, "atlas mapping requires clamp wrap"), z(s.atlas, `${t}.mapping.atlas`), R(s.atlasVersion, `${t}.mapping.atlasVersion`, 1, 4294967295), z(s.atlasContentHash, `${t}.mapping.atlasContentHash`);
	let c = M(s.region, `${t}.mapping.region`, [
		"id",
		"contentMin",
		"contentExtent",
		"padding",
		"inset"
	]);
	z(c.id, `${t}.mapping.region.id`), D(c.contentMin, `${t}.mapping.region.contentMin`, 0, 4294967295), D(c.contentExtent, `${t}.mapping.region.contentExtent`, 1, 4294967295), B(c.inset, `${t}.mapping.region.inset`, ["halfTexel"]);
	let l = M(c.padding, `${t}.mapping.region.padding`, [
		"left",
		"right",
		"bottom",
		"top"
	]);
	for (let e of [
		"left",
		"right",
		"bottom",
		"top"
	]) {
		let n = +(i === "linear");
		R(l[e], `${t}.mapping.region.padding.${e}`, n, 32);
	}
}
function E(e, t, n, r) {
	let i = Te(e, t, 2);
	Le(i[0], `${t}[0]`, n, r), Le(i[1], `${t}[1]`, n, r);
}
function D(e, t, n, r) {
	let i = Te(e, t, 2);
	R(i[0], `${t}[0]`, n, r), R(i[1], `${t}[1]`, n, r);
}
function O(e, t) {
	let n = M(e, t, [
		"textureTint",
		"emissionColor",
		"emissionIntensity"
	]);
	Ee(n.textureTint, `${t}.textureTint`), I(n.emissionColor, `${t}.emissionColor`), Ie(n.emissionIntensity, `${t}.emissionIntensity`);
}
function k(e, t) {
	let n = we(e, t, [
		"id",
		"width",
		"height",
		"filter",
		"wrap",
		"contentHash",
		"version"
	], ["payload"]);
	if (z(n.id, `${t}.id`), R(n.width, `${t}.width`, 1, 4096) * R(n.height, `${t}.height`, 1, 4096) > 16777216 && V(t, "texture texel quota exceeded"), B(n.filter, `${t}.filter`, ["nearest", "linear"]), B(n.wrap, `${t}.wrap`, ["clamp", "repeat"]), L(n.contentHash, `${t}.contentHash`, z), R(n.version, `${t}.version`, 1, 4294967295), Object.hasOwn(n, "payload")) {
		let e = M(n.payload, `${t}.payload`, [
			"encoding",
			"colorSpace",
			"contentHash",
			"byteLength",
			"source"
		]);
		B(e.encoding, `${t}.payload.encoding`, ["pngRgba8"]), B(e.colorSpace, `${t}.payload.colorSpace`, ["srgb"]);
		let r = z(e.contentHash, `${t}.payload.contentHash`), i = /^sha256:([0-9a-f]{64})$/u.exec(r)?.[1];
		(i === void 0 || n.contentHash !== r) && V(`${t}.payload.contentHash`, "must be the canonical texture content hash");
		let a = R(e.byteLength, `${t}.payload.byteLength`, 1, 16 * 1024 * 1024), o = N(e.source, `${t}.payload.source`);
		B(o.kind, `${t}.payload.source.kind`, ["inline", "resource"]) === "inline" ? Oe(M(o, `${t}.payload.source`, ["kind", "encodedBytes"]).encodedBytes, `${t}.payload.source.encodedBytes`, a, !0).forEach((e, n) => {
			e > 255 && V(`${t}.payload.source.encodedBytes[${String(n)}]`, "must be a byte");
		}) : M(o, `${t}.payload.source`, ["kind", "resource"]).resource !== `texture-resource/${i}` && V(`${t}.payload.source.resource`, "must match the content hash");
	}
}
function A(e, t) {
	let n = M(e, t, [
		"id",
		"texture",
		"frames"
	]);
	z(n.id, `${t}.id`), z(n.texture, `${t}.texture`);
	let r = P(n.frames, `${t}.frames`);
	r.length === 0 && V(`${t}.frames`, "must not be empty");
	let i = /* @__PURE__ */ new Set();
	r.forEach((e, n) => {
		let r = `${t}.frames[${String(n)}]`, a = we(e, r, [
			"frame",
			"uvMin",
			"uvMax"
		], ["size"]), o = Me(a.frame, `${r}.frame`);
		i.has(o) && V(`${r}.frame`, "is duplicated"), i.add(o);
		let s = De(a.uvMin, `${r}.uvMin`, 2, 0, 1), c = De(a.uvMax, `${r}.uvMax`, 2, 0, 1);
		(c[0] <= s[0] || c[1] <= s[1]) && V(r, "UV rectangle is degenerate"), a.size !== void 0 && Te(a.size, `${r}.size`, 2).forEach((e, t) => Fe(e, `${r}.size[${String(t)}]`));
	});
}
function ee(e, t) {
	let n = M(e, t, [
		"asset",
		"frame",
		"pivot",
		"size",
		"sizeMode",
		"billboard",
		"tint",
		"renderOrder",
		"depth",
		"shading",
		"visible",
		"transform",
		"attachment",
		"metadata"
	]);
	z(n.asset, `${t}.asset`), Me(n.frame, `${t}.frame`), De(n.pivot, `${t}.pivot`, 2, 0, 1), Te(n.size, `${t}.size`, 2).forEach((e, n) => Fe(e, `${t}.size[${String(n)}]`)), B(n.sizeMode, `${t}.sizeMode`, ["world", "pixel"]), B(n.billboard, `${t}.billboard`, [
		"none",
		"spherical",
		"cylindrical"
	]), Ee(n.tint, `${t}.tint`), Ne(n.renderOrder, `${t}.renderOrder`), B(n.depth, `${t}.depth`, [
		"default",
		"depthTestOff",
		"depthWriteOff"
	]), B(n.shading, `${t}.shading`, [
		"unlit",
		"lit",
		"shadowed",
		"custom"
	]), ze(n.visible, `${t}.visible`), d(n.transform, `${t}.transform`);
	let r = M(n.attachment, `${t}.attachment`, [
		"sourceEntity",
		"sourceSceneNode",
		"attachmentPoint"
	]);
	L(r.sourceEntity, `${t}.attachment.sourceEntity`, je), L(r.sourceSceneNode, `${t}.attachment.sourceSceneNode`, je), L(r.attachmentPoint, `${t}.attachment.attachmentPoint`, z), f(n.metadata, `${t}.metadata`);
}
function te(e, t) {
	let r = B(N(e, t).kind, `${t}.kind`, [
		"ambient",
		"directional",
		"point",
		"spot"
	]), i = [
		"kind",
		"color",
		"intensity",
		"enabled",
		"shadowIntent"
	], a = M(e, t, r === "ambient" ? i : r === "directional" ? [...i, "direction"] : r === "point" ? [
		...i,
		"position",
		"range",
		"decay"
	] : [
		...i,
		"position",
		"direction",
		"range",
		"decay",
		"outerAngleRadians",
		"penumbra"
	]);
	I(a.color, `${t}.color`), Le(a.intensity, `${t}.intensity`, 0, n), ze(a.enabled, `${t}.enabled`), B(a.shadowIntent, `${t}.shadowIntent`, ["disabled", "requested"]), (r === "directional" || r === "spot") && ne(a.direction, `${t}.direction`), (r === "point" || r === "spot") && (F(a.position, `${t}.position`), L(a.range, `${t}.range`, Fe), Ie(a.decay, `${t}.decay`)), r === "spot" && (Le(a.outerAngleRadians, `${t}.outerAngleRadians`, Number.MIN_VALUE, Math.PI / 2), Le(a.penumbra, `${t}.penumbra`, 0, 1));
}
function ne(e, t) {
	F(e, t).every((e) => e === 0) && V(t, "must be non-zero");
}
function re(e, t, n) {
	let r = Re(N(t, n).op, `${n}.op`);
	if (e === "audio") return ie(r, t, n);
	if (e === "billboard") return oe(r, t, n);
	if (e === "particle") return de(r, t, n);
	if (e === "telemetryOverlay") return _e(r, t, n);
	be(r, t, n);
}
function ie(e, t, n) {
	if (e === "emit") {
		let e = M(t, n, [
			"op",
			"signalId",
			"descriptor"
		]);
		z(e.signalId, `${n}.signalId`), ae(e.descriptor, `${n}.descriptor`);
	} else if (e === "create") {
		let e = M(t, n, [
			"op",
			"handle",
			"descriptor"
		]);
		Ae(e.handle, `${n}.handle`), ae(e.descriptor, `${n}.descriptor`);
	} else if (e === "update") {
		let e = M(t, n, [
			"op",
			"handle",
			"patch"
		]);
		Ae(e.handle, `${n}.handle`);
		let r = M(e.patch, `${n}.patch`, [
			"volume",
			"pitch",
			"looping",
			"spatialBlend",
			"attenuation",
			"pan",
			"emitter"
		]);
		L(r.volume, `${n}.patch.volume`, (e, t) => Le(e, t, 0, 1)), L(r.pitch, `${n}.patch.pitch`, (e, t) => Le(e, t, .25, 4)), L(r.looping, `${n}.patch.looping`, ze), L(r.spatialBlend, `${n}.patch.spatialBlend`, (e, t) => Le(e, t, 0, 1)), L(r.attenuation, `${n}.patch.attenuation`, Fe), L(r.pan, `${n}.patch.pan`, (e, t) => Le(e, t, -1, 1)), L(r.emitter, `${n}.patch.emitter`, (e, t) => j(e, t, !0));
	} else e === "destroy" ? Ae(M(t, n, ["op", "handle"]).handle, `${n}.handle`) : V(`${n}.op`, "is unsupported for audio");
}
function ae(e, t) {
	let n = M(e, t, [
		"clip",
		"bus",
		"volume",
		"pitch",
		"looping",
		"spatialBlend",
		"attenuation",
		"pan",
		"emitter"
	]), r = M(n.clip, `${t}.clip`, ["asset", "contentHash"]);
	z(r.asset, `${t}.clip.asset`), z(r.contentHash, `${t}.clip.contentHash`), B(n.bus, `${t}.bus`, [
		"sfx",
		"ambient",
		"ui"
	]), Le(n.volume, `${t}.volume`, 0, 1), Le(n.pitch, `${t}.pitch`, .25, 4), Le(n.spatialBlend, `${t}.spatialBlend`, 0, 1), Fe(n.attenuation, `${t}.attenuation`), Le(n.pan, `${t}.pan`, -1, 1), ze(n.looping, `${t}.looping`), j(n.emitter, `${t}.emitter`, !0);
}
function oe(e, t, n) {
	Ce(e, t, n, se, ce);
}
function se(e, t) {
	let n = M(e, t, [
		"anchor",
		"content",
		"font",
		"heightPixels",
		"color",
		"background",
		"maxDistance",
		"layer",
		"visible"
	]);
	j(n.anchor, `${t}.anchor`, !1), le(n.content, `${t}.content`), ue(n.font, `${t}.font`), Le(n.heightPixels, `${t}.heightPixels`, 8, 256), Ee(n.color, `${t}.color`), Ee(n.background, `${t}.background`), Le(n.maxDistance, `${t}.maxDistance`, Number.MIN_VALUE, 1e4), B(n.layer, `${t}.layer`, [
		"alwaysOnTop",
		"depthTested",
		"occluded"
	]), ze(n.visible, `${t}.visible`);
}
function ce(e, t) {
	let n = M(e, t, [
		"anchor",
		"content",
		"font",
		"heightPixels",
		"color",
		"background",
		"maxDistance",
		"layer",
		"visible"
	]);
	L(n.anchor, `${t}.anchor`, (e, t) => j(e, t, !1)), L(n.content, `${t}.content`, le), L(n.font, `${t}.font`, ue), L(n.heightPixels, `${t}.heightPixels`, (e, t) => Le(e, t, 8, 256)), L(n.color, `${t}.color`, Ee), L(n.background, `${t}.background`, Ee), L(n.maxDistance, `${t}.maxDistance`, (e, t) => Le(e, t, Number.MIN_VALUE, 1e4)), L(n.layer, `${t}.layer`, (e, t) => B(e, t, [
		"alwaysOnTop",
		"depthTested",
		"occluded"
	])), L(n.visible, `${t}.visible`, ze);
}
function le(e, t) {
	let n = B(N(e, t).kind, `${t}.kind`, [
		"text",
		"value",
		"icon"
	]);
	if (n === "text") {
		let n = M(e, t, [
			"kind",
			"localizationKey",
			"fallbackText",
			"arguments"
		]);
		z(n.localizationKey, `${t}.localizationKey`), z(n.fallbackText, `${t}.fallbackText`);
		let r = /* @__PURE__ */ new Set(), i = P(n.arguments, `${t}.arguments`);
		i.length > 8 && V(`${t}.arguments`, "must contain at most 8 entries"), i.forEach((e, n) => {
			let i = `${t}.arguments[${String(n)}]`, a = M(e, i, ["name", "value"]), o = z(a.name, `${i}.name`);
			z(a.value, `${i}.value`), r.has(o) && V(`${i}.name`, "is duplicated"), r.add(o);
		});
	} else if (n === "value") {
		let n = M(e, t, [
			"kind",
			"labelKey",
			"fallbackLabel",
			"value",
			"unitKey",
			"fallbackUnit"
		]);
		z(n.labelKey, `${t}.labelKey`), z(n.fallbackLabel, `${t}.fallbackLabel`), z(n.value, `${t}.value`), L(n.unitKey, `${t}.unitKey`, z), L(n.fallbackUnit, `${t}.fallbackUnit`, z);
	} else {
		let n = M(e, t, [
			"kind",
			"texture",
			"altKey",
			"fallbackAlt"
		]), r = M(n.texture, `${t}.texture`, ["asset", "contentHash"]);
		z(r.asset, `${t}.texture.asset`), z(r.contentHash, `${t}.texture.contentHash`), z(n.altKey, `${t}.altKey`), z(n.fallbackAlt, `${t}.fallbackAlt`);
	}
}
function ue(e, t) {
	if (B(N(e, t).kind, `${t}.kind`, ["system", "asset"]) === "system") z(M(e, t, ["kind", "family"]).family, `${t}.family`);
	else {
		let n = M(e, t, [
			"kind",
			"asset",
			"contentHash",
			"family"
		]);
		z(n.asset, `${t}.asset`), z(n.contentHash, `${t}.contentHash`), z(n.family, `${t}.family`);
	}
}
function de(e, t, n) {
	if (e === "emit") {
		let e = M(t, n, [
			"op",
			"signalId",
			"descriptor"
		]);
		z(e.signalId, `${n}.signalId`), fe(e.descriptor, `${n}.descriptor`);
	} else Ce(e, t, n, fe, pe);
}
function fe(e, t) {
	let n = M(e, t, [
		"anchor",
		"sprite",
		"ratePerSecond",
		"burstCount",
		"lifetimeSeconds",
		"velocityMin",
		"velocityMax",
		"acceleration",
		"sizeCurve",
		"colorCurve",
		"flipbookFramesPerSecond",
		"seed",
		"maxParticles",
		"visible"
	]);
	j(n.anchor, `${t}.anchor`, !1), me(n.sprite, `${t}.sprite`), Le(n.ratePerSecond, `${t}.ratePerSecond`, 0, 1e4), Le(n.flipbookFramesPerSecond, `${t}.flipbookFramesPerSecond`, 0, 120), Me(n.burstCount, `${t}.burstCount`), De(n.lifetimeSeconds, `${t}.lifetimeSeconds`, 2, 0, Number.MAX_VALUE), F(n.velocityMin, `${t}.velocityMin`), F(n.velocityMax, `${t}.velocityMax`), F(n.acceleration, `${t}.acceleration`), he(n.sizeCurve, `${t}.sizeCurve`), ge(n.colorCurve, `${t}.colorCurve`), je(n.seed, `${t}.seed`), Me(n.maxParticles, `${t}.maxParticles`), ze(n.visible, `${t}.visible`);
}
function pe(e, t) {
	let n = M(e, t, [
		"anchor",
		"sprite",
		"ratePerSecond",
		"burstCount",
		"lifetimeSeconds",
		"velocityMin",
		"velocityMax",
		"acceleration",
		"sizeCurve",
		"colorCurve",
		"flipbookFramesPerSecond",
		"maxParticles",
		"visible"
	]);
	L(n.anchor, `${t}.anchor`, (e, t) => j(e, t, !1)), L(n.sprite, `${t}.sprite`, me), L(n.ratePerSecond, `${t}.ratePerSecond`, Ie), L(n.burstCount, `${t}.burstCount`, Me), L(n.lifetimeSeconds, `${t}.lifetimeSeconds`, (e, t) => De(e, t, 2, 0, 60)), L(n.velocityMin, `${t}.velocityMin`, F), L(n.velocityMax, `${t}.velocityMax`, F), L(n.acceleration, `${t}.acceleration`, F), L(n.sizeCurve, `${t}.sizeCurve`, he), L(n.colorCurve, `${t}.colorCurve`, ge), L(n.flipbookFramesPerSecond, `${t}.flipbookFramesPerSecond`, (e, t) => Le(e, t, 0, 120)), L(n.maxParticles, `${t}.maxParticles`, Me), L(n.visible, `${t}.visible`, ze);
}
function me(e, t) {
	let n = M(e, t, [
		"asset",
		"contentHash",
		"frameCount"
	]);
	z(n.asset, `${t}.asset`), z(n.contentHash, `${t}.contentHash`), R(n.frameCount, `${t}.frameCount`, 1, 65535);
}
function he(e, t) {
	let n = P(e, t);
	(n.length < 2 || n.length > 8) && V(t, "must contain 2 to 8 keys");
	let r = -1;
	n.forEach((e, n) => {
		let i = `${t}[${String(n)}]`, a = M(e, i, ["age", "value"]), o = Le(a.age, `${i}.age`, 0, 1);
		Ie(a.value, `${i}.value`), o <= r && V(`${i}.age`, "must be strictly increasing"), r = o;
	}), (N(n[0], `${t}[0]`).age !== 0 || N(n[n.length - 1], `${t}[${String(n.length - 1)}]`).age !== 1) && V(t, "must start at age 0 and end at age 1");
}
function ge(e, t) {
	let n = P(e, t);
	(n.length < 2 || n.length > 8) && V(t, "must contain 2 to 8 keys");
	let r = -1;
	n.forEach((e, n) => {
		let i = `${t}[${String(n)}]`, a = M(e, i, ["age", "color"]), o = Le(a.age, `${i}.age`, 0, 1);
		Ee(a.color, `${i}.color`), o <= r && V(`${i}.age`, "must be strictly increasing"), r = o;
	}), (N(n[0], `${t}[0]`).age !== 0 || N(n[n.length - 1], `${t}[${String(n.length - 1)}]`).age !== 1) && V(t, "must start at age 0 and end at age 1");
}
function _e(e, t, n) {
	Ce(e, t, n, ve, ye);
}
function ve(e, t) {
	let n = M(e, t, [
		"title",
		"corner",
		"refreshIntervalMs",
		"maxFrameTimeSamples",
		"visible"
	]);
	z(n.title, `${t}.title`), B(n.corner, `${t}.corner`, [
		"topLeft",
		"topRight",
		"bottomLeft",
		"bottomRight"
	]), R(n.refreshIntervalMs, `${t}.refreshIntervalMs`, 100, 5e3), R(n.maxFrameTimeSamples, `${t}.maxFrameTimeSamples`, 1, 240), ze(n.visible, `${t}.visible`);
}
function ye(e, t) {
	let n = M(e, t, [
		"title",
		"corner",
		"refreshIntervalMs",
		"maxFrameTimeSamples",
		"visible"
	]);
	L(n.title, `${t}.title`, z), L(n.corner, `${t}.corner`, (e, t) => B(e, t, [
		"topLeft",
		"topRight",
		"bottomLeft",
		"bottomRight"
	])), L(n.refreshIntervalMs, `${t}.refreshIntervalMs`, (e, t) => R(e, t, 100, 5e3)), L(n.maxFrameTimeSamples, `${t}.maxFrameTimeSamples`, (e, t) => R(e, t, 1, 240)), L(n.visible, `${t}.visible`, ze);
}
function be(e, t, n) {
	if (e === "create") {
		let e = M(t, n, [
			"op",
			"handle",
			"descriptor"
		]);
		Ae(e.handle, `${n}.handle`);
		let r = M(e.descriptor, `${n}.descriptor`, [
			"target",
			"asset",
			"contentHash",
			"tickDurationMillis",
			"controller"
		]);
		Ae(r.target, `${n}.descriptor.target`), z(r.asset, `${n}.descriptor.asset`), z(r.contentHash, `${n}.descriptor.contentHash`), Me(r.tickDurationMillis, `${n}.descriptor.tickDurationMillis`), xe(r.controller, `${n}.descriptor.controller`);
	} else if (e === "update") {
		let e = M(t, n, [
			"op",
			"handle",
			"controller"
		]);
		Ae(e.handle, `${n}.handle`), xe(e.controller, `${n}.controller`);
	} else e === "destroy" ? Ae(M(t, n, ["op", "handle"]).handle, `${n}.handle`) : V(`${n}.op`, "is unsupported for animation");
}
function xe(e, t) {
	let n = M(e, t, [
		"entity",
		"graphId",
		"graphVersion",
		"stateId",
		"revision",
		"controllerTick",
		"motion",
		"transition",
		"transitionFact"
	]);
	je(n.entity, `${t}.entity`), z(n.graphId, `${t}.graphId`), Me(n.graphVersion, `${t}.graphVersion`), z(n.stateId, `${t}.stateId`), je(n.revision, `${t}.revision`), je(n.controllerTick, `${t}.controllerTick`), Se(n.motion, `${t}.motion`), L(n.transition, `${t}.transition`, (e, t) => {
		let n = M(e, t, [
			"transitionId",
			"fromStateId",
			"toStateId",
			"elapsedTicks",
			"durationTicks",
			"targetMotion"
		]);
		z(n.transitionId, `${t}.transitionId`), z(n.fromStateId, `${t}.fromStateId`), z(n.toStateId, `${t}.toStateId`), Me(n.elapsedTicks, `${t}.elapsedTicks`), Me(n.durationTicks, `${t}.durationTicks`), Se(n.targetMotion, `${t}.targetMotion`);
	}), L(n.transitionFact, `${t}.transitionFact`, (e, t) => {
		let n = M(e, t, [
			"controllerTick",
			"transitionId",
			"fromStateId",
			"toStateId",
			"moment",
			"durationTicks"
		]);
		je(n.controllerTick, `${t}.controllerTick`), z(n.transitionId, `${t}.transitionId`), z(n.fromStateId, `${t}.fromStateId`), z(n.toStateId, `${t}.toStateId`), B(n.moment, `${t}.moment`, ["started", "completed"]), Me(n.durationTicks, `${t}.durationTicks`);
	});
}
function Se(e, t) {
	let n = M(e, t, [
		"clipA",
		"clipB",
		"blendWeightMilli",
		"speedMilli"
	]);
	z(n.clipA, `${t}.clipA`), L(n.clipB, `${t}.clipB`, z), Ne(n.blendWeightMilli, `${t}.blendWeightMilli`), Ne(n.speedMilli, `${t}.speedMilli`);
}
function Ce(e, t, n, r, i) {
	if (e === "create") {
		let e = M(t, n, [
			"op",
			"handle",
			"descriptor"
		]);
		Ae(e.handle, `${n}.handle`), r(e.descriptor, `${n}.descriptor`);
	} else if (e === "update") {
		let e = M(t, n, [
			"op",
			"handle",
			"patch"
		]);
		Ae(e.handle, `${n}.handle`), i(e.patch, `${n}.patch`);
	} else e === "destroy" ? Ae(M(t, n, ["op", "handle"]).handle, `${n}.handle`) : V(`${n}.op`, "is unsupported for retained presentation");
}
function j(e, t, n) {
	let r = N(e, t), i = n ? [
		"global2d",
		"world3d",
		"entityAttached"
	] : ["world", "entityAttached"], a = B(r.kind, `${t}.kind`, i);
	if (a === "global2d") M(e, t, ["kind"]);
	else if (a === "world" || a === "world3d") F(M(e, t, ["kind", "position"]).position, `${t}.position`);
	else {
		let n = M(e, t, [
			"kind",
			"entity",
			"offset"
		]);
		je(n.entity, `${t}.entity`), F(n.offset, `${t}.offset`);
	}
}
function M(e, t, n) {
	let r = N(e, t), i = new Set(n);
	return Object.keys(r).forEach((e) => {
		i.has(e) || V(`${t}.${e}`, "is unknown");
	}), n.forEach((e) => {
		Object.hasOwn(r, e) || V(`${t}.${e}`, "is required");
	}), r;
}
function we(e, t, n, r) {
	let i = N(e, t), a = /* @__PURE__ */ new Set([...n, ...r]);
	return Object.keys(i).forEach((e) => {
		a.has(e) || V(`${t}.${e}`, "is unknown");
	}), n.forEach((e) => {
		Object.hasOwn(i, e) || V(`${t}.${e}`, "is required");
	}), i;
}
function N(e, t) {
	return (typeof e != "object" || !e || Array.isArray(e)) && V(t, "must be an object"), e;
}
function P(e, t) {
	return Array.isArray(e) || V(t, "must be an array"), e;
}
function Te(e, t, n) {
	let r = P(e, t);
	return r.length !== n && V(t, `must contain ${String(n)} values`), r;
}
function F(e, t) {
	return Te(e, t, 3).map((e, n) => Pe(e, `${t}[${String(n)}]`));
}
function I(e, t) {
	De(e, t, 3, 0, 1);
}
function Ee(e, t) {
	De(e, t, 4, 0, 1);
}
function De(e, t, n, r, i) {
	return Te(e, t, n).map((e, n) => Le(e, `${t}[${String(n)}]`, r, i));
}
function Oe(e, t, n, r) {
	let i = P(e, t);
	return i.length !== n && V(t, `must contain ${String(n)} values`), i.map((e, n) => r ? Me(e, `${t}[${String(n)}]`) : Pe(e, `${t}[${String(n)}]`));
}
function L(e, t, n) {
	e !== null && n(e, t);
}
function ke(e, t) {
	L(e, t, Ae);
}
function Ae(e, t) {
	return je(e, t);
}
function je(e, t) {
	return R(e, t, 0, r);
}
function Me(e, t) {
	return R(e, t, 0, 2 ** 53 - 1);
}
function Ne(e, t) {
	return (typeof e != "number" || !Number.isSafeInteger(e)) && V(t, "must be a safe integer"), e;
}
function R(e, t, n, r) {
	let i = Ne(e, t);
	return (i < n || i > r) && V(t, `must be in ${String(n)}..=${String(r)}`), i;
}
function Pe(e, t) {
	return (typeof e != "number" || !Number.isFinite(e)) && V(t, "must be finite"), e;
}
function Fe(e, t) {
	let n = Pe(e, t);
	return n <= 0 && V(t, "must be positive"), n;
}
function Ie(e, t) {
	let n = Pe(e, t);
	return n < 0 && V(t, "must be non-negative"), n;
}
function Le(e, t, n, r) {
	let i = Pe(e, t);
	return (i < n || i > r) && V(t, `must be in ${String(n)}..=${String(r)}`), i;
}
function Re(e, t) {
	return typeof e != "string" && V(t, "must be a string"), e;
}
function z(e, t) {
	let n = Re(e, t);
	return n.trim() === "" && V(t, "must be non-empty"), n;
}
function ze(e, t) {
	return typeof e != "boolean" && V(t, "must be a boolean"), e;
}
function B(e, t, n) {
	let r = Re(e, t);
	return n.includes(r) || V(t, `must be one of ${n.join(", ")}`), r;
}
function V(e, t) {
	throw new i(`${e} ${t}`);
}
var Be = 2048, Ve = 8388608, He = class extends Error {
	path;
	code = "invalid_view_composition";
	constructor(e, t) {
		super(`${e} ${t}`), this.path = e, this.name = "RendererViewCompositionValidationError";
	}
};
function Ue(e) {
	e.schemaVersion !== 1 && $e("composition.schemaVersion", "must equal 1"), Je(e.cameras, "composition.cameras", 4), Je(e.targets, "composition.targets", 4), Je(e.views, "composition.views", 8), Je(e.presentations, "composition.presentations", 4);
	let t = /* @__PURE__ */ new Map();
	for (let [n, r] of e.cameras.entries()) {
		let e = `composition.cameras[${String(n)}]`;
		Ke(r.id, `${e}.id`, t), Ye(r.pose.position, `${e}.pose.position`), Xe(r.pose.pitchDegrees, `${e}.pose.pitchDegrees`), Xe(r.pose.yawDegrees, `${e}.pose.yawDegrees`), We(r.projection, `${e}.projection`), t.set(r.id, r);
	}
	let n = /* @__PURE__ */ new Map(), r = 0;
	for (let [t, i] of e.targets.entries()) {
		let e = `composition.targets[${String(t)}]`;
		Ke(i.id, `${e}.id`, n), Ze(i.revision, `${e}.revision`, 1, 2 ** 53 - 1), Ze(i.width, `${e}.width`, 1, Be), Ze(i.height, `${e}.height`, 1, Be), i.color !== "rgba8_srgb" && $e(`${e}.color`, "must equal rgba8_srgb"), i.depth !== "depth24" && i.depth !== "none" && $e(`${e}.depth`, "must equal depth24 or none"), i.sampling !== "linear" && i.sampling !== "nearest" && $e(`${e}.sampling`, "must equal linear or nearest"), r = Qe(r, i.width * i.height, "composition.targets"), r > 8388608 && $e("composition.targets", `aggregate pixels must not exceed ${String(Ve)}`), n.set(i.id, i);
	}
	let i = /* @__PURE__ */ new Set(), a = /* @__PURE__ */ new Set();
	for (let [r, o] of e.views.entries()) {
		let e = `composition.views[${String(r)}]`;
		if (Ke(o.id, `${e}.id`, i), qe(o.cameraId, `${e}.cameraId`), t.has(o.cameraId) || $e(`${e}.cameraId`, `does not name an admitted camera ${JSON.stringify(o.cameraId)}`), Ge(o.viewport, `${e}.viewport`), Ze(o.order, `${e}.order`, 0, 65535), o.target.kind === "primary") continue;
		o.target.kind !== "offscreen" && $e(`${e}.target.kind`, "must equal primary or offscreen");
		let s = n.get(o.target.targetId);
		s === void 0 && $e(`${e}.target.targetId`, "does not name an admitted target"), s.revision !== o.target.targetRevision && $e(`${e}.target.targetRevision`, "must equal the admitted target revision"), a.has(s.id) && $e(`${e}.target.targetId`, "already has a producing view"), a.add(s.id);
	}
	let o = /* @__PURE__ */ new Set();
	for (let [t, r] of e.presentations.entries()) {
		let e = `composition.presentations[${String(t)}]`;
		Ke(r.id, `${e}.id`, o);
		let i = n.get(r.sourceTargetId);
		i === void 0 && $e(`${e}.sourceTargetId`, "does not name an admitted target"), i.revision !== r.sourceTargetRevision && $e(`${e}.sourceTargetRevision`, "must equal the admitted target revision"), a.has(i.id) || $e(`${e}.sourceTargetId`, "must have one producing view in the same composition"), r.destination.kind !== "primary" && $e(`${e}.destination.kind`, "must equal primary; render-target feedback is unsupported"), Ge(r.destination.viewport, `${e}.destination.viewport`), Ze(r.order, `${e}.order`, 0, 65535);
	}
	return e;
}
function We(e, t) {
	if (Xe(e.near, `${t}.near`), Xe(e.far, `${t}.far`), (e.near <= 0 || e.far <= e.near) && $e(t, "must have 0 < near < far"), e.kind === "perspective") {
		Xe(e.fovYDegrees, `${t}.fovYDegrees`), (e.fovYDegrees <= 0 || e.fovYDegrees >= 180) && $e(`${t}.fovYDegrees`, "must be greater than 0 and less than 180");
		return;
	}
	if (e.kind === "orthographic") {
		Xe(e.verticalSize, `${t}.verticalSize`), e.verticalSize <= 0 && $e(`${t}.verticalSize`, "must be greater than 0");
		return;
	}
	$e(`${t}.kind`, "must equal perspective or orthographic");
}
function Ge(e, t) {
	Xe(e.x, `${t}.x`), Xe(e.y, `${t}.y`), Xe(e.width, `${t}.width`), Xe(e.height, `${t}.height`), (e.x < 0 || e.y < 0 || e.width <= 0 || e.height <= 0) && $e(t, "must have non-negative origin and positive extent"), (e.x + e.width > 1 || e.y + e.height > 1) && $e(t, "must fit inside normalized destination bounds");
}
function Ke(e, t, n) {
	qe(e, t), n.has(e) && $e(t, `duplicates ${JSON.stringify(e)}`);
}
function qe(e, t) {
	/^[a-z][a-z0-9._-]{0,63}$/u.test(e) || $e(t, "must be a lowercase stable identifier of at most 64 characters");
}
function Je(e, t, n) {
	Array.isArray(e) || $e(t, "must be an array"), e.length > n && $e(t, `must contain at most ${String(n)} entries`);
}
function Ye(e, t) {
	(!Array.isArray(e) || e.length !== 3) && $e(t, "must contain exactly 3 values"), e.forEach((e, n) => Xe(e, `${t}[${String(n)}]`));
}
function Xe(e, t) {
	Number.isFinite(e) || $e(t, "must be finite");
}
function Ze(e, t, n, r) {
	(!Number.isSafeInteger(e) || e < n || e > r) && $e(t, `must be a safe integer in ${String(n)}..=${String(r)}`);
}
function Qe(e, t, n) {
	let r = e + t;
	return Number.isSafeInteger(r) || $e(n, "aggregate size overflowed"), r;
}
function $e(e, t) {
	throw new He(e, t);
}
//#endregion
//#region packages/render-projection/dist/retained-projection.js
var et, H = class extends Error {
	constructor(e) {
		super(e), this.name = "RenderProjectionError";
	}
}, tt = class {
	#e = /* @__PURE__ */ new Map();
	#t = /* @__PURE__ */ new Map();
	#n = /* @__PURE__ */ new Map();
	#r = /* @__PURE__ */ new Map();
	#i = /* @__PURE__ */ new Map();
	#a = /* @__PURE__ */ new Map();
	#o = /* @__PURE__ */ new Map();
	#s = /* @__PURE__ */ new Map();
	#c = st();
	#l = !1;
	applyFrame(e) {
		let { staged: t, instructions: n } = this.#q(e);
		return this.#K(t), n;
	}
	validateFrame(e) {
		return this.#q(e).instructions;
	}
	applyDiff(e) {
		switch (xt(e), e.op) {
			case "create": return [this.#u(e)];
			case "update": return [this.#d(e)];
			case "destroy": return this.#f(e.handle);
			case "replaceMeshPayload": return [this.#p(e)];
			case "createLight": return [this.#m(e)];
			case "updateLight": return [this.#h(e)];
			case "defineMaterial": return [this.#g(e.material)];
			case "setMaterialInstanceParameters": return [this.#S(e)];
			case "defineTexture": return [this.#_(e.texture)];
			case "defineSpriteAtlas": return [this.#v(e.atlas)];
			case "defineStaticMesh": return [this.#y(e.asset)];
			case "defineAnimatedMesh": return [this.#b(e.asset)];
			case "defineVoxelObject": return [this.#T(e.asset)];
			case "releaseVoxelObject": return [this.#E(e.asset)];
			case "createStaticMeshInstance": return [this.#x(e)];
			case "createAnimatedMeshInstance": return [this.#C(e)];
			case "setAnimatedMeshPlayback": return [this.#w(e)];
			case "createVoxelObjectInstance": return [this.#D(e)];
			case "setVoxelObjectFrame": return [this.#O(e)];
			case "createSprite": return [this.#k(e)];
			case "updateSprite": return [this.#A(e)];
			default: throw new H(`unsupported render diff op ${JSON.stringify(e.op)}`);
		}
	}
	has(e) {
		return this.#e.has(e) || this.#t.has(e);
	}
	get handleCount() {
		return this.#e.size + this.#t.size;
	}
	lastFrameStagingStatistics() {
		return { ...this.#c };
	}
	node(e) {
		let t = this.#e.get(e);
		return t === void 0 ? void 0 : lt(t);
	}
	light(e) {
		let t = this.#t.get(e);
		return t === void 0 ? void 0 : ct(t);
	}
	materialDescriptor(e) {
		return U(this.#n.get(e));
	}
	textureDescriptor(e) {
		return U(this.#r.get(e));
	}
	spriteAtlas(e) {
		return U(this.#i.get(e));
	}
	staticMesh(e) {
		return U(this.#a.get(e)?.asset);
	}
	animatedMesh(e) {
		return U(this.#o.get(e)?.asset);
	}
	voxelObject(e) {
		return U(this.#s.get(e)?.asset);
	}
	staticMeshRefCount(e) {
		return this.#a.get(e)?.refCount ?? 0;
	}
	animatedMeshRefCount(e) {
		return this.#o.get(e)?.refCount ?? 0;
	}
	voxelObjectRefCount(e) {
		return this.#s.get(e)?.refCount ?? 0;
	}
	snapshot() {
		return {
			nodes: Et(this.#e).map((e) => lt(this.#R(e, "snapshot"))),
			lights: Et(this.#t).map((e) => ct(this.#z(e, "snapshot"))),
			materials: Dt(this.#n),
			textures: Dt(this.#r),
			spriteAtlases: Dt(this.#i),
			staticMeshes: [...this.#a.values()].map((e) => U(e.asset)).sort((e, t) => e.asset.localeCompare(t.asset)),
			animatedMeshes: [...this.#o.values()].map((e) => U(e.asset)).sort((e, t) => e.asset.localeCompare(t.asset)),
			voxelObjects: [...this.#s.values()].map((e) => U(e.asset)).sort((e, t) => e.asset.localeCompare(t.asset))
		};
	}
	pickMesh(e) {
		let t = this.#e.get(e), n = t?.meshPayload;
		if (!(t === void 0 || n == null)) return {
			handle: e,
			provenance: n.provenance,
			sourceEntity: t.metadata.sourceEntity,
			sourceSceneNode: t.metadata.sourceSceneNode
		};
	}
	pickSprite(e) {
		let t = this.#e.get(e);
		if (t?.kind !== "sprite") return;
		let n = t.sprite.attachment;
		return {
			handle: e,
			sourceEntity: n.sourceEntity,
			sourceSceneNode: n.sourceSceneNode,
			asset: t.sprite.asset,
			attachmentPoint: n.attachmentPoint
		};
	}
	#u(e) {
		this.#I(e.handle, "create");
		let t = this.#L(e.parent, "create.parent"), n = U(e.node), r = {
			handle: e.handle,
			parent: t,
			children: /* @__PURE__ */ new Set(),
			kind: "primitive",
			layer: t === null ? n.layer : this.#R(t, "create.parent").layer,
			transform: U(n.transform),
			visible: n.visible,
			metadata: U(n.metadata),
			material: U(n.material),
			meshPayload: null,
			node: n
		};
		return this.#P(r, "create"), this.#N(r), {
			op: "upsertNode",
			node: lt(r)
		};
	}
	#d(e) {
		this.#R(e.handle, "update").layer === "viewmodel" && e.transform !== null && rt(e.transform, "update.transform");
		let t = this.#B(e.handle, "update");
		return e.transform !== null && (t.transform = U(e.transform), t.kind === "primitive" ? t.node = {
			...t.node,
			transform: U(e.transform)
		} : t.kind === "staticMesh" || t.kind === "animatedMesh" || t.kind === "voxelObject" ? t.instance = {
			...t.instance,
			transform: U(e.transform)
		} : t.sprite = {
			...t.sprite,
			transform: U(e.transform)
		}), e.material !== null && (t.material = U(e.material), t.kind === "primitive" && (t.node = {
			...t.node,
			material: U(e.material)
		})), e.visible !== null && (t.visible = e.visible, t.kind === "primitive" ? t.node = {
			...t.node,
			visible: e.visible
		} : t.kind === "staticMesh" || t.kind === "animatedMesh" || t.kind === "voxelObject" ? t.instance = {
			...t.instance,
			visible: e.visible
		} : t.sprite = {
			...t.sprite,
			visible: e.visible
		}), e.metadata !== null && (t.metadata = U(e.metadata), t.kind === "primitive" ? t.node = {
			...t.node,
			metadata: U(e.metadata)
		} : t.kind === "staticMesh" || t.kind === "animatedMesh" || t.kind === "voxelObject" ? t.instance = {
			...t.instance,
			metadata: U(e.metadata)
		} : t.sprite = {
			...t.sprite,
			metadata: U(e.metadata)
		}), {
			op: "upsertNode",
			node: lt(t)
		};
	}
	#f(e) {
		let t = this.#t.get(e);
		if (t !== void 0) return this.#t.delete(e), t.parent !== null && this.#B(t.parent, "destroyLight.parent").children.delete(e), [{
			op: "removeLight",
			handle: e
		}];
		let n = this.#R(e, "destroy"), r = [];
		for (let e of [...n.children].sort(Ot)) r.push(...this.#f(e));
		if (this.#e.delete(e), n.parent !== null && this.#B(n.parent, "destroy.parent").children.delete(e), n.kind === "staticMesh") {
			let e = this.#H(n.asset);
			e !== void 0 && --e.refCount;
		} else if (n.kind === "animatedMesh") {
			let e = this.#U(n.asset);
			e !== void 0 && --e.refCount;
		} else if (n.kind === "voxelObject") {
			let e = this.#W(n.asset);
			e !== void 0 && --e.refCount;
		}
		return r.push({
			op: "removeNode",
			handle: e
		}), r;
	}
	#p(e) {
		let t = this.#R(e.handle, "replaceMeshPayload");
		if (t.kind !== "primitive" || t.node.geometry.kind === "group") throw new H(`replaceMeshPayload: handle ${e.handle} is not a primitive mesh`);
		bt(e.payload, "replaceMeshPayload.payload"), t.layer === "viewmodel" && it(e.payload.bounds, "replaceMeshPayload.payload.bounds");
		let n = this.#B(e.handle, "replaceMeshPayload");
		if (n.kind !== "primitive") throw new H(`replaceMeshPayload: handle ${e.handle} is not a primitive mesh`);
		return n.meshPayload = U(e.payload), {
			op: "upsertNode",
			node: lt(n)
		};
	}
	#m(e) {
		if (this.#I(e.handle, "createLight"), this.#t.size >= 256) throw new H("createLight: retained light quota 256 exceeded");
		let t = this.#L(e.parent, "createLight.parent");
		if (t !== null && this.#R(t, "createLight.parent").layer === "viewmodel") throw new H("createLight: camera-relative presentation uses the backend-owned neutral light rig");
		ht(e.light, "createLight.light");
		let n = {
			handle: e.handle,
			parent: t,
			light: U(e.light)
		};
		return this.#t.set(e.handle, n), t !== null && this.#B(t, "createLight.parent").children.add(e.handle), {
			op: "upsertLight",
			light: ct(n)
		};
	}
	#h(e) {
		let t = this.#z(e.handle, "updateLight");
		if (ht(e.light, "updateLight.light"), t.light.kind !== e.light.kind) throw new H(`updateLight: handle ${e.handle} cannot change kind from ${t.light.kind} to ${e.light.kind}`);
		let n = this.#V(e.handle, "updateLight");
		return n.light = U(e.light), {
			op: "upsertLight",
			light: ct(n)
		};
	}
	#g(e) {
		return this.#n.set(e.id, U(e)), {
			op: "defineMaterial",
			material: U(e)
		};
	}
	#_(e) {
		return this.#r.set(e.id, U(e)), {
			op: "defineTexture",
			texture: U(e)
		};
	}
	#v(e) {
		return this.#i.set(e.id, U(e)), {
			op: "defineSpriteAtlas",
			atlas: U(e)
		};
	}
	#y(e) {
		bt(e.payload, `defineStaticMesh(${e.asset}).payload`);
		let t = this.#a.get(e.asset);
		if (t !== void 0 && t.refCount > 0) throw new H(`defineStaticMesh: asset ${e.asset} is in use by ${t.refCount} instance(s)`);
		return this.#a.set(e.asset, {
			asset: U(e),
			refCount: 0
		}), {
			op: "defineStaticMesh",
			asset: U(e)
		};
	}
	#b(e) {
		ut(e, `defineAnimatedMesh(${e.asset})`);
		let t = this.#o.get(e.asset);
		if (t !== void 0 && t.refCount > 0) throw new H(`defineAnimatedMesh: asset ${e.asset} is in use by ${t.refCount} instance(s)`);
		return this.#o.set(e.asset, {
			asset: U(e),
			refCount: 0
		}), {
			op: "defineAnimatedMesh",
			asset: U(e)
		};
	}
	#x(e) {
		this.#I(e.handle, "createStaticMeshInstance");
		let t = this.#a.get(e.instance.asset);
		if (t === void 0) throw new H(`createStaticMeshInstance: undefined static mesh asset ${e.instance.asset}`);
		let n = this.#L(e.parent, "createStaticMeshInstance.parent"), r = U(e.instance), i = new Set(t.asset.materialSlots.map((e) => e.slot));
		for (let e of r.materialOverrides) if (!i.has(e.slot)) throw new H(`createStaticMeshInstance: override for unbound slot ${e.slot} on ${r.asset}`);
		let a = {
			handle: e.handle,
			parent: n,
			children: /* @__PURE__ */ new Set(),
			kind: "staticMesh",
			layer: n === null ? "scene" : this.#R(n, "createStaticMeshInstance.parent").layer,
			transform: U(r.transform),
			visible: r.visible,
			metadata: U(r.metadata),
			material: null,
			meshPayload: U(t.asset.payload),
			asset: r.asset,
			instance: r,
			materialParameters: /* @__PURE__ */ new Map()
		};
		return this.#P(a, "createStaticMeshInstance"), this.#H(r.asset).refCount += 1, this.#N(a), {
			op: "upsertNode",
			node: lt(a)
		};
	}
	#S(e) {
		let t = this.#R(e.handle, "setMaterialInstanceParameters");
		if (t.kind !== "staticMesh") throw new H(`setMaterialInstanceParameters: handle ${e.handle} is not a static mesh`);
		let n = this.#a.get(t.asset);
		if (n === void 0 || !n.asset.materialSlots.some((t) => t.slot === e.slot)) throw new H(`setMaterialInstanceParameters: unbound slot ${e.slot} on ${t.asset}`);
		let r = this.#B(e.handle, "setMaterialInstanceParameters");
		if (r.kind !== "staticMesh") throw new H(`setMaterialInstanceParameters: handle ${e.handle} is not a static mesh`);
		return e.parameters === null ? r.materialParameters.delete(e.slot) : r.materialParameters.set(e.slot, U(e.parameters)), {
			op: "upsertNode",
			node: lt(r)
		};
	}
	#C(e) {
		this.#I(e.handle, "createAnimatedMeshInstance");
		let t = this.#o.get(e.instance.asset);
		if (t === void 0) throw new H(`createAnimatedMeshInstance: undefined animated mesh asset ${e.instance.asset}`);
		e.instance.playback !== null && mt(t.asset, e.instance.playback, "createAnimatedMeshInstance.playback");
		let n = this.#L(e.parent, "createAnimatedMeshInstance.parent"), r = U(e.instance), i = {
			handle: e.handle,
			parent: n,
			children: /* @__PURE__ */ new Set(),
			kind: "animatedMesh",
			layer: n === null ? "scene" : this.#R(n, "createAnimatedMeshInstance.parent").layer,
			transform: U(r.transform),
			visible: r.visible,
			metadata: U(r.metadata),
			material: null,
			meshPayload: null,
			asset: r.asset,
			instance: r,
			playback: U(r.playback)
		};
		return this.#P(i, "createAnimatedMeshInstance"), this.#U(r.asset).refCount += 1, this.#N(i), {
			op: "upsertNode",
			node: lt(i)
		};
	}
	#w(e) {
		let t = this.#R(e.handle, "setAnimatedMeshPlayback");
		if (t.kind !== "animatedMesh") throw new H(`setAnimatedMeshPlayback: handle ${e.handle} is not an animated mesh`);
		let n = this.#o.get(t.asset);
		if (n === void 0) throw new H(`setAnimatedMeshPlayback: missing animated mesh asset ${t.asset}`);
		mt(n.asset, e.playback, "setAnimatedMeshPlayback.playback");
		let r = this.#B(e.handle, "setAnimatedMeshPlayback");
		if (r.kind !== "animatedMesh") throw new H(`setAnimatedMeshPlayback: handle ${e.handle} is not an animated mesh`);
		return r.playback = U(e.playback), r.instance = {
			...r.instance,
			playback: U(e.playback)
		}, {
			op: "upsertNode",
			node: lt(r)
		};
	}
	#T(e) {
		dt(e, `defineVoxelObject(${e.asset})`);
		let t = this.#s.get(e.asset), n = [];
		if (t !== void 0) for (let t of this.#e.values()) {
			if (t.kind !== "voxelObject" || t.asset !== e.asset) continue;
			ft(e, t.frame, "defineVoxelObject.liveInstance"), pt(e, t.instance.materialOverrides, "defineVoxelObject.liveInstance");
			let r = e.meshes[e.frames[t.frame].mesh].payload;
			t.layer === "viewmodel" && it(r.bounds, "defineVoxelObject.liveInstance.bounds"), n.push({
				payload: r,
				handle: t.handle
			});
		}
		for (let e of n) {
			let t = this.#B(e.handle, "defineVoxelObject.liveInstance");
			if (t.kind !== "voxelObject") throw new H(`defineVoxelObject.liveInstance: handle ${e.handle} is not a voxel object`);
			t.meshPayload = U(e.payload);
		}
		return this.#s.set(e.asset, {
			asset: U(e),
			refCount: t?.refCount ?? 0
		}), {
			op: "defineVoxelObject",
			asset: U(e)
		};
	}
	#E(e) {
		let t = this.#s.get(e);
		if (t === void 0) throw new H(`releaseVoxelObject: undefined voxel object ${e}`);
		if (t.refCount !== 0) throw new H(`releaseVoxelObject: ${e} is in use by ${t.refCount} instance(s)`);
		return this.#s.delete(e), {
			op: "releaseVoxelObject",
			asset: e
		};
	}
	#D(e) {
		this.#I(e.handle, "createVoxelObjectInstance");
		let t = this.#s.get(e.instance.asset);
		if (t === void 0) throw new H(`createVoxelObjectInstance: undefined voxel object ${e.instance.asset}`);
		ft(t.asset, e.instance.frame, "createVoxelObjectInstance.frame"), pt(t.asset, e.instance.materialOverrides, "createVoxelObjectInstance.materialOverrides");
		let n = this.#L(e.parent, "createVoxelObjectInstance.parent"), r = U(e.instance), i = {
			handle: e.handle,
			parent: n,
			children: /* @__PURE__ */ new Set(),
			kind: "voxelObject",
			layer: n === null ? "scene" : this.#R(n, "createVoxelObjectInstance.parent").layer,
			transform: U(r.transform),
			visible: r.visible,
			metadata: U(r.metadata),
			material: null,
			meshPayload: U(t.asset.meshes[t.asset.frames[r.frame].mesh].payload),
			asset: r.asset,
			instance: r,
			frame: r.frame
		};
		return this.#P(i, "createVoxelObjectInstance"), this.#W(r.asset).refCount += 1, this.#N(i), {
			op: "upsertNode",
			node: lt(i)
		};
	}
	#O(e) {
		let t = this.#R(e.handle, "setVoxelObjectFrame");
		if (t.kind !== "voxelObject") throw new H(`setVoxelObjectFrame: handle ${e.handle} is not a voxel object`);
		let n = this.#s.get(t.asset);
		if (n === void 0) throw new H(`setVoxelObjectFrame: missing voxel object ${t.asset}`);
		ft(n.asset, e.frame, "setVoxelObjectFrame.frame");
		let r = n.asset.meshes[n.asset.frames[e.frame].mesh].payload;
		t.layer === "viewmodel" && it(r.bounds, "setVoxelObjectFrame.bounds");
		let i = this.#B(e.handle, "setVoxelObjectFrame");
		if (i.kind !== "voxelObject") throw new H(`setVoxelObjectFrame: handle ${e.handle} is not a voxel object`);
		return i.frame = e.frame, i.instance = {
			...i.instance,
			frame: e.frame
		}, i.meshPayload = U(r), {
			op: "upsertNode",
			node: lt(i)
		};
	}
	#k(e) {
		this.#I(e.handle, "createSprite");
		let t = this.#L(e.parent, "createSprite.parent"), n = U(e.sprite), r = {
			handle: e.handle,
			parent: t,
			children: /* @__PURE__ */ new Set(),
			kind: "sprite",
			layer: t === null ? "scene" : this.#R(t, "createSprite.parent").layer,
			transform: U(n.transform),
			visible: n.visible,
			metadata: U(n.metadata),
			material: null,
			meshPayload: null,
			sprite: n,
			frameUv: this.#j(n.asset, n.frame),
			frameSize: this.#M(n.asset, n.frame, n.size),
			renderOrder: n.renderOrder
		};
		return this.#P(r, "createSprite"), this.#N(r), {
			op: "upsertNode",
			node: lt(r)
		};
	}
	#A(e) {
		if (this.#R(e.handle, "updateSprite").kind !== "sprite") throw new H(`updateSprite: handle ${e.handle} is not a sprite`);
		let t = this.#B(e.handle, "updateSprite");
		if (t.kind !== "sprite") throw new H(`updateSprite: handle ${e.handle} is not a sprite`);
		return e.frame !== null && (t.sprite = {
			...t.sprite,
			frame: e.frame
		}, t.frameUv = this.#j(t.sprite.asset, e.frame), t.frameSize = this.#M(t.sprite.asset, e.frame, t.sprite.size)), e.tint !== null && (t.sprite = {
			...t.sprite,
			tint: U(e.tint)
		}), e.renderOrder !== null && (t.sprite = {
			...t.sprite,
			renderOrder: e.renderOrder
		}, t.renderOrder = e.renderOrder), e.visible !== null && (t.visible = e.visible, t.sprite = {
			...t.sprite,
			visible: e.visible
		}), {
			op: "upsertNode",
			node: lt(t)
		};
	}
	#j(e, t) {
		let n = this.#i.get(e)?.frames.find((e) => e.frame === t);
		return n === void 0 ? [
			0,
			0,
			1,
			1
		] : [
			n.uvMin[0],
			n.uvMin[1],
			n.uvMax[0],
			n.uvMax[1]
		];
	}
	#M(e, t, n) {
		let r = this.#i.get(e)?.frames.find((e) => e.frame === t);
		return r?.size === void 0 ? [n[0], n[1]] : [r.size[0], r.size[1]];
	}
	#N(e) {
		this.#e.set(e.handle, e), e.parent !== null && this.#B(e.parent, "insert.parent").children.add(e.handle);
	}
	#P(e, t) {
		if (e.layer !== "viewmodel") return;
		rt(e.transform, `${t}.transform`), this.#F(e, t);
		let n = [...this.#e.values()].filter((e) => e.layer === "viewmodel");
		if (n.length >= 128) throw new H(`${t}: viewmodel node capacity 128 is exhausted`);
		let r = ot(e);
		if (r === null) return;
		let i = new Set(n.map(ot).filter((e) => e !== null));
		if (!i.has(r) && i.size >= 16) throw new H(`${t}: viewmodel asset capacity 16 is exhausted`);
	}
	#F(e, t) {
		if (e.kind === "primitive") {
			e.node.geometry.kind === "line" && at([e.node.geometry.a, e.node.geometry.b], `${t}.geometry`), e.meshPayload !== null && it(e.meshPayload.bounds, `${t}.meshPayload.bounds`);
			return;
		}
		if (e.kind === "animatedMesh") {
			let n = this.#o.get(e.asset);
			if (n === void 0) throw new H(`${t}: missing animated mesh asset ${e.asset}`);
			it(n.asset.bounds, `${t}.asset.bounds`);
			return;
		}
		if (e.kind === "sprite") {
			if (e.sprite.size.some((e) => e > 16)) throw new H(`${t}.sprite.size: viewmodel dimensions must not exceed 16`);
			return;
		}
		e.meshPayload !== null && it(e.meshPayload.bounds, `${t}.asset.bounds`);
	}
	#I(e, t) {
		if (this.#e.has(e) || this.#t.has(e)) throw new H(`${t}: handle ${e} already exists`);
	}
	#L(e, t) {
		return e !== null && this.#R(e, t), e;
	}
	#R(e, t) {
		let n = this.#e.get(e);
		if (n === void 0) throw new H(`${t}: unknown handle ${e}`);
		return n;
	}
	#z(e, t) {
		let n = this.#t.get(e);
		if (n === void 0) throw new H(`${t}: unknown light handle ${e}`);
		return n;
	}
	#B(e, t) {
		let n = nt(this.#R(e, t));
		return this.#e.set(e, n), this.#l && (this.#c.copiedNodeRecords += 1), n;
	}
	#V(e, t) {
		let n = { ...this.#z(e, t) };
		return this.#t.set(e, n), this.#l && (this.#c.copiedLightRecords += 1), n;
	}
	#H(e) {
		let t = this.#a.get(e);
		if (t === void 0) return;
		let n = { ...t };
		return this.#a.set(e, n), this.#l && (this.#c.copiedResourceRecords += 1), n;
	}
	#U(e) {
		let t = this.#o.get(e);
		if (t === void 0) return;
		let n = { ...t };
		return this.#o.set(e, n), this.#l && (this.#c.copiedResourceRecords += 1), n;
	}
	#W(e) {
		let t = this.#s.get(e);
		if (t === void 0) return;
		let n = { ...t };
		return this.#s.set(e, n), this.#l && (this.#c.copiedResourceRecords += 1), n;
	}
	#G() {
		let e = new et();
		return e.#e = new Map(this.#e), e.#t = new Map(this.#t), e.#n = new Map(this.#n), e.#r = new Map(this.#r), e.#i = new Map(this.#i), e.#a = new Map(this.#a), e.#o = new Map(this.#o), e.#s = new Map(this.#s), e.#c = {
			...st(),
			sharedDefinitionRecords: this.#n.size + this.#r.size + this.#i.size + this.#a.size + this.#o.size + this.#s.size
		}, e.#l = !0, e;
	}
	#K(e) {
		this.#e = e.#e, this.#t = e.#t, this.#n = e.#n, this.#r = e.#r, this.#i = e.#i, this.#a = e.#a, this.#o = e.#o, this.#s = e.#s, this.#c = e.#c, this.#l = !1;
	}
	#q(e) {
		let t = this.#G(), n = [];
		for (let r of e.ops) n.push(...t.applyDiff(r));
		return {
			staged: t,
			instructions: n
		};
	}
};
et = tt;
function nt(e) {
	let t = new Set(e.children);
	return e.kind === "staticMesh" ? {
		...e,
		children: t,
		materialParameters: new Map(e.materialParameters)
	} : {
		...e,
		children: t
	};
}
function rt(e, t) {
	if (e.translation.some((e) => Math.abs(e) > 16)) throw new H(`${t}: viewmodel translation components must be within +/−16`);
	if (e.rotation.some((e) => Math.abs(e) > 1)) throw new H(`${t}: viewmodel rotation components must be within +/−1`);
	if (e.scale.some((e) => Math.abs(e) > 64)) throw new H(`${t}: viewmodel scale components must be within +/−64`);
}
function it(e, t) {
	at([e.min, e.max], t);
}
function at(e, t) {
	if (e.some((e) => e.some((e) => Math.abs(e) > 16))) throw new H(`${t}: viewmodel asset coordinates must be within +/−16`);
}
function ot(e) {
	switch (e.kind) {
		case "primitive": return null;
		case "staticMesh": return `staticMesh:${e.asset}`;
		case "animatedMesh": return `animatedMesh:${e.asset}`;
		case "voxelObject": return `voxelObject:${e.asset}`;
		case "sprite": return `sprite:${e.sprite.asset}`;
	}
}
function st() {
	return {
		copiedNodeRecords: 0,
		copiedLightRecords: 0,
		copiedResourceRecords: 0,
		sharedDefinitionRecords: 0
	};
}
function ct(e) {
	return {
		handle: e.handle,
		parent: e.parent,
		light: U(e.light)
	};
}
function lt(e) {
	let t = {
		handle: e.handle,
		parent: e.parent,
		children: [...e.children].sort(Ot),
		layer: e.layer,
		transform: U(e.transform),
		visible: e.visible,
		metadata: U(e.metadata),
		material: U(e.material),
		meshPayload: U(e.meshPayload)
	};
	return e.kind === "primitive" ? {
		...t,
		kind: "primitive",
		node: U(e.node)
	} : e.kind === "staticMesh" ? {
		...t,
		kind: "staticMesh",
		asset: e.asset,
		instance: U(e.instance),
		materialParameters: [...e.materialParameters.entries()].sort(([e], [t]) => e - t).map(([e, t]) => ({
			slot: e,
			parameters: U(t)
		}))
	} : e.kind === "animatedMesh" ? {
		...t,
		kind: "animatedMesh",
		asset: e.asset,
		instance: U(e.instance),
		playback: U(e.playback)
	} : e.kind === "voxelObject" ? {
		...t,
		kind: "voxelObject",
		asset: e.asset,
		instance: U(e.instance),
		frame: e.frame
	} : {
		...t,
		kind: "sprite",
		sprite: U(e.sprite),
		frameUv: U(e.frameUv),
		frameSize: U(e.frameSize),
		renderOrder: e.renderOrder
	};
}
function ut(e, t) {
	if (e.asset.length === 0) throw new H(`${t}.asset must be non-empty`);
	if (e.runtimeFormat !== "glb") throw new H(`${t}.runtimeFormat unsupported: ${e.runtimeFormat}`);
	let n = /* @__PURE__ */ new Set();
	for (let r = 0; r < e.clips.length; r += 1) {
		let i = e.clips[r];
		if (i.id.length === 0) throw new H(`${t}.clips[${r}].id must be non-empty`);
		if (n.has(i.id)) throw new H(`${t}.clips duplicate clip ${i.id}`);
		n.add(i.id);
	}
	if (e.defaultClip !== null && !n.has(e.defaultClip)) throw new H(`${t}.defaultClip ${e.defaultClip} is not declared`);
	let r = /* @__PURE__ */ new Set();
	for (let n = 0; n < e.materialSlots.length; n += 1) {
		let i = Tt(e.materialSlots[n].slot, `${t}.materialSlots[${n}].slot`);
		if (r.has(i)) throw new H(`${t}.materialSlots duplicate slot ${i}`);
		r.add(i);
	}
}
function dt(e, t) {
	if (e.asset.length === 0 || e.contentHash.length === 0) throw new H(`${t} asset and contentHash must be non-empty`);
	if (e.meshes.length === 0 || e.meshes.length > 8193) throw new H(`${t}.meshes must contain 1..=8193 entries`);
	if (e.frames.length === 0 || e.frames.length > 8193) throw new H(`${t}.frames must contain 1..=8193 entries`);
	let n = /* @__PURE__ */ new Set();
	e.materialSlots.forEach((e, r) => {
		let i = Tt(e.slot, `${t}.materialSlots[${r}].slot`);
		if (n.has(i)) throw new H(`${t}.materialSlots duplicate slot ${i}`);
		n.add(i);
	});
	let r = 0, i = 0;
	if (e.meshes.forEach((e, a) => {
		bt(e.payload, `${t}.meshes[${a}].payload`), r += e.payload.layout.vertexCount, i += e.payload.layout.indexCount, e.payload.groups.forEach((e, r) => {
			if (!n.has(e.materialSlot)) throw new H(`${t}.meshes[${a}].payload.groups[${r}] uses unbound slot ${e.materialSlot}`);
		});
	}), r > 8e6 || i > 12e6) throw new H(`${t}.meshes exceeds aggregate vertex/index work limits`);
	let a = /* @__PURE__ */ new Set();
	e.frames.forEach((n, r) => {
		if (n.id.length === 0 || a.has(n.id)) throw new H(`${t}.frames[${r}].id must be non-empty and unique`);
		a.add(n.id), ft(e, r, `${t}.frames[${r}]`);
	});
}
function ft(e, t, n) {
	let r = Tt(t, n), i = e.frames[r];
	if (i === void 0 || e.meshes[i.mesh] === void 0) throw new H(`${n} ${r} is outside voxel object ${e.asset} frame resources`);
}
function pt(e, t, n) {
	let r = new Set(e.materialSlots.map((e) => e.slot)), i = /* @__PURE__ */ new Set();
	t.forEach((e, t) => {
		if (i.has(e.slot)) throw new H(`${n}[${t}] duplicates slot ${e.slot}`);
		if (!r.has(e.slot)) throw new H(`${n}[${t}] uses unbound slot ${e.slot}`);
		i.add(e.slot);
	});
}
function mt(e, t, n) {
	if (t.kind === "play") {
		if (!e.clips.some((e) => e.id === t.clip)) throw new H(`${n}.clip ${t.clip} is not defined on ${e.asset}`);
		if (t.speed <= 0) throw new H(`${n}.speed must be positive`);
		if (t.weight < 0 || t.weight > 1) throw new H(`${n}.weight must be in 0..=1`);
	}
}
function ht(e, t) {
	if (gt(e.color, `${t}.color`), yt(e.intensity, `${t}.intensity`), e.intensity > 1e4) throw new H(`${t}.intensity must not exceed ${String(n)}`);
	if (e.kind === "directional") {
		_t(e.direction, `${t}.direction`);
		return;
	}
	if (e.kind === "point" || e.kind === "spot") {
		if (e.position.forEach((e, n) => vt(e, `${t}.position[${n}]`)), e.range !== null && (!Number.isFinite(e.range) || e.range <= 0)) throw new H(`${t}.range must be null or finite and positive`);
		yt(e.decay, `${t}.decay`);
	}
	if (e.kind === "spot") {
		if (_t(e.direction, `${t}.direction`), !Number.isFinite(e.outerAngleRadians) || e.outerAngleRadians <= 0 || e.outerAngleRadians > Math.PI / 2) throw new H(`${t}.outerAngleRadians must be in (0, pi/2]`);
		if (!Number.isFinite(e.penumbra) || e.penumbra < 0 || e.penumbra > 1) throw new H(`${t}.penumbra must be in 0..=1`);
	}
}
function gt(e, t) {
	e.forEach((e, n) => {
		if (!Number.isFinite(e) || e < 0 || e > 1) throw new H(`${t}[${n}] must be finite and in 0..=1`);
	});
}
function _t(e, t) {
	if (e.forEach((e, n) => vt(e, `${t}[${n}]`)), e.reduce((e, t) => e + t * t, 0) <= 2 ** -52) throw new H(`${t} must be non-zero`);
}
function vt(e, t) {
	if (!Number.isFinite(e)) throw new H(`${t} must be finite`);
}
function yt(e, t) {
	if (!Number.isFinite(e) || e < 0) throw new H(`${t} must be finite and non-negative`);
}
function bt(e, t) {
	let n = Tt(e.layout.vertexCount, `${t}.layout.vertexCount`), r = Tt(e.layout.indexCount, `${t}.layout.indexCount`), i = Ct(e, "position", t), a = Ct(e, "normal", t), o = e.layout.attributes.find((e) => e.name === "uv") !== void 0;
	if (e.source.kind === "inline") {
		if (wt(e.source.positions, n * i, `${t}.source.positions`), wt(e.source.normals, n * a, `${t}.source.normals`), o !== (e.source.uvs !== void 0)) throw new H(`${t}.source.uvs must match the declared uv attribute`);
		if (e.source.uvs !== void 0 && (wt(e.source.uvs, n * 2, `${t}.source.uvs`), e.source.uvs.forEach((e, n) => vt(e, `${t}.source.uvs[${n}]`)), (e.provenance === "voxelChunk" || e.provenance === "voxelObject") && e.source.uvs.some((e) => Math.abs(e) > 16777216))) throw new H(`${t}.source.uvs exceeds the voxel tile-coordinate range`);
		wt(e.source.indices, r, `${t}.source.indices`), e.source.indices.forEach((e, r) => {
			let i = Tt(e, `${t}.source.indices[${r}]`);
			if (i >= n) throw new H(`${t}.source.indices[${r}] ${i} is out of range for ${n} vertices`);
		});
	} else if (e.source.kind === "sharedBuffer") {
		if (Tt(e.source.buffer, `${t}.source.buffer`), Tt(e.source.positionsByteOffset, `${t}.source.positionsByteOffset`), Tt(e.source.normalsByteOffset, `${t}.source.normalsByteOffset`), o !== (e.source.uvsByteOffset !== void 0)) throw new H(`${t}.source.uvsByteOffset must match the declared uv attribute`);
		e.source.uvsByteOffset !== void 0 && Tt(e.source.uvsByteOffset, `${t}.source.uvsByteOffset`), Tt(e.source.indicesByteOffset, `${t}.source.indicesByteOffset`);
	} else {
		let s = /^sha256:([0-9a-f]{64})$/u.exec(e.source.contentHash)?.[1];
		if (s === void 0 || e.source.resource !== `mesh-resource/${s}`) throw new H(`${t}.source has an invalid content-addressed identity`);
		let c = Tt(e.source.byteLength, `${t}.source.byteLength`);
		if (c < 16 || c > 64 * 1024 * 1024) throw new H(`${t}.source.byteLength exceeds the resource bounds`);
		let l = Tt(e.source.positionsByteOffset, `${t}.source.positionsByteOffset`), u = Tt(e.source.normalsByteOffset, `${t}.source.normalsByteOffset`), d = e.source.uvsByteOffset === void 0 ? void 0 : Tt(e.source.uvsByteOffset, `${t}.source.uvsByteOffset`);
		if (o !== (d !== void 0) || e.source.encoding === "packedStreamsLeV1" && d !== void 0 || e.source.encoding === "packedStreamsLeV2" && d === void 0) throw new H(`${t}.source encoding and uv stream must agree`);
		let f = Tt(e.source.indicesByteOffset, `${t}.source.indicesByteOffset`);
		if ([
			l,
			u,
			d,
			f
		].filter((e) => e !== void 0).some((e) => e < 16 || e % 4 != 0)) throw new H(`${t}.source offsets must be aligned after the header`);
		let p = l + n * i * 4, m = u + n * a * 4, h = d === void 0 ? m : d + n * 2 * 4, g = f + r * 4;
		if (p > c || m > c || h > c || g > c || p > u || (d === void 0 ? m : h) > f || d !== void 0 && m > d) throw new H(`${t}.source streams exceed or overlap the resource`);
	}
	for (let n = 0; n < e.groups.length; n += 1) {
		let i = e.groups[n], a = Tt(i.start, `${t}.groups[${n}].start`), o = Tt(i.count, `${t}.groups[${n}].count`);
		if (Tt(i.materialSlot, `${t}.groups[${n}].materialSlot`), a + o > r) throw new H(`${t}.groups[${n}] window [${a}, ${a + o}) exceeds indexCount ${r}`);
		let s = n === 0 ? 0 : e.groups[n - 1].start + e.groups[n - 1].count;
		if (a !== s) throw new H(`${t}.groups[${n}] starts at ${a}; contiguous coverage requires ${s}`);
	}
	if (e.groups.length > 0) {
		let n = e.groups[e.groups.length - 1];
		if (n.start + n.count !== r) throw new H(`${t}.groups must cover all ${r} indices`);
	}
}
function xt(e) {
	switch (e.op) {
		case "create":
		case "createLight":
		case "createStaticMeshInstance":
		case "createAnimatedMeshInstance":
		case "createVoxelObjectInstance":
		case "createSprite":
			St(e.handle, `${e.op}.handle`), e.parent !== null && St(e.parent, `${e.op}.parent`);
			return;
		case "update":
		case "destroy":
		case "replaceMeshPayload":
		case "updateLight":
		case "setMaterialInstanceParameters":
		case "setAnimatedMeshPlayback":
		case "setVoxelObjectFrame":
		case "updateSprite":
			St(e.handle, `${e.op}.handle`);
			return;
		case "defineMaterial":
		case "defineTexture":
		case "defineSpriteAtlas":
		case "defineStaticMesh":
		case "defineAnimatedMesh":
		case "defineVoxelObject":
		case "releaseVoxelObject": return;
	}
}
function St(e, t) {
	if (!Number.isSafeInteger(e) || e < 0) throw new H(`${t} must be a non-negative JSON-safe integer`);
}
function Ct(e, t, n) {
	let r = e.layout.attributes.find((e) => e.name === t);
	if (r === void 0) throw new H(`${n}.layout.attributes missing ${t}`);
	return Tt(r.components, `${n}.layout.attributes.${t}.components`);
}
function wt(e, t, n) {
	if (e.length !== t) throw new H(`${n} expected length ${t}, got ${e.length}`);
}
function Tt(e, t) {
	if (!Number.isInteger(e) || e < 0) throw new H(`${t} must be a non-negative integer`);
	return e;
}
function Et(e) {
	return [...e.keys()].sort(Ot);
}
function Dt(e) {
	return [...e.values()].map((e) => U(e)).sort((e, t) => e.id.localeCompare(t.id));
}
function Ot(e, t) {
	return e - t;
}
function U(e) {
	return e === void 0 ? e : JSON.parse(JSON.stringify(e));
}
var kt = "attached", At = 1e3, jt = 1001, Mt = 1002, Nt = 1003, Pt = 1004, Ft = 1005, It = 1006, Lt = 1007, Rt = 1008, zt = 1009, Bt = 1010, Vt = 1011, Ht = 1012, Ut = 1013, Wt = 1014, Gt = 1015, Kt = 1016, qt = 1017, Jt = 1018, Yt = 1020, Xt = 35902, Zt = 35899, Qt = 1021, $t = 1022, en = 1023, tn = 1026, nn = 1027, rn = 1028, an = 1029, on = 1030, sn = 1031, cn = 1033, ln = 33776, un = 33777, dn = 33778, fn = 33779, pn = 35840, mn = 35841, hn = 35842, gn = 35843, _n = 36196, vn = 37492, yn = 37496, bn = 37488, xn = 37489, Sn = 37490, Cn = 37491, wn = 37808, Tn = 37809, En = 37810, Dn = 37811, On = 37812, kn = 37813, An = 37814, jn = 37815, Mn = 37816, Nn = 37817, Pn = 37818, Fn = 37819, In = 37820, Ln = 37821, Rn = 36492, zn = 36494, Bn = 36495, Vn = 36283, Hn = 36284, Un = 36285, Wn = 36286, Gn = 2200, Kn = 2201, qn = 2202, Jn = 2300, Yn = 2301, Xn = 2302, Zn = 2303, Qn = 2400, $n = 2401, er = 2402, tr = 2500, nr = 2501, rr = 3200, ir = "srgb", ar = "srgb-linear", or = "linear", sr = "srgb", cr = 7680, lr = 35044, ur = 35048, dr = 2e3;
function fr(e) {
	for (let t = e.length - 1; t >= 0; --t) if (e[t] >= 65535) return !0;
	return !1;
}
function pr(e) {
	return ArrayBuffer.isView(e) && !(e instanceof DataView);
}
function mr(e) {
	return document.createElementNS("http://www.w3.org/1999/xhtml", e);
}
function hr() {
	let e = mr("canvas");
	return e.style.display = "block", e;
}
var gr = {};
function _r(...e) {
	let t = "THREE." + e.shift();
	console.log(t, ...e);
}
function vr(e) {
	let t = e[0];
	if (typeof t == "string" && t.startsWith("TSL:")) {
		let t = e[1];
		t && t.isStackTrace ? e[0] += " " + t.getLocation() : e[1] = "Stack trace not available. Enable \"THREE.Node.captureStackTrace\" to capture stack traces.";
	}
	return e;
}
function W(...e) {
	e = vr(e);
	let t = "THREE." + e.shift();
	{
		let n = e[0];
		n && n.isStackTrace ? console.warn(n.getError(t)) : console.warn(t, ...e);
	}
}
function G(...e) {
	e = vr(e);
	let t = "THREE." + e.shift();
	{
		let n = e[0];
		n && n.isStackTrace ? console.error(n.getError(t)) : console.error(t, ...e);
	}
}
function yr(...e) {
	let t = e.join(" ");
	t in gr || (gr[t] = !0, W(...e));
}
function br(e, t, n) {
	return new Promise(function(r, i) {
		function a() {
			switch (e.clientWaitSync(t, e.SYNC_FLUSH_COMMANDS_BIT, 0)) {
				case e.WAIT_FAILED:
					i();
					break;
				case e.TIMEOUT_EXPIRED:
					setTimeout(a, n);
					break;
				default: r();
			}
		}
		setTimeout(a, n);
	});
}
var xr = {
	0: 1,
	2: 6,
	4: 7,
	3: 5,
	1: 0,
	6: 2,
	7: 4,
	5: 3
}, Sr = class {
	addEventListener(e, t) {
		this._listeners === void 0 && (this._listeners = {});
		let n = this._listeners;
		n[e] === void 0 && (n[e] = []), n[e].indexOf(t) === -1 && n[e].push(t);
	}
	hasEventListener(e, t) {
		let n = this._listeners;
		return n !== void 0 && n[e] !== void 0 && n[e].indexOf(t) !== -1;
	}
	removeEventListener(e, t) {
		let n = this._listeners;
		if (n === void 0) return;
		let r = n[e];
		if (r !== void 0) {
			let e = r.indexOf(t);
			e !== -1 && r.splice(e, 1);
		}
	}
	dispatchEvent(e) {
		let t = this._listeners;
		if (t === void 0) return;
		let n = t[e.type];
		if (n !== void 0) {
			e.target = this;
			let t = n.slice(0);
			for (let n = 0, r = t.length; n < r; n++) t[n].call(this, e);
			e.target = null;
		}
	}
}, Cr = /* @__PURE__ */ "00.01.02.03.04.05.06.07.08.09.0a.0b.0c.0d.0e.0f.10.11.12.13.14.15.16.17.18.19.1a.1b.1c.1d.1e.1f.20.21.22.23.24.25.26.27.28.29.2a.2b.2c.2d.2e.2f.30.31.32.33.34.35.36.37.38.39.3a.3b.3c.3d.3e.3f.40.41.42.43.44.45.46.47.48.49.4a.4b.4c.4d.4e.4f.50.51.52.53.54.55.56.57.58.59.5a.5b.5c.5d.5e.5f.60.61.62.63.64.65.66.67.68.69.6a.6b.6c.6d.6e.6f.70.71.72.73.74.75.76.77.78.79.7a.7b.7c.7d.7e.7f.80.81.82.83.84.85.86.87.88.89.8a.8b.8c.8d.8e.8f.90.91.92.93.94.95.96.97.98.99.9a.9b.9c.9d.9e.9f.a0.a1.a2.a3.a4.a5.a6.a7.a8.a9.aa.ab.ac.ad.ae.af.b0.b1.b2.b3.b4.b5.b6.b7.b8.b9.ba.bb.bc.bd.be.bf.c0.c1.c2.c3.c4.c5.c6.c7.c8.c9.ca.cb.cc.cd.ce.cf.d0.d1.d2.d3.d4.d5.d6.d7.d8.d9.da.db.dc.dd.de.df.e0.e1.e2.e3.e4.e5.e6.e7.e8.e9.ea.eb.ec.ed.ee.ef.f0.f1.f2.f3.f4.f5.f6.f7.f8.f9.fa.fb.fc.fd.fe.ff".split("."), wr = 1234567, Tr = Math.PI / 180, Er = 180 / Math.PI;
function Dr() {
	let e = Math.random() * 4294967295 | 0, t = Math.random() * 4294967295 | 0, n = Math.random() * 4294967295 | 0, r = Math.random() * 4294967295 | 0;
	return (Cr[e & 255] + Cr[e >> 8 & 255] + Cr[e >> 16 & 255] + Cr[e >> 24 & 255] + "-" + Cr[t & 255] + Cr[t >> 8 & 255] + "-" + Cr[t >> 16 & 15 | 64] + Cr[t >> 24 & 255] + "-" + Cr[n & 63 | 128] + Cr[n >> 8 & 255] + "-" + Cr[n >> 16 & 255] + Cr[n >> 24 & 255] + Cr[r & 255] + Cr[r >> 8 & 255] + Cr[r >> 16 & 255] + Cr[r >> 24 & 255]).toLowerCase();
}
function K(e, t, n) {
	return Math.max(t, Math.min(n, e));
}
function Or(e, t) {
	return (e % t + t) % t;
}
function kr(e, t, n, r, i) {
	return r + (e - t) * (i - r) / (n - t);
}
function Ar(e, t, n) {
	return e === t ? 0 : (n - e) / (t - e);
}
function jr(e, t, n) {
	return (1 - n) * e + n * t;
}
function Mr(e, t, n, r) {
	return jr(e, t, 1 - Math.exp(-n * r));
}
function Nr(e, t = 1) {
	return t - Math.abs(Or(e, t * 2) - t);
}
function Pr(e, t, n) {
	return e <= t ? 0 : e >= n ? 1 : (e = (e - t) / (n - t), e * e * (3 - 2 * e));
}
function Fr(e, t, n) {
	return e <= t ? 0 : e >= n ? 1 : (e = (e - t) / (n - t), e * e * e * (e * (e * 6 - 15) + 10));
}
function Ir(e, t) {
	return e + Math.floor(Math.random() * (t - e + 1));
}
function Lr(e, t) {
	return e + Math.random() * (t - e);
}
function Rr(e) {
	return e * (.5 - Math.random());
}
function zr(e) {
	e !== void 0 && (wr = e);
	let t = wr += 1831565813;
	return t = Math.imul(t ^ t >>> 15, t | 1), t ^= t + Math.imul(t ^ t >>> 7, t | 61), ((t ^ t >>> 14) >>> 0) / 4294967296;
}
function Br(e) {
	return e * Tr;
}
function Vr(e) {
	return e * Er;
}
function Hr(e) {
	return (e & e - 1) == 0 && e !== 0;
}
function Ur(e) {
	return 2 ** Math.ceil(Math.log(e) / Math.LN2);
}
function Wr(e) {
	return 2 ** Math.floor(Math.log(e) / Math.LN2);
}
function Gr(e, t, n, r, i) {
	let a = Math.cos, o = Math.sin, s = a(n / 2), c = o(n / 2), l = a((t + r) / 2), u = o((t + r) / 2), d = a((t - r) / 2), f = o((t - r) / 2), p = a((r - t) / 2), m = o((r - t) / 2);
	switch (i) {
		case "XYX":
			e.set(s * u, c * d, c * f, s * l);
			break;
		case "YZY":
			e.set(c * f, s * u, c * d, s * l);
			break;
		case "ZXZ":
			e.set(c * d, c * f, s * u, s * l);
			break;
		case "XZX":
			e.set(s * u, c * m, c * p, s * l);
			break;
		case "YXY":
			e.set(c * p, s * u, c * m, s * l);
			break;
		case "ZYZ":
			e.set(c * m, c * p, s * u, s * l);
			break;
		default: W("MathUtils: .setQuaternionFromProperEuler() encountered an unknown order: " + i);
	}
}
function Kr(e, t) {
	switch (t.constructor) {
		case Float32Array: return e;
		case Uint32Array: return e / 4294967295;
		case Uint16Array: return e / 65535;
		case Uint8Array: return e / 255;
		case Int32Array: return Math.max(e / 2147483647, -1);
		case Int16Array: return Math.max(e / 32767, -1);
		case Int8Array: return Math.max(e / 127, -1);
		default: throw Error("Invalid component type.");
	}
}
function qr(e, t) {
	switch (t.constructor) {
		case Float32Array: return e;
		case Uint32Array: return Math.round(e * 4294967295);
		case Uint16Array: return Math.round(e * 65535);
		case Uint8Array: return Math.round(e * 255);
		case Int32Array: return Math.round(e * 2147483647);
		case Int16Array: return Math.round(e * 32767);
		case Int8Array: return Math.round(e * 127);
		default: throw Error("Invalid component type.");
	}
}
var Jr = {
	DEG2RAD: Tr,
	RAD2DEG: Er,
	generateUUID: Dr,
	clamp: K,
	euclideanModulo: Or,
	mapLinear: kr,
	inverseLerp: Ar,
	lerp: jr,
	damp: Mr,
	pingpong: Nr,
	smoothstep: Pr,
	smootherstep: Fr,
	randInt: Ir,
	randFloat: Lr,
	randFloatSpread: Rr,
	seededRandom: zr,
	degToRad: Br,
	radToDeg: Vr,
	isPowerOfTwo: Hr,
	ceilPowerOfTwo: Ur,
	floorPowerOfTwo: Wr,
	setQuaternionFromProperEuler: Gr,
	normalize: qr,
	denormalize: Kr
}, Yr = class e {
	static {
		e.prototype.isVector2 = !0;
	}
	constructor(e = 0, t = 0) {
		this.x = e, this.y = t;
	}
	get width() {
		return this.x;
	}
	set width(e) {
		this.x = e;
	}
	get height() {
		return this.y;
	}
	set height(e) {
		this.y = e;
	}
	set(e, t) {
		return this.x = e, this.y = t, this;
	}
	setScalar(e) {
		return this.x = e, this.y = e, this;
	}
	setX(e) {
		return this.x = e, this;
	}
	setY(e) {
		return this.y = e, this;
	}
	setComponent(e, t) {
		switch (e) {
			case 0:
				this.x = t;
				break;
			case 1:
				this.y = t;
				break;
			default: throw Error("index is out of range: " + e);
		}
		return this;
	}
	getComponent(e) {
		switch (e) {
			case 0: return this.x;
			case 1: return this.y;
			default: throw Error("index is out of range: " + e);
		}
	}
	clone() {
		return new this.constructor(this.x, this.y);
	}
	copy(e) {
		return this.x = e.x, this.y = e.y, this;
	}
	add(e) {
		return this.x += e.x, this.y += e.y, this;
	}
	addScalar(e) {
		return this.x += e, this.y += e, this;
	}
	addVectors(e, t) {
		return this.x = e.x + t.x, this.y = e.y + t.y, this;
	}
	addScaledVector(e, t) {
		return this.x += e.x * t, this.y += e.y * t, this;
	}
	sub(e) {
		return this.x -= e.x, this.y -= e.y, this;
	}
	subScalar(e) {
		return this.x -= e, this.y -= e, this;
	}
	subVectors(e, t) {
		return this.x = e.x - t.x, this.y = e.y - t.y, this;
	}
	multiply(e) {
		return this.x *= e.x, this.y *= e.y, this;
	}
	multiplyScalar(e) {
		return this.x *= e, this.y *= e, this;
	}
	divide(e) {
		return this.x /= e.x, this.y /= e.y, this;
	}
	divideScalar(e) {
		return this.multiplyScalar(1 / e);
	}
	applyMatrix3(e) {
		let t = this.x, n = this.y, r = e.elements;
		return this.x = r[0] * t + r[3] * n + r[6], this.y = r[1] * t + r[4] * n + r[7], this;
	}
	min(e) {
		return this.x = Math.min(this.x, e.x), this.y = Math.min(this.y, e.y), this;
	}
	max(e) {
		return this.x = Math.max(this.x, e.x), this.y = Math.max(this.y, e.y), this;
	}
	clamp(e, t) {
		return this.x = K(this.x, e.x, t.x), this.y = K(this.y, e.y, t.y), this;
	}
	clampScalar(e, t) {
		return this.x = K(this.x, e, t), this.y = K(this.y, e, t), this;
	}
	clampLength(e, t) {
		let n = this.length();
		return this.divideScalar(n || 1).multiplyScalar(K(n, e, t));
	}
	floor() {
		return this.x = Math.floor(this.x), this.y = Math.floor(this.y), this;
	}
	ceil() {
		return this.x = Math.ceil(this.x), this.y = Math.ceil(this.y), this;
	}
	round() {
		return this.x = Math.round(this.x), this.y = Math.round(this.y), this;
	}
	roundToZero() {
		return this.x = Math.trunc(this.x), this.y = Math.trunc(this.y), this;
	}
	negate() {
		return this.x = -this.x, this.y = -this.y, this;
	}
	dot(e) {
		return this.x * e.x + this.y * e.y;
	}
	cross(e) {
		return this.x * e.y - this.y * e.x;
	}
	lengthSq() {
		return this.x * this.x + this.y * this.y;
	}
	length() {
		return Math.sqrt(this.x * this.x + this.y * this.y);
	}
	manhattanLength() {
		return Math.abs(this.x) + Math.abs(this.y);
	}
	normalize() {
		return this.divideScalar(this.length() || 1);
	}
	angle() {
		return Math.atan2(-this.y, -this.x) + Math.PI;
	}
	angleTo(e) {
		let t = Math.sqrt(this.lengthSq() * e.lengthSq());
		if (t === 0) return Math.PI / 2;
		let n = this.dot(e) / t;
		return Math.acos(K(n, -1, 1));
	}
	distanceTo(e) {
		return Math.sqrt(this.distanceToSquared(e));
	}
	distanceToSquared(e) {
		let t = this.x - e.x, n = this.y - e.y;
		return t * t + n * n;
	}
	manhattanDistanceTo(e) {
		return Math.abs(this.x - e.x) + Math.abs(this.y - e.y);
	}
	setLength(e) {
		return this.normalize().multiplyScalar(e);
	}
	lerp(e, t) {
		return this.x += (e.x - this.x) * t, this.y += (e.y - this.y) * t, this;
	}
	lerpVectors(e, t, n) {
		return this.x = e.x + (t.x - e.x) * n, this.y = e.y + (t.y - e.y) * n, this;
	}
	equals(e) {
		return e.x === this.x && e.y === this.y;
	}
	fromArray(e, t = 0) {
		return this.x = e[t], this.y = e[t + 1], this;
	}
	toArray(e = [], t = 0) {
		return e[t] = this.x, e[t + 1] = this.y, e;
	}
	fromBufferAttribute(e, t) {
		return this.x = e.getX(t), this.y = e.getY(t), this;
	}
	rotateAround(e, t) {
		let n = Math.cos(t), r = Math.sin(t), i = this.x - e.x, a = this.y - e.y;
		return this.x = i * n - a * r + e.x, this.y = i * r + a * n + e.y, this;
	}
	random() {
		return this.x = Math.random(), this.y = Math.random(), this;
	}
	*[Symbol.iterator]() {
		yield this.x, yield this.y;
	}
}, Xr = class {
	constructor(e = 0, t = 0, n = 0, r = 1) {
		this.isQuaternion = !0, this._x = e, this._y = t, this._z = n, this._w = r;
	}
	static slerpFlat(e, t, n, r, i, a, o) {
		let s = n[r + 0], c = n[r + 1], l = n[r + 2], u = n[r + 3], d = i[a + 0], f = i[a + 1], p = i[a + 2], m = i[a + 3];
		if (u !== m || s !== d || c !== f || l !== p) {
			let e = s * d + c * f + l * p + u * m;
			e < 0 && (d = -d, f = -f, p = -p, m = -m, e = -e);
			let t = 1 - o;
			if (e < .9995) {
				let n = Math.acos(e), r = Math.sin(n);
				t = Math.sin(t * n) / r, o = Math.sin(o * n) / r, s = s * t + d * o, c = c * t + f * o, l = l * t + p * o, u = u * t + m * o;
			} else {
				s = s * t + d * o, c = c * t + f * o, l = l * t + p * o, u = u * t + m * o;
				let e = 1 / Math.sqrt(s * s + c * c + l * l + u * u);
				s *= e, c *= e, l *= e, u *= e;
			}
		}
		e[t] = s, e[t + 1] = c, e[t + 2] = l, e[t + 3] = u;
	}
	static multiplyQuaternionsFlat(e, t, n, r, i, a) {
		let o = n[r], s = n[r + 1], c = n[r + 2], l = n[r + 3], u = i[a], d = i[a + 1], f = i[a + 2], p = i[a + 3];
		return e[t] = o * p + l * u + s * f - c * d, e[t + 1] = s * p + l * d + c * u - o * f, e[t + 2] = c * p + l * f + o * d - s * u, e[t + 3] = l * p - o * u - s * d - c * f, e;
	}
	get x() {
		return this._x;
	}
	set x(e) {
		this._x = e, this._onChangeCallback();
	}
	get y() {
		return this._y;
	}
	set y(e) {
		this._y = e, this._onChangeCallback();
	}
	get z() {
		return this._z;
	}
	set z(e) {
		this._z = e, this._onChangeCallback();
	}
	get w() {
		return this._w;
	}
	set w(e) {
		this._w = e, this._onChangeCallback();
	}
	set(e, t, n, r) {
		return this._x = e, this._y = t, this._z = n, this._w = r, this._onChangeCallback(), this;
	}
	clone() {
		return new this.constructor(this._x, this._y, this._z, this._w);
	}
	copy(e) {
		return this._x = e.x, this._y = e.y, this._z = e.z, this._w = e.w, this._onChangeCallback(), this;
	}
	setFromEuler(e, t = !0) {
		let n = e._x, r = e._y, i = e._z, a = e._order, o = Math.cos, s = Math.sin, c = o(n / 2), l = o(r / 2), u = o(i / 2), d = s(n / 2), f = s(r / 2), p = s(i / 2);
		switch (a) {
			case "XYZ":
				this._x = d * l * u + c * f * p, this._y = c * f * u - d * l * p, this._z = c * l * p + d * f * u, this._w = c * l * u - d * f * p;
				break;
			case "YXZ":
				this._x = d * l * u + c * f * p, this._y = c * f * u - d * l * p, this._z = c * l * p - d * f * u, this._w = c * l * u + d * f * p;
				break;
			case "ZXY":
				this._x = d * l * u - c * f * p, this._y = c * f * u + d * l * p, this._z = c * l * p + d * f * u, this._w = c * l * u - d * f * p;
				break;
			case "ZYX":
				this._x = d * l * u - c * f * p, this._y = c * f * u + d * l * p, this._z = c * l * p - d * f * u, this._w = c * l * u + d * f * p;
				break;
			case "YZX":
				this._x = d * l * u + c * f * p, this._y = c * f * u + d * l * p, this._z = c * l * p - d * f * u, this._w = c * l * u - d * f * p;
				break;
			case "XZY":
				this._x = d * l * u - c * f * p, this._y = c * f * u - d * l * p, this._z = c * l * p + d * f * u, this._w = c * l * u + d * f * p;
				break;
			default: W("Quaternion: .setFromEuler() encountered an unknown order: " + a);
		}
		return t === !0 && this._onChangeCallback(), this;
	}
	setFromAxisAngle(e, t) {
		let n = t / 2, r = Math.sin(n);
		return this._x = e.x * r, this._y = e.y * r, this._z = e.z * r, this._w = Math.cos(n), this._onChangeCallback(), this;
	}
	setFromRotationMatrix(e) {
		let t = e.elements, n = t[0], r = t[4], i = t[8], a = t[1], o = t[5], s = t[9], c = t[2], l = t[6], u = t[10], d = n + o + u;
		if (d > 0) {
			let e = .5 / Math.sqrt(d + 1);
			this._w = .25 / e, this._x = (l - s) * e, this._y = (i - c) * e, this._z = (a - r) * e;
		} else if (n > o && n > u) {
			let e = 2 * Math.sqrt(1 + n - o - u);
			this._w = (l - s) / e, this._x = .25 * e, this._y = (r + a) / e, this._z = (i + c) / e;
		} else if (o > u) {
			let e = 2 * Math.sqrt(1 + o - n - u);
			this._w = (i - c) / e, this._x = (r + a) / e, this._y = .25 * e, this._z = (s + l) / e;
		} else {
			let e = 2 * Math.sqrt(1 + u - n - o);
			this._w = (a - r) / e, this._x = (i + c) / e, this._y = (s + l) / e, this._z = .25 * e;
		}
		return this._onChangeCallback(), this;
	}
	setFromUnitVectors(e, t) {
		let n = e.dot(t) + 1;
		return n < 1e-8 ? (n = 0, Math.abs(e.x) > Math.abs(e.z) ? (this._x = -e.y, this._y = e.x, this._z = 0, this._w = n) : (this._x = 0, this._y = -e.z, this._z = e.y, this._w = n)) : (this._x = e.y * t.z - e.z * t.y, this._y = e.z * t.x - e.x * t.z, this._z = e.x * t.y - e.y * t.x, this._w = n), this.normalize();
	}
	angleTo(e) {
		return 2 * Math.acos(Math.abs(K(this.dot(e), -1, 1)));
	}
	rotateTowards(e, t) {
		let n = this.angleTo(e);
		if (n === 0) return this;
		let r = Math.min(1, t / n);
		return this.slerp(e, r), this;
	}
	identity() {
		return this.set(0, 0, 0, 1);
	}
	invert() {
		return this.conjugate();
	}
	conjugate() {
		return this._x *= -1, this._y *= -1, this._z *= -1, this._onChangeCallback(), this;
	}
	dot(e) {
		return this._x * e._x + this._y * e._y + this._z * e._z + this._w * e._w;
	}
	lengthSq() {
		return this._x * this._x + this._y * this._y + this._z * this._z + this._w * this._w;
	}
	length() {
		return Math.sqrt(this._x * this._x + this._y * this._y + this._z * this._z + this._w * this._w);
	}
	normalize() {
		let e = this.length();
		return e === 0 ? (this._x = 0, this._y = 0, this._z = 0, this._w = 1) : (e = 1 / e, this._x *= e, this._y *= e, this._z *= e, this._w *= e), this._onChangeCallback(), this;
	}
	multiply(e) {
		return this.multiplyQuaternions(this, e);
	}
	premultiply(e) {
		return this.multiplyQuaternions(e, this);
	}
	multiplyQuaternions(e, t) {
		let n = e._x, r = e._y, i = e._z, a = e._w, o = t._x, s = t._y, c = t._z, l = t._w;
		return this._x = n * l + a * o + r * c - i * s, this._y = r * l + a * s + i * o - n * c, this._z = i * l + a * c + n * s - r * o, this._w = a * l - n * o - r * s - i * c, this._onChangeCallback(), this;
	}
	slerp(e, t) {
		let n = e._x, r = e._y, i = e._z, a = e._w, o = this.dot(e);
		o < 0 && (n = -n, r = -r, i = -i, a = -a, o = -o);
		let s = 1 - t;
		if (o < .9995) {
			let e = Math.acos(o), c = Math.sin(e);
			s = Math.sin(s * e) / c, t = Math.sin(t * e) / c, this._x = this._x * s + n * t, this._y = this._y * s + r * t, this._z = this._z * s + i * t, this._w = this._w * s + a * t, this._onChangeCallback();
		} else this._x = this._x * s + n * t, this._y = this._y * s + r * t, this._z = this._z * s + i * t, this._w = this._w * s + a * t, this.normalize();
		return this;
	}
	slerpQuaternions(e, t, n) {
		return this.copy(e).slerp(t, n);
	}
	random() {
		let e = 2 * Math.PI * Math.random(), t = 2 * Math.PI * Math.random(), n = Math.random(), r = Math.sqrt(1 - n), i = Math.sqrt(n);
		return this.set(r * Math.sin(e), r * Math.cos(e), i * Math.sin(t), i * Math.cos(t));
	}
	equals(e) {
		return e._x === this._x && e._y === this._y && e._z === this._z && e._w === this._w;
	}
	fromArray(e, t = 0) {
		return this._x = e[t], this._y = e[t + 1], this._z = e[t + 2], this._w = e[t + 3], this._onChangeCallback(), this;
	}
	toArray(e = [], t = 0) {
		return e[t] = this._x, e[t + 1] = this._y, e[t + 2] = this._z, e[t + 3] = this._w, e;
	}
	fromBufferAttribute(e, t) {
		return this._x = e.getX(t), this._y = e.getY(t), this._z = e.getZ(t), this._w = e.getW(t), this._onChangeCallback(), this;
	}
	toJSON() {
		return this.toArray();
	}
	_onChange(e) {
		return this._onChangeCallback = e, this;
	}
	_onChangeCallback() {}
	*[Symbol.iterator]() {
		yield this._x, yield this._y, yield this._z, yield this._w;
	}
}, q = class e {
	static {
		e.prototype.isVector3 = !0;
	}
	constructor(e = 0, t = 0, n = 0) {
		this.x = e, this.y = t, this.z = n;
	}
	set(e, t, n) {
		return n === void 0 && (n = this.z), this.x = e, this.y = t, this.z = n, this;
	}
	setScalar(e) {
		return this.x = e, this.y = e, this.z = e, this;
	}
	setX(e) {
		return this.x = e, this;
	}
	setY(e) {
		return this.y = e, this;
	}
	setZ(e) {
		return this.z = e, this;
	}
	setComponent(e, t) {
		switch (e) {
			case 0:
				this.x = t;
				break;
			case 1:
				this.y = t;
				break;
			case 2:
				this.z = t;
				break;
			default: throw Error("index is out of range: " + e);
		}
		return this;
	}
	getComponent(e) {
		switch (e) {
			case 0: return this.x;
			case 1: return this.y;
			case 2: return this.z;
			default: throw Error("index is out of range: " + e);
		}
	}
	clone() {
		return new this.constructor(this.x, this.y, this.z);
	}
	copy(e) {
		return this.x = e.x, this.y = e.y, this.z = e.z, this;
	}
	add(e) {
		return this.x += e.x, this.y += e.y, this.z += e.z, this;
	}
	addScalar(e) {
		return this.x += e, this.y += e, this.z += e, this;
	}
	addVectors(e, t) {
		return this.x = e.x + t.x, this.y = e.y + t.y, this.z = e.z + t.z, this;
	}
	addScaledVector(e, t) {
		return this.x += e.x * t, this.y += e.y * t, this.z += e.z * t, this;
	}
	sub(e) {
		return this.x -= e.x, this.y -= e.y, this.z -= e.z, this;
	}
	subScalar(e) {
		return this.x -= e, this.y -= e, this.z -= e, this;
	}
	subVectors(e, t) {
		return this.x = e.x - t.x, this.y = e.y - t.y, this.z = e.z - t.z, this;
	}
	multiply(e) {
		return this.x *= e.x, this.y *= e.y, this.z *= e.z, this;
	}
	multiplyScalar(e) {
		return this.x *= e, this.y *= e, this.z *= e, this;
	}
	multiplyVectors(e, t) {
		return this.x = e.x * t.x, this.y = e.y * t.y, this.z = e.z * t.z, this;
	}
	applyEuler(e) {
		return this.applyQuaternion(Qr.setFromEuler(e));
	}
	applyAxisAngle(e, t) {
		return this.applyQuaternion(Qr.setFromAxisAngle(e, t));
	}
	applyMatrix3(e) {
		let t = this.x, n = this.y, r = this.z, i = e.elements;
		return this.x = i[0] * t + i[3] * n + i[6] * r, this.y = i[1] * t + i[4] * n + i[7] * r, this.z = i[2] * t + i[5] * n + i[8] * r, this;
	}
	applyNormalMatrix(e) {
		return this.applyMatrix3(e).normalize();
	}
	applyMatrix4(e) {
		let t = this.x, n = this.y, r = this.z, i = e.elements, a = 1 / (i[3] * t + i[7] * n + i[11] * r + i[15]);
		return this.x = (i[0] * t + i[4] * n + i[8] * r + i[12]) * a, this.y = (i[1] * t + i[5] * n + i[9] * r + i[13]) * a, this.z = (i[2] * t + i[6] * n + i[10] * r + i[14]) * a, this;
	}
	applyQuaternion(e) {
		let t = this.x, n = this.y, r = this.z, i = e.x, a = e.y, o = e.z, s = e.w, c = 2 * (a * r - o * n), l = 2 * (o * t - i * r), u = 2 * (i * n - a * t);
		return this.x = t + s * c + a * u - o * l, this.y = n + s * l + o * c - i * u, this.z = r + s * u + i * l - a * c, this;
	}
	project(e) {
		return this.applyMatrix4(e.matrixWorldInverse).applyMatrix4(e.projectionMatrix);
	}
	unproject(e) {
		return this.applyMatrix4(e.projectionMatrixInverse).applyMatrix4(e.matrixWorld);
	}
	transformDirection(e) {
		let t = this.x, n = this.y, r = this.z, i = e.elements;
		return this.x = i[0] * t + i[4] * n + i[8] * r, this.y = i[1] * t + i[5] * n + i[9] * r, this.z = i[2] * t + i[6] * n + i[10] * r, this.normalize();
	}
	divide(e) {
		return this.x /= e.x, this.y /= e.y, this.z /= e.z, this;
	}
	divideScalar(e) {
		return this.multiplyScalar(1 / e);
	}
	min(e) {
		return this.x = Math.min(this.x, e.x), this.y = Math.min(this.y, e.y), this.z = Math.min(this.z, e.z), this;
	}
	max(e) {
		return this.x = Math.max(this.x, e.x), this.y = Math.max(this.y, e.y), this.z = Math.max(this.z, e.z), this;
	}
	clamp(e, t) {
		return this.x = K(this.x, e.x, t.x), this.y = K(this.y, e.y, t.y), this.z = K(this.z, e.z, t.z), this;
	}
	clampScalar(e, t) {
		return this.x = K(this.x, e, t), this.y = K(this.y, e, t), this.z = K(this.z, e, t), this;
	}
	clampLength(e, t) {
		let n = this.length();
		return this.divideScalar(n || 1).multiplyScalar(K(n, e, t));
	}
	floor() {
		return this.x = Math.floor(this.x), this.y = Math.floor(this.y), this.z = Math.floor(this.z), this;
	}
	ceil() {
		return this.x = Math.ceil(this.x), this.y = Math.ceil(this.y), this.z = Math.ceil(this.z), this;
	}
	round() {
		return this.x = Math.round(this.x), this.y = Math.round(this.y), this.z = Math.round(this.z), this;
	}
	roundToZero() {
		return this.x = Math.trunc(this.x), this.y = Math.trunc(this.y), this.z = Math.trunc(this.z), this;
	}
	negate() {
		return this.x = -this.x, this.y = -this.y, this.z = -this.z, this;
	}
	dot(e) {
		return this.x * e.x + this.y * e.y + this.z * e.z;
	}
	lengthSq() {
		return this.x * this.x + this.y * this.y + this.z * this.z;
	}
	length() {
		return Math.sqrt(this.x * this.x + this.y * this.y + this.z * this.z);
	}
	manhattanLength() {
		return Math.abs(this.x) + Math.abs(this.y) + Math.abs(this.z);
	}
	normalize() {
		return this.divideScalar(this.length() || 1);
	}
	setLength(e) {
		return this.normalize().multiplyScalar(e);
	}
	lerp(e, t) {
		return this.x += (e.x - this.x) * t, this.y += (e.y - this.y) * t, this.z += (e.z - this.z) * t, this;
	}
	lerpVectors(e, t, n) {
		return this.x = e.x + (t.x - e.x) * n, this.y = e.y + (t.y - e.y) * n, this.z = e.z + (t.z - e.z) * n, this;
	}
	cross(e) {
		return this.crossVectors(this, e);
	}
	crossVectors(e, t) {
		let n = e.x, r = e.y, i = e.z, a = t.x, o = t.y, s = t.z;
		return this.x = r * s - i * o, this.y = i * a - n * s, this.z = n * o - r * a, this;
	}
	projectOnVector(e) {
		let t = e.lengthSq();
		if (t === 0) return this.set(0, 0, 0);
		let n = e.dot(this) / t;
		return this.copy(e).multiplyScalar(n);
	}
	projectOnPlane(e) {
		return Zr.copy(this).projectOnVector(e), this.sub(Zr);
	}
	reflect(e) {
		return this.sub(Zr.copy(e).multiplyScalar(2 * this.dot(e)));
	}
	angleTo(e) {
		let t = Math.sqrt(this.lengthSq() * e.lengthSq());
		if (t === 0) return Math.PI / 2;
		let n = this.dot(e) / t;
		return Math.acos(K(n, -1, 1));
	}
	distanceTo(e) {
		return Math.sqrt(this.distanceToSquared(e));
	}
	distanceToSquared(e) {
		let t = this.x - e.x, n = this.y - e.y, r = this.z - e.z;
		return t * t + n * n + r * r;
	}
	manhattanDistanceTo(e) {
		return Math.abs(this.x - e.x) + Math.abs(this.y - e.y) + Math.abs(this.z - e.z);
	}
	setFromSpherical(e) {
		return this.setFromSphericalCoords(e.radius, e.phi, e.theta);
	}
	setFromSphericalCoords(e, t, n) {
		let r = Math.sin(t) * e;
		return this.x = r * Math.sin(n), this.y = Math.cos(t) * e, this.z = r * Math.cos(n), this;
	}
	setFromCylindrical(e) {
		return this.setFromCylindricalCoords(e.radius, e.theta, e.y);
	}
	setFromCylindricalCoords(e, t, n) {
		return this.x = e * Math.sin(t), this.y = n, this.z = e * Math.cos(t), this;
	}
	setFromMatrixPosition(e) {
		let t = e.elements;
		return this.x = t[12], this.y = t[13], this.z = t[14], this;
	}
	setFromMatrixScale(e) {
		let t = this.setFromMatrixColumn(e, 0).length(), n = this.setFromMatrixColumn(e, 1).length(), r = this.setFromMatrixColumn(e, 2).length();
		return this.x = t, this.y = n, this.z = r, this;
	}
	setFromMatrixColumn(e, t) {
		return this.fromArray(e.elements, t * 4);
	}
	setFromMatrix3Column(e, t) {
		return this.fromArray(e.elements, t * 3);
	}
	setFromEuler(e) {
		return this.x = e._x, this.y = e._y, this.z = e._z, this;
	}
	setFromColor(e) {
		return this.x = e.r, this.y = e.g, this.z = e.b, this;
	}
	equals(e) {
		return e.x === this.x && e.y === this.y && e.z === this.z;
	}
	fromArray(e, t = 0) {
		return this.x = e[t], this.y = e[t + 1], this.z = e[t + 2], this;
	}
	toArray(e = [], t = 0) {
		return e[t] = this.x, e[t + 1] = this.y, e[t + 2] = this.z, e;
	}
	fromBufferAttribute(e, t) {
		return this.x = e.getX(t), this.y = e.getY(t), this.z = e.getZ(t), this;
	}
	random() {
		return this.x = Math.random(), this.y = Math.random(), this.z = Math.random(), this;
	}
	randomDirection() {
		let e = Math.random() * Math.PI * 2, t = Math.random() * 2 - 1, n = Math.sqrt(1 - t * t);
		return this.x = n * Math.cos(e), this.y = t, this.z = n * Math.sin(e), this;
	}
	*[Symbol.iterator]() {
		yield this.x, yield this.y, yield this.z;
	}
}, Zr = /*@__PURE__*/ new q(), Qr = /*@__PURE__*/ new Xr(), J = class e {
	static {
		e.prototype.isMatrix3 = !0;
	}
	constructor(e, t, n, r, i, a, o, s, c) {
		this.elements = [
			1,
			0,
			0,
			0,
			1,
			0,
			0,
			0,
			1
		], e !== void 0 && this.set(e, t, n, r, i, a, o, s, c);
	}
	set(e, t, n, r, i, a, o, s, c) {
		let l = this.elements;
		return l[0] = e, l[1] = r, l[2] = o, l[3] = t, l[4] = i, l[5] = s, l[6] = n, l[7] = a, l[8] = c, this;
	}
	identity() {
		return this.set(1, 0, 0, 0, 1, 0, 0, 0, 1), this;
	}
	copy(e) {
		let t = this.elements, n = e.elements;
		return t[0] = n[0], t[1] = n[1], t[2] = n[2], t[3] = n[3], t[4] = n[4], t[5] = n[5], t[6] = n[6], t[7] = n[7], t[8] = n[8], this;
	}
	extractBasis(e, t, n) {
		return e.setFromMatrix3Column(this, 0), t.setFromMatrix3Column(this, 1), n.setFromMatrix3Column(this, 2), this;
	}
	setFromMatrix4(e) {
		let t = e.elements;
		return this.set(t[0], t[4], t[8], t[1], t[5], t[9], t[2], t[6], t[10]), this;
	}
	multiply(e) {
		return this.multiplyMatrices(this, e);
	}
	premultiply(e) {
		return this.multiplyMatrices(e, this);
	}
	multiplyMatrices(e, t) {
		let n = e.elements, r = t.elements, i = this.elements, a = n[0], o = n[3], s = n[6], c = n[1], l = n[4], u = n[7], d = n[2], f = n[5], p = n[8], m = r[0], h = r[3], g = r[6], _ = r[1], v = r[4], y = r[7], b = r[2], x = r[5], S = r[8];
		return i[0] = a * m + o * _ + s * b, i[3] = a * h + o * v + s * x, i[6] = a * g + o * y + s * S, i[1] = c * m + l * _ + u * b, i[4] = c * h + l * v + u * x, i[7] = c * g + l * y + u * S, i[2] = d * m + f * _ + p * b, i[5] = d * h + f * v + p * x, i[8] = d * g + f * y + p * S, this;
	}
	multiplyScalar(e) {
		let t = this.elements;
		return t[0] *= e, t[3] *= e, t[6] *= e, t[1] *= e, t[4] *= e, t[7] *= e, t[2] *= e, t[5] *= e, t[8] *= e, this;
	}
	determinant() {
		let e = this.elements, t = e[0], n = e[1], r = e[2], i = e[3], a = e[4], o = e[5], s = e[6], c = e[7], l = e[8];
		return t * a * l - t * o * c - n * i * l + n * o * s + r * i * c - r * a * s;
	}
	invert() {
		let e = this.elements, t = e[0], n = e[1], r = e[2], i = e[3], a = e[4], o = e[5], s = e[6], c = e[7], l = e[8], u = l * a - o * c, d = o * s - l * i, f = c * i - a * s, p = t * u + n * d + r * f;
		if (p === 0) return this.set(0, 0, 0, 0, 0, 0, 0, 0, 0);
		let m = 1 / p;
		return e[0] = u * m, e[1] = (r * c - l * n) * m, e[2] = (o * n - r * a) * m, e[3] = d * m, e[4] = (l * t - r * s) * m, e[5] = (r * i - o * t) * m, e[6] = f * m, e[7] = (n * s - c * t) * m, e[8] = (a * t - n * i) * m, this;
	}
	transpose() {
		let e, t = this.elements;
		return e = t[1], t[1] = t[3], t[3] = e, e = t[2], t[2] = t[6], t[6] = e, e = t[5], t[5] = t[7], t[7] = e, this;
	}
	getNormalMatrix(e) {
		return this.setFromMatrix4(e).invert().transpose();
	}
	transposeIntoArray(e) {
		let t = this.elements;
		return e[0] = t[0], e[1] = t[3], e[2] = t[6], e[3] = t[1], e[4] = t[4], e[5] = t[7], e[6] = t[2], e[7] = t[5], e[8] = t[8], this;
	}
	setUvTransform(e, t, n, r, i, a, o) {
		let s = Math.cos(i), c = Math.sin(i);
		return this.set(n * s, n * c, -n * (s * a + c * o) + a + e, -r * c, r * s, -r * (-c * a + s * o) + o + t, 0, 0, 1), this;
	}
	scale(e, t) {
		return this.premultiply($r.makeScale(e, t)), this;
	}
	rotate(e) {
		return this.premultiply($r.makeRotation(-e)), this;
	}
	translate(e, t) {
		return this.premultiply($r.makeTranslation(e, t)), this;
	}
	makeTranslation(e, t) {
		return e.isVector2 ? this.set(1, 0, e.x, 0, 1, e.y, 0, 0, 1) : this.set(1, 0, e, 0, 1, t, 0, 0, 1), this;
	}
	makeRotation(e) {
		let t = Math.cos(e), n = Math.sin(e);
		return this.set(t, -n, 0, n, t, 0, 0, 0, 1), this;
	}
	makeScale(e, t) {
		return this.set(e, 0, 0, 0, t, 0, 0, 0, 1), this;
	}
	equals(e) {
		let t = this.elements, n = e.elements;
		for (let e = 0; e < 9; e++) if (t[e] !== n[e]) return !1;
		return !0;
	}
	fromArray(e, t = 0) {
		for (let n = 0; n < 9; n++) this.elements[n] = e[n + t];
		return this;
	}
	toArray(e = [], t = 0) {
		let n = this.elements;
		return e[t] = n[0], e[t + 1] = n[1], e[t + 2] = n[2], e[t + 3] = n[3], e[t + 4] = n[4], e[t + 5] = n[5], e[t + 6] = n[6], e[t + 7] = n[7], e[t + 8] = n[8], e;
	}
	clone() {
		return new this.constructor().fromArray(this.elements);
	}
}, $r = /*@__PURE__*/ new J(), ei = /*@__PURE__*/ new J().set(.4123908, .3575843, .1804808, .212639, .7151687, .0721923, .0193308, .1191948, .9505322), ti = /*@__PURE__*/ new J().set(3.2409699, -1.5373832, -.4986108, -.9692436, 1.8759675, .0415551, .0556301, -.203977, 1.0569715);
function ni() {
	let e = {
		enabled: !0,
		workingColorSpace: ar,
		spaces: {},
		convert: function(e, t, n) {
			return this.enabled === !1 || t === n || !t || !n ? e : (this.spaces[t].transfer === "srgb" && (e.r = ri(e.r), e.g = ri(e.g), e.b = ri(e.b)), this.spaces[t].primaries !== this.spaces[n].primaries && (e.applyMatrix3(this.spaces[t].toXYZ), e.applyMatrix3(this.spaces[n].fromXYZ)), this.spaces[n].transfer === "srgb" && (e.r = ii(e.r), e.g = ii(e.g), e.b = ii(e.b)), e);
		},
		workingToColorSpace: function(e, t) {
			return this.convert(e, this.workingColorSpace, t);
		},
		colorSpaceToWorking: function(e, t) {
			return this.convert(e, t, this.workingColorSpace);
		},
		getPrimaries: function(e) {
			return this.spaces[e].primaries;
		},
		getTransfer: function(e) {
			return e === "" ? or : this.spaces[e].transfer;
		},
		getToneMappingMode: function(e) {
			return this.spaces[e].outputColorSpaceConfig.toneMappingMode || "standard";
		},
		getLuminanceCoefficients: function(e, t = this.workingColorSpace) {
			return e.fromArray(this.spaces[t].luminanceCoefficients);
		},
		define: function(e) {
			Object.assign(this.spaces, e);
		},
		_getMatrix: function(e, t, n) {
			return e.copy(this.spaces[t].toXYZ).multiply(this.spaces[n].fromXYZ);
		},
		_getDrawingBufferColorSpace: function(e) {
			return this.spaces[e].outputColorSpaceConfig.drawingBufferColorSpace;
		},
		_getUnpackColorSpace: function(e = this.workingColorSpace) {
			return this.spaces[e].workingColorSpaceConfig.unpackColorSpace;
		},
		fromWorkingColorSpace: function(t, n) {
			return yr("ColorManagement: .fromWorkingColorSpace() has been renamed to .workingToColorSpace()."), e.workingToColorSpace(t, n);
		},
		toWorkingColorSpace: function(t, n) {
			return yr("ColorManagement: .toWorkingColorSpace() has been renamed to .colorSpaceToWorking()."), e.colorSpaceToWorking(t, n);
		}
	}, t = [
		.64,
		.33,
		.3,
		.6,
		.15,
		.06
	], n = [
		.2126,
		.7152,
		.0722
	], r = [.3127, .329];
	return e.define({
		[ar]: {
			primaries: t,
			whitePoint: r,
			transfer: or,
			toXYZ: ei,
			fromXYZ: ti,
			luminanceCoefficients: n,
			workingColorSpaceConfig: { unpackColorSpace: ir },
			outputColorSpaceConfig: { drawingBufferColorSpace: ir }
		},
		[ir]: {
			primaries: t,
			whitePoint: r,
			transfer: sr,
			toXYZ: ei,
			fromXYZ: ti,
			luminanceCoefficients: n,
			outputColorSpaceConfig: { drawingBufferColorSpace: ir }
		}
	}), e;
}
var Y = /*@__PURE__*/ ni();
function ri(e) {
	return e < .04045 ? e * .0773993808 : (e * .9478672986 + .0521327014) ** 2.4;
}
function ii(e) {
	return e < .0031308 ? e * 12.92 : 1.055 * e ** .41666 - .055;
}
var ai, oi = class {
	static getDataURL(e, t = "image/png") {
		if (/^data:/i.test(e.src) || typeof HTMLCanvasElement > "u") return e.src;
		let n;
		if (e instanceof HTMLCanvasElement) n = e;
		else {
			ai === void 0 && (ai = mr("canvas")), ai.width = e.width, ai.height = e.height;
			let t = ai.getContext("2d");
			e instanceof ImageData ? t.putImageData(e, 0, 0) : t.drawImage(e, 0, 0, e.width, e.height), n = ai;
		}
		return n.toDataURL(t);
	}
	static sRGBToLinear(e) {
		if (typeof HTMLImageElement < "u" && e instanceof HTMLImageElement || typeof HTMLCanvasElement < "u" && e instanceof HTMLCanvasElement || typeof ImageBitmap < "u" && e instanceof ImageBitmap) {
			let t = mr("canvas");
			t.width = e.width, t.height = e.height;
			let n = t.getContext("2d");
			n.drawImage(e, 0, 0, e.width, e.height);
			let r = n.getImageData(0, 0, e.width, e.height), i = r.data;
			for (let e = 0; e < i.length; e++) i[e] = ri(i[e] / 255) * 255;
			return n.putImageData(r, 0, 0), t;
		} else if (e.data) {
			let t = e.data.slice(0);
			for (let e = 0; e < t.length; e++) t instanceof Uint8Array || t instanceof Uint8ClampedArray ? t[e] = Math.floor(ri(t[e] / 255) * 255) : t[e] = ri(t[e]);
			return {
				data: t,
				width: e.width,
				height: e.height
			};
		} else return W("ImageUtils.sRGBToLinear(): Unsupported image type. No color space conversion applied."), e;
	}
}, si = 0, ci = class {
	constructor(e = null) {
		this.isSource = !0, Object.defineProperty(this, "id", { value: si++ }), this.uuid = Dr(), this.data = e, this.dataReady = !0, this.version = 0;
	}
	getSize(e) {
		let t = this.data;
		return typeof HTMLVideoElement < "u" && t instanceof HTMLVideoElement ? e.set(t.videoWidth, t.videoHeight, 0) : typeof VideoFrame < "u" && t instanceof VideoFrame ? e.set(t.displayWidth, t.displayHeight, 0) : t === null ? e.set(0, 0, 0) : e.set(t.width, t.height, t.depth || 0), e;
	}
	set needsUpdate(e) {
		e === !0 && this.version++;
	}
	toJSON(e) {
		let t = e === void 0 || typeof e == "string";
		if (!t && e.images[this.uuid] !== void 0) return e.images[this.uuid];
		let n = {
			uuid: this.uuid,
			url: ""
		}, r = this.data;
		if (r !== null) {
			let e;
			if (Array.isArray(r)) {
				e = [];
				for (let t = 0, n = r.length; t < n; t++) r[t].isDataTexture ? e.push(li(r[t].image)) : e.push(li(r[t]));
			} else e = li(r);
			n.url = e;
		}
		return t || (e.images[this.uuid] = n), n;
	}
};
function li(e) {
	return typeof HTMLImageElement < "u" && e instanceof HTMLImageElement || typeof HTMLCanvasElement < "u" && e instanceof HTMLCanvasElement || typeof ImageBitmap < "u" && e instanceof ImageBitmap ? oi.getDataURL(e) : e.data ? {
		data: Array.from(e.data),
		width: e.width,
		height: e.height,
		type: e.data.constructor.name
	} : (W("Texture: Unable to serialize Texture."), {});
}
var ui = 0, di = /*@__PURE__*/ new q(), fi = class e extends Sr {
	constructor(t = e.DEFAULT_IMAGE, n = e.DEFAULT_MAPPING, r = jt, i = jt, a = It, o = Rt, s = en, c = zt, l = e.DEFAULT_ANISOTROPY, u = "") {
		super(), this.isTexture = !0, Object.defineProperty(this, "id", { value: ui++ }), this.uuid = Dr(), this.name = "", this.source = new ci(t), this.mipmaps = [], this.mapping = n, this.channel = 0, this.wrapS = r, this.wrapT = i, this.magFilter = a, this.minFilter = o, this.anisotropy = l, this.format = s, this.internalFormat = null, this.type = c, this.offset = new Yr(0, 0), this.repeat = new Yr(1, 1), this.center = new Yr(0, 0), this.rotation = 0, this.matrixAutoUpdate = !0, this.matrix = new J(), this.generateMipmaps = !0, this.premultiplyAlpha = !1, this.flipY = !0, this.unpackAlignment = 4, this.colorSpace = u, this.userData = {}, this.updateRanges = [], this.version = 0, this.onUpdate = null, this.renderTarget = null, this.isRenderTargetTexture = !1, this.isArrayTexture = !!(t && t.depth && t.depth > 1), this.pmremVersion = 0, this.normalized = !1;
	}
	get width() {
		return this.source.getSize(di).x;
	}
	get height() {
		return this.source.getSize(di).y;
	}
	get depth() {
		return this.source.getSize(di).z;
	}
	get image() {
		return this.source.data;
	}
	set image(e) {
		this.source.data = e;
	}
	updateMatrix() {
		this.matrix.setUvTransform(this.offset.x, this.offset.y, this.repeat.x, this.repeat.y, this.rotation, this.center.x, this.center.y);
	}
	addUpdateRange(e, t) {
		this.updateRanges.push({
			start: e,
			count: t
		});
	}
	clearUpdateRanges() {
		this.updateRanges.length = 0;
	}
	clone() {
		return new this.constructor().copy(this);
	}
	copy(e) {
		return this.name = e.name, this.source = e.source, this.mipmaps = e.mipmaps.slice(0), this.mapping = e.mapping, this.channel = e.channel, this.wrapS = e.wrapS, this.wrapT = e.wrapT, this.magFilter = e.magFilter, this.minFilter = e.minFilter, this.anisotropy = e.anisotropy, this.format = e.format, this.internalFormat = e.internalFormat, this.type = e.type, this.normalized = e.normalized, this.offset.copy(e.offset), this.repeat.copy(e.repeat), this.center.copy(e.center), this.rotation = e.rotation, this.matrixAutoUpdate = e.matrixAutoUpdate, this.matrix.copy(e.matrix), this.generateMipmaps = e.generateMipmaps, this.premultiplyAlpha = e.premultiplyAlpha, this.flipY = e.flipY, this.unpackAlignment = e.unpackAlignment, this.colorSpace = e.colorSpace, this.renderTarget = e.renderTarget, this.isRenderTargetTexture = e.isRenderTargetTexture, this.isArrayTexture = e.isArrayTexture, this.userData = JSON.parse(JSON.stringify(e.userData)), this.needsUpdate = !0, this;
	}
	setValues(e) {
		for (let t in e) {
			let n = e[t];
			if (n === void 0) {
				W(`Texture.setValues(): parameter '${t}' has value of undefined.`);
				continue;
			}
			let r = this[t];
			if (r === void 0) {
				W(`Texture.setValues(): property '${t}' does not exist.`);
				continue;
			}
			r && n && r.isVector2 && n.isVector2 || r && n && r.isVector3 && n.isVector3 || r && n && r.isMatrix3 && n.isMatrix3 ? r.copy(n) : this[t] = n;
		}
	}
	toJSON(e) {
		let t = e === void 0 || typeof e == "string";
		if (!t && e.textures[this.uuid] !== void 0) return e.textures[this.uuid];
		let n = {
			metadata: {
				version: 4.7,
				type: "Texture",
				generator: "Texture.toJSON"
			},
			uuid: this.uuid,
			name: this.name,
			image: this.source.toJSON(e).uuid,
			mapping: this.mapping,
			channel: this.channel,
			repeat: [this.repeat.x, this.repeat.y],
			offset: [this.offset.x, this.offset.y],
			center: [this.center.x, this.center.y],
			rotation: this.rotation,
			wrap: [this.wrapS, this.wrapT],
			format: this.format,
			internalFormat: this.internalFormat,
			type: this.type,
			normalized: this.normalized,
			colorSpace: this.colorSpace,
			minFilter: this.minFilter,
			magFilter: this.magFilter,
			anisotropy: this.anisotropy,
			flipY: this.flipY,
			generateMipmaps: this.generateMipmaps,
			premultiplyAlpha: this.premultiplyAlpha,
			unpackAlignment: this.unpackAlignment
		};
		return Object.keys(this.userData).length > 0 && (n.userData = this.userData), t || (e.textures[this.uuid] = n), n;
	}
	dispose() {
		this.dispatchEvent({ type: "dispose" });
	}
	transformUv(e) {
		if (this.mapping !== 300) return e;
		if (e.applyMatrix3(this.matrix), e.x < 0 || e.x > 1) switch (this.wrapS) {
			case At:
				e.x -= Math.floor(e.x);
				break;
			case jt:
				e.x = e.x < 0 ? 0 : 1;
				break;
			case Mt:
				Math.abs(Math.floor(e.x) % 2) === 1 ? e.x = Math.ceil(e.x) - e.x : e.x -= Math.floor(e.x);
				break;
		}
		if (e.y < 0 || e.y > 1) switch (this.wrapT) {
			case At:
				e.y -= Math.floor(e.y);
				break;
			case jt:
				e.y = e.y < 0 ? 0 : 1;
				break;
			case Mt:
				Math.abs(Math.floor(e.y) % 2) === 1 ? e.y = Math.ceil(e.y) - e.y : e.y -= Math.floor(e.y);
				break;
		}
		return this.flipY && (e.y = 1 - e.y), e;
	}
	set needsUpdate(e) {
		e === !0 && (this.version++, this.source.needsUpdate = !0);
	}
	set needsPMREMUpdate(e) {
		e === !0 && this.pmremVersion++;
	}
};
fi.DEFAULT_IMAGE = null, fi.DEFAULT_MAPPING = 300, fi.DEFAULT_ANISOTROPY = 1;
var pi = class e {
	static {
		e.prototype.isVector4 = !0;
	}
	constructor(e = 0, t = 0, n = 0, r = 1) {
		this.x = e, this.y = t, this.z = n, this.w = r;
	}
	get width() {
		return this.z;
	}
	set width(e) {
		this.z = e;
	}
	get height() {
		return this.w;
	}
	set height(e) {
		this.w = e;
	}
	set(e, t, n, r) {
		return this.x = e, this.y = t, this.z = n, this.w = r, this;
	}
	setScalar(e) {
		return this.x = e, this.y = e, this.z = e, this.w = e, this;
	}
	setX(e) {
		return this.x = e, this;
	}
	setY(e) {
		return this.y = e, this;
	}
	setZ(e) {
		return this.z = e, this;
	}
	setW(e) {
		return this.w = e, this;
	}
	setComponent(e, t) {
		switch (e) {
			case 0:
				this.x = t;
				break;
			case 1:
				this.y = t;
				break;
			case 2:
				this.z = t;
				break;
			case 3:
				this.w = t;
				break;
			default: throw Error("index is out of range: " + e);
		}
		return this;
	}
	getComponent(e) {
		switch (e) {
			case 0: return this.x;
			case 1: return this.y;
			case 2: return this.z;
			case 3: return this.w;
			default: throw Error("index is out of range: " + e);
		}
	}
	clone() {
		return new this.constructor(this.x, this.y, this.z, this.w);
	}
	copy(e) {
		return this.x = e.x, this.y = e.y, this.z = e.z, this.w = e.w === void 0 ? 1 : e.w, this;
	}
	add(e) {
		return this.x += e.x, this.y += e.y, this.z += e.z, this.w += e.w, this;
	}
	addScalar(e) {
		return this.x += e, this.y += e, this.z += e, this.w += e, this;
	}
	addVectors(e, t) {
		return this.x = e.x + t.x, this.y = e.y + t.y, this.z = e.z + t.z, this.w = e.w + t.w, this;
	}
	addScaledVector(e, t) {
		return this.x += e.x * t, this.y += e.y * t, this.z += e.z * t, this.w += e.w * t, this;
	}
	sub(e) {
		return this.x -= e.x, this.y -= e.y, this.z -= e.z, this.w -= e.w, this;
	}
	subScalar(e) {
		return this.x -= e, this.y -= e, this.z -= e, this.w -= e, this;
	}
	subVectors(e, t) {
		return this.x = e.x - t.x, this.y = e.y - t.y, this.z = e.z - t.z, this.w = e.w - t.w, this;
	}
	multiply(e) {
		return this.x *= e.x, this.y *= e.y, this.z *= e.z, this.w *= e.w, this;
	}
	multiplyScalar(e) {
		return this.x *= e, this.y *= e, this.z *= e, this.w *= e, this;
	}
	applyMatrix4(e) {
		let t = this.x, n = this.y, r = this.z, i = this.w, a = e.elements;
		return this.x = a[0] * t + a[4] * n + a[8] * r + a[12] * i, this.y = a[1] * t + a[5] * n + a[9] * r + a[13] * i, this.z = a[2] * t + a[6] * n + a[10] * r + a[14] * i, this.w = a[3] * t + a[7] * n + a[11] * r + a[15] * i, this;
	}
	divide(e) {
		return this.x /= e.x, this.y /= e.y, this.z /= e.z, this.w /= e.w, this;
	}
	divideScalar(e) {
		return this.multiplyScalar(1 / e);
	}
	setAxisAngleFromQuaternion(e) {
		this.w = 2 * Math.acos(e.w);
		let t = Math.sqrt(1 - e.w * e.w);
		return t < 1e-4 ? (this.x = 1, this.y = 0, this.z = 0) : (this.x = e.x / t, this.y = e.y / t, this.z = e.z / t), this;
	}
	setAxisAngleFromRotationMatrix(e) {
		let t, n, r, i, a = .01, o = .1, s = e.elements, c = s[0], l = s[4], u = s[8], d = s[1], f = s[5], p = s[9], m = s[2], h = s[6], g = s[10];
		if (Math.abs(l - d) < a && Math.abs(u - m) < a && Math.abs(p - h) < a) {
			if (Math.abs(l + d) < o && Math.abs(u + m) < o && Math.abs(p + h) < o && Math.abs(c + f + g - 3) < o) return this.set(1, 0, 0, 0), this;
			t = Math.PI;
			let e = (c + 1) / 2, s = (f + 1) / 2, _ = (g + 1) / 2, v = (l + d) / 4, y = (u + m) / 4, b = (p + h) / 4;
			return e > s && e > _ ? e < a ? (n = 0, r = .707106781, i = .707106781) : (n = Math.sqrt(e), r = v / n, i = y / n) : s > _ ? s < a ? (n = .707106781, r = 0, i = .707106781) : (r = Math.sqrt(s), n = v / r, i = b / r) : _ < a ? (n = .707106781, r = .707106781, i = 0) : (i = Math.sqrt(_), n = y / i, r = b / i), this.set(n, r, i, t), this;
		}
		let _ = Math.sqrt((h - p) * (h - p) + (u - m) * (u - m) + (d - l) * (d - l));
		return Math.abs(_) < .001 && (_ = 1), this.x = (h - p) / _, this.y = (u - m) / _, this.z = (d - l) / _, this.w = Math.acos((c + f + g - 1) / 2), this;
	}
	setFromMatrixPosition(e) {
		let t = e.elements;
		return this.x = t[12], this.y = t[13], this.z = t[14], this.w = t[15], this;
	}
	min(e) {
		return this.x = Math.min(this.x, e.x), this.y = Math.min(this.y, e.y), this.z = Math.min(this.z, e.z), this.w = Math.min(this.w, e.w), this;
	}
	max(e) {
		return this.x = Math.max(this.x, e.x), this.y = Math.max(this.y, e.y), this.z = Math.max(this.z, e.z), this.w = Math.max(this.w, e.w), this;
	}
	clamp(e, t) {
		return this.x = K(this.x, e.x, t.x), this.y = K(this.y, e.y, t.y), this.z = K(this.z, e.z, t.z), this.w = K(this.w, e.w, t.w), this;
	}
	clampScalar(e, t) {
		return this.x = K(this.x, e, t), this.y = K(this.y, e, t), this.z = K(this.z, e, t), this.w = K(this.w, e, t), this;
	}
	clampLength(e, t) {
		let n = this.length();
		return this.divideScalar(n || 1).multiplyScalar(K(n, e, t));
	}
	floor() {
		return this.x = Math.floor(this.x), this.y = Math.floor(this.y), this.z = Math.floor(this.z), this.w = Math.floor(this.w), this;
	}
	ceil() {
		return this.x = Math.ceil(this.x), this.y = Math.ceil(this.y), this.z = Math.ceil(this.z), this.w = Math.ceil(this.w), this;
	}
	round() {
		return this.x = Math.round(this.x), this.y = Math.round(this.y), this.z = Math.round(this.z), this.w = Math.round(this.w), this;
	}
	roundToZero() {
		return this.x = Math.trunc(this.x), this.y = Math.trunc(this.y), this.z = Math.trunc(this.z), this.w = Math.trunc(this.w), this;
	}
	negate() {
		return this.x = -this.x, this.y = -this.y, this.z = -this.z, this.w = -this.w, this;
	}
	dot(e) {
		return this.x * e.x + this.y * e.y + this.z * e.z + this.w * e.w;
	}
	lengthSq() {
		return this.x * this.x + this.y * this.y + this.z * this.z + this.w * this.w;
	}
	length() {
		return Math.sqrt(this.x * this.x + this.y * this.y + this.z * this.z + this.w * this.w);
	}
	manhattanLength() {
		return Math.abs(this.x) + Math.abs(this.y) + Math.abs(this.z) + Math.abs(this.w);
	}
	normalize() {
		return this.divideScalar(this.length() || 1);
	}
	setLength(e) {
		return this.normalize().multiplyScalar(e);
	}
	lerp(e, t) {
		return this.x += (e.x - this.x) * t, this.y += (e.y - this.y) * t, this.z += (e.z - this.z) * t, this.w += (e.w - this.w) * t, this;
	}
	lerpVectors(e, t, n) {
		return this.x = e.x + (t.x - e.x) * n, this.y = e.y + (t.y - e.y) * n, this.z = e.z + (t.z - e.z) * n, this.w = e.w + (t.w - e.w) * n, this;
	}
	equals(e) {
		return e.x === this.x && e.y === this.y && e.z === this.z && e.w === this.w;
	}
	fromArray(e, t = 0) {
		return this.x = e[t], this.y = e[t + 1], this.z = e[t + 2], this.w = e[t + 3], this;
	}
	toArray(e = [], t = 0) {
		return e[t] = this.x, e[t + 1] = this.y, e[t + 2] = this.z, e[t + 3] = this.w, e;
	}
	fromBufferAttribute(e, t) {
		return this.x = e.getX(t), this.y = e.getY(t), this.z = e.getZ(t), this.w = e.getW(t), this;
	}
	random() {
		return this.x = Math.random(), this.y = Math.random(), this.z = Math.random(), this.w = Math.random(), this;
	}
	*[Symbol.iterator]() {
		yield this.x, yield this.y, yield this.z, yield this.w;
	}
}, mi = class extends Sr {
	constructor(e = 1, t = 1, n = {}) {
		super(), n = Object.assign({
			generateMipmaps: !1,
			internalFormat: null,
			minFilter: It,
			depthBuffer: !0,
			stencilBuffer: !1,
			resolveDepthBuffer: !0,
			resolveStencilBuffer: !0,
			depthTexture: null,
			samples: 0,
			count: 1,
			depth: 1,
			multiview: !1
		}, n), this.isRenderTarget = !0, this.width = e, this.height = t, this.depth = n.depth, this.scissor = new pi(0, 0, e, t), this.scissorTest = !1, this.viewport = new pi(0, 0, e, t), this.textures = [];
		let r = new fi({
			width: e,
			height: t,
			depth: n.depth
		}), i = n.count;
		for (let e = 0; e < i; e++) this.textures[e] = r.clone(), this.textures[e].isRenderTargetTexture = !0, this.textures[e].renderTarget = this;
		this._setTextureOptions(n), this.depthBuffer = n.depthBuffer, this.stencilBuffer = n.stencilBuffer, this.resolveDepthBuffer = n.resolveDepthBuffer, this.resolveStencilBuffer = n.resolveStencilBuffer, this._depthTexture = null, this.depthTexture = n.depthTexture, this.samples = n.samples, this.multiview = n.multiview;
	}
	_setTextureOptions(e = {}) {
		let t = {
			minFilter: It,
			generateMipmaps: !1,
			flipY: !1,
			internalFormat: null
		};
		e.mapping !== void 0 && (t.mapping = e.mapping), e.wrapS !== void 0 && (t.wrapS = e.wrapS), e.wrapT !== void 0 && (t.wrapT = e.wrapT), e.wrapR !== void 0 && (t.wrapR = e.wrapR), e.magFilter !== void 0 && (t.magFilter = e.magFilter), e.minFilter !== void 0 && (t.minFilter = e.minFilter), e.format !== void 0 && (t.format = e.format), e.type !== void 0 && (t.type = e.type), e.anisotropy !== void 0 && (t.anisotropy = e.anisotropy), e.colorSpace !== void 0 && (t.colorSpace = e.colorSpace), e.flipY !== void 0 && (t.flipY = e.flipY), e.generateMipmaps !== void 0 && (t.generateMipmaps = e.generateMipmaps), e.internalFormat !== void 0 && (t.internalFormat = e.internalFormat);
		for (let e = 0; e < this.textures.length; e++) this.textures[e].setValues(t);
	}
	get texture() {
		return this.textures[0];
	}
	set texture(e) {
		this.textures[0] = e;
	}
	set depthTexture(e) {
		this._depthTexture !== null && (this._depthTexture.renderTarget = null), e !== null && (e.renderTarget = this), this._depthTexture = e;
	}
	get depthTexture() {
		return this._depthTexture;
	}
	setSize(e, t, n = 1) {
		if (this.width !== e || this.height !== t || this.depth !== n) {
			this.width = e, this.height = t, this.depth = n;
			for (let r = 0, i = this.textures.length; r < i; r++) this.textures[r].image.width = e, this.textures[r].image.height = t, this.textures[r].image.depth = n, this.textures[r].isData3DTexture !== !0 && (this.textures[r].isArrayTexture = this.textures[r].image.depth > 1);
			this.dispose();
		}
		this.viewport.set(0, 0, e, t), this.scissor.set(0, 0, e, t);
	}
	clone() {
		return new this.constructor().copy(this);
	}
	copy(e) {
		this.width = e.width, this.height = e.height, this.depth = e.depth, this.scissor.copy(e.scissor), this.scissorTest = e.scissorTest, this.viewport.copy(e.viewport), this.textures.length = 0;
		for (let t = 0, n = e.textures.length; t < n; t++) {
			this.textures[t] = e.textures[t].clone(), this.textures[t].isRenderTargetTexture = !0, this.textures[t].renderTarget = this;
			let n = Object.assign({}, e.textures[t].image);
			this.textures[t].source = new ci(n);
		}
		return this.depthBuffer = e.depthBuffer, this.stencilBuffer = e.stencilBuffer, this.resolveDepthBuffer = e.resolveDepthBuffer, this.resolveStencilBuffer = e.resolveStencilBuffer, e.depthTexture !== null && (this.depthTexture = e.depthTexture.clone()), this.samples = e.samples, this.multiview = e.multiview, this;
	}
	dispose() {
		this.dispatchEvent({ type: "dispose" });
	}
}, hi = class extends mi {
	constructor(e = 1, t = 1, n = {}) {
		super(e, t, n), this.isWebGLRenderTarget = !0;
	}
}, gi = class extends fi {
	constructor(e = null, t = 1, n = 1, r = 1) {
		super(null), this.isDataArrayTexture = !0, this.image = {
			data: e,
			width: t,
			height: n,
			depth: r
		}, this.magFilter = Nt, this.minFilter = Nt, this.wrapR = jt, this.generateMipmaps = !1, this.flipY = !1, this.unpackAlignment = 1, this.layerUpdates = /* @__PURE__ */ new Set();
	}
	addLayerUpdate(e) {
		this.layerUpdates.add(e);
	}
	clearLayerUpdates() {
		this.layerUpdates.clear();
	}
}, _i = class extends fi {
	constructor(e = null, t = 1, n = 1, r = 1) {
		super(null), this.isData3DTexture = !0, this.image = {
			data: e,
			width: t,
			height: n,
			depth: r
		}, this.magFilter = Nt, this.minFilter = Nt, this.wrapR = jt, this.generateMipmaps = !1, this.flipY = !1, this.unpackAlignment = 1;
	}
}, vi = class e {
	static {
		e.prototype.isMatrix4 = !0;
	}
	constructor(e, t, n, r, i, a, o, s, c, l, u, d, f, p, m, h) {
		this.elements = [
			1,
			0,
			0,
			0,
			0,
			1,
			0,
			0,
			0,
			0,
			1,
			0,
			0,
			0,
			0,
			1
		], e !== void 0 && this.set(e, t, n, r, i, a, o, s, c, l, u, d, f, p, m, h);
	}
	set(e, t, n, r, i, a, o, s, c, l, u, d, f, p, m, h) {
		let g = this.elements;
		return g[0] = e, g[4] = t, g[8] = n, g[12] = r, g[1] = i, g[5] = a, g[9] = o, g[13] = s, g[2] = c, g[6] = l, g[10] = u, g[14] = d, g[3] = f, g[7] = p, g[11] = m, g[15] = h, this;
	}
	identity() {
		return this.set(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1), this;
	}
	clone() {
		return new e().fromArray(this.elements);
	}
	copy(e) {
		let t = this.elements, n = e.elements;
		return t[0] = n[0], t[1] = n[1], t[2] = n[2], t[3] = n[3], t[4] = n[4], t[5] = n[5], t[6] = n[6], t[7] = n[7], t[8] = n[8], t[9] = n[9], t[10] = n[10], t[11] = n[11], t[12] = n[12], t[13] = n[13], t[14] = n[14], t[15] = n[15], this;
	}
	copyPosition(e) {
		let t = this.elements, n = e.elements;
		return t[12] = n[12], t[13] = n[13], t[14] = n[14], this;
	}
	setFromMatrix3(e) {
		let t = e.elements;
		return this.set(t[0], t[3], t[6], 0, t[1], t[4], t[7], 0, t[2], t[5], t[8], 0, 0, 0, 0, 1), this;
	}
	extractBasis(e, t, n) {
		return this.determinant() === 0 ? (e.set(1, 0, 0), t.set(0, 1, 0), n.set(0, 0, 1), this) : (e.setFromMatrixColumn(this, 0), t.setFromMatrixColumn(this, 1), n.setFromMatrixColumn(this, 2), this);
	}
	makeBasis(e, t, n) {
		return this.set(e.x, t.x, n.x, 0, e.y, t.y, n.y, 0, e.z, t.z, n.z, 0, 0, 0, 0, 1), this;
	}
	extractRotation(e) {
		if (e.determinant() === 0) return this.identity();
		let t = this.elements, n = e.elements, r = 1 / yi.setFromMatrixColumn(e, 0).length(), i = 1 / yi.setFromMatrixColumn(e, 1).length(), a = 1 / yi.setFromMatrixColumn(e, 2).length();
		return t[0] = n[0] * r, t[1] = n[1] * r, t[2] = n[2] * r, t[3] = 0, t[4] = n[4] * i, t[5] = n[5] * i, t[6] = n[6] * i, t[7] = 0, t[8] = n[8] * a, t[9] = n[9] * a, t[10] = n[10] * a, t[11] = 0, t[12] = 0, t[13] = 0, t[14] = 0, t[15] = 1, this;
	}
	makeRotationFromEuler(e) {
		let t = this.elements, n = e.x, r = e.y, i = e.z, a = Math.cos(n), o = Math.sin(n), s = Math.cos(r), c = Math.sin(r), l = Math.cos(i), u = Math.sin(i);
		if (e.order === "XYZ") {
			let e = a * l, n = a * u, r = o * l, i = o * u;
			t[0] = s * l, t[4] = -s * u, t[8] = c, t[1] = n + r * c, t[5] = e - i * c, t[9] = -o * s, t[2] = i - e * c, t[6] = r + n * c, t[10] = a * s;
		} else if (e.order === "YXZ") {
			let e = s * l, n = s * u, r = c * l, i = c * u;
			t[0] = e + i * o, t[4] = r * o - n, t[8] = a * c, t[1] = a * u, t[5] = a * l, t[9] = -o, t[2] = n * o - r, t[6] = i + e * o, t[10] = a * s;
		} else if (e.order === "ZXY") {
			let e = s * l, n = s * u, r = c * l, i = c * u;
			t[0] = e - i * o, t[4] = -a * u, t[8] = r + n * o, t[1] = n + r * o, t[5] = a * l, t[9] = i - e * o, t[2] = -a * c, t[6] = o, t[10] = a * s;
		} else if (e.order === "ZYX") {
			let e = a * l, n = a * u, r = o * l, i = o * u;
			t[0] = s * l, t[4] = r * c - n, t[8] = e * c + i, t[1] = s * u, t[5] = i * c + e, t[9] = n * c - r, t[2] = -c, t[6] = o * s, t[10] = a * s;
		} else if (e.order === "YZX") {
			let e = a * s, n = a * c, r = o * s, i = o * c;
			t[0] = s * l, t[4] = i - e * u, t[8] = r * u + n, t[1] = u, t[5] = a * l, t[9] = -o * l, t[2] = -c * l, t[6] = n * u + r, t[10] = e - i * u;
		} else if (e.order === "XZY") {
			let e = a * s, n = a * c, r = o * s, i = o * c;
			t[0] = s * l, t[4] = -u, t[8] = c * l, t[1] = e * u + i, t[5] = a * l, t[9] = n * u - r, t[2] = r * u - n, t[6] = o * l, t[10] = i * u + e;
		}
		return t[3] = 0, t[7] = 0, t[11] = 0, t[12] = 0, t[13] = 0, t[14] = 0, t[15] = 1, this;
	}
	makeRotationFromQuaternion(e) {
		return this.compose(xi, e, Si);
	}
	lookAt(e, t, n) {
		let r = this.elements;
		return Ti.subVectors(e, t), Ti.lengthSq() === 0 && (Ti.z = 1), Ti.normalize(), Ci.crossVectors(n, Ti), Ci.lengthSq() === 0 && (Math.abs(n.z) === 1 ? Ti.x += 1e-4 : Ti.z += 1e-4, Ti.normalize(), Ci.crossVectors(n, Ti)), Ci.normalize(), wi.crossVectors(Ti, Ci), r[0] = Ci.x, r[4] = wi.x, r[8] = Ti.x, r[1] = Ci.y, r[5] = wi.y, r[9] = Ti.y, r[2] = Ci.z, r[6] = wi.z, r[10] = Ti.z, this;
	}
	multiply(e) {
		return this.multiplyMatrices(this, e);
	}
	premultiply(e) {
		return this.multiplyMatrices(e, this);
	}
	multiplyMatrices(e, t) {
		let n = e.elements, r = t.elements, i = this.elements, a = n[0], o = n[4], s = n[8], c = n[12], l = n[1], u = n[5], d = n[9], f = n[13], p = n[2], m = n[6], h = n[10], g = n[14], _ = n[3], v = n[7], y = n[11], b = n[15], x = r[0], S = r[4], C = r[8], w = r[12], T = r[1], E = r[5], D = r[9], O = r[13], k = r[2], A = r[6], ee = r[10], te = r[14], ne = r[3], re = r[7], ie = r[11], ae = r[15];
		return i[0] = a * x + o * T + s * k + c * ne, i[4] = a * S + o * E + s * A + c * re, i[8] = a * C + o * D + s * ee + c * ie, i[12] = a * w + o * O + s * te + c * ae, i[1] = l * x + u * T + d * k + f * ne, i[5] = l * S + u * E + d * A + f * re, i[9] = l * C + u * D + d * ee + f * ie, i[13] = l * w + u * O + d * te + f * ae, i[2] = p * x + m * T + h * k + g * ne, i[6] = p * S + m * E + h * A + g * re, i[10] = p * C + m * D + h * ee + g * ie, i[14] = p * w + m * O + h * te + g * ae, i[3] = _ * x + v * T + y * k + b * ne, i[7] = _ * S + v * E + y * A + b * re, i[11] = _ * C + v * D + y * ee + b * ie, i[15] = _ * w + v * O + y * te + b * ae, this;
	}
	multiplyScalar(e) {
		let t = this.elements;
		return t[0] *= e, t[4] *= e, t[8] *= e, t[12] *= e, t[1] *= e, t[5] *= e, t[9] *= e, t[13] *= e, t[2] *= e, t[6] *= e, t[10] *= e, t[14] *= e, t[3] *= e, t[7] *= e, t[11] *= e, t[15] *= e, this;
	}
	determinant() {
		let e = this.elements, t = e[0], n = e[4], r = e[8], i = e[12], a = e[1], o = e[5], s = e[9], c = e[13], l = e[2], u = e[6], d = e[10], f = e[14], p = e[3], m = e[7], h = e[11], g = e[15], _ = s * f - c * d, v = o * f - c * u, y = o * d - s * u, b = a * f - c * l, x = a * d - s * l, S = a * u - o * l;
		return t * (m * _ - h * v + g * y) - n * (p * _ - h * b + g * x) + r * (p * v - m * b + g * S) - i * (p * y - m * x + h * S);
	}
	transpose() {
		let e = this.elements, t;
		return t = e[1], e[1] = e[4], e[4] = t, t = e[2], e[2] = e[8], e[8] = t, t = e[6], e[6] = e[9], e[9] = t, t = e[3], e[3] = e[12], e[12] = t, t = e[7], e[7] = e[13], e[13] = t, t = e[11], e[11] = e[14], e[14] = t, this;
	}
	setPosition(e, t, n) {
		let r = this.elements;
		return e.isVector3 ? (r[12] = e.x, r[13] = e.y, r[14] = e.z) : (r[12] = e, r[13] = t, r[14] = n), this;
	}
	invert() {
		let e = this.elements, t = e[0], n = e[1], r = e[2], i = e[3], a = e[4], o = e[5], s = e[6], c = e[7], l = e[8], u = e[9], d = e[10], f = e[11], p = e[12], m = e[13], h = e[14], g = e[15], _ = t * o - n * a, v = t * s - r * a, y = t * c - i * a, b = n * s - r * o, x = n * c - i * o, S = r * c - i * s, C = l * m - u * p, w = l * h - d * p, T = l * g - f * p, E = u * h - d * m, D = u * g - f * m, O = d * g - f * h, k = _ * O - v * D + y * E + b * T - x * w + S * C;
		if (k === 0) return this.set(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
		let A = 1 / k;
		return e[0] = (o * O - s * D + c * E) * A, e[1] = (r * D - n * O - i * E) * A, e[2] = (m * S - h * x + g * b) * A, e[3] = (d * x - u * S - f * b) * A, e[4] = (s * T - a * O - c * w) * A, e[5] = (t * O - r * T + i * w) * A, e[6] = (h * y - p * S - g * v) * A, e[7] = (l * S - d * y + f * v) * A, e[8] = (a * D - o * T + c * C) * A, e[9] = (n * T - t * D - i * C) * A, e[10] = (p * x - m * y + g * _) * A, e[11] = (u * y - l * x - f * _) * A, e[12] = (o * w - a * E - s * C) * A, e[13] = (t * E - n * w + r * C) * A, e[14] = (m * v - p * b - h * _) * A, e[15] = (l * b - u * v + d * _) * A, this;
	}
	scale(e) {
		let t = this.elements, n = e.x, r = e.y, i = e.z;
		return t[0] *= n, t[4] *= r, t[8] *= i, t[1] *= n, t[5] *= r, t[9] *= i, t[2] *= n, t[6] *= r, t[10] *= i, t[3] *= n, t[7] *= r, t[11] *= i, this;
	}
	getMaxScaleOnAxis() {
		let e = this.elements, t = e[0] * e[0] + e[1] * e[1] + e[2] * e[2], n = e[4] * e[4] + e[5] * e[5] + e[6] * e[6], r = e[8] * e[8] + e[9] * e[9] + e[10] * e[10];
		return Math.sqrt(Math.max(t, n, r));
	}
	makeTranslation(e, t, n) {
		return e.isVector3 ? this.set(1, 0, 0, e.x, 0, 1, 0, e.y, 0, 0, 1, e.z, 0, 0, 0, 1) : this.set(1, 0, 0, e, 0, 1, 0, t, 0, 0, 1, n, 0, 0, 0, 1), this;
	}
	makeRotationX(e) {
		let t = Math.cos(e), n = Math.sin(e);
		return this.set(1, 0, 0, 0, 0, t, -n, 0, 0, n, t, 0, 0, 0, 0, 1), this;
	}
	makeRotationY(e) {
		let t = Math.cos(e), n = Math.sin(e);
		return this.set(t, 0, n, 0, 0, 1, 0, 0, -n, 0, t, 0, 0, 0, 0, 1), this;
	}
	makeRotationZ(e) {
		let t = Math.cos(e), n = Math.sin(e);
		return this.set(t, -n, 0, 0, n, t, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1), this;
	}
	makeRotationAxis(e, t) {
		let n = Math.cos(t), r = Math.sin(t), i = 1 - n, a = e.x, o = e.y, s = e.z, c = i * a, l = i * o;
		return this.set(c * a + n, c * o - r * s, c * s + r * o, 0, c * o + r * s, l * o + n, l * s - r * a, 0, c * s - r * o, l * s + r * a, i * s * s + n, 0, 0, 0, 0, 1), this;
	}
	makeScale(e, t, n) {
		return this.set(e, 0, 0, 0, 0, t, 0, 0, 0, 0, n, 0, 0, 0, 0, 1), this;
	}
	makeShear(e, t, n, r, i, a) {
		return this.set(1, n, i, 0, e, 1, a, 0, t, r, 1, 0, 0, 0, 0, 1), this;
	}
	compose(e, t, n) {
		let r = this.elements, i = t._x, a = t._y, o = t._z, s = t._w, c = i + i, l = a + a, u = o + o, d = i * c, f = i * l, p = i * u, m = a * l, h = a * u, g = o * u, _ = s * c, v = s * l, y = s * u, b = n.x, x = n.y, S = n.z;
		return r[0] = (1 - (m + g)) * b, r[1] = (f + y) * b, r[2] = (p - v) * b, r[3] = 0, r[4] = (f - y) * x, r[5] = (1 - (d + g)) * x, r[6] = (h + _) * x, r[7] = 0, r[8] = (p + v) * S, r[9] = (h - _) * S, r[10] = (1 - (d + m)) * S, r[11] = 0, r[12] = e.x, r[13] = e.y, r[14] = e.z, r[15] = 1, this;
	}
	decompose(e, t, n) {
		let r = this.elements;
		e.x = r[12], e.y = r[13], e.z = r[14];
		let i = this.determinant();
		if (i === 0) return n.set(1, 1, 1), t.identity(), this;
		let a = yi.set(r[0], r[1], r[2]).length(), o = yi.set(r[4], r[5], r[6]).length(), s = yi.set(r[8], r[9], r[10]).length();
		i < 0 && (a = -a), bi.copy(this);
		let c = 1 / a, l = 1 / o, u = 1 / s;
		return bi.elements[0] *= c, bi.elements[1] *= c, bi.elements[2] *= c, bi.elements[4] *= l, bi.elements[5] *= l, bi.elements[6] *= l, bi.elements[8] *= u, bi.elements[9] *= u, bi.elements[10] *= u, t.setFromRotationMatrix(bi), n.x = a, n.y = o, n.z = s, this;
	}
	makePerspective(e, t, n, r, i, a, o = dr, s = !1) {
		let c = this.elements, l = 2 * i / (t - e), u = 2 * i / (n - r), d = (t + e) / (t - e), f = (n + r) / (n - r), p, m;
		if (s) p = i / (a - i), m = a * i / (a - i);
		else if (o === 2e3) p = -(a + i) / (a - i), m = -2 * a * i / (a - i);
		else if (o === 2001) p = -a / (a - i), m = -a * i / (a - i);
		else throw Error("THREE.Matrix4.makePerspective(): Invalid coordinate system: " + o);
		return c[0] = l, c[4] = 0, c[8] = d, c[12] = 0, c[1] = 0, c[5] = u, c[9] = f, c[13] = 0, c[2] = 0, c[6] = 0, c[10] = p, c[14] = m, c[3] = 0, c[7] = 0, c[11] = -1, c[15] = 0, this;
	}
	makeOrthographic(e, t, n, r, i, a, o = dr, s = !1) {
		let c = this.elements, l = 2 / (t - e), u = 2 / (n - r), d = -(t + e) / (t - e), f = -(n + r) / (n - r), p, m;
		if (s) p = 1 / (a - i), m = a / (a - i);
		else if (o === 2e3) p = -2 / (a - i), m = -(a + i) / (a - i);
		else if (o === 2001) p = -1 / (a - i), m = -i / (a - i);
		else throw Error("THREE.Matrix4.makeOrthographic(): Invalid coordinate system: " + o);
		return c[0] = l, c[4] = 0, c[8] = 0, c[12] = d, c[1] = 0, c[5] = u, c[9] = 0, c[13] = f, c[2] = 0, c[6] = 0, c[10] = p, c[14] = m, c[3] = 0, c[7] = 0, c[11] = 0, c[15] = 1, this;
	}
	equals(e) {
		let t = this.elements, n = e.elements;
		for (let e = 0; e < 16; e++) if (t[e] !== n[e]) return !1;
		return !0;
	}
	fromArray(e, t = 0) {
		for (let n = 0; n < 16; n++) this.elements[n] = e[n + t];
		return this;
	}
	toArray(e = [], t = 0) {
		let n = this.elements;
		return e[t] = n[0], e[t + 1] = n[1], e[t + 2] = n[2], e[t + 3] = n[3], e[t + 4] = n[4], e[t + 5] = n[5], e[t + 6] = n[6], e[t + 7] = n[7], e[t + 8] = n[8], e[t + 9] = n[9], e[t + 10] = n[10], e[t + 11] = n[11], e[t + 12] = n[12], e[t + 13] = n[13], e[t + 14] = n[14], e[t + 15] = n[15], e;
	}
}, yi = /*@__PURE__*/ new q(), bi = /*@__PURE__*/ new vi(), xi = /*@__PURE__*/ new q(0, 0, 0), Si = /*@__PURE__*/ new q(1, 1, 1), Ci = /*@__PURE__*/ new q(), wi = /*@__PURE__*/ new q(), Ti = /*@__PURE__*/ new q(), Ei = /*@__PURE__*/ new vi(), Di = /*@__PURE__*/ new Xr(), Oi = class e {
	constructor(t = 0, n = 0, r = 0, i = e.DEFAULT_ORDER) {
		this.isEuler = !0, this._x = t, this._y = n, this._z = r, this._order = i;
	}
	get x() {
		return this._x;
	}
	set x(e) {
		this._x = e, this._onChangeCallback();
	}
	get y() {
		return this._y;
	}
	set y(e) {
		this._y = e, this._onChangeCallback();
	}
	get z() {
		return this._z;
	}
	set z(e) {
		this._z = e, this._onChangeCallback();
	}
	get order() {
		return this._order;
	}
	set order(e) {
		this._order = e, this._onChangeCallback();
	}
	set(e, t, n, r = this._order) {
		return this._x = e, this._y = t, this._z = n, this._order = r, this._onChangeCallback(), this;
	}
	clone() {
		return new this.constructor(this._x, this._y, this._z, this._order);
	}
	copy(e) {
		return this._x = e._x, this._y = e._y, this._z = e._z, this._order = e._order, this._onChangeCallback(), this;
	}
	setFromRotationMatrix(e, t = this._order, n = !0) {
		let r = e.elements, i = r[0], a = r[4], o = r[8], s = r[1], c = r[5], l = r[9], u = r[2], d = r[6], f = r[10];
		switch (t) {
			case "XYZ":
				this._y = Math.asin(K(o, -1, 1)), Math.abs(o) < .9999999 ? (this._x = Math.atan2(-l, f), this._z = Math.atan2(-a, i)) : (this._x = Math.atan2(d, c), this._z = 0);
				break;
			case "YXZ":
				this._x = Math.asin(-K(l, -1, 1)), Math.abs(l) < .9999999 ? (this._y = Math.atan2(o, f), this._z = Math.atan2(s, c)) : (this._y = Math.atan2(-u, i), this._z = 0);
				break;
			case "ZXY":
				this._x = Math.asin(K(d, -1, 1)), Math.abs(d) < .9999999 ? (this._y = Math.atan2(-u, f), this._z = Math.atan2(-a, c)) : (this._y = 0, this._z = Math.atan2(s, i));
				break;
			case "ZYX":
				this._y = Math.asin(-K(u, -1, 1)), Math.abs(u) < .9999999 ? (this._x = Math.atan2(d, f), this._z = Math.atan2(s, i)) : (this._x = 0, this._z = Math.atan2(-a, c));
				break;
			case "YZX":
				this._z = Math.asin(K(s, -1, 1)), Math.abs(s) < .9999999 ? (this._x = Math.atan2(-l, c), this._y = Math.atan2(-u, i)) : (this._x = 0, this._y = Math.atan2(o, f));
				break;
			case "XZY":
				this._z = Math.asin(-K(a, -1, 1)), Math.abs(a) < .9999999 ? (this._x = Math.atan2(d, c), this._y = Math.atan2(o, i)) : (this._x = Math.atan2(-l, f), this._y = 0);
				break;
			default: W("Euler: .setFromRotationMatrix() encountered an unknown order: " + t);
		}
		return this._order = t, n === !0 && this._onChangeCallback(), this;
	}
	setFromQuaternion(e, t, n) {
		return Ei.makeRotationFromQuaternion(e), this.setFromRotationMatrix(Ei, t, n);
	}
	setFromVector3(e, t = this._order) {
		return this.set(e.x, e.y, e.z, t);
	}
	reorder(e) {
		return Di.setFromEuler(this), this.setFromQuaternion(Di, e);
	}
	equals(e) {
		return e._x === this._x && e._y === this._y && e._z === this._z && e._order === this._order;
	}
	fromArray(e) {
		return this._x = e[0], this._y = e[1], this._z = e[2], e[3] !== void 0 && (this._order = e[3]), this._onChangeCallback(), this;
	}
	toArray(e = [], t = 0) {
		return e[t] = this._x, e[t + 1] = this._y, e[t + 2] = this._z, e[t + 3] = this._order, e;
	}
	_onChange(e) {
		return this._onChangeCallback = e, this;
	}
	_onChangeCallback() {}
	*[Symbol.iterator]() {
		yield this._x, yield this._y, yield this._z, yield this._order;
	}
};
Oi.DEFAULT_ORDER = "XYZ";
var ki = class {
	constructor() {
		this.mask = 1;
	}
	set(e) {
		this.mask = (1 << e | 0) >>> 0;
	}
	enable(e) {
		this.mask |= 1 << e | 0;
	}
	enableAll() {
		this.mask = -1;
	}
	toggle(e) {
		this.mask ^= 1 << e | 0;
	}
	disable(e) {
		this.mask &= ~(1 << e | 0);
	}
	disableAll() {
		this.mask = 0;
	}
	test(e) {
		return (this.mask & e.mask) !== 0;
	}
	isEnabled(e) {
		return (this.mask & (1 << e | 0)) != 0;
	}
}, Ai = 0, ji = /*@__PURE__*/ new q(), Mi = /*@__PURE__*/ new Xr(), Ni = /*@__PURE__*/ new vi(), Pi = /*@__PURE__*/ new q(), Fi = /*@__PURE__*/ new q(), Ii = /*@__PURE__*/ new q(), Li = /*@__PURE__*/ new Xr(), Ri = /*@__PURE__*/ new q(1, 0, 0), zi = /*@__PURE__*/ new q(0, 1, 0), Bi = /*@__PURE__*/ new q(0, 0, 1), Vi = { type: "added" }, Hi = { type: "removed" }, Ui = {
	type: "childadded",
	child: null
}, Wi = {
	type: "childremoved",
	child: null
}, Gi = class e extends Sr {
	constructor() {
		super(), this.isObject3D = !0, Object.defineProperty(this, "id", { value: Ai++ }), this.uuid = Dr(), this.name = "", this.type = "Object3D", this.parent = null, this.children = [], this.up = e.DEFAULT_UP.clone();
		let t = new q(), n = new Oi(), r = new Xr(), i = new q(1, 1, 1);
		function a() {
			r.setFromEuler(n, !1);
		}
		function o() {
			n.setFromQuaternion(r, void 0, !1);
		}
		n._onChange(a), r._onChange(o), Object.defineProperties(this, {
			position: {
				configurable: !0,
				enumerable: !0,
				value: t
			},
			rotation: {
				configurable: !0,
				enumerable: !0,
				value: n
			},
			quaternion: {
				configurable: !0,
				enumerable: !0,
				value: r
			},
			scale: {
				configurable: !0,
				enumerable: !0,
				value: i
			},
			modelViewMatrix: { value: new vi() },
			normalMatrix: { value: new J() }
		}), this.matrix = new vi(), this.matrixWorld = new vi(), this.matrixAutoUpdate = e.DEFAULT_MATRIX_AUTO_UPDATE, this.matrixWorldAutoUpdate = e.DEFAULT_MATRIX_WORLD_AUTO_UPDATE, this.matrixWorldNeedsUpdate = !1, this.layers = new ki(), this.visible = !0, this.castShadow = !1, this.receiveShadow = !1, this.frustumCulled = !0, this.renderOrder = 0, this.animations = [], this.customDepthMaterial = void 0, this.customDistanceMaterial = void 0, this.static = !1, this.userData = {}, this.pivot = null;
	}
	onBeforeShadow() {}
	onAfterShadow() {}
	onBeforeRender() {}
	onAfterRender() {}
	applyMatrix4(e) {
		this.matrixAutoUpdate && this.updateMatrix(), this.matrix.premultiply(e), this.matrix.decompose(this.position, this.quaternion, this.scale);
	}
	applyQuaternion(e) {
		return this.quaternion.premultiply(e), this;
	}
	setRotationFromAxisAngle(e, t) {
		this.quaternion.setFromAxisAngle(e, t);
	}
	setRotationFromEuler(e) {
		this.quaternion.setFromEuler(e, !0);
	}
	setRotationFromMatrix(e) {
		this.quaternion.setFromRotationMatrix(e);
	}
	setRotationFromQuaternion(e) {
		this.quaternion.copy(e);
	}
	rotateOnAxis(e, t) {
		return Mi.setFromAxisAngle(e, t), this.quaternion.multiply(Mi), this;
	}
	rotateOnWorldAxis(e, t) {
		return Mi.setFromAxisAngle(e, t), this.quaternion.premultiply(Mi), this;
	}
	rotateX(e) {
		return this.rotateOnAxis(Ri, e);
	}
	rotateY(e) {
		return this.rotateOnAxis(zi, e);
	}
	rotateZ(e) {
		return this.rotateOnAxis(Bi, e);
	}
	translateOnAxis(e, t) {
		return ji.copy(e).applyQuaternion(this.quaternion), this.position.add(ji.multiplyScalar(t)), this;
	}
	translateX(e) {
		return this.translateOnAxis(Ri, e);
	}
	translateY(e) {
		return this.translateOnAxis(zi, e);
	}
	translateZ(e) {
		return this.translateOnAxis(Bi, e);
	}
	localToWorld(e) {
		return this.updateWorldMatrix(!0, !1), e.applyMatrix4(this.matrixWorld);
	}
	worldToLocal(e) {
		return this.updateWorldMatrix(!0, !1), e.applyMatrix4(Ni.copy(this.matrixWorld).invert());
	}
	lookAt(e, t, n) {
		e.isVector3 ? Pi.copy(e) : Pi.set(e, t, n);
		let r = this.parent;
		this.updateWorldMatrix(!0, !1), Fi.setFromMatrixPosition(this.matrixWorld), this.isCamera || this.isLight ? Ni.lookAt(Fi, Pi, this.up) : Ni.lookAt(Pi, Fi, this.up), this.quaternion.setFromRotationMatrix(Ni), r && (Ni.extractRotation(r.matrixWorld), Mi.setFromRotationMatrix(Ni), this.quaternion.premultiply(Mi.invert()));
	}
	add(e) {
		if (arguments.length > 1) {
			for (let e = 0; e < arguments.length; e++) this.add(arguments[e]);
			return this;
		}
		return e === this ? (G("Object3D.add: object can't be added as a child of itself.", e), this) : (e && e.isObject3D ? (e.removeFromParent(), e.parent = this, this.children.push(e), e.dispatchEvent(Vi), Ui.child = e, this.dispatchEvent(Ui), Ui.child = null) : G("Object3D.add: object not an instance of THREE.Object3D.", e), this);
	}
	remove(e) {
		if (arguments.length > 1) {
			for (let e = 0; e < arguments.length; e++) this.remove(arguments[e]);
			return this;
		}
		let t = this.children.indexOf(e);
		return t !== -1 && (e.parent = null, this.children.splice(t, 1), e.dispatchEvent(Hi), Wi.child = e, this.dispatchEvent(Wi), Wi.child = null), this;
	}
	removeFromParent() {
		let e = this.parent;
		return e !== null && e.remove(this), this;
	}
	clear() {
		return this.remove(...this.children);
	}
	attach(e) {
		return this.updateWorldMatrix(!0, !1), Ni.copy(this.matrixWorld).invert(), e.parent !== null && (e.parent.updateWorldMatrix(!0, !1), Ni.multiply(e.parent.matrixWorld)), e.applyMatrix4(Ni), e.removeFromParent(), e.parent = this, this.children.push(e), e.updateWorldMatrix(!1, !0), e.dispatchEvent(Vi), Ui.child = e, this.dispatchEvent(Ui), Ui.child = null, this;
	}
	getObjectById(e) {
		return this.getObjectByProperty("id", e);
	}
	getObjectByName(e) {
		return this.getObjectByProperty("name", e);
	}
	getObjectByProperty(e, t) {
		if (this[e] === t) return this;
		for (let n = 0, r = this.children.length; n < r; n++) {
			let r = this.children[n].getObjectByProperty(e, t);
			if (r !== void 0) return r;
		}
	}
	getObjectsByProperty(e, t, n = []) {
		this[e] === t && n.push(this);
		let r = this.children;
		for (let i = 0, a = r.length; i < a; i++) r[i].getObjectsByProperty(e, t, n);
		return n;
	}
	getWorldPosition(e) {
		return this.updateWorldMatrix(!0, !1), e.setFromMatrixPosition(this.matrixWorld);
	}
	getWorldQuaternion(e) {
		return this.updateWorldMatrix(!0, !1), this.matrixWorld.decompose(Fi, e, Ii), e;
	}
	getWorldScale(e) {
		return this.updateWorldMatrix(!0, !1), this.matrixWorld.decompose(Fi, Li, e), e;
	}
	getWorldDirection(e) {
		this.updateWorldMatrix(!0, !1);
		let t = this.matrixWorld.elements;
		return e.set(t[8], t[9], t[10]).normalize();
	}
	raycast() {}
	traverse(e) {
		e(this);
		let t = this.children;
		for (let n = 0, r = t.length; n < r; n++) t[n].traverse(e);
	}
	traverseVisible(e) {
		if (this.visible === !1) return;
		e(this);
		let t = this.children;
		for (let n = 0, r = t.length; n < r; n++) t[n].traverseVisible(e);
	}
	traverseAncestors(e) {
		let t = this.parent;
		t !== null && (e(t), t.traverseAncestors(e));
	}
	updateMatrix() {
		this.matrix.compose(this.position, this.quaternion, this.scale);
		let e = this.pivot;
		if (e !== null) {
			let t = e.x, n = e.y, r = e.z, i = this.matrix.elements;
			i[12] += t - i[0] * t - i[4] * n - i[8] * r, i[13] += n - i[1] * t - i[5] * n - i[9] * r, i[14] += r - i[2] * t - i[6] * n - i[10] * r;
		}
		this.matrixWorldNeedsUpdate = !0;
	}
	updateMatrixWorld(e) {
		this.matrixAutoUpdate && this.updateMatrix(), (this.matrixWorldNeedsUpdate || e) && (this.matrixWorldAutoUpdate === !0 && (this.parent === null ? this.matrixWorld.copy(this.matrix) : this.matrixWorld.multiplyMatrices(this.parent.matrixWorld, this.matrix)), this.matrixWorldNeedsUpdate = !1, e = !0);
		let t = this.children;
		for (let n = 0, r = t.length; n < r; n++) t[n].updateMatrixWorld(e);
	}
	updateWorldMatrix(e, t) {
		let n = this.parent;
		if (e === !0 && n !== null && n.updateWorldMatrix(!0, !1), this.matrixAutoUpdate && this.updateMatrix(), this.matrixWorldAutoUpdate === !0 && (this.parent === null ? this.matrixWorld.copy(this.matrix) : this.matrixWorld.multiplyMatrices(this.parent.matrixWorld, this.matrix)), t === !0) {
			let e = this.children;
			for (let t = 0, n = e.length; t < n; t++) e[t].updateWorldMatrix(!1, !0);
		}
	}
	toJSON(e) {
		let t = e === void 0 || typeof e == "string", n = {};
		t && (e = {
			geometries: {},
			materials: {},
			textures: {},
			images: {},
			shapes: {},
			skeletons: {},
			animations: {},
			nodes: {}
		}, n.metadata = {
			version: 4.7,
			type: "Object",
			generator: "Object3D.toJSON"
		});
		let r = {};
		r.uuid = this.uuid, r.type = this.type, this.name !== "" && (r.name = this.name), this.castShadow === !0 && (r.castShadow = !0), this.receiveShadow === !0 && (r.receiveShadow = !0), this.visible === !1 && (r.visible = !1), this.frustumCulled === !1 && (r.frustumCulled = !1), this.renderOrder !== 0 && (r.renderOrder = this.renderOrder), this.static !== !1 && (r.static = this.static), Object.keys(this.userData).length > 0 && (r.userData = this.userData), r.layers = this.layers.mask, r.matrix = this.matrix.toArray(), r.up = this.up.toArray(), this.pivot !== null && (r.pivot = this.pivot.toArray()), this.matrixAutoUpdate === !1 && (r.matrixAutoUpdate = !1), this.morphTargetDictionary !== void 0 && (r.morphTargetDictionary = Object.assign({}, this.morphTargetDictionary)), this.morphTargetInfluences !== void 0 && (r.morphTargetInfluences = this.morphTargetInfluences.slice()), this.isInstancedMesh && (r.type = "InstancedMesh", r.count = this.count, r.instanceMatrix = this.instanceMatrix.toJSON(), this.instanceColor !== null && (r.instanceColor = this.instanceColor.toJSON())), this.isBatchedMesh && (r.type = "BatchedMesh", r.perObjectFrustumCulled = this.perObjectFrustumCulled, r.sortObjects = this.sortObjects, r.drawRanges = this._drawRanges, r.reservedRanges = this._reservedRanges, r.geometryInfo = this._geometryInfo.map((e) => ({
			...e,
			boundingBox: e.boundingBox ? e.boundingBox.toJSON() : void 0,
			boundingSphere: e.boundingSphere ? e.boundingSphere.toJSON() : void 0
		})), r.instanceInfo = this._instanceInfo.map((e) => ({ ...e })), r.availableInstanceIds = this._availableInstanceIds.slice(), r.availableGeometryIds = this._availableGeometryIds.slice(), r.nextIndexStart = this._nextIndexStart, r.nextVertexStart = this._nextVertexStart, r.geometryCount = this._geometryCount, r.maxInstanceCount = this._maxInstanceCount, r.maxVertexCount = this._maxVertexCount, r.maxIndexCount = this._maxIndexCount, r.geometryInitialized = this._geometryInitialized, r.matricesTexture = this._matricesTexture.toJSON(e), r.indirectTexture = this._indirectTexture.toJSON(e), this._colorsTexture !== null && (r.colorsTexture = this._colorsTexture.toJSON(e)), this.boundingSphere !== null && (r.boundingSphere = this.boundingSphere.toJSON()), this.boundingBox !== null && (r.boundingBox = this.boundingBox.toJSON()));
		function i(t, n) {
			return t[n.uuid] === void 0 && (t[n.uuid] = n.toJSON(e)), n.uuid;
		}
		if (this.isScene) this.background && (this.background.isColor ? r.background = this.background.toJSON() : this.background.isTexture && (r.background = this.background.toJSON(e).uuid)), this.environment && this.environment.isTexture && this.environment.isRenderTargetTexture !== !0 && (r.environment = this.environment.toJSON(e).uuid);
		else if (this.isMesh || this.isLine || this.isPoints) {
			r.geometry = i(e.geometries, this.geometry);
			let t = this.geometry.parameters;
			if (t !== void 0 && t.shapes !== void 0) {
				let n = t.shapes;
				if (Array.isArray(n)) for (let t = 0, r = n.length; t < r; t++) {
					let r = n[t];
					i(e.shapes, r);
				}
				else i(e.shapes, n);
			}
		}
		if (this.isSkinnedMesh && (r.bindMode = this.bindMode, r.bindMatrix = this.bindMatrix.toArray(), this.skeleton !== void 0 && (i(e.skeletons, this.skeleton), r.skeleton = this.skeleton.uuid)), this.material !== void 0) if (Array.isArray(this.material)) {
			let t = [];
			for (let n = 0, r = this.material.length; n < r; n++) t.push(i(e.materials, this.material[n]));
			r.material = t;
		} else r.material = i(e.materials, this.material);
		if (this.children.length > 0) {
			r.children = [];
			for (let t = 0; t < this.children.length; t++) r.children.push(this.children[t].toJSON(e).object);
		}
		if (this.animations.length > 0) {
			r.animations = [];
			for (let t = 0; t < this.animations.length; t++) {
				let n = this.animations[t];
				r.animations.push(i(e.animations, n));
			}
		}
		if (t) {
			let t = a(e.geometries), r = a(e.materials), i = a(e.textures), o = a(e.images), s = a(e.shapes), c = a(e.skeletons), l = a(e.animations), u = a(e.nodes);
			t.length > 0 && (n.geometries = t), r.length > 0 && (n.materials = r), i.length > 0 && (n.textures = i), o.length > 0 && (n.images = o), s.length > 0 && (n.shapes = s), c.length > 0 && (n.skeletons = c), l.length > 0 && (n.animations = l), u.length > 0 && (n.nodes = u);
		}
		return n.object = r, n;
		function a(e) {
			let t = [];
			for (let n in e) {
				let r = e[n];
				delete r.metadata, t.push(r);
			}
			return t;
		}
	}
	clone(e) {
		return new this.constructor().copy(this, e);
	}
	copy(e, t = !0) {
		if (this.name = e.name, this.up.copy(e.up), this.position.copy(e.position), this.rotation.order = e.rotation.order, this.quaternion.copy(e.quaternion), this.scale.copy(e.scale), this.pivot = e.pivot === null ? null : e.pivot.clone(), this.matrix.copy(e.matrix), this.matrixWorld.copy(e.matrixWorld), this.matrixAutoUpdate = e.matrixAutoUpdate, this.matrixWorldAutoUpdate = e.matrixWorldAutoUpdate, this.matrixWorldNeedsUpdate = e.matrixWorldNeedsUpdate, this.layers.mask = e.layers.mask, this.visible = e.visible, this.castShadow = e.castShadow, this.receiveShadow = e.receiveShadow, this.frustumCulled = e.frustumCulled, this.renderOrder = e.renderOrder, this.static = e.static, this.animations = e.animations.slice(), this.userData = JSON.parse(JSON.stringify(e.userData)), t === !0) for (let t = 0; t < e.children.length; t++) {
			let n = e.children[t];
			this.add(n.clone());
		}
		return this;
	}
};
Gi.DEFAULT_UP = /*@__PURE__*/ new q(0, 1, 0), Gi.DEFAULT_MATRIX_AUTO_UPDATE = !0, Gi.DEFAULT_MATRIX_WORLD_AUTO_UPDATE = !0;
var Ki = class extends Gi {
	constructor() {
		super(), this.isGroup = !0, this.type = "Group";
	}
}, qi = { type: "move" }, Ji = class {
	constructor() {
		this._targetRay = null, this._grip = null, this._hand = null;
	}
	getHandSpace() {
		return this._hand === null && (this._hand = new Ki(), this._hand.matrixAutoUpdate = !1, this._hand.visible = !1, this._hand.joints = {}, this._hand.inputState = { pinching: !1 }), this._hand;
	}
	getTargetRaySpace() {
		return this._targetRay === null && (this._targetRay = new Ki(), this._targetRay.matrixAutoUpdate = !1, this._targetRay.visible = !1, this._targetRay.hasLinearVelocity = !1, this._targetRay.linearVelocity = new q(), this._targetRay.hasAngularVelocity = !1, this._targetRay.angularVelocity = new q()), this._targetRay;
	}
	getGripSpace() {
		return this._grip === null && (this._grip = new Ki(), this._grip.matrixAutoUpdate = !1, this._grip.visible = !1, this._grip.hasLinearVelocity = !1, this._grip.linearVelocity = new q(), this._grip.hasAngularVelocity = !1, this._grip.angularVelocity = new q(), this._grip.eventsEnabled = !1), this._grip;
	}
	dispatchEvent(e) {
		return this._targetRay !== null && this._targetRay.dispatchEvent(e), this._grip !== null && this._grip.dispatchEvent(e), this._hand !== null && this._hand.dispatchEvent(e), this;
	}
	connect(e) {
		if (e && e.hand) {
			let t = this._hand;
			if (t) for (let n of e.hand.values()) this._getHandJoint(t, n);
		}
		return this.dispatchEvent({
			type: "connected",
			data: e
		}), this;
	}
	disconnect(e) {
		return this.dispatchEvent({
			type: "disconnected",
			data: e
		}), this._targetRay !== null && (this._targetRay.visible = !1), this._grip !== null && (this._grip.visible = !1), this._hand !== null && (this._hand.visible = !1), this;
	}
	update(e, t, n) {
		let r = null, i = null, a = null, o = this._targetRay, s = this._grip, c = this._hand;
		if (e && t.session.visibilityState !== "visible-blurred") {
			if (c && e.hand) {
				a = !0;
				for (let r of e.hand.values()) {
					let e = t.getJointPose(r, n), i = this._getHandJoint(c, r);
					e !== null && (i.matrix.fromArray(e.transform.matrix), i.matrix.decompose(i.position, i.rotation, i.scale), i.matrixWorldNeedsUpdate = !0, i.jointRadius = e.radius), i.visible = e !== null;
				}
				let r = c.joints["index-finger-tip"], i = c.joints["thumb-tip"], o = r.position.distanceTo(i.position);
				c.inputState.pinching && o > .025 ? (c.inputState.pinching = !1, this.dispatchEvent({
					type: "pinchend",
					handedness: e.handedness,
					target: this
				})) : !c.inputState.pinching && o <= .015 && (c.inputState.pinching = !0, this.dispatchEvent({
					type: "pinchstart",
					handedness: e.handedness,
					target: this
				}));
			} else s !== null && e.gripSpace && (i = t.getPose(e.gripSpace, n), i !== null && (s.matrix.fromArray(i.transform.matrix), s.matrix.decompose(s.position, s.rotation, s.scale), s.matrixWorldNeedsUpdate = !0, i.linearVelocity ? (s.hasLinearVelocity = !0, s.linearVelocity.copy(i.linearVelocity)) : s.hasLinearVelocity = !1, i.angularVelocity ? (s.hasAngularVelocity = !0, s.angularVelocity.copy(i.angularVelocity)) : s.hasAngularVelocity = !1, s.eventsEnabled && s.dispatchEvent({
				type: "gripUpdated",
				data: e,
				target: this
			})));
			o !== null && (r = t.getPose(e.targetRaySpace, n), r === null && i !== null && (r = i), r !== null && (o.matrix.fromArray(r.transform.matrix), o.matrix.decompose(o.position, o.rotation, o.scale), o.matrixWorldNeedsUpdate = !0, r.linearVelocity ? (o.hasLinearVelocity = !0, o.linearVelocity.copy(r.linearVelocity)) : o.hasLinearVelocity = !1, r.angularVelocity ? (o.hasAngularVelocity = !0, o.angularVelocity.copy(r.angularVelocity)) : o.hasAngularVelocity = !1, this.dispatchEvent(qi)));
		}
		return o !== null && (o.visible = r !== null), s !== null && (s.visible = i !== null), c !== null && (c.visible = a !== null), this;
	}
	_getHandJoint(e, t) {
		if (e.joints[t.jointName] === void 0) {
			let n = new Ki();
			n.matrixAutoUpdate = !1, n.visible = !1, e.joints[t.jointName] = n, e.add(n);
		}
		return e.joints[t.jointName];
	}
}, Yi = {
	aliceblue: 15792383,
	antiquewhite: 16444375,
	aqua: 65535,
	aquamarine: 8388564,
	azure: 15794175,
	beige: 16119260,
	bisque: 16770244,
	black: 0,
	blanchedalmond: 16772045,
	blue: 255,
	blueviolet: 9055202,
	brown: 10824234,
	burlywood: 14596231,
	cadetblue: 6266528,
	chartreuse: 8388352,
	chocolate: 13789470,
	coral: 16744272,
	cornflowerblue: 6591981,
	cornsilk: 16775388,
	crimson: 14423100,
	cyan: 65535,
	darkblue: 139,
	darkcyan: 35723,
	darkgoldenrod: 12092939,
	darkgray: 11119017,
	darkgreen: 25600,
	darkgrey: 11119017,
	darkkhaki: 12433259,
	darkmagenta: 9109643,
	darkolivegreen: 5597999,
	darkorange: 16747520,
	darkorchid: 10040012,
	darkred: 9109504,
	darksalmon: 15308410,
	darkseagreen: 9419919,
	darkslateblue: 4734347,
	darkslategray: 3100495,
	darkslategrey: 3100495,
	darkturquoise: 52945,
	darkviolet: 9699539,
	deeppink: 16716947,
	deepskyblue: 49151,
	dimgray: 6908265,
	dimgrey: 6908265,
	dodgerblue: 2003199,
	firebrick: 11674146,
	floralwhite: 16775920,
	forestgreen: 2263842,
	fuchsia: 16711935,
	gainsboro: 14474460,
	ghostwhite: 16316671,
	gold: 16766720,
	goldenrod: 14329120,
	gray: 8421504,
	green: 32768,
	greenyellow: 11403055,
	grey: 8421504,
	honeydew: 15794160,
	hotpink: 16738740,
	indianred: 13458524,
	indigo: 4915330,
	ivory: 16777200,
	khaki: 15787660,
	lavender: 15132410,
	lavenderblush: 16773365,
	lawngreen: 8190976,
	lemonchiffon: 16775885,
	lightblue: 11393254,
	lightcoral: 15761536,
	lightcyan: 14745599,
	lightgoldenrodyellow: 16448210,
	lightgray: 13882323,
	lightgreen: 9498256,
	lightgrey: 13882323,
	lightpink: 16758465,
	lightsalmon: 16752762,
	lightseagreen: 2142890,
	lightskyblue: 8900346,
	lightslategray: 7833753,
	lightslategrey: 7833753,
	lightsteelblue: 11584734,
	lightyellow: 16777184,
	lime: 65280,
	limegreen: 3329330,
	linen: 16445670,
	magenta: 16711935,
	maroon: 8388608,
	mediumaquamarine: 6737322,
	mediumblue: 205,
	mediumorchid: 12211667,
	mediumpurple: 9662683,
	mediumseagreen: 3978097,
	mediumslateblue: 8087790,
	mediumspringgreen: 64154,
	mediumturquoise: 4772300,
	mediumvioletred: 13047173,
	midnightblue: 1644912,
	mintcream: 16121850,
	mistyrose: 16770273,
	moccasin: 16770229,
	navajowhite: 16768685,
	navy: 128,
	oldlace: 16643558,
	olive: 8421376,
	olivedrab: 7048739,
	orange: 16753920,
	orangered: 16729344,
	orchid: 14315734,
	palegoldenrod: 15657130,
	palegreen: 10025880,
	paleturquoise: 11529966,
	palevioletred: 14381203,
	papayawhip: 16773077,
	peachpuff: 16767673,
	peru: 13468991,
	pink: 16761035,
	plum: 14524637,
	powderblue: 11591910,
	purple: 8388736,
	rebeccapurple: 6697881,
	red: 16711680,
	rosybrown: 12357519,
	royalblue: 4286945,
	saddlebrown: 9127187,
	salmon: 16416882,
	sandybrown: 16032864,
	seagreen: 3050327,
	seashell: 16774638,
	sienna: 10506797,
	silver: 12632256,
	skyblue: 8900331,
	slateblue: 6970061,
	slategray: 7372944,
	slategrey: 7372944,
	snow: 16775930,
	springgreen: 65407,
	steelblue: 4620980,
	tan: 13808780,
	teal: 32896,
	thistle: 14204888,
	tomato: 16737095,
	turquoise: 4251856,
	violet: 15631086,
	wheat: 16113331,
	white: 16777215,
	whitesmoke: 16119285,
	yellow: 16776960,
	yellowgreen: 10145074
}, Xi = {
	h: 0,
	s: 0,
	l: 0
}, Zi = {
	h: 0,
	s: 0,
	l: 0
};
function Qi(e, t, n) {
	return n < 0 && (n += 1), n > 1 && --n, n < 1 / 6 ? e + (t - e) * 6 * n : n < 1 / 2 ? t : n < 2 / 3 ? e + (t - e) * 6 * (2 / 3 - n) : e;
}
var X = class {
	constructor(e, t, n) {
		return this.isColor = !0, this.r = 1, this.g = 1, this.b = 1, this.set(e, t, n);
	}
	set(e, t, n) {
		if (t === void 0 && n === void 0) {
			let t = e;
			t && t.isColor ? this.copy(t) : typeof t == "number" ? this.setHex(t) : typeof t == "string" && this.setStyle(t);
		} else this.setRGB(e, t, n);
		return this;
	}
	setScalar(e) {
		return this.r = e, this.g = e, this.b = e, this;
	}
	setHex(e, t = ir) {
		return e = Math.floor(e), this.r = (e >> 16 & 255) / 255, this.g = (e >> 8 & 255) / 255, this.b = (e & 255) / 255, Y.colorSpaceToWorking(this, t), this;
	}
	setRGB(e, t, n, r = Y.workingColorSpace) {
		return this.r = e, this.g = t, this.b = n, Y.colorSpaceToWorking(this, r), this;
	}
	setHSL(e, t, n, r = Y.workingColorSpace) {
		if (e = Or(e, 1), t = K(t, 0, 1), n = K(n, 0, 1), t === 0) this.r = this.g = this.b = n;
		else {
			let r = n <= .5 ? n * (1 + t) : n + t - n * t, i = 2 * n - r;
			this.r = Qi(i, r, e + 1 / 3), this.g = Qi(i, r, e), this.b = Qi(i, r, e - 1 / 3);
		}
		return Y.colorSpaceToWorking(this, r), this;
	}
	setStyle(e, t = ir) {
		function n(t) {
			t !== void 0 && parseFloat(t) < 1 && W("Color: Alpha component of " + e + " will be ignored.");
		}
		let r;
		if (r = /^(\w+)\(([^\)]*)\)/.exec(e)) {
			let i, a = r[1], o = r[2];
			switch (a) {
				case "rgb":
				case "rgba":
					if (i = /^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o)) return n(i[4]), this.setRGB(Math.min(255, parseInt(i[1], 10)) / 255, Math.min(255, parseInt(i[2], 10)) / 255, Math.min(255, parseInt(i[3], 10)) / 255, t);
					if (i = /^\s*(\d+)\%\s*,\s*(\d+)\%\s*,\s*(\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o)) return n(i[4]), this.setRGB(Math.min(100, parseInt(i[1], 10)) / 100, Math.min(100, parseInt(i[2], 10)) / 100, Math.min(100, parseInt(i[3], 10)) / 100, t);
					break;
				case "hsl":
				case "hsla":
					if (i = /^\s*(\d*\.?\d+)\s*,\s*(\d*\.?\d+)\%\s*,\s*(\d*\.?\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o)) return n(i[4]), this.setHSL(parseFloat(i[1]) / 360, parseFloat(i[2]) / 100, parseFloat(i[3]) / 100, t);
					break;
				default: W("Color: Unknown color model " + e);
			}
		} else if (r = /^\#([A-Fa-f\d]+)$/.exec(e)) {
			let n = r[1], i = n.length;
			if (i === 3) return this.setRGB(parseInt(n.charAt(0), 16) / 15, parseInt(n.charAt(1), 16) / 15, parseInt(n.charAt(2), 16) / 15, t);
			if (i === 6) return this.setHex(parseInt(n, 16), t);
			W("Color: Invalid hex color " + e);
		} else if (e && e.length > 0) return this.setColorName(e, t);
		return this;
	}
	setColorName(e, t = ir) {
		let n = Yi[e.toLowerCase()];
		return n === void 0 ? W("Color: Unknown color " + e) : this.setHex(n, t), this;
	}
	clone() {
		return new this.constructor(this.r, this.g, this.b);
	}
	copy(e) {
		return this.r = e.r, this.g = e.g, this.b = e.b, this;
	}
	copySRGBToLinear(e) {
		return this.r = ri(e.r), this.g = ri(e.g), this.b = ri(e.b), this;
	}
	copyLinearToSRGB(e) {
		return this.r = ii(e.r), this.g = ii(e.g), this.b = ii(e.b), this;
	}
	convertSRGBToLinear() {
		return this.copySRGBToLinear(this), this;
	}
	convertLinearToSRGB() {
		return this.copyLinearToSRGB(this), this;
	}
	getHex(e = ir) {
		return Y.workingToColorSpace($i.copy(this), e), Math.round(K($i.r * 255, 0, 255)) * 65536 + Math.round(K($i.g * 255, 0, 255)) * 256 + Math.round(K($i.b * 255, 0, 255));
	}
	getHexString(e = ir) {
		return ("000000" + this.getHex(e).toString(16)).slice(-6);
	}
	getHSL(e, t = Y.workingColorSpace) {
		Y.workingToColorSpace($i.copy(this), t);
		let n = $i.r, r = $i.g, i = $i.b, a = Math.max(n, r, i), o = Math.min(n, r, i), s, c, l = (o + a) / 2;
		if (o === a) s = 0, c = 0;
		else {
			let e = a - o;
			switch (c = l <= .5 ? e / (a + o) : e / (2 - a - o), a) {
				case n:
					s = (r - i) / e + (r < i ? 6 : 0);
					break;
				case r:
					s = (i - n) / e + 2;
					break;
				case i:
					s = (n - r) / e + 4;
					break;
			}
			s /= 6;
		}
		return e.h = s, e.s = c, e.l = l, e;
	}
	getRGB(e, t = Y.workingColorSpace) {
		return Y.workingToColorSpace($i.copy(this), t), e.r = $i.r, e.g = $i.g, e.b = $i.b, e;
	}
	getStyle(e = ir) {
		Y.workingToColorSpace($i.copy(this), e);
		let t = $i.r, n = $i.g, r = $i.b;
		return e === "srgb" ? `rgb(${Math.round(t * 255)},${Math.round(n * 255)},${Math.round(r * 255)})` : `color(${e} ${t.toFixed(3)} ${n.toFixed(3)} ${r.toFixed(3)})`;
	}
	offsetHSL(e, t, n) {
		return this.getHSL(Xi), this.setHSL(Xi.h + e, Xi.s + t, Xi.l + n);
	}
	add(e) {
		return this.r += e.r, this.g += e.g, this.b += e.b, this;
	}
	addColors(e, t) {
		return this.r = e.r + t.r, this.g = e.g + t.g, this.b = e.b + t.b, this;
	}
	addScalar(e) {
		return this.r += e, this.g += e, this.b += e, this;
	}
	sub(e) {
		return this.r = Math.max(0, this.r - e.r), this.g = Math.max(0, this.g - e.g), this.b = Math.max(0, this.b - e.b), this;
	}
	multiply(e) {
		return this.r *= e.r, this.g *= e.g, this.b *= e.b, this;
	}
	multiplyScalar(e) {
		return this.r *= e, this.g *= e, this.b *= e, this;
	}
	lerp(e, t) {
		return this.r += (e.r - this.r) * t, this.g += (e.g - this.g) * t, this.b += (e.b - this.b) * t, this;
	}
	lerpColors(e, t, n) {
		return this.r = e.r + (t.r - e.r) * n, this.g = e.g + (t.g - e.g) * n, this.b = e.b + (t.b - e.b) * n, this;
	}
	lerpHSL(e, t) {
		this.getHSL(Xi), e.getHSL(Zi);
		let n = jr(Xi.h, Zi.h, t), r = jr(Xi.s, Zi.s, t), i = jr(Xi.l, Zi.l, t);
		return this.setHSL(n, r, i), this;
	}
	setFromVector3(e) {
		return this.r = e.x, this.g = e.y, this.b = e.z, this;
	}
	applyMatrix3(e) {
		let t = this.r, n = this.g, r = this.b, i = e.elements;
		return this.r = i[0] * t + i[3] * n + i[6] * r, this.g = i[1] * t + i[4] * n + i[7] * r, this.b = i[2] * t + i[5] * n + i[8] * r, this;
	}
	equals(e) {
		return e.r === this.r && e.g === this.g && e.b === this.b;
	}
	fromArray(e, t = 0) {
		return this.r = e[t], this.g = e[t + 1], this.b = e[t + 2], this;
	}
	toArray(e = [], t = 0) {
		return e[t] = this.r, e[t + 1] = this.g, e[t + 2] = this.b, e;
	}
	fromBufferAttribute(e, t) {
		return this.r = e.getX(t), this.g = e.getY(t), this.b = e.getZ(t), this;
	}
	toJSON() {
		return this.getHex();
	}
	*[Symbol.iterator]() {
		yield this.r, yield this.g, yield this.b;
	}
}, $i = /*@__PURE__*/ new X();
X.NAMES = Yi;
var ea = class extends Gi {
	constructor() {
		super(), this.isScene = !0, this.type = "Scene", this.background = null, this.environment = null, this.fog = null, this.backgroundBlurriness = 0, this.backgroundIntensity = 1, this.backgroundRotation = new Oi(), this.environmentIntensity = 1, this.environmentRotation = new Oi(), this.overrideMaterial = null, typeof __THREE_DEVTOOLS__ < "u" && __THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe", { detail: this }));
	}
	copy(e, t) {
		return super.copy(e, t), e.background !== null && (this.background = e.background.clone()), e.environment !== null && (this.environment = e.environment.clone()), e.fog !== null && (this.fog = e.fog.clone()), this.backgroundBlurriness = e.backgroundBlurriness, this.backgroundIntensity = e.backgroundIntensity, this.backgroundRotation.copy(e.backgroundRotation), this.environmentIntensity = e.environmentIntensity, this.environmentRotation.copy(e.environmentRotation), e.overrideMaterial !== null && (this.overrideMaterial = e.overrideMaterial.clone()), this.matrixAutoUpdate = e.matrixAutoUpdate, this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return this.fog !== null && (t.object.fog = this.fog.toJSON()), this.backgroundBlurriness > 0 && (t.object.backgroundBlurriness = this.backgroundBlurriness), this.backgroundIntensity !== 1 && (t.object.backgroundIntensity = this.backgroundIntensity), t.object.backgroundRotation = this.backgroundRotation.toArray(), this.environmentIntensity !== 1 && (t.object.environmentIntensity = this.environmentIntensity), t.object.environmentRotation = this.environmentRotation.toArray(), t;
	}
}, ta = /*@__PURE__*/ new q(), na = /*@__PURE__*/ new q(), ra = /*@__PURE__*/ new q(), ia = /*@__PURE__*/ new q(), aa = /*@__PURE__*/ new q(), oa = /*@__PURE__*/ new q(), sa = /*@__PURE__*/ new q(), ca = /*@__PURE__*/ new q(), la = /*@__PURE__*/ new q(), ua = /*@__PURE__*/ new q(), da = /*@__PURE__*/ new pi(), fa = /*@__PURE__*/ new pi(), pa = /*@__PURE__*/ new pi(), ma = class e {
	constructor(e = new q(), t = new q(), n = new q()) {
		this.a = e, this.b = t, this.c = n;
	}
	static getNormal(e, t, n, r) {
		r.subVectors(n, t), ta.subVectors(e, t), r.cross(ta);
		let i = r.lengthSq();
		return i > 0 ? r.multiplyScalar(1 / Math.sqrt(i)) : r.set(0, 0, 0);
	}
	static getBarycoord(e, t, n, r, i) {
		ta.subVectors(r, t), na.subVectors(n, t), ra.subVectors(e, t);
		let a = ta.dot(ta), o = ta.dot(na), s = ta.dot(ra), c = na.dot(na), l = na.dot(ra), u = a * c - o * o;
		if (u === 0) return i.set(0, 0, 0), null;
		let d = 1 / u, f = (c * s - o * l) * d, p = (a * l - o * s) * d;
		return i.set(1 - f - p, p, f);
	}
	static containsPoint(e, t, n, r) {
		return this.getBarycoord(e, t, n, r, ia) !== null && ia.x >= 0 && ia.y >= 0 && ia.x + ia.y <= 1;
	}
	static getInterpolation(e, t, n, r, i, a, o, s) {
		return this.getBarycoord(e, t, n, r, ia) === null ? (s.x = 0, s.y = 0, "z" in s && (s.z = 0), "w" in s && (s.w = 0), null) : (s.setScalar(0), s.addScaledVector(i, ia.x), s.addScaledVector(a, ia.y), s.addScaledVector(o, ia.z), s);
	}
	static getInterpolatedAttribute(e, t, n, r, i, a) {
		return da.setScalar(0), fa.setScalar(0), pa.setScalar(0), da.fromBufferAttribute(e, t), fa.fromBufferAttribute(e, n), pa.fromBufferAttribute(e, r), a.setScalar(0), a.addScaledVector(da, i.x), a.addScaledVector(fa, i.y), a.addScaledVector(pa, i.z), a;
	}
	static isFrontFacing(e, t, n, r) {
		return ta.subVectors(n, t), na.subVectors(e, t), ta.cross(na).dot(r) < 0;
	}
	set(e, t, n) {
		return this.a.copy(e), this.b.copy(t), this.c.copy(n), this;
	}
	setFromPointsAndIndices(e, t, n, r) {
		return this.a.copy(e[t]), this.b.copy(e[n]), this.c.copy(e[r]), this;
	}
	setFromAttributeAndIndices(e, t, n, r) {
		return this.a.fromBufferAttribute(e, t), this.b.fromBufferAttribute(e, n), this.c.fromBufferAttribute(e, r), this;
	}
	clone() {
		return new this.constructor().copy(this);
	}
	copy(e) {
		return this.a.copy(e.a), this.b.copy(e.b), this.c.copy(e.c), this;
	}
	getArea() {
		return ta.subVectors(this.c, this.b), na.subVectors(this.a, this.b), ta.cross(na).length() * .5;
	}
	getMidpoint(e) {
		return e.addVectors(this.a, this.b).add(this.c).multiplyScalar(1 / 3);
	}
	getNormal(t) {
		return e.getNormal(this.a, this.b, this.c, t);
	}
	getPlane(e) {
		return e.setFromCoplanarPoints(this.a, this.b, this.c);
	}
	getBarycoord(t, n) {
		return e.getBarycoord(t, this.a, this.b, this.c, n);
	}
	getInterpolation(t, n, r, i, a) {
		return e.getInterpolation(t, this.a, this.b, this.c, n, r, i, a);
	}
	containsPoint(t) {
		return e.containsPoint(t, this.a, this.b, this.c);
	}
	isFrontFacing(t) {
		return e.isFrontFacing(this.a, this.b, this.c, t);
	}
	intersectsBox(e) {
		return e.intersectsTriangle(this);
	}
	closestPointToPoint(e, t) {
		let n = this.a, r = this.b, i = this.c, a, o;
		aa.subVectors(r, n), oa.subVectors(i, n), ca.subVectors(e, n);
		let s = aa.dot(ca), c = oa.dot(ca);
		if (s <= 0 && c <= 0) return t.copy(n);
		la.subVectors(e, r);
		let l = aa.dot(la), u = oa.dot(la);
		if (l >= 0 && u <= l) return t.copy(r);
		let d = s * u - l * c;
		if (d <= 0 && s >= 0 && l <= 0) return a = s / (s - l), t.copy(n).addScaledVector(aa, a);
		ua.subVectors(e, i);
		let f = aa.dot(ua), p = oa.dot(ua);
		if (p >= 0 && f <= p) return t.copy(i);
		let m = f * c - s * p;
		if (m <= 0 && c >= 0 && p <= 0) return o = c / (c - p), t.copy(n).addScaledVector(oa, o);
		let h = l * p - f * u;
		if (h <= 0 && u - l >= 0 && f - p >= 0) return sa.subVectors(i, r), o = (u - l) / (u - l + (f - p)), t.copy(r).addScaledVector(sa, o);
		let g = 1 / (h + m + d);
		return a = m * g, o = d * g, t.copy(n).addScaledVector(aa, a).addScaledVector(oa, o);
	}
	equals(e) {
		return e.a.equals(this.a) && e.b.equals(this.b) && e.c.equals(this.c);
	}
}, ha = class {
	constructor(e = new q(Infinity, Infinity, Infinity), t = new q(-Infinity, -Infinity, -Infinity)) {
		this.isBox3 = !0, this.min = e, this.max = t;
	}
	set(e, t) {
		return this.min.copy(e), this.max.copy(t), this;
	}
	setFromArray(e) {
		this.makeEmpty();
		for (let t = 0, n = e.length; t < n; t += 3) this.expandByPoint(_a.fromArray(e, t));
		return this;
	}
	setFromBufferAttribute(e) {
		this.makeEmpty();
		for (let t = 0, n = e.count; t < n; t++) this.expandByPoint(_a.fromBufferAttribute(e, t));
		return this;
	}
	setFromPoints(e) {
		this.makeEmpty();
		for (let t = 0, n = e.length; t < n; t++) this.expandByPoint(e[t]);
		return this;
	}
	setFromCenterAndSize(e, t) {
		let n = _a.copy(t).multiplyScalar(.5);
		return this.min.copy(e).sub(n), this.max.copy(e).add(n), this;
	}
	setFromObject(e, t = !1) {
		return this.makeEmpty(), this.expandByObject(e, t);
	}
	clone() {
		return new this.constructor().copy(this);
	}
	copy(e) {
		return this.min.copy(e.min), this.max.copy(e.max), this;
	}
	makeEmpty() {
		return this.min.x = this.min.y = this.min.z = Infinity, this.max.x = this.max.y = this.max.z = -Infinity, this;
	}
	isEmpty() {
		return this.max.x < this.min.x || this.max.y < this.min.y || this.max.z < this.min.z;
	}
	getCenter(e) {
		return this.isEmpty() ? e.set(0, 0, 0) : e.addVectors(this.min, this.max).multiplyScalar(.5);
	}
	getSize(e) {
		return this.isEmpty() ? e.set(0, 0, 0) : e.subVectors(this.max, this.min);
	}
	expandByPoint(e) {
		return this.min.min(e), this.max.max(e), this;
	}
	expandByVector(e) {
		return this.min.sub(e), this.max.add(e), this;
	}
	expandByScalar(e) {
		return this.min.addScalar(-e), this.max.addScalar(e), this;
	}
	expandByObject(e, t = !1) {
		e.updateWorldMatrix(!1, !1);
		let n = e.geometry;
		if (n !== void 0) {
			let r = n.getAttribute("position");
			if (t === !0 && r !== void 0 && e.isInstancedMesh !== !0) for (let t = 0, n = r.count; t < n; t++) e.isMesh === !0 ? e.getVertexPosition(t, _a) : _a.fromBufferAttribute(r, t), _a.applyMatrix4(e.matrixWorld), this.expandByPoint(_a);
			else e.boundingBox === void 0 ? (n.boundingBox === null && n.computeBoundingBox(), va.copy(n.boundingBox)) : (e.boundingBox === null && e.computeBoundingBox(), va.copy(e.boundingBox)), va.applyMatrix4(e.matrixWorld), this.union(va);
		}
		let r = e.children;
		for (let e = 0, n = r.length; e < n; e++) this.expandByObject(r[e], t);
		return this;
	}
	containsPoint(e) {
		return e.x >= this.min.x && e.x <= this.max.x && e.y >= this.min.y && e.y <= this.max.y && e.z >= this.min.z && e.z <= this.max.z;
	}
	containsBox(e) {
		return this.min.x <= e.min.x && e.max.x <= this.max.x && this.min.y <= e.min.y && e.max.y <= this.max.y && this.min.z <= e.min.z && e.max.z <= this.max.z;
	}
	getParameter(e, t) {
		return t.set((e.x - this.min.x) / (this.max.x - this.min.x), (e.y - this.min.y) / (this.max.y - this.min.y), (e.z - this.min.z) / (this.max.z - this.min.z));
	}
	intersectsBox(e) {
		return e.max.x >= this.min.x && e.min.x <= this.max.x && e.max.y >= this.min.y && e.min.y <= this.max.y && e.max.z >= this.min.z && e.min.z <= this.max.z;
	}
	intersectsSphere(e) {
		return this.clampPoint(e.center, _a), _a.distanceToSquared(e.center) <= e.radius * e.radius;
	}
	intersectsPlane(e) {
		let t, n;
		return e.normal.x > 0 ? (t = e.normal.x * this.min.x, n = e.normal.x * this.max.x) : (t = e.normal.x * this.max.x, n = e.normal.x * this.min.x), e.normal.y > 0 ? (t += e.normal.y * this.min.y, n += e.normal.y * this.max.y) : (t += e.normal.y * this.max.y, n += e.normal.y * this.min.y), e.normal.z > 0 ? (t += e.normal.z * this.min.z, n += e.normal.z * this.max.z) : (t += e.normal.z * this.max.z, n += e.normal.z * this.min.z), t <= -e.constant && n >= -e.constant;
	}
	intersectsTriangle(e) {
		if (this.isEmpty()) return !1;
		this.getCenter(Ta), Ea.subVectors(this.max, Ta), ya.subVectors(e.a, Ta), ba.subVectors(e.b, Ta), xa.subVectors(e.c, Ta), Sa.subVectors(ba, ya), Ca.subVectors(xa, ba), wa.subVectors(ya, xa);
		let t = [
			0,
			-Sa.z,
			Sa.y,
			0,
			-Ca.z,
			Ca.y,
			0,
			-wa.z,
			wa.y,
			Sa.z,
			0,
			-Sa.x,
			Ca.z,
			0,
			-Ca.x,
			wa.z,
			0,
			-wa.x,
			-Sa.y,
			Sa.x,
			0,
			-Ca.y,
			Ca.x,
			0,
			-wa.y,
			wa.x,
			0
		];
		return !ka(t, ya, ba, xa, Ea) || (t = [
			1,
			0,
			0,
			0,
			1,
			0,
			0,
			0,
			1
		], !ka(t, ya, ba, xa, Ea)) ? !1 : (Da.crossVectors(Sa, Ca), t = [
			Da.x,
			Da.y,
			Da.z
		], ka(t, ya, ba, xa, Ea));
	}
	clampPoint(e, t) {
		return t.copy(e).clamp(this.min, this.max);
	}
	distanceToPoint(e) {
		return this.clampPoint(e, _a).distanceTo(e);
	}
	getBoundingSphere(e) {
		return this.isEmpty() ? e.makeEmpty() : (this.getCenter(e.center), e.radius = this.getSize(_a).length() * .5), e;
	}
	intersect(e) {
		return this.min.max(e.min), this.max.min(e.max), this.isEmpty() && this.makeEmpty(), this;
	}
	union(e) {
		return this.min.min(e.min), this.max.max(e.max), this;
	}
	applyMatrix4(e) {
		return this.isEmpty() ? this : (ga[0].set(this.min.x, this.min.y, this.min.z).applyMatrix4(e), ga[1].set(this.min.x, this.min.y, this.max.z).applyMatrix4(e), ga[2].set(this.min.x, this.max.y, this.min.z).applyMatrix4(e), ga[3].set(this.min.x, this.max.y, this.max.z).applyMatrix4(e), ga[4].set(this.max.x, this.min.y, this.min.z).applyMatrix4(e), ga[5].set(this.max.x, this.min.y, this.max.z).applyMatrix4(e), ga[6].set(this.max.x, this.max.y, this.min.z).applyMatrix4(e), ga[7].set(this.max.x, this.max.y, this.max.z).applyMatrix4(e), this.setFromPoints(ga), this);
	}
	translate(e) {
		return this.min.add(e), this.max.add(e), this;
	}
	equals(e) {
		return e.min.equals(this.min) && e.max.equals(this.max);
	}
	toJSON() {
		return {
			min: this.min.toArray(),
			max: this.max.toArray()
		};
	}
	fromJSON(e) {
		return this.min.fromArray(e.min), this.max.fromArray(e.max), this;
	}
}, ga = [
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q(),
	/*@__PURE__*/ new q()
], _a = /*@__PURE__*/ new q(), va = /*@__PURE__*/ new ha(), ya = /*@__PURE__*/ new q(), ba = /*@__PURE__*/ new q(), xa = /*@__PURE__*/ new q(), Sa = /*@__PURE__*/ new q(), Ca = /*@__PURE__*/ new q(), wa = /*@__PURE__*/ new q(), Ta = /*@__PURE__*/ new q(), Ea = /*@__PURE__*/ new q(), Da = /*@__PURE__*/ new q(), Oa = /*@__PURE__*/ new q();
function ka(e, t, n, r, i) {
	for (let a = 0, o = e.length - 3; a <= o; a += 3) {
		Oa.fromArray(e, a);
		let o = i.x * Math.abs(Oa.x) + i.y * Math.abs(Oa.y) + i.z * Math.abs(Oa.z), s = t.dot(Oa), c = n.dot(Oa), l = r.dot(Oa);
		if (Math.max(-Math.max(s, c, l), Math.min(s, c, l)) > o) return !1;
	}
	return !0;
}
var Aa = /*@__PURE__*/ new q(), ja = /*@__PURE__*/ new Yr(), Ma = 0, Na = class extends Sr {
	constructor(e, t, n = !1) {
		if (super(), Array.isArray(e)) throw TypeError("THREE.BufferAttribute: array should be a Typed Array.");
		this.isBufferAttribute = !0, Object.defineProperty(this, "id", { value: Ma++ }), this.name = "", this.array = e, this.itemSize = t, this.count = e === void 0 ? 0 : e.length / t, this.normalized = n, this.usage = lr, this.updateRanges = [], this.gpuType = Gt, this.version = 0;
	}
	onUploadCallback() {}
	set needsUpdate(e) {
		e === !0 && this.version++;
	}
	setUsage(e) {
		return this.usage = e, this;
	}
	addUpdateRange(e, t) {
		this.updateRanges.push({
			start: e,
			count: t
		});
	}
	clearUpdateRanges() {
		this.updateRanges.length = 0;
	}
	copy(e) {
		return this.name = e.name, this.array = new e.array.constructor(e.array), this.itemSize = e.itemSize, this.count = e.count, this.normalized = e.normalized, this.usage = e.usage, this.gpuType = e.gpuType, this;
	}
	copyAt(e, t, n) {
		e *= this.itemSize, n *= t.itemSize;
		for (let r = 0, i = this.itemSize; r < i; r++) this.array[e + r] = t.array[n + r];
		return this;
	}
	copyArray(e) {
		return this.array.set(e), this;
	}
	applyMatrix3(e) {
		if (this.itemSize === 2) for (let t = 0, n = this.count; t < n; t++) ja.fromBufferAttribute(this, t), ja.applyMatrix3(e), this.setXY(t, ja.x, ja.y);
		else if (this.itemSize === 3) for (let t = 0, n = this.count; t < n; t++) Aa.fromBufferAttribute(this, t), Aa.applyMatrix3(e), this.setXYZ(t, Aa.x, Aa.y, Aa.z);
		return this;
	}
	applyMatrix4(e) {
		for (let t = 0, n = this.count; t < n; t++) Aa.fromBufferAttribute(this, t), Aa.applyMatrix4(e), this.setXYZ(t, Aa.x, Aa.y, Aa.z);
		return this;
	}
	applyNormalMatrix(e) {
		for (let t = 0, n = this.count; t < n; t++) Aa.fromBufferAttribute(this, t), Aa.applyNormalMatrix(e), this.setXYZ(t, Aa.x, Aa.y, Aa.z);
		return this;
	}
	transformDirection(e) {
		for (let t = 0, n = this.count; t < n; t++) Aa.fromBufferAttribute(this, t), Aa.transformDirection(e), this.setXYZ(t, Aa.x, Aa.y, Aa.z);
		return this;
	}
	set(e, t = 0) {
		return this.array.set(e, t), this;
	}
	getComponent(e, t) {
		let n = this.array[e * this.itemSize + t];
		return this.normalized && (n = Kr(n, this.array)), n;
	}
	setComponent(e, t, n) {
		return this.normalized && (n = qr(n, this.array)), this.array[e * this.itemSize + t] = n, this;
	}
	getX(e) {
		let t = this.array[e * this.itemSize];
		return this.normalized && (t = Kr(t, this.array)), t;
	}
	setX(e, t) {
		return this.normalized && (t = qr(t, this.array)), this.array[e * this.itemSize] = t, this;
	}
	getY(e) {
		let t = this.array[e * this.itemSize + 1];
		return this.normalized && (t = Kr(t, this.array)), t;
	}
	setY(e, t) {
		return this.normalized && (t = qr(t, this.array)), this.array[e * this.itemSize + 1] = t, this;
	}
	getZ(e) {
		let t = this.array[e * this.itemSize + 2];
		return this.normalized && (t = Kr(t, this.array)), t;
	}
	setZ(e, t) {
		return this.normalized && (t = qr(t, this.array)), this.array[e * this.itemSize + 2] = t, this;
	}
	getW(e) {
		let t = this.array[e * this.itemSize + 3];
		return this.normalized && (t = Kr(t, this.array)), t;
	}
	setW(e, t) {
		return this.normalized && (t = qr(t, this.array)), this.array[e * this.itemSize + 3] = t, this;
	}
	setXY(e, t, n) {
		return e *= this.itemSize, this.normalized && (t = qr(t, this.array), n = qr(n, this.array)), this.array[e + 0] = t, this.array[e + 1] = n, this;
	}
	setXYZ(e, t, n, r) {
		return e *= this.itemSize, this.normalized && (t = qr(t, this.array), n = qr(n, this.array), r = qr(r, this.array)), this.array[e + 0] = t, this.array[e + 1] = n, this.array[e + 2] = r, this;
	}
	setXYZW(e, t, n, r, i) {
		return e *= this.itemSize, this.normalized && (t = qr(t, this.array), n = qr(n, this.array), r = qr(r, this.array), i = qr(i, this.array)), this.array[e + 0] = t, this.array[e + 1] = n, this.array[e + 2] = r, this.array[e + 3] = i, this;
	}
	onUpload(e) {
		return this.onUploadCallback = e, this;
	}
	clone() {
		return new this.constructor(this.array, this.itemSize).copy(this);
	}
	toJSON() {
		let e = {
			itemSize: this.itemSize,
			type: this.array.constructor.name,
			array: Array.from(this.array),
			normalized: this.normalized
		};
		return this.name !== "" && (e.name = this.name), this.usage !== 35044 && (e.usage = this.usage), e;
	}
	dispose() {
		this.dispatchEvent({ type: "dispose" });
	}
}, Pa = class extends Na {
	constructor(e, t, n) {
		super(new Uint16Array(e), t, n);
	}
}, Fa = class extends Na {
	constructor(e, t, n) {
		super(new Uint32Array(e), t, n);
	}
}, Ia = class extends Na {
	constructor(e, t, n) {
		super(new Float32Array(e), t, n);
	}
}, La = /*@__PURE__*/ new ha(), Ra = /*@__PURE__*/ new q(), za = /*@__PURE__*/ new q(), Ba = class {
	constructor(e = new q(), t = -1) {
		this.isSphere = !0, this.center = e, this.radius = t;
	}
	set(e, t) {
		return this.center.copy(e), this.radius = t, this;
	}
	setFromPoints(e, t) {
		let n = this.center;
		t === void 0 ? La.setFromPoints(e).getCenter(n) : n.copy(t);
		let r = 0;
		for (let t = 0, i = e.length; t < i; t++) r = Math.max(r, n.distanceToSquared(e[t]));
		return this.radius = Math.sqrt(r), this;
	}
	copy(e) {
		return this.center.copy(e.center), this.radius = e.radius, this;
	}
	isEmpty() {
		return this.radius < 0;
	}
	makeEmpty() {
		return this.center.set(0, 0, 0), this.radius = -1, this;
	}
	containsPoint(e) {
		return e.distanceToSquared(this.center) <= this.radius * this.radius;
	}
	distanceToPoint(e) {
		return e.distanceTo(this.center) - this.radius;
	}
	intersectsSphere(e) {
		let t = this.radius + e.radius;
		return e.center.distanceToSquared(this.center) <= t * t;
	}
	intersectsBox(e) {
		return e.intersectsSphere(this);
	}
	intersectsPlane(e) {
		return Math.abs(e.distanceToPoint(this.center)) <= this.radius;
	}
	clampPoint(e, t) {
		let n = this.center.distanceToSquared(e);
		return t.copy(e), n > this.radius * this.radius && (t.sub(this.center).normalize(), t.multiplyScalar(this.radius).add(this.center)), t;
	}
	getBoundingBox(e) {
		return this.isEmpty() ? (e.makeEmpty(), e) : (e.set(this.center, this.center), e.expandByScalar(this.radius), e);
	}
	applyMatrix4(e) {
		return this.center.applyMatrix4(e), this.radius *= e.getMaxScaleOnAxis(), this;
	}
	translate(e) {
		return this.center.add(e), this;
	}
	expandByPoint(e) {
		if (this.isEmpty()) return this.center.copy(e), this.radius = 0, this;
		Ra.subVectors(e, this.center);
		let t = Ra.lengthSq();
		if (t > this.radius * this.radius) {
			let e = Math.sqrt(t), n = (e - this.radius) * .5;
			this.center.addScaledVector(Ra, n / e), this.radius += n;
		}
		return this;
	}
	union(e) {
		return e.isEmpty() ? this : this.isEmpty() ? (this.copy(e), this) : (this.center.equals(e.center) === !0 ? this.radius = Math.max(this.radius, e.radius) : (za.subVectors(e.center, this.center).setLength(e.radius), this.expandByPoint(Ra.copy(e.center).add(za)), this.expandByPoint(Ra.copy(e.center).sub(za))), this);
	}
	equals(e) {
		return e.center.equals(this.center) && e.radius === this.radius;
	}
	clone() {
		return new this.constructor().copy(this);
	}
	toJSON() {
		return {
			radius: this.radius,
			center: this.center.toArray()
		};
	}
	fromJSON(e) {
		return this.radius = e.radius, this.center.fromArray(e.center), this;
	}
}, Va = 0, Ha = /*@__PURE__*/ new vi(), Ua = /*@__PURE__*/ new Gi(), Wa = /*@__PURE__*/ new q(), Ga = /*@__PURE__*/ new ha(), Ka = /*@__PURE__*/ new ha(), qa = /*@__PURE__*/ new q(), Ja = class e extends Sr {
	constructor() {
		super(), this.isBufferGeometry = !0, Object.defineProperty(this, "id", { value: Va++ }), this.uuid = Dr(), this.name = "", this.type = "BufferGeometry", this.index = null, this.indirect = null, this.indirectOffset = 0, this.attributes = {}, this.morphAttributes = {}, this.morphTargetsRelative = !1, this.groups = [], this.boundingBox = null, this.boundingSphere = null, this.drawRange = {
			start: 0,
			count: Infinity
		}, this.userData = {};
	}
	getIndex() {
		return this.index;
	}
	setIndex(e) {
		return Array.isArray(e) ? this.index = new (fr(e) ? Fa : Pa)(e, 1) : this.index = e, this;
	}
	setIndirect(e, t = 0) {
		return this.indirect = e, this.indirectOffset = t, this;
	}
	getIndirect() {
		return this.indirect;
	}
	getAttribute(e) {
		return this.attributes[e];
	}
	setAttribute(e, t) {
		return this.attributes[e] = t, this;
	}
	deleteAttribute(e) {
		return delete this.attributes[e], this;
	}
	hasAttribute(e) {
		return this.attributes[e] !== void 0;
	}
	addGroup(e, t, n = 0) {
		this.groups.push({
			start: e,
			count: t,
			materialIndex: n
		});
	}
	clearGroups() {
		this.groups = [];
	}
	setDrawRange(e, t) {
		this.drawRange.start = e, this.drawRange.count = t;
	}
	applyMatrix4(e) {
		let t = this.attributes.position;
		t !== void 0 && (t.applyMatrix4(e), t.needsUpdate = !0);
		let n = this.attributes.normal;
		if (n !== void 0) {
			let t = new J().getNormalMatrix(e);
			n.applyNormalMatrix(t), n.needsUpdate = !0;
		}
		let r = this.attributes.tangent;
		return r !== void 0 && (r.transformDirection(e), r.needsUpdate = !0), this.boundingBox !== null && this.computeBoundingBox(), this.boundingSphere !== null && this.computeBoundingSphere(), this;
	}
	applyQuaternion(e) {
		return Ha.makeRotationFromQuaternion(e), this.applyMatrix4(Ha), this;
	}
	rotateX(e) {
		return Ha.makeRotationX(e), this.applyMatrix4(Ha), this;
	}
	rotateY(e) {
		return Ha.makeRotationY(e), this.applyMatrix4(Ha), this;
	}
	rotateZ(e) {
		return Ha.makeRotationZ(e), this.applyMatrix4(Ha), this;
	}
	translate(e, t, n) {
		return Ha.makeTranslation(e, t, n), this.applyMatrix4(Ha), this;
	}
	scale(e, t, n) {
		return Ha.makeScale(e, t, n), this.applyMatrix4(Ha), this;
	}
	lookAt(e) {
		return Ua.lookAt(e), Ua.updateMatrix(), this.applyMatrix4(Ua.matrix), this;
	}
	center() {
		return this.computeBoundingBox(), this.boundingBox.getCenter(Wa).negate(), this.translate(Wa.x, Wa.y, Wa.z), this;
	}
	setFromPoints(e) {
		let t = this.getAttribute("position");
		if (t === void 0) {
			let t = [];
			for (let n = 0, r = e.length; n < r; n++) {
				let r = e[n];
				t.push(r.x, r.y, r.z || 0);
			}
			this.setAttribute("position", new Ia(t, 3));
		} else {
			let n = Math.min(e.length, t.count);
			for (let r = 0; r < n; r++) {
				let n = e[r];
				t.setXYZ(r, n.x, n.y, n.z || 0);
			}
			e.length > t.count && W("BufferGeometry: Buffer size too small for points data. Use .dispose() and create a new geometry."), t.needsUpdate = !0;
		}
		return this;
	}
	computeBoundingBox() {
		this.boundingBox === null && (this.boundingBox = new ha());
		let e = this.attributes.position, t = this.morphAttributes.position;
		if (e && e.isGLBufferAttribute) {
			G("BufferGeometry.computeBoundingBox(): GLBufferAttribute requires a manual bounding box.", this), this.boundingBox.set(new q(-Infinity, -Infinity, -Infinity), new q(Infinity, Infinity, Infinity));
			return;
		}
		if (e !== void 0) {
			if (this.boundingBox.setFromBufferAttribute(e), t) for (let e = 0, n = t.length; e < n; e++) {
				let n = t[e];
				Ga.setFromBufferAttribute(n), this.morphTargetsRelative ? (qa.addVectors(this.boundingBox.min, Ga.min), this.boundingBox.expandByPoint(qa), qa.addVectors(this.boundingBox.max, Ga.max), this.boundingBox.expandByPoint(qa)) : (this.boundingBox.expandByPoint(Ga.min), this.boundingBox.expandByPoint(Ga.max));
			}
		} else this.boundingBox.makeEmpty();
		(isNaN(this.boundingBox.min.x) || isNaN(this.boundingBox.min.y) || isNaN(this.boundingBox.min.z)) && G("BufferGeometry.computeBoundingBox(): Computed min/max have NaN values. The \"position\" attribute is likely to have NaN values.", this);
	}
	computeBoundingSphere() {
		this.boundingSphere === null && (this.boundingSphere = new Ba());
		let e = this.attributes.position, t = this.morphAttributes.position;
		if (e && e.isGLBufferAttribute) {
			G("BufferGeometry.computeBoundingSphere(): GLBufferAttribute requires a manual bounding sphere.", this), this.boundingSphere.set(new q(), Infinity);
			return;
		}
		if (e) {
			let n = this.boundingSphere.center;
			if (Ga.setFromBufferAttribute(e), t) for (let e = 0, n = t.length; e < n; e++) {
				let n = t[e];
				Ka.setFromBufferAttribute(n), this.morphTargetsRelative ? (qa.addVectors(Ga.min, Ka.min), Ga.expandByPoint(qa), qa.addVectors(Ga.max, Ka.max), Ga.expandByPoint(qa)) : (Ga.expandByPoint(Ka.min), Ga.expandByPoint(Ka.max));
			}
			Ga.getCenter(n);
			let r = 0;
			for (let t = 0, i = e.count; t < i; t++) qa.fromBufferAttribute(e, t), r = Math.max(r, n.distanceToSquared(qa));
			if (t) for (let i = 0, a = t.length; i < a; i++) {
				let a = t[i], o = this.morphTargetsRelative;
				for (let t = 0, i = a.count; t < i; t++) qa.fromBufferAttribute(a, t), o && (Wa.fromBufferAttribute(e, t), qa.add(Wa)), r = Math.max(r, n.distanceToSquared(qa));
			}
			this.boundingSphere.radius = Math.sqrt(r), isNaN(this.boundingSphere.radius) && G("BufferGeometry.computeBoundingSphere(): Computed radius is NaN. The \"position\" attribute is likely to have NaN values.", this);
		}
	}
	computeTangents() {
		let e = this.index, t = this.attributes;
		if (e === null || t.position === void 0 || t.normal === void 0 || t.uv === void 0) {
			G("BufferGeometry: .computeTangents() failed. Missing required attributes (index, position, normal or uv)");
			return;
		}
		let n = t.position, r = t.normal, i = t.uv;
		this.hasAttribute("tangent") === !1 && this.setAttribute("tangent", new Na(new Float32Array(4 * n.count), 4));
		let a = this.getAttribute("tangent"), o = [], s = [];
		for (let e = 0; e < n.count; e++) o[e] = new q(), s[e] = new q();
		let c = new q(), l = new q(), u = new q(), d = new Yr(), f = new Yr(), p = new Yr(), m = new q(), h = new q();
		function g(e, t, r) {
			c.fromBufferAttribute(n, e), l.fromBufferAttribute(n, t), u.fromBufferAttribute(n, r), d.fromBufferAttribute(i, e), f.fromBufferAttribute(i, t), p.fromBufferAttribute(i, r), l.sub(c), u.sub(c), f.sub(d), p.sub(d);
			let a = 1 / (f.x * p.y - p.x * f.y);
			isFinite(a) && (m.copy(l).multiplyScalar(p.y).addScaledVector(u, -f.y).multiplyScalar(a), h.copy(u).multiplyScalar(f.x).addScaledVector(l, -p.x).multiplyScalar(a), o[e].add(m), o[t].add(m), o[r].add(m), s[e].add(h), s[t].add(h), s[r].add(h));
		}
		let _ = this.groups;
		_.length === 0 && (_ = [{
			start: 0,
			count: e.count
		}]);
		for (let t = 0, n = _.length; t < n; ++t) {
			let n = _[t], r = n.start, i = n.count;
			for (let t = r, n = r + i; t < n; t += 3) g(e.getX(t + 0), e.getX(t + 1), e.getX(t + 2));
		}
		let v = new q(), y = new q(), b = new q(), x = new q();
		function S(e) {
			b.fromBufferAttribute(r, e), x.copy(b);
			let t = o[e];
			v.copy(t), v.sub(b.multiplyScalar(b.dot(t))).normalize(), y.crossVectors(x, t);
			let n = y.dot(s[e]) < 0 ? -1 : 1;
			a.setXYZW(e, v.x, v.y, v.z, n);
		}
		for (let t = 0, n = _.length; t < n; ++t) {
			let n = _[t], r = n.start, i = n.count;
			for (let t = r, n = r + i; t < n; t += 3) S(e.getX(t + 0)), S(e.getX(t + 1)), S(e.getX(t + 2));
		}
	}
	computeVertexNormals() {
		let e = this.index, t = this.getAttribute("position");
		if (t !== void 0) {
			let n = this.getAttribute("normal");
			if (n === void 0) n = new Na(new Float32Array(t.count * 3), 3), this.setAttribute("normal", n);
			else for (let e = 0, t = n.count; e < t; e++) n.setXYZ(e, 0, 0, 0);
			let r = new q(), i = new q(), a = new q(), o = new q(), s = new q(), c = new q(), l = new q(), u = new q();
			if (e) for (let d = 0, f = e.count; d < f; d += 3) {
				let f = e.getX(d + 0), p = e.getX(d + 1), m = e.getX(d + 2);
				r.fromBufferAttribute(t, f), i.fromBufferAttribute(t, p), a.fromBufferAttribute(t, m), l.subVectors(a, i), u.subVectors(r, i), l.cross(u), o.fromBufferAttribute(n, f), s.fromBufferAttribute(n, p), c.fromBufferAttribute(n, m), o.add(l), s.add(l), c.add(l), n.setXYZ(f, o.x, o.y, o.z), n.setXYZ(p, s.x, s.y, s.z), n.setXYZ(m, c.x, c.y, c.z);
			}
			else for (let e = 0, o = t.count; e < o; e += 3) r.fromBufferAttribute(t, e + 0), i.fromBufferAttribute(t, e + 1), a.fromBufferAttribute(t, e + 2), l.subVectors(a, i), u.subVectors(r, i), l.cross(u), n.setXYZ(e + 0, l.x, l.y, l.z), n.setXYZ(e + 1, l.x, l.y, l.z), n.setXYZ(e + 2, l.x, l.y, l.z);
			this.normalizeNormals(), n.needsUpdate = !0;
		}
	}
	normalizeNormals() {
		let e = this.attributes.normal;
		for (let t = 0, n = e.count; t < n; t++) qa.fromBufferAttribute(e, t), qa.normalize(), e.setXYZ(t, qa.x, qa.y, qa.z);
	}
	toNonIndexed() {
		function t(e, t) {
			let n = e.array, r = e.itemSize, i = e.normalized, a = new n.constructor(t.length * r), o = 0, s = 0;
			for (let i = 0, c = t.length; i < c; i++) {
				o = e.isInterleavedBufferAttribute ? t[i] * e.data.stride + e.offset : t[i] * r;
				for (let e = 0; e < r; e++) a[s++] = n[o++];
			}
			return new Na(a, r, i);
		}
		if (this.index === null) return W("BufferGeometry.toNonIndexed(): BufferGeometry is already non-indexed."), this;
		let n = new e(), r = this.index.array, i = this.attributes;
		for (let e in i) {
			let a = i[e], o = t(a, r);
			n.setAttribute(e, o);
		}
		let a = this.morphAttributes;
		for (let e in a) {
			let i = [], o = a[e];
			for (let e = 0, n = o.length; e < n; e++) {
				let n = o[e], a = t(n, r);
				i.push(a);
			}
			n.morphAttributes[e] = i;
		}
		n.morphTargetsRelative = this.morphTargetsRelative;
		let o = this.groups;
		for (let e = 0, t = o.length; e < t; e++) {
			let t = o[e];
			n.addGroup(t.start, t.count, t.materialIndex);
		}
		return n;
	}
	toJSON() {
		let e = { metadata: {
			version: 4.7,
			type: "BufferGeometry",
			generator: "BufferGeometry.toJSON"
		} };
		if (e.uuid = this.uuid, e.type = this.type, this.name !== "" && (e.name = this.name), Object.keys(this.userData).length > 0 && (e.userData = this.userData), this.parameters !== void 0) {
			let t = this.parameters;
			for (let n in t) t[n] !== void 0 && (e[n] = t[n]);
			return e;
		}
		e.data = { attributes: {} };
		let t = this.index;
		t !== null && (e.data.index = {
			type: t.array.constructor.name,
			array: Array.prototype.slice.call(t.array)
		});
		let n = this.attributes;
		for (let t in n) {
			let r = n[t];
			e.data.attributes[t] = r.toJSON(e.data);
		}
		let r = {}, i = !1;
		for (let t in this.morphAttributes) {
			let n = this.morphAttributes[t], a = [];
			for (let t = 0, r = n.length; t < r; t++) {
				let r = n[t];
				a.push(r.toJSON(e.data));
			}
			a.length > 0 && (r[t] = a, i = !0);
		}
		i && (e.data.morphAttributes = r, e.data.morphTargetsRelative = this.morphTargetsRelative);
		let a = this.groups;
		a.length > 0 && (e.data.groups = JSON.parse(JSON.stringify(a)));
		let o = this.boundingSphere;
		return o !== null && (e.data.boundingSphere = o.toJSON()), e;
	}
	clone() {
		return new this.constructor().copy(this);
	}
	copy(e) {
		this.index = null, this.attributes = {}, this.morphAttributes = {}, this.groups = [], this.boundingBox = null, this.boundingSphere = null;
		let t = {};
		this.name = e.name;
		let n = e.index;
		n !== null && this.setIndex(n.clone());
		let r = e.attributes;
		for (let e in r) {
			let n = r[e];
			this.setAttribute(e, n.clone(t));
		}
		let i = e.morphAttributes;
		for (let e in i) {
			let n = [], r = i[e];
			for (let e = 0, i = r.length; e < i; e++) n.push(r[e].clone(t));
			this.morphAttributes[e] = n;
		}
		this.morphTargetsRelative = e.morphTargetsRelative;
		let a = e.groups;
		for (let e = 0, t = a.length; e < t; e++) {
			let t = a[e];
			this.addGroup(t.start, t.count, t.materialIndex);
		}
		let o = e.boundingBox;
		o !== null && (this.boundingBox = o.clone());
		let s = e.boundingSphere;
		return s !== null && (this.boundingSphere = s.clone()), this.drawRange.start = e.drawRange.start, this.drawRange.count = e.drawRange.count, this.userData = e.userData, this;
	}
	dispose() {
		this.dispatchEvent({ type: "dispose" });
	}
}, Ya = 0, Xa = class extends Sr {
	constructor() {
		super(), this.isMaterial = !0, Object.defineProperty(this, "id", { value: Ya++ }), this.uuid = Dr(), this.name = "", this.type = "Material", this.blending = 1, this.side = 0, this.vertexColors = !1, this.opacity = 1, this.transparent = !1, this.alphaHash = !1, this.blendSrc = 204, this.blendDst = 205, this.blendEquation = 100, this.blendSrcAlpha = null, this.blendDstAlpha = null, this.blendEquationAlpha = null, this.blendColor = new X(0, 0, 0), this.blendAlpha = 0, this.depthFunc = 3, this.depthTest = !0, this.depthWrite = !0, this.stencilWriteMask = 255, this.stencilFunc = 519, this.stencilRef = 0, this.stencilFuncMask = 255, this.stencilFail = cr, this.stencilZFail = cr, this.stencilZPass = cr, this.stencilWrite = !1, this.clippingPlanes = null, this.clipIntersection = !1, this.clipShadows = !1, this.shadowSide = null, this.colorWrite = !0, this.precision = null, this.polygonOffset = !1, this.polygonOffsetFactor = 0, this.polygonOffsetUnits = 0, this.dithering = !1, this.alphaToCoverage = !1, this.premultipliedAlpha = !1, this.forceSinglePass = !1, this.allowOverride = !0, this.visible = !0, this.toneMapped = !0, this.userData = {}, this.version = 0, this._alphaTest = 0;
	}
	get alphaTest() {
		return this._alphaTest;
	}
	set alphaTest(e) {
		this._alphaTest > 0 != e > 0 && this.version++, this._alphaTest = e;
	}
	onBeforeRender() {}
	onBeforeCompile() {}
	customProgramCacheKey() {
		return this.onBeforeCompile.toString();
	}
	setValues(e) {
		if (e !== void 0) for (let t in e) {
			let n = e[t];
			if (n === void 0) {
				W(`Material: parameter '${t}' has value of undefined.`);
				continue;
			}
			let r = this[t];
			if (r === void 0) {
				W(`Material: '${t}' is not a property of THREE.${this.type}.`);
				continue;
			}
			r && r.isColor ? r.set(n) : r && r.isVector3 && n && n.isVector3 ? r.copy(n) : this[t] = n;
		}
	}
	toJSON(e) {
		let t = e === void 0 || typeof e == "string";
		t && (e = {
			textures: {},
			images: {}
		});
		let n = { metadata: {
			version: 4.7,
			type: "Material",
			generator: "Material.toJSON"
		} };
		n.uuid = this.uuid, n.type = this.type, this.name !== "" && (n.name = this.name), this.color && this.color.isColor && (n.color = this.color.getHex()), this.roughness !== void 0 && (n.roughness = this.roughness), this.metalness !== void 0 && (n.metalness = this.metalness), this.sheen !== void 0 && (n.sheen = this.sheen), this.sheenColor && this.sheenColor.isColor && (n.sheenColor = this.sheenColor.getHex()), this.sheenRoughness !== void 0 && (n.sheenRoughness = this.sheenRoughness), this.emissive && this.emissive.isColor && (n.emissive = this.emissive.getHex()), this.emissiveIntensity !== void 0 && this.emissiveIntensity !== 1 && (n.emissiveIntensity = this.emissiveIntensity), this.specular && this.specular.isColor && (n.specular = this.specular.getHex()), this.specularIntensity !== void 0 && (n.specularIntensity = this.specularIntensity), this.specularColor && this.specularColor.isColor && (n.specularColor = this.specularColor.getHex()), this.shininess !== void 0 && (n.shininess = this.shininess), this.clearcoat !== void 0 && (n.clearcoat = this.clearcoat), this.clearcoatRoughness !== void 0 && (n.clearcoatRoughness = this.clearcoatRoughness), this.clearcoatMap && this.clearcoatMap.isTexture && (n.clearcoatMap = this.clearcoatMap.toJSON(e).uuid), this.clearcoatRoughnessMap && this.clearcoatRoughnessMap.isTexture && (n.clearcoatRoughnessMap = this.clearcoatRoughnessMap.toJSON(e).uuid), this.clearcoatNormalMap && this.clearcoatNormalMap.isTexture && (n.clearcoatNormalMap = this.clearcoatNormalMap.toJSON(e).uuid, n.clearcoatNormalScale = this.clearcoatNormalScale.toArray()), this.sheenColorMap && this.sheenColorMap.isTexture && (n.sheenColorMap = this.sheenColorMap.toJSON(e).uuid), this.sheenRoughnessMap && this.sheenRoughnessMap.isTexture && (n.sheenRoughnessMap = this.sheenRoughnessMap.toJSON(e).uuid), this.dispersion !== void 0 && (n.dispersion = this.dispersion), this.iridescence !== void 0 && (n.iridescence = this.iridescence), this.iridescenceIOR !== void 0 && (n.iridescenceIOR = this.iridescenceIOR), this.iridescenceThicknessRange !== void 0 && (n.iridescenceThicknessRange = this.iridescenceThicknessRange), this.iridescenceMap && this.iridescenceMap.isTexture && (n.iridescenceMap = this.iridescenceMap.toJSON(e).uuid), this.iridescenceThicknessMap && this.iridescenceThicknessMap.isTexture && (n.iridescenceThicknessMap = this.iridescenceThicknessMap.toJSON(e).uuid), this.anisotropy !== void 0 && (n.anisotropy = this.anisotropy), this.anisotropyRotation !== void 0 && (n.anisotropyRotation = this.anisotropyRotation), this.anisotropyMap && this.anisotropyMap.isTexture && (n.anisotropyMap = this.anisotropyMap.toJSON(e).uuid), this.map && this.map.isTexture && (n.map = this.map.toJSON(e).uuid), this.matcap && this.matcap.isTexture && (n.matcap = this.matcap.toJSON(e).uuid), this.alphaMap && this.alphaMap.isTexture && (n.alphaMap = this.alphaMap.toJSON(e).uuid), this.lightMap && this.lightMap.isTexture && (n.lightMap = this.lightMap.toJSON(e).uuid, n.lightMapIntensity = this.lightMapIntensity), this.aoMap && this.aoMap.isTexture && (n.aoMap = this.aoMap.toJSON(e).uuid, n.aoMapIntensity = this.aoMapIntensity), this.bumpMap && this.bumpMap.isTexture && (n.bumpMap = this.bumpMap.toJSON(e).uuid, n.bumpScale = this.bumpScale), this.normalMap && this.normalMap.isTexture && (n.normalMap = this.normalMap.toJSON(e).uuid, n.normalMapType = this.normalMapType, n.normalScale = this.normalScale.toArray()), this.displacementMap && this.displacementMap.isTexture && (n.displacementMap = this.displacementMap.toJSON(e).uuid, n.displacementScale = this.displacementScale, n.displacementBias = this.displacementBias), this.roughnessMap && this.roughnessMap.isTexture && (n.roughnessMap = this.roughnessMap.toJSON(e).uuid), this.metalnessMap && this.metalnessMap.isTexture && (n.metalnessMap = this.metalnessMap.toJSON(e).uuid), this.emissiveMap && this.emissiveMap.isTexture && (n.emissiveMap = this.emissiveMap.toJSON(e).uuid), this.specularMap && this.specularMap.isTexture && (n.specularMap = this.specularMap.toJSON(e).uuid), this.specularIntensityMap && this.specularIntensityMap.isTexture && (n.specularIntensityMap = this.specularIntensityMap.toJSON(e).uuid), this.specularColorMap && this.specularColorMap.isTexture && (n.specularColorMap = this.specularColorMap.toJSON(e).uuid), this.envMap && this.envMap.isTexture && (n.envMap = this.envMap.toJSON(e).uuid, this.combine !== void 0 && (n.combine = this.combine)), this.envMapRotation !== void 0 && (n.envMapRotation = this.envMapRotation.toArray()), this.envMapIntensity !== void 0 && (n.envMapIntensity = this.envMapIntensity), this.reflectivity !== void 0 && (n.reflectivity = this.reflectivity), this.refractionRatio !== void 0 && (n.refractionRatio = this.refractionRatio), this.gradientMap && this.gradientMap.isTexture && (n.gradientMap = this.gradientMap.toJSON(e).uuid), this.transmission !== void 0 && (n.transmission = this.transmission), this.transmissionMap && this.transmissionMap.isTexture && (n.transmissionMap = this.transmissionMap.toJSON(e).uuid), this.thickness !== void 0 && (n.thickness = this.thickness), this.thicknessMap && this.thicknessMap.isTexture && (n.thicknessMap = this.thicknessMap.toJSON(e).uuid), this.attenuationDistance !== void 0 && this.attenuationDistance !== Infinity && (n.attenuationDistance = this.attenuationDistance), this.attenuationColor !== void 0 && (n.attenuationColor = this.attenuationColor.getHex()), this.size !== void 0 && (n.size = this.size), this.shadowSide !== null && (n.shadowSide = this.shadowSide), this.sizeAttenuation !== void 0 && (n.sizeAttenuation = this.sizeAttenuation), this.blending !== 1 && (n.blending = this.blending), this.side !== 0 && (n.side = this.side), this.vertexColors === !0 && (n.vertexColors = !0), this.opacity < 1 && (n.opacity = this.opacity), this.transparent === !0 && (n.transparent = !0), this.blendSrc !== 204 && (n.blendSrc = this.blendSrc), this.blendDst !== 205 && (n.blendDst = this.blendDst), this.blendEquation !== 100 && (n.blendEquation = this.blendEquation), this.blendSrcAlpha !== null && (n.blendSrcAlpha = this.blendSrcAlpha), this.blendDstAlpha !== null && (n.blendDstAlpha = this.blendDstAlpha), this.blendEquationAlpha !== null && (n.blendEquationAlpha = this.blendEquationAlpha), this.blendColor && this.blendColor.isColor && (n.blendColor = this.blendColor.getHex()), this.blendAlpha !== 0 && (n.blendAlpha = this.blendAlpha), this.depthFunc !== 3 && (n.depthFunc = this.depthFunc), this.depthTest === !1 && (n.depthTest = this.depthTest), this.depthWrite === !1 && (n.depthWrite = this.depthWrite), this.colorWrite === !1 && (n.colorWrite = this.colorWrite), this.stencilWriteMask !== 255 && (n.stencilWriteMask = this.stencilWriteMask), this.stencilFunc !== 519 && (n.stencilFunc = this.stencilFunc), this.stencilRef !== 0 && (n.stencilRef = this.stencilRef), this.stencilFuncMask !== 255 && (n.stencilFuncMask = this.stencilFuncMask), this.stencilFail !== 7680 && (n.stencilFail = this.stencilFail), this.stencilZFail !== 7680 && (n.stencilZFail = this.stencilZFail), this.stencilZPass !== 7680 && (n.stencilZPass = this.stencilZPass), this.stencilWrite === !0 && (n.stencilWrite = this.stencilWrite), this.rotation !== void 0 && this.rotation !== 0 && (n.rotation = this.rotation), this.polygonOffset === !0 && (n.polygonOffset = !0), this.polygonOffsetFactor !== 0 && (n.polygonOffsetFactor = this.polygonOffsetFactor), this.polygonOffsetUnits !== 0 && (n.polygonOffsetUnits = this.polygonOffsetUnits), this.linewidth !== void 0 && this.linewidth !== 1 && (n.linewidth = this.linewidth), this.dashSize !== void 0 && (n.dashSize = this.dashSize), this.gapSize !== void 0 && (n.gapSize = this.gapSize), this.scale !== void 0 && (n.scale = this.scale), this.dithering === !0 && (n.dithering = !0), this.alphaTest > 0 && (n.alphaTest = this.alphaTest), this.alphaHash === !0 && (n.alphaHash = !0), this.alphaToCoverage === !0 && (n.alphaToCoverage = !0), this.premultipliedAlpha === !0 && (n.premultipliedAlpha = !0), this.forceSinglePass === !0 && (n.forceSinglePass = !0), this.allowOverride === !1 && (n.allowOverride = !1), this.wireframe === !0 && (n.wireframe = !0), this.wireframeLinewidth > 1 && (n.wireframeLinewidth = this.wireframeLinewidth), this.wireframeLinecap !== "round" && (n.wireframeLinecap = this.wireframeLinecap), this.wireframeLinejoin !== "round" && (n.wireframeLinejoin = this.wireframeLinejoin), this.flatShading === !0 && (n.flatShading = !0), this.visible === !1 && (n.visible = !1), this.toneMapped === !1 && (n.toneMapped = !1), this.fog === !1 && (n.fog = !1), Object.keys(this.userData).length > 0 && (n.userData = this.userData);
		function r(e) {
			let t = [];
			for (let n in e) {
				let r = e[n];
				delete r.metadata, t.push(r);
			}
			return t;
		}
		if (t) {
			let t = r(e.textures), i = r(e.images);
			t.length > 0 && (n.textures = t), i.length > 0 && (n.images = i);
		}
		return n;
	}
	clone() {
		return new this.constructor().copy(this);
	}
	copy(e) {
		this.name = e.name, this.blending = e.blending, this.side = e.side, this.vertexColors = e.vertexColors, this.opacity = e.opacity, this.transparent = e.transparent, this.blendSrc = e.blendSrc, this.blendDst = e.blendDst, this.blendEquation = e.blendEquation, this.blendSrcAlpha = e.blendSrcAlpha, this.blendDstAlpha = e.blendDstAlpha, this.blendEquationAlpha = e.blendEquationAlpha, this.blendColor.copy(e.blendColor), this.blendAlpha = e.blendAlpha, this.depthFunc = e.depthFunc, this.depthTest = e.depthTest, this.depthWrite = e.depthWrite, this.stencilWriteMask = e.stencilWriteMask, this.stencilFunc = e.stencilFunc, this.stencilRef = e.stencilRef, this.stencilFuncMask = e.stencilFuncMask, this.stencilFail = e.stencilFail, this.stencilZFail = e.stencilZFail, this.stencilZPass = e.stencilZPass, this.stencilWrite = e.stencilWrite;
		let t = e.clippingPlanes, n = null;
		if (t !== null) {
			let e = t.length;
			n = Array(e);
			for (let r = 0; r !== e; ++r) n[r] = t[r].clone();
		}
		return this.clippingPlanes = n, this.clipIntersection = e.clipIntersection, this.clipShadows = e.clipShadows, this.shadowSide = e.shadowSide, this.colorWrite = e.colorWrite, this.precision = e.precision, this.polygonOffset = e.polygonOffset, this.polygonOffsetFactor = e.polygonOffsetFactor, this.polygonOffsetUnits = e.polygonOffsetUnits, this.dithering = e.dithering, this.alphaTest = e.alphaTest, this.alphaHash = e.alphaHash, this.alphaToCoverage = e.alphaToCoverage, this.premultipliedAlpha = e.premultipliedAlpha, this.forceSinglePass = e.forceSinglePass, this.allowOverride = e.allowOverride, this.visible = e.visible, this.toneMapped = e.toneMapped, this.userData = JSON.parse(JSON.stringify(e.userData)), this;
	}
	dispose() {
		this.dispatchEvent({ type: "dispose" });
	}
	set needsUpdate(e) {
		e === !0 && this.version++;
	}
}, Za = /*@__PURE__*/ new q(), Qa = /*@__PURE__*/ new q(), $a = /*@__PURE__*/ new q(), eo = /*@__PURE__*/ new q(), to = /*@__PURE__*/ new q(), no = /*@__PURE__*/ new q(), ro = /*@__PURE__*/ new q(), io = class {
	constructor(e = new q(), t = new q(0, 0, -1)) {
		this.origin = e, this.direction = t;
	}
	set(e, t) {
		return this.origin.copy(e), this.direction.copy(t), this;
	}
	copy(e) {
		return this.origin.copy(e.origin), this.direction.copy(e.direction), this;
	}
	at(e, t) {
		return t.copy(this.origin).addScaledVector(this.direction, e);
	}
	lookAt(e) {
		return this.direction.copy(e).sub(this.origin).normalize(), this;
	}
	recast(e) {
		return this.origin.copy(this.at(e, Za)), this;
	}
	closestPointToPoint(e, t) {
		t.subVectors(e, this.origin);
		let n = t.dot(this.direction);
		return n < 0 ? t.copy(this.origin) : t.copy(this.origin).addScaledVector(this.direction, n);
	}
	distanceToPoint(e) {
		return Math.sqrt(this.distanceSqToPoint(e));
	}
	distanceSqToPoint(e) {
		let t = Za.subVectors(e, this.origin).dot(this.direction);
		return t < 0 ? this.origin.distanceToSquared(e) : (Za.copy(this.origin).addScaledVector(this.direction, t), Za.distanceToSquared(e));
	}
	distanceSqToSegment(e, t, n, r) {
		Qa.copy(e).add(t).multiplyScalar(.5), $a.copy(t).sub(e).normalize(), eo.copy(this.origin).sub(Qa);
		let i = e.distanceTo(t) * .5, a = -this.direction.dot($a), o = eo.dot(this.direction), s = -eo.dot($a), c = eo.lengthSq(), l = Math.abs(1 - a * a), u, d, f, p;
		if (l > 0) if (u = a * s - o, d = a * o - s, p = i * l, u >= 0) if (d >= -p) if (d <= p) {
			let e = 1 / l;
			u *= e, d *= e, f = u * (u + a * d + 2 * o) + d * (a * u + d + 2 * s) + c;
		} else d = i, u = Math.max(0, -(a * d + o)), f = -u * u + d * (d + 2 * s) + c;
		else d = -i, u = Math.max(0, -(a * d + o)), f = -u * u + d * (d + 2 * s) + c;
		else d <= -p ? (u = Math.max(0, -(-a * i + o)), d = u > 0 ? -i : Math.min(Math.max(-i, -s), i), f = -u * u + d * (d + 2 * s) + c) : d <= p ? (u = 0, d = Math.min(Math.max(-i, -s), i), f = d * (d + 2 * s) + c) : (u = Math.max(0, -(a * i + o)), d = u > 0 ? i : Math.min(Math.max(-i, -s), i), f = -u * u + d * (d + 2 * s) + c);
		else d = a > 0 ? -i : i, u = Math.max(0, -(a * d + o)), f = -u * u + d * (d + 2 * s) + c;
		return n && n.copy(this.origin).addScaledVector(this.direction, u), r && r.copy(Qa).addScaledVector($a, d), f;
	}
	intersectSphere(e, t) {
		Za.subVectors(e.center, this.origin);
		let n = Za.dot(this.direction), r = Za.dot(Za) - n * n, i = e.radius * e.radius;
		if (r > i) return null;
		let a = Math.sqrt(i - r), o = n - a, s = n + a;
		return s < 0 ? null : o < 0 ? this.at(s, t) : this.at(o, t);
	}
	intersectsSphere(e) {
		return e.radius < 0 ? !1 : this.distanceSqToPoint(e.center) <= e.radius * e.radius;
	}
	distanceToPlane(e) {
		let t = e.normal.dot(this.direction);
		if (t === 0) return e.distanceToPoint(this.origin) === 0 ? 0 : null;
		let n = -(this.origin.dot(e.normal) + e.constant) / t;
		return n >= 0 ? n : null;
	}
	intersectPlane(e, t) {
		let n = this.distanceToPlane(e);
		return n === null ? null : this.at(n, t);
	}
	intersectsPlane(e) {
		let t = e.distanceToPoint(this.origin);
		return t === 0 || e.normal.dot(this.direction) * t < 0;
	}
	intersectBox(e, t) {
		let n, r, i, a, o, s, c = 1 / this.direction.x, l = 1 / this.direction.y, u = 1 / this.direction.z, d = this.origin;
		return c >= 0 ? (n = (e.min.x - d.x) * c, r = (e.max.x - d.x) * c) : (n = (e.max.x - d.x) * c, r = (e.min.x - d.x) * c), l >= 0 ? (i = (e.min.y - d.y) * l, a = (e.max.y - d.y) * l) : (i = (e.max.y - d.y) * l, a = (e.min.y - d.y) * l), n > a || i > r || ((i > n || isNaN(n)) && (n = i), (a < r || isNaN(r)) && (r = a), u >= 0 ? (o = (e.min.z - d.z) * u, s = (e.max.z - d.z) * u) : (o = (e.max.z - d.z) * u, s = (e.min.z - d.z) * u), n > s || o > r) || ((o > n || n !== n) && (n = o), (s < r || r !== r) && (r = s), r < 0) ? null : this.at(n >= 0 ? n : r, t);
	}
	intersectsBox(e) {
		return this.intersectBox(e, Za) !== null;
	}
	intersectTriangle(e, t, n, r, i) {
		to.subVectors(t, e), no.subVectors(n, e), ro.crossVectors(to, no);
		let a = this.direction.dot(ro), o;
		if (a > 0) {
			if (r) return null;
			o = 1;
		} else if (a < 0) o = -1, a = -a;
		else return null;
		eo.subVectors(this.origin, e);
		let s = o * this.direction.dot(no.crossVectors(eo, no));
		if (s < 0) return null;
		let c = o * this.direction.dot(to.cross(eo));
		if (c < 0 || s + c > a) return null;
		let l = -o * eo.dot(ro);
		return l < 0 ? null : this.at(l / a, i);
	}
	applyMatrix4(e) {
		return this.origin.applyMatrix4(e), this.direction.transformDirection(e), this;
	}
	equals(e) {
		return e.origin.equals(this.origin) && e.direction.equals(this.direction);
	}
	clone() {
		return new this.constructor().copy(this);
	}
}, ao = class extends Xa {
	constructor(e) {
		super(), this.isMeshBasicMaterial = !0, this.type = "MeshBasicMaterial", this.color = new X(16777215), this.map = null, this.lightMap = null, this.lightMapIntensity = 1, this.aoMap = null, this.aoMapIntensity = 1, this.specularMap = null, this.alphaMap = null, this.envMap = null, this.envMapRotation = new Oi(), this.combine = 0, this.reflectivity = 1, this.refractionRatio = .98, this.wireframe = !1, this.wireframeLinewidth = 1, this.wireframeLinecap = "round", this.wireframeLinejoin = "round", this.fog = !0, this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.color.copy(e.color), this.map = e.map, this.lightMap = e.lightMap, this.lightMapIntensity = e.lightMapIntensity, this.aoMap = e.aoMap, this.aoMapIntensity = e.aoMapIntensity, this.specularMap = e.specularMap, this.alphaMap = e.alphaMap, this.envMap = e.envMap, this.envMapRotation.copy(e.envMapRotation), this.combine = e.combine, this.reflectivity = e.reflectivity, this.refractionRatio = e.refractionRatio, this.wireframe = e.wireframe, this.wireframeLinewidth = e.wireframeLinewidth, this.wireframeLinecap = e.wireframeLinecap, this.wireframeLinejoin = e.wireframeLinejoin, this.fog = e.fog, this;
	}
}, oo = /*@__PURE__*/ new vi(), so = /*@__PURE__*/ new io(), co = /*@__PURE__*/ new Ba(), lo = /*@__PURE__*/ new q(), uo = /*@__PURE__*/ new q(), fo = /*@__PURE__*/ new q(), po = /*@__PURE__*/ new q(), mo = /*@__PURE__*/ new q(), ho = /*@__PURE__*/ new q(), go = /*@__PURE__*/ new q(), _o = /*@__PURE__*/ new q(), vo = class extends Gi {
	constructor(e = new Ja(), t = new ao()) {
		super(), this.isMesh = !0, this.type = "Mesh", this.geometry = e, this.material = t, this.morphTargetDictionary = void 0, this.morphTargetInfluences = void 0, this.count = 1, this.updateMorphTargets();
	}
	copy(e, t) {
		return super.copy(e, t), e.morphTargetInfluences !== void 0 && (this.morphTargetInfluences = e.morphTargetInfluences.slice()), e.morphTargetDictionary !== void 0 && (this.morphTargetDictionary = Object.assign({}, e.morphTargetDictionary)), this.material = Array.isArray(e.material) ? e.material.slice() : e.material, this.geometry = e.geometry, this;
	}
	updateMorphTargets() {
		let e = this.geometry.morphAttributes, t = Object.keys(e);
		if (t.length > 0) {
			let n = e[t[0]];
			if (n !== void 0) {
				this.morphTargetInfluences = [], this.morphTargetDictionary = {};
				for (let e = 0, t = n.length; e < t; e++) {
					let t = n[e].name || String(e);
					this.morphTargetInfluences.push(0), this.morphTargetDictionary[t] = e;
				}
			}
		}
	}
	getVertexPosition(e, t) {
		let n = this.geometry, r = n.attributes.position, i = n.morphAttributes.position, a = n.morphTargetsRelative;
		t.fromBufferAttribute(r, e);
		let o = this.morphTargetInfluences;
		if (i && o) {
			ho.set(0, 0, 0);
			for (let n = 0, r = i.length; n < r; n++) {
				let r = o[n], s = i[n];
				r !== 0 && (mo.fromBufferAttribute(s, e), a ? ho.addScaledVector(mo, r) : ho.addScaledVector(mo.sub(t), r));
			}
			t.add(ho);
		}
		return t;
	}
	raycast(e, t) {
		let n = this.geometry, r = this.material, i = this.matrixWorld;
		r !== void 0 && (n.boundingSphere === null && n.computeBoundingSphere(), co.copy(n.boundingSphere), co.applyMatrix4(i), so.copy(e.ray).recast(e.near), !(co.containsPoint(so.origin) === !1 && (so.intersectSphere(co, lo) === null || so.origin.distanceToSquared(lo) > (e.far - e.near) ** 2)) && (oo.copy(i).invert(), so.copy(e.ray).applyMatrix4(oo), !(n.boundingBox !== null && so.intersectsBox(n.boundingBox) === !1) && this._computeIntersections(e, t, so)));
	}
	_computeIntersections(e, t, n) {
		let r, i = this.geometry, a = this.material, o = i.index, s = i.attributes.position, c = i.attributes.uv, l = i.attributes.uv1, u = i.attributes.normal, d = i.groups, f = i.drawRange;
		if (o !== null) if (Array.isArray(a)) for (let i = 0, s = d.length; i < s; i++) {
			let s = d[i], p = a[s.materialIndex], m = Math.max(s.start, f.start), h = Math.min(o.count, Math.min(s.start + s.count, f.start + f.count));
			for (let i = m, a = h; i < a; i += 3) {
				let a = o.getX(i), d = o.getX(i + 1), f = o.getX(i + 2);
				r = bo(this, p, e, n, c, l, u, a, d, f), r && (r.faceIndex = Math.floor(i / 3), r.face.materialIndex = s.materialIndex, t.push(r));
			}
		}
		else {
			let i = Math.max(0, f.start), s = Math.min(o.count, f.start + f.count);
			for (let d = i, f = s; d < f; d += 3) {
				let i = o.getX(d), s = o.getX(d + 1), f = o.getX(d + 2);
				r = bo(this, a, e, n, c, l, u, i, s, f), r && (r.faceIndex = Math.floor(d / 3), t.push(r));
			}
		}
		else if (s !== void 0) if (Array.isArray(a)) for (let i = 0, o = d.length; i < o; i++) {
			let o = d[i], p = a[o.materialIndex], m = Math.max(o.start, f.start), h = Math.min(s.count, Math.min(o.start + o.count, f.start + f.count));
			for (let i = m, a = h; i < a; i += 3) {
				let a = i, s = i + 1, d = i + 2;
				r = bo(this, p, e, n, c, l, u, a, s, d), r && (r.faceIndex = Math.floor(i / 3), r.face.materialIndex = o.materialIndex, t.push(r));
			}
		}
		else {
			let i = Math.max(0, f.start), o = Math.min(s.count, f.start + f.count);
			for (let s = i, d = o; s < d; s += 3) {
				let i = s, o = s + 1, d = s + 2;
				r = bo(this, a, e, n, c, l, u, i, o, d), r && (r.faceIndex = Math.floor(s / 3), t.push(r));
			}
		}
	}
};
function yo(e, t, n, r, i, a, o, s) {
	let c;
	if (c = t.side === 1 ? r.intersectTriangle(o, a, i, !0, s) : r.intersectTriangle(i, a, o, t.side === 0, s), c === null) return null;
	_o.copy(s), _o.applyMatrix4(e.matrixWorld);
	let l = n.ray.origin.distanceTo(_o);
	return l < n.near || l > n.far ? null : {
		distance: l,
		point: _o.clone(),
		object: e
	};
}
function bo(e, t, n, r, i, a, o, s, c, l) {
	e.getVertexPosition(s, uo), e.getVertexPosition(c, fo), e.getVertexPosition(l, po);
	let u = yo(e, t, n, r, uo, fo, po, go);
	if (u) {
		let e = new q();
		ma.getBarycoord(go, uo, fo, po, e), i && (u.uv = ma.getInterpolatedAttribute(i, s, c, l, e, new Yr())), a && (u.uv1 = ma.getInterpolatedAttribute(a, s, c, l, e, new Yr())), o && (u.normal = ma.getInterpolatedAttribute(o, s, c, l, e, new q()), u.normal.dot(r.direction) > 0 && u.normal.multiplyScalar(-1));
		let t = {
			a: s,
			b: c,
			c: l,
			normal: new q(),
			materialIndex: 0
		};
		ma.getNormal(uo, fo, po, t.normal), u.face = t, u.barycoord = e;
	}
	return u;
}
var xo = /*@__PURE__*/ new pi(), So = /*@__PURE__*/ new pi(), Co = /*@__PURE__*/ new pi(), wo = /*@__PURE__*/ new pi(), To = /*@__PURE__*/ new vi(), Eo = /*@__PURE__*/ new q(), Do = /*@__PURE__*/ new Ba(), Oo = /*@__PURE__*/ new vi(), ko = /*@__PURE__*/ new io(), Ao = class extends vo {
	constructor(e, t) {
		super(e, t), this.isSkinnedMesh = !0, this.type = "SkinnedMesh", this.bindMode = kt, this.bindMatrix = new vi(), this.bindMatrixInverse = new vi(), this.boundingBox = null, this.boundingSphere = null;
	}
	computeBoundingBox() {
		let e = this.geometry;
		this.boundingBox === null && (this.boundingBox = new ha()), this.boundingBox.makeEmpty();
		let t = e.getAttribute("position");
		for (let e = 0; e < t.count; e++) this.getVertexPosition(e, Eo), this.boundingBox.expandByPoint(Eo);
	}
	computeBoundingSphere() {
		let e = this.geometry;
		this.boundingSphere === null && (this.boundingSphere = new Ba()), this.boundingSphere.makeEmpty();
		let t = e.getAttribute("position");
		for (let e = 0; e < t.count; e++) this.getVertexPosition(e, Eo), this.boundingSphere.expandByPoint(Eo);
	}
	copy(e, t) {
		return super.copy(e, t), this.bindMode = e.bindMode, this.bindMatrix.copy(e.bindMatrix), this.bindMatrixInverse.copy(e.bindMatrixInverse), this.skeleton = e.skeleton, e.boundingBox !== null && (this.boundingBox = e.boundingBox.clone()), e.boundingSphere !== null && (this.boundingSphere = e.boundingSphere.clone()), this;
	}
	raycast(e, t) {
		let n = this.material, r = this.matrixWorld;
		n !== void 0 && (this.boundingSphere === null && this.computeBoundingSphere(), Do.copy(this.boundingSphere), Do.applyMatrix4(r), e.ray.intersectsSphere(Do) !== !1 && (Oo.copy(r).invert(), ko.copy(e.ray).applyMatrix4(Oo), !(this.boundingBox !== null && ko.intersectsBox(this.boundingBox) === !1) && this._computeIntersections(e, t, ko)));
	}
	getVertexPosition(e, t) {
		return super.getVertexPosition(e, t), this.applyBoneTransform(e, t), t;
	}
	bind(e, t) {
		this.skeleton = e, t === void 0 && (this.updateMatrixWorld(!0), this.skeleton.calculateInverses(), t = this.matrixWorld), this.bindMatrix.copy(t), this.bindMatrixInverse.copy(t).invert();
	}
	pose() {
		this.skeleton.pose();
	}
	normalizeSkinWeights() {
		let e = new pi(), t = this.geometry.attributes.skinWeight;
		for (let n = 0, r = t.count; n < r; n++) {
			e.fromBufferAttribute(t, n);
			let r = 1 / e.manhattanLength();
			r === Infinity ? e.set(1, 0, 0, 0) : e.multiplyScalar(r), t.setXYZW(n, e.x, e.y, e.z, e.w);
		}
	}
	updateMatrixWorld(e) {
		super.updateMatrixWorld(e), this.bindMode === "attached" ? this.bindMatrixInverse.copy(this.matrixWorld).invert() : this.bindMode === "detached" ? this.bindMatrixInverse.copy(this.bindMatrix).invert() : W("SkinnedMesh: Unrecognized bindMode: " + this.bindMode);
	}
	applyBoneTransform(e, t) {
		let n = this.skeleton, r = this.geometry;
		So.fromBufferAttribute(r.attributes.skinIndex, e), Co.fromBufferAttribute(r.attributes.skinWeight, e), t.isVector4 ? (xo.copy(t), t.set(0, 0, 0, 0)) : (xo.set(...t, 1), t.set(0, 0, 0)), xo.applyMatrix4(this.bindMatrix);
		for (let e = 0; e < 4; e++) {
			let r = Co.getComponent(e);
			if (r !== 0) {
				let i = So.getComponent(e);
				To.multiplyMatrices(n.bones[i].matrixWorld, n.boneInverses[i]), t.addScaledVector(wo.copy(xo).applyMatrix4(To), r);
			}
		}
		return t.isVector4 && (t.w = xo.w), t.applyMatrix4(this.bindMatrixInverse);
	}
}, jo = class extends Gi {
	constructor() {
		super(), this.isBone = !0, this.type = "Bone";
	}
}, Mo = class extends fi {
	constructor(e = null, t = 1, n = 1, r, i, a, o, s, c = Nt, l = Nt, u, d) {
		super(null, a, o, s, c, l, r, i, u, d), this.isDataTexture = !0, this.image = {
			data: e,
			width: t,
			height: n
		}, this.generateMipmaps = !1, this.flipY = !1, this.unpackAlignment = 1;
	}
}, No = class extends Na {
	constructor(e, t, n, r = 1) {
		super(e, t, n), this.isInstancedBufferAttribute = !0, this.meshPerAttribute = r;
	}
	copy(e) {
		return super.copy(e), this.meshPerAttribute = e.meshPerAttribute, this;
	}
	toJSON() {
		let e = super.toJSON();
		return e.meshPerAttribute = this.meshPerAttribute, e.isInstancedBufferAttribute = !0, e;
	}
}, Po = /*@__PURE__*/ new vi(), Fo = /*@__PURE__*/ new vi(), Io = [], Lo = /*@__PURE__*/ new ha(), Ro = /*@__PURE__*/ new vi(), zo = /*@__PURE__*/ new vo(), Bo = /*@__PURE__*/ new Ba(), Vo = class extends vo {
	constructor(e, t, n) {
		super(e, t), this.isInstancedMesh = !0, this.instanceMatrix = new No(new Float32Array(n * 16), 16), this.previousInstanceMatrix = null, this.instanceColor = null, this.morphTexture = null, this.count = n, this.boundingBox = null, this.boundingSphere = null;
		for (let e = 0; e < n; e++) this.setMatrixAt(e, Ro);
	}
	computeBoundingBox() {
		let e = this.geometry, t = this.count;
		this.boundingBox === null && (this.boundingBox = new ha()), e.boundingBox === null && e.computeBoundingBox(), this.boundingBox.makeEmpty();
		for (let n = 0; n < t; n++) this.getMatrixAt(n, Po), Lo.copy(e.boundingBox).applyMatrix4(Po), this.boundingBox.union(Lo);
	}
	computeBoundingSphere() {
		let e = this.geometry, t = this.count;
		this.boundingSphere === null && (this.boundingSphere = new Ba()), e.boundingSphere === null && e.computeBoundingSphere(), this.boundingSphere.makeEmpty();
		for (let n = 0; n < t; n++) this.getMatrixAt(n, Po), Bo.copy(e.boundingSphere).applyMatrix4(Po), this.boundingSphere.union(Bo);
	}
	copy(e, t) {
		return super.copy(e, t), this.instanceMatrix.copy(e.instanceMatrix), e.previousInstanceMatrix !== null && (this.previousInstanceMatrix = e.previousInstanceMatrix.clone()), e.morphTexture !== null && (this.morphTexture = e.morphTexture.clone()), e.instanceColor !== null && (this.instanceColor = e.instanceColor.clone()), this.count = e.count, e.boundingBox !== null && (this.boundingBox = e.boundingBox.clone()), e.boundingSphere !== null && (this.boundingSphere = e.boundingSphere.clone()), this;
	}
	getColorAt(e, t) {
		return this.instanceColor === null ? t.setRGB(1, 1, 1) : t.fromArray(this.instanceColor.array, e * 3);
	}
	getMatrixAt(e, t) {
		return t.fromArray(this.instanceMatrix.array, e * 16);
	}
	getMorphAt(e, t) {
		let n = t.morphTargetInfluences, r = this.morphTexture.source.data.data, i = e * (n.length + 1) + 1;
		for (let e = 0; e < n.length; e++) n[e] = r[i + e];
	}
	raycast(e, t) {
		let n = this.matrixWorld, r = this.count;
		if (zo.geometry = this.geometry, zo.material = this.material, zo.material !== void 0 && (this.boundingSphere === null && this.computeBoundingSphere(), Bo.copy(this.boundingSphere), Bo.applyMatrix4(n), e.ray.intersectsSphere(Bo) !== !1)) for (let i = 0; i < r; i++) {
			this.getMatrixAt(i, Po), Fo.multiplyMatrices(n, Po), zo.matrixWorld = Fo, zo.raycast(e, Io);
			for (let e = 0, n = Io.length; e < n; e++) {
				let n = Io[e];
				n.instanceId = i, n.object = this, t.push(n);
			}
			Io.length = 0;
		}
	}
	setColorAt(e, t) {
		return this.instanceColor === null && (this.instanceColor = new No(new Float32Array(this.instanceMatrix.count * 3).fill(1), 3)), t.toArray(this.instanceColor.array, e * 3), this;
	}
	setMatrixAt(e, t) {
		return t.toArray(this.instanceMatrix.array, e * 16), this;
	}
	setMorphAt(e, t) {
		let n = t.morphTargetInfluences, r = n.length + 1;
		this.morphTexture === null && (this.morphTexture = new Mo(new Float32Array(r * this.count), r, this.count, rn, Gt));
		let i = this.morphTexture.source.data.data, a = 0;
		for (let e = 0; e < n.length; e++) a += n[e];
		let o = this.geometry.morphTargetsRelative ? 1 : 1 - a, s = r * e;
		return i[s] = o, i.set(n, s + 1), this;
	}
	updateMorphTargets() {}
	dispose() {
		this.dispatchEvent({ type: "dispose" }), this.morphTexture !== null && (this.morphTexture.dispose(), this.morphTexture = null);
	}
}, Ho = /*@__PURE__*/ new q(), Uo = /*@__PURE__*/ new q(), Wo = /*@__PURE__*/ new J(), Go = class {
	constructor(e = new q(1, 0, 0), t = 0) {
		this.isPlane = !0, this.normal = e, this.constant = t;
	}
	set(e, t) {
		return this.normal.copy(e), this.constant = t, this;
	}
	setComponents(e, t, n, r) {
		return this.normal.set(e, t, n), this.constant = r, this;
	}
	setFromNormalAndCoplanarPoint(e, t) {
		return this.normal.copy(e), this.constant = -t.dot(this.normal), this;
	}
	setFromCoplanarPoints(e, t, n) {
		let r = Ho.subVectors(n, t).cross(Uo.subVectors(e, t)).normalize();
		return this.setFromNormalAndCoplanarPoint(r, e), this;
	}
	copy(e) {
		return this.normal.copy(e.normal), this.constant = e.constant, this;
	}
	normalize() {
		let e = 1 / this.normal.length();
		return this.normal.multiplyScalar(e), this.constant *= e, this;
	}
	negate() {
		return this.constant *= -1, this.normal.negate(), this;
	}
	distanceToPoint(e) {
		return this.normal.dot(e) + this.constant;
	}
	distanceToSphere(e) {
		return this.distanceToPoint(e.center) - e.radius;
	}
	projectPoint(e, t) {
		return t.copy(e).addScaledVector(this.normal, -this.distanceToPoint(e));
	}
	intersectLine(e, t, n = !0) {
		let r = e.delta(Ho), i = this.normal.dot(r);
		if (i === 0) return this.distanceToPoint(e.start) === 0 ? t.copy(e.start) : null;
		let a = -(e.start.dot(this.normal) + this.constant) / i;
		return n === !0 && (a < 0 || a > 1) ? null : t.copy(e.start).addScaledVector(r, a);
	}
	intersectsLine(e) {
		let t = this.distanceToPoint(e.start), n = this.distanceToPoint(e.end);
		return t < 0 && n > 0 || n < 0 && t > 0;
	}
	intersectsBox(e) {
		return e.intersectsPlane(this);
	}
	intersectsSphere(e) {
		return e.intersectsPlane(this);
	}
	coplanarPoint(e) {
		return e.copy(this.normal).multiplyScalar(-this.constant);
	}
	applyMatrix4(e, t) {
		let n = t || Wo.getNormalMatrix(e), r = this.coplanarPoint(Ho).applyMatrix4(e), i = this.normal.applyMatrix3(n).normalize();
		return this.constant = -r.dot(i), this;
	}
	translate(e) {
		return this.constant -= e.dot(this.normal), this;
	}
	equals(e) {
		return e.normal.equals(this.normal) && e.constant === this.constant;
	}
	clone() {
		return new this.constructor().copy(this);
	}
}, Ko = /*@__PURE__*/ new Ba(), qo = /*@__PURE__*/ new Yr(.5, .5), Jo = /*@__PURE__*/ new q(), Yo = class {
	constructor(e = new Go(), t = new Go(), n = new Go(), r = new Go(), i = new Go(), a = new Go()) {
		this.planes = [
			e,
			t,
			n,
			r,
			i,
			a
		];
	}
	set(e, t, n, r, i, a) {
		let o = this.planes;
		return o[0].copy(e), o[1].copy(t), o[2].copy(n), o[3].copy(r), o[4].copy(i), o[5].copy(a), this;
	}
	copy(e) {
		let t = this.planes;
		for (let n = 0; n < 6; n++) t[n].copy(e.planes[n]);
		return this;
	}
	setFromProjectionMatrix(e, t = dr, n = !1) {
		let r = this.planes, i = e.elements, a = i[0], o = i[1], s = i[2], c = i[3], l = i[4], u = i[5], d = i[6], f = i[7], p = i[8], m = i[9], h = i[10], g = i[11], _ = i[12], v = i[13], y = i[14], b = i[15];
		if (r[0].setComponents(c - a, f - l, g - p, b - _).normalize(), r[1].setComponents(c + a, f + l, g + p, b + _).normalize(), r[2].setComponents(c + o, f + u, g + m, b + v).normalize(), r[3].setComponents(c - o, f - u, g - m, b - v).normalize(), n) r[4].setComponents(s, d, h, y).normalize(), r[5].setComponents(c - s, f - d, g - h, b - y).normalize();
		else if (r[4].setComponents(c - s, f - d, g - h, b - y).normalize(), t === 2e3) r[5].setComponents(c + s, f + d, g + h, b + y).normalize();
		else if (t === 2001) r[5].setComponents(s, d, h, y).normalize();
		else throw Error("THREE.Frustum.setFromProjectionMatrix(): Invalid coordinate system: " + t);
		return this;
	}
	intersectsObject(e) {
		if (e.boundingSphere !== void 0) e.boundingSphere === null && e.computeBoundingSphere(), Ko.copy(e.boundingSphere).applyMatrix4(e.matrixWorld);
		else {
			let t = e.geometry;
			t.boundingSphere === null && t.computeBoundingSphere(), Ko.copy(t.boundingSphere).applyMatrix4(e.matrixWorld);
		}
		return this.intersectsSphere(Ko);
	}
	intersectsSprite(e) {
		return Ko.center.set(0, 0, 0), Ko.radius = .7071067811865476 + qo.distanceTo(e.center), Ko.applyMatrix4(e.matrixWorld), this.intersectsSphere(Ko);
	}
	intersectsSphere(e) {
		let t = this.planes, n = e.center, r = -e.radius;
		for (let e = 0; e < 6; e++) if (t[e].distanceToPoint(n) < r) return !1;
		return !0;
	}
	intersectsBox(e) {
		let t = this.planes;
		for (let n = 0; n < 6; n++) {
			let r = t[n];
			if (Jo.x = r.normal.x > 0 ? e.max.x : e.min.x, Jo.y = r.normal.y > 0 ? e.max.y : e.min.y, Jo.z = r.normal.z > 0 ? e.max.z : e.min.z, r.distanceToPoint(Jo) < 0) return !1;
		}
		return !0;
	}
	containsPoint(e) {
		let t = this.planes;
		for (let n = 0; n < 6; n++) if (t[n].distanceToPoint(e) < 0) return !1;
		return !0;
	}
	clone() {
		return new this.constructor().copy(this);
	}
}, Xo = class extends Xa {
	constructor(e) {
		super(), this.isLineBasicMaterial = !0, this.type = "LineBasicMaterial", this.color = new X(16777215), this.map = null, this.linewidth = 1, this.linecap = "round", this.linejoin = "round", this.fog = !0, this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.color.copy(e.color), this.map = e.map, this.linewidth = e.linewidth, this.linecap = e.linecap, this.linejoin = e.linejoin, this.fog = e.fog, this;
	}
}, Zo = /*@__PURE__*/ new q(), Qo = /*@__PURE__*/ new q(), $o = /*@__PURE__*/ new vi(), es = /*@__PURE__*/ new io(), ts = /*@__PURE__*/ new Ba(), ns = /*@__PURE__*/ new q(), rs = /*@__PURE__*/ new q(), is = class extends Gi {
	constructor(e = new Ja(), t = new Xo()) {
		super(), this.isLine = !0, this.type = "Line", this.geometry = e, this.material = t, this.morphTargetDictionary = void 0, this.morphTargetInfluences = void 0, this.updateMorphTargets();
	}
	copy(e, t) {
		return super.copy(e, t), this.material = Array.isArray(e.material) ? e.material.slice() : e.material, this.geometry = e.geometry, this;
	}
	computeLineDistances() {
		let e = this.geometry;
		if (e.index === null) {
			let t = e.attributes.position, n = [0];
			for (let e = 1, r = t.count; e < r; e++) Zo.fromBufferAttribute(t, e - 1), Qo.fromBufferAttribute(t, e), n[e] = n[e - 1], n[e] += Zo.distanceTo(Qo);
			e.setAttribute("lineDistance", new Ia(n, 1));
		} else W("Line.computeLineDistances(): Computation only possible with non-indexed BufferGeometry.");
		return this;
	}
	raycast(e, t) {
		let n = this.geometry, r = this.matrixWorld, i = e.params.Line.threshold, a = n.drawRange;
		if (n.boundingSphere === null && n.computeBoundingSphere(), ts.copy(n.boundingSphere), ts.applyMatrix4(r), ts.radius += i, e.ray.intersectsSphere(ts) === !1) return;
		$o.copy(r).invert(), es.copy(e.ray).applyMatrix4($o);
		let o = i / ((this.scale.x + this.scale.y + this.scale.z) / 3), s = o * o, c = this.isLineSegments ? 2 : 1, l = n.index, u = n.attributes.position;
		if (l !== null) {
			let n = Math.max(0, a.start), r = Math.min(l.count, a.start + a.count);
			for (let i = n, a = r - 1; i < a; i += c) {
				let n = l.getX(i), r = l.getX(i + 1), a = as(this, e, es, s, n, r, i);
				a && t.push(a);
			}
			if (this.isLineLoop) {
				let i = l.getX(r - 1), a = l.getX(n), o = as(this, e, es, s, i, a, r - 1);
				o && t.push(o);
			}
		} else {
			let n = Math.max(0, a.start), r = Math.min(u.count, a.start + a.count);
			for (let i = n, a = r - 1; i < a; i += c) {
				let n = as(this, e, es, s, i, i + 1, i);
				n && t.push(n);
			}
			if (this.isLineLoop) {
				let i = as(this, e, es, s, r - 1, n, r - 1);
				i && t.push(i);
			}
		}
	}
	updateMorphTargets() {
		let e = this.geometry.morphAttributes, t = Object.keys(e);
		if (t.length > 0) {
			let n = e[t[0]];
			if (n !== void 0) {
				this.morphTargetInfluences = [], this.morphTargetDictionary = {};
				for (let e = 0, t = n.length; e < t; e++) {
					let t = n[e].name || String(e);
					this.morphTargetInfluences.push(0), this.morphTargetDictionary[t] = e;
				}
			}
		}
	}
};
function as(e, t, n, r, i, a, o) {
	let s = e.geometry.attributes.position;
	if (Zo.fromBufferAttribute(s, i), Qo.fromBufferAttribute(s, a), n.distanceSqToSegment(Zo, Qo, ns, rs) > r) return;
	ns.applyMatrix4(e.matrixWorld);
	let c = t.ray.origin.distanceTo(ns);
	if (!(c < t.near || c > t.far)) return {
		distance: c,
		point: rs.clone().applyMatrix4(e.matrixWorld),
		index: o,
		face: null,
		faceIndex: null,
		barycoord: null,
		object: e
	};
}
var os = /*@__PURE__*/ new q(), ss = /*@__PURE__*/ new q(), cs = class extends is {
	constructor(e, t) {
		super(e, t), this.isLineSegments = !0, this.type = "LineSegments";
	}
	computeLineDistances() {
		let e = this.geometry;
		if (e.index === null) {
			let t = e.attributes.position, n = [];
			for (let e = 0, r = t.count; e < r; e += 2) os.fromBufferAttribute(t, e), ss.fromBufferAttribute(t, e + 1), n[e] = e === 0 ? 0 : n[e - 1], n[e + 1] = n[e] + os.distanceTo(ss);
			e.setAttribute("lineDistance", new Ia(n, 1));
		} else W("LineSegments.computeLineDistances(): Computation only possible with non-indexed BufferGeometry.");
		return this;
	}
}, ls = class extends Xa {
	constructor(e) {
		super(), this.isPointsMaterial = !0, this.type = "PointsMaterial", this.color = new X(16777215), this.map = null, this.alphaMap = null, this.size = 1, this.sizeAttenuation = !0, this.fog = !0, this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.color.copy(e.color), this.map = e.map, this.alphaMap = e.alphaMap, this.size = e.size, this.sizeAttenuation = e.sizeAttenuation, this.fog = e.fog, this;
	}
}, us = /*@__PURE__*/ new vi(), ds = /*@__PURE__*/ new io(), fs = /*@__PURE__*/ new Ba(), ps = /*@__PURE__*/ new q(), ms = class extends Gi {
	constructor(e = new Ja(), t = new ls()) {
		super(), this.isPoints = !0, this.type = "Points", this.geometry = e, this.material = t, this.morphTargetDictionary = void 0, this.morphTargetInfluences = void 0, this.updateMorphTargets();
	}
	copy(e, t) {
		return super.copy(e, t), this.material = Array.isArray(e.material) ? e.material.slice() : e.material, this.geometry = e.geometry, this;
	}
	raycast(e, t) {
		let n = this.geometry, r = this.matrixWorld, i = e.params.Points.threshold, a = n.drawRange;
		if (n.boundingSphere === null && n.computeBoundingSphere(), fs.copy(n.boundingSphere), fs.applyMatrix4(r), fs.radius += i, e.ray.intersectsSphere(fs) === !1) return;
		us.copy(r).invert(), ds.copy(e.ray).applyMatrix4(us);
		let o = i / ((this.scale.x + this.scale.y + this.scale.z) / 3), s = o * o, c = n.index, l = n.attributes.position;
		if (c !== null) {
			let n = Math.max(0, a.start), i = Math.min(c.count, a.start + a.count);
			for (let a = n, o = i; a < o; a++) {
				let n = c.getX(a);
				ps.fromBufferAttribute(l, n), hs(ps, n, s, r, e, t, this);
			}
		} else {
			let n = Math.max(0, a.start), i = Math.min(l.count, a.start + a.count);
			for (let a = n, o = i; a < o; a++) ps.fromBufferAttribute(l, a), hs(ps, a, s, r, e, t, this);
		}
	}
	updateMorphTargets() {
		let e = this.geometry.morphAttributes, t = Object.keys(e);
		if (t.length > 0) {
			let n = e[t[0]];
			if (n !== void 0) {
				this.morphTargetInfluences = [], this.morphTargetDictionary = {};
				for (let e = 0, t = n.length; e < t; e++) {
					let t = n[e].name || String(e);
					this.morphTargetInfluences.push(0), this.morphTargetDictionary[t] = e;
				}
			}
		}
	}
};
function hs(e, t, n, r, i, a, o) {
	let s = ds.distanceSqToPoint(e);
	if (s < n) {
		let n = new q();
		ds.closestPointToPoint(e, n), n.applyMatrix4(r);
		let c = i.ray.origin.distanceTo(n);
		if (c < i.near || c > i.far) return;
		a.push({
			distance: c,
			distanceToRay: Math.sqrt(s),
			point: n,
			index: t,
			face: null,
			faceIndex: null,
			barycoord: null,
			object: o
		});
	}
}
var gs = class extends fi {
	constructor(e = [], t = 301, n, r, i, a, o, s, c, l) {
		super(e, t, n, r, i, a, o, s, c, l), this.isCubeTexture = !0, this.flipY = !1;
	}
	get images() {
		return this.image;
	}
	set images(e) {
		this.image = e;
	}
}, _s = class extends fi {
	constructor(e, t, n = Wt, r, i, a, o = Nt, s = Nt, c, l = tn, u = 1) {
		if (l !== 1026 && l !== 1027) throw Error("DepthTexture format must be either THREE.DepthFormat or THREE.DepthStencilFormat");
		super({
			width: e,
			height: t,
			depth: u
		}, r, i, a, o, s, l, n, c), this.isDepthTexture = !0, this.flipY = !1, this.generateMipmaps = !1, this.compareFunction = null;
	}
	copy(e) {
		return super.copy(e), this.source = new ci(Object.assign({}, e.image)), this.compareFunction = e.compareFunction, this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return this.compareFunction !== null && (t.compareFunction = this.compareFunction), t;
	}
}, vs = class extends _s {
	constructor(e, t = Wt, n = 301, r, i, a = Nt, o = Nt, s, c = tn) {
		let l = {
			width: e,
			height: e,
			depth: 1
		}, u = [
			l,
			l,
			l,
			l,
			l,
			l
		];
		super(e, e, t, n, r, i, a, o, s, c), this.image = u, this.isCubeDepthTexture = !0, this.isCubeTexture = !0;
	}
	get images() {
		return this.image;
	}
	set images(e) {
		this.image = e;
	}
}, ys = class extends fi {
	constructor(e = null) {
		super(), this.sourceTexture = e, this.isExternalTexture = !0;
	}
	copy(e) {
		return super.copy(e), this.sourceTexture = e.sourceTexture, this;
	}
}, bs = class e extends Ja {
	constructor(e = 1, t = 1, n = 1, r = 1, i = 1, a = 1) {
		super(), this.type = "BoxGeometry", this.parameters = {
			width: e,
			height: t,
			depth: n,
			widthSegments: r,
			heightSegments: i,
			depthSegments: a
		};
		let o = this;
		r = Math.floor(r), i = Math.floor(i), a = Math.floor(a);
		let s = [], c = [], l = [], u = [], d = 0, f = 0;
		p("z", "y", "x", -1, -1, n, t, e, a, i, 0), p("z", "y", "x", 1, -1, n, t, -e, a, i, 1), p("x", "z", "y", 1, 1, e, n, t, r, a, 2), p("x", "z", "y", 1, -1, e, n, -t, r, a, 3), p("x", "y", "z", 1, -1, e, t, n, r, i, 4), p("x", "y", "z", -1, -1, e, t, -n, r, i, 5), this.setIndex(s), this.setAttribute("position", new Ia(c, 3)), this.setAttribute("normal", new Ia(l, 3)), this.setAttribute("uv", new Ia(u, 2));
		function p(e, t, n, r, i, a, p, m, h, g, _) {
			let v = a / h, y = p / g, b = a / 2, x = p / 2, S = m / 2, C = h + 1, w = g + 1, T = 0, E = 0, D = new q();
			for (let a = 0; a < w; a++) {
				let o = a * y - x;
				for (let s = 0; s < C; s++) D[e] = (s * v - b) * r, D[t] = o * i, D[n] = S, c.push(D.x, D.y, D.z), D[e] = 0, D[t] = 0, D[n] = m > 0 ? 1 : -1, l.push(D.x, D.y, D.z), u.push(s / h), u.push(1 - a / g), T += 1;
			}
			for (let e = 0; e < g; e++) for (let t = 0; t < h; t++) {
				let n = d + t + C * e, r = d + t + C * (e + 1), i = d + (t + 1) + C * (e + 1), a = d + (t + 1) + C * e;
				s.push(n, r, a), s.push(r, i, a), E += 6;
			}
			o.addGroup(f, E, _), f += E, d += T;
		}
	}
	copy(e) {
		return super.copy(e), this.parameters = Object.assign({}, e.parameters), this;
	}
	static fromJSON(t) {
		return new e(t.width, t.height, t.depth, t.widthSegments, t.heightSegments, t.depthSegments);
	}
}, xs = class e extends Ja {
	constructor(e = 1, t = 1, n = 1, r = 1) {
		super(), this.type = "PlaneGeometry", this.parameters = {
			width: e,
			height: t,
			widthSegments: n,
			heightSegments: r
		};
		let i = e / 2, a = t / 2, o = Math.floor(n), s = Math.floor(r), c = o + 1, l = s + 1, u = e / o, d = t / s, f = [], p = [], m = [], h = [];
		for (let e = 0; e < l; e++) {
			let t = e * d - a;
			for (let n = 0; n < c; n++) {
				let r = n * u - i;
				p.push(r, -t, 0), m.push(0, 0, 1), h.push(n / o), h.push(1 - e / s);
			}
		}
		for (let e = 0; e < s; e++) for (let t = 0; t < o; t++) {
			let n = t + c * e, r = t + c * (e + 1), i = t + 1 + c * (e + 1), a = t + 1 + c * e;
			f.push(n, r, a), f.push(r, i, a);
		}
		this.setIndex(f), this.setAttribute("position", new Ia(p, 3)), this.setAttribute("normal", new Ia(m, 3)), this.setAttribute("uv", new Ia(h, 2));
	}
	copy(e) {
		return super.copy(e), this.parameters = Object.assign({}, e.parameters), this;
	}
	static fromJSON(t) {
		return new e(t.width, t.height, t.widthSegments, t.heightSegments);
	}
}, Ss = class e extends Ja {
	constructor(e = 1, t = 32, n = 16, r = 0, i = Math.PI * 2, a = 0, o = Math.PI) {
		super(), this.type = "SphereGeometry", this.parameters = {
			radius: e,
			widthSegments: t,
			heightSegments: n,
			phiStart: r,
			phiLength: i,
			thetaStart: a,
			thetaLength: o
		}, t = Math.max(3, Math.floor(t)), n = Math.max(2, Math.floor(n));
		let s = Math.min(a + o, Math.PI), c = 0, l = [], u = new q(), d = new q(), f = [], p = [], m = [], h = [];
		for (let f = 0; f <= n; f++) {
			let g = [], _ = f / n, v = 0;
			f === 0 && a === 0 ? v = .5 / t : f === n && s === Math.PI && (v = -.5 / t);
			for (let n = 0; n <= t; n++) {
				let s = n / t;
				u.x = -e * Math.cos(r + s * i) * Math.sin(a + _ * o), u.y = e * Math.cos(a + _ * o), u.z = e * Math.sin(r + s * i) * Math.sin(a + _ * o), p.push(u.x, u.y, u.z), d.copy(u).normalize(), m.push(d.x, d.y, d.z), h.push(s + v, 1 - _), g.push(c++);
			}
			l.push(g);
		}
		for (let e = 0; e < n; e++) for (let r = 0; r < t; r++) {
			let t = l[e][r + 1], i = l[e][r], o = l[e + 1][r], c = l[e + 1][r + 1];
			(e !== 0 || a > 0) && f.push(t, i, c), (e !== n - 1 || s < Math.PI) && f.push(i, o, c);
		}
		this.setIndex(f), this.setAttribute("position", new Ia(p, 3)), this.setAttribute("normal", new Ia(m, 3)), this.setAttribute("uv", new Ia(h, 2));
	}
	copy(e) {
		return super.copy(e), this.parameters = Object.assign({}, e.parameters), this;
	}
	static fromJSON(t) {
		return new e(t.radius, t.widthSegments, t.heightSegments, t.phiStart, t.phiLength, t.thetaStart, t.thetaLength);
	}
};
function Cs(e) {
	let t = {};
	for (let n in e) {
		t[n] = {};
		for (let r in e[n]) {
			let i = e[n][r];
			if (Ts(i)) i.isRenderTargetTexture ? (W("UniformsUtils: Textures of render targets cannot be cloned via cloneUniforms() or mergeUniforms()."), t[n][r] = null) : t[n][r] = i.clone();
			else if (Array.isArray(i)) if (Ts(i[0])) {
				let e = [];
				for (let t = 0, n = i.length; t < n; t++) e[t] = i[t].clone();
				t[n][r] = e;
			} else t[n][r] = i.slice();
			else t[n][r] = i;
		}
	}
	return t;
}
function ws(e) {
	let t = {};
	for (let n = 0; n < e.length; n++) {
		let r = Cs(e[n]);
		for (let e in r) t[e] = r[e];
	}
	return t;
}
function Ts(e) {
	return e && (e.isColor || e.isMatrix3 || e.isMatrix4 || e.isVector2 || e.isVector3 || e.isVector4 || e.isTexture || e.isQuaternion);
}
function Es(e) {
	let t = [];
	for (let n = 0; n < e.length; n++) t.push(e[n].clone());
	return t;
}
function Ds(e) {
	let t = e.getRenderTarget();
	return t === null ? e.outputColorSpace : t.isXRRenderTarget === !0 ? t.texture.colorSpace : Y.workingColorSpace;
}
var Os = {
	clone: Cs,
	merge: ws
}, ks = "void main() {\n	gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );\n}", As = "void main() {\n	gl_FragColor = vec4( 1.0, 0.0, 0.0, 1.0 );\n}", js = class extends Xa {
	constructor(e) {
		super(), this.isShaderMaterial = !0, this.type = "ShaderMaterial", this.defines = {}, this.uniforms = {}, this.uniformsGroups = [], this.vertexShader = ks, this.fragmentShader = As, this.linewidth = 1, this.wireframe = !1, this.wireframeLinewidth = 1, this.fog = !1, this.lights = !1, this.clipping = !1, this.forceSinglePass = !0, this.extensions = {
			clipCullDistance: !1,
			multiDraw: !1
		}, this.defaultAttributeValues = {
			color: [
				1,
				1,
				1
			],
			uv: [0, 0],
			uv1: [0, 0]
		}, this.index0AttributeName = void 0, this.uniformsNeedUpdate = !1, this.glslVersion = null, e !== void 0 && this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.fragmentShader = e.fragmentShader, this.vertexShader = e.vertexShader, this.uniforms = Cs(e.uniforms), this.uniformsGroups = Es(e.uniformsGroups), this.defines = Object.assign({}, e.defines), this.wireframe = e.wireframe, this.wireframeLinewidth = e.wireframeLinewidth, this.fog = e.fog, this.lights = e.lights, this.clipping = e.clipping, this.extensions = Object.assign({}, e.extensions), this.glslVersion = e.glslVersion, this.defaultAttributeValues = Object.assign({}, e.defaultAttributeValues), this.index0AttributeName = e.index0AttributeName, this.uniformsNeedUpdate = e.uniformsNeedUpdate, this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		t.glslVersion = this.glslVersion, t.uniforms = {};
		for (let n in this.uniforms) {
			let r = this.uniforms[n].value;
			r && r.isTexture ? t.uniforms[n] = {
				type: "t",
				value: r.toJSON(e).uuid
			} : r && r.isColor ? t.uniforms[n] = {
				type: "c",
				value: r.getHex()
			} : r && r.isVector2 ? t.uniforms[n] = {
				type: "v2",
				value: r.toArray()
			} : r && r.isVector3 ? t.uniforms[n] = {
				type: "v3",
				value: r.toArray()
			} : r && r.isVector4 ? t.uniforms[n] = {
				type: "v4",
				value: r.toArray()
			} : r && r.isMatrix3 ? t.uniforms[n] = {
				type: "m3",
				value: r.toArray()
			} : r && r.isMatrix4 ? t.uniforms[n] = {
				type: "m4",
				value: r.toArray()
			} : t.uniforms[n] = { value: r };
		}
		Object.keys(this.defines).length > 0 && (t.defines = this.defines), t.vertexShader = this.vertexShader, t.fragmentShader = this.fragmentShader, t.lights = this.lights, t.clipping = this.clipping;
		let n = {};
		for (let e in this.extensions) this.extensions[e] === !0 && (n[e] = !0);
		return Object.keys(n).length > 0 && (t.extensions = n), t;
	}
}, Ms = class extends js {
	constructor(e) {
		super(e), this.isRawShaderMaterial = !0, this.type = "RawShaderMaterial";
	}
}, Ns = class extends Xa {
	constructor(e) {
		super(), this.isMeshStandardMaterial = !0, this.type = "MeshStandardMaterial", this.defines = { STANDARD: "" }, this.color = new X(16777215), this.roughness = 1, this.metalness = 0, this.map = null, this.lightMap = null, this.lightMapIntensity = 1, this.aoMap = null, this.aoMapIntensity = 1, this.emissive = new X(0), this.emissiveIntensity = 1, this.emissiveMap = null, this.bumpMap = null, this.bumpScale = 1, this.normalMap = null, this.normalMapType = 0, this.normalScale = new Yr(1, 1), this.displacementMap = null, this.displacementScale = 1, this.displacementBias = 0, this.roughnessMap = null, this.metalnessMap = null, this.alphaMap = null, this.envMap = null, this.envMapRotation = new Oi(), this.envMapIntensity = 1, this.wireframe = !1, this.wireframeLinewidth = 1, this.wireframeLinecap = "round", this.wireframeLinejoin = "round", this.flatShading = !1, this.fog = !0, this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.defines = { STANDARD: "" }, this.color.copy(e.color), this.roughness = e.roughness, this.metalness = e.metalness, this.map = e.map, this.lightMap = e.lightMap, this.lightMapIntensity = e.lightMapIntensity, this.aoMap = e.aoMap, this.aoMapIntensity = e.aoMapIntensity, this.emissive.copy(e.emissive), this.emissiveMap = e.emissiveMap, this.emissiveIntensity = e.emissiveIntensity, this.bumpMap = e.bumpMap, this.bumpScale = e.bumpScale, this.normalMap = e.normalMap, this.normalMapType = e.normalMapType, this.normalScale.copy(e.normalScale), this.displacementMap = e.displacementMap, this.displacementScale = e.displacementScale, this.displacementBias = e.displacementBias, this.roughnessMap = e.roughnessMap, this.metalnessMap = e.metalnessMap, this.alphaMap = e.alphaMap, this.envMap = e.envMap, this.envMapRotation.copy(e.envMapRotation), this.envMapIntensity = e.envMapIntensity, this.wireframe = e.wireframe, this.wireframeLinewidth = e.wireframeLinewidth, this.wireframeLinecap = e.wireframeLinecap, this.wireframeLinejoin = e.wireframeLinejoin, this.flatShading = e.flatShading, this.fog = e.fog, this;
	}
}, Ps = class extends Xa {
	constructor(e) {
		super(), this.isMeshDepthMaterial = !0, this.type = "MeshDepthMaterial", this.depthPacking = rr, this.map = null, this.alphaMap = null, this.displacementMap = null, this.displacementScale = 1, this.displacementBias = 0, this.wireframe = !1, this.wireframeLinewidth = 1, this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.depthPacking = e.depthPacking, this.map = e.map, this.alphaMap = e.alphaMap, this.displacementMap = e.displacementMap, this.displacementScale = e.displacementScale, this.displacementBias = e.displacementBias, this.wireframe = e.wireframe, this.wireframeLinewidth = e.wireframeLinewidth, this;
	}
}, Fs = class extends Xa {
	constructor(e) {
		super(), this.isMeshDistanceMaterial = !0, this.type = "MeshDistanceMaterial", this.map = null, this.alphaMap = null, this.displacementMap = null, this.displacementScale = 1, this.displacementBias = 0, this.setValues(e);
	}
	copy(e) {
		return super.copy(e), this.map = e.map, this.alphaMap = e.alphaMap, this.displacementMap = e.displacementMap, this.displacementScale = e.displacementScale, this.displacementBias = e.displacementBias, this;
	}
};
function Is(e, t) {
	return !e || e.constructor === t ? e : typeof t.BYTES_PER_ELEMENT == "number" ? new t(e) : Array.prototype.slice.call(e);
}
function Ls(e) {
	function t(t, n) {
		return e[t] - e[n];
	}
	let n = e.length, r = Array(n);
	for (let e = 0; e !== n; ++e) r[e] = e;
	return r.sort(t), r;
}
function Rs(e, t, n) {
	let r = e.length, i = new e.constructor(r);
	for (let a = 0, o = 0; o !== r; ++a) {
		let r = n[a] * t;
		for (let n = 0; n !== t; ++n) i[o++] = e[r + n];
	}
	return i;
}
function zs(e, t, n, r) {
	let i = 1, a = e[0];
	for (; a !== void 0 && a[r] === void 0;) a = e[i++];
	if (a === void 0) return;
	let o = a[r];
	if (o !== void 0) if (Array.isArray(o)) do
		o = a[r], o !== void 0 && (t.push(a.time), n.push(...o)), a = e[i++];
	while (a !== void 0);
	else if (o.toArray !== void 0) do
		o = a[r], o !== void 0 && (t.push(a.time), o.toArray(n, n.length)), a = e[i++];
	while (a !== void 0);
	else do
		o = a[r], o !== void 0 && (t.push(a.time), n.push(o)), a = e[i++];
	while (a !== void 0);
}
var Bs = class {
	constructor(e, t, n, r) {
		this.parameterPositions = e, this._cachedIndex = 0, this.resultBuffer = r === void 0 ? new t.constructor(n) : r, this.sampleValues = t, this.valueSize = n, this.settings = null, this.DefaultSettings_ = {};
	}
	evaluate(e) {
		let t = this.parameterPositions, n = this._cachedIndex, r = t[n], i = t[n - 1];
		validate_interval: {
			seek: {
				let a;
				linear_scan: {
					forward_scan: if (!(e < r)) {
						for (let a = n + 2;;) {
							if (r === void 0) {
								if (e < i) break forward_scan;
								return n = t.length, this._cachedIndex = n, this.copySampleValue_(n - 1);
							}
							if (n === a) break;
							if (i = r, r = t[++n], e < r) break seek;
						}
						a = t.length;
						break linear_scan;
					}
					if (!(e >= i)) {
						let o = t[1];
						e < o && (n = 2, i = o);
						for (let a = n - 2;;) {
							if (i === void 0) return this._cachedIndex = 0, this.copySampleValue_(0);
							if (n === a) break;
							if (r = i, i = t[--n - 1], e >= i) break seek;
						}
						a = n, n = 0;
						break linear_scan;
					}
					break validate_interval;
				}
				for (; n < a;) {
					let r = n + a >>> 1;
					e < t[r] ? a = r : n = r + 1;
				}
				if (r = t[n], i = t[n - 1], i === void 0) return this._cachedIndex = 0, this.copySampleValue_(0);
				if (r === void 0) return n = t.length, this._cachedIndex = n, this.copySampleValue_(n - 1);
			}
			this._cachedIndex = n, this.intervalChanged_(n, i, r);
		}
		return this.interpolate_(n, i, e, r);
	}
	getSettings_() {
		return this.settings || this.DefaultSettings_;
	}
	copySampleValue_(e) {
		let t = this.resultBuffer, n = this.sampleValues, r = this.valueSize, i = e * r;
		for (let e = 0; e !== r; ++e) t[e] = n[i + e];
		return t;
	}
	interpolate_() {
		throw Error("call to abstract method");
	}
	intervalChanged_() {}
}, Vs = class extends Bs {
	constructor(e, t, n, r) {
		super(e, t, n, r), this._weightPrev = -0, this._offsetPrev = -0, this._weightNext = -0, this._offsetNext = -0, this.DefaultSettings_ = {
			endingStart: Qn,
			endingEnd: Qn
		};
	}
	intervalChanged_(e, t, n) {
		let r = this.parameterPositions, i = e - 2, a = e + 1, o = r[i], s = r[a];
		if (o === void 0) switch (this.getSettings_().endingStart) {
			case $n:
				i = e, o = 2 * t - n;
				break;
			case er:
				i = r.length - 2, o = t + r[i] - r[i + 1];
				break;
			default: i = e, o = n;
		}
		if (s === void 0) switch (this.getSettings_().endingEnd) {
			case $n:
				a = e, s = 2 * n - t;
				break;
			case er:
				a = 1, s = n + r[1] - r[0];
				break;
			default: a = e - 1, s = t;
		}
		let c = (n - t) * .5, l = this.valueSize;
		this._weightPrev = c / (t - o), this._weightNext = c / (s - n), this._offsetPrev = i * l, this._offsetNext = a * l;
	}
	interpolate_(e, t, n, r) {
		let i = this.resultBuffer, a = this.sampleValues, o = this.valueSize, s = e * o, c = s - o, l = this._offsetPrev, u = this._offsetNext, d = this._weightPrev, f = this._weightNext, p = (n - t) / (r - t), m = p * p, h = m * p, g = -d * h + 2 * d * m - d * p, _ = (1 + d) * h + (-1.5 - 2 * d) * m + (-.5 + d) * p + 1, v = (-1 - f) * h + (1.5 + f) * m + .5 * p, y = f * h - f * m;
		for (let e = 0; e !== o; ++e) i[e] = g * a[l + e] + _ * a[c + e] + v * a[s + e] + y * a[u + e];
		return i;
	}
}, Hs = class extends Bs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
	interpolate_(e, t, n, r) {
		let i = this.resultBuffer, a = this.sampleValues, o = this.valueSize, s = e * o, c = s - o, l = (n - t) / (r - t), u = 1 - l;
		for (let e = 0; e !== o; ++e) i[e] = a[c + e] * u + a[s + e] * l;
		return i;
	}
}, Us = class extends Bs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
	interpolate_(e) {
		return this.copySampleValue_(e - 1);
	}
}, Ws = class extends Bs {
	interpolate_(e, t, n, r) {
		let i = this.resultBuffer, a = this.sampleValues, o = this.valueSize, s = e * o, c = s - o, l = this.settings || this.DefaultSettings_, u = l.inTangents, d = l.outTangents;
		if (!u || !d) {
			let e = (n - t) / (r - t), l = 1 - e;
			for (let t = 0; t !== o; ++t) i[t] = a[c + t] * l + a[s + t] * e;
			return i;
		}
		let f = o * 2, p = e - 1;
		for (let l = 0; l !== o; ++l) {
			let o = a[c + l], m = a[s + l], h = p * f + l * 2, g = d[h], _ = d[h + 1], v = e * f + l * 2, y = u[v], b = u[v + 1], x = (n - t) / (r - t), S, C, w, T, E;
			for (let e = 0; e < 8; e++) {
				S = x * x, C = S * x, w = 1 - x, T = w * w, E = T * w;
				let e = E * t + 3 * T * x * g + 3 * w * S * y + C * r - n;
				if (Math.abs(e) < 1e-10) break;
				let i = 3 * T * (g - t) + 6 * w * x * (y - g) + 3 * S * (r - y);
				if (Math.abs(i) < 1e-10) break;
				x -= e / i, x = Math.max(0, Math.min(1, x));
			}
			i[l] = E * o + 3 * T * x * _ + 3 * w * S * b + C * m;
		}
		return i;
	}
}, Gs = class {
	constructor(e, t, n, r) {
		if (e === void 0) throw Error("THREE.KeyframeTrack: track name is undefined");
		if (t === void 0 || t.length === 0) throw Error("THREE.KeyframeTrack: no keyframes in track named " + e);
		this.name = e, this.times = Is(t, this.TimeBufferType), this.values = Is(n, this.ValueBufferType), this.setInterpolation(r || this.DefaultInterpolation);
	}
	static toJSON(e) {
		let t = e.constructor, n;
		if (t.toJSON !== this.toJSON) n = t.toJSON(e);
		else {
			n = {
				name: e.name,
				times: Is(e.times, Array),
				values: Is(e.values, Array)
			};
			let t = e.getInterpolation();
			t !== e.DefaultInterpolation && (n.interpolation = t);
		}
		return n.type = e.ValueTypeName, n;
	}
	InterpolantFactoryMethodDiscrete(e) {
		return new Us(this.times, this.values, this.getValueSize(), e);
	}
	InterpolantFactoryMethodLinear(e) {
		return new Hs(this.times, this.values, this.getValueSize(), e);
	}
	InterpolantFactoryMethodSmooth(e) {
		return new Vs(this.times, this.values, this.getValueSize(), e);
	}
	InterpolantFactoryMethodBezier(e) {
		let t = new Ws(this.times, this.values, this.getValueSize(), e);
		return this.settings && (t.settings = this.settings), t;
	}
	setInterpolation(e) {
		let t;
		switch (e) {
			case Jn:
				t = this.InterpolantFactoryMethodDiscrete;
				break;
			case Yn:
				t = this.InterpolantFactoryMethodLinear;
				break;
			case Xn:
				t = this.InterpolantFactoryMethodSmooth;
				break;
			case Zn:
				t = this.InterpolantFactoryMethodBezier;
				break;
		}
		if (t === void 0) {
			let t = "unsupported interpolation for " + this.ValueTypeName + " keyframe track named " + this.name;
			if (this.createInterpolant === void 0) if (e !== this.DefaultInterpolation) this.setInterpolation(this.DefaultInterpolation);
			else throw Error(t);
			return W("KeyframeTrack:", t), this;
		}
		return this.createInterpolant = t, this;
	}
	getInterpolation() {
		switch (this.createInterpolant) {
			case this.InterpolantFactoryMethodDiscrete: return Jn;
			case this.InterpolantFactoryMethodLinear: return Yn;
			case this.InterpolantFactoryMethodSmooth: return Xn;
			case this.InterpolantFactoryMethodBezier: return Zn;
		}
	}
	getValueSize() {
		return this.values.length / this.times.length;
	}
	shift(e) {
		if (e !== 0) {
			let t = this.times;
			for (let n = 0, r = t.length; n !== r; ++n) t[n] += e;
		}
		return this;
	}
	scale(e) {
		if (e !== 1) {
			let t = this.times;
			for (let n = 0, r = t.length; n !== r; ++n) t[n] *= e;
		}
		return this;
	}
	trim(e, t) {
		let n = this.times, r = n.length, i = 0, a = r - 1;
		for (; i !== r && n[i] < e;) ++i;
		for (; a !== -1 && n[a] > t;) --a;
		if (++a, i !== 0 || a !== r) {
			i >= a && (a = Math.max(a, 1), i = a - 1);
			let e = this.getValueSize();
			this.times = n.slice(i, a), this.values = this.values.slice(i * e, a * e);
		}
		return this;
	}
	validate() {
		let e = !0, t = this.getValueSize();
		t - Math.floor(t) !== 0 && (G("KeyframeTrack: Invalid value size in track.", this), e = !1);
		let n = this.times, r = this.values, i = n.length;
		i === 0 && (G("KeyframeTrack: Track is empty.", this), e = !1);
		let a = null;
		for (let t = 0; t !== i; t++) {
			let r = n[t];
			if (typeof r == "number" && isNaN(r)) {
				G("KeyframeTrack: Time is not a valid number.", this, t, r), e = !1;
				break;
			}
			if (a !== null && a > r) {
				G("KeyframeTrack: Out of order keys.", this, t, r, a), e = !1;
				break;
			}
			a = r;
		}
		if (r !== void 0 && pr(r)) for (let t = 0, n = r.length; t !== n; ++t) {
			let n = r[t];
			if (isNaN(n)) {
				G("KeyframeTrack: Value is not a valid number.", this, t, n), e = !1;
				break;
			}
		}
		return e;
	}
	optimize() {
		let e = this.times.slice(), t = this.values.slice(), n = this.getValueSize(), r = this.getInterpolation() === Xn, i = e.length - 1, a = 1;
		for (let o = 1; o < i; ++o) {
			let i = !1, s = e[o];
			if (s !== e[o + 1] && (o !== 1 || s !== e[0])) if (r) i = !0;
			else {
				let e = o * n, r = e - n, a = e + n;
				for (let o = 0; o !== n; ++o) {
					let n = t[e + o];
					if (n !== t[r + o] || n !== t[a + o]) {
						i = !0;
						break;
					}
				}
			}
			if (i) {
				if (o !== a) {
					e[a] = e[o];
					let r = o * n, i = a * n;
					for (let e = 0; e !== n; ++e) t[i + e] = t[r + e];
				}
				++a;
			}
		}
		if (i > 0) {
			e[a] = e[i];
			for (let e = i * n, r = a * n, o = 0; o !== n; ++o) t[r + o] = t[e + o];
			++a;
		}
		return a === e.length ? (this.times = e, this.values = t) : (this.times = e.slice(0, a), this.values = t.slice(0, a * n)), this;
	}
	clone() {
		let e = this.times.slice(), t = this.values.slice(), n = this.constructor, r = new n(this.name, e, t);
		return r.createInterpolant = this.createInterpolant, r;
	}
};
Gs.prototype.ValueTypeName = "", Gs.prototype.TimeBufferType = Float32Array, Gs.prototype.ValueBufferType = Float32Array, Gs.prototype.DefaultInterpolation = Yn;
var Ks = class extends Gs {
	constructor(e, t, n) {
		super(e, t, n);
	}
};
Ks.prototype.ValueTypeName = "bool", Ks.prototype.ValueBufferType = Array, Ks.prototype.DefaultInterpolation = Jn, Ks.prototype.InterpolantFactoryMethodLinear = void 0, Ks.prototype.InterpolantFactoryMethodSmooth = void 0;
var qs = class extends Gs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
};
qs.prototype.ValueTypeName = "color";
var Js = class extends Gs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
};
Js.prototype.ValueTypeName = "number";
var Ys = class extends Bs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
	interpolate_(e, t, n, r) {
		let i = this.resultBuffer, a = this.sampleValues, o = this.valueSize, s = (n - t) / (r - t), c = e * o;
		for (let e = c + o; c !== e; c += 4) Xr.slerpFlat(i, 0, a, c - o, a, c, s);
		return i;
	}
}, Xs = class extends Gs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
	InterpolantFactoryMethodLinear(e) {
		return new Ys(this.times, this.values, this.getValueSize(), e);
	}
};
Xs.prototype.ValueTypeName = "quaternion", Xs.prototype.InterpolantFactoryMethodSmooth = void 0;
var Zs = class extends Gs {
	constructor(e, t, n) {
		super(e, t, n);
	}
};
Zs.prototype.ValueTypeName = "string", Zs.prototype.ValueBufferType = Array, Zs.prototype.DefaultInterpolation = Jn, Zs.prototype.InterpolantFactoryMethodLinear = void 0, Zs.prototype.InterpolantFactoryMethodSmooth = void 0;
var Qs = class extends Gs {
	constructor(e, t, n, r) {
		super(e, t, n, r);
	}
};
Qs.prototype.ValueTypeName = "vector";
var $s = class {
	constructor(e = "", t = -1, n = [], r = tr) {
		this.name = e, this.tracks = n, this.duration = t, this.blendMode = r, this.uuid = Dr(), this.userData = {}, this.duration < 0 && this.resetDuration();
	}
	static parse(e) {
		let t = [], n = e.tracks, r = 1 / (e.fps || 1);
		for (let e = 0, i = n.length; e !== i; ++e) t.push(tc(n[e]).scale(r));
		let i = new this(e.name, e.duration, t, e.blendMode);
		return i.uuid = e.uuid, i.userData = JSON.parse(e.userData || "{}"), i;
	}
	static toJSON(e) {
		let t = [], n = e.tracks, r = {
			name: e.name,
			duration: e.duration,
			tracks: t,
			uuid: e.uuid,
			blendMode: e.blendMode,
			userData: JSON.stringify(e.userData)
		};
		for (let e = 0, r = n.length; e !== r; ++e) t.push(Gs.toJSON(n[e]));
		return r;
	}
	static CreateFromMorphTargetSequence(e, t, n, r) {
		let i = t.length, a = [];
		for (let e = 0; e < i; e++) {
			let o = [], s = [];
			o.push((e + i - 1) % i, e, (e + 1) % i), s.push(0, 1, 0);
			let c = Ls(o);
			o = Rs(o, 1, c), s = Rs(s, 1, c), !r && o[0] === 0 && (o.push(i), s.push(s[0])), a.push(new Js(".morphTargetInfluences[" + t[e].name + "]", o, s).scale(1 / n));
		}
		return new this(e, -1, a);
	}
	static findByName(e, t) {
		let n = e;
		if (!Array.isArray(e)) {
			let t = e;
			n = t.geometry && t.geometry.animations || t.animations;
		}
		for (let e = 0; e < n.length; e++) if (n[e].name === t) return n[e];
		return null;
	}
	static CreateClipsFromMorphTargetSequences(e, t, n) {
		let r = {}, i = /^([\w-]*?)([\d]+)$/;
		for (let t = 0, n = e.length; t < n; t++) {
			let n = e[t], a = n.name.match(i);
			if (a && a.length > 1) {
				let e = a[1], t = r[e];
				t || (r[e] = t = []), t.push(n);
			}
		}
		let a = [];
		for (let e in r) a.push(this.CreateFromMorphTargetSequence(e, r[e], t, n));
		return a;
	}
	static parseAnimation(e, t) {
		if (W("AnimationClip: parseAnimation() is deprecated and will be removed with r185"), !e) return G("AnimationClip: No animation in JSONLoader data."), null;
		let n = function(e, t, n, r, i) {
			if (n.length !== 0) {
				let a = [], o = [];
				zs(n, a, o, r), a.length !== 0 && i.push(new e(t, a, o));
			}
		}, r = [], i = e.name || "default", a = e.fps || 30, o = e.blendMode, s = e.length || -1, c = e.hierarchy || [];
		for (let e = 0; e < c.length; e++) {
			let i = c[e].keys;
			if (!(!i || i.length === 0)) if (i[0].morphTargets) {
				let e = {}, t;
				for (t = 0; t < i.length; t++) if (i[t].morphTargets) for (let n = 0; n < i[t].morphTargets.length; n++) e[i[t].morphTargets[n]] = -1;
				for (let n in e) {
					let e = [], a = [];
					for (let r = 0; r !== i[t].morphTargets.length; ++r) {
						let r = i[t];
						e.push(r.time), a.push(+(r.morphTarget === n));
					}
					r.push(new Js(".morphTargetInfluence[" + n + "]", e, a));
				}
				s = e.length * a;
			} else {
				let a = ".bones[" + t[e].name + "]";
				n(Qs, a + ".position", i, "pos", r), n(Xs, a + ".quaternion", i, "rot", r), n(Qs, a + ".scale", i, "scl", r);
			}
		}
		return r.length === 0 ? null : new this(i, s, r, o);
	}
	resetDuration() {
		let e = this.tracks, t = 0;
		for (let n = 0, r = e.length; n !== r; ++n) {
			let e = this.tracks[n];
			t = Math.max(t, e.times[e.times.length - 1]);
		}
		return this.duration = t, this;
	}
	trim() {
		for (let e = 0; e < this.tracks.length; e++) this.tracks[e].trim(0, this.duration);
		return this;
	}
	validate() {
		let e = !0;
		for (let t = 0; t < this.tracks.length; t++) e &&= this.tracks[t].validate();
		return e;
	}
	optimize() {
		for (let e = 0; e < this.tracks.length; e++) this.tracks[e].optimize();
		return this;
	}
	clone() {
		let e = [];
		for (let t = 0; t < this.tracks.length; t++) e.push(this.tracks[t].clone());
		let t = new this.constructor(this.name, this.duration, e, this.blendMode);
		return t.userData = JSON.parse(JSON.stringify(this.userData)), t;
	}
	toJSON() {
		return this.constructor.toJSON(this);
	}
};
function ec(e) {
	switch (e.toLowerCase()) {
		case "scalar":
		case "double":
		case "float":
		case "number":
		case "integer": return Js;
		case "vector":
		case "vector2":
		case "vector3":
		case "vector4": return Qs;
		case "color": return qs;
		case "quaternion": return Xs;
		case "bool":
		case "boolean": return Ks;
		case "string": return Zs;
	}
	throw Error("THREE.KeyframeTrack: Unsupported typeName: " + e);
}
function tc(e) {
	if (e.type === void 0) throw Error("THREE.KeyframeTrack: track type undefined, can not parse");
	let t = ec(e.type);
	if (e.times === void 0) {
		let t = [], n = [];
		zs(e.keys, t, n, "value"), e.times = t, e.values = n;
	}
	return t.parse === void 0 ? new t(e.name, e.times, e.values, e.interpolation) : t.parse(e);
}
var nc = /*@__PURE__*/ new class {
	constructor(e, t, n) {
		let r = this, i = !1, a = 0, o = 0, s, c = [];
		this.onStart = void 0, this.onLoad = e, this.onProgress = t, this.onError = n, this._abortController = null, this.itemStart = function(e) {
			o++, i === !1 && r.onStart !== void 0 && r.onStart(e, a, o), i = !0;
		}, this.itemEnd = function(e) {
			a++, r.onProgress !== void 0 && r.onProgress(e, a, o), a === o && (i = !1, r.onLoad !== void 0 && r.onLoad());
		}, this.itemError = function(e) {
			r.onError !== void 0 && r.onError(e);
		}, this.resolveURL = function(e) {
			return s ? s(e) : e;
		}, this.setURLModifier = function(e) {
			return s = e, this;
		}, this.addHandler = function(e, t) {
			return c.push(e, t), this;
		}, this.removeHandler = function(e) {
			let t = c.indexOf(e);
			return t !== -1 && c.splice(t, 2), this;
		}, this.getHandler = function(e) {
			for (let t = 0, n = c.length; t < n; t += 2) {
				let n = c[t], r = c[t + 1];
				if (n.global && (n.lastIndex = 0), n.test(e)) return r;
			}
			return null;
		}, this.abort = function() {
			return this.abortController.abort(), this._abortController = null, this;
		};
	}
	get abortController() {
		return this._abortController ||= new AbortController(), this._abortController;
	}
}(), rc = class {
	constructor(e) {
		this.manager = e === void 0 ? nc : e, this.crossOrigin = "anonymous", this.withCredentials = !1, this.path = "", this.resourcePath = "", this.requestHeader = {}, typeof __THREE_DEVTOOLS__ < "u" && __THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe", { detail: this }));
	}
	load() {}
	loadAsync(e, t) {
		let n = this;
		return new Promise(function(r, i) {
			n.load(e, r, t, i);
		});
	}
	parse() {}
	setCrossOrigin(e) {
		return this.crossOrigin = e, this;
	}
	setWithCredentials(e) {
		return this.withCredentials = e, this;
	}
	setPath(e) {
		return this.path = e, this;
	}
	setResourcePath(e) {
		return this.resourcePath = e, this;
	}
	setRequestHeader(e) {
		return this.requestHeader = e, this;
	}
	abort() {
		return this;
	}
};
rc.DEFAULT_MATERIAL_NAME = "__DEFAULT";
var ic = class extends Gi {
	constructor(e, t = 1) {
		super(), this.isLight = !0, this.type = "Light", this.color = new X(e), this.intensity = t;
	}
	dispose() {
		this.dispatchEvent({ type: "dispose" });
	}
	copy(e, t) {
		return super.copy(e, t), this.color.copy(e.color), this.intensity = e.intensity, this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.color = this.color.getHex(), t.object.intensity = this.intensity, t;
	}
}, ac = class extends ic {
	constructor(e, t, n) {
		super(e, n), this.isHemisphereLight = !0, this.type = "HemisphereLight", this.position.copy(Gi.DEFAULT_UP), this.updateMatrix(), this.groundColor = new X(t);
	}
	copy(e, t) {
		return super.copy(e, t), this.groundColor.copy(e.groundColor), this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.groundColor = this.groundColor.getHex(), t;
	}
}, oc = /*@__PURE__*/ new vi(), sc = /*@__PURE__*/ new q(), cc = /*@__PURE__*/ new q(), lc = class {
	constructor(e) {
		this.camera = e, this.intensity = 1, this.bias = 0, this.biasNode = null, this.normalBias = 0, this.radius = 1, this.blurSamples = 8, this.mapSize = new Yr(512, 512), this.mapType = zt, this.map = null, this.mapPass = null, this.matrix = new vi(), this.autoUpdate = !0, this.needsUpdate = !1, this._frustum = new Yo(), this._frameExtents = new Yr(1, 1), this._viewportCount = 1, this._viewports = [new pi(0, 0, 1, 1)];
	}
	getViewportCount() {
		return this._viewportCount;
	}
	getFrustum() {
		return this._frustum;
	}
	updateMatrices(e) {
		let t = this.camera, n = this.matrix;
		sc.setFromMatrixPosition(e.matrixWorld), t.position.copy(sc), cc.setFromMatrixPosition(e.target.matrixWorld), t.lookAt(cc), t.updateMatrixWorld(), oc.multiplyMatrices(t.projectionMatrix, t.matrixWorldInverse), this._frustum.setFromProjectionMatrix(oc, t.coordinateSystem, t.reversedDepth), t.coordinateSystem === 2001 || t.reversedDepth ? n.set(.5, 0, 0, .5, 0, .5, 0, .5, 0, 0, 1, 0, 0, 0, 0, 1) : n.set(.5, 0, 0, .5, 0, .5, 0, .5, 0, 0, .5, .5, 0, 0, 0, 1), n.multiply(oc);
	}
	getViewport(e) {
		return this._viewports[e];
	}
	getFrameExtents() {
		return this._frameExtents;
	}
	dispose() {
		this.map && this.map.dispose(), this.mapPass && this.mapPass.dispose();
	}
	copy(e) {
		return this.camera = e.camera.clone(), this.intensity = e.intensity, this.bias = e.bias, this.radius = e.radius, this.autoUpdate = e.autoUpdate, this.needsUpdate = e.needsUpdate, this.normalBias = e.normalBias, this.blurSamples = e.blurSamples, this.mapSize.copy(e.mapSize), this.biasNode = e.biasNode, this;
	}
	clone() {
		return new this.constructor().copy(this);
	}
	toJSON() {
		let e = {};
		return this.intensity !== 1 && (e.intensity = this.intensity), this.bias !== 0 && (e.bias = this.bias), this.normalBias !== 0 && (e.normalBias = this.normalBias), this.radius !== 1 && (e.radius = this.radius), (this.mapSize.x !== 512 || this.mapSize.y !== 512) && (e.mapSize = this.mapSize.toArray()), e.camera = this.camera.toJSON(!1).object, delete e.camera.matrix, e;
	}
}, uc = /*@__PURE__*/ new q(), dc = /*@__PURE__*/ new Xr(), fc = /*@__PURE__*/ new q(), pc = class extends Gi {
	constructor() {
		super(), this.isCamera = !0, this.type = "Camera", this.matrixWorldInverse = new vi(), this.projectionMatrix = new vi(), this.projectionMatrixInverse = new vi(), this.coordinateSystem = dr, this._reversedDepth = !1;
	}
	get reversedDepth() {
		return this._reversedDepth;
	}
	copy(e, t) {
		return super.copy(e, t), this.matrixWorldInverse.copy(e.matrixWorldInverse), this.projectionMatrix.copy(e.projectionMatrix), this.projectionMatrixInverse.copy(e.projectionMatrixInverse), this.coordinateSystem = e.coordinateSystem, this;
	}
	getWorldDirection(e) {
		return super.getWorldDirection(e).negate();
	}
	updateMatrixWorld(e) {
		super.updateMatrixWorld(e), this.matrixWorld.decompose(uc, dc, fc), fc.x === 1 && fc.y === 1 && fc.z === 1 ? this.matrixWorldInverse.copy(this.matrixWorld).invert() : this.matrixWorldInverse.compose(uc, dc, fc.set(1, 1, 1)).invert();
	}
	updateWorldMatrix(e, t) {
		super.updateWorldMatrix(e, t), this.matrixWorld.decompose(uc, dc, fc), fc.x === 1 && fc.y === 1 && fc.z === 1 ? this.matrixWorldInverse.copy(this.matrixWorld).invert() : this.matrixWorldInverse.compose(uc, dc, fc.set(1, 1, 1)).invert();
	}
	clone() {
		return new this.constructor().copy(this);
	}
}, mc = /*@__PURE__*/ new q(), hc = /*@__PURE__*/ new Yr(), gc = /*@__PURE__*/ new Yr(), _c = class extends pc {
	constructor(e = 50, t = 1, n = .1, r = 2e3) {
		super(), this.isPerspectiveCamera = !0, this.type = "PerspectiveCamera", this.fov = e, this.zoom = 1, this.near = n, this.far = r, this.focus = 10, this.aspect = t, this.view = null, this.filmGauge = 35, this.filmOffset = 0, this.updateProjectionMatrix();
	}
	copy(e, t) {
		return super.copy(e, t), this.fov = e.fov, this.zoom = e.zoom, this.near = e.near, this.far = e.far, this.focus = e.focus, this.aspect = e.aspect, this.view = e.view === null ? null : Object.assign({}, e.view), this.filmGauge = e.filmGauge, this.filmOffset = e.filmOffset, this;
	}
	setFocalLength(e) {
		let t = .5 * this.getFilmHeight() / e;
		this.fov = Er * 2 * Math.atan(t), this.updateProjectionMatrix();
	}
	getFocalLength() {
		let e = Math.tan(Tr * .5 * this.fov);
		return .5 * this.getFilmHeight() / e;
	}
	getEffectiveFOV() {
		return Er * 2 * Math.atan(Math.tan(Tr * .5 * this.fov) / this.zoom);
	}
	getFilmWidth() {
		return this.filmGauge * Math.min(this.aspect, 1);
	}
	getFilmHeight() {
		return this.filmGauge / Math.max(this.aspect, 1);
	}
	getViewBounds(e, t, n) {
		mc.set(-1, -1, .5).applyMatrix4(this.projectionMatrixInverse), t.set(mc.x, mc.y).multiplyScalar(-e / mc.z), mc.set(1, 1, .5).applyMatrix4(this.projectionMatrixInverse), n.set(mc.x, mc.y).multiplyScalar(-e / mc.z);
	}
	getViewSize(e, t) {
		return this.getViewBounds(e, hc, gc), t.subVectors(gc, hc);
	}
	setViewOffset(e, t, n, r, i, a) {
		this.aspect = e / t, this.view === null && (this.view = {
			enabled: !0,
			fullWidth: 1,
			fullHeight: 1,
			offsetX: 0,
			offsetY: 0,
			width: 1,
			height: 1
		}), this.view.enabled = !0, this.view.fullWidth = e, this.view.fullHeight = t, this.view.offsetX = n, this.view.offsetY = r, this.view.width = i, this.view.height = a, this.updateProjectionMatrix();
	}
	clearViewOffset() {
		this.view !== null && (this.view.enabled = !1), this.updateProjectionMatrix();
	}
	updateProjectionMatrix() {
		let e = this.near, t = e * Math.tan(Tr * .5 * this.fov) / this.zoom, n = 2 * t, r = this.aspect * n, i = -.5 * r, a = this.view;
		if (this.view !== null && this.view.enabled) {
			let e = a.fullWidth, o = a.fullHeight;
			i += a.offsetX * r / e, t -= a.offsetY * n / o, r *= a.width / e, n *= a.height / o;
		}
		let o = this.filmOffset;
		o !== 0 && (i += e * o / this.getFilmWidth()), this.projectionMatrix.makePerspective(i, i + r, t, t - n, e, this.far, this.coordinateSystem, this.reversedDepth), this.projectionMatrixInverse.copy(this.projectionMatrix).invert();
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.fov = this.fov, t.object.zoom = this.zoom, t.object.near = this.near, t.object.far = this.far, t.object.focus = this.focus, t.object.aspect = this.aspect, this.view !== null && (t.object.view = Object.assign({}, this.view)), t.object.filmGauge = this.filmGauge, t.object.filmOffset = this.filmOffset, t;
	}
}, vc = class extends lc {
	constructor() {
		super(new _c(50, 1, .5, 500)), this.isSpotLightShadow = !0, this.focus = 1, this.aspect = 1;
	}
	updateMatrices(e) {
		let t = this.camera, n = Er * 2 * e.angle * this.focus, r = this.mapSize.width / this.mapSize.height * this.aspect, i = e.distance || t.far;
		(n !== t.fov || r !== t.aspect || i !== t.far) && (t.fov = n, t.aspect = r, t.far = i, t.updateProjectionMatrix()), super.updateMatrices(e);
	}
	copy(e) {
		return super.copy(e), this.focus = e.focus, this;
	}
}, yc = class extends ic {
	constructor(e, t, n = 0, r = Math.PI / 3, i = 0, a = 2) {
		super(e, t), this.isSpotLight = !0, this.type = "SpotLight", this.position.copy(Gi.DEFAULT_UP), this.updateMatrix(), this.target = new Gi(), this.distance = n, this.angle = r, this.penumbra = i, this.decay = a, this.map = null, this.shadow = new vc();
	}
	get power() {
		return this.intensity * Math.PI;
	}
	set power(e) {
		this.intensity = e / Math.PI;
	}
	dispose() {
		super.dispose(), this.shadow.dispose();
	}
	copy(e, t) {
		return super.copy(e, t), this.distance = e.distance, this.angle = e.angle, this.penumbra = e.penumbra, this.decay = e.decay, this.target = e.target.clone(), this.map = e.map, this.shadow = e.shadow.clone(), this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.distance = this.distance, t.object.angle = this.angle, t.object.decay = this.decay, t.object.penumbra = this.penumbra, t.object.target = this.target.uuid, this.map && this.map.isTexture && (t.object.map = this.map.toJSON(e).uuid), t.object.shadow = this.shadow.toJSON(), t;
	}
}, bc = class extends lc {
	constructor() {
		super(new _c(90, 1, .5, 500)), this.isPointLightShadow = !0;
	}
}, xc = class extends ic {
	constructor(e, t, n = 0, r = 2) {
		super(e, t), this.isPointLight = !0, this.type = "PointLight", this.distance = n, this.decay = r, this.shadow = new bc();
	}
	get power() {
		return this.intensity * 4 * Math.PI;
	}
	set power(e) {
		this.intensity = e / (4 * Math.PI);
	}
	dispose() {
		super.dispose(), this.shadow.dispose();
	}
	copy(e, t) {
		return super.copy(e, t), this.distance = e.distance, this.decay = e.decay, this.shadow = e.shadow.clone(), this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.distance = this.distance, t.object.decay = this.decay, t.object.shadow = this.shadow.toJSON(), t;
	}
}, Sc = class extends pc {
	constructor(e = -1, t = 1, n = 1, r = -1, i = .1, a = 2e3) {
		super(), this.isOrthographicCamera = !0, this.type = "OrthographicCamera", this.zoom = 1, this.view = null, this.left = e, this.right = t, this.top = n, this.bottom = r, this.near = i, this.far = a, this.updateProjectionMatrix();
	}
	copy(e, t) {
		return super.copy(e, t), this.left = e.left, this.right = e.right, this.top = e.top, this.bottom = e.bottom, this.near = e.near, this.far = e.far, this.zoom = e.zoom, this.view = e.view === null ? null : Object.assign({}, e.view), this;
	}
	setViewOffset(e, t, n, r, i, a) {
		this.view === null && (this.view = {
			enabled: !0,
			fullWidth: 1,
			fullHeight: 1,
			offsetX: 0,
			offsetY: 0,
			width: 1,
			height: 1
		}), this.view.enabled = !0, this.view.fullWidth = e, this.view.fullHeight = t, this.view.offsetX = n, this.view.offsetY = r, this.view.width = i, this.view.height = a, this.updateProjectionMatrix();
	}
	clearViewOffset() {
		this.view !== null && (this.view.enabled = !1), this.updateProjectionMatrix();
	}
	updateProjectionMatrix() {
		let e = (this.right - this.left) / (2 * this.zoom), t = (this.top - this.bottom) / (2 * this.zoom), n = (this.right + this.left) / 2, r = (this.top + this.bottom) / 2, i = n - e, a = n + e, o = r + t, s = r - t;
		if (this.view !== null && this.view.enabled) {
			let e = (this.right - this.left) / this.view.fullWidth / this.zoom, t = (this.top - this.bottom) / this.view.fullHeight / this.zoom;
			i += e * this.view.offsetX, a = i + e * this.view.width, o -= t * this.view.offsetY, s = o - t * this.view.height;
		}
		this.projectionMatrix.makeOrthographic(i, a, o, s, this.near, this.far, this.coordinateSystem, this.reversedDepth), this.projectionMatrixInverse.copy(this.projectionMatrix).invert();
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.zoom = this.zoom, t.object.left = this.left, t.object.right = this.right, t.object.top = this.top, t.object.bottom = this.bottom, t.object.near = this.near, t.object.far = this.far, this.view !== null && (t.object.view = Object.assign({}, this.view)), t;
	}
}, Cc = class extends lc {
	constructor() {
		super(new Sc(-5, 5, 5, -5, .5, 500)), this.isDirectionalLightShadow = !0;
	}
}, wc = class extends ic {
	constructor(e, t) {
		super(e, t), this.isDirectionalLight = !0, this.type = "DirectionalLight", this.position.copy(Gi.DEFAULT_UP), this.updateMatrix(), this.target = new Gi(), this.shadow = new Cc();
	}
	dispose() {
		super.dispose(), this.shadow.dispose();
	}
	copy(e) {
		return super.copy(e), this.target = e.target.clone(), this.shadow = e.shadow.clone(), this;
	}
	toJSON(e) {
		let t = super.toJSON(e);
		return t.object.shadow = this.shadow.toJSON(), t.object.target = this.target.uuid, t;
	}
}, Tc = class extends ic {
	constructor(e, t) {
		super(e, t), this.isAmbientLight = !0, this.type = "AmbientLight";
	}
}, Ec = -90, Dc = 1, Oc = class extends Gi {
	constructor(e, t, n) {
		super(), this.type = "CubeCamera", this.renderTarget = n, this.coordinateSystem = null, this.activeMipmapLevel = 0;
		let r = new _c(Ec, Dc, e, t);
		r.layers = this.layers, this.add(r);
		let i = new _c(Ec, Dc, e, t);
		i.layers = this.layers, this.add(i);
		let a = new _c(Ec, Dc, e, t);
		a.layers = this.layers, this.add(a);
		let o = new _c(Ec, Dc, e, t);
		o.layers = this.layers, this.add(o);
		let s = new _c(Ec, Dc, e, t);
		s.layers = this.layers, this.add(s);
		let c = new _c(Ec, Dc, e, t);
		c.layers = this.layers, this.add(c);
	}
	updateCoordinateSystem() {
		let e = this.coordinateSystem, t = this.children.concat(), [n, r, i, a, o, s] = t;
		for (let e of t) this.remove(e);
		if (e === 2e3) n.up.set(0, 1, 0), n.lookAt(1, 0, 0), r.up.set(0, 1, 0), r.lookAt(-1, 0, 0), i.up.set(0, 0, -1), i.lookAt(0, 1, 0), a.up.set(0, 0, 1), a.lookAt(0, -1, 0), o.up.set(0, 1, 0), o.lookAt(0, 0, 1), s.up.set(0, 1, 0), s.lookAt(0, 0, -1);
		else if (e === 2001) n.up.set(0, -1, 0), n.lookAt(-1, 0, 0), r.up.set(0, -1, 0), r.lookAt(1, 0, 0), i.up.set(0, 0, 1), i.lookAt(0, 1, 0), a.up.set(0, 0, -1), a.lookAt(0, -1, 0), o.up.set(0, -1, 0), o.lookAt(0, 0, 1), s.up.set(0, -1, 0), s.lookAt(0, 0, -1);
		else throw Error("THREE.CubeCamera.updateCoordinateSystem(): Invalid coordinate system: " + e);
		for (let e of t) this.add(e), e.updateMatrixWorld();
	}
	update(e, t) {
		this.parent === null && this.updateMatrixWorld();
		let { renderTarget: n, activeMipmapLevel: r } = this;
		this.coordinateSystem !== e.coordinateSystem && (this.coordinateSystem = e.coordinateSystem, this.updateCoordinateSystem());
		let [i, a, o, s, c, l] = this.children, u = e.getRenderTarget(), d = e.getActiveCubeFace(), f = e.getActiveMipmapLevel(), p = e.xr.enabled;
		e.xr.enabled = !1;
		let m = n.texture.generateMipmaps;
		n.texture.generateMipmaps = !1;
		let h = !1;
		h = e.isWebGLRenderer === !0 ? e.state.buffers.depth.getReversed() : e.reversedDepthBuffer, e.setRenderTarget(n, 0, r), h && e.autoClear === !1 && e.clearDepth(), e.render(t, i), e.setRenderTarget(n, 1, r), h && e.autoClear === !1 && e.clearDepth(), e.render(t, a), e.setRenderTarget(n, 2, r), h && e.autoClear === !1 && e.clearDepth(), e.render(t, o), e.setRenderTarget(n, 3, r), h && e.autoClear === !1 && e.clearDepth(), e.render(t, s), e.setRenderTarget(n, 4, r), h && e.autoClear === !1 && e.clearDepth(), e.render(t, c), n.texture.generateMipmaps = m, e.setRenderTarget(n, 5, r), h && e.autoClear === !1 && e.clearDepth(), e.render(t, l), e.setRenderTarget(u, d, f), e.xr.enabled = p, n.texture.needsPMREMUpdate = !0;
	}
}, kc = class extends _c {
	constructor(e = []) {
		super(), this.isArrayCamera = !0, this.isMultiViewCamera = !1, this.cameras = e;
	}
}, Ac = class {
	constructor(e, t, n) {
		this.binding = e, this.valueSize = n;
		let r, i, a;
		switch (t) {
			case "quaternion":
				r = this._slerp, i = this._slerpAdditive, a = this._setAdditiveIdentityQuaternion, this.buffer = new Float64Array(n * 6), this._workIndex = 5;
				break;
			case "string":
			case "bool":
				r = this._select, i = this._select, a = this._setAdditiveIdentityOther, this.buffer = Array(n * 5);
				break;
			default: r = this._lerp, i = this._lerpAdditive, a = this._setAdditiveIdentityNumeric, this.buffer = new Float64Array(n * 5);
		}
		this._mixBufferRegion = r, this._mixBufferRegionAdditive = i, this._setIdentity = a, this._origIndex = 3, this._addIndex = 4, this.cumulativeWeight = 0, this.cumulativeWeightAdditive = 0, this.useCount = 0, this.referenceCount = 0;
	}
	accumulate(e, t) {
		let n = this.buffer, r = this.valueSize, i = e * r + r, a = this.cumulativeWeight;
		if (a === 0) {
			for (let e = 0; e !== r; ++e) n[i + e] = n[e];
			a = t;
		} else {
			a += t;
			let e = t / a;
			this._mixBufferRegion(n, i, 0, e, r);
		}
		this.cumulativeWeight = a;
	}
	accumulateAdditive(e) {
		let t = this.buffer, n = this.valueSize, r = n * this._addIndex;
		this.cumulativeWeightAdditive === 0 && this._setIdentity(), this._mixBufferRegionAdditive(t, r, 0, e, n), this.cumulativeWeightAdditive += e;
	}
	apply(e) {
		let t = this.valueSize, n = this.buffer, r = e * t + t, i = this.cumulativeWeight, a = this.cumulativeWeightAdditive, o = this.binding;
		if (this.cumulativeWeight = 0, this.cumulativeWeightAdditive = 0, i < 1) {
			let e = t * this._origIndex;
			this._mixBufferRegion(n, r, e, 1 - i, t);
		}
		a > 0 && this._mixBufferRegionAdditive(n, r, this._addIndex * t, 1, t);
		for (let e = t, i = t + t; e !== i; ++e) if (n[e] !== n[e + t]) {
			o.setValue(n, r);
			break;
		}
	}
	saveOriginalState() {
		let e = this.binding, t = this.buffer, n = this.valueSize, r = n * this._origIndex;
		e.getValue(t, r);
		for (let e = n, i = r; e !== i; ++e) t[e] = t[r + e % n];
		this._setIdentity(), this.cumulativeWeight = 0, this.cumulativeWeightAdditive = 0;
	}
	restoreOriginalState() {
		let e = this.valueSize * 3;
		this.binding.setValue(this.buffer, e);
	}
	_setAdditiveIdentityNumeric() {
		let e = this._addIndex * this.valueSize, t = e + this.valueSize;
		for (let n = e; n < t; n++) this.buffer[n] = 0;
	}
	_setAdditiveIdentityQuaternion() {
		this._setAdditiveIdentityNumeric(), this.buffer[this._addIndex * this.valueSize + 3] = 1;
	}
	_setAdditiveIdentityOther() {
		let e = this._origIndex * this.valueSize, t = this._addIndex * this.valueSize;
		for (let n = 0; n < this.valueSize; n++) this.buffer[t + n] = this.buffer[e + n];
	}
	_select(e, t, n, r, i) {
		if (r >= .5) for (let r = 0; r !== i; ++r) e[t + r] = e[n + r];
	}
	_slerp(e, t, n, r) {
		Xr.slerpFlat(e, t, e, t, e, n, r);
	}
	_slerpAdditive(e, t, n, r, i) {
		let a = this._workIndex * i;
		Xr.multiplyQuaternionsFlat(e, a, e, t, e, n), Xr.slerpFlat(e, t, e, t, e, a, r);
	}
	_lerp(e, t, n, r, i) {
		let a = 1 - r;
		for (let o = 0; o !== i; ++o) {
			let i = t + o;
			e[i] = e[i] * a + e[n + o] * r;
		}
	}
	_lerpAdditive(e, t, n, r, i) {
		for (let a = 0; a !== i; ++a) {
			let i = t + a;
			e[i] = e[i] + e[n + a] * r;
		}
	}
}, jc = "\\[\\]\\.:\\/", Mc = /* @__PURE__ */ RegExp("[\\[\\]\\.:\\/]", "g"), Nc = "[^\\[\\]\\.:\\/]", Pc = "[^" + jc.replace("\\.", "") + "]", Fc = /*@__PURE__*/ "((?:WC+[\\/:])*)".replace("WC", Nc), Ic = /*@__PURE__*/ "(WCOD+)?".replace("WCOD", Pc), Lc = /*@__PURE__*/ "(?:\\.(WC+)(?:\\[(.+)\\])?)?".replace("WC", Nc), Rc = /*@__PURE__*/ "\\.(WC+)(?:\\[(.+)\\])?".replace("WC", Nc), zc = RegExp("^" + Fc + Ic + Lc + Rc + "$"), Bc = [
	"material",
	"materials",
	"bones",
	"map"
], Vc = class {
	constructor(e, t, n) {
		let r = n || Hc.parseTrackName(t);
		this._targetGroup = e, this._bindings = e.subscribe_(t, r);
	}
	getValue(e, t) {
		this.bind();
		let n = this._targetGroup.nCachedObjects_, r = this._bindings[n];
		r !== void 0 && r.getValue(e, t);
	}
	setValue(e, t) {
		let n = this._bindings;
		for (let r = this._targetGroup.nCachedObjects_, i = n.length; r !== i; ++r) n[r].setValue(e, t);
	}
	bind() {
		let e = this._bindings;
		for (let t = this._targetGroup.nCachedObjects_, n = e.length; t !== n; ++t) e[t].bind();
	}
	unbind() {
		let e = this._bindings;
		for (let t = this._targetGroup.nCachedObjects_, n = e.length; t !== n; ++t) e[t].unbind();
	}
}, Hc = class e {
	constructor(t, n, r) {
		this.path = n, this.parsedPath = r || e.parseTrackName(n), this.node = e.findNode(t, this.parsedPath.nodeName), this.rootNode = t, this.getValue = this._getValue_unbound, this.setValue = this._setValue_unbound;
	}
	static create(t, n, r) {
		return t && t.isAnimationObjectGroup ? new e.Composite(t, n, r) : new e(t, n, r);
	}
	static sanitizeNodeName(e) {
		return e.replace(/\s/g, "_").replace(Mc, "");
	}
	static parseTrackName(e) {
		let t = zc.exec(e);
		if (t === null) throw Error("PropertyBinding: Cannot parse trackName: " + e);
		let n = {
			nodeName: t[2],
			objectName: t[3],
			objectIndex: t[4],
			propertyName: t[5],
			propertyIndex: t[6]
		}, r = n.nodeName && n.nodeName.lastIndexOf(".");
		if (r !== void 0 && r !== -1) {
			let e = n.nodeName.substring(r + 1);
			Bc.indexOf(e) !== -1 && (n.nodeName = n.nodeName.substring(0, r), n.objectName = e);
		}
		if (n.propertyName === null || n.propertyName.length === 0) throw Error("PropertyBinding: can not parse propertyName from trackName: " + e);
		return n;
	}
	static findNode(e, t) {
		if (t === void 0 || t === "" || t === "." || t === -1 || t === e.name || t === e.uuid) return e;
		if (e.skeleton) {
			let n = e.skeleton.getBoneByName(t);
			if (n !== void 0) return n;
		}
		if (e.children) {
			let n = function(e) {
				for (let r = 0; r < e.length; r++) {
					let i = e[r];
					if (i.name === t || i.uuid === t) return i;
					let a = n(i.children);
					if (a) return a;
				}
				return null;
			}, r = n(e.children);
			if (r) return r;
		}
		return null;
	}
	_getValue_unavailable() {}
	_setValue_unavailable() {}
	_getValue_direct(e, t) {
		e[t] = this.targetObject[this.propertyName];
	}
	_getValue_array(e, t) {
		let n = this.resolvedProperty;
		for (let r = 0, i = n.length; r !== i; ++r) e[t++] = n[r];
	}
	_getValue_arrayElement(e, t) {
		e[t] = this.resolvedProperty[this.propertyIndex];
	}
	_getValue_toArray(e, t) {
		this.resolvedProperty.toArray(e, t);
	}
	_setValue_direct(e, t) {
		this.targetObject[this.propertyName] = e[t];
	}
	_setValue_direct_setNeedsUpdate(e, t) {
		this.targetObject[this.propertyName] = e[t], this.targetObject.needsUpdate = !0;
	}
	_setValue_direct_setMatrixWorldNeedsUpdate(e, t) {
		this.targetObject[this.propertyName] = e[t], this.targetObject.matrixWorldNeedsUpdate = !0;
	}
	_setValue_array(e, t) {
		let n = this.resolvedProperty;
		for (let r = 0, i = n.length; r !== i; ++r) n[r] = e[t++];
	}
	_setValue_array_setNeedsUpdate(e, t) {
		let n = this.resolvedProperty;
		for (let r = 0, i = n.length; r !== i; ++r) n[r] = e[t++];
		this.targetObject.needsUpdate = !0;
	}
	_setValue_array_setMatrixWorldNeedsUpdate(e, t) {
		let n = this.resolvedProperty;
		for (let r = 0, i = n.length; r !== i; ++r) n[r] = e[t++];
		this.targetObject.matrixWorldNeedsUpdate = !0;
	}
	_setValue_arrayElement(e, t) {
		this.resolvedProperty[this.propertyIndex] = e[t];
	}
	_setValue_arrayElement_setNeedsUpdate(e, t) {
		this.resolvedProperty[this.propertyIndex] = e[t], this.targetObject.needsUpdate = !0;
	}
	_setValue_arrayElement_setMatrixWorldNeedsUpdate(e, t) {
		this.resolvedProperty[this.propertyIndex] = e[t], this.targetObject.matrixWorldNeedsUpdate = !0;
	}
	_setValue_fromArray(e, t) {
		this.resolvedProperty.fromArray(e, t);
	}
	_setValue_fromArray_setNeedsUpdate(e, t) {
		this.resolvedProperty.fromArray(e, t), this.targetObject.needsUpdate = !0;
	}
	_setValue_fromArray_setMatrixWorldNeedsUpdate(e, t) {
		this.resolvedProperty.fromArray(e, t), this.targetObject.matrixWorldNeedsUpdate = !0;
	}
	_getValue_unbound(e, t) {
		this.bind(), this.getValue(e, t);
	}
	_setValue_unbound(e, t) {
		this.bind(), this.setValue(e, t);
	}
	bind() {
		let t = this.node, n = this.parsedPath, r = n.objectName, i = n.propertyName, a = n.propertyIndex;
		if (t || (t = e.findNode(this.rootNode, n.nodeName), this.node = t), this.getValue = this._getValue_unavailable, this.setValue = this._setValue_unavailable, !t) {
			W("PropertyBinding: No target node found for track: " + this.path + ".");
			return;
		}
		if (r) {
			let e = n.objectIndex;
			switch (r) {
				case "materials":
					if (!t.material) {
						G("PropertyBinding: Can not bind to material as node does not have a material.", this);
						return;
					}
					if (!t.material.materials) {
						G("PropertyBinding: Can not bind to material.materials as node.material does not have a materials array.", this);
						return;
					}
					t = t.material.materials;
					break;
				case "bones":
					if (!t.skeleton) {
						G("PropertyBinding: Can not bind to bones as node does not have a skeleton.", this);
						return;
					}
					t = t.skeleton.bones;
					for (let n = 0; n < t.length; n++) if (t[n].name === e) {
						e = n;
						break;
					}
					break;
				case "map":
					if ("map" in t) {
						t = t.map;
						break;
					}
					if (!t.material) {
						G("PropertyBinding: Can not bind to material as node does not have a material.", this);
						return;
					}
					if (!t.material.map) {
						G("PropertyBinding: Can not bind to material.map as node.material does not have a map.", this);
						return;
					}
					t = t.material.map;
					break;
				default:
					if (t[r] === void 0) {
						G("PropertyBinding: Can not bind to objectName of node undefined.", this);
						return;
					}
					t = t[r];
			}
			if (e !== void 0) {
				if (t[e] === void 0) {
					G("PropertyBinding: Trying to bind to objectIndex of objectName, but is undefined.", this, t);
					return;
				}
				t = t[e];
			}
		}
		let o = t[i];
		if (o === void 0) {
			let e = n.nodeName;
			G("PropertyBinding: Trying to update property for track: " + e + "." + i + " but it wasn't found.", t);
			return;
		}
		let s = this.Versioning.None;
		this.targetObject = t, t.isMaterial === !0 ? s = this.Versioning.NeedsUpdate : t.isObject3D === !0 && (s = this.Versioning.MatrixWorldNeedsUpdate);
		let c = this.BindingType.Direct;
		if (a !== void 0) {
			if (i === "morphTargetInfluences") {
				if (!t.geometry) {
					G("PropertyBinding: Can not bind to morphTargetInfluences because node does not have a geometry.", this);
					return;
				}
				if (!t.geometry.morphAttributes) {
					G("PropertyBinding: Can not bind to morphTargetInfluences because node does not have a geometry.morphAttributes.", this);
					return;
				}
				t.morphTargetDictionary[a] !== void 0 && (a = t.morphTargetDictionary[a]);
			}
			c = this.BindingType.ArrayElement, this.resolvedProperty = o, this.propertyIndex = a;
		} else o.fromArray !== void 0 && o.toArray !== void 0 ? (c = this.BindingType.HasFromToArray, this.resolvedProperty = o) : Array.isArray(o) ? (c = this.BindingType.EntireArray, this.resolvedProperty = o) : this.propertyName = i;
		this.getValue = this.GetterByBindingType[c], this.setValue = this.SetterByBindingTypeAndVersioning[c][s];
	}
	unbind() {
		this.node = null, this.getValue = this._getValue_unbound, this.setValue = this._setValue_unbound;
	}
};
Hc.Composite = Vc, Hc.prototype.BindingType = {
	Direct: 0,
	EntireArray: 1,
	ArrayElement: 2,
	HasFromToArray: 3
}, Hc.prototype.Versioning = {
	None: 0,
	NeedsUpdate: 1,
	MatrixWorldNeedsUpdate: 2
}, Hc.prototype.GetterByBindingType = [
	Hc.prototype._getValue_direct,
	Hc.prototype._getValue_array,
	Hc.prototype._getValue_arrayElement,
	Hc.prototype._getValue_toArray
], Hc.prototype.SetterByBindingTypeAndVersioning = [
	[
		Hc.prototype._setValue_direct,
		Hc.prototype._setValue_direct_setNeedsUpdate,
		Hc.prototype._setValue_direct_setMatrixWorldNeedsUpdate
	],
	[
		Hc.prototype._setValue_array,
		Hc.prototype._setValue_array_setNeedsUpdate,
		Hc.prototype._setValue_array_setMatrixWorldNeedsUpdate
	],
	[
		Hc.prototype._setValue_arrayElement,
		Hc.prototype._setValue_arrayElement_setNeedsUpdate,
		Hc.prototype._setValue_arrayElement_setMatrixWorldNeedsUpdate
	],
	[
		Hc.prototype._setValue_fromArray,
		Hc.prototype._setValue_fromArray_setNeedsUpdate,
		Hc.prototype._setValue_fromArray_setMatrixWorldNeedsUpdate
	]
];
var Uc = class {
	constructor(e, t, n = null, r = t.blendMode) {
		this._mixer = e, this._clip = t, this._localRoot = n, this.blendMode = r;
		let i = t.tracks, a = i.length, o = Array(a), s = {
			endingStart: Qn,
			endingEnd: Qn
		};
		for (let e = 0; e !== a; ++e) {
			let t = i[e].createInterpolant(null);
			o[e] = t, t.settings && Object.assign(s, t.settings), t.settings = s;
		}
		this._interpolantSettings = s, this._interpolants = o, this._propertyBindings = Array(a), this._cacheIndex = null, this._byClipCacheIndex = null, this._timeScaleInterpolant = null, this._weightInterpolant = null, this.loop = Kn, this._loopCount = -1, this._startTime = null, this.time = 0, this.timeScale = 1, this._effectiveTimeScale = 1, this.weight = 1, this._effectiveWeight = 1, this.repetitions = Infinity, this.paused = !1, this.enabled = !0, this.clampWhenFinished = !1, this.zeroSlopeAtStart = !0, this.zeroSlopeAtEnd = !0;
	}
	play() {
		return this._mixer._activateAction(this), this;
	}
	stop() {
		return this._mixer._deactivateAction(this), this.reset();
	}
	reset() {
		return this.paused = !1, this.enabled = !0, this.time = 0, this._loopCount = -1, this._startTime = null, this.stopFading().stopWarping();
	}
	isRunning() {
		return this.enabled && !this.paused && this.timeScale !== 0 && this._startTime === null && this._mixer._isActiveAction(this);
	}
	isScheduled() {
		return this._mixer._isActiveAction(this);
	}
	startAt(e) {
		return this._startTime = e, this;
	}
	setLoop(e, t) {
		return this.loop = e, this.repetitions = t, this;
	}
	setEffectiveWeight(e) {
		return this.weight = e, this._effectiveWeight = this.enabled ? e : 0, this.stopFading();
	}
	getEffectiveWeight() {
		return this._effectiveWeight;
	}
	fadeIn(e) {
		return this._scheduleFading(e, 0, 1);
	}
	fadeOut(e) {
		return this._scheduleFading(e, 1, 0);
	}
	crossFadeFrom(e, t, n = !1) {
		if (e.fadeOut(t), this.fadeIn(t), n === !0) {
			let n = this._clip.duration, r = e._clip.duration, i = r / n, a = n / r;
			e.warp(1, i, t), this.warp(a, 1, t);
		}
		return this;
	}
	crossFadeTo(e, t, n = !1) {
		return e.crossFadeFrom(this, t, n);
	}
	stopFading() {
		let e = this._weightInterpolant;
		return e !== null && (this._weightInterpolant = null, this._mixer._takeBackControlInterpolant(e)), this;
	}
	setEffectiveTimeScale(e) {
		return this.timeScale = e, this._effectiveTimeScale = this.paused ? 0 : e, this.stopWarping();
	}
	getEffectiveTimeScale() {
		return this._effectiveTimeScale;
	}
	setDuration(e) {
		return this.timeScale = this._clip.duration / e, this.stopWarping();
	}
	syncWith(e) {
		return this.time = e.time, this.timeScale = e.timeScale, this.stopWarping();
	}
	halt(e) {
		return this.warp(this._effectiveTimeScale, 0, e);
	}
	warp(e, t, n) {
		let r = this._mixer, i = r.time, a = this.timeScale, o = this._timeScaleInterpolant;
		o === null && (o = r._lendControlInterpolant(), this._timeScaleInterpolant = o);
		let s = o.parameterPositions, c = o.sampleValues;
		return s[0] = i, s[1] = i + n, c[0] = e / a, c[1] = t / a, this;
	}
	stopWarping() {
		let e = this._timeScaleInterpolant;
		return e !== null && (this._timeScaleInterpolant = null, this._mixer._takeBackControlInterpolant(e)), this;
	}
	getMixer() {
		return this._mixer;
	}
	getClip() {
		return this._clip;
	}
	getRoot() {
		return this._localRoot || this._mixer._root;
	}
	_update(e, t, n, r) {
		if (!this.enabled) {
			this._updateWeight(e);
			return;
		}
		let i = this._startTime;
		if (i !== null) {
			let r = (e - i) * n;
			r < 0 || n === 0 ? t = 0 : (this._startTime = null, t = n * r);
		}
		t *= this._updateTimeScale(e);
		let a = this._updateTime(t), o = this._updateWeight(e);
		if (o > 0) {
			let e = this._interpolants, t = this._propertyBindings;
			switch (this.blendMode) {
				case nr:
					for (let n = 0, r = e.length; n !== r; ++n) e[n].evaluate(a), t[n].accumulateAdditive(o);
					break;
				case tr:
				default: for (let n = 0, i = e.length; n !== i; ++n) e[n].evaluate(a), t[n].accumulate(r, o);
			}
		}
	}
	_updateWeight(e) {
		let t = 0;
		if (this.enabled) {
			t = this.weight;
			let n = this._weightInterpolant;
			if (n !== null) {
				let r = n.evaluate(e)[0];
				t *= r, e > n.parameterPositions[1] && (this.stopFading(), r === 0 && (this.enabled = !1));
			}
		}
		return this._effectiveWeight = t, t;
	}
	_updateTimeScale(e) {
		let t = 0;
		if (!this.paused) {
			t = this.timeScale;
			let n = this._timeScaleInterpolant;
			if (n !== null) {
				let r = n.evaluate(e)[0];
				t *= r, e > n.parameterPositions[1] && (this.stopWarping(), t === 0 ? this.paused = !0 : this.timeScale = t);
			}
		}
		return this._effectiveTimeScale = t, t;
	}
	_updateTime(e) {
		let t = this._clip.duration, n = this.loop, r = this.time + e, i = this._loopCount, a = n === qn;
		if (e === 0) return i === -1 ? r : a && (i & 1) == 1 ? t - r : r;
		if (n === 2200) {
			i === -1 && (this._loopCount = 0, this._setEndings(!0, !0, !1));
			handle_stop: {
				if (r >= t) r = t;
				else if (r < 0) r = 0;
				else {
					this.time = r;
					break handle_stop;
				}
				this.clampWhenFinished ? this.paused = !0 : this.enabled = !1, this.time = r, this._mixer.dispatchEvent({
					type: "finished",
					action: this,
					direction: e < 0 ? -1 : 1
				});
			}
		} else {
			if (i === -1 && (e >= 0 ? (i = 0, this._setEndings(!0, this.repetitions === 0, a)) : this._setEndings(this.repetitions === 0, !0, a)), r >= t || r < 0) {
				let n = Math.floor(r / t);
				r -= t * n, i += Math.abs(n);
				let o = this.repetitions - i;
				if (o <= 0) this.clampWhenFinished ? this.paused = !0 : this.enabled = !1, r = e > 0 ? t : 0, this.time = r, this._mixer.dispatchEvent({
					type: "finished",
					action: this,
					direction: e > 0 ? 1 : -1
				});
				else {
					if (o === 1) {
						let t = e < 0;
						this._setEndings(t, !t, a);
					} else this._setEndings(!1, !1, a);
					this._loopCount = i, this.time = r, this._mixer.dispatchEvent({
						type: "loop",
						action: this,
						loopDelta: n
					});
				}
			} else this._loopCount = i, this.time = r;
			if (a && (i & 1) == 1) return t - r;
		}
		return r;
	}
	_setEndings(e, t, n) {
		let r = this._interpolantSettings;
		n ? (r.endingStart = $n, r.endingEnd = $n) : (e ? r.endingStart = this.zeroSlopeAtStart ? $n : Qn : r.endingStart = er, t ? r.endingEnd = this.zeroSlopeAtEnd ? $n : Qn : r.endingEnd = er);
	}
	_scheduleFading(e, t, n) {
		let r = this._mixer, i = r.time, a = this._weightInterpolant;
		a === null && (a = r._lendControlInterpolant(), this._weightInterpolant = a);
		let o = a.parameterPositions, s = a.sampleValues;
		return o[0] = i, s[0] = t, o[1] = i + e, s[1] = n, this;
	}
}, Wc = /* @__PURE__ */ new Float32Array(1), Gc = class extends Sr {
	constructor(e) {
		super(), this._root = e, this._initMemoryManager(), this._accuIndex = 0, this.time = 0, this.timeScale = 1, typeof __THREE_DEVTOOLS__ < "u" && __THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe", { detail: this }));
	}
	_bindAction(e, t) {
		let n = e._localRoot || this._root, r = e._clip.tracks, i = r.length, a = e._propertyBindings, o = e._interpolants, s = n.uuid, c = this._bindingsByRootAndName, l = c[s];
		l === void 0 && (l = {}, c[s] = l);
		for (let e = 0; e !== i; ++e) {
			let i = r[e], c = i.name, u = l[c];
			if (u !== void 0) ++u.referenceCount, a[e] = u;
			else {
				if (u = a[e], u !== void 0) {
					u._cacheIndex === null && (++u.referenceCount, this._addInactiveBinding(u, s, c));
					continue;
				}
				let r = t && t._propertyBindings[e].binding.parsedPath;
				u = new Ac(Hc.create(n, c, r), i.ValueTypeName, i.getValueSize()), ++u.referenceCount, this._addInactiveBinding(u, s, c), a[e] = u;
			}
			o[e].resultBuffer = u.buffer;
		}
	}
	_activateAction(e) {
		if (!this._isActiveAction(e)) {
			if (e._cacheIndex === null) {
				let t = (e._localRoot || this._root).uuid, n = e._clip.uuid, r = this._actionsByClip[n];
				this._bindAction(e, r && r.knownActions[0]), this._addInactiveAction(e, n, t);
			}
			let t = e._propertyBindings;
			for (let e = 0, n = t.length; e !== n; ++e) {
				let n = t[e];
				n.useCount++ === 0 && (this._lendBinding(n), n.saveOriginalState());
			}
			this._lendAction(e);
		}
	}
	_deactivateAction(e) {
		if (this._isActiveAction(e)) {
			let t = e._propertyBindings;
			for (let e = 0, n = t.length; e !== n; ++e) {
				let n = t[e];
				--n.useCount === 0 && (n.restoreOriginalState(), this._takeBackBinding(n));
			}
			this._takeBackAction(e);
		}
	}
	_initMemoryManager() {
		this._actions = [], this._nActiveActions = 0, this._actionsByClip = {}, this._bindings = [], this._nActiveBindings = 0, this._bindingsByRootAndName = {}, this._controlInterpolants = [], this._nActiveControlInterpolants = 0;
		let e = this;
		this.stats = {
			actions: {
				get total() {
					return e._actions.length;
				},
				get inUse() {
					return e._nActiveActions;
				}
			},
			bindings: {
				get total() {
					return e._bindings.length;
				},
				get inUse() {
					return e._nActiveBindings;
				}
			},
			controlInterpolants: {
				get total() {
					return e._controlInterpolants.length;
				},
				get inUse() {
					return e._nActiveControlInterpolants;
				}
			}
		};
	}
	_isActiveAction(e) {
		let t = e._cacheIndex;
		return t !== null && t < this._nActiveActions;
	}
	_addInactiveAction(e, t, n) {
		let r = this._actions, i = this._actionsByClip, a = i[t];
		if (a === void 0) a = {
			knownActions: [e],
			actionByRoot: {}
		}, e._byClipCacheIndex = 0, i[t] = a;
		else {
			let t = a.knownActions;
			e._byClipCacheIndex = t.length, t.push(e);
		}
		e._cacheIndex = r.length, r.push(e), a.actionByRoot[n] = e;
	}
	_removeInactiveAction(e) {
		let t = this._actions, n = t[t.length - 1], r = e._cacheIndex;
		n._cacheIndex = r, t[r] = n, t.pop(), e._cacheIndex = null;
		let i = e._clip.uuid, a = this._actionsByClip, o = a[i], s = o.knownActions, c = s[s.length - 1], l = e._byClipCacheIndex;
		c._byClipCacheIndex = l, s[l] = c, s.pop(), e._byClipCacheIndex = null;
		let u = o.actionByRoot, d = (e._localRoot || this._root).uuid;
		delete u[d], s.length === 0 && delete a[i], this._removeInactiveBindingsForAction(e);
	}
	_removeInactiveBindingsForAction(e) {
		let t = e._propertyBindings;
		for (let e = 0, n = t.length; e !== n; ++e) {
			let n = t[e];
			--n.referenceCount === 0 && this._removeInactiveBinding(n);
		}
	}
	_lendAction(e) {
		let t = this._actions, n = e._cacheIndex, r = this._nActiveActions++, i = t[r];
		e._cacheIndex = r, t[r] = e, i._cacheIndex = n, t[n] = i;
	}
	_takeBackAction(e) {
		let t = this._actions, n = e._cacheIndex, r = --this._nActiveActions, i = t[r];
		e._cacheIndex = r, t[r] = e, i._cacheIndex = n, t[n] = i;
	}
	_addInactiveBinding(e, t, n) {
		let r = this._bindingsByRootAndName, i = this._bindings, a = r[t];
		a === void 0 && (a = {}, r[t] = a), a[n] = e, e._cacheIndex = i.length, i.push(e);
	}
	_removeInactiveBinding(e) {
		let t = this._bindings, n = e.binding, r = n.rootNode.uuid, i = n.path, a = this._bindingsByRootAndName, o = a[r], s = t[t.length - 1], c = e._cacheIndex;
		s._cacheIndex = c, t[c] = s, t.pop(), delete o[i], Object.keys(o).length === 0 && delete a[r];
	}
	_lendBinding(e) {
		let t = this._bindings, n = e._cacheIndex, r = this._nActiveBindings++, i = t[r];
		e._cacheIndex = r, t[r] = e, i._cacheIndex = n, t[n] = i;
	}
	_takeBackBinding(e) {
		let t = this._bindings, n = e._cacheIndex, r = --this._nActiveBindings, i = t[r];
		e._cacheIndex = r, t[r] = e, i._cacheIndex = n, t[n] = i;
	}
	_lendControlInterpolant() {
		let e = this._controlInterpolants, t = this._nActiveControlInterpolants++, n = e[t];
		return n === void 0 && (n = new Hs(/* @__PURE__ */ new Float32Array(2), /* @__PURE__ */ new Float32Array(2), 1, Wc), n.__cacheIndex = t, e[t] = n), n;
	}
	_takeBackControlInterpolant(e) {
		let t = this._controlInterpolants, n = e.__cacheIndex, r = --this._nActiveControlInterpolants, i = t[r];
		e.__cacheIndex = r, t[r] = e, i.__cacheIndex = n, t[n] = i;
	}
	clipAction(e, t, n) {
		let r = t || this._root, i = r.uuid, a = typeof e == "string" ? $s.findByName(r, e) : e, o = a === null ? e : a.uuid, s = this._actionsByClip[o], c = null;
		if (n === void 0 && (n = a === null ? tr : a.blendMode), s !== void 0) {
			let e = s.actionByRoot[i];
			if (e !== void 0 && e.blendMode === n) return e;
			c = s.knownActions[0], a === null && (a = c._clip);
		}
		if (a === null) return null;
		let l = new Uc(this, a, t, n);
		return this._bindAction(l, c), this._addInactiveAction(l, o, i), l;
	}
	existingAction(e, t) {
		let n = t || this._root, r = n.uuid, i = typeof e == "string" ? $s.findByName(n, e) : e, a = i ? i.uuid : e, o = this._actionsByClip[a];
		return o === void 0 ? null : o.actionByRoot[r] || null;
	}
	stopAllAction() {
		let e = this._actions, t = this._nActiveActions;
		for (let n = t - 1; n >= 0; --n) e[n].stop();
		return this;
	}
	update(e) {
		e *= this.timeScale;
		let t = this._actions, n = this._nActiveActions, r = this.time += e, i = Math.sign(e), a = this._accuIndex ^= 1;
		for (let o = 0; o !== n; ++o) t[o]._update(r, e, i, a);
		let o = this._bindings, s = this._nActiveBindings;
		for (let e = 0; e !== s; ++e) o[e].apply(a);
		return this;
	}
	setTime(e) {
		this.time = 0;
		for (let e = 0; e < this._actions.length; e++) this._actions[e].time = 0;
		return this.update(e);
	}
	getRoot() {
		return this._root;
	}
	uncacheClip(e) {
		let t = this._actions, n = e.uuid, r = this._actionsByClip, i = r[n];
		if (i !== void 0) {
			let e = i.knownActions;
			for (let n = 0, r = e.length; n !== r; ++n) {
				let r = e[n];
				this._deactivateAction(r);
				let i = r._cacheIndex, a = t[t.length - 1];
				r._cacheIndex = null, r._byClipCacheIndex = null, a._cacheIndex = i, t[i] = a, t.pop(), this._removeInactiveBindingsForAction(r);
			}
			delete r[n];
		}
	}
	uncacheRoot(e) {
		let t = e.uuid, n = this._actionsByClip;
		for (let e in n) {
			let r = n[e].actionByRoot[t];
			r !== void 0 && (this._deactivateAction(r), this._removeInactiveAction(r));
		}
		let r = this._bindingsByRootAndName[t];
		if (r !== void 0) for (let e in r) {
			let t = r[e];
			t.restoreOriginalState(), this._removeInactiveBinding(t);
		}
	}
	uncacheAction(e, t) {
		let n = this.existingAction(e, t);
		n !== null && (this._deactivateAction(n), this._removeInactiveAction(n));
	}
}, Kc = /*@__PURE__*/ new vi(), qc = class {
	constructor(e, t, n = 0, r = Infinity) {
		this.ray = new io(e, t), this.near = n, this.far = r, this.camera = null, this.layers = new ki(), this.params = {
			Mesh: {},
			Line: { threshold: 1 },
			LOD: {},
			Points: { threshold: 1 },
			Sprite: {}
		};
	}
	set(e, t) {
		this.ray.set(e, t);
	}
	setFromCamera(e, t) {
		t.isPerspectiveCamera ? (this.ray.origin.setFromMatrixPosition(t.matrixWorld), this.ray.direction.set(e.x, e.y, .5).unproject(t).sub(this.ray.origin).normalize(), this.camera = t) : t.isOrthographicCamera ? (this.ray.origin.set(e.x, e.y, (t.near + t.far) / (t.near - t.far)).unproject(t), this.ray.direction.set(0, 0, -1).transformDirection(t.matrixWorld), this.camera = t) : G("Raycaster: Unsupported camera type: " + t.type);
	}
	setFromXRController(e) {
		return Kc.identity().extractRotation(e.matrixWorld), this.ray.origin.setFromMatrixPosition(e.matrixWorld), this.ray.direction.set(0, 0, -1).applyMatrix4(Kc), this;
	}
	intersectObject(e, t = !0, n = []) {
		return Yc(e, this, n, t), n.sort(Jc), n;
	}
	intersectObjects(e, t = !0, n = []) {
		for (let r = 0, i = e.length; r < i; r++) Yc(e[r], this, n, t);
		return n.sort(Jc), n;
	}
};
function Jc(e, t) {
	return e.distance - t.distance;
}
function Yc(e, t, n, r) {
	let i = !0;
	if (e.layers.test(t.layers) && e.raycast(t, n) === !1 && (i = !1), i === !0 && r === !0) {
		let r = e.children;
		for (let e = 0, i = r.length; e < i; e++) Yc(r[e], t, n, !0);
	}
}
(class e {
	static {
		e.prototype.isMatrix2 = !0;
	}
	constructor(e, t, n, r) {
		this.elements = [
			1,
			0,
			0,
			1
		], e !== void 0 && this.set(e, t, n, r);
	}
	identity() {
		return this.set(1, 0, 0, 1), this;
	}
	fromArray(e, t = 0) {
		for (let n = 0; n < 4; n++) this.elements[n] = e[n + t];
		return this;
	}
	set(e, t, n, r) {
		let i = this.elements;
		return i[0] = e, i[2] = t, i[1] = n, i[3] = r, this;
	}
});
function Xc(e, t, n, r) {
	let i = Zc(r);
	switch (n) {
		case Qt: return e * t;
		case rn: return e * t / i.components * i.byteLength;
		case an: return e * t / i.components * i.byteLength;
		case on: return e * t * 2 / i.components * i.byteLength;
		case sn: return e * t * 2 / i.components * i.byteLength;
		case $t: return e * t * 3 / i.components * i.byteLength;
		case en: return e * t * 4 / i.components * i.byteLength;
		case cn: return e * t * 4 / i.components * i.byteLength;
		case ln:
		case un: return Math.floor((e + 3) / 4) * Math.floor((t + 3) / 4) * 8;
		case dn:
		case fn: return Math.floor((e + 3) / 4) * Math.floor((t + 3) / 4) * 16;
		case mn:
		case gn: return Math.max(e, 16) * Math.max(t, 8) / 4;
		case pn:
		case hn: return Math.max(e, 8) * Math.max(t, 8) / 2;
		case _n:
		case vn:
		case bn:
		case xn: return Math.floor((e + 3) / 4) * Math.floor((t + 3) / 4) * 8;
		case yn:
		case Sn:
		case Cn: return Math.floor((e + 3) / 4) * Math.floor((t + 3) / 4) * 16;
		case wn: return Math.floor((e + 3) / 4) * Math.floor((t + 3) / 4) * 16;
		case Tn: return Math.floor((e + 4) / 5) * Math.floor((t + 3) / 4) * 16;
		case En: return Math.floor((e + 4) / 5) * Math.floor((t + 4) / 5) * 16;
		case Dn: return Math.floor((e + 5) / 6) * Math.floor((t + 4) / 5) * 16;
		case On: return Math.floor((e + 5) / 6) * Math.floor((t + 5) / 6) * 16;
		case kn: return Math.floor((e + 7) / 8) * Math.floor((t + 4) / 5) * 16;
		case An: return Math.floor((e + 7) / 8) * Math.floor((t + 5) / 6) * 16;
		case jn: return Math.floor((e + 7) / 8) * Math.floor((t + 7) / 8) * 16;
		case Mn: return Math.floor((e + 9) / 10) * Math.floor((t + 4) / 5) * 16;
		case Nn: return Math.floor((e + 9) / 10) * Math.floor((t + 5) / 6) * 16;
		case Pn: return Math.floor((e + 9) / 10) * Math.floor((t + 7) / 8) * 16;
		case Fn: return Math.floor((e + 9) / 10) * Math.floor((t + 9) / 10) * 16;
		case In: return Math.floor((e + 11) / 12) * Math.floor((t + 9) / 10) * 16;
		case Ln: return Math.floor((e + 11) / 12) * Math.floor((t + 11) / 12) * 16;
		case Rn:
		case zn:
		case Bn: return Math.ceil(e / 4) * Math.ceil(t / 4) * 16;
		case Vn:
		case Hn: return Math.ceil(e / 4) * Math.ceil(t / 4) * 8;
		case Un:
		case Wn: return Math.ceil(e / 4) * Math.ceil(t / 4) * 16;
	}
	throw Error(`Unable to determine texture byte length for ${n} format.`);
}
function Zc(e) {
	switch (e) {
		case zt:
		case Bt: return {
			byteLength: 1,
			components: 1
		};
		case Ht:
		case Vt:
		case Kt: return {
			byteLength: 2,
			components: 1
		};
		case qt:
		case Jt: return {
			byteLength: 2,
			components: 4
		};
		case Wt:
		case Ut:
		case Gt: return {
			byteLength: 4,
			components: 1
		};
		case Xt:
		case Zt: return {
			byteLength: 4,
			components: 3
		};
	}
	throw Error(`Unknown texture type ${e}.`);
}
typeof __THREE_DEVTOOLS__ < "u" && __THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("register", { detail: { revision: "184" } })), typeof window < "u" && (window.__THREE__ ? W("WARNING: Multiple instances of Three.js being imported.") : window.__THREE__ = "184");
//#endregion
//#region node_modules/.pnpm/three@0.184.0/node_modules/three/build/three.module.js
function Qc() {
	let e = null, t = !1, n = null, r = null;
	function i(t, a) {
		n(t, a), r = e.requestAnimationFrame(i);
	}
	return {
		start: function() {
			t !== !0 && n !== null && e !== null && (r = e.requestAnimationFrame(i), t = !0);
		},
		stop: function() {
			e !== null && e.cancelAnimationFrame(r), t = !1;
		},
		setAnimationLoop: function(e) {
			n = e;
		},
		setContext: function(t) {
			e = t;
		}
	};
}
function $c(e) {
	let t = /* @__PURE__ */ new WeakMap();
	function n(t, n) {
		let r = t.array, i = t.usage, a = r.byteLength, o = e.createBuffer();
		e.bindBuffer(n, o), e.bufferData(n, r, i), t.onUploadCallback();
		let s;
		if (r instanceof Float32Array) s = e.FLOAT;
		else if (typeof Float16Array < "u" && r instanceof Float16Array) s = e.HALF_FLOAT;
		else if (r instanceof Uint16Array) s = t.isFloat16BufferAttribute ? e.HALF_FLOAT : e.UNSIGNED_SHORT;
		else if (r instanceof Int16Array) s = e.SHORT;
		else if (r instanceof Uint32Array) s = e.UNSIGNED_INT;
		else if (r instanceof Int32Array) s = e.INT;
		else if (r instanceof Int8Array) s = e.BYTE;
		else if (r instanceof Uint8Array) s = e.UNSIGNED_BYTE;
		else if (r instanceof Uint8ClampedArray) s = e.UNSIGNED_BYTE;
		else throw Error("THREE.WebGLAttributes: Unsupported buffer data format: " + r);
		return {
			buffer: o,
			type: s,
			bytesPerElement: r.BYTES_PER_ELEMENT,
			version: t.version,
			size: a
		};
	}
	function r(t, n, r) {
		let i = n.array, a = n.updateRanges;
		if (e.bindBuffer(r, t), a.length === 0) e.bufferSubData(r, 0, i);
		else {
			a.sort((e, t) => e.start - t.start);
			let t = 0;
			for (let e = 1; e < a.length; e++) {
				let n = a[t], r = a[e];
				r.start <= n.start + n.count + 1 ? n.count = Math.max(n.count, r.start + r.count - n.start) : (++t, a[t] = r);
			}
			a.length = t + 1;
			for (let t = 0, n = a.length; t < n; t++) {
				let n = a[t];
				e.bufferSubData(r, n.start * i.BYTES_PER_ELEMENT, i, n.start, n.count);
			}
			n.clearUpdateRanges();
		}
		n.onUploadCallback();
	}
	function i(e) {
		return e.isInterleavedBufferAttribute && (e = e.data), t.get(e);
	}
	function a(n) {
		n.isInterleavedBufferAttribute && (n = n.data);
		let r = t.get(n);
		r && (e.deleteBuffer(r.buffer), t.delete(n));
	}
	function o(e, i) {
		if (e.isInterleavedBufferAttribute && (e = e.data), e.isGLBufferAttribute) {
			let n = t.get(e);
			(!n || n.version < e.version) && t.set(e, {
				buffer: e.buffer,
				type: e.type,
				bytesPerElement: e.elementSize,
				version: e.version
			});
			return;
		}
		let a = t.get(e);
		if (a === void 0) t.set(e, n(e, i));
		else if (a.version < e.version) {
			if (a.size !== e.array.byteLength) throw Error("THREE.WebGLAttributes: The size of the buffer attribute's array buffer does not match the original size. Resizing buffer attributes is not supported.");
			r(a.buffer, e, i), a.version = e.version;
		}
	}
	return {
		get: i,
		remove: a,
		update: o
	};
}
var Z = {
	alphahash_fragment: "#ifdef USE_ALPHAHASH\n	if ( diffuseColor.a < getAlphaHashThreshold( vPosition ) ) discard;\n#endif",
	alphahash_pars_fragment: "#ifdef USE_ALPHAHASH\n	const float ALPHA_HASH_SCALE = 0.05;\n	float hash2D( vec2 value ) {\n		return fract( 1.0e4 * sin( 17.0 * value.x + 0.1 * value.y ) * ( 0.1 + abs( sin( 13.0 * value.y + value.x ) ) ) );\n	}\n	float hash3D( vec3 value ) {\n		return hash2D( vec2( hash2D( value.xy ), value.z ) );\n	}\n	float getAlphaHashThreshold( vec3 position ) {\n		float maxDeriv = max(\n			length( dFdx( position.xyz ) ),\n			length( dFdy( position.xyz ) )\n		);\n		float pixScale = 1.0 / ( ALPHA_HASH_SCALE * maxDeriv );\n		vec2 pixScales = vec2(\n			exp2( floor( log2( pixScale ) ) ),\n			exp2( ceil( log2( pixScale ) ) )\n		);\n		vec2 alpha = vec2(\n			hash3D( floor( pixScales.x * position.xyz ) ),\n			hash3D( floor( pixScales.y * position.xyz ) )\n		);\n		float lerpFactor = fract( log2( pixScale ) );\n		float x = ( 1.0 - lerpFactor ) * alpha.x + lerpFactor * alpha.y;\n		float a = min( lerpFactor, 1.0 - lerpFactor );\n		vec3 cases = vec3(\n			x * x / ( 2.0 * a * ( 1.0 - a ) ),\n			( x - 0.5 * a ) / ( 1.0 - a ),\n			1.0 - ( ( 1.0 - x ) * ( 1.0 - x ) / ( 2.0 * a * ( 1.0 - a ) ) )\n		);\n		float threshold = ( x < ( 1.0 - a ) )\n			? ( ( x < a ) ? cases.x : cases.y )\n			: cases.z;\n		return clamp( threshold , 1.0e-6, 1.0 );\n	}\n#endif",
	alphamap_fragment: "#ifdef USE_ALPHAMAP\n	diffuseColor.a *= texture2D( alphaMap, vAlphaMapUv ).g;\n#endif",
	alphamap_pars_fragment: "#ifdef USE_ALPHAMAP\n	uniform sampler2D alphaMap;\n#endif",
	alphatest_fragment: "#ifdef USE_ALPHATEST\n	#ifdef ALPHA_TO_COVERAGE\n	diffuseColor.a = smoothstep( alphaTest, alphaTest + fwidth( diffuseColor.a ), diffuseColor.a );\n	if ( diffuseColor.a == 0.0 ) discard;\n	#else\n	if ( diffuseColor.a < alphaTest ) discard;\n	#endif\n#endif",
	alphatest_pars_fragment: "#ifdef USE_ALPHATEST\n	uniform float alphaTest;\n#endif",
	aomap_fragment: "#ifdef USE_AOMAP\n	float ambientOcclusion = ( texture2D( aoMap, vAoMapUv ).r - 1.0 ) * aoMapIntensity + 1.0;\n	reflectedLight.indirectDiffuse *= ambientOcclusion;\n	#if defined( USE_CLEARCOAT ) \n		clearcoatSpecularIndirect *= ambientOcclusion;\n	#endif\n	#if defined( USE_SHEEN ) \n		sheenSpecularIndirect *= ambientOcclusion;\n	#endif\n	#if defined( USE_ENVMAP ) && defined( STANDARD )\n		float dotNV = saturate( dot( geometryNormal, geometryViewDir ) );\n		reflectedLight.indirectSpecular *= computeSpecularOcclusion( dotNV, ambientOcclusion, material.roughness );\n	#endif\n#endif",
	aomap_pars_fragment: "#ifdef USE_AOMAP\n	uniform sampler2D aoMap;\n	uniform float aoMapIntensity;\n#endif",
	batching_pars_vertex: "#ifdef USE_BATCHING\n	#if ! defined( GL_ANGLE_multi_draw )\n	#define gl_DrawID _gl_DrawID\n	uniform int _gl_DrawID;\n	#endif\n	uniform highp sampler2D batchingTexture;\n	uniform highp usampler2D batchingIdTexture;\n	mat4 getBatchingMatrix( const in float i ) {\n		int size = textureSize( batchingTexture, 0 ).x;\n		int j = int( i ) * 4;\n		int x = j % size;\n		int y = j / size;\n		vec4 v1 = texelFetch( batchingTexture, ivec2( x, y ), 0 );\n		vec4 v2 = texelFetch( batchingTexture, ivec2( x + 1, y ), 0 );\n		vec4 v3 = texelFetch( batchingTexture, ivec2( x + 2, y ), 0 );\n		vec4 v4 = texelFetch( batchingTexture, ivec2( x + 3, y ), 0 );\n		return mat4( v1, v2, v3, v4 );\n	}\n	float getIndirectIndex( const in int i ) {\n		int size = textureSize( batchingIdTexture, 0 ).x;\n		int x = i % size;\n		int y = i / size;\n		return float( texelFetch( batchingIdTexture, ivec2( x, y ), 0 ).r );\n	}\n#endif\n#ifdef USE_BATCHING_COLOR\n	uniform sampler2D batchingColorTexture;\n	vec4 getBatchingColor( const in float i ) {\n		int size = textureSize( batchingColorTexture, 0 ).x;\n		int j = int( i );\n		int x = j % size;\n		int y = j / size;\n		return texelFetch( batchingColorTexture, ivec2( x, y ), 0 );\n	}\n#endif",
	batching_vertex: "#ifdef USE_BATCHING\n	mat4 batchingMatrix = getBatchingMatrix( getIndirectIndex( gl_DrawID ) );\n#endif",
	begin_vertex: "vec3 transformed = vec3( position );\n#ifdef USE_ALPHAHASH\n	vPosition = vec3( position );\n#endif",
	beginnormal_vertex: "vec3 objectNormal = vec3( normal );\n#ifdef USE_TANGENT\n	vec3 objectTangent = vec3( tangent.xyz );\n#endif",
	bsdfs: "float G_BlinnPhong_Implicit( ) {\n	return 0.25;\n}\nfloat D_BlinnPhong( const in float shininess, const in float dotNH ) {\n	return RECIPROCAL_PI * ( shininess * 0.5 + 1.0 ) * pow( dotNH, shininess );\n}\nvec3 BRDF_BlinnPhong( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in vec3 specularColor, const in float shininess ) {\n	vec3 halfDir = normalize( lightDir + viewDir );\n	float dotNH = saturate( dot( normal, halfDir ) );\n	float dotVH = saturate( dot( viewDir, halfDir ) );\n	vec3 F = F_Schlick( specularColor, 1.0, dotVH );\n	float G = G_BlinnPhong_Implicit( );\n	float D = D_BlinnPhong( shininess, dotNH );\n	return F * ( G * D );\n} // validated",
	iridescence_fragment: "#ifdef USE_IRIDESCENCE\n	const mat3 XYZ_TO_REC709 = mat3(\n		 3.2404542, -0.9692660,  0.0556434,\n		-1.5371385,  1.8760108, -0.2040259,\n		-0.4985314,  0.0415560,  1.0572252\n	);\n	vec3 Fresnel0ToIor( vec3 fresnel0 ) {\n		vec3 sqrtF0 = sqrt( fresnel0 );\n		return ( vec3( 1.0 ) + sqrtF0 ) / ( vec3( 1.0 ) - sqrtF0 );\n	}\n	vec3 IorToFresnel0( vec3 transmittedIor, float incidentIor ) {\n		return pow2( ( transmittedIor - vec3( incidentIor ) ) / ( transmittedIor + vec3( incidentIor ) ) );\n	}\n	float IorToFresnel0( float transmittedIor, float incidentIor ) {\n		return pow2( ( transmittedIor - incidentIor ) / ( transmittedIor + incidentIor ));\n	}\n	vec3 evalSensitivity( float OPD, vec3 shift ) {\n		float phase = 2.0 * PI * OPD * 1.0e-9;\n		vec3 val = vec3( 5.4856e-13, 4.4201e-13, 5.2481e-13 );\n		vec3 pos = vec3( 1.6810e+06, 1.7953e+06, 2.2084e+06 );\n		vec3 var = vec3( 4.3278e+09, 9.3046e+09, 6.6121e+09 );\n		vec3 xyz = val * sqrt( 2.0 * PI * var ) * cos( pos * phase + shift ) * exp( - pow2( phase ) * var );\n		xyz.x += 9.7470e-14 * sqrt( 2.0 * PI * 4.5282e+09 ) * cos( 2.2399e+06 * phase + shift[ 0 ] ) * exp( - 4.5282e+09 * pow2( phase ) );\n		xyz /= 1.0685e-7;\n		vec3 rgb = XYZ_TO_REC709 * xyz;\n		return rgb;\n	}\n	vec3 evalIridescence( float outsideIOR, float eta2, float cosTheta1, float thinFilmThickness, vec3 baseF0 ) {\n		vec3 I;\n		float iridescenceIOR = mix( outsideIOR, eta2, smoothstep( 0.0, 0.03, thinFilmThickness ) );\n		float sinTheta2Sq = pow2( outsideIOR / iridescenceIOR ) * ( 1.0 - pow2( cosTheta1 ) );\n		float cosTheta2Sq = 1.0 - sinTheta2Sq;\n		if ( cosTheta2Sq < 0.0 ) {\n			return vec3( 1.0 );\n		}\n		float cosTheta2 = sqrt( cosTheta2Sq );\n		float R0 = IorToFresnel0( iridescenceIOR, outsideIOR );\n		float R12 = F_Schlick( R0, 1.0, cosTheta1 );\n		float T121 = 1.0 - R12;\n		float phi12 = 0.0;\n		if ( iridescenceIOR < outsideIOR ) phi12 = PI;\n		float phi21 = PI - phi12;\n		vec3 baseIOR = Fresnel0ToIor( clamp( baseF0, 0.0, 0.9999 ) );		vec3 R1 = IorToFresnel0( baseIOR, iridescenceIOR );\n		vec3 R23 = F_Schlick( R1, 1.0, cosTheta2 );\n		vec3 phi23 = vec3( 0.0 );\n		if ( baseIOR[ 0 ] < iridescenceIOR ) phi23[ 0 ] = PI;\n		if ( baseIOR[ 1 ] < iridescenceIOR ) phi23[ 1 ] = PI;\n		if ( baseIOR[ 2 ] < iridescenceIOR ) phi23[ 2 ] = PI;\n		float OPD = 2.0 * iridescenceIOR * thinFilmThickness * cosTheta2;\n		vec3 phi = vec3( phi21 ) + phi23;\n		vec3 R123 = clamp( R12 * R23, 1e-5, 0.9999 );\n		vec3 r123 = sqrt( R123 );\n		vec3 Rs = pow2( T121 ) * R23 / ( vec3( 1.0 ) - R123 );\n		vec3 C0 = R12 + Rs;\n		I = C0;\n		vec3 Cm = Rs - T121;\n		for ( int m = 1; m <= 2; ++ m ) {\n			Cm *= r123;\n			vec3 Sm = 2.0 * evalSensitivity( float( m ) * OPD, float( m ) * phi );\n			I += Cm * Sm;\n		}\n		return max( I, vec3( 0.0 ) );\n	}\n#endif",
	bumpmap_pars_fragment: "#ifdef USE_BUMPMAP\n	uniform sampler2D bumpMap;\n	uniform float bumpScale;\n	vec2 dHdxy_fwd() {\n		vec2 dSTdx = dFdx( vBumpMapUv );\n		vec2 dSTdy = dFdy( vBumpMapUv );\n		float Hll = bumpScale * texture2D( bumpMap, vBumpMapUv ).x;\n		float dBx = bumpScale * texture2D( bumpMap, vBumpMapUv + dSTdx ).x - Hll;\n		float dBy = bumpScale * texture2D( bumpMap, vBumpMapUv + dSTdy ).x - Hll;\n		return vec2( dBx, dBy );\n	}\n	vec3 perturbNormalArb( vec3 surf_pos, vec3 surf_norm, vec2 dHdxy, float faceDirection ) {\n		vec3 vSigmaX = normalize( dFdx( surf_pos.xyz ) );\n		vec3 vSigmaY = normalize( dFdy( surf_pos.xyz ) );\n		vec3 vN = surf_norm;\n		vec3 R1 = cross( vSigmaY, vN );\n		vec3 R2 = cross( vN, vSigmaX );\n		float fDet = dot( vSigmaX, R1 ) * faceDirection;\n		vec3 vGrad = sign( fDet ) * ( dHdxy.x * R1 + dHdxy.y * R2 );\n		return normalize( abs( fDet ) * surf_norm - vGrad );\n	}\n#endif",
	clipping_planes_fragment: "#if NUM_CLIPPING_PLANES > 0\n	vec4 plane;\n	#ifdef ALPHA_TO_COVERAGE\n		float distanceToPlane, distanceGradient;\n		float clipOpacity = 1.0;\n		#pragma unroll_loop_start\n		for ( int i = 0; i < UNION_CLIPPING_PLANES; i ++ ) {\n			plane = clippingPlanes[ i ];\n			distanceToPlane = - dot( vClipPosition, plane.xyz ) + plane.w;\n			distanceGradient = fwidth( distanceToPlane ) / 2.0;\n			clipOpacity *= smoothstep( - distanceGradient, distanceGradient, distanceToPlane );\n			if ( clipOpacity == 0.0 ) discard;\n		}\n		#pragma unroll_loop_end\n		#if UNION_CLIPPING_PLANES < NUM_CLIPPING_PLANES\n			float unionClipOpacity = 1.0;\n			#pragma unroll_loop_start\n			for ( int i = UNION_CLIPPING_PLANES; i < NUM_CLIPPING_PLANES; i ++ ) {\n				plane = clippingPlanes[ i ];\n				distanceToPlane = - dot( vClipPosition, plane.xyz ) + plane.w;\n				distanceGradient = fwidth( distanceToPlane ) / 2.0;\n				unionClipOpacity *= 1.0 - smoothstep( - distanceGradient, distanceGradient, distanceToPlane );\n			}\n			#pragma unroll_loop_end\n			clipOpacity *= 1.0 - unionClipOpacity;\n		#endif\n		diffuseColor.a *= clipOpacity;\n		if ( diffuseColor.a == 0.0 ) discard;\n	#else\n		#pragma unroll_loop_start\n		for ( int i = 0; i < UNION_CLIPPING_PLANES; i ++ ) {\n			plane = clippingPlanes[ i ];\n			if ( dot( vClipPosition, plane.xyz ) > plane.w ) discard;\n		}\n		#pragma unroll_loop_end\n		#if UNION_CLIPPING_PLANES < NUM_CLIPPING_PLANES\n			bool clipped = true;\n			#pragma unroll_loop_start\n			for ( int i = UNION_CLIPPING_PLANES; i < NUM_CLIPPING_PLANES; i ++ ) {\n				plane = clippingPlanes[ i ];\n				clipped = ( dot( vClipPosition, plane.xyz ) > plane.w ) && clipped;\n			}\n			#pragma unroll_loop_end\n			if ( clipped ) discard;\n		#endif\n	#endif\n#endif",
	clipping_planes_pars_fragment: "#if NUM_CLIPPING_PLANES > 0\n	varying vec3 vClipPosition;\n	uniform vec4 clippingPlanes[ NUM_CLIPPING_PLANES ];\n#endif",
	clipping_planes_pars_vertex: "#if NUM_CLIPPING_PLANES > 0\n	varying vec3 vClipPosition;\n#endif",
	clipping_planes_vertex: "#if NUM_CLIPPING_PLANES > 0\n	vClipPosition = - mvPosition.xyz;\n#endif",
	color_fragment: "#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )\n	diffuseColor *= vColor;\n#endif",
	color_pars_fragment: "#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )\n	varying vec4 vColor;\n#endif",
	color_pars_vertex: "#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )\n	varying vec4 vColor;\n#endif",
	color_vertex: "#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )\n	vColor = vec4( 1.0 );\n#endif\n#ifdef USE_COLOR_ALPHA\n	vColor *= color;\n#elif defined( USE_COLOR )\n	vColor.rgb *= color;\n#endif\n#ifdef USE_INSTANCING_COLOR\n	vColor.rgb *= instanceColor.rgb;\n#endif\n#ifdef USE_BATCHING_COLOR\n	vColor *= getBatchingColor( getIndirectIndex( gl_DrawID ) );\n#endif",
	common: "#define PI 3.141592653589793\n#define PI2 6.283185307179586\n#define PI_HALF 1.5707963267948966\n#define RECIPROCAL_PI 0.3183098861837907\n#define RECIPROCAL_PI2 0.15915494309189535\n#define EPSILON 1e-6\n#ifndef saturate\n#define saturate( a ) clamp( a, 0.0, 1.0 )\n#endif\n#define whiteComplement( a ) ( 1.0 - saturate( a ) )\nfloat pow2( const in float x ) { return x*x; }\nvec3 pow2( const in vec3 x ) { return x*x; }\nfloat pow3( const in float x ) { return x*x*x; }\nfloat pow4( const in float x ) { float x2 = x*x; return x2*x2; }\nfloat max3( const in vec3 v ) { return max( max( v.x, v.y ), v.z ); }\nfloat average( const in vec3 v ) { return dot( v, vec3( 0.3333333 ) ); }\nhighp float rand( const in vec2 uv ) {\n	const highp float a = 12.9898, b = 78.233, c = 43758.5453;\n	highp float dt = dot( uv.xy, vec2( a,b ) ), sn = mod( dt, PI );\n	return fract( sin( sn ) * c );\n}\n#ifdef HIGH_PRECISION\n	float precisionSafeLength( vec3 v ) { return length( v ); }\n#else\n	float precisionSafeLength( vec3 v ) {\n		float maxComponent = max3( abs( v ) );\n		return length( v / maxComponent ) * maxComponent;\n	}\n#endif\nstruct IncidentLight {\n	vec3 color;\n	vec3 direction;\n	bool visible;\n};\nstruct ReflectedLight {\n	vec3 directDiffuse;\n	vec3 directSpecular;\n	vec3 indirectDiffuse;\n	vec3 indirectSpecular;\n};\n#ifdef USE_ALPHAHASH\n	varying vec3 vPosition;\n#endif\nvec3 transformDirection( in vec3 dir, in mat4 matrix ) {\n	return normalize( ( matrix * vec4( dir, 0.0 ) ).xyz );\n}\nvec3 inverseTransformDirection( in vec3 dir, in mat4 matrix ) {\n	return normalize( ( vec4( dir, 0.0 ) * matrix ).xyz );\n}\nbool isPerspectiveMatrix( mat4 m ) {\n	return m[ 2 ][ 3 ] == - 1.0;\n}\nvec2 equirectUv( in vec3 dir ) {\n	float u = atan( dir.z, dir.x ) * RECIPROCAL_PI2 + 0.5;\n	float v = asin( clamp( dir.y, - 1.0, 1.0 ) ) * RECIPROCAL_PI + 0.5;\n	return vec2( u, v );\n}\nvec3 BRDF_Lambert( const in vec3 diffuseColor ) {\n	return RECIPROCAL_PI * diffuseColor;\n}\nvec3 F_Schlick( const in vec3 f0, const in float f90, const in float dotVH ) {\n	float fresnel = exp2( ( - 5.55473 * dotVH - 6.98316 ) * dotVH );\n	return f0 * ( 1.0 - fresnel ) + ( f90 * fresnel );\n}\nfloat F_Schlick( const in float f0, const in float f90, const in float dotVH ) {\n	float fresnel = exp2( ( - 5.55473 * dotVH - 6.98316 ) * dotVH );\n	return f0 * ( 1.0 - fresnel ) + ( f90 * fresnel );\n} // validated",
	cube_uv_reflection_fragment: "#ifdef ENVMAP_TYPE_CUBE_UV\n	#define cubeUV_minMipLevel 4.0\n	#define cubeUV_minTileSize 16.0\n	float getFace( vec3 direction ) {\n		vec3 absDirection = abs( direction );\n		float face = - 1.0;\n		if ( absDirection.x > absDirection.z ) {\n			if ( absDirection.x > absDirection.y )\n				face = direction.x > 0.0 ? 0.0 : 3.0;\n			else\n				face = direction.y > 0.0 ? 1.0 : 4.0;\n		} else {\n			if ( absDirection.z > absDirection.y )\n				face = direction.z > 0.0 ? 2.0 : 5.0;\n			else\n				face = direction.y > 0.0 ? 1.0 : 4.0;\n		}\n		return face;\n	}\n	vec2 getUV( vec3 direction, float face ) {\n		vec2 uv;\n		if ( face == 0.0 ) {\n			uv = vec2( direction.z, direction.y ) / abs( direction.x );\n		} else if ( face == 1.0 ) {\n			uv = vec2( - direction.x, - direction.z ) / abs( direction.y );\n		} else if ( face == 2.0 ) {\n			uv = vec2( - direction.x, direction.y ) / abs( direction.z );\n		} else if ( face == 3.0 ) {\n			uv = vec2( - direction.z, direction.y ) / abs( direction.x );\n		} else if ( face == 4.0 ) {\n			uv = vec2( - direction.x, direction.z ) / abs( direction.y );\n		} else {\n			uv = vec2( direction.x, direction.y ) / abs( direction.z );\n		}\n		return 0.5 * ( uv + 1.0 );\n	}\n	vec3 bilinearCubeUV( sampler2D envMap, vec3 direction, float mipInt ) {\n		float face = getFace( direction );\n		float filterInt = max( cubeUV_minMipLevel - mipInt, 0.0 );\n		mipInt = max( mipInt, cubeUV_minMipLevel );\n		float faceSize = exp2( mipInt );\n		highp vec2 uv = getUV( direction, face ) * ( faceSize - 2.0 ) + 1.0;\n		if ( face > 2.0 ) {\n			uv.y += faceSize;\n			face -= 3.0;\n		}\n		uv.x += face * faceSize;\n		uv.x += filterInt * 3.0 * cubeUV_minTileSize;\n		uv.y += 4.0 * ( exp2( CUBEUV_MAX_MIP ) - faceSize );\n		uv.x *= CUBEUV_TEXEL_WIDTH;\n		uv.y *= CUBEUV_TEXEL_HEIGHT;\n		#ifdef texture2DGradEXT\n			return texture2DGradEXT( envMap, uv, vec2( 0.0 ), vec2( 0.0 ) ).rgb;\n		#else\n			return texture2D( envMap, uv ).rgb;\n		#endif\n	}\n	#define cubeUV_r0 1.0\n	#define cubeUV_m0 - 2.0\n	#define cubeUV_r1 0.8\n	#define cubeUV_m1 - 1.0\n	#define cubeUV_r4 0.4\n	#define cubeUV_m4 2.0\n	#define cubeUV_r5 0.305\n	#define cubeUV_m5 3.0\n	#define cubeUV_r6 0.21\n	#define cubeUV_m6 4.0\n	float roughnessToMip( float roughness ) {\n		float mip = 0.0;\n		if ( roughness >= cubeUV_r1 ) {\n			mip = ( cubeUV_r0 - roughness ) * ( cubeUV_m1 - cubeUV_m0 ) / ( cubeUV_r0 - cubeUV_r1 ) + cubeUV_m0;\n		} else if ( roughness >= cubeUV_r4 ) {\n			mip = ( cubeUV_r1 - roughness ) * ( cubeUV_m4 - cubeUV_m1 ) / ( cubeUV_r1 - cubeUV_r4 ) + cubeUV_m1;\n		} else if ( roughness >= cubeUV_r5 ) {\n			mip = ( cubeUV_r4 - roughness ) * ( cubeUV_m5 - cubeUV_m4 ) / ( cubeUV_r4 - cubeUV_r5 ) + cubeUV_m4;\n		} else if ( roughness >= cubeUV_r6 ) {\n			mip = ( cubeUV_r5 - roughness ) * ( cubeUV_m6 - cubeUV_m5 ) / ( cubeUV_r5 - cubeUV_r6 ) + cubeUV_m5;\n		} else {\n			mip = - 2.0 * log2( 1.16 * roughness );		}\n		return mip;\n	}\n	vec4 textureCubeUV( sampler2D envMap, vec3 sampleDir, float roughness ) {\n		float mip = clamp( roughnessToMip( roughness ), cubeUV_m0, CUBEUV_MAX_MIP );\n		float mipF = fract( mip );\n		float mipInt = floor( mip );\n		vec3 color0 = bilinearCubeUV( envMap, sampleDir, mipInt );\n		if ( mipF == 0.0 ) {\n			return vec4( color0, 1.0 );\n		} else {\n			vec3 color1 = bilinearCubeUV( envMap, sampleDir, mipInt + 1.0 );\n			return vec4( mix( color0, color1, mipF ), 1.0 );\n		}\n	}\n#endif",
	defaultnormal_vertex: "vec3 transformedNormal = objectNormal;\n#ifdef USE_TANGENT\n	vec3 transformedTangent = objectTangent;\n#endif\n#ifdef USE_BATCHING\n	mat3 bm = mat3( batchingMatrix );\n	transformedNormal /= vec3( dot( bm[ 0 ], bm[ 0 ] ), dot( bm[ 1 ], bm[ 1 ] ), dot( bm[ 2 ], bm[ 2 ] ) );\n	transformedNormal = bm * transformedNormal;\n	#ifdef USE_TANGENT\n		transformedTangent = bm * transformedTangent;\n	#endif\n#endif\n#ifdef USE_INSTANCING\n	mat3 im = mat3( instanceMatrix );\n	transformedNormal /= vec3( dot( im[ 0 ], im[ 0 ] ), dot( im[ 1 ], im[ 1 ] ), dot( im[ 2 ], im[ 2 ] ) );\n	transformedNormal = im * transformedNormal;\n	#ifdef USE_TANGENT\n		transformedTangent = im * transformedTangent;\n	#endif\n#endif\ntransformedNormal = normalMatrix * transformedNormal;\n#ifdef FLIP_SIDED\n	transformedNormal = - transformedNormal;\n#endif\n#ifdef USE_TANGENT\n	transformedTangent = ( modelViewMatrix * vec4( transformedTangent, 0.0 ) ).xyz;\n	#ifdef FLIP_SIDED\n		transformedTangent = - transformedTangent;\n	#endif\n#endif",
	displacementmap_pars_vertex: "#ifdef USE_DISPLACEMENTMAP\n	uniform sampler2D displacementMap;\n	uniform float displacementScale;\n	uniform float displacementBias;\n#endif",
	displacementmap_vertex: "#ifdef USE_DISPLACEMENTMAP\n	transformed += normalize( objectNormal ) * ( texture2D( displacementMap, vDisplacementMapUv ).x * displacementScale + displacementBias );\n#endif",
	emissivemap_fragment: "#ifdef USE_EMISSIVEMAP\n	vec4 emissiveColor = texture2D( emissiveMap, vEmissiveMapUv );\n	#ifdef DECODE_VIDEO_TEXTURE_EMISSIVE\n		emissiveColor = sRGBTransferEOTF( emissiveColor );\n	#endif\n	totalEmissiveRadiance *= emissiveColor.rgb;\n#endif",
	emissivemap_pars_fragment: "#ifdef USE_EMISSIVEMAP\n	uniform sampler2D emissiveMap;\n#endif",
	colorspace_fragment: "gl_FragColor = linearToOutputTexel( gl_FragColor );",
	colorspace_pars_fragment: "vec4 LinearTransferOETF( in vec4 value ) {\n	return value;\n}\nvec4 sRGBTransferEOTF( in vec4 value ) {\n	return vec4( mix( pow( value.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), value.rgb * 0.0773993808, vec3( lessThanEqual( value.rgb, vec3( 0.04045 ) ) ) ), value.a );\n}\nvec4 sRGBTransferOETF( in vec4 value ) {\n	return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );\n}",
	envmap_fragment: "#ifdef USE_ENVMAP\n	#ifdef ENV_WORLDPOS\n		vec3 cameraToFrag;\n		if ( isOrthographic ) {\n			cameraToFrag = normalize( vec3( - viewMatrix[ 0 ][ 2 ], - viewMatrix[ 1 ][ 2 ], - viewMatrix[ 2 ][ 2 ] ) );\n		} else {\n			cameraToFrag = normalize( vWorldPosition - cameraPosition );\n		}\n		vec3 worldNormal = inverseTransformDirection( normal, viewMatrix );\n		#ifdef ENVMAP_MODE_REFLECTION\n			vec3 reflectVec = reflect( cameraToFrag, worldNormal );\n		#else\n			vec3 reflectVec = refract( cameraToFrag, worldNormal, refractionRatio );\n		#endif\n	#else\n		vec3 reflectVec = vReflect;\n	#endif\n	#ifdef ENVMAP_TYPE_CUBE\n		vec4 envColor = textureCube( envMap, envMapRotation * reflectVec );\n		#ifdef ENVMAP_BLENDING_MULTIPLY\n			outgoingLight = mix( outgoingLight, outgoingLight * envColor.xyz, specularStrength * reflectivity );\n		#elif defined( ENVMAP_BLENDING_MIX )\n			outgoingLight = mix( outgoingLight, envColor.xyz, specularStrength * reflectivity );\n		#elif defined( ENVMAP_BLENDING_ADD )\n			outgoingLight += envColor.xyz * specularStrength * reflectivity;\n		#endif\n	#endif\n#endif",
	envmap_common_pars_fragment: "#ifdef USE_ENVMAP\n	uniform float envMapIntensity;\n	uniform mat3 envMapRotation;\n	#ifdef ENVMAP_TYPE_CUBE\n		uniform samplerCube envMap;\n	#else\n		uniform sampler2D envMap;\n	#endif\n#endif",
	envmap_pars_fragment: "#ifdef USE_ENVMAP\n	uniform float reflectivity;\n	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )\n		#define ENV_WORLDPOS\n	#endif\n	#ifdef ENV_WORLDPOS\n		varying vec3 vWorldPosition;\n		uniform float refractionRatio;\n	#else\n		varying vec3 vReflect;\n	#endif\n#endif",
	envmap_pars_vertex: "#ifdef USE_ENVMAP\n	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )\n		#define ENV_WORLDPOS\n	#endif\n	#ifdef ENV_WORLDPOS\n		\n		varying vec3 vWorldPosition;\n	#else\n		varying vec3 vReflect;\n		uniform float refractionRatio;\n	#endif\n#endif",
	envmap_physical_pars_fragment: "#ifdef USE_ENVMAP\n	vec3 getIBLIrradiance( const in vec3 normal ) {\n		#ifdef ENVMAP_TYPE_CUBE_UV\n			vec3 worldNormal = inverseTransformDirection( normal, viewMatrix );\n			vec4 envMapColor = textureCubeUV( envMap, envMapRotation * worldNormal, 1.0 );\n			return PI * envMapColor.rgb * envMapIntensity;\n		#else\n			return vec3( 0.0 );\n		#endif\n	}\n	vec3 getIBLRadiance( const in vec3 viewDir, const in vec3 normal, const in float roughness ) {\n		#ifdef ENVMAP_TYPE_CUBE_UV\n			vec3 reflectVec = reflect( - viewDir, normal );\n			reflectVec = normalize( mix( reflectVec, normal, pow4( roughness ) ) );\n			reflectVec = inverseTransformDirection( reflectVec, viewMatrix );\n			vec4 envMapColor = textureCubeUV( envMap, envMapRotation * reflectVec, roughness );\n			return envMapColor.rgb * envMapIntensity;\n		#else\n			return vec3( 0.0 );\n		#endif\n	}\n	#ifdef USE_ANISOTROPY\n		vec3 getIBLAnisotropyRadiance( const in vec3 viewDir, const in vec3 normal, const in float roughness, const in vec3 bitangent, const in float anisotropy ) {\n			#ifdef ENVMAP_TYPE_CUBE_UV\n				vec3 bentNormal = cross( bitangent, viewDir );\n				bentNormal = normalize( cross( bentNormal, bitangent ) );\n				bentNormal = normalize( mix( bentNormal, normal, pow2( pow2( 1.0 - anisotropy * ( 1.0 - roughness ) ) ) ) );\n				return getIBLRadiance( viewDir, bentNormal, roughness );\n			#else\n				return vec3( 0.0 );\n			#endif\n		}\n	#endif\n#endif",
	envmap_vertex: "#ifdef USE_ENVMAP\n	#ifdef ENV_WORLDPOS\n		vWorldPosition = worldPosition.xyz;\n	#else\n		vec3 cameraToVertex;\n		if ( isOrthographic ) {\n			cameraToVertex = normalize( vec3( - viewMatrix[ 0 ][ 2 ], - viewMatrix[ 1 ][ 2 ], - viewMatrix[ 2 ][ 2 ] ) );\n		} else {\n			cameraToVertex = normalize( worldPosition.xyz - cameraPosition );\n		}\n		vec3 worldNormal = inverseTransformDirection( transformedNormal, viewMatrix );\n		#ifdef ENVMAP_MODE_REFLECTION\n			vReflect = reflect( cameraToVertex, worldNormal );\n		#else\n			vReflect = refract( cameraToVertex, worldNormal, refractionRatio );\n		#endif\n	#endif\n#endif",
	fog_vertex: "#ifdef USE_FOG\n	vFogDepth = - mvPosition.z;\n#endif",
	fog_pars_vertex: "#ifdef USE_FOG\n	varying float vFogDepth;\n#endif",
	fog_fragment: "#ifdef USE_FOG\n	#ifdef FOG_EXP2\n		float fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );\n	#else\n		float fogFactor = smoothstep( fogNear, fogFar, vFogDepth );\n	#endif\n	gl_FragColor.rgb = mix( gl_FragColor.rgb, fogColor, fogFactor );\n#endif",
	fog_pars_fragment: "#ifdef USE_FOG\n	uniform vec3 fogColor;\n	varying float vFogDepth;\n	#ifdef FOG_EXP2\n		uniform float fogDensity;\n	#else\n		uniform float fogNear;\n		uniform float fogFar;\n	#endif\n#endif",
	gradientmap_pars_fragment: "#ifdef USE_GRADIENTMAP\n	uniform sampler2D gradientMap;\n#endif\nvec3 getGradientIrradiance( vec3 normal, vec3 lightDirection ) {\n	float dotNL = dot( normal, lightDirection );\n	vec2 coord = vec2( dotNL * 0.5 + 0.5, 0.0 );\n	#ifdef USE_GRADIENTMAP\n		return vec3( texture2D( gradientMap, coord ).r );\n	#else\n		vec2 fw = fwidth( coord ) * 0.5;\n		return mix( vec3( 0.7 ), vec3( 1.0 ), smoothstep( 0.7 - fw.x, 0.7 + fw.x, coord.x ) );\n	#endif\n}",
	lightmap_pars_fragment: "#ifdef USE_LIGHTMAP\n	uniform sampler2D lightMap;\n	uniform float lightMapIntensity;\n#endif",
	lights_lambert_fragment: "LambertMaterial material;\nmaterial.diffuseColor = diffuseColor.rgb;\nmaterial.specularStrength = specularStrength;",
	lights_lambert_pars_fragment: "varying vec3 vViewPosition;\nstruct LambertMaterial {\n	vec3 diffuseColor;\n	float specularStrength;\n};\nvoid RE_Direct_Lambert( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in LambertMaterial material, inout ReflectedLight reflectedLight ) {\n	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );\n	vec3 irradiance = dotNL * directLight.color;\n	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );\n}\nvoid RE_IndirectDiffuse_Lambert( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in LambertMaterial material, inout ReflectedLight reflectedLight ) {\n	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );\n}\n#define RE_Direct				RE_Direct_Lambert\n#define RE_IndirectDiffuse		RE_IndirectDiffuse_Lambert",
	lights_pars_begin: "uniform bool receiveShadow;\nuniform vec3 ambientLightColor;\n#if defined( USE_LIGHT_PROBES )\n	uniform vec3 lightProbe[ 9 ];\n#endif\nvec3 shGetIrradianceAt( in vec3 normal, in vec3 shCoefficients[ 9 ] ) {\n	float x = normal.x, y = normal.y, z = normal.z;\n	vec3 result = shCoefficients[ 0 ] * 0.886227;\n	result += shCoefficients[ 1 ] * 2.0 * 0.511664 * y;\n	result += shCoefficients[ 2 ] * 2.0 * 0.511664 * z;\n	result += shCoefficients[ 3 ] * 2.0 * 0.511664 * x;\n	result += shCoefficients[ 4 ] * 2.0 * 0.429043 * x * y;\n	result += shCoefficients[ 5 ] * 2.0 * 0.429043 * y * z;\n	result += shCoefficients[ 6 ] * ( 0.743125 * z * z - 0.247708 );\n	result += shCoefficients[ 7 ] * 2.0 * 0.429043 * x * z;\n	result += shCoefficients[ 8 ] * 0.429043 * ( x * x - y * y );\n	return result;\n}\nvec3 getLightProbeIrradiance( const in vec3 lightProbe[ 9 ], const in vec3 normal ) {\n	vec3 worldNormal = inverseTransformDirection( normal, viewMatrix );\n	vec3 irradiance = shGetIrradianceAt( worldNormal, lightProbe );\n	return irradiance;\n}\nvec3 getAmbientLightIrradiance( const in vec3 ambientLightColor ) {\n	vec3 irradiance = ambientLightColor;\n	return irradiance;\n}\nfloat getDistanceAttenuation( const in float lightDistance, const in float cutoffDistance, const in float decayExponent ) {\n	float distanceFalloff = 1.0 / max( pow( lightDistance, decayExponent ), 0.01 );\n	if ( cutoffDistance > 0.0 ) {\n		distanceFalloff *= pow2( saturate( 1.0 - pow4( lightDistance / cutoffDistance ) ) );\n	}\n	return distanceFalloff;\n}\nfloat getSpotAttenuation( const in float coneCosine, const in float penumbraCosine, const in float angleCosine ) {\n	return smoothstep( coneCosine, penumbraCosine, angleCosine );\n}\n#if NUM_DIR_LIGHTS > 0\n	struct DirectionalLight {\n		vec3 direction;\n		vec3 color;\n	};\n	uniform DirectionalLight directionalLights[ NUM_DIR_LIGHTS ];\n	void getDirectionalLightInfo( const in DirectionalLight directionalLight, out IncidentLight light ) {\n		light.color = directionalLight.color;\n		light.direction = directionalLight.direction;\n		light.visible = true;\n	}\n#endif\n#if NUM_POINT_LIGHTS > 0\n	struct PointLight {\n		vec3 position;\n		vec3 color;\n		float distance;\n		float decay;\n	};\n	uniform PointLight pointLights[ NUM_POINT_LIGHTS ];\n	void getPointLightInfo( const in PointLight pointLight, const in vec3 geometryPosition, out IncidentLight light ) {\n		vec3 lVector = pointLight.position - geometryPosition;\n		light.direction = normalize( lVector );\n		float lightDistance = length( lVector );\n		light.color = pointLight.color;\n		light.color *= getDistanceAttenuation( lightDistance, pointLight.distance, pointLight.decay );\n		light.visible = ( light.color != vec3( 0.0 ) );\n	}\n#endif\n#if NUM_SPOT_LIGHTS > 0\n	struct SpotLight {\n		vec3 position;\n		vec3 direction;\n		vec3 color;\n		float distance;\n		float decay;\n		float coneCos;\n		float penumbraCos;\n	};\n	uniform SpotLight spotLights[ NUM_SPOT_LIGHTS ];\n	void getSpotLightInfo( const in SpotLight spotLight, const in vec3 geometryPosition, out IncidentLight light ) {\n		vec3 lVector = spotLight.position - geometryPosition;\n		light.direction = normalize( lVector );\n		float angleCos = dot( light.direction, spotLight.direction );\n		float spotAttenuation = getSpotAttenuation( spotLight.coneCos, spotLight.penumbraCos, angleCos );\n		if ( spotAttenuation > 0.0 ) {\n			float lightDistance = length( lVector );\n			light.color = spotLight.color * spotAttenuation;\n			light.color *= getDistanceAttenuation( lightDistance, spotLight.distance, spotLight.decay );\n			light.visible = ( light.color != vec3( 0.0 ) );\n		} else {\n			light.color = vec3( 0.0 );\n			light.visible = false;\n		}\n	}\n#endif\n#if NUM_RECT_AREA_LIGHTS > 0\n	struct RectAreaLight {\n		vec3 color;\n		vec3 position;\n		vec3 halfWidth;\n		vec3 halfHeight;\n	};\n	uniform sampler2D ltc_1;	uniform sampler2D ltc_2;\n	uniform RectAreaLight rectAreaLights[ NUM_RECT_AREA_LIGHTS ];\n#endif\n#if NUM_HEMI_LIGHTS > 0\n	struct HemisphereLight {\n		vec3 direction;\n		vec3 skyColor;\n		vec3 groundColor;\n	};\n	uniform HemisphereLight hemisphereLights[ NUM_HEMI_LIGHTS ];\n	vec3 getHemisphereLightIrradiance( const in HemisphereLight hemiLight, const in vec3 normal ) {\n		float dotNL = dot( normal, hemiLight.direction );\n		float hemiDiffuseWeight = 0.5 * dotNL + 0.5;\n		vec3 irradiance = mix( hemiLight.groundColor, hemiLight.skyColor, hemiDiffuseWeight );\n		return irradiance;\n	}\n#endif\n#include <lightprobes_pars_fragment>",
	lights_toon_fragment: "ToonMaterial material;\nmaterial.diffuseColor = diffuseColor.rgb;",
	lights_toon_pars_fragment: "varying vec3 vViewPosition;\nstruct ToonMaterial {\n	vec3 diffuseColor;\n};\nvoid RE_Direct_Toon( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in ToonMaterial material, inout ReflectedLight reflectedLight ) {\n	vec3 irradiance = getGradientIrradiance( geometryNormal, directLight.direction ) * directLight.color;\n	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );\n}\nvoid RE_IndirectDiffuse_Toon( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in ToonMaterial material, inout ReflectedLight reflectedLight ) {\n	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );\n}\n#define RE_Direct				RE_Direct_Toon\n#define RE_IndirectDiffuse		RE_IndirectDiffuse_Toon",
	lights_phong_fragment: "BlinnPhongMaterial material;\nmaterial.diffuseColor = diffuseColor.rgb;\nmaterial.specularColor = specular;\nmaterial.specularShininess = shininess;\nmaterial.specularStrength = specularStrength;",
	lights_phong_pars_fragment: "varying vec3 vViewPosition;\nstruct BlinnPhongMaterial {\n	vec3 diffuseColor;\n	vec3 specularColor;\n	float specularShininess;\n	float specularStrength;\n};\nvoid RE_Direct_BlinnPhong( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in BlinnPhongMaterial material, inout ReflectedLight reflectedLight ) {\n	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );\n	vec3 irradiance = dotNL * directLight.color;\n	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );\n	reflectedLight.directSpecular += irradiance * BRDF_BlinnPhong( directLight.direction, geometryViewDir, geometryNormal, material.specularColor, material.specularShininess ) * material.specularStrength;\n}\nvoid RE_IndirectDiffuse_BlinnPhong( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in BlinnPhongMaterial material, inout ReflectedLight reflectedLight ) {\n	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );\n}\n#define RE_Direct				RE_Direct_BlinnPhong\n#define RE_IndirectDiffuse		RE_IndirectDiffuse_BlinnPhong",
	lights_physical_fragment: "PhysicalMaterial material;\nmaterial.diffuseColor = diffuseColor.rgb;\nmaterial.diffuseContribution = diffuseColor.rgb * ( 1.0 - metalnessFactor );\nmaterial.metalness = metalnessFactor;\nvec3 dxy = max( abs( dFdx( nonPerturbedNormal ) ), abs( dFdy( nonPerturbedNormal ) ) );\nfloat geometryRoughness = max( max( dxy.x, dxy.y ), dxy.z );\nmaterial.roughness = max( roughnessFactor, 0.0525 );material.roughness += geometryRoughness;\nmaterial.roughness = min( material.roughness, 1.0 );\n#ifdef IOR\n	material.ior = ior;\n	#ifdef USE_SPECULAR\n		float specularIntensityFactor = specularIntensity;\n		vec3 specularColorFactor = specularColor;\n		#ifdef USE_SPECULAR_COLORMAP\n			specularColorFactor *= texture2D( specularColorMap, vSpecularColorMapUv ).rgb;\n		#endif\n		#ifdef USE_SPECULAR_INTENSITYMAP\n			specularIntensityFactor *= texture2D( specularIntensityMap, vSpecularIntensityMapUv ).a;\n		#endif\n		material.specularF90 = mix( specularIntensityFactor, 1.0, metalnessFactor );\n	#else\n		float specularIntensityFactor = 1.0;\n		vec3 specularColorFactor = vec3( 1.0 );\n		material.specularF90 = 1.0;\n	#endif\n	material.specularColor = min( pow2( ( material.ior - 1.0 ) / ( material.ior + 1.0 ) ) * specularColorFactor, vec3( 1.0 ) ) * specularIntensityFactor;\n	material.specularColorBlended = mix( material.specularColor, diffuseColor.rgb, metalnessFactor );\n#else\n	material.specularColor = vec3( 0.04 );\n	material.specularColorBlended = mix( material.specularColor, diffuseColor.rgb, metalnessFactor );\n	material.specularF90 = 1.0;\n#endif\n#ifdef USE_CLEARCOAT\n	material.clearcoat = clearcoat;\n	material.clearcoatRoughness = clearcoatRoughness;\n	material.clearcoatF0 = vec3( 0.04 );\n	material.clearcoatF90 = 1.0;\n	#ifdef USE_CLEARCOATMAP\n		material.clearcoat *= texture2D( clearcoatMap, vClearcoatMapUv ).x;\n	#endif\n	#ifdef USE_CLEARCOAT_ROUGHNESSMAP\n		material.clearcoatRoughness *= texture2D( clearcoatRoughnessMap, vClearcoatRoughnessMapUv ).y;\n	#endif\n	material.clearcoat = saturate( material.clearcoat );	material.clearcoatRoughness = max( material.clearcoatRoughness, 0.0525 );\n	material.clearcoatRoughness += geometryRoughness;\n	material.clearcoatRoughness = min( material.clearcoatRoughness, 1.0 );\n#endif\n#ifdef USE_DISPERSION\n	material.dispersion = dispersion;\n#endif\n#ifdef USE_IRIDESCENCE\n	material.iridescence = iridescence;\n	material.iridescenceIOR = iridescenceIOR;\n	#ifdef USE_IRIDESCENCEMAP\n		material.iridescence *= texture2D( iridescenceMap, vIridescenceMapUv ).r;\n	#endif\n	#ifdef USE_IRIDESCENCE_THICKNESSMAP\n		material.iridescenceThickness = (iridescenceThicknessMaximum - iridescenceThicknessMinimum) * texture2D( iridescenceThicknessMap, vIridescenceThicknessMapUv ).g + iridescenceThicknessMinimum;\n	#else\n		material.iridescenceThickness = iridescenceThicknessMaximum;\n	#endif\n#endif\n#ifdef USE_SHEEN\n	material.sheenColor = sheenColor;\n	#ifdef USE_SHEEN_COLORMAP\n		material.sheenColor *= texture2D( sheenColorMap, vSheenColorMapUv ).rgb;\n	#endif\n	material.sheenRoughness = clamp( sheenRoughness, 0.0001, 1.0 );\n	#ifdef USE_SHEEN_ROUGHNESSMAP\n		material.sheenRoughness *= texture2D( sheenRoughnessMap, vSheenRoughnessMapUv ).a;\n	#endif\n#endif\n#ifdef USE_ANISOTROPY\n	#ifdef USE_ANISOTROPYMAP\n		mat2 anisotropyMat = mat2( anisotropyVector.x, anisotropyVector.y, - anisotropyVector.y, anisotropyVector.x );\n		vec3 anisotropyPolar = texture2D( anisotropyMap, vAnisotropyMapUv ).rgb;\n		vec2 anisotropyV = anisotropyMat * normalize( 2.0 * anisotropyPolar.rg - vec2( 1.0 ) ) * anisotropyPolar.b;\n	#else\n		vec2 anisotropyV = anisotropyVector;\n	#endif\n	material.anisotropy = length( anisotropyV );\n	if( material.anisotropy == 0.0 ) {\n		anisotropyV = vec2( 1.0, 0.0 );\n	} else {\n		anisotropyV /= material.anisotropy;\n		material.anisotropy = saturate( material.anisotropy );\n	}\n	material.alphaT = mix( pow2( material.roughness ), 1.0, pow2( material.anisotropy ) );\n	material.anisotropyT = tbn[ 0 ] * anisotropyV.x + tbn[ 1 ] * anisotropyV.y;\n	material.anisotropyB = tbn[ 1 ] * anisotropyV.x - tbn[ 0 ] * anisotropyV.y;\n#endif",
	lights_physical_pars_fragment: "uniform sampler2D dfgLUT;\nstruct PhysicalMaterial {\n	vec3 diffuseColor;\n	vec3 diffuseContribution;\n	vec3 specularColor;\n	vec3 specularColorBlended;\n	float roughness;\n	float metalness;\n	float specularF90;\n	float dispersion;\n	#ifdef USE_CLEARCOAT\n		float clearcoat;\n		float clearcoatRoughness;\n		vec3 clearcoatF0;\n		float clearcoatF90;\n	#endif\n	#ifdef USE_IRIDESCENCE\n		float iridescence;\n		float iridescenceIOR;\n		float iridescenceThickness;\n		vec3 iridescenceFresnel;\n		vec3 iridescenceF0;\n		vec3 iridescenceFresnelDielectric;\n		vec3 iridescenceFresnelMetallic;\n	#endif\n	#ifdef USE_SHEEN\n		vec3 sheenColor;\n		float sheenRoughness;\n	#endif\n	#ifdef IOR\n		float ior;\n	#endif\n	#ifdef USE_TRANSMISSION\n		float transmission;\n		float transmissionAlpha;\n		float thickness;\n		float attenuationDistance;\n		vec3 attenuationColor;\n	#endif\n	#ifdef USE_ANISOTROPY\n		float anisotropy;\n		float alphaT;\n		vec3 anisotropyT;\n		vec3 anisotropyB;\n	#endif\n};\nvec3 clearcoatSpecularDirect = vec3( 0.0 );\nvec3 clearcoatSpecularIndirect = vec3( 0.0 );\nvec3 sheenSpecularDirect = vec3( 0.0 );\nvec3 sheenSpecularIndirect = vec3(0.0 );\nvec3 Schlick_to_F0( const in vec3 f, const in float f90, const in float dotVH ) {\n    float x = clamp( 1.0 - dotVH, 0.0, 1.0 );\n    float x2 = x * x;\n    float x5 = clamp( x * x2 * x2, 0.0, 0.9999 );\n    return ( f - vec3( f90 ) * x5 ) / ( 1.0 - x5 );\n}\nfloat V_GGX_SmithCorrelated( const in float alpha, const in float dotNL, const in float dotNV ) {\n	float a2 = pow2( alpha );\n	float gv = dotNL * sqrt( a2 + ( 1.0 - a2 ) * pow2( dotNV ) );\n	float gl = dotNV * sqrt( a2 + ( 1.0 - a2 ) * pow2( dotNL ) );\n	return 0.5 / max( gv + gl, EPSILON );\n}\nfloat D_GGX( const in float alpha, const in float dotNH ) {\n	float a2 = pow2( alpha );\n	float denom = pow2( dotNH ) * ( a2 - 1.0 ) + 1.0;\n	return RECIPROCAL_PI * a2 / pow2( denom );\n}\n#ifdef USE_ANISOTROPY\n	float V_GGX_SmithCorrelated_Anisotropic( const in float alphaT, const in float alphaB, const in float dotTV, const in float dotBV, const in float dotTL, const in float dotBL, const in float dotNV, const in float dotNL ) {\n		float gv = dotNL * length( vec3( alphaT * dotTV, alphaB * dotBV, dotNV ) );\n		float gl = dotNV * length( vec3( alphaT * dotTL, alphaB * dotBL, dotNL ) );\n		return 0.5 / max( gv + gl, EPSILON );\n	}\n	float D_GGX_Anisotropic( const in float alphaT, const in float alphaB, const in float dotNH, const in float dotTH, const in float dotBH ) {\n		float a2 = alphaT * alphaB;\n		highp vec3 v = vec3( alphaB * dotTH, alphaT * dotBH, a2 * dotNH );\n		highp float v2 = dot( v, v );\n		float w2 = a2 / v2;\n		return RECIPROCAL_PI * a2 * pow2 ( w2 );\n	}\n#endif\n#ifdef USE_CLEARCOAT\n	vec3 BRDF_GGX_Clearcoat( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material) {\n		vec3 f0 = material.clearcoatF0;\n		float f90 = material.clearcoatF90;\n		float roughness = material.clearcoatRoughness;\n		float alpha = pow2( roughness );\n		vec3 halfDir = normalize( lightDir + viewDir );\n		float dotNL = saturate( dot( normal, lightDir ) );\n		float dotNV = saturate( dot( normal, viewDir ) );\n		float dotNH = saturate( dot( normal, halfDir ) );\n		float dotVH = saturate( dot( viewDir, halfDir ) );\n		vec3 F = F_Schlick( f0, f90, dotVH );\n		float V = V_GGX_SmithCorrelated( alpha, dotNL, dotNV );\n		float D = D_GGX( alpha, dotNH );\n		return F * ( V * D );\n	}\n#endif\nvec3 BRDF_GGX( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material ) {\n	vec3 f0 = material.specularColorBlended;\n	float f90 = material.specularF90;\n	float roughness = material.roughness;\n	float alpha = pow2( roughness );\n	vec3 halfDir = normalize( lightDir + viewDir );\n	float dotNL = saturate( dot( normal, lightDir ) );\n	float dotNV = saturate( dot( normal, viewDir ) );\n	float dotNH = saturate( dot( normal, halfDir ) );\n	float dotVH = saturate( dot( viewDir, halfDir ) );\n	vec3 F = F_Schlick( f0, f90, dotVH );\n	#ifdef USE_IRIDESCENCE\n		F = mix( F, material.iridescenceFresnel, material.iridescence );\n	#endif\n	#ifdef USE_ANISOTROPY\n		float dotTL = dot( material.anisotropyT, lightDir );\n		float dotTV = dot( material.anisotropyT, viewDir );\n		float dotTH = dot( material.anisotropyT, halfDir );\n		float dotBL = dot( material.anisotropyB, lightDir );\n		float dotBV = dot( material.anisotropyB, viewDir );\n		float dotBH = dot( material.anisotropyB, halfDir );\n		float V = V_GGX_SmithCorrelated_Anisotropic( material.alphaT, alpha, dotTV, dotBV, dotTL, dotBL, dotNV, dotNL );\n		float D = D_GGX_Anisotropic( material.alphaT, alpha, dotNH, dotTH, dotBH );\n	#else\n		float V = V_GGX_SmithCorrelated( alpha, dotNL, dotNV );\n		float D = D_GGX( alpha, dotNH );\n	#endif\n	return F * ( V * D );\n}\nvec2 LTC_Uv( const in vec3 N, const in vec3 V, const in float roughness ) {\n	const float LUT_SIZE = 64.0;\n	const float LUT_SCALE = ( LUT_SIZE - 1.0 ) / LUT_SIZE;\n	const float LUT_BIAS = 0.5 / LUT_SIZE;\n	float dotNV = saturate( dot( N, V ) );\n	vec2 uv = vec2( roughness, sqrt( 1.0 - dotNV ) );\n	uv = uv * LUT_SCALE + LUT_BIAS;\n	return uv;\n}\nfloat LTC_ClippedSphereFormFactor( const in vec3 f ) {\n	float l = length( f );\n	return max( ( l * l + f.z ) / ( l + 1.0 ), 0.0 );\n}\nvec3 LTC_EdgeVectorFormFactor( const in vec3 v1, const in vec3 v2 ) {\n	float x = dot( v1, v2 );\n	float y = abs( x );\n	float a = 0.8543985 + ( 0.4965155 + 0.0145206 * y ) * y;\n	float b = 3.4175940 + ( 4.1616724 + y ) * y;\n	float v = a / b;\n	float theta_sintheta = ( x > 0.0 ) ? v : 0.5 * inversesqrt( max( 1.0 - x * x, 1e-7 ) ) - v;\n	return cross( v1, v2 ) * theta_sintheta;\n}\nvec3 LTC_Evaluate( const in vec3 N, const in vec3 V, const in vec3 P, const in mat3 mInv, const in vec3 rectCoords[ 4 ] ) {\n	vec3 v1 = rectCoords[ 1 ] - rectCoords[ 0 ];\n	vec3 v2 = rectCoords[ 3 ] - rectCoords[ 0 ];\n	vec3 lightNormal = cross( v1, v2 );\n	if( dot( lightNormal, P - rectCoords[ 0 ] ) < 0.0 ) return vec3( 0.0 );\n	vec3 T1, T2;\n	T1 = normalize( V - N * dot( V, N ) );\n	T2 = - cross( N, T1 );\n	mat3 mat = mInv * transpose( mat3( T1, T2, N ) );\n	vec3 coords[ 4 ];\n	coords[ 0 ] = mat * ( rectCoords[ 0 ] - P );\n	coords[ 1 ] = mat * ( rectCoords[ 1 ] - P );\n	coords[ 2 ] = mat * ( rectCoords[ 2 ] - P );\n	coords[ 3 ] = mat * ( rectCoords[ 3 ] - P );\n	coords[ 0 ] = normalize( coords[ 0 ] );\n	coords[ 1 ] = normalize( coords[ 1 ] );\n	coords[ 2 ] = normalize( coords[ 2 ] );\n	coords[ 3 ] = normalize( coords[ 3 ] );\n	vec3 vectorFormFactor = vec3( 0.0 );\n	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 0 ], coords[ 1 ] );\n	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 1 ], coords[ 2 ] );\n	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 2 ], coords[ 3 ] );\n	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 3 ], coords[ 0 ] );\n	float result = LTC_ClippedSphereFormFactor( vectorFormFactor );\n	return vec3( result );\n}\n#if defined( USE_SHEEN )\nfloat D_Charlie( float roughness, float dotNH ) {\n	float alpha = pow2( roughness );\n	float invAlpha = 1.0 / alpha;\n	float cos2h = dotNH * dotNH;\n	float sin2h = max( 1.0 - cos2h, 0.0078125 );\n	return ( 2.0 + invAlpha ) * pow( sin2h, invAlpha * 0.5 ) / ( 2.0 * PI );\n}\nfloat V_Neubelt( float dotNV, float dotNL ) {\n	return saturate( 1.0 / ( 4.0 * ( dotNL + dotNV - dotNL * dotNV ) ) );\n}\nvec3 BRDF_Sheen( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, vec3 sheenColor, const in float sheenRoughness ) {\n	vec3 halfDir = normalize( lightDir + viewDir );\n	float dotNL = saturate( dot( normal, lightDir ) );\n	float dotNV = saturate( dot( normal, viewDir ) );\n	float dotNH = saturate( dot( normal, halfDir ) );\n	float D = D_Charlie( sheenRoughness, dotNH );\n	float V = V_Neubelt( dotNV, dotNL );\n	return sheenColor * ( D * V );\n}\n#endif\nfloat IBLSheenBRDF( const in vec3 normal, const in vec3 viewDir, const in float roughness ) {\n	float dotNV = saturate( dot( normal, viewDir ) );\n	float r2 = roughness * roughness;\n	float rInv = 1.0 / ( roughness + 0.1 );\n	float a = -1.9362 + 1.0678 * roughness + 0.4573 * r2 - 0.8469 * rInv;\n	float b = -0.6014 + 0.5538 * roughness - 0.4670 * r2 - 0.1255 * rInv;\n	float DG = exp( a * dotNV + b );\n	return saturate( DG );\n}\nvec3 EnvironmentBRDF( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float roughness ) {\n	float dotNV = saturate( dot( normal, viewDir ) );\n	vec2 fab = texture2D( dfgLUT, vec2( roughness, dotNV ) ).rg;\n	return specularColor * fab.x + specularF90 * fab.y;\n}\n#ifdef USE_IRIDESCENCE\nvoid computeMultiscatteringIridescence( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float iridescence, const in vec3 iridescenceF0, const in float roughness, inout vec3 singleScatter, inout vec3 multiScatter ) {\n#else\nvoid computeMultiscattering( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float roughness, inout vec3 singleScatter, inout vec3 multiScatter ) {\n#endif\n	float dotNV = saturate( dot( normal, viewDir ) );\n	vec2 fab = texture2D( dfgLUT, vec2( roughness, dotNV ) ).rg;\n	#ifdef USE_IRIDESCENCE\n		vec3 Fr = mix( specularColor, iridescenceF0, iridescence );\n	#else\n		vec3 Fr = specularColor;\n	#endif\n	vec3 FssEss = Fr * fab.x + specularF90 * fab.y;\n	float Ess = fab.x + fab.y;\n	float Ems = 1.0 - Ess;\n	vec3 Favg = Fr + ( 1.0 - Fr ) * 0.047619;	vec3 Fms = FssEss * Favg / ( 1.0 - Ems * Favg );\n	singleScatter += FssEss;\n	multiScatter += Fms * Ems;\n}\nvec3 BRDF_GGX_Multiscatter( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material ) {\n	vec3 singleScatter = BRDF_GGX( lightDir, viewDir, normal, material );\n	float dotNL = saturate( dot( normal, lightDir ) );\n	float dotNV = saturate( dot( normal, viewDir ) );\n	vec2 dfgV = texture2D( dfgLUT, vec2( material.roughness, dotNV ) ).rg;\n	vec2 dfgL = texture2D( dfgLUT, vec2( material.roughness, dotNL ) ).rg;\n	vec3 FssEss_V = material.specularColorBlended * dfgV.x + material.specularF90 * dfgV.y;\n	vec3 FssEss_L = material.specularColorBlended * dfgL.x + material.specularF90 * dfgL.y;\n	float Ess_V = dfgV.x + dfgV.y;\n	float Ess_L = dfgL.x + dfgL.y;\n	float Ems_V = 1.0 - Ess_V;\n	float Ems_L = 1.0 - Ess_L;\n	vec3 Favg = material.specularColorBlended + ( 1.0 - material.specularColorBlended ) * 0.047619;\n	vec3 Fms = FssEss_V * FssEss_L * Favg / ( 1.0 - Ems_V * Ems_L * Favg + EPSILON );\n	float compensationFactor = Ems_V * Ems_L;\n	vec3 multiScatter = Fms * compensationFactor;\n	return singleScatter + multiScatter;\n}\n#if NUM_RECT_AREA_LIGHTS > 0\n	void RE_Direct_RectArea_Physical( const in RectAreaLight rectAreaLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {\n		vec3 normal = geometryNormal;\n		vec3 viewDir = geometryViewDir;\n		vec3 position = geometryPosition;\n		vec3 lightPos = rectAreaLight.position;\n		vec3 halfWidth = rectAreaLight.halfWidth;\n		vec3 halfHeight = rectAreaLight.halfHeight;\n		vec3 lightColor = rectAreaLight.color;\n		float roughness = material.roughness;\n		vec3 rectCoords[ 4 ];\n		rectCoords[ 0 ] = lightPos + halfWidth - halfHeight;		rectCoords[ 1 ] = lightPos - halfWidth - halfHeight;\n		rectCoords[ 2 ] = lightPos - halfWidth + halfHeight;\n		rectCoords[ 3 ] = lightPos + halfWidth + halfHeight;\n		vec2 uv = LTC_Uv( normal, viewDir, roughness );\n		vec4 t1 = texture2D( ltc_1, uv );\n		vec4 t2 = texture2D( ltc_2, uv );\n		mat3 mInv = mat3(\n			vec3( t1.x, 0, t1.y ),\n			vec3(    0, 1,    0 ),\n			vec3( t1.z, 0, t1.w )\n		);\n		vec3 fresnel = ( material.specularColorBlended * t2.x + ( material.specularF90 - material.specularColorBlended ) * t2.y );\n		reflectedLight.directSpecular += lightColor * fresnel * LTC_Evaluate( normal, viewDir, position, mInv, rectCoords );\n		reflectedLight.directDiffuse += lightColor * material.diffuseContribution * LTC_Evaluate( normal, viewDir, position, mat3( 1.0 ), rectCoords );\n		#ifdef USE_CLEARCOAT\n			vec3 Ncc = geometryClearcoatNormal;\n			vec2 uvClearcoat = LTC_Uv( Ncc, viewDir, material.clearcoatRoughness );\n			vec4 t1Clearcoat = texture2D( ltc_1, uvClearcoat );\n			vec4 t2Clearcoat = texture2D( ltc_2, uvClearcoat );\n			mat3 mInvClearcoat = mat3(\n				vec3( t1Clearcoat.x, 0, t1Clearcoat.y ),\n				vec3(             0, 1,             0 ),\n				vec3( t1Clearcoat.z, 0, t1Clearcoat.w )\n			);\n			vec3 fresnelClearcoat = material.clearcoatF0 * t2Clearcoat.x + ( material.clearcoatF90 - material.clearcoatF0 ) * t2Clearcoat.y;\n			clearcoatSpecularDirect += lightColor * fresnelClearcoat * LTC_Evaluate( Ncc, viewDir, position, mInvClearcoat, rectCoords );\n		#endif\n	}\n#endif\nvoid RE_Direct_Physical( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {\n	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );\n	vec3 irradiance = dotNL * directLight.color;\n	#ifdef USE_CLEARCOAT\n		float dotNLcc = saturate( dot( geometryClearcoatNormal, directLight.direction ) );\n		vec3 ccIrradiance = dotNLcc * directLight.color;\n		clearcoatSpecularDirect += ccIrradiance * BRDF_GGX_Clearcoat( directLight.direction, geometryViewDir, geometryClearcoatNormal, material );\n	#endif\n	#ifdef USE_SHEEN\n \n 		sheenSpecularDirect += irradiance * BRDF_Sheen( directLight.direction, geometryViewDir, geometryNormal, material.sheenColor, material.sheenRoughness );\n \n 		float sheenAlbedoV = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );\n 		float sheenAlbedoL = IBLSheenBRDF( geometryNormal, directLight.direction, material.sheenRoughness );\n \n 		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * max( sheenAlbedoV, sheenAlbedoL );\n \n 		irradiance *= sheenEnergyComp;\n \n 	#endif\n	reflectedLight.directSpecular += irradiance * BRDF_GGX_Multiscatter( directLight.direction, geometryViewDir, geometryNormal, material );\n	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseContribution );\n}\nvoid RE_IndirectDiffuse_Physical( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {\n	vec3 diffuse = irradiance * BRDF_Lambert( material.diffuseContribution );\n	#ifdef USE_SHEEN\n		float sheenAlbedo = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );\n		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * sheenAlbedo;\n		diffuse *= sheenEnergyComp;\n	#endif\n	reflectedLight.indirectDiffuse += diffuse;\n}\nvoid RE_IndirectSpecular_Physical( const in vec3 radiance, const in vec3 irradiance, const in vec3 clearcoatRadiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight) {\n	#ifdef USE_CLEARCOAT\n		clearcoatSpecularIndirect += clearcoatRadiance * EnvironmentBRDF( geometryClearcoatNormal, geometryViewDir, material.clearcoatF0, material.clearcoatF90, material.clearcoatRoughness );\n	#endif\n	#ifdef USE_SHEEN\n		sheenSpecularIndirect += irradiance * material.sheenColor * IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness ) * RECIPROCAL_PI;\n 	#endif\n	vec3 singleScatteringDielectric = vec3( 0.0 );\n	vec3 multiScatteringDielectric = vec3( 0.0 );\n	vec3 singleScatteringMetallic = vec3( 0.0 );\n	vec3 multiScatteringMetallic = vec3( 0.0 );\n	#ifdef USE_IRIDESCENCE\n		computeMultiscatteringIridescence( geometryNormal, geometryViewDir, material.specularColor, material.specularF90, material.iridescence, material.iridescenceFresnelDielectric, material.roughness, singleScatteringDielectric, multiScatteringDielectric );\n		computeMultiscatteringIridescence( geometryNormal, geometryViewDir, material.diffuseColor, material.specularF90, material.iridescence, material.iridescenceFresnelMetallic, material.roughness, singleScatteringMetallic, multiScatteringMetallic );\n	#else\n		computeMultiscattering( geometryNormal, geometryViewDir, material.specularColor, material.specularF90, material.roughness, singleScatteringDielectric, multiScatteringDielectric );\n		computeMultiscattering( geometryNormal, geometryViewDir, material.diffuseColor, material.specularF90, material.roughness, singleScatteringMetallic, multiScatteringMetallic );\n	#endif\n	vec3 singleScattering = mix( singleScatteringDielectric, singleScatteringMetallic, material.metalness );\n	vec3 multiScattering = mix( multiScatteringDielectric, multiScatteringMetallic, material.metalness );\n	vec3 totalScatteringDielectric = singleScatteringDielectric + multiScatteringDielectric;\n	vec3 diffuse = material.diffuseContribution * ( 1.0 - totalScatteringDielectric );\n	vec3 cosineWeightedIrradiance = irradiance * RECIPROCAL_PI;\n	vec3 indirectSpecular = radiance * singleScattering;\n	indirectSpecular += multiScattering * cosineWeightedIrradiance;\n	vec3 indirectDiffuse = diffuse * cosineWeightedIrradiance;\n	#ifdef USE_SHEEN\n		float sheenAlbedo = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );\n		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * sheenAlbedo;\n		indirectSpecular *= sheenEnergyComp;\n		indirectDiffuse *= sheenEnergyComp;\n	#endif\n	reflectedLight.indirectSpecular += indirectSpecular;\n	reflectedLight.indirectDiffuse += indirectDiffuse;\n}\n#define RE_Direct				RE_Direct_Physical\n#define RE_Direct_RectArea		RE_Direct_RectArea_Physical\n#define RE_IndirectDiffuse		RE_IndirectDiffuse_Physical\n#define RE_IndirectSpecular		RE_IndirectSpecular_Physical\nfloat computeSpecularOcclusion( const in float dotNV, const in float ambientOcclusion, const in float roughness ) {\n	return saturate( pow( dotNV + ambientOcclusion, exp2( - 16.0 * roughness - 1.0 ) ) - 1.0 + ambientOcclusion );\n}",
	lights_fragment_begin: "\nvec3 geometryPosition = - vViewPosition;\nvec3 geometryNormal = normal;\nvec3 geometryViewDir = ( isOrthographic ) ? vec3( 0, 0, 1 ) : normalize( vViewPosition );\nvec3 geometryClearcoatNormal = vec3( 0.0 );\n#ifdef USE_CLEARCOAT\n	geometryClearcoatNormal = clearcoatNormal;\n#endif\n#ifdef USE_IRIDESCENCE\n	float dotNVi = saturate( dot( normal, geometryViewDir ) );\n	if ( material.iridescenceThickness == 0.0 ) {\n		material.iridescence = 0.0;\n	} else {\n		material.iridescence = saturate( material.iridescence );\n	}\n	if ( material.iridescence > 0.0 ) {\n		material.iridescenceFresnelDielectric = evalIridescence( 1.0, material.iridescenceIOR, dotNVi, material.iridescenceThickness, material.specularColor );\n		material.iridescenceFresnelMetallic = evalIridescence( 1.0, material.iridescenceIOR, dotNVi, material.iridescenceThickness, material.diffuseColor );\n		material.iridescenceFresnel = mix( material.iridescenceFresnelDielectric, material.iridescenceFresnelMetallic, material.metalness );\n		material.iridescenceF0 = Schlick_to_F0( material.iridescenceFresnel, 1.0, dotNVi );\n	}\n#endif\nIncidentLight directLight;\n#if ( NUM_POINT_LIGHTS > 0 ) && defined( RE_Direct )\n	PointLight pointLight;\n	#if defined( USE_SHADOWMAP ) && NUM_POINT_LIGHT_SHADOWS > 0\n	PointLightShadow pointLightShadow;\n	#endif\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_POINT_LIGHTS; i ++ ) {\n		pointLight = pointLights[ i ];\n		getPointLightInfo( pointLight, geometryPosition, directLight );\n		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_POINT_LIGHT_SHADOWS ) && ( defined( SHADOWMAP_TYPE_PCF ) || defined( SHADOWMAP_TYPE_BASIC ) )\n		pointLightShadow = pointLightShadows[ i ];\n		directLight.color *= ( directLight.visible && receiveShadow ) ? getPointShadow( pointShadowMap[ i ], pointLightShadow.shadowMapSize, pointLightShadow.shadowIntensity, pointLightShadow.shadowBias, pointLightShadow.shadowRadius, vPointShadowCoord[ i ], pointLightShadow.shadowCameraNear, pointLightShadow.shadowCameraFar ) : 1.0;\n		#endif\n		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );\n	}\n	#pragma unroll_loop_end\n#endif\n#if ( NUM_SPOT_LIGHTS > 0 ) && defined( RE_Direct )\n	SpotLight spotLight;\n	vec4 spotColor;\n	vec3 spotLightCoord;\n	bool inSpotLightMap;\n	#if defined( USE_SHADOWMAP ) && NUM_SPOT_LIGHT_SHADOWS > 0\n	SpotLightShadow spotLightShadow;\n	#endif\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_SPOT_LIGHTS; i ++ ) {\n		spotLight = spotLights[ i ];\n		getSpotLightInfo( spotLight, geometryPosition, directLight );\n		#if ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS )\n		#define SPOT_LIGHT_MAP_INDEX UNROLLED_LOOP_INDEX\n		#elif ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )\n		#define SPOT_LIGHT_MAP_INDEX NUM_SPOT_LIGHT_MAPS\n		#else\n		#define SPOT_LIGHT_MAP_INDEX ( UNROLLED_LOOP_INDEX - NUM_SPOT_LIGHT_SHADOWS + NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS )\n		#endif\n		#if ( SPOT_LIGHT_MAP_INDEX < NUM_SPOT_LIGHT_MAPS )\n			spotLightCoord = vSpotLightCoord[ i ].xyz / vSpotLightCoord[ i ].w;\n			inSpotLightMap = all( lessThan( abs( spotLightCoord * 2. - 1. ), vec3( 1.0 ) ) );\n			spotColor = texture2D( spotLightMap[ SPOT_LIGHT_MAP_INDEX ], spotLightCoord.xy );\n			directLight.color = inSpotLightMap ? directLight.color * spotColor.rgb : directLight.color;\n		#endif\n		#undef SPOT_LIGHT_MAP_INDEX\n		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )\n		spotLightShadow = spotLightShadows[ i ];\n		directLight.color *= ( directLight.visible && receiveShadow ) ? getShadow( spotShadowMap[ i ], spotLightShadow.shadowMapSize, spotLightShadow.shadowIntensity, spotLightShadow.shadowBias, spotLightShadow.shadowRadius, vSpotLightCoord[ i ] ) : 1.0;\n		#endif\n		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );\n	}\n	#pragma unroll_loop_end\n#endif\n#if ( NUM_DIR_LIGHTS > 0 ) && defined( RE_Direct )\n	DirectionalLight directionalLight;\n	#if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0\n	DirectionalLightShadow directionalLightShadow;\n	#endif\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_DIR_LIGHTS; i ++ ) {\n		directionalLight = directionalLights[ i ];\n		getDirectionalLightInfo( directionalLight, directLight );\n		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_DIR_LIGHT_SHADOWS )\n		directionalLightShadow = directionalLightShadows[ i ];\n		directLight.color *= ( directLight.visible && receiveShadow ) ? getShadow( directionalShadowMap[ i ], directionalLightShadow.shadowMapSize, directionalLightShadow.shadowIntensity, directionalLightShadow.shadowBias, directionalLightShadow.shadowRadius, vDirectionalShadowCoord[ i ] ) : 1.0;\n		#endif\n		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );\n	}\n	#pragma unroll_loop_end\n#endif\n#if ( NUM_RECT_AREA_LIGHTS > 0 ) && defined( RE_Direct_RectArea )\n	RectAreaLight rectAreaLight;\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_RECT_AREA_LIGHTS; i ++ ) {\n		rectAreaLight = rectAreaLights[ i ];\n		RE_Direct_RectArea( rectAreaLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );\n	}\n	#pragma unroll_loop_end\n#endif\n#if defined( RE_IndirectDiffuse )\n	vec3 iblIrradiance = vec3( 0.0 );\n	vec3 irradiance = getAmbientLightIrradiance( ambientLightColor );\n	#if defined( USE_LIGHT_PROBES )\n		irradiance += getLightProbeIrradiance( lightProbe, geometryNormal );\n	#endif\n	#if ( NUM_HEMI_LIGHTS > 0 )\n		#pragma unroll_loop_start\n		for ( int i = 0; i < NUM_HEMI_LIGHTS; i ++ ) {\n			irradiance += getHemisphereLightIrradiance( hemisphereLights[ i ], geometryNormal );\n		}\n		#pragma unroll_loop_end\n	#endif\n	#ifdef USE_LIGHT_PROBES_GRID\n		vec3 probeWorldPos = ( ( vec4( geometryPosition, 1.0 ) - viewMatrix[ 3 ] ) * viewMatrix ).xyz;\n		vec3 probeWorldNormal = inverseTransformDirection( geometryNormal, viewMatrix );\n		irradiance += getLightProbeGridIrradiance( probeWorldPos, probeWorldNormal );\n	#endif\n#endif\n#if defined( RE_IndirectSpecular )\n	vec3 radiance = vec3( 0.0 );\n	vec3 clearcoatRadiance = vec3( 0.0 );\n#endif",
	lights_fragment_maps: "#if defined( RE_IndirectDiffuse )\n	#ifdef USE_LIGHTMAP\n		vec4 lightMapTexel = texture2D( lightMap, vLightMapUv );\n		vec3 lightMapIrradiance = lightMapTexel.rgb * lightMapIntensity;\n		irradiance += lightMapIrradiance;\n	#endif\n	#if defined( USE_ENVMAP ) && defined( ENVMAP_TYPE_CUBE_UV )\n		#if defined( STANDARD ) || defined( LAMBERT ) || defined( PHONG )\n			iblIrradiance += getIBLIrradiance( geometryNormal );\n		#endif\n	#endif\n#endif\n#if defined( USE_ENVMAP ) && defined( RE_IndirectSpecular )\n	#ifdef USE_ANISOTROPY\n		radiance += getIBLAnisotropyRadiance( geometryViewDir, geometryNormal, material.roughness, material.anisotropyB, material.anisotropy );\n	#else\n		radiance += getIBLRadiance( geometryViewDir, geometryNormal, material.roughness );\n	#endif\n	#ifdef USE_CLEARCOAT\n		clearcoatRadiance += getIBLRadiance( geometryViewDir, geometryClearcoatNormal, material.clearcoatRoughness );\n	#endif\n#endif",
	lights_fragment_end: "#if defined( RE_IndirectDiffuse )\n	#if defined( LAMBERT ) || defined( PHONG )\n		irradiance += iblIrradiance;\n	#endif\n	RE_IndirectDiffuse( irradiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );\n#endif\n#if defined( RE_IndirectSpecular )\n	RE_IndirectSpecular( radiance, iblIrradiance, clearcoatRadiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );\n#endif",
	lightprobes_pars_fragment: "#ifdef USE_LIGHT_PROBES_GRID\nuniform highp sampler3D probesSH;\nuniform vec3 probesMin;\nuniform vec3 probesMax;\nuniform vec3 probesResolution;\nvec3 getLightProbeGridIrradiance( vec3 worldPos, vec3 worldNormal ) {\n	vec3 res = probesResolution;\n	vec3 gridRange = probesMax - probesMin;\n	vec3 resMinusOne = res - 1.0;\n	vec3 probeSpacing = gridRange / resMinusOne;\n	vec3 samplePos = worldPos + worldNormal * probeSpacing * 0.5;\n	vec3 uvw = clamp( ( samplePos - probesMin ) / gridRange, 0.0, 1.0 );\n	uvw = uvw * resMinusOne / res + 0.5 / res;\n	float nz          = res.z;\n	float paddedSlices = nz + 2.0;\n	float atlasDepth  = 7.0 * paddedSlices;\n	float uvZBase     = uvw.z * nz + 1.0;\n	vec4 s0 = texture( probesSH, vec3( uvw.xy, ( uvZBase                       ) / atlasDepth ) );\n	vec4 s1 = texture( probesSH, vec3( uvw.xy, ( uvZBase +       paddedSlices   ) / atlasDepth ) );\n	vec4 s2 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 2.0 * paddedSlices   ) / atlasDepth ) );\n	vec4 s3 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 3.0 * paddedSlices   ) / atlasDepth ) );\n	vec4 s4 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 4.0 * paddedSlices   ) / atlasDepth ) );\n	vec4 s5 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 5.0 * paddedSlices   ) / atlasDepth ) );\n	vec4 s6 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 6.0 * paddedSlices   ) / atlasDepth ) );\n	vec3 c0 = s0.xyz;\n	vec3 c1 = vec3( s0.w, s1.xy );\n	vec3 c2 = vec3( s1.zw, s2.x );\n	vec3 c3 = s2.yzw;\n	vec3 c4 = s3.xyz;\n	vec3 c5 = vec3( s3.w, s4.xy );\n	vec3 c6 = vec3( s4.zw, s5.x );\n	vec3 c7 = s5.yzw;\n	vec3 c8 = s6.xyz;\n	float x = worldNormal.x, y = worldNormal.y, z = worldNormal.z;\n	vec3 result = c0 * 0.886227;\n	result += c1 * 2.0 * 0.511664 * y;\n	result += c2 * 2.0 * 0.511664 * z;\n	result += c3 * 2.0 * 0.511664 * x;\n	result += c4 * 2.0 * 0.429043 * x * y;\n	result += c5 * 2.0 * 0.429043 * y * z;\n	result += c6 * ( 0.743125 * z * z - 0.247708 );\n	result += c7 * 2.0 * 0.429043 * x * z;\n	result += c8 * 0.429043 * ( x * x - y * y );\n	return max( result, vec3( 0.0 ) );\n}\n#endif",
	logdepthbuf_fragment: "#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )\n	gl_FragDepth = vIsPerspective == 0.0 ? gl_FragCoord.z : log2( vFragDepth ) * logDepthBufFC * 0.5;\n#endif",
	logdepthbuf_pars_fragment: "#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )\n	uniform float logDepthBufFC;\n	varying float vFragDepth;\n	varying float vIsPerspective;\n#endif",
	logdepthbuf_pars_vertex: "#ifdef USE_LOGARITHMIC_DEPTH_BUFFER\n	varying float vFragDepth;\n	varying float vIsPerspective;\n#endif",
	logdepthbuf_vertex: "#ifdef USE_LOGARITHMIC_DEPTH_BUFFER\n	vFragDepth = 1.0 + gl_Position.w;\n	vIsPerspective = float( isPerspectiveMatrix( projectionMatrix ) );\n#endif",
	map_fragment: "#ifdef USE_MAP\n	vec4 sampledDiffuseColor = texture2D( map, vMapUv );\n	#ifdef DECODE_VIDEO_TEXTURE\n		sampledDiffuseColor = sRGBTransferEOTF( sampledDiffuseColor );\n	#endif\n	diffuseColor *= sampledDiffuseColor;\n#endif",
	map_pars_fragment: "#ifdef USE_MAP\n	uniform sampler2D map;\n#endif",
	map_particle_fragment: "#if defined( USE_MAP ) || defined( USE_ALPHAMAP )\n	#if defined( USE_POINTS_UV )\n		vec2 uv = vUv;\n	#else\n		vec2 uv = ( uvTransform * vec3( gl_PointCoord.x, 1.0 - gl_PointCoord.y, 1 ) ).xy;\n	#endif\n#endif\n#ifdef USE_MAP\n	diffuseColor *= texture2D( map, uv );\n#endif\n#ifdef USE_ALPHAMAP\n	diffuseColor.a *= texture2D( alphaMap, uv ).g;\n#endif",
	map_particle_pars_fragment: "#if defined( USE_POINTS_UV )\n	varying vec2 vUv;\n#else\n	#if defined( USE_MAP ) || defined( USE_ALPHAMAP )\n		uniform mat3 uvTransform;\n	#endif\n#endif\n#ifdef USE_MAP\n	uniform sampler2D map;\n#endif\n#ifdef USE_ALPHAMAP\n	uniform sampler2D alphaMap;\n#endif",
	metalnessmap_fragment: "float metalnessFactor = metalness;\n#ifdef USE_METALNESSMAP\n	vec4 texelMetalness = texture2D( metalnessMap, vMetalnessMapUv );\n	metalnessFactor *= texelMetalness.b;\n#endif",
	metalnessmap_pars_fragment: "#ifdef USE_METALNESSMAP\n	uniform sampler2D metalnessMap;\n#endif",
	morphinstance_vertex: "#ifdef USE_INSTANCING_MORPH\n	float morphTargetInfluences[ MORPHTARGETS_COUNT ];\n	float morphTargetBaseInfluence = texelFetch( morphTexture, ivec2( 0, gl_InstanceID ), 0 ).r;\n	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {\n		morphTargetInfluences[i] =  texelFetch( morphTexture, ivec2( i + 1, gl_InstanceID ), 0 ).r;\n	}\n#endif",
	morphcolor_vertex: "#if defined( USE_MORPHCOLORS )\n	vColor *= morphTargetBaseInfluence;\n	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {\n		#if defined( USE_COLOR_ALPHA )\n			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ) * morphTargetInfluences[ i ];\n		#elif defined( USE_COLOR )\n			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ).rgb * morphTargetInfluences[ i ];\n		#endif\n	}\n#endif",
	morphnormal_vertex: "#ifdef USE_MORPHNORMALS\n	objectNormal *= morphTargetBaseInfluence;\n	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {\n		if ( morphTargetInfluences[ i ] != 0.0 ) objectNormal += getMorph( gl_VertexID, i, 1 ).xyz * morphTargetInfluences[ i ];\n	}\n#endif",
	morphtarget_pars_vertex: "#ifdef USE_MORPHTARGETS\n	#ifndef USE_INSTANCING_MORPH\n		uniform float morphTargetBaseInfluence;\n		uniform float morphTargetInfluences[ MORPHTARGETS_COUNT ];\n	#endif\n	uniform sampler2DArray morphTargetsTexture;\n	uniform ivec2 morphTargetsTextureSize;\n	vec4 getMorph( const in int vertexIndex, const in int morphTargetIndex, const in int offset ) {\n		int texelIndex = vertexIndex * MORPHTARGETS_TEXTURE_STRIDE + offset;\n		int y = texelIndex / morphTargetsTextureSize.x;\n		int x = texelIndex - y * morphTargetsTextureSize.x;\n		ivec3 morphUV = ivec3( x, y, morphTargetIndex );\n		return texelFetch( morphTargetsTexture, morphUV, 0 );\n	}\n#endif",
	morphtarget_vertex: "#ifdef USE_MORPHTARGETS\n	transformed *= morphTargetBaseInfluence;\n	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {\n		if ( morphTargetInfluences[ i ] != 0.0 ) transformed += getMorph( gl_VertexID, i, 0 ).xyz * morphTargetInfluences[ i ];\n	}\n#endif",
	normal_fragment_begin: "float faceDirection = gl_FrontFacing ? 1.0 : - 1.0;\n#ifdef FLAT_SHADED\n	vec3 fdx = dFdx( vViewPosition );\n	vec3 fdy = dFdy( vViewPosition );\n	vec3 normal = normalize( cross( fdx, fdy ) );\n#else\n	vec3 normal = normalize( vNormal );\n	#ifdef DOUBLE_SIDED\n		normal *= faceDirection;\n	#endif\n#endif\n#if defined( USE_NORMALMAP_TANGENTSPACE ) || defined( USE_CLEARCOAT_NORMALMAP ) || defined( USE_ANISOTROPY )\n	#ifdef USE_TANGENT\n		mat3 tbn = mat3( normalize( vTangent ), normalize( vBitangent ), normal );\n	#else\n		mat3 tbn = getTangentFrame( - vViewPosition, normal,\n		#if defined( USE_NORMALMAP )\n			vNormalMapUv\n		#elif defined( USE_CLEARCOAT_NORMALMAP )\n			vClearcoatNormalMapUv\n		#else\n			vUv\n		#endif\n		);\n	#endif\n	#if defined( DOUBLE_SIDED ) && ! defined( FLAT_SHADED )\n		tbn[0] *= faceDirection;\n		tbn[1] *= faceDirection;\n	#endif\n#endif\n#ifdef USE_CLEARCOAT_NORMALMAP\n	#ifdef USE_TANGENT\n		mat3 tbn2 = mat3( normalize( vTangent ), normalize( vBitangent ), normal );\n	#else\n		mat3 tbn2 = getTangentFrame( - vViewPosition, normal, vClearcoatNormalMapUv );\n	#endif\n	#if defined( DOUBLE_SIDED ) && ! defined( FLAT_SHADED )\n		tbn2[0] *= faceDirection;\n		tbn2[1] *= faceDirection;\n	#endif\n#endif\nvec3 nonPerturbedNormal = normal;",
	normal_fragment_maps: "#ifdef USE_NORMALMAP_OBJECTSPACE\n	normal = texture2D( normalMap, vNormalMapUv ).xyz * 2.0 - 1.0;\n	#ifdef FLIP_SIDED\n		normal = - normal;\n	#endif\n	#ifdef DOUBLE_SIDED\n		normal = normal * faceDirection;\n	#endif\n	normal = normalize( normalMatrix * normal );\n#elif defined( USE_NORMALMAP_TANGENTSPACE )\n	vec3 mapN = texture2D( normalMap, vNormalMapUv ).xyz * 2.0 - 1.0;\n	#if defined( USE_PACKED_NORMALMAP )\n		mapN = vec3( mapN.xy, sqrt( saturate( 1.0 - dot( mapN.xy, mapN.xy ) ) ) );\n	#endif\n	mapN.xy *= normalScale;\n	normal = normalize( tbn * mapN );\n#elif defined( USE_BUMPMAP )\n	normal = perturbNormalArb( - vViewPosition, normal, dHdxy_fwd(), faceDirection );\n#endif",
	normal_pars_fragment: "#ifndef FLAT_SHADED\n	varying vec3 vNormal;\n	#ifdef USE_TANGENT\n		varying vec3 vTangent;\n		varying vec3 vBitangent;\n	#endif\n#endif",
	normal_pars_vertex: "#ifndef FLAT_SHADED\n	varying vec3 vNormal;\n	#ifdef USE_TANGENT\n		varying vec3 vTangent;\n		varying vec3 vBitangent;\n	#endif\n#endif",
	normal_vertex: "#ifndef FLAT_SHADED\n	vNormal = normalize( transformedNormal );\n	#ifdef USE_TANGENT\n		vTangent = normalize( transformedTangent );\n		vBitangent = normalize( cross( vNormal, vTangent ) * tangent.w );\n	#endif\n#endif",
	normalmap_pars_fragment: "#ifdef USE_NORMALMAP\n	uniform sampler2D normalMap;\n	uniform vec2 normalScale;\n#endif\n#ifdef USE_NORMALMAP_OBJECTSPACE\n	uniform mat3 normalMatrix;\n#endif\n#if ! defined ( USE_TANGENT ) && ( defined ( USE_NORMALMAP_TANGENTSPACE ) || defined ( USE_CLEARCOAT_NORMALMAP ) || defined( USE_ANISOTROPY ) )\n	mat3 getTangentFrame( vec3 eye_pos, vec3 surf_norm, vec2 uv ) {\n		vec3 q0 = dFdx( eye_pos.xyz );\n		vec3 q1 = dFdy( eye_pos.xyz );\n		vec2 st0 = dFdx( uv.st );\n		vec2 st1 = dFdy( uv.st );\n		vec3 N = surf_norm;\n		vec3 q1perp = cross( q1, N );\n		vec3 q0perp = cross( N, q0 );\n		vec3 T = q1perp * st0.x + q0perp * st1.x;\n		vec3 B = q1perp * st0.y + q0perp * st1.y;\n		float det = max( dot( T, T ), dot( B, B ) );\n		float scale = ( det == 0.0 ) ? 0.0 : inversesqrt( det );\n		return mat3( T * scale, B * scale, N );\n	}\n#endif",
	clearcoat_normal_fragment_begin: "#ifdef USE_CLEARCOAT\n	vec3 clearcoatNormal = nonPerturbedNormal;\n#endif",
	clearcoat_normal_fragment_maps: "#ifdef USE_CLEARCOAT_NORMALMAP\n	vec3 clearcoatMapN = texture2D( clearcoatNormalMap, vClearcoatNormalMapUv ).xyz * 2.0 - 1.0;\n	clearcoatMapN.xy *= clearcoatNormalScale;\n	clearcoatNormal = normalize( tbn2 * clearcoatMapN );\n#endif",
	clearcoat_pars_fragment: "#ifdef USE_CLEARCOATMAP\n	uniform sampler2D clearcoatMap;\n#endif\n#ifdef USE_CLEARCOAT_NORMALMAP\n	uniform sampler2D clearcoatNormalMap;\n	uniform vec2 clearcoatNormalScale;\n#endif\n#ifdef USE_CLEARCOAT_ROUGHNESSMAP\n	uniform sampler2D clearcoatRoughnessMap;\n#endif",
	iridescence_pars_fragment: "#ifdef USE_IRIDESCENCEMAP\n	uniform sampler2D iridescenceMap;\n#endif\n#ifdef USE_IRIDESCENCE_THICKNESSMAP\n	uniform sampler2D iridescenceThicknessMap;\n#endif",
	opaque_fragment: "#ifdef OPAQUE\ndiffuseColor.a = 1.0;\n#endif\n#ifdef USE_TRANSMISSION\ndiffuseColor.a *= material.transmissionAlpha;\n#endif\ngl_FragColor = vec4( outgoingLight, diffuseColor.a );",
	packing: "vec3 packNormalToRGB( const in vec3 normal ) {\n	return normalize( normal ) * 0.5 + 0.5;\n}\nvec3 unpackRGBToNormal( const in vec3 rgb ) {\n	return 2.0 * rgb.xyz - 1.0;\n}\nconst float PackUpscale = 256. / 255.;const float UnpackDownscale = 255. / 256.;const float ShiftRight8 = 1. / 256.;\nconst float Inv255 = 1. / 255.;\nconst vec4 PackFactors = vec4( 1.0, 256.0, 256.0 * 256.0, 256.0 * 256.0 * 256.0 );\nconst vec2 UnpackFactors2 = vec2( UnpackDownscale, 1.0 / PackFactors.g );\nconst vec3 UnpackFactors3 = vec3( UnpackDownscale / PackFactors.rg, 1.0 / PackFactors.b );\nconst vec4 UnpackFactors4 = vec4( UnpackDownscale / PackFactors.rgb, 1.0 / PackFactors.a );\nvec4 packDepthToRGBA( const in float v ) {\n	if( v <= 0.0 )\n		return vec4( 0., 0., 0., 0. );\n	if( v >= 1.0 )\n		return vec4( 1., 1., 1., 1. );\n	float vuf;\n	float af = modf( v * PackFactors.a, vuf );\n	float bf = modf( vuf * ShiftRight8, vuf );\n	float gf = modf( vuf * ShiftRight8, vuf );\n	return vec4( vuf * Inv255, gf * PackUpscale, bf * PackUpscale, af );\n}\nvec3 packDepthToRGB( const in float v ) {\n	if( v <= 0.0 )\n		return vec3( 0., 0., 0. );\n	if( v >= 1.0 )\n		return vec3( 1., 1., 1. );\n	float vuf;\n	float bf = modf( v * PackFactors.b, vuf );\n	float gf = modf( vuf * ShiftRight8, vuf );\n	return vec3( vuf * Inv255, gf * PackUpscale, bf );\n}\nvec2 packDepthToRG( const in float v ) {\n	if( v <= 0.0 )\n		return vec2( 0., 0. );\n	if( v >= 1.0 )\n		return vec2( 1., 1. );\n	float vuf;\n	float gf = modf( v * 256., vuf );\n	return vec2( vuf * Inv255, gf );\n}\nfloat unpackRGBAToDepth( const in vec4 v ) {\n	return dot( v, UnpackFactors4 );\n}\nfloat unpackRGBToDepth( const in vec3 v ) {\n	return dot( v, UnpackFactors3 );\n}\nfloat unpackRGToDepth( const in vec2 v ) {\n	return v.r * UnpackFactors2.r + v.g * UnpackFactors2.g;\n}\nvec4 pack2HalfToRGBA( const in vec2 v ) {\n	vec4 r = vec4( v.x, fract( v.x * 255.0 ), v.y, fract( v.y * 255.0 ) );\n	return vec4( r.x - r.y / 255.0, r.y, r.z - r.w / 255.0, r.w );\n}\nvec2 unpackRGBATo2Half( const in vec4 v ) {\n	return vec2( v.x + ( v.y / 255.0 ), v.z + ( v.w / 255.0 ) );\n}\nfloat viewZToOrthographicDepth( const in float viewZ, const in float near, const in float far ) {\n	return ( viewZ + near ) / ( near - far );\n}\nfloat orthographicDepthToViewZ( const in float depth, const in float near, const in float far ) {\n	#ifdef USE_REVERSED_DEPTH_BUFFER\n	\n		return depth * ( far - near ) - far;\n	#else\n		return depth * ( near - far ) - near;\n	#endif\n}\nfloat viewZToPerspectiveDepth( const in float viewZ, const in float near, const in float far ) {\n	return ( ( near + viewZ ) * far ) / ( ( far - near ) * viewZ );\n}\nfloat perspectiveDepthToViewZ( const in float depth, const in float near, const in float far ) {\n	\n	#ifdef USE_REVERSED_DEPTH_BUFFER\n		return ( near * far ) / ( ( near - far ) * depth - near );\n	#else\n		return ( near * far ) / ( ( far - near ) * depth - far );\n	#endif\n}",
	premultiplied_alpha_fragment: "#ifdef PREMULTIPLIED_ALPHA\n	gl_FragColor.rgb *= gl_FragColor.a;\n#endif",
	project_vertex: "vec4 mvPosition = vec4( transformed, 1.0 );\n#ifdef USE_BATCHING\n	mvPosition = batchingMatrix * mvPosition;\n#endif\n#ifdef USE_INSTANCING\n	mvPosition = instanceMatrix * mvPosition;\n#endif\nmvPosition = modelViewMatrix * mvPosition;\ngl_Position = projectionMatrix * mvPosition;",
	dithering_fragment: "#ifdef DITHERING\n	gl_FragColor.rgb = dithering( gl_FragColor.rgb );\n#endif",
	dithering_pars_fragment: "#ifdef DITHERING\n	vec3 dithering( vec3 color ) {\n		float grid_position = rand( gl_FragCoord.xy );\n		vec3 dither_shift_RGB = vec3( 0.25 / 255.0, -0.25 / 255.0, 0.25 / 255.0 );\n		dither_shift_RGB = mix( 2.0 * dither_shift_RGB, -2.0 * dither_shift_RGB, grid_position );\n		return color + dither_shift_RGB;\n	}\n#endif",
	roughnessmap_fragment: "float roughnessFactor = roughness;\n#ifdef USE_ROUGHNESSMAP\n	vec4 texelRoughness = texture2D( roughnessMap, vRoughnessMapUv );\n	roughnessFactor *= texelRoughness.g;\n#endif",
	roughnessmap_pars_fragment: "#ifdef USE_ROUGHNESSMAP\n	uniform sampler2D roughnessMap;\n#endif",
	shadowmap_pars_fragment: "#if NUM_SPOT_LIGHT_COORDS > 0\n	varying vec4 vSpotLightCoord[ NUM_SPOT_LIGHT_COORDS ];\n#endif\n#if NUM_SPOT_LIGHT_MAPS > 0\n	uniform sampler2D spotLightMap[ NUM_SPOT_LIGHT_MAPS ];\n#endif\n#ifdef USE_SHADOWMAP\n	#if NUM_DIR_LIGHT_SHADOWS > 0\n		#if defined( SHADOWMAP_TYPE_PCF )\n			uniform sampler2DShadow directionalShadowMap[ NUM_DIR_LIGHT_SHADOWS ];\n		#else\n			uniform sampler2D directionalShadowMap[ NUM_DIR_LIGHT_SHADOWS ];\n		#endif\n		varying vec4 vDirectionalShadowCoord[ NUM_DIR_LIGHT_SHADOWS ];\n		struct DirectionalLightShadow {\n			float shadowIntensity;\n			float shadowBias;\n			float shadowNormalBias;\n			float shadowRadius;\n			vec2 shadowMapSize;\n		};\n		uniform DirectionalLightShadow directionalLightShadows[ NUM_DIR_LIGHT_SHADOWS ];\n	#endif\n	#if NUM_SPOT_LIGHT_SHADOWS > 0\n		#if defined( SHADOWMAP_TYPE_PCF )\n			uniform sampler2DShadow spotShadowMap[ NUM_SPOT_LIGHT_SHADOWS ];\n		#else\n			uniform sampler2D spotShadowMap[ NUM_SPOT_LIGHT_SHADOWS ];\n		#endif\n		struct SpotLightShadow {\n			float shadowIntensity;\n			float shadowBias;\n			float shadowNormalBias;\n			float shadowRadius;\n			vec2 shadowMapSize;\n		};\n		uniform SpotLightShadow spotLightShadows[ NUM_SPOT_LIGHT_SHADOWS ];\n	#endif\n	#if NUM_POINT_LIGHT_SHADOWS > 0\n		#if defined( SHADOWMAP_TYPE_PCF )\n			uniform samplerCubeShadow pointShadowMap[ NUM_POINT_LIGHT_SHADOWS ];\n		#elif defined( SHADOWMAP_TYPE_BASIC )\n			uniform samplerCube pointShadowMap[ NUM_POINT_LIGHT_SHADOWS ];\n		#endif\n		varying vec4 vPointShadowCoord[ NUM_POINT_LIGHT_SHADOWS ];\n		struct PointLightShadow {\n			float shadowIntensity;\n			float shadowBias;\n			float shadowNormalBias;\n			float shadowRadius;\n			vec2 shadowMapSize;\n			float shadowCameraNear;\n			float shadowCameraFar;\n		};\n		uniform PointLightShadow pointLightShadows[ NUM_POINT_LIGHT_SHADOWS ];\n	#endif\n	#if defined( SHADOWMAP_TYPE_PCF )\n		float interleavedGradientNoise( vec2 position ) {\n			return fract( 52.9829189 * fract( dot( position, vec2( 0.06711056, 0.00583715 ) ) ) );\n		}\n		vec2 vogelDiskSample( int sampleIndex, int samplesCount, float phi ) {\n			const float goldenAngle = 2.399963229728653;\n			float r = sqrt( ( float( sampleIndex ) + 0.5 ) / float( samplesCount ) );\n			float theta = float( sampleIndex ) * goldenAngle + phi;\n			return vec2( cos( theta ), sin( theta ) ) * r;\n		}\n	#endif\n	#if defined( SHADOWMAP_TYPE_PCF )\n		float getShadow( sampler2DShadow shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {\n			float shadow = 1.0;\n			shadowCoord.xyz /= shadowCoord.w;\n			shadowCoord.z += shadowBias;\n			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;\n			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;\n			if ( frustumTest ) {\n				vec2 texelSize = vec2( 1.0 ) / shadowMapSize;\n				float radius = shadowRadius * texelSize.x;\n				float phi = interleavedGradientNoise( gl_FragCoord.xy ) * PI2;\n				shadow = (\n					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 0, 5, phi ) * radius, shadowCoord.z ) ) +\n					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 1, 5, phi ) * radius, shadowCoord.z ) ) +\n					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 2, 5, phi ) * radius, shadowCoord.z ) ) +\n					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 3, 5, phi ) * radius, shadowCoord.z ) ) +\n					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 4, 5, phi ) * radius, shadowCoord.z ) )\n				) * 0.2;\n			}\n			return mix( 1.0, shadow, shadowIntensity );\n		}\n	#elif defined( SHADOWMAP_TYPE_VSM )\n		float getShadow( sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {\n			float shadow = 1.0;\n			shadowCoord.xyz /= shadowCoord.w;\n			#ifdef USE_REVERSED_DEPTH_BUFFER\n				shadowCoord.z -= shadowBias;\n			#else\n				shadowCoord.z += shadowBias;\n			#endif\n			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;\n			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;\n			if ( frustumTest ) {\n				vec2 distribution = texture2D( shadowMap, shadowCoord.xy ).rg;\n				float mean = distribution.x;\n				float variance = distribution.y * distribution.y;\n				#ifdef USE_REVERSED_DEPTH_BUFFER\n					float hard_shadow = step( mean, shadowCoord.z );\n				#else\n					float hard_shadow = step( shadowCoord.z, mean );\n				#endif\n				\n				if ( hard_shadow == 1.0 ) {\n					shadow = 1.0;\n				} else {\n					variance = max( variance, 0.0000001 );\n					float d = shadowCoord.z - mean;\n					float p_max = variance / ( variance + d * d );\n					p_max = clamp( ( p_max - 0.3 ) / 0.65, 0.0, 1.0 );\n					shadow = max( hard_shadow, p_max );\n				}\n			}\n			return mix( 1.0, shadow, shadowIntensity );\n		}\n	#else\n		float getShadow( sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {\n			float shadow = 1.0;\n			shadowCoord.xyz /= shadowCoord.w;\n			#ifdef USE_REVERSED_DEPTH_BUFFER\n				shadowCoord.z -= shadowBias;\n			#else\n				shadowCoord.z += shadowBias;\n			#endif\n			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;\n			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;\n			if ( frustumTest ) {\n				float depth = texture2D( shadowMap, shadowCoord.xy ).r;\n				#ifdef USE_REVERSED_DEPTH_BUFFER\n					shadow = step( depth, shadowCoord.z );\n				#else\n					shadow = step( shadowCoord.z, depth );\n				#endif\n			}\n			return mix( 1.0, shadow, shadowIntensity );\n		}\n	#endif\n	#if NUM_POINT_LIGHT_SHADOWS > 0\n	#if defined( SHADOWMAP_TYPE_PCF )\n	float getPointShadow( samplerCubeShadow shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord, float shadowCameraNear, float shadowCameraFar ) {\n		float shadow = 1.0;\n		vec3 lightToPosition = shadowCoord.xyz;\n		vec3 bd3D = normalize( lightToPosition );\n		vec3 absVec = abs( lightToPosition );\n		float viewSpaceZ = max( max( absVec.x, absVec.y ), absVec.z );\n		if ( viewSpaceZ - shadowCameraFar <= 0.0 && viewSpaceZ - shadowCameraNear >= 0.0 ) {\n			#ifdef USE_REVERSED_DEPTH_BUFFER\n				float dp = ( shadowCameraNear * ( shadowCameraFar - viewSpaceZ ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );\n				dp -= shadowBias;\n			#else\n				float dp = ( shadowCameraFar * ( viewSpaceZ - shadowCameraNear ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );\n				dp += shadowBias;\n			#endif\n			float texelSize = shadowRadius / shadowMapSize.x;\n			vec3 absDir = abs( bd3D );\n			vec3 tangent = absDir.x > absDir.z ? vec3( 0.0, 1.0, 0.0 ) : vec3( 1.0, 0.0, 0.0 );\n			tangent = normalize( cross( bd3D, tangent ) );\n			vec3 bitangent = cross( bd3D, tangent );\n			float phi = interleavedGradientNoise( gl_FragCoord.xy ) * PI2;\n			vec2 sample0 = vogelDiskSample( 0, 5, phi );\n			vec2 sample1 = vogelDiskSample( 1, 5, phi );\n			vec2 sample2 = vogelDiskSample( 2, 5, phi );\n			vec2 sample3 = vogelDiskSample( 3, 5, phi );\n			vec2 sample4 = vogelDiskSample( 4, 5, phi );\n			shadow = (\n				texture( shadowMap, vec4( bd3D + ( tangent * sample0.x + bitangent * sample0.y ) * texelSize, dp ) ) +\n				texture( shadowMap, vec4( bd3D + ( tangent * sample1.x + bitangent * sample1.y ) * texelSize, dp ) ) +\n				texture( shadowMap, vec4( bd3D + ( tangent * sample2.x + bitangent * sample2.y ) * texelSize, dp ) ) +\n				texture( shadowMap, vec4( bd3D + ( tangent * sample3.x + bitangent * sample3.y ) * texelSize, dp ) ) +\n				texture( shadowMap, vec4( bd3D + ( tangent * sample4.x + bitangent * sample4.y ) * texelSize, dp ) )\n			) * 0.2;\n		}\n		return mix( 1.0, shadow, shadowIntensity );\n	}\n	#elif defined( SHADOWMAP_TYPE_BASIC )\n	float getPointShadow( samplerCube shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord, float shadowCameraNear, float shadowCameraFar ) {\n		float shadow = 1.0;\n		vec3 lightToPosition = shadowCoord.xyz;\n		vec3 absVec = abs( lightToPosition );\n		float viewSpaceZ = max( max( absVec.x, absVec.y ), absVec.z );\n		if ( viewSpaceZ - shadowCameraFar <= 0.0 && viewSpaceZ - shadowCameraNear >= 0.0 ) {\n			float dp = ( shadowCameraFar * ( viewSpaceZ - shadowCameraNear ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );\n			dp += shadowBias;\n			vec3 bd3D = normalize( lightToPosition );\n			float depth = textureCube( shadowMap, bd3D ).r;\n			#ifdef USE_REVERSED_DEPTH_BUFFER\n				depth = 1.0 - depth;\n			#endif\n			shadow = step( dp, depth );\n		}\n		return mix( 1.0, shadow, shadowIntensity );\n	}\n	#endif\n	#endif\n#endif",
	shadowmap_pars_vertex: "#if NUM_SPOT_LIGHT_COORDS > 0\n	uniform mat4 spotLightMatrix[ NUM_SPOT_LIGHT_COORDS ];\n	varying vec4 vSpotLightCoord[ NUM_SPOT_LIGHT_COORDS ];\n#endif\n#ifdef USE_SHADOWMAP\n	#if NUM_DIR_LIGHT_SHADOWS > 0\n		uniform mat4 directionalShadowMatrix[ NUM_DIR_LIGHT_SHADOWS ];\n		varying vec4 vDirectionalShadowCoord[ NUM_DIR_LIGHT_SHADOWS ];\n		struct DirectionalLightShadow {\n			float shadowIntensity;\n			float shadowBias;\n			float shadowNormalBias;\n			float shadowRadius;\n			vec2 shadowMapSize;\n		};\n		uniform DirectionalLightShadow directionalLightShadows[ NUM_DIR_LIGHT_SHADOWS ];\n	#endif\n	#if NUM_SPOT_LIGHT_SHADOWS > 0\n		struct SpotLightShadow {\n			float shadowIntensity;\n			float shadowBias;\n			float shadowNormalBias;\n			float shadowRadius;\n			vec2 shadowMapSize;\n		};\n		uniform SpotLightShadow spotLightShadows[ NUM_SPOT_LIGHT_SHADOWS ];\n	#endif\n	#if NUM_POINT_LIGHT_SHADOWS > 0\n		uniform mat4 pointShadowMatrix[ NUM_POINT_LIGHT_SHADOWS ];\n		varying vec4 vPointShadowCoord[ NUM_POINT_LIGHT_SHADOWS ];\n		struct PointLightShadow {\n			float shadowIntensity;\n			float shadowBias;\n			float shadowNormalBias;\n			float shadowRadius;\n			vec2 shadowMapSize;\n			float shadowCameraNear;\n			float shadowCameraFar;\n		};\n		uniform PointLightShadow pointLightShadows[ NUM_POINT_LIGHT_SHADOWS ];\n	#endif\n#endif",
	shadowmap_vertex: "#if ( defined( USE_SHADOWMAP ) && ( NUM_DIR_LIGHT_SHADOWS > 0 || NUM_POINT_LIGHT_SHADOWS > 0 ) ) || ( NUM_SPOT_LIGHT_COORDS > 0 )\n	#ifdef HAS_NORMAL\n		vec3 shadowWorldNormal = inverseTransformDirection( transformedNormal, viewMatrix );\n	#else\n		vec3 shadowWorldNormal = vec3( 0.0 );\n	#endif\n	vec4 shadowWorldPosition;\n#endif\n#if defined( USE_SHADOWMAP )\n	#if NUM_DIR_LIGHT_SHADOWS > 0\n		#pragma unroll_loop_start\n		for ( int i = 0; i < NUM_DIR_LIGHT_SHADOWS; i ++ ) {\n			shadowWorldPosition = worldPosition + vec4( shadowWorldNormal * directionalLightShadows[ i ].shadowNormalBias, 0 );\n			vDirectionalShadowCoord[ i ] = directionalShadowMatrix[ i ] * shadowWorldPosition;\n		}\n		#pragma unroll_loop_end\n	#endif\n	#if NUM_POINT_LIGHT_SHADOWS > 0\n		#pragma unroll_loop_start\n		for ( int i = 0; i < NUM_POINT_LIGHT_SHADOWS; i ++ ) {\n			shadowWorldPosition = worldPosition + vec4( shadowWorldNormal * pointLightShadows[ i ].shadowNormalBias, 0 );\n			vPointShadowCoord[ i ] = pointShadowMatrix[ i ] * shadowWorldPosition;\n		}\n		#pragma unroll_loop_end\n	#endif\n#endif\n#if NUM_SPOT_LIGHT_COORDS > 0\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_SPOT_LIGHT_COORDS; i ++ ) {\n		shadowWorldPosition = worldPosition;\n		#if ( defined( USE_SHADOWMAP ) && UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )\n			shadowWorldPosition.xyz += shadowWorldNormal * spotLightShadows[ i ].shadowNormalBias;\n		#endif\n		vSpotLightCoord[ i ] = spotLightMatrix[ i ] * shadowWorldPosition;\n	}\n	#pragma unroll_loop_end\n#endif",
	shadowmask_pars_fragment: "float getShadowMask() {\n	float shadow = 1.0;\n	#ifdef USE_SHADOWMAP\n	#if NUM_DIR_LIGHT_SHADOWS > 0\n	DirectionalLightShadow directionalLight;\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_DIR_LIGHT_SHADOWS; i ++ ) {\n		directionalLight = directionalLightShadows[ i ];\n		shadow *= receiveShadow ? getShadow( directionalShadowMap[ i ], directionalLight.shadowMapSize, directionalLight.shadowIntensity, directionalLight.shadowBias, directionalLight.shadowRadius, vDirectionalShadowCoord[ i ] ) : 1.0;\n	}\n	#pragma unroll_loop_end\n	#endif\n	#if NUM_SPOT_LIGHT_SHADOWS > 0\n	SpotLightShadow spotLight;\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_SPOT_LIGHT_SHADOWS; i ++ ) {\n		spotLight = spotLightShadows[ i ];\n		shadow *= receiveShadow ? getShadow( spotShadowMap[ i ], spotLight.shadowMapSize, spotLight.shadowIntensity, spotLight.shadowBias, spotLight.shadowRadius, vSpotLightCoord[ i ] ) : 1.0;\n	}\n	#pragma unroll_loop_end\n	#endif\n	#if NUM_POINT_LIGHT_SHADOWS > 0 && ( defined( SHADOWMAP_TYPE_PCF ) || defined( SHADOWMAP_TYPE_BASIC ) )\n	PointLightShadow pointLight;\n	#pragma unroll_loop_start\n	for ( int i = 0; i < NUM_POINT_LIGHT_SHADOWS; i ++ ) {\n		pointLight = pointLightShadows[ i ];\n		shadow *= receiveShadow ? getPointShadow( pointShadowMap[ i ], pointLight.shadowMapSize, pointLight.shadowIntensity, pointLight.shadowBias, pointLight.shadowRadius, vPointShadowCoord[ i ], pointLight.shadowCameraNear, pointLight.shadowCameraFar ) : 1.0;\n	}\n	#pragma unroll_loop_end\n	#endif\n	#endif\n	return shadow;\n}",
	skinbase_vertex: "#ifdef USE_SKINNING\n	mat4 boneMatX = getBoneMatrix( skinIndex.x );\n	mat4 boneMatY = getBoneMatrix( skinIndex.y );\n	mat4 boneMatZ = getBoneMatrix( skinIndex.z );\n	mat4 boneMatW = getBoneMatrix( skinIndex.w );\n#endif",
	skinning_pars_vertex: "#ifdef USE_SKINNING\n	uniform mat4 bindMatrix;\n	uniform mat4 bindMatrixInverse;\n	uniform highp sampler2D boneTexture;\n	mat4 getBoneMatrix( const in float i ) {\n		int size = textureSize( boneTexture, 0 ).x;\n		int j = int( i ) * 4;\n		int x = j % size;\n		int y = j / size;\n		vec4 v1 = texelFetch( boneTexture, ivec2( x, y ), 0 );\n		vec4 v2 = texelFetch( boneTexture, ivec2( x + 1, y ), 0 );\n		vec4 v3 = texelFetch( boneTexture, ivec2( x + 2, y ), 0 );\n		vec4 v4 = texelFetch( boneTexture, ivec2( x + 3, y ), 0 );\n		return mat4( v1, v2, v3, v4 );\n	}\n#endif",
	skinning_vertex: "#ifdef USE_SKINNING\n	vec4 skinVertex = bindMatrix * vec4( transformed, 1.0 );\n	vec4 skinned = vec4( 0.0 );\n	skinned += boneMatX * skinVertex * skinWeight.x;\n	skinned += boneMatY * skinVertex * skinWeight.y;\n	skinned += boneMatZ * skinVertex * skinWeight.z;\n	skinned += boneMatW * skinVertex * skinWeight.w;\n	transformed = ( bindMatrixInverse * skinned ).xyz;\n#endif",
	skinnormal_vertex: "#ifdef USE_SKINNING\n	mat4 skinMatrix = mat4( 0.0 );\n	skinMatrix += skinWeight.x * boneMatX;\n	skinMatrix += skinWeight.y * boneMatY;\n	skinMatrix += skinWeight.z * boneMatZ;\n	skinMatrix += skinWeight.w * boneMatW;\n	skinMatrix = bindMatrixInverse * skinMatrix * bindMatrix;\n	objectNormal = vec4( skinMatrix * vec4( objectNormal, 0.0 ) ).xyz;\n	#ifdef USE_TANGENT\n		objectTangent = vec4( skinMatrix * vec4( objectTangent, 0.0 ) ).xyz;\n	#endif\n#endif",
	specularmap_fragment: "float specularStrength;\n#ifdef USE_SPECULARMAP\n	vec4 texelSpecular = texture2D( specularMap, vSpecularMapUv );\n	specularStrength = texelSpecular.r;\n#else\n	specularStrength = 1.0;\n#endif",
	specularmap_pars_fragment: "#ifdef USE_SPECULARMAP\n	uniform sampler2D specularMap;\n#endif",
	tonemapping_fragment: "#if defined( TONE_MAPPING )\n	gl_FragColor.rgb = toneMapping( gl_FragColor.rgb );\n#endif",
	tonemapping_pars_fragment: "#ifndef saturate\n#define saturate( a ) clamp( a, 0.0, 1.0 )\n#endif\nuniform float toneMappingExposure;\nvec3 LinearToneMapping( vec3 color ) {\n	return saturate( toneMappingExposure * color );\n}\nvec3 ReinhardToneMapping( vec3 color ) {\n	color *= toneMappingExposure;\n	return saturate( color / ( vec3( 1.0 ) + color ) );\n}\nvec3 CineonToneMapping( vec3 color ) {\n	color *= toneMappingExposure;\n	color = max( vec3( 0.0 ), color - 0.004 );\n	return pow( ( color * ( 6.2 * color + 0.5 ) ) / ( color * ( 6.2 * color + 1.7 ) + 0.06 ), vec3( 2.2 ) );\n}\nvec3 RRTAndODTFit( vec3 v ) {\n	vec3 a = v * ( v + 0.0245786 ) - 0.000090537;\n	vec3 b = v * ( 0.983729 * v + 0.4329510 ) + 0.238081;\n	return a / b;\n}\nvec3 ACESFilmicToneMapping( vec3 color ) {\n	const mat3 ACESInputMat = mat3(\n		vec3( 0.59719, 0.07600, 0.02840 ),		vec3( 0.35458, 0.90834, 0.13383 ),\n		vec3( 0.04823, 0.01566, 0.83777 )\n	);\n	const mat3 ACESOutputMat = mat3(\n		vec3(  1.60475, -0.10208, -0.00327 ),		vec3( -0.53108,  1.10813, -0.07276 ),\n		vec3( -0.07367, -0.00605,  1.07602 )\n	);\n	color *= toneMappingExposure / 0.6;\n	color = ACESInputMat * color;\n	color = RRTAndODTFit( color );\n	color = ACESOutputMat * color;\n	return saturate( color );\n}\nconst mat3 LINEAR_REC2020_TO_LINEAR_SRGB = mat3(\n	vec3( 1.6605, - 0.1246, - 0.0182 ),\n	vec3( - 0.5876, 1.1329, - 0.1006 ),\n	vec3( - 0.0728, - 0.0083, 1.1187 )\n);\nconst mat3 LINEAR_SRGB_TO_LINEAR_REC2020 = mat3(\n	vec3( 0.6274, 0.0691, 0.0164 ),\n	vec3( 0.3293, 0.9195, 0.0880 ),\n	vec3( 0.0433, 0.0113, 0.8956 )\n);\nvec3 agxDefaultContrastApprox( vec3 x ) {\n	vec3 x2 = x * x;\n	vec3 x4 = x2 * x2;\n	return + 15.5 * x4 * x2\n		- 40.14 * x4 * x\n		+ 31.96 * x4\n		- 6.868 * x2 * x\n		+ 0.4298 * x2\n		+ 0.1191 * x\n		- 0.00232;\n}\nvec3 AgXToneMapping( vec3 color ) {\n	const mat3 AgXInsetMatrix = mat3(\n		vec3( 0.856627153315983, 0.137318972929847, 0.11189821299995 ),\n		vec3( 0.0951212405381588, 0.761241990602591, 0.0767994186031903 ),\n		vec3( 0.0482516061458583, 0.101439036467562, 0.811302368396859 )\n	);\n	const mat3 AgXOutsetMatrix = mat3(\n		vec3( 1.1271005818144368, - 0.1413297634984383, - 0.14132976349843826 ),\n		vec3( - 0.11060664309660323, 1.157823702216272, - 0.11060664309660294 ),\n		vec3( - 0.016493938717834573, - 0.016493938717834257, 1.2519364065950405 )\n	);\n	const float AgxMinEv = - 12.47393;	const float AgxMaxEv = 4.026069;\n	color *= toneMappingExposure;\n	color = LINEAR_SRGB_TO_LINEAR_REC2020 * color;\n	color = AgXInsetMatrix * color;\n	color = max( color, 1e-10 );	color = log2( color );\n	color = ( color - AgxMinEv ) / ( AgxMaxEv - AgxMinEv );\n	color = clamp( color, 0.0, 1.0 );\n	color = agxDefaultContrastApprox( color );\n	color = AgXOutsetMatrix * color;\n	color = pow( max( vec3( 0.0 ), color ), vec3( 2.2 ) );\n	color = LINEAR_REC2020_TO_LINEAR_SRGB * color;\n	color = clamp( color, 0.0, 1.0 );\n	return color;\n}\nvec3 NeutralToneMapping( vec3 color ) {\n	const float StartCompression = 0.8 - 0.04;\n	const float Desaturation = 0.15;\n	color *= toneMappingExposure;\n	float x = min( color.r, min( color.g, color.b ) );\n	float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;\n	color -= offset;\n	float peak = max( color.r, max( color.g, color.b ) );\n	if ( peak < StartCompression ) return color;\n	float d = 1. - StartCompression;\n	float newPeak = 1. - d * d / ( peak + d - StartCompression );\n	color *= newPeak / peak;\n	float g = 1. - 1. / ( Desaturation * ( peak - newPeak ) + 1. );\n	return mix( color, vec3( newPeak ), g );\n}\nvec3 CustomToneMapping( vec3 color ) { return color; }",
	transmission_fragment: "#ifdef USE_TRANSMISSION\n	material.transmission = transmission;\n	material.transmissionAlpha = 1.0;\n	material.thickness = thickness;\n	material.attenuationDistance = attenuationDistance;\n	material.attenuationColor = attenuationColor;\n	#ifdef USE_TRANSMISSIONMAP\n		material.transmission *= texture2D( transmissionMap, vTransmissionMapUv ).r;\n	#endif\n	#ifdef USE_THICKNESSMAP\n		material.thickness *= texture2D( thicknessMap, vThicknessMapUv ).g;\n	#endif\n	vec3 pos = vWorldPosition;\n	vec3 v = normalize( cameraPosition - pos );\n	vec3 n = inverseTransformDirection( normal, viewMatrix );\n	vec4 transmitted = getIBLVolumeRefraction(\n		n, v, material.roughness, material.diffuseContribution, material.specularColorBlended, material.specularF90,\n		pos, modelMatrix, viewMatrix, projectionMatrix, material.dispersion, material.ior, material.thickness,\n		material.attenuationColor, material.attenuationDistance );\n	material.transmissionAlpha = mix( material.transmissionAlpha, transmitted.a, material.transmission );\n	totalDiffuse = mix( totalDiffuse, transmitted.rgb, material.transmission );\n#endif",
	transmission_pars_fragment: "#ifdef USE_TRANSMISSION\n	uniform float transmission;\n	uniform float thickness;\n	uniform float attenuationDistance;\n	uniform vec3 attenuationColor;\n	#ifdef USE_TRANSMISSIONMAP\n		uniform sampler2D transmissionMap;\n	#endif\n	#ifdef USE_THICKNESSMAP\n		uniform sampler2D thicknessMap;\n	#endif\n	uniform vec2 transmissionSamplerSize;\n	uniform sampler2D transmissionSamplerMap;\n	uniform mat4 modelMatrix;\n	uniform mat4 projectionMatrix;\n	varying vec3 vWorldPosition;\n	float w0( float a ) {\n		return ( 1.0 / 6.0 ) * ( a * ( a * ( - a + 3.0 ) - 3.0 ) + 1.0 );\n	}\n	float w1( float a ) {\n		return ( 1.0 / 6.0 ) * ( a *  a * ( 3.0 * a - 6.0 ) + 4.0 );\n	}\n	float w2( float a ){\n		return ( 1.0 / 6.0 ) * ( a * ( a * ( - 3.0 * a + 3.0 ) + 3.0 ) + 1.0 );\n	}\n	float w3( float a ) {\n		return ( 1.0 / 6.0 ) * ( a * a * a );\n	}\n	float g0( float a ) {\n		return w0( a ) + w1( a );\n	}\n	float g1( float a ) {\n		return w2( a ) + w3( a );\n	}\n	float h0( float a ) {\n		return - 1.0 + w1( a ) / ( w0( a ) + w1( a ) );\n	}\n	float h1( float a ) {\n		return 1.0 + w3( a ) / ( w2( a ) + w3( a ) );\n	}\n	vec4 bicubic( sampler2D tex, vec2 uv, vec4 texelSize, float lod ) {\n		uv = uv * texelSize.zw + 0.5;\n		vec2 iuv = floor( uv );\n		vec2 fuv = fract( uv );\n		float g0x = g0( fuv.x );\n		float g1x = g1( fuv.x );\n		float h0x = h0( fuv.x );\n		float h1x = h1( fuv.x );\n		float h0y = h0( fuv.y );\n		float h1y = h1( fuv.y );\n		vec2 p0 = ( vec2( iuv.x + h0x, iuv.y + h0y ) - 0.5 ) * texelSize.xy;\n		vec2 p1 = ( vec2( iuv.x + h1x, iuv.y + h0y ) - 0.5 ) * texelSize.xy;\n		vec2 p2 = ( vec2( iuv.x + h0x, iuv.y + h1y ) - 0.5 ) * texelSize.xy;\n		vec2 p3 = ( vec2( iuv.x + h1x, iuv.y + h1y ) - 0.5 ) * texelSize.xy;\n		return g0( fuv.y ) * ( g0x * textureLod( tex, p0, lod ) + g1x * textureLod( tex, p1, lod ) ) +\n			g1( fuv.y ) * ( g0x * textureLod( tex, p2, lod ) + g1x * textureLod( tex, p3, lod ) );\n	}\n	vec4 textureBicubic( sampler2D sampler, vec2 uv, float lod ) {\n		vec2 fLodSize = vec2( textureSize( sampler, int( lod ) ) );\n		vec2 cLodSize = vec2( textureSize( sampler, int( lod + 1.0 ) ) );\n		vec2 fLodSizeInv = 1.0 / fLodSize;\n		vec2 cLodSizeInv = 1.0 / cLodSize;\n		vec4 fSample = bicubic( sampler, uv, vec4( fLodSizeInv, fLodSize ), floor( lod ) );\n		vec4 cSample = bicubic( sampler, uv, vec4( cLodSizeInv, cLodSize ), ceil( lod ) );\n		return mix( fSample, cSample, fract( lod ) );\n	}\n	vec3 getVolumeTransmissionRay( const in vec3 n, const in vec3 v, const in float thickness, const in float ior, const in mat4 modelMatrix ) {\n		vec3 refractionVector = refract( - v, normalize( n ), 1.0 / ior );\n		vec3 modelScale;\n		modelScale.x = length( vec3( modelMatrix[ 0 ].xyz ) );\n		modelScale.y = length( vec3( modelMatrix[ 1 ].xyz ) );\n		modelScale.z = length( vec3( modelMatrix[ 2 ].xyz ) );\n		return normalize( refractionVector ) * thickness * modelScale;\n	}\n	float applyIorToRoughness( const in float roughness, const in float ior ) {\n		return roughness * clamp( ior * 2.0 - 2.0, 0.0, 1.0 );\n	}\n	vec4 getTransmissionSample( const in vec2 fragCoord, const in float roughness, const in float ior ) {\n		float lod = log2( transmissionSamplerSize.x ) * applyIorToRoughness( roughness, ior );\n		return textureBicubic( transmissionSamplerMap, fragCoord.xy, lod );\n	}\n	vec3 volumeAttenuation( const in float transmissionDistance, const in vec3 attenuationColor, const in float attenuationDistance ) {\n		if ( isinf( attenuationDistance ) ) {\n			return vec3( 1.0 );\n		} else {\n			vec3 attenuationCoefficient = -log( attenuationColor ) / attenuationDistance;\n			vec3 transmittance = exp( - attenuationCoefficient * transmissionDistance );			return transmittance;\n		}\n	}\n	vec4 getIBLVolumeRefraction( const in vec3 n, const in vec3 v, const in float roughness, const in vec3 diffuseColor,\n		const in vec3 specularColor, const in float specularF90, const in vec3 position, const in mat4 modelMatrix,\n		const in mat4 viewMatrix, const in mat4 projMatrix, const in float dispersion, const in float ior, const in float thickness,\n		const in vec3 attenuationColor, const in float attenuationDistance ) {\n		vec4 transmittedLight;\n		vec3 transmittance;\n		#ifdef USE_DISPERSION\n			float halfSpread = ( ior - 1.0 ) * 0.025 * dispersion;\n			vec3 iors = vec3( ior - halfSpread, ior, ior + halfSpread );\n			for ( int i = 0; i < 3; i ++ ) {\n				vec3 transmissionRay = getVolumeTransmissionRay( n, v, thickness, iors[ i ], modelMatrix );\n				vec3 refractedRayExit = position + transmissionRay;\n				vec4 ndcPos = projMatrix * viewMatrix * vec4( refractedRayExit, 1.0 );\n				vec2 refractionCoords = ndcPos.xy / ndcPos.w;\n				refractionCoords += 1.0;\n				refractionCoords /= 2.0;\n				vec4 transmissionSample = getTransmissionSample( refractionCoords, roughness, iors[ i ] );\n				transmittedLight[ i ] = transmissionSample[ i ];\n				transmittedLight.a += transmissionSample.a;\n				transmittance[ i ] = diffuseColor[ i ] * volumeAttenuation( length( transmissionRay ), attenuationColor, attenuationDistance )[ i ];\n			}\n			transmittedLight.a /= 3.0;\n		#else\n			vec3 transmissionRay = getVolumeTransmissionRay( n, v, thickness, ior, modelMatrix );\n			vec3 refractedRayExit = position + transmissionRay;\n			vec4 ndcPos = projMatrix * viewMatrix * vec4( refractedRayExit, 1.0 );\n			vec2 refractionCoords = ndcPos.xy / ndcPos.w;\n			refractionCoords += 1.0;\n			refractionCoords /= 2.0;\n			transmittedLight = getTransmissionSample( refractionCoords, roughness, ior );\n			transmittance = diffuseColor * volumeAttenuation( length( transmissionRay ), attenuationColor, attenuationDistance );\n		#endif\n		vec3 attenuatedColor = transmittance * transmittedLight.rgb;\n		vec3 F = EnvironmentBRDF( n, v, specularColor, specularF90, roughness );\n		float transmittanceFactor = ( transmittance.r + transmittance.g + transmittance.b ) / 3.0;\n		return vec4( ( 1.0 - F ) * attenuatedColor, 1.0 - ( 1.0 - transmittedLight.a ) * transmittanceFactor );\n	}\n#endif",
	uv_pars_fragment: "#if defined( USE_UV ) || defined( USE_ANISOTROPY )\n	varying vec2 vUv;\n#endif\n#ifdef USE_MAP\n	varying vec2 vMapUv;\n#endif\n#ifdef USE_ALPHAMAP\n	varying vec2 vAlphaMapUv;\n#endif\n#ifdef USE_LIGHTMAP\n	varying vec2 vLightMapUv;\n#endif\n#ifdef USE_AOMAP\n	varying vec2 vAoMapUv;\n#endif\n#ifdef USE_BUMPMAP\n	varying vec2 vBumpMapUv;\n#endif\n#ifdef USE_NORMALMAP\n	varying vec2 vNormalMapUv;\n#endif\n#ifdef USE_EMISSIVEMAP\n	varying vec2 vEmissiveMapUv;\n#endif\n#ifdef USE_METALNESSMAP\n	varying vec2 vMetalnessMapUv;\n#endif\n#ifdef USE_ROUGHNESSMAP\n	varying vec2 vRoughnessMapUv;\n#endif\n#ifdef USE_ANISOTROPYMAP\n	varying vec2 vAnisotropyMapUv;\n#endif\n#ifdef USE_CLEARCOATMAP\n	varying vec2 vClearcoatMapUv;\n#endif\n#ifdef USE_CLEARCOAT_NORMALMAP\n	varying vec2 vClearcoatNormalMapUv;\n#endif\n#ifdef USE_CLEARCOAT_ROUGHNESSMAP\n	varying vec2 vClearcoatRoughnessMapUv;\n#endif\n#ifdef USE_IRIDESCENCEMAP\n	varying vec2 vIridescenceMapUv;\n#endif\n#ifdef USE_IRIDESCENCE_THICKNESSMAP\n	varying vec2 vIridescenceThicknessMapUv;\n#endif\n#ifdef USE_SHEEN_COLORMAP\n	varying vec2 vSheenColorMapUv;\n#endif\n#ifdef USE_SHEEN_ROUGHNESSMAP\n	varying vec2 vSheenRoughnessMapUv;\n#endif\n#ifdef USE_SPECULARMAP\n	varying vec2 vSpecularMapUv;\n#endif\n#ifdef USE_SPECULAR_COLORMAP\n	varying vec2 vSpecularColorMapUv;\n#endif\n#ifdef USE_SPECULAR_INTENSITYMAP\n	varying vec2 vSpecularIntensityMapUv;\n#endif\n#ifdef USE_TRANSMISSIONMAP\n	uniform mat3 transmissionMapTransform;\n	varying vec2 vTransmissionMapUv;\n#endif\n#ifdef USE_THICKNESSMAP\n	uniform mat3 thicknessMapTransform;\n	varying vec2 vThicknessMapUv;\n#endif",
	uv_pars_vertex: "#if defined( USE_UV ) || defined( USE_ANISOTROPY )\n	varying vec2 vUv;\n#endif\n#ifdef USE_MAP\n	uniform mat3 mapTransform;\n	varying vec2 vMapUv;\n#endif\n#ifdef USE_ALPHAMAP\n	uniform mat3 alphaMapTransform;\n	varying vec2 vAlphaMapUv;\n#endif\n#ifdef USE_LIGHTMAP\n	uniform mat3 lightMapTransform;\n	varying vec2 vLightMapUv;\n#endif\n#ifdef USE_AOMAP\n	uniform mat3 aoMapTransform;\n	varying vec2 vAoMapUv;\n#endif\n#ifdef USE_BUMPMAP\n	uniform mat3 bumpMapTransform;\n	varying vec2 vBumpMapUv;\n#endif\n#ifdef USE_NORMALMAP\n	uniform mat3 normalMapTransform;\n	varying vec2 vNormalMapUv;\n#endif\n#ifdef USE_DISPLACEMENTMAP\n	uniform mat3 displacementMapTransform;\n	varying vec2 vDisplacementMapUv;\n#endif\n#ifdef USE_EMISSIVEMAP\n	uniform mat3 emissiveMapTransform;\n	varying vec2 vEmissiveMapUv;\n#endif\n#ifdef USE_METALNESSMAP\n	uniform mat3 metalnessMapTransform;\n	varying vec2 vMetalnessMapUv;\n#endif\n#ifdef USE_ROUGHNESSMAP\n	uniform mat3 roughnessMapTransform;\n	varying vec2 vRoughnessMapUv;\n#endif\n#ifdef USE_ANISOTROPYMAP\n	uniform mat3 anisotropyMapTransform;\n	varying vec2 vAnisotropyMapUv;\n#endif\n#ifdef USE_CLEARCOATMAP\n	uniform mat3 clearcoatMapTransform;\n	varying vec2 vClearcoatMapUv;\n#endif\n#ifdef USE_CLEARCOAT_NORMALMAP\n	uniform mat3 clearcoatNormalMapTransform;\n	varying vec2 vClearcoatNormalMapUv;\n#endif\n#ifdef USE_CLEARCOAT_ROUGHNESSMAP\n	uniform mat3 clearcoatRoughnessMapTransform;\n	varying vec2 vClearcoatRoughnessMapUv;\n#endif\n#ifdef USE_SHEEN_COLORMAP\n	uniform mat3 sheenColorMapTransform;\n	varying vec2 vSheenColorMapUv;\n#endif\n#ifdef USE_SHEEN_ROUGHNESSMAP\n	uniform mat3 sheenRoughnessMapTransform;\n	varying vec2 vSheenRoughnessMapUv;\n#endif\n#ifdef USE_IRIDESCENCEMAP\n	uniform mat3 iridescenceMapTransform;\n	varying vec2 vIridescenceMapUv;\n#endif\n#ifdef USE_IRIDESCENCE_THICKNESSMAP\n	uniform mat3 iridescenceThicknessMapTransform;\n	varying vec2 vIridescenceThicknessMapUv;\n#endif\n#ifdef USE_SPECULARMAP\n	uniform mat3 specularMapTransform;\n	varying vec2 vSpecularMapUv;\n#endif\n#ifdef USE_SPECULAR_COLORMAP\n	uniform mat3 specularColorMapTransform;\n	varying vec2 vSpecularColorMapUv;\n#endif\n#ifdef USE_SPECULAR_INTENSITYMAP\n	uniform mat3 specularIntensityMapTransform;\n	varying vec2 vSpecularIntensityMapUv;\n#endif\n#ifdef USE_TRANSMISSIONMAP\n	uniform mat3 transmissionMapTransform;\n	varying vec2 vTransmissionMapUv;\n#endif\n#ifdef USE_THICKNESSMAP\n	uniform mat3 thicknessMapTransform;\n	varying vec2 vThicknessMapUv;\n#endif",
	uv_vertex: "#if defined( USE_UV ) || defined( USE_ANISOTROPY )\n	vUv = vec3( uv, 1 ).xy;\n#endif\n#ifdef USE_MAP\n	vMapUv = ( mapTransform * vec3( MAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_ALPHAMAP\n	vAlphaMapUv = ( alphaMapTransform * vec3( ALPHAMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_LIGHTMAP\n	vLightMapUv = ( lightMapTransform * vec3( LIGHTMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_AOMAP\n	vAoMapUv = ( aoMapTransform * vec3( AOMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_BUMPMAP\n	vBumpMapUv = ( bumpMapTransform * vec3( BUMPMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_NORMALMAP\n	vNormalMapUv = ( normalMapTransform * vec3( NORMALMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_DISPLACEMENTMAP\n	vDisplacementMapUv = ( displacementMapTransform * vec3( DISPLACEMENTMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_EMISSIVEMAP\n	vEmissiveMapUv = ( emissiveMapTransform * vec3( EMISSIVEMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_METALNESSMAP\n	vMetalnessMapUv = ( metalnessMapTransform * vec3( METALNESSMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_ROUGHNESSMAP\n	vRoughnessMapUv = ( roughnessMapTransform * vec3( ROUGHNESSMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_ANISOTROPYMAP\n	vAnisotropyMapUv = ( anisotropyMapTransform * vec3( ANISOTROPYMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_CLEARCOATMAP\n	vClearcoatMapUv = ( clearcoatMapTransform * vec3( CLEARCOATMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_CLEARCOAT_NORMALMAP\n	vClearcoatNormalMapUv = ( clearcoatNormalMapTransform * vec3( CLEARCOAT_NORMALMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_CLEARCOAT_ROUGHNESSMAP\n	vClearcoatRoughnessMapUv = ( clearcoatRoughnessMapTransform * vec3( CLEARCOAT_ROUGHNESSMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_IRIDESCENCEMAP\n	vIridescenceMapUv = ( iridescenceMapTransform * vec3( IRIDESCENCEMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_IRIDESCENCE_THICKNESSMAP\n	vIridescenceThicknessMapUv = ( iridescenceThicknessMapTransform * vec3( IRIDESCENCE_THICKNESSMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_SHEEN_COLORMAP\n	vSheenColorMapUv = ( sheenColorMapTransform * vec3( SHEEN_COLORMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_SHEEN_ROUGHNESSMAP\n	vSheenRoughnessMapUv = ( sheenRoughnessMapTransform * vec3( SHEEN_ROUGHNESSMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_SPECULARMAP\n	vSpecularMapUv = ( specularMapTransform * vec3( SPECULARMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_SPECULAR_COLORMAP\n	vSpecularColorMapUv = ( specularColorMapTransform * vec3( SPECULAR_COLORMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_SPECULAR_INTENSITYMAP\n	vSpecularIntensityMapUv = ( specularIntensityMapTransform * vec3( SPECULAR_INTENSITYMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_TRANSMISSIONMAP\n	vTransmissionMapUv = ( transmissionMapTransform * vec3( TRANSMISSIONMAP_UV, 1 ) ).xy;\n#endif\n#ifdef USE_THICKNESSMAP\n	vThicknessMapUv = ( thicknessMapTransform * vec3( THICKNESSMAP_UV, 1 ) ).xy;\n#endif",
	worldpos_vertex: "#if defined( USE_ENVMAP ) || defined( DISTANCE ) || defined ( USE_SHADOWMAP ) || defined ( USE_TRANSMISSION ) || NUM_SPOT_LIGHT_COORDS > 0\n	vec4 worldPosition = vec4( transformed, 1.0 );\n	#ifdef USE_BATCHING\n		worldPosition = batchingMatrix * worldPosition;\n	#endif\n	#ifdef USE_INSTANCING\n		worldPosition = instanceMatrix * worldPosition;\n	#endif\n	worldPosition = modelMatrix * worldPosition;\n#endif",
	background_vert: "varying vec2 vUv;\nuniform mat3 uvTransform;\nvoid main() {\n	vUv = ( uvTransform * vec3( uv, 1 ) ).xy;\n	gl_Position = vec4( position.xy, 1.0, 1.0 );\n}",
	background_frag: "uniform sampler2D t2D;\nuniform float backgroundIntensity;\nvarying vec2 vUv;\nvoid main() {\n	vec4 texColor = texture2D( t2D, vUv );\n	#ifdef DECODE_VIDEO_TEXTURE\n		texColor = vec4( mix( pow( texColor.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), texColor.rgb * 0.0773993808, vec3( lessThanEqual( texColor.rgb, vec3( 0.04045 ) ) ) ), texColor.w );\n	#endif\n	texColor.rgb *= backgroundIntensity;\n	gl_FragColor = texColor;\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n}",
	backgroundCube_vert: "varying vec3 vWorldDirection;\n#include <common>\nvoid main() {\n	vWorldDirection = transformDirection( position, modelMatrix );\n	#include <begin_vertex>\n	#include <project_vertex>\n	gl_Position.z = gl_Position.w;\n}",
	backgroundCube_frag: "#ifdef ENVMAP_TYPE_CUBE\n	uniform samplerCube envMap;\n#elif defined( ENVMAP_TYPE_CUBE_UV )\n	uniform sampler2D envMap;\n#endif\nuniform float backgroundBlurriness;\nuniform float backgroundIntensity;\nuniform mat3 backgroundRotation;\nvarying vec3 vWorldDirection;\n#include <cube_uv_reflection_fragment>\nvoid main() {\n	#ifdef ENVMAP_TYPE_CUBE\n		vec4 texColor = textureCube( envMap, backgroundRotation * vWorldDirection );\n	#elif defined( ENVMAP_TYPE_CUBE_UV )\n		vec4 texColor = textureCubeUV( envMap, backgroundRotation * vWorldDirection, backgroundBlurriness );\n	#else\n		vec4 texColor = vec4( 0.0, 0.0, 0.0, 1.0 );\n	#endif\n	texColor.rgb *= backgroundIntensity;\n	gl_FragColor = texColor;\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n}",
	cube_vert: "varying vec3 vWorldDirection;\n#include <common>\nvoid main() {\n	vWorldDirection = transformDirection( position, modelMatrix );\n	#include <begin_vertex>\n	#include <project_vertex>\n	gl_Position.z = gl_Position.w;\n}",
	cube_frag: "uniform samplerCube tCube;\nuniform float tFlip;\nuniform float opacity;\nvarying vec3 vWorldDirection;\nvoid main() {\n	vec4 texColor = textureCube( tCube, vec3( tFlip * vWorldDirection.x, vWorldDirection.yz ) );\n	gl_FragColor = texColor;\n	gl_FragColor.a *= opacity;\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n}",
	depth_vert: "#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvarying vec2 vHighPrecisionZW;\nvoid main() {\n	#include <uv_vertex>\n	#include <batching_vertex>\n	#include <skinbase_vertex>\n	#include <morphinstance_vertex>\n	#ifdef USE_DISPLACEMENTMAP\n		#include <beginnormal_vertex>\n		#include <morphnormal_vertex>\n		#include <skinnormal_vertex>\n	#endif\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	vHighPrecisionZW = gl_Position.zw;\n}",
	depth_frag: "#if DEPTH_PACKING == 3200\n	uniform float opacity;\n#endif\n#include <common>\n#include <packing>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvarying vec2 vHighPrecisionZW;\nvoid main() {\n	vec4 diffuseColor = vec4( 1.0 );\n	#include <clipping_planes_fragment>\n	#if DEPTH_PACKING == 3200\n		diffuseColor.a = opacity;\n	#endif\n	#include <map_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <logdepthbuf_fragment>\n	#ifdef USE_REVERSED_DEPTH_BUFFER\n		float fragCoordZ = vHighPrecisionZW[ 0 ] / vHighPrecisionZW[ 1 ];\n	#else\n		float fragCoordZ = 0.5 * vHighPrecisionZW[ 0 ] / vHighPrecisionZW[ 1 ] + 0.5;\n	#endif\n	#if DEPTH_PACKING == 3200\n		gl_FragColor = vec4( vec3( 1.0 - fragCoordZ ), opacity );\n	#elif DEPTH_PACKING == 3201\n		gl_FragColor = packDepthToRGBA( fragCoordZ );\n	#elif DEPTH_PACKING == 3202\n		gl_FragColor = vec4( packDepthToRGB( fragCoordZ ), 1.0 );\n	#elif DEPTH_PACKING == 3203\n		gl_FragColor = vec4( packDepthToRG( fragCoordZ ), 0.0, 1.0 );\n	#endif\n}",
	distance_vert: "#define DISTANCE\nvarying vec3 vWorldPosition;\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <batching_vertex>\n	#include <skinbase_vertex>\n	#include <morphinstance_vertex>\n	#ifdef USE_DISPLACEMENTMAP\n		#include <beginnormal_vertex>\n		#include <morphnormal_vertex>\n		#include <skinnormal_vertex>\n	#endif\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <worldpos_vertex>\n	#include <clipping_planes_vertex>\n	vWorldPosition = worldPosition.xyz;\n}",
	distance_frag: "#define DISTANCE\nuniform vec3 referencePosition;\nuniform float nearDistance;\nuniform float farDistance;\nvarying vec3 vWorldPosition;\n#include <common>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main () {\n	vec4 diffuseColor = vec4( 1.0 );\n	#include <clipping_planes_fragment>\n	#include <map_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	float dist = length( vWorldPosition - referencePosition );\n	dist = ( dist - nearDistance ) / ( farDistance - nearDistance );\n	dist = saturate( dist );\n	gl_FragColor = vec4( dist, 0.0, 0.0, 1.0 );\n}",
	equirect_vert: "varying vec3 vWorldDirection;\n#include <common>\nvoid main() {\n	vWorldDirection = transformDirection( position, modelMatrix );\n	#include <begin_vertex>\n	#include <project_vertex>\n}",
	equirect_frag: "uniform sampler2D tEquirect;\nvarying vec3 vWorldDirection;\n#include <common>\nvoid main() {\n	vec3 direction = normalize( vWorldDirection );\n	vec2 sampleUV = equirectUv( direction );\n	gl_FragColor = texture2D( tEquirect, sampleUV );\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n}",
	linedashed_vert: "uniform float scale;\nattribute float lineDistance;\nvarying float vLineDistance;\n#include <common>\n#include <uv_pars_vertex>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	vLineDistance = scale * lineDistance;\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	#include <fog_vertex>\n}",
	linedashed_frag: "uniform vec3 diffuse;\nuniform float opacity;\nuniform float dashSize;\nuniform float totalSize;\nvarying float vLineDistance;\n#include <common>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <fog_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	if ( mod( vLineDistance, totalSize ) > dashSize ) {\n		discard;\n	}\n	vec3 outgoingLight = vec3( 0.0 );\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	outgoingLight = diffuseColor.rgb;\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n}",
	meshbasic_vert: "#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <envmap_pars_vertex>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <batching_vertex>\n	#if defined ( USE_ENVMAP ) || defined ( USE_SKINNING )\n		#include <beginnormal_vertex>\n		#include <morphnormal_vertex>\n		#include <skinbase_vertex>\n		#include <skinnormal_vertex>\n		#include <defaultnormal_vertex>\n	#endif\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	#include <worldpos_vertex>\n	#include <envmap_vertex>\n	#include <fog_vertex>\n}",
	meshbasic_frag: "uniform vec3 diffuse;\nuniform float opacity;\n#ifndef FLAT_SHADED\n	varying vec3 vNormal;\n#endif\n#include <common>\n#include <dithering_pars_fragment>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <aomap_pars_fragment>\n#include <lightmap_pars_fragment>\n#include <envmap_common_pars_fragment>\n#include <envmap_pars_fragment>\n#include <fog_pars_fragment>\n#include <specularmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <specularmap_fragment>\n	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );\n	#ifdef USE_LIGHTMAP\n		vec4 lightMapTexel = texture2D( lightMap, vLightMapUv );\n		reflectedLight.indirectDiffuse += lightMapTexel.rgb * lightMapIntensity * RECIPROCAL_PI;\n	#else\n		reflectedLight.indirectDiffuse += vec3( 1.0 );\n	#endif\n	#include <aomap_fragment>\n	reflectedLight.indirectDiffuse *= diffuseColor.rgb;\n	vec3 outgoingLight = reflectedLight.indirectDiffuse;\n	#include <envmap_fragment>\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n	#include <dithering_fragment>\n}",
	meshlambert_vert: "#define LAMBERT\nvarying vec3 vViewPosition;\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <envmap_pars_vertex>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <normal_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <shadowmap_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <normal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	vViewPosition = - mvPosition.xyz;\n	#include <worldpos_vertex>\n	#include <envmap_vertex>\n	#include <shadowmap_vertex>\n	#include <fog_vertex>\n}",
	meshlambert_frag: "#define LAMBERT\nuniform vec3 diffuse;\nuniform vec3 emissive;\nuniform float opacity;\n#include <common>\n#include <dithering_pars_fragment>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <aomap_pars_fragment>\n#include <lightmap_pars_fragment>\n#include <emissivemap_pars_fragment>\n#include <cube_uv_reflection_fragment>\n#include <envmap_common_pars_fragment>\n#include <envmap_pars_fragment>\n#include <envmap_physical_pars_fragment>\n#include <fog_pars_fragment>\n#include <bsdfs>\n#include <lights_pars_begin>\n#include <normal_pars_fragment>\n#include <lights_lambert_pars_fragment>\n#include <shadowmap_pars_fragment>\n#include <bumpmap_pars_fragment>\n#include <normalmap_pars_fragment>\n#include <specularmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );\n	vec3 totalEmissiveRadiance = emissive;\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <specularmap_fragment>\n	#include <normal_fragment_begin>\n	#include <normal_fragment_maps>\n	#include <emissivemap_fragment>\n	#include <lights_lambert_fragment>\n	#include <lights_fragment_begin>\n	#include <lights_fragment_maps>\n	#include <lights_fragment_end>\n	#include <aomap_fragment>\n	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + totalEmissiveRadiance;\n	#include <envmap_fragment>\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n	#include <dithering_fragment>\n}",
	meshmatcap_vert: "#define MATCAP\nvarying vec3 vViewPosition;\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <color_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <fog_pars_vertex>\n#include <normal_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <normal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	#include <fog_vertex>\n	vViewPosition = - mvPosition.xyz;\n}",
	meshmatcap_frag: "#define MATCAP\nuniform vec3 diffuse;\nuniform float opacity;\nuniform sampler2D matcap;\nvarying vec3 vViewPosition;\n#include <common>\n#include <dithering_pars_fragment>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <fog_pars_fragment>\n#include <normal_pars_fragment>\n#include <bumpmap_pars_fragment>\n#include <normalmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <normal_fragment_begin>\n	#include <normal_fragment_maps>\n	vec3 viewDir = normalize( vViewPosition );\n	vec3 x = normalize( vec3( viewDir.z, 0.0, - viewDir.x ) );\n	vec3 y = cross( viewDir, x );\n	vec2 uv = vec2( dot( x, normal ), dot( y, normal ) ) * 0.495 + 0.5;\n	#ifdef USE_MATCAP\n		vec4 matcapColor = texture2D( matcap, uv );\n	#else\n		vec4 matcapColor = vec4( vec3( mix( 0.2, 0.8, uv.y ) ), 1.0 );\n	#endif\n	vec3 outgoingLight = diffuseColor.rgb * matcapColor.rgb;\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n	#include <dithering_fragment>\n}",
	meshnormal_vert: "#define NORMAL\n#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )\n	varying vec3 vViewPosition;\n#endif\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <normal_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphinstance_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <normal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )\n	vViewPosition = - mvPosition.xyz;\n#endif\n}",
	meshnormal_frag: "#define NORMAL\nuniform float opacity;\n#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )\n	varying vec3 vViewPosition;\n#endif\n#include <uv_pars_fragment>\n#include <normal_pars_fragment>\n#include <bumpmap_pars_fragment>\n#include <normalmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( 0.0, 0.0, 0.0, opacity );\n	#include <clipping_planes_fragment>\n	#include <logdepthbuf_fragment>\n	#include <normal_fragment_begin>\n	#include <normal_fragment_maps>\n	gl_FragColor = vec4( normalize( normal ) * 0.5 + 0.5, diffuseColor.a );\n	#ifdef OPAQUE\n		gl_FragColor.a = 1.0;\n	#endif\n}",
	meshphong_vert: "#define PHONG\nvarying vec3 vViewPosition;\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <envmap_pars_vertex>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <normal_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <shadowmap_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphcolor_vertex>\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphinstance_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <normal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	vViewPosition = - mvPosition.xyz;\n	#include <worldpos_vertex>\n	#include <envmap_vertex>\n	#include <shadowmap_vertex>\n	#include <fog_vertex>\n}",
	meshphong_frag: "#define PHONG\nuniform vec3 diffuse;\nuniform vec3 emissive;\nuniform vec3 specular;\nuniform float shininess;\nuniform float opacity;\n#include <common>\n#include <dithering_pars_fragment>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <aomap_pars_fragment>\n#include <lightmap_pars_fragment>\n#include <emissivemap_pars_fragment>\n#include <cube_uv_reflection_fragment>\n#include <envmap_common_pars_fragment>\n#include <envmap_pars_fragment>\n#include <envmap_physical_pars_fragment>\n#include <fog_pars_fragment>\n#include <bsdfs>\n#include <lights_pars_begin>\n#include <normal_pars_fragment>\n#include <lights_phong_pars_fragment>\n#include <shadowmap_pars_fragment>\n#include <bumpmap_pars_fragment>\n#include <normalmap_pars_fragment>\n#include <specularmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );\n	vec3 totalEmissiveRadiance = emissive;\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <specularmap_fragment>\n	#include <normal_fragment_begin>\n	#include <normal_fragment_maps>\n	#include <emissivemap_fragment>\n	#include <lights_phong_fragment>\n	#include <lights_fragment_begin>\n	#include <lights_fragment_maps>\n	#include <lights_fragment_end>\n	#include <aomap_fragment>\n	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + reflectedLight.directSpecular + reflectedLight.indirectSpecular + totalEmissiveRadiance;\n	#include <envmap_fragment>\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n	#include <dithering_fragment>\n}",
	meshphysical_vert: "#define STANDARD\nvarying vec3 vViewPosition;\n#ifdef USE_TRANSMISSION\n	varying vec3 vWorldPosition;\n#endif\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <normal_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <shadowmap_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <normal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	vViewPosition = - mvPosition.xyz;\n	#include <worldpos_vertex>\n	#include <shadowmap_vertex>\n	#include <fog_vertex>\n#ifdef USE_TRANSMISSION\n	vWorldPosition = worldPosition.xyz;\n#endif\n}",
	meshphysical_frag: "#define STANDARD\n#ifdef PHYSICAL\n	#define IOR\n	#define USE_SPECULAR\n#endif\nuniform vec3 diffuse;\nuniform vec3 emissive;\nuniform float roughness;\nuniform float metalness;\nuniform float opacity;\n#ifdef IOR\n	uniform float ior;\n#endif\n#ifdef USE_SPECULAR\n	uniform float specularIntensity;\n	uniform vec3 specularColor;\n	#ifdef USE_SPECULAR_COLORMAP\n		uniform sampler2D specularColorMap;\n	#endif\n	#ifdef USE_SPECULAR_INTENSITYMAP\n		uniform sampler2D specularIntensityMap;\n	#endif\n#endif\n#ifdef USE_CLEARCOAT\n	uniform float clearcoat;\n	uniform float clearcoatRoughness;\n#endif\n#ifdef USE_DISPERSION\n	uniform float dispersion;\n#endif\n#ifdef USE_IRIDESCENCE\n	uniform float iridescence;\n	uniform float iridescenceIOR;\n	uniform float iridescenceThicknessMinimum;\n	uniform float iridescenceThicknessMaximum;\n#endif\n#ifdef USE_SHEEN\n	uniform vec3 sheenColor;\n	uniform float sheenRoughness;\n	#ifdef USE_SHEEN_COLORMAP\n		uniform sampler2D sheenColorMap;\n	#endif\n	#ifdef USE_SHEEN_ROUGHNESSMAP\n		uniform sampler2D sheenRoughnessMap;\n	#endif\n#endif\n#ifdef USE_ANISOTROPY\n	uniform vec2 anisotropyVector;\n	#ifdef USE_ANISOTROPYMAP\n		uniform sampler2D anisotropyMap;\n	#endif\n#endif\nvarying vec3 vViewPosition;\n#include <common>\n#include <dithering_pars_fragment>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <aomap_pars_fragment>\n#include <lightmap_pars_fragment>\n#include <emissivemap_pars_fragment>\n#include <iridescence_fragment>\n#include <cube_uv_reflection_fragment>\n#include <envmap_common_pars_fragment>\n#include <envmap_physical_pars_fragment>\n#include <fog_pars_fragment>\n#include <lights_pars_begin>\n#include <normal_pars_fragment>\n#include <lights_physical_pars_fragment>\n#include <transmission_pars_fragment>\n#include <shadowmap_pars_fragment>\n#include <bumpmap_pars_fragment>\n#include <normalmap_pars_fragment>\n#include <clearcoat_pars_fragment>\n#include <iridescence_pars_fragment>\n#include <roughnessmap_pars_fragment>\n#include <metalnessmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );\n	vec3 totalEmissiveRadiance = emissive;\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <roughnessmap_fragment>\n	#include <metalnessmap_fragment>\n	#include <normal_fragment_begin>\n	#include <normal_fragment_maps>\n	#include <clearcoat_normal_fragment_begin>\n	#include <clearcoat_normal_fragment_maps>\n	#include <emissivemap_fragment>\n	#include <lights_physical_fragment>\n	#include <lights_fragment_begin>\n	#include <lights_fragment_maps>\n	#include <lights_fragment_end>\n	#include <aomap_fragment>\n	vec3 totalDiffuse = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse;\n	vec3 totalSpecular = reflectedLight.directSpecular + reflectedLight.indirectSpecular;\n	#include <transmission_fragment>\n	vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;\n	#ifdef USE_SHEEN\n \n		outgoingLight = outgoingLight + sheenSpecularDirect + sheenSpecularIndirect;\n \n 	#endif\n	#ifdef USE_CLEARCOAT\n		float dotNVcc = saturate( dot( geometryClearcoatNormal, geometryViewDir ) );\n		vec3 Fcc = F_Schlick( material.clearcoatF0, material.clearcoatF90, dotNVcc );\n		outgoingLight = outgoingLight * ( 1.0 - material.clearcoat * Fcc ) + ( clearcoatSpecularDirect + clearcoatSpecularIndirect ) * material.clearcoat;\n	#endif\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n	#include <dithering_fragment>\n}",
	meshtoon_vert: "#define TOON\nvarying vec3 vViewPosition;\n#include <common>\n#include <batching_pars_vertex>\n#include <uv_pars_vertex>\n#include <displacementmap_pars_vertex>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <normal_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <shadowmap_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <normal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <displacementmap_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	vViewPosition = - mvPosition.xyz;\n	#include <worldpos_vertex>\n	#include <shadowmap_vertex>\n	#include <fog_vertex>\n}",
	meshtoon_frag: "#define TOON\nuniform vec3 diffuse;\nuniform vec3 emissive;\nuniform float opacity;\n#include <common>\n#include <dithering_pars_fragment>\n#include <color_pars_fragment>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <aomap_pars_fragment>\n#include <lightmap_pars_fragment>\n#include <emissivemap_pars_fragment>\n#include <gradientmap_pars_fragment>\n#include <fog_pars_fragment>\n#include <bsdfs>\n#include <lights_pars_begin>\n#include <normal_pars_fragment>\n#include <lights_toon_pars_fragment>\n#include <shadowmap_pars_fragment>\n#include <bumpmap_pars_fragment>\n#include <normalmap_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );\n	vec3 totalEmissiveRadiance = emissive;\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <color_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	#include <normal_fragment_begin>\n	#include <normal_fragment_maps>\n	#include <emissivemap_fragment>\n	#include <lights_toon_fragment>\n	#include <lights_fragment_begin>\n	#include <lights_fragment_maps>\n	#include <lights_fragment_end>\n	#include <aomap_fragment>\n	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + totalEmissiveRadiance;\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n	#include <dithering_fragment>\n}",
	points_vert: "uniform float size;\nuniform float scale;\n#include <common>\n#include <color_pars_vertex>\n#include <fog_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\n#ifdef USE_POINTS_UV\n	varying vec2 vUv;\n	uniform mat3 uvTransform;\n#endif\nvoid main() {\n	#ifdef USE_POINTS_UV\n		vUv = ( uvTransform * vec3( uv, 1 ) ).xy;\n	#endif\n	#include <color_vertex>\n	#include <morphinstance_vertex>\n	#include <morphcolor_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <project_vertex>\n	gl_PointSize = size;\n	#ifdef USE_SIZEATTENUATION\n		bool isPerspective = isPerspectiveMatrix( projectionMatrix );\n		if ( isPerspective ) gl_PointSize *= ( scale / - mvPosition.z );\n	#endif\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	#include <worldpos_vertex>\n	#include <fog_vertex>\n}",
	points_frag: "uniform vec3 diffuse;\nuniform float opacity;\n#include <common>\n#include <color_pars_fragment>\n#include <map_particle_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <fog_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	vec3 outgoingLight = vec3( 0.0 );\n	#include <logdepthbuf_fragment>\n	#include <map_particle_fragment>\n	#include <color_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	outgoingLight = diffuseColor.rgb;\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n}",
	shadow_vert: "#include <common>\n#include <batching_pars_vertex>\n#include <fog_pars_vertex>\n#include <morphtarget_pars_vertex>\n#include <skinning_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <shadowmap_pars_vertex>\nvoid main() {\n	#include <batching_vertex>\n	#include <beginnormal_vertex>\n	#include <morphinstance_vertex>\n	#include <morphnormal_vertex>\n	#include <skinbase_vertex>\n	#include <skinnormal_vertex>\n	#include <defaultnormal_vertex>\n	#include <begin_vertex>\n	#include <morphtarget_vertex>\n	#include <skinning_vertex>\n	#include <project_vertex>\n	#include <logdepthbuf_vertex>\n	#include <worldpos_vertex>\n	#include <shadowmap_vertex>\n	#include <fog_vertex>\n}",
	shadow_frag: "uniform vec3 color;\nuniform float opacity;\n#include <common>\n#include <fog_pars_fragment>\n#include <bsdfs>\n#include <lights_pars_begin>\n#include <logdepthbuf_pars_fragment>\n#include <shadowmap_pars_fragment>\n#include <shadowmask_pars_fragment>\nvoid main() {\n	#include <logdepthbuf_fragment>\n	gl_FragColor = vec4( color, opacity * ( 1.0 - getShadowMask() ) );\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n	#include <premultiplied_alpha_fragment>\n}",
	sprite_vert: "uniform float rotation;\nuniform vec2 center;\n#include <common>\n#include <uv_pars_vertex>\n#include <fog_pars_vertex>\n#include <logdepthbuf_pars_vertex>\n#include <clipping_planes_pars_vertex>\nvoid main() {\n	#include <uv_vertex>\n	vec4 mvPosition = modelViewMatrix[ 3 ];\n	vec2 scale = vec2( length( modelMatrix[ 0 ].xyz ), length( modelMatrix[ 1 ].xyz ) );\n	#ifndef USE_SIZEATTENUATION\n		bool isPerspective = isPerspectiveMatrix( projectionMatrix );\n		if ( isPerspective ) scale *= - mvPosition.z;\n	#endif\n	vec2 alignedPosition = ( position.xy - ( center - vec2( 0.5 ) ) ) * scale;\n	vec2 rotatedPosition;\n	rotatedPosition.x = cos( rotation ) * alignedPosition.x - sin( rotation ) * alignedPosition.y;\n	rotatedPosition.y = sin( rotation ) * alignedPosition.x + cos( rotation ) * alignedPosition.y;\n	mvPosition.xy += rotatedPosition;\n	gl_Position = projectionMatrix * mvPosition;\n	#include <logdepthbuf_vertex>\n	#include <clipping_planes_vertex>\n	#include <fog_vertex>\n}",
	sprite_frag: "uniform vec3 diffuse;\nuniform float opacity;\n#include <common>\n#include <uv_pars_fragment>\n#include <map_pars_fragment>\n#include <alphamap_pars_fragment>\n#include <alphatest_pars_fragment>\n#include <alphahash_pars_fragment>\n#include <fog_pars_fragment>\n#include <logdepthbuf_pars_fragment>\n#include <clipping_planes_pars_fragment>\nvoid main() {\n	vec4 diffuseColor = vec4( diffuse, opacity );\n	#include <clipping_planes_fragment>\n	vec3 outgoingLight = vec3( 0.0 );\n	#include <logdepthbuf_fragment>\n	#include <map_fragment>\n	#include <alphamap_fragment>\n	#include <alphatest_fragment>\n	#include <alphahash_fragment>\n	outgoingLight = diffuseColor.rgb;\n	#include <opaque_fragment>\n	#include <tonemapping_fragment>\n	#include <colorspace_fragment>\n	#include <fog_fragment>\n}"
}, Q = {
	common: {
		diffuse: { value: /*@__PURE__*/ new X(16777215) },
		opacity: { value: 1 },
		map: { value: null },
		mapTransform: { value: /*@__PURE__*/ new J() },
		alphaMap: { value: null },
		alphaMapTransform: { value: /*@__PURE__*/ new J() },
		alphaTest: { value: 0 }
	},
	specularmap: {
		specularMap: { value: null },
		specularMapTransform: { value: /*@__PURE__*/ new J() }
	},
	envmap: {
		envMap: { value: null },
		envMapRotation: { value: /*@__PURE__*/ new J() },
		reflectivity: { value: 1 },
		ior: { value: 1.5 },
		refractionRatio: { value: .98 },
		dfgLUT: { value: null }
	},
	aomap: {
		aoMap: { value: null },
		aoMapIntensity: { value: 1 },
		aoMapTransform: { value: /*@__PURE__*/ new J() }
	},
	lightmap: {
		lightMap: { value: null },
		lightMapIntensity: { value: 1 },
		lightMapTransform: { value: /*@__PURE__*/ new J() }
	},
	bumpmap: {
		bumpMap: { value: null },
		bumpMapTransform: { value: /*@__PURE__*/ new J() },
		bumpScale: { value: 1 }
	},
	normalmap: {
		normalMap: { value: null },
		normalMapTransform: { value: /*@__PURE__*/ new J() },
		normalScale: { value: /*@__PURE__*/ new Yr(1, 1) }
	},
	displacementmap: {
		displacementMap: { value: null },
		displacementMapTransform: { value: /*@__PURE__*/ new J() },
		displacementScale: { value: 1 },
		displacementBias: { value: 0 }
	},
	emissivemap: {
		emissiveMap: { value: null },
		emissiveMapTransform: { value: /*@__PURE__*/ new J() }
	},
	metalnessmap: {
		metalnessMap: { value: null },
		metalnessMapTransform: { value: /*@__PURE__*/ new J() }
	},
	roughnessmap: {
		roughnessMap: { value: null },
		roughnessMapTransform: { value: /*@__PURE__*/ new J() }
	},
	gradientmap: { gradientMap: { value: null } },
	fog: {
		fogDensity: { value: 25e-5 },
		fogNear: { value: 1 },
		fogFar: { value: 2e3 },
		fogColor: { value: /*@__PURE__*/ new X(16777215) }
	},
	lights: {
		ambientLightColor: { value: [] },
		lightProbe: { value: [] },
		directionalLights: {
			value: [],
			properties: {
				direction: {},
				color: {}
			}
		},
		directionalLightShadows: {
			value: [],
			properties: {
				shadowIntensity: 1,
				shadowBias: {},
				shadowNormalBias: {},
				shadowRadius: {},
				shadowMapSize: {}
			}
		},
		directionalShadowMatrix: { value: [] },
		spotLights: {
			value: [],
			properties: {
				color: {},
				position: {},
				direction: {},
				distance: {},
				coneCos: {},
				penumbraCos: {},
				decay: {}
			}
		},
		spotLightShadows: {
			value: [],
			properties: {
				shadowIntensity: 1,
				shadowBias: {},
				shadowNormalBias: {},
				shadowRadius: {},
				shadowMapSize: {}
			}
		},
		spotLightMap: { value: [] },
		spotLightMatrix: { value: [] },
		pointLights: {
			value: [],
			properties: {
				color: {},
				position: {},
				decay: {},
				distance: {}
			}
		},
		pointLightShadows: {
			value: [],
			properties: {
				shadowIntensity: 1,
				shadowBias: {},
				shadowNormalBias: {},
				shadowRadius: {},
				shadowMapSize: {},
				shadowCameraNear: {},
				shadowCameraFar: {}
			}
		},
		pointShadowMatrix: { value: [] },
		hemisphereLights: {
			value: [],
			properties: {
				direction: {},
				skyColor: {},
				groundColor: {}
			}
		},
		rectAreaLights: {
			value: [],
			properties: {
				color: {},
				position: {},
				width: {},
				height: {}
			}
		},
		ltc_1: { value: null },
		ltc_2: { value: null },
		probesSH: { value: null },
		probesMin: { value: /*@__PURE__*/ new q() },
		probesMax: { value: /*@__PURE__*/ new q() },
		probesResolution: { value: /*@__PURE__*/ new q() }
	},
	points: {
		diffuse: { value: /*@__PURE__*/ new X(16777215) },
		opacity: { value: 1 },
		size: { value: 1 },
		scale: { value: 1 },
		map: { value: null },
		alphaMap: { value: null },
		alphaMapTransform: { value: /*@__PURE__*/ new J() },
		alphaTest: { value: 0 },
		uvTransform: { value: /*@__PURE__*/ new J() }
	},
	sprite: {
		diffuse: { value: /*@__PURE__*/ new X(16777215) },
		opacity: { value: 1 },
		center: { value: /*@__PURE__*/ new Yr(.5, .5) },
		rotation: { value: 0 },
		map: { value: null },
		mapTransform: { value: /*@__PURE__*/ new J() },
		alphaMap: { value: null },
		alphaMapTransform: { value: /*@__PURE__*/ new J() },
		alphaTest: { value: 0 }
	}
}, el = {
	basic: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.specularmap,
			Q.envmap,
			Q.aomap,
			Q.lightmap,
			Q.fog
		]),
		vertexShader: Z.meshbasic_vert,
		fragmentShader: Z.meshbasic_frag
	},
	lambert: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.specularmap,
			Q.envmap,
			Q.aomap,
			Q.lightmap,
			Q.emissivemap,
			Q.bumpmap,
			Q.normalmap,
			Q.displacementmap,
			Q.fog,
			Q.lights,
			{
				emissive: { value: /*@__PURE__*/ new X(0) },
				envMapIntensity: { value: 1 }
			}
		]),
		vertexShader: Z.meshlambert_vert,
		fragmentShader: Z.meshlambert_frag
	},
	phong: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.specularmap,
			Q.envmap,
			Q.aomap,
			Q.lightmap,
			Q.emissivemap,
			Q.bumpmap,
			Q.normalmap,
			Q.displacementmap,
			Q.fog,
			Q.lights,
			{
				emissive: { value: /*@__PURE__*/ new X(0) },
				specular: { value: /*@__PURE__*/ new X(1118481) },
				shininess: { value: 30 },
				envMapIntensity: { value: 1 }
			}
		]),
		vertexShader: Z.meshphong_vert,
		fragmentShader: Z.meshphong_frag
	},
	standard: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.envmap,
			Q.aomap,
			Q.lightmap,
			Q.emissivemap,
			Q.bumpmap,
			Q.normalmap,
			Q.displacementmap,
			Q.roughnessmap,
			Q.metalnessmap,
			Q.fog,
			Q.lights,
			{
				emissive: { value: /*@__PURE__*/ new X(0) },
				roughness: { value: 1 },
				metalness: { value: 0 },
				envMapIntensity: { value: 1 }
			}
		]),
		vertexShader: Z.meshphysical_vert,
		fragmentShader: Z.meshphysical_frag
	},
	toon: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.aomap,
			Q.lightmap,
			Q.emissivemap,
			Q.bumpmap,
			Q.normalmap,
			Q.displacementmap,
			Q.gradientmap,
			Q.fog,
			Q.lights,
			{ emissive: { value: /*@__PURE__*/ new X(0) } }
		]),
		vertexShader: Z.meshtoon_vert,
		fragmentShader: Z.meshtoon_frag
	},
	matcap: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.bumpmap,
			Q.normalmap,
			Q.displacementmap,
			Q.fog,
			{ matcap: { value: null } }
		]),
		vertexShader: Z.meshmatcap_vert,
		fragmentShader: Z.meshmatcap_frag
	},
	points: {
		uniforms: /*@__PURE__*/ ws([Q.points, Q.fog]),
		vertexShader: Z.points_vert,
		fragmentShader: Z.points_frag
	},
	dashed: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.fog,
			{
				scale: { value: 1 },
				dashSize: { value: 1 },
				totalSize: { value: 2 }
			}
		]),
		vertexShader: Z.linedashed_vert,
		fragmentShader: Z.linedashed_frag
	},
	depth: {
		uniforms: /*@__PURE__*/ ws([Q.common, Q.displacementmap]),
		vertexShader: Z.depth_vert,
		fragmentShader: Z.depth_frag
	},
	normal: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.bumpmap,
			Q.normalmap,
			Q.displacementmap,
			{ opacity: { value: 1 } }
		]),
		vertexShader: Z.meshnormal_vert,
		fragmentShader: Z.meshnormal_frag
	},
	sprite: {
		uniforms: /*@__PURE__*/ ws([Q.sprite, Q.fog]),
		vertexShader: Z.sprite_vert,
		fragmentShader: Z.sprite_frag
	},
	background: {
		uniforms: {
			uvTransform: { value: /*@__PURE__*/ new J() },
			t2D: { value: null },
			backgroundIntensity: { value: 1 }
		},
		vertexShader: Z.background_vert,
		fragmentShader: Z.background_frag
	},
	backgroundCube: {
		uniforms: {
			envMap: { value: null },
			backgroundBlurriness: { value: 0 },
			backgroundIntensity: { value: 1 },
			backgroundRotation: { value: /*@__PURE__*/ new J() }
		},
		vertexShader: Z.backgroundCube_vert,
		fragmentShader: Z.backgroundCube_frag
	},
	cube: {
		uniforms: {
			tCube: { value: null },
			tFlip: { value: -1 },
			opacity: { value: 1 }
		},
		vertexShader: Z.cube_vert,
		fragmentShader: Z.cube_frag
	},
	equirect: {
		uniforms: { tEquirect: { value: null } },
		vertexShader: Z.equirect_vert,
		fragmentShader: Z.equirect_frag
	},
	distance: {
		uniforms: /*@__PURE__*/ ws([
			Q.common,
			Q.displacementmap,
			{
				referencePosition: { value: /*@__PURE__*/ new q() },
				nearDistance: { value: 1 },
				farDistance: { value: 1e3 }
			}
		]),
		vertexShader: Z.distance_vert,
		fragmentShader: Z.distance_frag
	},
	shadow: {
		uniforms: /*@__PURE__*/ ws([
			Q.lights,
			Q.fog,
			{
				color: { value: /*@__PURE__*/ new X(0) },
				opacity: { value: 1 }
			}
		]),
		vertexShader: Z.shadow_vert,
		fragmentShader: Z.shadow_frag
	}
};
el.physical = {
	uniforms: /*@__PURE__*/ ws([el.standard.uniforms, {
		clearcoat: { value: 0 },
		clearcoatMap: { value: null },
		clearcoatMapTransform: { value: /*@__PURE__*/ new J() },
		clearcoatNormalMap: { value: null },
		clearcoatNormalMapTransform: { value: /*@__PURE__*/ new J() },
		clearcoatNormalScale: { value: /*@__PURE__*/ new Yr(1, 1) },
		clearcoatRoughness: { value: 0 },
		clearcoatRoughnessMap: { value: null },
		clearcoatRoughnessMapTransform: { value: /*@__PURE__*/ new J() },
		dispersion: { value: 0 },
		iridescence: { value: 0 },
		iridescenceMap: { value: null },
		iridescenceMapTransform: { value: /*@__PURE__*/ new J() },
		iridescenceIOR: { value: 1.3 },
		iridescenceThicknessMinimum: { value: 100 },
		iridescenceThicknessMaximum: { value: 400 },
		iridescenceThicknessMap: { value: null },
		iridescenceThicknessMapTransform: { value: /*@__PURE__*/ new J() },
		sheen: { value: 0 },
		sheenColor: { value: /*@__PURE__*/ new X(0) },
		sheenColorMap: { value: null },
		sheenColorMapTransform: { value: /*@__PURE__*/ new J() },
		sheenRoughness: { value: 1 },
		sheenRoughnessMap: { value: null },
		sheenRoughnessMapTransform: { value: /*@__PURE__*/ new J() },
		transmission: { value: 0 },
		transmissionMap: { value: null },
		transmissionMapTransform: { value: /*@__PURE__*/ new J() },
		transmissionSamplerSize: { value: /*@__PURE__*/ new Yr() },
		transmissionSamplerMap: { value: null },
		thickness: { value: 0 },
		thicknessMap: { value: null },
		thicknessMapTransform: { value: /*@__PURE__*/ new J() },
		attenuationDistance: { value: 0 },
		attenuationColor: { value: /*@__PURE__*/ new X(0) },
		specularColor: { value: /*@__PURE__*/ new X(1, 1, 1) },
		specularColorMap: { value: null },
		specularColorMapTransform: { value: /*@__PURE__*/ new J() },
		specularIntensity: { value: 1 },
		specularIntensityMap: { value: null },
		specularIntensityMapTransform: { value: /*@__PURE__*/ new J() },
		anisotropyVector: { value: /*@__PURE__*/ new Yr() },
		anisotropyMap: { value: null },
		anisotropyMapTransform: { value: /*@__PURE__*/ new J() }
	}]),
	vertexShader: Z.meshphysical_vert,
	fragmentShader: Z.meshphysical_frag
};
var tl = {
	r: 0,
	b: 0,
	g: 0
}, nl = /*@__PURE__*/ new vi(), rl = /*@__PURE__*/ new J();
rl.set(-1, 0, 0, 0, 1, 0, 0, 0, 1);
function il(e, t, n, r, i, a) {
	let o = new X(0), s = i === !0 ? 0 : 1, c, l, u = null, d = 0, f = null;
	function p(e) {
		let n = e.isScene === !0 ? e.background : null;
		if (n && n.isTexture) {
			let r = e.backgroundBlurriness > 0;
			n = t.get(n, r);
		}
		return n;
	}
	function m(t) {
		let r = !1, i = p(t);
		i === null ? g(o, s) : i && i.isColor && (g(i, 1), r = !0);
		let c = e.xr.getEnvironmentBlendMode();
		c === "additive" ? n.buffers.color.setClear(0, 0, 0, 1, a) : c === "alpha-blend" && n.buffers.color.setClear(0, 0, 0, 0, a), (e.autoClear || r) && (n.buffers.depth.setTest(!0), n.buffers.depth.setMask(!0), n.buffers.color.setMask(!0), e.clear(e.autoClearColor, e.autoClearDepth, e.autoClearStencil));
	}
	function h(t, n) {
		let i = p(n);
		i && (i.isCubeTexture || i.mapping === 306) ? (l === void 0 && (l = new vo(new bs(1, 1, 1), new js({
			name: "BackgroundCubeMaterial",
			uniforms: Cs(el.backgroundCube.uniforms),
			vertexShader: el.backgroundCube.vertexShader,
			fragmentShader: el.backgroundCube.fragmentShader,
			side: 1,
			depthTest: !1,
			depthWrite: !1,
			fog: !1,
			allowOverride: !1
		})), l.geometry.deleteAttribute("normal"), l.geometry.deleteAttribute("uv"), l.onBeforeRender = function(e, t, n) {
			this.matrixWorld.copyPosition(n.matrixWorld);
		}, Object.defineProperty(l.material, "envMap", { get: function() {
			return this.uniforms.envMap.value;
		} }), r.update(l)), l.material.uniforms.envMap.value = i, l.material.uniforms.backgroundBlurriness.value = n.backgroundBlurriness, l.material.uniforms.backgroundIntensity.value = n.backgroundIntensity, l.material.uniforms.backgroundRotation.value.setFromMatrix4(nl.makeRotationFromEuler(n.backgroundRotation)).transpose(), i.isCubeTexture && i.isRenderTargetTexture === !1 && l.material.uniforms.backgroundRotation.value.premultiply(rl), l.material.toneMapped = Y.getTransfer(i.colorSpace) !== sr, (u !== i || d !== i.version || f !== e.toneMapping) && (l.material.needsUpdate = !0, u = i, d = i.version, f = e.toneMapping), l.layers.enableAll(), t.unshift(l, l.geometry, l.material, 0, 0, null)) : i && i.isTexture && (c === void 0 && (c = new vo(new xs(2, 2), new js({
			name: "BackgroundMaterial",
			uniforms: Cs(el.background.uniforms),
			vertexShader: el.background.vertexShader,
			fragmentShader: el.background.fragmentShader,
			side: 0,
			depthTest: !1,
			depthWrite: !1,
			fog: !1,
			allowOverride: !1
		})), c.geometry.deleteAttribute("normal"), Object.defineProperty(c.material, "map", { get: function() {
			return this.uniforms.t2D.value;
		} }), r.update(c)), c.material.uniforms.t2D.value = i, c.material.uniforms.backgroundIntensity.value = n.backgroundIntensity, c.material.toneMapped = Y.getTransfer(i.colorSpace) !== sr, i.matrixAutoUpdate === !0 && i.updateMatrix(), c.material.uniforms.uvTransform.value.copy(i.matrix), (u !== i || d !== i.version || f !== e.toneMapping) && (c.material.needsUpdate = !0, u = i, d = i.version, f = e.toneMapping), c.layers.enableAll(), t.unshift(c, c.geometry, c.material, 0, 0, null));
	}
	function g(t, r) {
		t.getRGB(tl, Ds(e)), n.buffers.color.setClear(tl.r, tl.g, tl.b, r, a);
	}
	function _() {
		l !== void 0 && (l.geometry.dispose(), l.material.dispose(), l = void 0), c !== void 0 && (c.geometry.dispose(), c.material.dispose(), c = void 0);
	}
	return {
		getClearColor: function() {
			return o;
		},
		setClearColor: function(e, t = 1) {
			o.set(e), s = t, g(o, s);
		},
		getClearAlpha: function() {
			return s;
		},
		setClearAlpha: function(e) {
			s = e, g(o, s);
		},
		render: m,
		addToRenderList: h,
		dispose: _
	};
}
function al(e, t) {
	let n = e.getParameter(e.MAX_VERTEX_ATTRIBS), r = {}, i = f(null), a = i, o = !1;
	function s(n, r, i, s, c) {
		let u = !1, f = d(n, s, i, r);
		a !== f && (a = f, l(a.object)), u = p(n, s, i, c), u && m(n, s, i, c), c !== null && t.update(c, e.ELEMENT_ARRAY_BUFFER), (u || o) && (o = !1, b(n, r, i, s), c !== null && e.bindBuffer(e.ELEMENT_ARRAY_BUFFER, t.get(c).buffer));
	}
	function c() {
		return e.createVertexArray();
	}
	function l(t) {
		return e.bindVertexArray(t);
	}
	function u(t) {
		return e.deleteVertexArray(t);
	}
	function d(e, t, n, i) {
		let a = i.wireframe === !0, o = r[t.id];
		o === void 0 && (o = {}, r[t.id] = o);
		let s = e.isInstancedMesh === !0 ? e.id : 0, l = o[s];
		l === void 0 && (l = {}, o[s] = l);
		let u = l[n.id];
		u === void 0 && (u = {}, l[n.id] = u);
		let d = u[a];
		return d === void 0 && (d = f(c()), u[a] = d), d;
	}
	function f(e) {
		let t = [], r = [], i = [];
		for (let e = 0; e < n; e++) t[e] = 0, r[e] = 0, i[e] = 0;
		return {
			geometry: null,
			program: null,
			wireframe: !1,
			newAttributes: t,
			enabledAttributes: r,
			attributeDivisors: i,
			object: e,
			attributes: {},
			index: null
		};
	}
	function p(e, t, n, r) {
		let i = a.attributes, o = t.attributes, s = 0, c = n.getAttributes();
		for (let t in c) if (c[t].location >= 0) {
			let n = i[t], r = o[t];
			if (r === void 0 && (t === "instanceMatrix" && e.instanceMatrix && (r = e.instanceMatrix), t === "instanceColor" && e.instanceColor && (r = e.instanceColor)), n === void 0 || n.attribute !== r || r && n.data !== r.data) return !0;
			s++;
		}
		return a.attributesNum !== s || a.index !== r;
	}
	function m(e, t, n, r) {
		let i = {}, o = t.attributes, s = 0, c = n.getAttributes();
		for (let t in c) if (c[t].location >= 0) {
			let n = o[t];
			n === void 0 && (t === "instanceMatrix" && e.instanceMatrix && (n = e.instanceMatrix), t === "instanceColor" && e.instanceColor && (n = e.instanceColor));
			let r = {};
			r.attribute = n, n && n.data && (r.data = n.data), i[t] = r, s++;
		}
		a.attributes = i, a.attributesNum = s, a.index = r;
	}
	function h() {
		let e = a.newAttributes;
		for (let t = 0, n = e.length; t < n; t++) e[t] = 0;
	}
	function g(e) {
		_(e, 0);
	}
	function _(t, n) {
		let r = a.newAttributes, i = a.enabledAttributes, o = a.attributeDivisors;
		r[t] = 1, i[t] === 0 && (e.enableVertexAttribArray(t), i[t] = 1), o[t] !== n && (e.vertexAttribDivisor(t, n), o[t] = n);
	}
	function v() {
		let t = a.newAttributes, n = a.enabledAttributes;
		for (let r = 0, i = n.length; r < i; r++) n[r] !== t[r] && (e.disableVertexAttribArray(r), n[r] = 0);
	}
	function y(t, n, r, i, a, o, s) {
		s === !0 ? e.vertexAttribIPointer(t, n, r, a, o) : e.vertexAttribPointer(t, n, r, i, a, o);
	}
	function b(n, r, i, a) {
		h();
		let o = a.attributes, s = i.getAttributes(), c = r.defaultAttributeValues;
		for (let r in s) {
			let i = s[r];
			if (i.location >= 0) {
				let s = o[r];
				if (s === void 0 && (r === "instanceMatrix" && n.instanceMatrix && (s = n.instanceMatrix), r === "instanceColor" && n.instanceColor && (s = n.instanceColor)), s !== void 0) {
					let r = s.normalized, o = s.itemSize, c = t.get(s);
					if (c === void 0) continue;
					let l = c.buffer, u = c.type, d = c.bytesPerElement, f = u === e.INT || u === e.UNSIGNED_INT || s.gpuType === 1013;
					if (s.isInterleavedBufferAttribute) {
						let t = s.data, c = t.stride, p = s.offset;
						if (t.isInstancedInterleavedBuffer) {
							for (let e = 0; e < i.locationSize; e++) _(i.location + e, t.meshPerAttribute);
							n.isInstancedMesh !== !0 && a._maxInstanceCount === void 0 && (a._maxInstanceCount = t.meshPerAttribute * t.count);
						} else for (let e = 0; e < i.locationSize; e++) g(i.location + e);
						e.bindBuffer(e.ARRAY_BUFFER, l);
						for (let e = 0; e < i.locationSize; e++) y(i.location + e, o / i.locationSize, u, r, c * d, (p + o / i.locationSize * e) * d, f);
					} else {
						if (s.isInstancedBufferAttribute) {
							for (let e = 0; e < i.locationSize; e++) _(i.location + e, s.meshPerAttribute);
							n.isInstancedMesh !== !0 && a._maxInstanceCount === void 0 && (a._maxInstanceCount = s.meshPerAttribute * s.count);
						} else for (let e = 0; e < i.locationSize; e++) g(i.location + e);
						e.bindBuffer(e.ARRAY_BUFFER, l);
						for (let e = 0; e < i.locationSize; e++) y(i.location + e, o / i.locationSize, u, r, o * d, o / i.locationSize * e * d, f);
					}
				} else if (c !== void 0) {
					let t = c[r];
					if (t !== void 0) switch (t.length) {
						case 2:
							e.vertexAttrib2fv(i.location, t);
							break;
						case 3:
							e.vertexAttrib3fv(i.location, t);
							break;
						case 4:
							e.vertexAttrib4fv(i.location, t);
							break;
						default: e.vertexAttrib1fv(i.location, t);
					}
				}
			}
		}
		v();
	}
	function x() {
		T();
		for (let e in r) {
			let t = r[e];
			for (let e in t) {
				let n = t[e];
				for (let e in n) {
					let t = n[e];
					for (let e in t) u(t[e].object), delete t[e];
					delete n[e];
				}
			}
			delete r[e];
		}
	}
	function S(e) {
		if (r[e.id] === void 0) return;
		let t = r[e.id];
		for (let e in t) {
			let n = t[e];
			for (let e in n) {
				let t = n[e];
				for (let e in t) u(t[e].object), delete t[e];
				delete n[e];
			}
		}
		delete r[e.id];
	}
	function C(e) {
		for (let t in r) {
			let n = r[t];
			for (let t in n) {
				let r = n[t];
				if (r[e.id] === void 0) continue;
				let i = r[e.id];
				for (let e in i) u(i[e].object), delete i[e];
				delete r[e.id];
			}
		}
	}
	function w(e) {
		for (let t in r) {
			let n = r[t], i = e.isInstancedMesh === !0 ? e.id : 0, a = n[i];
			if (a !== void 0) {
				for (let e in a) {
					let t = a[e];
					for (let e in t) u(t[e].object), delete t[e];
					delete a[e];
				}
				delete n[i], Object.keys(n).length === 0 && delete r[t];
			}
		}
	}
	function T() {
		E(), o = !0, a !== i && (a = i, l(a.object));
	}
	function E() {
		i.geometry = null, i.program = null, i.wireframe = !1;
	}
	return {
		setup: s,
		reset: T,
		resetDefaultState: E,
		dispose: x,
		releaseStatesOfGeometry: S,
		releaseStatesOfObject: w,
		releaseStatesOfProgram: C,
		initAttributes: h,
		enableAttribute: g,
		disableUnusedAttributes: v
	};
}
function ol(e, t, n) {
	let r;
	function i(e) {
		r = e;
	}
	function a(t, i) {
		e.drawArrays(r, t, i), n.update(i, r, 1);
	}
	function o(t, i, a) {
		a !== 0 && (e.drawArraysInstanced(r, t, i, a), n.update(i, r, a));
	}
	function s(e, i, a) {
		if (a === 0) return;
		t.get("WEBGL_multi_draw").multiDrawArraysWEBGL(r, e, 0, i, 0, a);
		let o = 0;
		for (let e = 0; e < a; e++) o += i[e];
		n.update(o, r, 1);
	}
	this.setMode = i, this.render = a, this.renderInstances = o, this.renderMultiDraw = s;
}
function sl(e, t, n, r) {
	let i;
	function a() {
		if (i !== void 0) return i;
		if (t.has("EXT_texture_filter_anisotropic") === !0) {
			let n = t.get("EXT_texture_filter_anisotropic");
			i = e.getParameter(n.MAX_TEXTURE_MAX_ANISOTROPY_EXT);
		} else i = 0;
		return i;
	}
	function o(t) {
		return !(t !== 1023 && r.convert(t) !== e.getParameter(e.IMPLEMENTATION_COLOR_READ_FORMAT));
	}
	function s(n) {
		let i = n === 1016 && (t.has("EXT_color_buffer_half_float") || t.has("EXT_color_buffer_float"));
		return !(n !== 1009 && r.convert(n) !== e.getParameter(e.IMPLEMENTATION_COLOR_READ_TYPE) && n !== 1015 && !i);
	}
	function c(t) {
		if (t === "highp") {
			if (e.getShaderPrecisionFormat(e.VERTEX_SHADER, e.HIGH_FLOAT).precision > 0 && e.getShaderPrecisionFormat(e.FRAGMENT_SHADER, e.HIGH_FLOAT).precision > 0) return "highp";
			t = "mediump";
		}
		return t === "mediump" && e.getShaderPrecisionFormat(e.VERTEX_SHADER, e.MEDIUM_FLOAT).precision > 0 && e.getShaderPrecisionFormat(e.FRAGMENT_SHADER, e.MEDIUM_FLOAT).precision > 0 ? "mediump" : "lowp";
	}
	let l = n.precision === void 0 ? "highp" : n.precision, u = c(l);
	u !== l && (W("WebGLRenderer:", l, "not supported, using", u, "instead."), l = u);
	let d = n.logarithmicDepthBuffer === !0, f = n.reversedDepthBuffer === !0 && t.has("EXT_clip_control");
	n.reversedDepthBuffer === !0 && f === !1 && W("WebGLRenderer: Unable to use reversed depth buffer due to missing EXT_clip_control extension. Fallback to default depth buffer.");
	let p = e.getParameter(e.MAX_TEXTURE_IMAGE_UNITS), m = e.getParameter(e.MAX_VERTEX_TEXTURE_IMAGE_UNITS), h = e.getParameter(e.MAX_TEXTURE_SIZE), g = e.getParameter(e.MAX_CUBE_MAP_TEXTURE_SIZE), _ = e.getParameter(e.MAX_VERTEX_ATTRIBS), v = e.getParameter(e.MAX_VERTEX_UNIFORM_VECTORS), y = e.getParameter(e.MAX_VARYING_VECTORS), b = e.getParameter(e.MAX_FRAGMENT_UNIFORM_VECTORS), x = e.getParameter(e.MAX_SAMPLES), S = e.getParameter(e.SAMPLES);
	return {
		isWebGL2: !0,
		getMaxAnisotropy: a,
		getMaxPrecision: c,
		textureFormatReadable: o,
		textureTypeReadable: s,
		precision: l,
		logarithmicDepthBuffer: d,
		reversedDepthBuffer: f,
		maxTextures: p,
		maxVertexTextures: m,
		maxTextureSize: h,
		maxCubemapSize: g,
		maxAttributes: _,
		maxVertexUniforms: v,
		maxVaryings: y,
		maxFragmentUniforms: b,
		maxSamples: x,
		samples: S
	};
}
function cl(e) {
	let t = this, n = null, r = 0, i = !1, a = !1, o = new Go(), s = new J(), c = {
		value: null,
		needsUpdate: !1
	};
	this.uniform = c, this.numPlanes = 0, this.numIntersection = 0, this.init = function(e, t) {
		let n = e.length !== 0 || t || r !== 0 || i;
		return i = t, r = e.length, n;
	}, this.beginShadows = function() {
		a = !0, u(null);
	}, this.endShadows = function() {
		a = !1;
	}, this.setGlobalState = function(e, t) {
		n = u(e, t, 0);
	}, this.setState = function(t, o, s) {
		let d = t.clippingPlanes, f = t.clipIntersection, p = t.clipShadows, m = e.get(t);
		if (!i || d === null || d.length === 0 || a && !p) a ? u(null) : l();
		else {
			let e = a ? 0 : r, t = e * 4, i = m.clippingState || null;
			c.value = i, i = u(d, o, t, s);
			for (let e = 0; e !== t; ++e) i[e] = n[e];
			m.clippingState = i, this.numIntersection = f ? this.numPlanes : 0, this.numPlanes += e;
		}
	};
	function l() {
		c.value !== n && (c.value = n, c.needsUpdate = r > 0), t.numPlanes = r, t.numIntersection = 0;
	}
	function u(e, n, r, i) {
		let a = e === null ? 0 : e.length, l = null;
		if (a !== 0) {
			if (l = c.value, i !== !0 || l === null) {
				let t = r + a * 4, i = n.matrixWorldInverse;
				s.getNormalMatrix(i), (l === null || l.length < t) && (l = new Float32Array(t));
				for (let t = 0, n = r; t !== a; ++t, n += 4) o.copy(e[t]).applyMatrix4(i, s), o.normal.toArray(l, n), l[n + 3] = o.constant;
			}
			c.value = l, c.needsUpdate = !0;
		}
		return t.numPlanes = a, t.numIntersection = 0, l;
	}
}
var ll = 4, ul = [
	.125,
	.215,
	.35,
	.446,
	.526,
	.582
], dl = 20, fl = 256, pl = /*@__PURE__*/ new Sc(), ml = /*@__PURE__*/ new X(), hl = null, gl = 0, _l = 0, vl = !1, yl = /*@__PURE__*/ new q(), bl = class {
	constructor(e) {
		this._renderer = e, this._pingPongRenderTarget = null, this._lodMax = 0, this._cubeSize = 0, this._sizeLods = [], this._sigmas = [], this._lodMeshes = [], this._backgroundBox = null, this._cubemapMaterial = null, this._equirectMaterial = null, this._blurMaterial = null, this._ggxMaterial = null;
	}
	fromScene(e, t = 0, n = .1, r = 100, i = {}) {
		let { size: a = 256, position: o = yl } = i;
		hl = this._renderer.getRenderTarget(), gl = this._renderer.getActiveCubeFace(), _l = this._renderer.getActiveMipmapLevel(), vl = this._renderer.xr.enabled, this._renderer.xr.enabled = !1, this._setSize(a);
		let s = this._allocateTargets();
		return s.depthBuffer = !0, this._sceneToCubeUV(e, n, r, s, o), t > 0 && this._blur(s, 0, 0, t), this._applyPMREM(s), this._cleanup(s), s;
	}
	fromEquirectangular(e, t = null) {
		return this._fromTexture(e, t);
	}
	fromCubemap(e, t = null) {
		return this._fromTexture(e, t);
	}
	compileCubemapShader() {
		this._cubemapMaterial === null && (this._cubemapMaterial = Dl(), this._compileMaterial(this._cubemapMaterial));
	}
	compileEquirectangularShader() {
		this._equirectMaterial === null && (this._equirectMaterial = El(), this._compileMaterial(this._equirectMaterial));
	}
	dispose() {
		this._dispose(), this._cubemapMaterial !== null && this._cubemapMaterial.dispose(), this._equirectMaterial !== null && this._equirectMaterial.dispose(), this._backgroundBox !== null && (this._backgroundBox.geometry.dispose(), this._backgroundBox.material.dispose());
	}
	_setSize(e) {
		this._lodMax = Math.floor(Math.log2(e)), this._cubeSize = 2 ** this._lodMax;
	}
	_dispose() {
		this._blurMaterial !== null && this._blurMaterial.dispose(), this._ggxMaterial !== null && this._ggxMaterial.dispose(), this._pingPongRenderTarget !== null && this._pingPongRenderTarget.dispose();
		for (let e = 0; e < this._lodMeshes.length; e++) this._lodMeshes[e].geometry.dispose();
	}
	_cleanup(e) {
		this._renderer.setRenderTarget(hl, gl, _l), this._renderer.xr.enabled = vl, e.scissorTest = !1, Cl(e, 0, 0, e.width, e.height);
	}
	_fromTexture(e, t) {
		e.mapping === 301 || e.mapping === 302 ? this._setSize(e.image.length === 0 ? 16 : e.image[0].width || e.image[0].image.width) : this._setSize(e.image.width / 4), hl = this._renderer.getRenderTarget(), gl = this._renderer.getActiveCubeFace(), _l = this._renderer.getActiveMipmapLevel(), vl = this._renderer.xr.enabled, this._renderer.xr.enabled = !1;
		let n = t || this._allocateTargets();
		return this._textureToCubeUV(e, n), this._applyPMREM(n), this._cleanup(n), n;
	}
	_allocateTargets() {
		let e = 3 * Math.max(this._cubeSize, 112), t = 4 * this._cubeSize, n = {
			magFilter: It,
			minFilter: It,
			generateMipmaps: !1,
			type: Kt,
			format: en,
			colorSpace: ar,
			depthBuffer: !1
		}, r = Sl(e, t, n);
		if (this._pingPongRenderTarget === null || this._pingPongRenderTarget.width !== e || this._pingPongRenderTarget.height !== t) {
			this._pingPongRenderTarget !== null && this._dispose(), this._pingPongRenderTarget = Sl(e, t, n);
			let { _lodMax: r } = this;
			({lodMeshes: this._lodMeshes, sizeLods: this._sizeLods, sigmas: this._sigmas} = xl(r)), this._blurMaterial = Tl(r, e, t), this._ggxMaterial = wl(r, e, t);
		}
		return r;
	}
	_compileMaterial(e) {
		let t = new vo(new Ja(), e);
		this._renderer.compile(t, pl);
	}
	_sceneToCubeUV(e, t, n, r, i) {
		let a = new _c(90, 1, t, n), o = [
			1,
			-1,
			1,
			1,
			1,
			1
		], s = [
			1,
			1,
			1,
			-1,
			-1,
			-1
		], c = this._renderer, l = c.autoClear, u = c.toneMapping;
		c.getClearColor(ml), c.toneMapping = 0, c.autoClear = !1, c.state.buffers.depth.getReversed() && (c.setRenderTarget(r), c.clearDepth(), c.setRenderTarget(null)), this._backgroundBox === null && (this._backgroundBox = new vo(new bs(), new ao({
			name: "PMREM.Background",
			side: 1,
			depthWrite: !1,
			depthTest: !1
		})));
		let d = this._backgroundBox, f = d.material, p = !1, m = e.background;
		m ? m.isColor && (f.color.copy(m), e.background = null, p = !0) : (f.color.copy(ml), p = !0);
		for (let t = 0; t < 6; t++) {
			let n = t % 3;
			n === 0 ? (a.up.set(0, o[t], 0), a.position.set(i.x, i.y, i.z), a.lookAt(i.x + s[t], i.y, i.z)) : n === 1 ? (a.up.set(0, 0, o[t]), a.position.set(i.x, i.y, i.z), a.lookAt(i.x, i.y + s[t], i.z)) : (a.up.set(0, o[t], 0), a.position.set(i.x, i.y, i.z), a.lookAt(i.x, i.y, i.z + s[t]));
			let l = this._cubeSize;
			Cl(r, n * l, t > 2 ? l : 0, l, l), c.setRenderTarget(r), p && c.render(d, a), c.render(e, a);
		}
		c.toneMapping = u, c.autoClear = l, e.background = m;
	}
	_textureToCubeUV(e, t) {
		let n = this._renderer, r = e.mapping === 301 || e.mapping === 302;
		r ? (this._cubemapMaterial === null && (this._cubemapMaterial = Dl()), this._cubemapMaterial.uniforms.flipEnvMap.value = e.isRenderTargetTexture === !1 ? -1 : 1) : this._equirectMaterial === null && (this._equirectMaterial = El());
		let i = r ? this._cubemapMaterial : this._equirectMaterial, a = this._lodMeshes[0];
		a.material = i;
		let o = i.uniforms;
		o.envMap.value = e;
		let s = this._cubeSize;
		Cl(t, 0, 0, 3 * s, 2 * s), n.setRenderTarget(t), n.render(a, pl);
	}
	_applyPMREM(e) {
		let t = this._renderer, n = t.autoClear;
		t.autoClear = !1;
		let r = this._lodMeshes.length;
		for (let t = 1; t < r; t++) this._applyGGXFilter(e, t - 1, t);
		t.autoClear = n;
	}
	_applyGGXFilter(e, t, n) {
		let r = this._renderer, i = this._pingPongRenderTarget, a = this._ggxMaterial, o = this._lodMeshes[n];
		o.material = a;
		let s = a.uniforms, c = n / (this._lodMeshes.length - 1), l = t / (this._lodMeshes.length - 1), u = Math.sqrt(c * c - l * l) * (0 + c * 1.25), { _lodMax: d } = this, f = this._sizeLods[n], p = 3 * f * (n > d - ll ? n - d + ll : 0), m = 4 * (this._cubeSize - f);
		s.envMap.value = e.texture, s.roughness.value = u, s.mipInt.value = d - t, Cl(i, p, m, 3 * f, 2 * f), r.setRenderTarget(i), r.render(o, pl), s.envMap.value = i.texture, s.roughness.value = 0, s.mipInt.value = d - n, Cl(e, p, m, 3 * f, 2 * f), r.setRenderTarget(e), r.render(o, pl);
	}
	_blur(e, t, n, r, i) {
		let a = this._pingPongRenderTarget;
		this._halfBlur(e, a, t, n, r, "latitudinal", i), this._halfBlur(a, e, n, n, r, "longitudinal", i);
	}
	_halfBlur(e, t, n, r, i, a, o) {
		let s = this._renderer, c = this._blurMaterial;
		a !== "latitudinal" && a !== "longitudinal" && G("blur direction must be either latitudinal or longitudinal!");
		let l = this._lodMeshes[r];
		l.material = c;
		let u = c.uniforms, d = this._sizeLods[n] - 1, f = isFinite(i) ? Math.PI / (2 * d) : 2 * Math.PI / (2 * dl - 1), p = i / f, m = isFinite(i) ? 1 + Math.floor(3 * p) : dl;
		m > dl && W(`sigmaRadians, ${i}, is too large and will clip, as it requested ${m} samples when the maximum is set to ${dl}`);
		let h = [], g = 0;
		for (let e = 0; e < dl; ++e) {
			let t = e / p, n = Math.exp(-t * t / 2);
			h.push(n), e === 0 ? g += n : e < m && (g += 2 * n);
		}
		for (let e = 0; e < h.length; e++) h[e] = h[e] / g;
		u.envMap.value = e.texture, u.samples.value = m, u.weights.value = h, u.latitudinal.value = a === "latitudinal", o && (u.poleAxis.value = o);
		let { _lodMax: _ } = this;
		u.dTheta.value = f, u.mipInt.value = _ - n;
		let v = this._sizeLods[r];
		Cl(t, 3 * v * (r > _ - ll ? r - _ + ll : 0), 4 * (this._cubeSize - v), 3 * v, 2 * v), s.setRenderTarget(t), s.render(l, pl);
	}
};
function xl(e) {
	let t = [], n = [], r = [], i = e, a = e - ll + 1 + ul.length;
	for (let o = 0; o < a; o++) {
		let a = 2 ** i;
		t.push(a);
		let s = 1 / a;
		o > e - ll ? s = ul[o - e + ll - 1] : o === 0 && (s = 0), n.push(s);
		let c = 1 / (a - 2), l = -c, u = 1 + c, d = [
			l,
			l,
			u,
			l,
			u,
			u,
			l,
			l,
			u,
			u,
			l,
			u
		], f = /* @__PURE__ */ new Float32Array(108), p = /* @__PURE__ */ new Float32Array(72), m = /* @__PURE__ */ new Float32Array(36);
		for (let e = 0; e < 6; e++) {
			let t = e % 3 * 2 / 3 - 1, n = e > 2 ? 0 : -1, r = [
				t,
				n,
				0,
				t + 2 / 3,
				n,
				0,
				t + 2 / 3,
				n + 1,
				0,
				t,
				n,
				0,
				t + 2 / 3,
				n + 1,
				0,
				t,
				n + 1,
				0
			];
			f.set(r, 18 * e), p.set(d, 12 * e);
			let i = [
				e,
				e,
				e,
				e,
				e,
				e
			];
			m.set(i, 6 * e);
		}
		let h = new Ja();
		h.setAttribute("position", new Na(f, 3)), h.setAttribute("uv", new Na(p, 2)), h.setAttribute("faceIndex", new Na(m, 1)), r.push(new vo(h, null)), i > ll && i--;
	}
	return {
		lodMeshes: r,
		sizeLods: t,
		sigmas: n
	};
}
function Sl(e, t, n) {
	let r = new hi(e, t, n);
	return r.texture.mapping = 306, r.texture.name = "PMREM.cubeUv", r.scissorTest = !0, r;
}
function Cl(e, t, n, r, i) {
	e.viewport.set(t, n, r, i), e.scissor.set(t, n, r, i);
}
function wl(e, t, n) {
	return new js({
		name: "PMREMGGXConvolution",
		defines: {
			GGX_SAMPLES: fl,
			CUBEUV_TEXEL_WIDTH: 1 / t,
			CUBEUV_TEXEL_HEIGHT: 1 / n,
			CUBEUV_MAX_MIP: `${e}.0`
		},
		uniforms: {
			envMap: { value: null },
			roughness: { value: 0 },
			mipInt: { value: 0 }
		},
		vertexShader: Ol(),
		fragmentShader: "\n\n			precision highp float;\n			precision highp int;\n\n			varying vec3 vOutputDirection;\n\n			uniform sampler2D envMap;\n			uniform float roughness;\n			uniform float mipInt;\n\n			#define ENVMAP_TYPE_CUBE_UV\n			#include <cube_uv_reflection_fragment>\n\n			#define PI 3.14159265359\n\n			// Van der Corput radical inverse\n			float radicalInverse_VdC(uint bits) {\n				bits = (bits << 16u) | (bits >> 16u);\n				bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);\n				bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);\n				bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);\n				bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);\n				return float(bits) * 2.3283064365386963e-10; // / 0x100000000\n			}\n\n			// Hammersley sequence\n			vec2 hammersley(uint i, uint N) {\n				return vec2(float(i) / float(N), radicalInverse_VdC(i));\n			}\n\n			// GGX VNDF importance sampling (Eric Heitz 2018)\n			// \"Sampling the GGX Distribution of Visible Normals\"\n			// https://jcgt.org/published/0007/04/01/\n			vec3 importanceSampleGGX_VNDF(vec2 Xi, vec3 V, float roughness) {\n				float alpha = roughness * roughness;\n\n				// Section 4.1: Orthonormal basis\n				vec3 T1 = vec3(1.0, 0.0, 0.0);\n				vec3 T2 = cross(V, T1);\n\n				// Section 4.2: Parameterization of projected area\n				float r = sqrt(Xi.x);\n				float phi = 2.0 * PI * Xi.y;\n				float t1 = r * cos(phi);\n				float t2 = r * sin(phi);\n				float s = 0.5 * (1.0 + V.z);\n				t2 = (1.0 - s) * sqrt(1.0 - t1 * t1) + s * t2;\n\n				// Section 4.3: Reprojection onto hemisphere\n				vec3 Nh = t1 * T1 + t2 * T2 + sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)) * V;\n\n				// Section 3.4: Transform back to ellipsoid configuration\n				return normalize(vec3(alpha * Nh.x, alpha * Nh.y, max(0.0, Nh.z)));\n			}\n\n			void main() {\n				vec3 N = normalize(vOutputDirection);\n				vec3 V = N; // Assume view direction equals normal for pre-filtering\n\n				vec3 prefilteredColor = vec3(0.0);\n				float totalWeight = 0.0;\n\n				// For very low roughness, just sample the environment directly\n				if (roughness < 0.001) {\n					gl_FragColor = vec4(bilinearCubeUV(envMap, N, mipInt), 1.0);\n					return;\n				}\n\n				// Tangent space basis for VNDF sampling\n				vec3 up = abs(N.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);\n				vec3 tangent = normalize(cross(up, N));\n				vec3 bitangent = cross(N, tangent);\n\n				for(uint i = 0u; i < uint(GGX_SAMPLES); i++) {\n					vec2 Xi = hammersley(i, uint(GGX_SAMPLES));\n\n					// For PMREM, V = N, so in tangent space V is always (0, 0, 1)\n					vec3 H_tangent = importanceSampleGGX_VNDF(Xi, vec3(0.0, 0.0, 1.0), roughness);\n\n					// Transform H back to world space\n					vec3 H = normalize(tangent * H_tangent.x + bitangent * H_tangent.y + N * H_tangent.z);\n					vec3 L = normalize(2.0 * dot(V, H) * H - V);\n\n					float NdotL = max(dot(N, L), 0.0);\n\n					if(NdotL > 0.0) {\n						// Sample environment at fixed mip level\n						// VNDF importance sampling handles the distribution filtering\n						vec3 sampleColor = bilinearCubeUV(envMap, L, mipInt);\n\n						// Weight by NdotL for the split-sum approximation\n						// VNDF PDF naturally accounts for the visible microfacet distribution\n						prefilteredColor += sampleColor * NdotL;\n						totalWeight += NdotL;\n					}\n				}\n\n				if (totalWeight > 0.0) {\n					prefilteredColor = prefilteredColor / totalWeight;\n				}\n\n				gl_FragColor = vec4(prefilteredColor, 1.0);\n			}\n		",
		blending: 0,
		depthTest: !1,
		depthWrite: !1
	});
}
function Tl(e, t, n) {
	let r = new Float32Array(dl), i = new q(0, 1, 0);
	return new js({
		name: "SphericalGaussianBlur",
		defines: {
			n: dl,
			CUBEUV_TEXEL_WIDTH: 1 / t,
			CUBEUV_TEXEL_HEIGHT: 1 / n,
			CUBEUV_MAX_MIP: `${e}.0`
		},
		uniforms: {
			envMap: { value: null },
			samples: { value: 1 },
			weights: { value: r },
			latitudinal: { value: !1 },
			dTheta: { value: 0 },
			mipInt: { value: 0 },
			poleAxis: { value: i }
		},
		vertexShader: Ol(),
		fragmentShader: "\n\n			precision mediump float;\n			precision mediump int;\n\n			varying vec3 vOutputDirection;\n\n			uniform sampler2D envMap;\n			uniform int samples;\n			uniform float weights[ n ];\n			uniform bool latitudinal;\n			uniform float dTheta;\n			uniform float mipInt;\n			uniform vec3 poleAxis;\n\n			#define ENVMAP_TYPE_CUBE_UV\n			#include <cube_uv_reflection_fragment>\n\n			vec3 getSample( float theta, vec3 axis ) {\n\n				float cosTheta = cos( theta );\n				// Rodrigues' axis-angle rotation\n				vec3 sampleDirection = vOutputDirection * cosTheta\n					+ cross( axis, vOutputDirection ) * sin( theta )\n					+ axis * dot( axis, vOutputDirection ) * ( 1.0 - cosTheta );\n\n				return bilinearCubeUV( envMap, sampleDirection, mipInt );\n\n			}\n\n			void main() {\n\n				vec3 axis = latitudinal ? poleAxis : cross( poleAxis, vOutputDirection );\n\n				if ( all( equal( axis, vec3( 0.0 ) ) ) ) {\n\n					axis = vec3( vOutputDirection.z, 0.0, - vOutputDirection.x );\n\n				}\n\n				axis = normalize( axis );\n\n				gl_FragColor = vec4( 0.0, 0.0, 0.0, 1.0 );\n				gl_FragColor.rgb += weights[ 0 ] * getSample( 0.0, axis );\n\n				for ( int i = 1; i < n; i++ ) {\n\n					if ( i >= samples ) {\n\n						break;\n\n					}\n\n					float theta = dTheta * float( i );\n					gl_FragColor.rgb += weights[ i ] * getSample( -1.0 * theta, axis );\n					gl_FragColor.rgb += weights[ i ] * getSample( theta, axis );\n\n				}\n\n			}\n		",
		blending: 0,
		depthTest: !1,
		depthWrite: !1
	});
}
function El() {
	return new js({
		name: "EquirectangularToCubeUV",
		uniforms: { envMap: { value: null } },
		vertexShader: Ol(),
		fragmentShader: "\n\n			precision mediump float;\n			precision mediump int;\n\n			varying vec3 vOutputDirection;\n\n			uniform sampler2D envMap;\n\n			#include <common>\n\n			void main() {\n\n				vec3 outputDirection = normalize( vOutputDirection );\n				vec2 uv = equirectUv( outputDirection );\n\n				gl_FragColor = vec4( texture2D ( envMap, uv ).rgb, 1.0 );\n\n			}\n		",
		blending: 0,
		depthTest: !1,
		depthWrite: !1
	});
}
function Dl() {
	return new js({
		name: "CubemapToCubeUV",
		uniforms: {
			envMap: { value: null },
			flipEnvMap: { value: -1 }
		},
		vertexShader: Ol(),
		fragmentShader: "\n\n			precision mediump float;\n			precision mediump int;\n\n			uniform float flipEnvMap;\n\n			varying vec3 vOutputDirection;\n\n			uniform samplerCube envMap;\n\n			void main() {\n\n				gl_FragColor = textureCube( envMap, vec3( flipEnvMap * vOutputDirection.x, vOutputDirection.yz ) );\n\n			}\n		",
		blending: 0,
		depthTest: !1,
		depthWrite: !1
	});
}
function Ol() {
	return "\n\n		precision mediump float;\n		precision mediump int;\n\n		attribute float faceIndex;\n\n		varying vec3 vOutputDirection;\n\n		// RH coordinate system; PMREM face-indexing convention\n		vec3 getDirection( vec2 uv, float face ) {\n\n			uv = 2.0 * uv - 1.0;\n\n			vec3 direction = vec3( uv, 1.0 );\n\n			if ( face == 0.0 ) {\n\n				direction = direction.zyx; // ( 1, v, u ) pos x\n\n			} else if ( face == 1.0 ) {\n\n				direction = direction.xzy;\n				direction.xz *= -1.0; // ( -u, 1, -v ) pos y\n\n			} else if ( face == 2.0 ) {\n\n				direction.x *= -1.0; // ( -u, v, 1 ) pos z\n\n			} else if ( face == 3.0 ) {\n\n				direction = direction.zyx;\n				direction.xz *= -1.0; // ( -1, v, -u ) neg x\n\n			} else if ( face == 4.0 ) {\n\n				direction = direction.xzy;\n				direction.xy *= -1.0; // ( -u, -1, v ) neg y\n\n			} else if ( face == 5.0 ) {\n\n				direction.z *= -1.0; // ( u, v, -1 ) neg z\n\n			}\n\n			return direction;\n\n		}\n\n		void main() {\n\n			vOutputDirection = getDirection( uv, faceIndex );\n			gl_Position = vec4( position, 1.0 );\n\n		}\n	";
}
var kl = class extends hi {
	constructor(e = 1, t = {}) {
		super(e, e, t), this.isWebGLCubeRenderTarget = !0;
		let n = {
			width: e,
			height: e,
			depth: 1
		}, r = [
			n,
			n,
			n,
			n,
			n,
			n
		];
		this.texture = new gs(r), this._setTextureOptions(t), this.texture.isRenderTargetTexture = !0;
	}
	fromEquirectangularTexture(e, t) {
		this.texture.type = t.type, this.texture.colorSpace = t.colorSpace, this.texture.generateMipmaps = t.generateMipmaps, this.texture.minFilter = t.minFilter, this.texture.magFilter = t.magFilter;
		let n = {
			uniforms: { tEquirect: { value: null } },
			vertexShader: "\n\n				varying vec3 vWorldDirection;\n\n				vec3 transformDirection( in vec3 dir, in mat4 matrix ) {\n\n					return normalize( ( matrix * vec4( dir, 0.0 ) ).xyz );\n\n				}\n\n				void main() {\n\n					vWorldDirection = transformDirection( position, modelMatrix );\n\n					#include <begin_vertex>\n					#include <project_vertex>\n\n				}\n			",
			fragmentShader: "\n\n				uniform sampler2D tEquirect;\n\n				varying vec3 vWorldDirection;\n\n				#include <common>\n\n				void main() {\n\n					vec3 direction = normalize( vWorldDirection );\n\n					vec2 sampleUV = equirectUv( direction );\n\n					gl_FragColor = texture2D( tEquirect, sampleUV );\n\n				}\n			"
		}, r = new bs(5, 5, 5), i = new js({
			name: "CubemapFromEquirect",
			uniforms: Cs(n.uniforms),
			vertexShader: n.vertexShader,
			fragmentShader: n.fragmentShader,
			side: 1,
			blending: 0
		});
		i.uniforms.tEquirect.value = t;
		let a = new vo(r, i), o = t.minFilter;
		return t.minFilter === 1008 && (t.minFilter = It), new Oc(1, 10, this).update(e, a), t.minFilter = o, a.geometry.dispose(), a.material.dispose(), this;
	}
	clear(e, t = !0, n = !0, r = !0) {
		let i = e.getRenderTarget();
		for (let i = 0; i < 6; i++) e.setRenderTarget(this, i), e.clear(t, n, r);
		e.setRenderTarget(i);
	}
};
function Al(e) {
	let t = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap(), r = null;
	function i(e, t = !1) {
		return e == null ? null : t ? o(e) : a(e);
	}
	function a(n) {
		if (n && n.isTexture) {
			let r = n.mapping;
			if (r === 303 || r === 304) if (t.has(n)) {
				let e = t.get(n).texture;
				return s(e, n.mapping);
			} else {
				let r = n.image;
				if (r && r.height > 0) {
					let i = new kl(r.height);
					return i.fromEquirectangularTexture(e, n), t.set(n, i), n.addEventListener("dispose", l), s(i.texture, n.mapping);
				} else return null;
			}
		}
		return n;
	}
	function o(t) {
		if (t && t.isTexture) {
			let i = t.mapping, a = i === 303 || i === 304, o = i === 301 || i === 302;
			if (a || o) {
				let i = n.get(t), s = i === void 0 ? 0 : i.texture.pmremVersion;
				if (t.isRenderTargetTexture && t.pmremVersion !== s) return r === null && (r = new bl(e)), i = a ? r.fromEquirectangular(t, i) : r.fromCubemap(t, i), i.texture.pmremVersion = t.pmremVersion, n.set(t, i), i.texture;
				if (i !== void 0) return i.texture;
				{
					let s = t.image;
					return a && s && s.height > 0 || o && s && c(s) ? (r === null && (r = new bl(e)), i = a ? r.fromEquirectangular(t) : r.fromCubemap(t), i.texture.pmremVersion = t.pmremVersion, n.set(t, i), t.addEventListener("dispose", u), i.texture) : null;
				}
			}
		}
		return t;
	}
	function s(e, t) {
		return t === 303 ? e.mapping = 301 : t === 304 && (e.mapping = 302), e;
	}
	function c(e) {
		let t = 0;
		for (let n = 0; n < 6; n++) e[n] !== void 0 && t++;
		return t === 6;
	}
	function l(e) {
		let n = e.target;
		n.removeEventListener("dispose", l);
		let r = t.get(n);
		r !== void 0 && (t.delete(n), r.dispose());
	}
	function u(e) {
		let t = e.target;
		t.removeEventListener("dispose", u);
		let r = n.get(t);
		r !== void 0 && (n.delete(t), r.dispose());
	}
	function d() {
		t = /* @__PURE__ */ new WeakMap(), n = /* @__PURE__ */ new WeakMap(), r !== null && (r.dispose(), r = null);
	}
	return {
		get: i,
		dispose: d
	};
}
function jl(e) {
	let t = {};
	function n(n) {
		if (t[n] !== void 0) return t[n];
		let r = e.getExtension(n);
		return t[n] = r, r;
	}
	return {
		has: function(e) {
			return n(e) !== null;
		},
		init: function() {
			n("EXT_color_buffer_float"), n("WEBGL_clip_cull_distance"), n("OES_texture_float_linear"), n("EXT_color_buffer_half_float"), n("WEBGL_multisampled_render_to_texture"), n("WEBGL_render_shared_exponent");
		},
		get: function(e) {
			let t = n(e);
			return t === null && yr("WebGLRenderer: " + e + " extension not supported."), t;
		}
	};
}
function Ml(e, t, n, r) {
	let i = {}, a = /* @__PURE__ */ new WeakMap();
	function o(e) {
		let s = e.target;
		s.index !== null && t.remove(s.index);
		for (let e in s.attributes) t.remove(s.attributes[e]);
		s.removeEventListener("dispose", o), delete i[s.id];
		let c = a.get(s);
		c && (t.remove(c), a.delete(s)), r.releaseStatesOfGeometry(s), s.isInstancedBufferGeometry === !0 && delete s._maxInstanceCount, n.memory.geometries--;
	}
	function s(e, t) {
		return i[t.id] === !0 ? t : (t.addEventListener("dispose", o), i[t.id] = !0, n.memory.geometries++, t);
	}
	function c(n) {
		let r = n.attributes;
		for (let n in r) t.update(r[n], e.ARRAY_BUFFER);
	}
	function l(e) {
		let n = [], r = e.index, i = e.attributes.position, o = 0;
		if (i === void 0) return;
		if (r !== null) {
			let e = r.array;
			o = r.version;
			for (let t = 0, r = e.length; t < r; t += 3) {
				let r = e[t + 0], i = e[t + 1], a = e[t + 2];
				n.push(r, i, i, a, a, r);
			}
		} else {
			let e = i.array;
			o = i.version;
			for (let t = 0, r = e.length / 3 - 1; t < r; t += 3) {
				let e = t + 0, r = t + 1, i = t + 2;
				n.push(e, r, r, i, i, e);
			}
		}
		let s = new (i.count >= 65535 ? Fa : Pa)(n, 1);
		s.version = o;
		let c = a.get(e);
		c && t.remove(c), a.set(e, s);
	}
	function u(e) {
		let t = a.get(e);
		if (t) {
			let n = e.index;
			n !== null && t.version < n.version && l(e);
		} else l(e);
		return a.get(e);
	}
	return {
		get: s,
		update: c,
		getWireframeAttribute: u
	};
}
function Nl(e, t, n) {
	let r;
	function i(e) {
		r = e;
	}
	let a, o;
	function s(e) {
		a = e.type, o = e.bytesPerElement;
	}
	function c(t, i) {
		e.drawElements(r, i, a, t * o), n.update(i, r, 1);
	}
	function l(t, i, s) {
		s !== 0 && (e.drawElementsInstanced(r, i, a, t * o, s), n.update(i, r, s));
	}
	function u(e, i, o) {
		if (o === 0) return;
		t.get("WEBGL_multi_draw").multiDrawElementsWEBGL(r, i, 0, a, e, 0, o);
		let s = 0;
		for (let e = 0; e < o; e++) s += i[e];
		n.update(s, r, 1);
	}
	this.setMode = i, this.setIndex = s, this.render = c, this.renderInstances = l, this.renderMultiDraw = u;
}
function Pl(e) {
	let t = {
		geometries: 0,
		textures: 0
	}, n = {
		frame: 0,
		calls: 0,
		triangles: 0,
		points: 0,
		lines: 0
	};
	function r(t, r, i) {
		switch (n.calls++, r) {
			case e.TRIANGLES:
				n.triangles += t / 3 * i;
				break;
			case e.LINES:
				n.lines += t / 2 * i;
				break;
			case e.LINE_STRIP:
				n.lines += i * (t - 1);
				break;
			case e.LINE_LOOP:
				n.lines += i * t;
				break;
			case e.POINTS:
				n.points += i * t;
				break;
			default:
				G("WebGLInfo: Unknown draw mode:", r);
				break;
		}
	}
	function i() {
		n.calls = 0, n.triangles = 0, n.points = 0, n.lines = 0;
	}
	return {
		memory: t,
		render: n,
		programs: null,
		autoReset: !0,
		reset: i,
		update: r
	};
}
function Fl(e, t, n) {
	let r = /* @__PURE__ */ new WeakMap(), i = new pi();
	function a(a, o, s) {
		let c = a.morphTargetInfluences, l = o.morphAttributes.position || o.morphAttributes.normal || o.morphAttributes.color, u = l === void 0 ? 0 : l.length, d = r.get(o);
		if (d === void 0 || d.count !== u) {
			d !== void 0 && d.texture.dispose();
			let e = o.morphAttributes.position !== void 0, n = o.morphAttributes.normal !== void 0, a = o.morphAttributes.color !== void 0, s = o.morphAttributes.position || [], c = o.morphAttributes.normal || [], l = o.morphAttributes.color || [], f = 0;
			e === !0 && (f = 1), n === !0 && (f = 2), a === !0 && (f = 3);
			let p = o.attributes.position.count * f, m = 1;
			p > t.maxTextureSize && (m = Math.ceil(p / t.maxTextureSize), p = t.maxTextureSize);
			let h = new Float32Array(p * m * 4 * u), g = new gi(h, p, m, u);
			g.type = Gt, g.needsUpdate = !0;
			let _ = f * 4;
			for (let t = 0; t < u; t++) {
				let r = s[t], o = c[t], u = l[t], d = p * m * 4 * t;
				for (let t = 0; t < r.count; t++) {
					let s = t * _;
					e === !0 && (i.fromBufferAttribute(r, t), h[d + s + 0] = i.x, h[d + s + 1] = i.y, h[d + s + 2] = i.z, h[d + s + 3] = 0), n === !0 && (i.fromBufferAttribute(o, t), h[d + s + 4] = i.x, h[d + s + 5] = i.y, h[d + s + 6] = i.z, h[d + s + 7] = 0), a === !0 && (i.fromBufferAttribute(u, t), h[d + s + 8] = i.x, h[d + s + 9] = i.y, h[d + s + 10] = i.z, h[d + s + 11] = u.itemSize === 4 ? i.w : 1);
				}
			}
			d = {
				count: u,
				texture: g,
				size: new Yr(p, m)
			}, r.set(o, d);
			function v() {
				g.dispose(), r.delete(o), o.removeEventListener("dispose", v);
			}
			o.addEventListener("dispose", v);
		}
		if (a.isInstancedMesh === !0 && a.morphTexture !== null) s.getUniforms().setValue(e, "morphTexture", a.morphTexture, n);
		else {
			let t = 0;
			for (let e = 0; e < c.length; e++) t += c[e];
			let n = o.morphTargetsRelative ? 1 : 1 - t;
			s.getUniforms().setValue(e, "morphTargetBaseInfluence", n), s.getUniforms().setValue(e, "morphTargetInfluences", c);
		}
		s.getUniforms().setValue(e, "morphTargetsTexture", d.texture, n), s.getUniforms().setValue(e, "morphTargetsTextureSize", d.size);
	}
	return { update: a };
}
function Il(e, t, n, r, i) {
	let a = /* @__PURE__ */ new WeakMap();
	function o(r) {
		let o = i.render.frame, s = r.geometry, l = t.get(r, s);
		if (a.get(l) !== o && (t.update(l), a.set(l, o)), r.isInstancedMesh && (r.hasEventListener("dispose", c) === !1 && r.addEventListener("dispose", c), a.get(r) !== o && (n.update(r.instanceMatrix, e.ARRAY_BUFFER), r.instanceColor !== null && n.update(r.instanceColor, e.ARRAY_BUFFER), a.set(r, o))), r.isSkinnedMesh) {
			let e = r.skeleton;
			a.get(e) !== o && (e.update(), a.set(e, o));
		}
		return l;
	}
	function s() {
		a = /* @__PURE__ */ new WeakMap();
	}
	function c(e) {
		let t = e.target;
		t.removeEventListener("dispose", c), r.releaseStatesOfObject(t), n.remove(t.instanceMatrix), t.instanceColor !== null && n.remove(t.instanceColor);
	}
	return {
		update: o,
		dispose: s
	};
}
var Ll = {
	1: "LINEAR_TONE_MAPPING",
	2: "REINHARD_TONE_MAPPING",
	3: "CINEON_TONE_MAPPING",
	4: "ACES_FILMIC_TONE_MAPPING",
	6: "AGX_TONE_MAPPING",
	7: "NEUTRAL_TONE_MAPPING",
	5: "CUSTOM_TONE_MAPPING"
};
function Rl(e, t, n, r, i) {
	let a = new hi(t, n, {
		type: e,
		depthBuffer: r,
		stencilBuffer: i,
		depthTexture: r ? new _s(t, n) : void 0
	}), o = new hi(t, n, {
		type: Kt,
		depthBuffer: !1,
		stencilBuffer: !1
	}), s = new Ja();
	s.setAttribute("position", new Ia([
		-1,
		3,
		0,
		-1,
		-1,
		0,
		3,
		-1,
		0
	], 3)), s.setAttribute("uv", new Ia([
		0,
		2,
		0,
		0,
		2,
		0
	], 2));
	let c = new Ms({
		uniforms: { tDiffuse: { value: null } },
		vertexShader: "\n			precision highp float;\n\n			uniform mat4 modelViewMatrix;\n			uniform mat4 projectionMatrix;\n\n			attribute vec3 position;\n			attribute vec2 uv;\n\n			varying vec2 vUv;\n\n			void main() {\n				vUv = uv;\n				gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );\n			}",
		fragmentShader: "\n			precision highp float;\n\n			uniform sampler2D tDiffuse;\n\n			varying vec2 vUv;\n\n			#include <tonemapping_pars_fragment>\n			#include <colorspace_pars_fragment>\n\n			void main() {\n				gl_FragColor = texture2D( tDiffuse, vUv );\n\n				#ifdef LINEAR_TONE_MAPPING\n					gl_FragColor.rgb = LinearToneMapping( gl_FragColor.rgb );\n				#elif defined( REINHARD_TONE_MAPPING )\n					gl_FragColor.rgb = ReinhardToneMapping( gl_FragColor.rgb );\n				#elif defined( CINEON_TONE_MAPPING )\n					gl_FragColor.rgb = CineonToneMapping( gl_FragColor.rgb );\n				#elif defined( ACES_FILMIC_TONE_MAPPING )\n					gl_FragColor.rgb = ACESFilmicToneMapping( gl_FragColor.rgb );\n				#elif defined( AGX_TONE_MAPPING )\n					gl_FragColor.rgb = AgXToneMapping( gl_FragColor.rgb );\n				#elif defined( NEUTRAL_TONE_MAPPING )\n					gl_FragColor.rgb = NeutralToneMapping( gl_FragColor.rgb );\n				#elif defined( CUSTOM_TONE_MAPPING )\n					gl_FragColor.rgb = CustomToneMapping( gl_FragColor.rgb );\n				#endif\n\n				#ifdef SRGB_TRANSFER\n					gl_FragColor = sRGBTransferOETF( gl_FragColor );\n				#endif\n			}",
		depthTest: !1,
		depthWrite: !1
	}), l = new vo(s, c), u = new Sc(-1, 1, 1, -1, 0, 1), d = null, f = null, p = !1, m, h = null, g = [], _ = !1;
	this.setSize = function(e, t) {
		a.setSize(e, t), o.setSize(e, t);
		for (let n = 0; n < g.length; n++) {
			let r = g[n];
			r.setSize && r.setSize(e, t);
		}
	}, this.setEffects = function(e) {
		g = e, _ = g.length > 0 && g[0].isRenderPass === !0;
		let t = a.width, n = a.height;
		for (let e = 0; e < g.length; e++) {
			let r = g[e];
			r.setSize && r.setSize(t, n);
		}
	}, this.begin = function(e, t) {
		if (p || e.toneMapping === 0 && g.length === 0) return !1;
		if (h = t, t !== null) {
			let e = t.width, n = t.height;
			(a.width !== e || a.height !== n) && this.setSize(e, n);
		}
		return _ === !1 && e.setRenderTarget(a), m = e.toneMapping, e.toneMapping = 0, !0;
	}, this.hasRenderPass = function() {
		return _;
	}, this.end = function(e, t) {
		e.toneMapping = m, p = !0;
		let n = a, r = o;
		for (let i = 0; i < g.length; i++) {
			let a = g[i];
			if (a.enabled !== !1 && (a.render(e, r, n, t), a.needsSwap !== !1)) {
				let e = n;
				n = r, r = e;
			}
		}
		if (d !== e.outputColorSpace || f !== e.toneMapping) {
			d = e.outputColorSpace, f = e.toneMapping, c.defines = {}, Y.getTransfer(d) === "srgb" && (c.defines.SRGB_TRANSFER = "");
			let t = Ll[f];
			t && (c.defines[t] = ""), c.needsUpdate = !0;
		}
		c.uniforms.tDiffuse.value = n.texture, e.setRenderTarget(h), e.render(l, u), h = null, p = !1;
	}, this.isCompositing = function() {
		return p;
	}, this.dispose = function() {
		a.depthTexture && a.depthTexture.dispose(), a.dispose(), o.dispose(), s.dispose(), c.dispose();
	};
}
var zl = /*@__PURE__*/ new fi(), Bl = /*@__PURE__*/ new _s(1, 1), Vl = /*@__PURE__*/ new gi(), Hl = /*@__PURE__*/ new _i(), Ul = /*@__PURE__*/ new gs(), Wl = [], Gl = [], Kl = /* @__PURE__ */ new Float32Array(16), ql = /* @__PURE__ */ new Float32Array(9), Jl = /* @__PURE__ */ new Float32Array(4);
function Yl(e, t, n) {
	let r = e[0];
	if (r <= 0 || r > 0) return e;
	let i = t * n, a = Wl[i];
	if (a === void 0 && (a = new Float32Array(i), Wl[i] = a), t !== 0) {
		r.toArray(a, 0);
		for (let r = 1, i = 0; r !== t; ++r) i += n, e[r].toArray(a, i);
	}
	return a;
}
function Xl(e, t) {
	if (e.length !== t.length) return !1;
	for (let n = 0, r = e.length; n < r; n++) if (e[n] !== t[n]) return !1;
	return !0;
}
function Zl(e, t) {
	for (let n = 0, r = t.length; n < r; n++) e[n] = t[n];
}
function Ql(e, t) {
	let n = Gl[t];
	n === void 0 && (n = new Int32Array(t), Gl[t] = n);
	for (let r = 0; r !== t; ++r) n[r] = e.allocateTextureUnit();
	return n;
}
function $l(e, t) {
	let n = this.cache;
	n[0] !== t && (e.uniform1f(this.addr, t), n[0] = t);
}
function eu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y) && (e.uniform2f(this.addr, t.x, t.y), n[0] = t.x, n[1] = t.y);
	else {
		if (Xl(n, t)) return;
		e.uniform2fv(this.addr, t), Zl(n, t);
	}
}
function tu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y || n[2] !== t.z) && (e.uniform3f(this.addr, t.x, t.y, t.z), n[0] = t.x, n[1] = t.y, n[2] = t.z);
	else if (t.r !== void 0) (n[0] !== t.r || n[1] !== t.g || n[2] !== t.b) && (e.uniform3f(this.addr, t.r, t.g, t.b), n[0] = t.r, n[1] = t.g, n[2] = t.b);
	else {
		if (Xl(n, t)) return;
		e.uniform3fv(this.addr, t), Zl(n, t);
	}
}
function nu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y || n[2] !== t.z || n[3] !== t.w) && (e.uniform4f(this.addr, t.x, t.y, t.z, t.w), n[0] = t.x, n[1] = t.y, n[2] = t.z, n[3] = t.w);
	else {
		if (Xl(n, t)) return;
		e.uniform4fv(this.addr, t), Zl(n, t);
	}
}
function ru(e, t) {
	let n = this.cache, r = t.elements;
	if (r === void 0) {
		if (Xl(n, t)) return;
		e.uniformMatrix2fv(this.addr, !1, t), Zl(n, t);
	} else {
		if (Xl(n, r)) return;
		Jl.set(r), e.uniformMatrix2fv(this.addr, !1, Jl), Zl(n, r);
	}
}
function iu(e, t) {
	let n = this.cache, r = t.elements;
	if (r === void 0) {
		if (Xl(n, t)) return;
		e.uniformMatrix3fv(this.addr, !1, t), Zl(n, t);
	} else {
		if (Xl(n, r)) return;
		ql.set(r), e.uniformMatrix3fv(this.addr, !1, ql), Zl(n, r);
	}
}
function au(e, t) {
	let n = this.cache, r = t.elements;
	if (r === void 0) {
		if (Xl(n, t)) return;
		e.uniformMatrix4fv(this.addr, !1, t), Zl(n, t);
	} else {
		if (Xl(n, r)) return;
		Kl.set(r), e.uniformMatrix4fv(this.addr, !1, Kl), Zl(n, r);
	}
}
function ou(e, t) {
	let n = this.cache;
	n[0] !== t && (e.uniform1i(this.addr, t), n[0] = t);
}
function su(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y) && (e.uniform2i(this.addr, t.x, t.y), n[0] = t.x, n[1] = t.y);
	else {
		if (Xl(n, t)) return;
		e.uniform2iv(this.addr, t), Zl(n, t);
	}
}
function cu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y || n[2] !== t.z) && (e.uniform3i(this.addr, t.x, t.y, t.z), n[0] = t.x, n[1] = t.y, n[2] = t.z);
	else {
		if (Xl(n, t)) return;
		e.uniform3iv(this.addr, t), Zl(n, t);
	}
}
function lu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y || n[2] !== t.z || n[3] !== t.w) && (e.uniform4i(this.addr, t.x, t.y, t.z, t.w), n[0] = t.x, n[1] = t.y, n[2] = t.z, n[3] = t.w);
	else {
		if (Xl(n, t)) return;
		e.uniform4iv(this.addr, t), Zl(n, t);
	}
}
function uu(e, t) {
	let n = this.cache;
	n[0] !== t && (e.uniform1ui(this.addr, t), n[0] = t);
}
function du(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y) && (e.uniform2ui(this.addr, t.x, t.y), n[0] = t.x, n[1] = t.y);
	else {
		if (Xl(n, t)) return;
		e.uniform2uiv(this.addr, t), Zl(n, t);
	}
}
function fu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y || n[2] !== t.z) && (e.uniform3ui(this.addr, t.x, t.y, t.z), n[0] = t.x, n[1] = t.y, n[2] = t.z);
	else {
		if (Xl(n, t)) return;
		e.uniform3uiv(this.addr, t), Zl(n, t);
	}
}
function pu(e, t) {
	let n = this.cache;
	if (t.x !== void 0) (n[0] !== t.x || n[1] !== t.y || n[2] !== t.z || n[3] !== t.w) && (e.uniform4ui(this.addr, t.x, t.y, t.z, t.w), n[0] = t.x, n[1] = t.y, n[2] = t.z, n[3] = t.w);
	else {
		if (Xl(n, t)) return;
		e.uniform4uiv(this.addr, t), Zl(n, t);
	}
}
function mu(e, t, n) {
	let r = this.cache, i = n.allocateTextureUnit();
	r[0] !== i && (e.uniform1i(this.addr, i), r[0] = i);
	let a;
	this.type === e.SAMPLER_2D_SHADOW ? (Bl.compareFunction = n.isReversedDepthBuffer() ? 518 : 515, a = Bl) : a = zl, n.setTexture2D(t || a, i);
}
function hu(e, t, n) {
	let r = this.cache, i = n.allocateTextureUnit();
	r[0] !== i && (e.uniform1i(this.addr, i), r[0] = i), n.setTexture3D(t || Hl, i);
}
function gu(e, t, n) {
	let r = this.cache, i = n.allocateTextureUnit();
	r[0] !== i && (e.uniform1i(this.addr, i), r[0] = i), n.setTextureCube(t || Ul, i);
}
function _u(e, t, n) {
	let r = this.cache, i = n.allocateTextureUnit();
	r[0] !== i && (e.uniform1i(this.addr, i), r[0] = i), n.setTexture2DArray(t || Vl, i);
}
function vu(e) {
	switch (e) {
		case 5126: return $l;
		case 35664: return eu;
		case 35665: return tu;
		case 35666: return nu;
		case 35674: return ru;
		case 35675: return iu;
		case 35676: return au;
		case 5124:
		case 35670: return ou;
		case 35667:
		case 35671: return su;
		case 35668:
		case 35672: return cu;
		case 35669:
		case 35673: return lu;
		case 5125: return uu;
		case 36294: return du;
		case 36295: return fu;
		case 36296: return pu;
		case 35678:
		case 36198:
		case 36298:
		case 36306:
		case 35682: return mu;
		case 35679:
		case 36299:
		case 36307: return hu;
		case 35680:
		case 36300:
		case 36308:
		case 36293: return gu;
		case 36289:
		case 36303:
		case 36311:
		case 36292: return _u;
	}
}
function yu(e, t) {
	e.uniform1fv(this.addr, t);
}
function bu(e, t) {
	let n = Yl(t, this.size, 2);
	e.uniform2fv(this.addr, n);
}
function xu(e, t) {
	let n = Yl(t, this.size, 3);
	e.uniform3fv(this.addr, n);
}
function Su(e, t) {
	let n = Yl(t, this.size, 4);
	e.uniform4fv(this.addr, n);
}
function Cu(e, t) {
	let n = Yl(t, this.size, 4);
	e.uniformMatrix2fv(this.addr, !1, n);
}
function wu(e, t) {
	let n = Yl(t, this.size, 9);
	e.uniformMatrix3fv(this.addr, !1, n);
}
function Tu(e, t) {
	let n = Yl(t, this.size, 16);
	e.uniformMatrix4fv(this.addr, !1, n);
}
function Eu(e, t) {
	e.uniform1iv(this.addr, t);
}
function Du(e, t) {
	e.uniform2iv(this.addr, t);
}
function Ou(e, t) {
	e.uniform3iv(this.addr, t);
}
function ku(e, t) {
	e.uniform4iv(this.addr, t);
}
function Au(e, t) {
	e.uniform1uiv(this.addr, t);
}
function ju(e, t) {
	e.uniform2uiv(this.addr, t);
}
function Mu(e, t) {
	e.uniform3uiv(this.addr, t);
}
function Nu(e, t) {
	e.uniform4uiv(this.addr, t);
}
function Pu(e, t, n) {
	let r = this.cache, i = t.length, a = Ql(n, i);
	Xl(r, a) || (e.uniform1iv(this.addr, a), Zl(r, a));
	let o;
	o = this.type === e.SAMPLER_2D_SHADOW ? Bl : zl;
	for (let e = 0; e !== i; ++e) n.setTexture2D(t[e] || o, a[e]);
}
function Fu(e, t, n) {
	let r = this.cache, i = t.length, a = Ql(n, i);
	Xl(r, a) || (e.uniform1iv(this.addr, a), Zl(r, a));
	for (let e = 0; e !== i; ++e) n.setTexture3D(t[e] || Hl, a[e]);
}
function Iu(e, t, n) {
	let r = this.cache, i = t.length, a = Ql(n, i);
	Xl(r, a) || (e.uniform1iv(this.addr, a), Zl(r, a));
	for (let e = 0; e !== i; ++e) n.setTextureCube(t[e] || Ul, a[e]);
}
function Lu(e, t, n) {
	let r = this.cache, i = t.length, a = Ql(n, i);
	Xl(r, a) || (e.uniform1iv(this.addr, a), Zl(r, a));
	for (let e = 0; e !== i; ++e) n.setTexture2DArray(t[e] || Vl, a[e]);
}
function Ru(e) {
	switch (e) {
		case 5126: return yu;
		case 35664: return bu;
		case 35665: return xu;
		case 35666: return Su;
		case 35674: return Cu;
		case 35675: return wu;
		case 35676: return Tu;
		case 5124:
		case 35670: return Eu;
		case 35667:
		case 35671: return Du;
		case 35668:
		case 35672: return Ou;
		case 35669:
		case 35673: return ku;
		case 5125: return Au;
		case 36294: return ju;
		case 36295: return Mu;
		case 36296: return Nu;
		case 35678:
		case 36198:
		case 36298:
		case 36306:
		case 35682: return Pu;
		case 35679:
		case 36299:
		case 36307: return Fu;
		case 35680:
		case 36300:
		case 36308:
		case 36293: return Iu;
		case 36289:
		case 36303:
		case 36311:
		case 36292: return Lu;
	}
}
var zu = class {
	constructor(e, t, n) {
		this.id = e, this.addr = n, this.cache = [], this.type = t.type, this.setValue = vu(t.type);
	}
}, Bu = class {
	constructor(e, t, n) {
		this.id = e, this.addr = n, this.cache = [], this.type = t.type, this.size = t.size, this.setValue = Ru(t.type);
	}
}, Vu = class {
	constructor(e) {
		this.id = e, this.seq = [], this.map = {};
	}
	setValue(e, t, n) {
		let r = this.seq;
		for (let i = 0, a = r.length; i !== a; ++i) {
			let a = r[i];
			a.setValue(e, t[a.id], n);
		}
	}
}, Hu = /(\w+)(\])?(\[|\.)?/g;
function Uu(e, t) {
	e.seq.push(t), e.map[t.id] = t;
}
function Wu(e, t, n) {
	let r = e.name, i = r.length;
	for (Hu.lastIndex = 0;;) {
		let a = Hu.exec(r), o = Hu.lastIndex, s = a[1], c = a[2] === "]", l = a[3];
		if (c && (s |= 0), l === void 0 || l === "[" && o + 2 === i) {
			Uu(n, l === void 0 ? new zu(s, e, t) : new Bu(s, e, t));
			break;
		} else {
			let e = n.map[s];
			e === void 0 && (e = new Vu(s), Uu(n, e)), n = e;
		}
	}
}
var Gu = class {
	constructor(e, t) {
		this.seq = [], this.map = {};
		let n = e.getProgramParameter(t, e.ACTIVE_UNIFORMS);
		for (let r = 0; r < n; ++r) {
			let n = e.getActiveUniform(t, r);
			Wu(n, e.getUniformLocation(t, n.name), this);
		}
		let r = [], i = [];
		for (let t of this.seq) t.type === e.SAMPLER_2D_SHADOW || t.type === e.SAMPLER_CUBE_SHADOW || t.type === e.SAMPLER_2D_ARRAY_SHADOW ? r.push(t) : i.push(t);
		r.length > 0 && (this.seq = r.concat(i));
	}
	setValue(e, t, n, r) {
		let i = this.map[t];
		i !== void 0 && i.setValue(e, n, r);
	}
	setOptional(e, t, n) {
		let r = t[n];
		r !== void 0 && this.setValue(e, n, r);
	}
	static upload(e, t, n, r) {
		for (let i = 0, a = t.length; i !== a; ++i) {
			let a = t[i], o = n[a.id];
			o.needsUpdate !== !1 && a.setValue(e, o.value, r);
		}
	}
	static seqWithValue(e, t) {
		let n = [];
		for (let r = 0, i = e.length; r !== i; ++r) {
			let i = e[r];
			i.id in t && n.push(i);
		}
		return n;
	}
};
function Ku(e, t, n) {
	let r = e.createShader(t);
	return e.shaderSource(r, n), e.compileShader(r), r;
}
var qu = 37297, Ju = 0;
function Yu(e, t) {
	let n = e.split("\n"), r = [], i = Math.max(t - 6, 0), a = Math.min(t + 6, n.length);
	for (let e = i; e < a; e++) {
		let i = e + 1;
		r.push(`${i === t ? ">" : " "} ${i}: ${n[e]}`);
	}
	return r.join("\n");
}
var Xu = /*@__PURE__*/ new J();
function Zu(e) {
	Y._getMatrix(Xu, Y.workingColorSpace, e);
	let t = `mat3( ${Xu.elements.map((e) => e.toFixed(4))} )`;
	switch (Y.getTransfer(e)) {
		case or: return [t, "LinearTransferOETF"];
		case sr: return [t, "sRGBTransferOETF"];
		default: return W("WebGLProgram: Unsupported color space: ", e), [t, "LinearTransferOETF"];
	}
}
function Qu(e, t, n) {
	let r = e.getShaderParameter(t, e.COMPILE_STATUS), i = (e.getShaderInfoLog(t) || "").trim();
	if (r && i === "") return "";
	let a = /ERROR: 0:(\d+)/.exec(i);
	if (a) {
		let r = parseInt(a[1]);
		return n.toUpperCase() + "\n\n" + i + "\n\n" + Yu(e.getShaderSource(t), r);
	} else return i;
}
function $u(e, t) {
	let n = Zu(t);
	return [
		`vec4 ${e}( vec4 value ) {`,
		`	return ${n[1]}( vec4( value.rgb * ${n[0]}, value.a ) );`,
		"}"
	].join("\n");
}
var ed = {
	1: "Linear",
	2: "Reinhard",
	3: "Cineon",
	4: "ACESFilmic",
	6: "AgX",
	7: "Neutral",
	5: "Custom"
};
function td(e, t) {
	let n = ed[t];
	return n === void 0 ? (W("WebGLProgram: Unsupported toneMapping:", t), "vec3 " + e + "( vec3 color ) { return LinearToneMapping( color ); }") : "vec3 " + e + "( vec3 color ) { return " + n + "ToneMapping( color ); }";
}
var nd = /*@__PURE__*/ new q();
function rd() {
	return Y.getLuminanceCoefficients(nd), [
		"float luminance( const in vec3 rgb ) {",
		`	const vec3 weights = vec3( ${nd.x.toFixed(4)}, ${nd.y.toFixed(4)}, ${nd.z.toFixed(4)} );`,
		"	return dot( weights, rgb );",
		"}"
	].join("\n");
}
function id(e) {
	return [e.extensionClipCullDistance ? "#extension GL_ANGLE_clip_cull_distance : require" : "", e.extensionMultiDraw ? "#extension GL_ANGLE_multi_draw : require" : ""].filter(sd).join("\n");
}
function ad(e) {
	let t = [];
	for (let n in e) {
		let r = e[n];
		r !== !1 && t.push("#define " + n + " " + r);
	}
	return t.join("\n");
}
function od(e, t) {
	let n = {}, r = e.getProgramParameter(t, e.ACTIVE_ATTRIBUTES);
	for (let i = 0; i < r; i++) {
		let r = e.getActiveAttrib(t, i), a = r.name, o = 1;
		r.type === e.FLOAT_MAT2 && (o = 2), r.type === e.FLOAT_MAT3 && (o = 3), r.type === e.FLOAT_MAT4 && (o = 4), n[a] = {
			type: r.type,
			location: e.getAttribLocation(t, a),
			locationSize: o
		};
	}
	return n;
}
function sd(e) {
	return e !== "";
}
function cd(e, t) {
	let n = t.numSpotLightShadows + t.numSpotLightMaps - t.numSpotLightShadowsWithMaps;
	return e.replace(/NUM_DIR_LIGHTS/g, t.numDirLights).replace(/NUM_SPOT_LIGHTS/g, t.numSpotLights).replace(/NUM_SPOT_LIGHT_MAPS/g, t.numSpotLightMaps).replace(/NUM_SPOT_LIGHT_COORDS/g, n).replace(/NUM_RECT_AREA_LIGHTS/g, t.numRectAreaLights).replace(/NUM_POINT_LIGHTS/g, t.numPointLights).replace(/NUM_HEMI_LIGHTS/g, t.numHemiLights).replace(/NUM_DIR_LIGHT_SHADOWS/g, t.numDirLightShadows).replace(/NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS/g, t.numSpotLightShadowsWithMaps).replace(/NUM_SPOT_LIGHT_SHADOWS/g, t.numSpotLightShadows).replace(/NUM_POINT_LIGHT_SHADOWS/g, t.numPointLightShadows);
}
function ld(e, t) {
	return e.replace(/NUM_CLIPPING_PLANES/g, t.numClippingPlanes).replace(/UNION_CLIPPING_PLANES/g, t.numClippingPlanes - t.numClipIntersection);
}
var ud = /^[ \t]*#include +<([\w\d./]+)>/gm;
function dd(e) {
	return e.replace(ud, pd);
}
var fd = /* @__PURE__ */ new Map();
function pd(e, t) {
	let n = Z[t];
	if (n === void 0) {
		let e = fd.get(t);
		if (e !== void 0) n = Z[e], W("WebGLRenderer: Shader chunk \"%s\" has been deprecated. Use \"%s\" instead.", t, e);
		else throw Error("Can not resolve #include <" + t + ">");
	}
	return dd(n);
}
var md = /#pragma unroll_loop_start\s+for\s*\(\s*int\s+i\s*=\s*(\d+)\s*;\s*i\s*<\s*(\d+)\s*;\s*i\s*\+\+\s*\)\s*{([\s\S]+?)}\s+#pragma unroll_loop_end/g;
function hd(e) {
	return e.replace(md, gd);
}
function gd(e, t, n, r) {
	let i = "";
	for (let e = parseInt(t); e < parseInt(n); e++) i += r.replace(/\[\s*i\s*\]/g, "[ " + e + " ]").replace(/UNROLLED_LOOP_INDEX/g, e);
	return i;
}
function _d(e) {
	let t = `precision ${e.precision} float;
	precision ${e.precision} int;
	precision ${e.precision} sampler2D;
	precision ${e.precision} samplerCube;
	precision ${e.precision} sampler3D;
	precision ${e.precision} sampler2DArray;
	precision ${e.precision} sampler2DShadow;
	precision ${e.precision} samplerCubeShadow;
	precision ${e.precision} sampler2DArrayShadow;
	precision ${e.precision} isampler2D;
	precision ${e.precision} isampler3D;
	precision ${e.precision} isamplerCube;
	precision ${e.precision} isampler2DArray;
	precision ${e.precision} usampler2D;
	precision ${e.precision} usampler3D;
	precision ${e.precision} usamplerCube;
	precision ${e.precision} usampler2DArray;
	`;
	return e.precision === "highp" ? t += "\n#define HIGH_PRECISION" : e.precision === "mediump" ? t += "\n#define MEDIUM_PRECISION" : e.precision === "lowp" && (t += "\n#define LOW_PRECISION"), t;
}
var vd = {
	1: "SHADOWMAP_TYPE_PCF",
	3: "SHADOWMAP_TYPE_VSM"
};
function yd(e) {
	return vd[e.shadowMapType] || "SHADOWMAP_TYPE_BASIC";
}
var bd = {
	301: "ENVMAP_TYPE_CUBE",
	302: "ENVMAP_TYPE_CUBE",
	306: "ENVMAP_TYPE_CUBE_UV"
};
function xd(e) {
	return e.envMap === !1 ? "ENVMAP_TYPE_CUBE" : bd[e.envMapMode] || "ENVMAP_TYPE_CUBE";
}
var Sd = { 302: "ENVMAP_MODE_REFRACTION" };
function Cd(e) {
	return e.envMap === !1 ? "ENVMAP_MODE_REFLECTION" : Sd[e.envMapMode] || "ENVMAP_MODE_REFLECTION";
}
var wd = {
	0: "ENVMAP_BLENDING_MULTIPLY",
	1: "ENVMAP_BLENDING_MIX",
	2: "ENVMAP_BLENDING_ADD"
};
function Td(e) {
	return e.envMap === !1 ? "ENVMAP_BLENDING_NONE" : wd[e.combine] || "ENVMAP_BLENDING_NONE";
}
function Ed(e) {
	let t = e.envMapCubeUVHeight;
	if (t === null) return null;
	let n = Math.log2(t) - 2, r = 1 / t;
	return {
		texelWidth: 1 / (3 * Math.max(2 ** n, 112)),
		texelHeight: r,
		maxMip: n
	};
}
function Dd(e, t, n, r) {
	let i = e.getContext(), a = n.defines, o = n.vertexShader, s = n.fragmentShader, c = yd(n), l = xd(n), u = Cd(n), d = Td(n), f = Ed(n), p = id(n), m = ad(a), h = i.createProgram(), g, _, v = n.glslVersion ? "#version " + n.glslVersion + "\n" : "";
	n.isRawShaderMaterial ? (g = [
		"#define SHADER_TYPE " + n.shaderType,
		"#define SHADER_NAME " + n.shaderName,
		m
	].filter(sd).join("\n"), g.length > 0 && (g += "\n"), _ = [
		"#define SHADER_TYPE " + n.shaderType,
		"#define SHADER_NAME " + n.shaderName,
		m
	].filter(sd).join("\n"), _.length > 0 && (_ += "\n")) : (g = [
		_d(n),
		"#define SHADER_TYPE " + n.shaderType,
		"#define SHADER_NAME " + n.shaderName,
		m,
		n.extensionClipCullDistance ? "#define USE_CLIP_DISTANCE" : "",
		n.batching ? "#define USE_BATCHING" : "",
		n.batchingColor ? "#define USE_BATCHING_COLOR" : "",
		n.instancing ? "#define USE_INSTANCING" : "",
		n.instancingColor ? "#define USE_INSTANCING_COLOR" : "",
		n.instancingMorph ? "#define USE_INSTANCING_MORPH" : "",
		n.useFog && n.fog ? "#define USE_FOG" : "",
		n.useFog && n.fogExp2 ? "#define FOG_EXP2" : "",
		n.map ? "#define USE_MAP" : "",
		n.envMap ? "#define USE_ENVMAP" : "",
		n.envMap ? "#define " + u : "",
		n.lightMap ? "#define USE_LIGHTMAP" : "",
		n.aoMap ? "#define USE_AOMAP" : "",
		n.bumpMap ? "#define USE_BUMPMAP" : "",
		n.normalMap ? "#define USE_NORMALMAP" : "",
		n.normalMapObjectSpace ? "#define USE_NORMALMAP_OBJECTSPACE" : "",
		n.normalMapTangentSpace ? "#define USE_NORMALMAP_TANGENTSPACE" : "",
		n.displacementMap ? "#define USE_DISPLACEMENTMAP" : "",
		n.emissiveMap ? "#define USE_EMISSIVEMAP" : "",
		n.anisotropy ? "#define USE_ANISOTROPY" : "",
		n.anisotropyMap ? "#define USE_ANISOTROPYMAP" : "",
		n.clearcoatMap ? "#define USE_CLEARCOATMAP" : "",
		n.clearcoatRoughnessMap ? "#define USE_CLEARCOAT_ROUGHNESSMAP" : "",
		n.clearcoatNormalMap ? "#define USE_CLEARCOAT_NORMALMAP" : "",
		n.iridescenceMap ? "#define USE_IRIDESCENCEMAP" : "",
		n.iridescenceThicknessMap ? "#define USE_IRIDESCENCE_THICKNESSMAP" : "",
		n.specularMap ? "#define USE_SPECULARMAP" : "",
		n.specularColorMap ? "#define USE_SPECULAR_COLORMAP" : "",
		n.specularIntensityMap ? "#define USE_SPECULAR_INTENSITYMAP" : "",
		n.roughnessMap ? "#define USE_ROUGHNESSMAP" : "",
		n.metalnessMap ? "#define USE_METALNESSMAP" : "",
		n.alphaMap ? "#define USE_ALPHAMAP" : "",
		n.alphaHash ? "#define USE_ALPHAHASH" : "",
		n.transmission ? "#define USE_TRANSMISSION" : "",
		n.transmissionMap ? "#define USE_TRANSMISSIONMAP" : "",
		n.thicknessMap ? "#define USE_THICKNESSMAP" : "",
		n.sheenColorMap ? "#define USE_SHEEN_COLORMAP" : "",
		n.sheenRoughnessMap ? "#define USE_SHEEN_ROUGHNESSMAP" : "",
		n.mapUv ? "#define MAP_UV " + n.mapUv : "",
		n.alphaMapUv ? "#define ALPHAMAP_UV " + n.alphaMapUv : "",
		n.lightMapUv ? "#define LIGHTMAP_UV " + n.lightMapUv : "",
		n.aoMapUv ? "#define AOMAP_UV " + n.aoMapUv : "",
		n.emissiveMapUv ? "#define EMISSIVEMAP_UV " + n.emissiveMapUv : "",
		n.bumpMapUv ? "#define BUMPMAP_UV " + n.bumpMapUv : "",
		n.normalMapUv ? "#define NORMALMAP_UV " + n.normalMapUv : "",
		n.displacementMapUv ? "#define DISPLACEMENTMAP_UV " + n.displacementMapUv : "",
		n.metalnessMapUv ? "#define METALNESSMAP_UV " + n.metalnessMapUv : "",
		n.roughnessMapUv ? "#define ROUGHNESSMAP_UV " + n.roughnessMapUv : "",
		n.anisotropyMapUv ? "#define ANISOTROPYMAP_UV " + n.anisotropyMapUv : "",
		n.clearcoatMapUv ? "#define CLEARCOATMAP_UV " + n.clearcoatMapUv : "",
		n.clearcoatNormalMapUv ? "#define CLEARCOAT_NORMALMAP_UV " + n.clearcoatNormalMapUv : "",
		n.clearcoatRoughnessMapUv ? "#define CLEARCOAT_ROUGHNESSMAP_UV " + n.clearcoatRoughnessMapUv : "",
		n.iridescenceMapUv ? "#define IRIDESCENCEMAP_UV " + n.iridescenceMapUv : "",
		n.iridescenceThicknessMapUv ? "#define IRIDESCENCE_THICKNESSMAP_UV " + n.iridescenceThicknessMapUv : "",
		n.sheenColorMapUv ? "#define SHEEN_COLORMAP_UV " + n.sheenColorMapUv : "",
		n.sheenRoughnessMapUv ? "#define SHEEN_ROUGHNESSMAP_UV " + n.sheenRoughnessMapUv : "",
		n.specularMapUv ? "#define SPECULARMAP_UV " + n.specularMapUv : "",
		n.specularColorMapUv ? "#define SPECULAR_COLORMAP_UV " + n.specularColorMapUv : "",
		n.specularIntensityMapUv ? "#define SPECULAR_INTENSITYMAP_UV " + n.specularIntensityMapUv : "",
		n.transmissionMapUv ? "#define TRANSMISSIONMAP_UV " + n.transmissionMapUv : "",
		n.thicknessMapUv ? "#define THICKNESSMAP_UV " + n.thicknessMapUv : "",
		n.vertexTangents && n.flatShading === !1 ? "#define USE_TANGENT" : "",
		n.vertexNormals ? "#define HAS_NORMAL" : "",
		n.vertexColors ? "#define USE_COLOR" : "",
		n.vertexAlphas ? "#define USE_COLOR_ALPHA" : "",
		n.vertexUv1s ? "#define USE_UV1" : "",
		n.vertexUv2s ? "#define USE_UV2" : "",
		n.vertexUv3s ? "#define USE_UV3" : "",
		n.pointsUvs ? "#define USE_POINTS_UV" : "",
		n.flatShading ? "#define FLAT_SHADED" : "",
		n.skinning ? "#define USE_SKINNING" : "",
		n.morphTargets ? "#define USE_MORPHTARGETS" : "",
		n.morphNormals && n.flatShading === !1 ? "#define USE_MORPHNORMALS" : "",
		n.morphColors ? "#define USE_MORPHCOLORS" : "",
		n.morphTargetsCount > 0 ? "#define MORPHTARGETS_TEXTURE_STRIDE " + n.morphTextureStride : "",
		n.morphTargetsCount > 0 ? "#define MORPHTARGETS_COUNT " + n.morphTargetsCount : "",
		n.doubleSided ? "#define DOUBLE_SIDED" : "",
		n.flipSided ? "#define FLIP_SIDED" : "",
		n.shadowMapEnabled ? "#define USE_SHADOWMAP" : "",
		n.shadowMapEnabled ? "#define " + c : "",
		n.sizeAttenuation ? "#define USE_SIZEATTENUATION" : "",
		n.numLightProbes > 0 ? "#define USE_LIGHT_PROBES" : "",
		n.logarithmicDepthBuffer ? "#define USE_LOGARITHMIC_DEPTH_BUFFER" : "",
		n.reversedDepthBuffer ? "#define USE_REVERSED_DEPTH_BUFFER" : "",
		"uniform mat4 modelMatrix;",
		"uniform mat4 modelViewMatrix;",
		"uniform mat4 projectionMatrix;",
		"uniform mat4 viewMatrix;",
		"uniform mat3 normalMatrix;",
		"uniform vec3 cameraPosition;",
		"uniform bool isOrthographic;",
		"#ifdef USE_INSTANCING",
		"	attribute mat4 instanceMatrix;",
		"#endif",
		"#ifdef USE_INSTANCING_COLOR",
		"	attribute vec3 instanceColor;",
		"#endif",
		"#ifdef USE_INSTANCING_MORPH",
		"	uniform sampler2D morphTexture;",
		"#endif",
		"attribute vec3 position;",
		"attribute vec3 normal;",
		"attribute vec2 uv;",
		"#ifdef USE_UV1",
		"	attribute vec2 uv1;",
		"#endif",
		"#ifdef USE_UV2",
		"	attribute vec2 uv2;",
		"#endif",
		"#ifdef USE_UV3",
		"	attribute vec2 uv3;",
		"#endif",
		"#ifdef USE_TANGENT",
		"	attribute vec4 tangent;",
		"#endif",
		"#if defined( USE_COLOR_ALPHA )",
		"	attribute vec4 color;",
		"#elif defined( USE_COLOR )",
		"	attribute vec3 color;",
		"#endif",
		"#ifdef USE_SKINNING",
		"	attribute vec4 skinIndex;",
		"	attribute vec4 skinWeight;",
		"#endif",
		"\n"
	].filter(sd).join("\n"), _ = [
		_d(n),
		"#define SHADER_TYPE " + n.shaderType,
		"#define SHADER_NAME " + n.shaderName,
		m,
		n.useFog && n.fog ? "#define USE_FOG" : "",
		n.useFog && n.fogExp2 ? "#define FOG_EXP2" : "",
		n.alphaToCoverage ? "#define ALPHA_TO_COVERAGE" : "",
		n.map ? "#define USE_MAP" : "",
		n.matcap ? "#define USE_MATCAP" : "",
		n.envMap ? "#define USE_ENVMAP" : "",
		n.envMap ? "#define " + l : "",
		n.envMap ? "#define " + u : "",
		n.envMap ? "#define " + d : "",
		f ? "#define CUBEUV_TEXEL_WIDTH " + f.texelWidth : "",
		f ? "#define CUBEUV_TEXEL_HEIGHT " + f.texelHeight : "",
		f ? "#define CUBEUV_MAX_MIP " + f.maxMip + ".0" : "",
		n.lightMap ? "#define USE_LIGHTMAP" : "",
		n.aoMap ? "#define USE_AOMAP" : "",
		n.bumpMap ? "#define USE_BUMPMAP" : "",
		n.normalMap ? "#define USE_NORMALMAP" : "",
		n.normalMapObjectSpace ? "#define USE_NORMALMAP_OBJECTSPACE" : "",
		n.normalMapTangentSpace ? "#define USE_NORMALMAP_TANGENTSPACE" : "",
		n.packedNormalMap ? "#define USE_PACKED_NORMALMAP" : "",
		n.emissiveMap ? "#define USE_EMISSIVEMAP" : "",
		n.anisotropy ? "#define USE_ANISOTROPY" : "",
		n.anisotropyMap ? "#define USE_ANISOTROPYMAP" : "",
		n.clearcoat ? "#define USE_CLEARCOAT" : "",
		n.clearcoatMap ? "#define USE_CLEARCOATMAP" : "",
		n.clearcoatRoughnessMap ? "#define USE_CLEARCOAT_ROUGHNESSMAP" : "",
		n.clearcoatNormalMap ? "#define USE_CLEARCOAT_NORMALMAP" : "",
		n.dispersion ? "#define USE_DISPERSION" : "",
		n.iridescence ? "#define USE_IRIDESCENCE" : "",
		n.iridescenceMap ? "#define USE_IRIDESCENCEMAP" : "",
		n.iridescenceThicknessMap ? "#define USE_IRIDESCENCE_THICKNESSMAP" : "",
		n.specularMap ? "#define USE_SPECULARMAP" : "",
		n.specularColorMap ? "#define USE_SPECULAR_COLORMAP" : "",
		n.specularIntensityMap ? "#define USE_SPECULAR_INTENSITYMAP" : "",
		n.roughnessMap ? "#define USE_ROUGHNESSMAP" : "",
		n.metalnessMap ? "#define USE_METALNESSMAP" : "",
		n.alphaMap ? "#define USE_ALPHAMAP" : "",
		n.alphaTest ? "#define USE_ALPHATEST" : "",
		n.alphaHash ? "#define USE_ALPHAHASH" : "",
		n.sheen ? "#define USE_SHEEN" : "",
		n.sheenColorMap ? "#define USE_SHEEN_COLORMAP" : "",
		n.sheenRoughnessMap ? "#define USE_SHEEN_ROUGHNESSMAP" : "",
		n.transmission ? "#define USE_TRANSMISSION" : "",
		n.transmissionMap ? "#define USE_TRANSMISSIONMAP" : "",
		n.thicknessMap ? "#define USE_THICKNESSMAP" : "",
		n.vertexTangents && n.flatShading === !1 ? "#define USE_TANGENT" : "",
		n.vertexColors || n.instancingColor ? "#define USE_COLOR" : "",
		n.vertexAlphas || n.batchingColor ? "#define USE_COLOR_ALPHA" : "",
		n.vertexUv1s ? "#define USE_UV1" : "",
		n.vertexUv2s ? "#define USE_UV2" : "",
		n.vertexUv3s ? "#define USE_UV3" : "",
		n.pointsUvs ? "#define USE_POINTS_UV" : "",
		n.gradientMap ? "#define USE_GRADIENTMAP" : "",
		n.flatShading ? "#define FLAT_SHADED" : "",
		n.doubleSided ? "#define DOUBLE_SIDED" : "",
		n.flipSided ? "#define FLIP_SIDED" : "",
		n.shadowMapEnabled ? "#define USE_SHADOWMAP" : "",
		n.shadowMapEnabled ? "#define " + c : "",
		n.premultipliedAlpha ? "#define PREMULTIPLIED_ALPHA" : "",
		n.numLightProbes > 0 ? "#define USE_LIGHT_PROBES" : "",
		n.numLightProbeGrids > 0 ? "#define USE_LIGHT_PROBES_GRID" : "",
		n.decodeVideoTexture ? "#define DECODE_VIDEO_TEXTURE" : "",
		n.decodeVideoTextureEmissive ? "#define DECODE_VIDEO_TEXTURE_EMISSIVE" : "",
		n.logarithmicDepthBuffer ? "#define USE_LOGARITHMIC_DEPTH_BUFFER" : "",
		n.reversedDepthBuffer ? "#define USE_REVERSED_DEPTH_BUFFER" : "",
		"uniform mat4 viewMatrix;",
		"uniform vec3 cameraPosition;",
		"uniform bool isOrthographic;",
		n.toneMapping === 0 ? "" : "#define TONE_MAPPING",
		n.toneMapping === 0 ? "" : Z.tonemapping_pars_fragment,
		n.toneMapping === 0 ? "" : td("toneMapping", n.toneMapping),
		n.dithering ? "#define DITHERING" : "",
		n.opaque ? "#define OPAQUE" : "",
		Z.colorspace_pars_fragment,
		$u("linearToOutputTexel", n.outputColorSpace),
		rd(),
		n.useDepthPacking ? "#define DEPTH_PACKING " + n.depthPacking : "",
		"\n"
	].filter(sd).join("\n")), o = dd(o), o = cd(o, n), o = ld(o, n), s = dd(s), s = cd(s, n), s = ld(s, n), o = hd(o), s = hd(s), n.isRawShaderMaterial !== !0 && (v = "#version 300 es\n", g = [
		p,
		"#define attribute in",
		"#define varying out",
		"#define texture2D texture"
	].join("\n") + "\n" + g, _ = [
		"#define varying in",
		n.glslVersion === "300 es" ? "" : "layout(location = 0) out highp vec4 pc_fragColor;",
		n.glslVersion === "300 es" ? "" : "#define gl_FragColor pc_fragColor",
		"#define gl_FragDepthEXT gl_FragDepth",
		"#define texture2D texture",
		"#define textureCube texture",
		"#define texture2DProj textureProj",
		"#define texture2DLodEXT textureLod",
		"#define texture2DProjLodEXT textureProjLod",
		"#define textureCubeLodEXT textureLod",
		"#define texture2DGradEXT textureGrad",
		"#define texture2DProjGradEXT textureProjGrad",
		"#define textureCubeGradEXT textureGrad"
	].join("\n") + "\n" + _);
	let y = v + g + o, b = v + _ + s, x = Ku(i, i.VERTEX_SHADER, y), S = Ku(i, i.FRAGMENT_SHADER, b);
	i.attachShader(h, x), i.attachShader(h, S), n.index0AttributeName === void 0 ? n.morphTargets === !0 && i.bindAttribLocation(h, 0, "position") : i.bindAttribLocation(h, 0, n.index0AttributeName), i.linkProgram(h);
	function C(t) {
		if (e.debug.checkShaderErrors) {
			let n = i.getProgramInfoLog(h) || "", r = i.getShaderInfoLog(x) || "", a = i.getShaderInfoLog(S) || "", o = n.trim(), s = r.trim(), c = a.trim(), l = !0, u = !0;
			if (i.getProgramParameter(h, i.LINK_STATUS) === !1) if (l = !1, typeof e.debug.onShaderError == "function") e.debug.onShaderError(i, h, x, S);
			else {
				let e = Qu(i, x, "vertex"), n = Qu(i, S, "fragment");
				G("THREE.WebGLProgram: Shader Error " + i.getError() + " - VALIDATE_STATUS " + i.getProgramParameter(h, i.VALIDATE_STATUS) + "\n\nMaterial Name: " + t.name + "\nMaterial Type: " + t.type + "\n\nProgram Info Log: " + o + "\n" + e + "\n" + n);
			}
			else o === "" ? (s === "" || c === "") && (u = !1) : W("WebGLProgram: Program Info Log:", o);
			u && (t.diagnostics = {
				runnable: l,
				programLog: o,
				vertexShader: {
					log: s,
					prefix: g
				},
				fragmentShader: {
					log: c,
					prefix: _
				}
			});
		}
		i.deleteShader(x), i.deleteShader(S), w = new Gu(i, h), T = od(i, h);
	}
	let w;
	this.getUniforms = function() {
		return w === void 0 && C(this), w;
	};
	let T;
	this.getAttributes = function() {
		return T === void 0 && C(this), T;
	};
	let E = n.rendererExtensionParallelShaderCompile === !1;
	return this.isReady = function() {
		return E === !1 && (E = i.getProgramParameter(h, qu)), E;
	}, this.destroy = function() {
		r.releaseStatesOfProgram(this), i.deleteProgram(h), this.program = void 0;
	}, this.type = n.shaderType, this.name = n.shaderName, this.id = Ju++, this.cacheKey = t, this.usedTimes = 1, this.program = h, this.vertexShader = x, this.fragmentShader = S, this;
}
var Od = 0, kd = class {
	constructor() {
		this.shaderCache = /* @__PURE__ */ new Map(), this.materialCache = /* @__PURE__ */ new Map();
	}
	update(e) {
		let t = e.vertexShader, n = e.fragmentShader, r = this._getShaderStage(t), i = this._getShaderStage(n), a = this._getShaderCacheForMaterial(e);
		return a.has(r) === !1 && (a.add(r), r.usedTimes++), a.has(i) === !1 && (a.add(i), i.usedTimes++), this;
	}
	remove(e) {
		let t = this.materialCache.get(e);
		for (let e of t) e.usedTimes--, e.usedTimes === 0 && this.shaderCache.delete(e.code);
		return this.materialCache.delete(e), this;
	}
	getVertexShaderID(e) {
		return this._getShaderStage(e.vertexShader).id;
	}
	getFragmentShaderID(e) {
		return this._getShaderStage(e.fragmentShader).id;
	}
	dispose() {
		this.shaderCache.clear(), this.materialCache.clear();
	}
	_getShaderCacheForMaterial(e) {
		let t = this.materialCache, n = t.get(e);
		return n === void 0 && (n = /* @__PURE__ */ new Set(), t.set(e, n)), n;
	}
	_getShaderStage(e) {
		let t = this.shaderCache, n = t.get(e);
		return n === void 0 && (n = new Ad(e), t.set(e, n)), n;
	}
}, Ad = class {
	constructor(e) {
		this.id = Od++, this.code = e, this.usedTimes = 0;
	}
};
function jd(e) {
	return e === 1030 || e === 37490 || e === 36285;
}
function Md(e, t, n, r, i, a) {
	let o = new ki(), s = new kd(), c = /* @__PURE__ */ new Set(), l = [], u = /* @__PURE__ */ new Map(), d = r.logarithmicDepthBuffer, f = r.precision, p = {
		MeshDepthMaterial: "depth",
		MeshDistanceMaterial: "distance",
		MeshNormalMaterial: "normal",
		MeshBasicMaterial: "basic",
		MeshLambertMaterial: "lambert",
		MeshPhongMaterial: "phong",
		MeshToonMaterial: "toon",
		MeshStandardMaterial: "physical",
		MeshPhysicalMaterial: "physical",
		MeshMatcapMaterial: "matcap",
		LineBasicMaterial: "basic",
		LineDashedMaterial: "dashed",
		PointsMaterial: "points",
		ShadowMaterial: "shadow",
		SpriteMaterial: "sprite"
	};
	function m(e) {
		return c.add(e), e === 0 ? "uv" : `uv${e}`;
	}
	function h(i, o, l, u, h, g) {
		let _ = u.fog, v = h.geometry, y = i.isMeshStandardMaterial || i.isMeshLambertMaterial || i.isMeshPhongMaterial ? u.environment : null, b = i.isMeshStandardMaterial || i.isMeshLambertMaterial && !i.envMap || i.isMeshPhongMaterial && !i.envMap, x = t.get(i.envMap || y, b), S = x && x.mapping === 306 ? x.image.height : null, C = p[i.type];
		i.precision !== null && (f = r.getMaxPrecision(i.precision), f !== i.precision && W("WebGLProgram.getParameters:", i.precision, "not supported, using", f, "instead."));
		let w = v.morphAttributes.position || v.morphAttributes.normal || v.morphAttributes.color, T = w === void 0 ? 0 : w.length, E = 0;
		v.morphAttributes.position !== void 0 && (E = 1), v.morphAttributes.normal !== void 0 && (E = 2), v.morphAttributes.color !== void 0 && (E = 3);
		let D, O, k, A;
		if (C) {
			let e = el[C];
			D = e.vertexShader, O = e.fragmentShader;
		} else D = i.vertexShader, O = i.fragmentShader, s.update(i), k = s.getVertexShaderID(i), A = s.getFragmentShaderID(i);
		let ee = e.getRenderTarget(), te = e.state.buffers.depth.getReversed(), ne = h.isInstancedMesh === !0, re = h.isBatchedMesh === !0, ie = !!i.map, ae = !!i.matcap, oe = !!x, se = !!i.aoMap, ce = !!i.lightMap, le = !!i.bumpMap, ue = !!i.normalMap, de = !!i.displacementMap, fe = !!i.emissiveMap, pe = !!i.metalnessMap, me = !!i.roughnessMap, he = i.anisotropy > 0, ge = i.clearcoat > 0, _e = i.dispersion > 0, ve = i.iridescence > 0, ye = i.sheen > 0, be = i.transmission > 0, xe = he && !!i.anisotropyMap, Se = ge && !!i.clearcoatMap, Ce = ge && !!i.clearcoatNormalMap, j = ge && !!i.clearcoatRoughnessMap, M = ve && !!i.iridescenceMap, we = ve && !!i.iridescenceThicknessMap, N = ye && !!i.sheenColorMap, P = ye && !!i.sheenRoughnessMap, Te = !!i.specularMap, F = !!i.specularColorMap, I = !!i.specularIntensityMap, Ee = be && !!i.transmissionMap, De = be && !!i.thicknessMap, Oe = !!i.gradientMap, L = !!i.alphaMap, ke = i.alphaTest > 0, Ae = !!i.alphaHash, je = !!i.extensions, Me = 0;
		i.toneMapped && (ee === null || ee.isXRRenderTarget === !0) && (Me = e.toneMapping);
		let Ne = {
			shaderID: C,
			shaderType: i.type,
			shaderName: i.name,
			vertexShader: D,
			fragmentShader: O,
			defines: i.defines,
			customVertexShaderID: k,
			customFragmentShaderID: A,
			isRawShaderMaterial: i.isRawShaderMaterial === !0,
			glslVersion: i.glslVersion,
			precision: f,
			batching: re,
			batchingColor: re && h._colorsTexture !== null,
			instancing: ne,
			instancingColor: ne && h.instanceColor !== null,
			instancingMorph: ne && h.morphTexture !== null,
			outputColorSpace: ee === null ? e.outputColorSpace : ee.isXRRenderTarget === !0 ? ee.texture.colorSpace : Y.workingColorSpace,
			alphaToCoverage: !!i.alphaToCoverage,
			map: ie,
			matcap: ae,
			envMap: oe,
			envMapMode: oe && x.mapping,
			envMapCubeUVHeight: S,
			aoMap: se,
			lightMap: ce,
			bumpMap: le,
			normalMap: ue,
			displacementMap: de,
			emissiveMap: fe,
			normalMapObjectSpace: ue && i.normalMapType === 1,
			normalMapTangentSpace: ue && i.normalMapType === 0,
			packedNormalMap: ue && i.normalMapType === 0 && jd(i.normalMap.format),
			metalnessMap: pe,
			roughnessMap: me,
			anisotropy: he,
			anisotropyMap: xe,
			clearcoat: ge,
			clearcoatMap: Se,
			clearcoatNormalMap: Ce,
			clearcoatRoughnessMap: j,
			dispersion: _e,
			iridescence: ve,
			iridescenceMap: M,
			iridescenceThicknessMap: we,
			sheen: ye,
			sheenColorMap: N,
			sheenRoughnessMap: P,
			specularMap: Te,
			specularColorMap: F,
			specularIntensityMap: I,
			transmission: be,
			transmissionMap: Ee,
			thicknessMap: De,
			gradientMap: Oe,
			opaque: i.transparent === !1 && i.blending === 1 && i.alphaToCoverage === !1,
			alphaMap: L,
			alphaTest: ke,
			alphaHash: Ae,
			combine: i.combine,
			mapUv: ie && m(i.map.channel),
			aoMapUv: se && m(i.aoMap.channel),
			lightMapUv: ce && m(i.lightMap.channel),
			bumpMapUv: le && m(i.bumpMap.channel),
			normalMapUv: ue && m(i.normalMap.channel),
			displacementMapUv: de && m(i.displacementMap.channel),
			emissiveMapUv: fe && m(i.emissiveMap.channel),
			metalnessMapUv: pe && m(i.metalnessMap.channel),
			roughnessMapUv: me && m(i.roughnessMap.channel),
			anisotropyMapUv: xe && m(i.anisotropyMap.channel),
			clearcoatMapUv: Se && m(i.clearcoatMap.channel),
			clearcoatNormalMapUv: Ce && m(i.clearcoatNormalMap.channel),
			clearcoatRoughnessMapUv: j && m(i.clearcoatRoughnessMap.channel),
			iridescenceMapUv: M && m(i.iridescenceMap.channel),
			iridescenceThicknessMapUv: we && m(i.iridescenceThicknessMap.channel),
			sheenColorMapUv: N && m(i.sheenColorMap.channel),
			sheenRoughnessMapUv: P && m(i.sheenRoughnessMap.channel),
			specularMapUv: Te && m(i.specularMap.channel),
			specularColorMapUv: F && m(i.specularColorMap.channel),
			specularIntensityMapUv: I && m(i.specularIntensityMap.channel),
			transmissionMapUv: Ee && m(i.transmissionMap.channel),
			thicknessMapUv: De && m(i.thicknessMap.channel),
			alphaMapUv: L && m(i.alphaMap.channel),
			vertexTangents: !!v.attributes.tangent && (ue || he),
			vertexNormals: !!v.attributes.normal,
			vertexColors: i.vertexColors,
			vertexAlphas: i.vertexColors === !0 && !!v.attributes.color && v.attributes.color.itemSize === 4,
			pointsUvs: h.isPoints === !0 && !!v.attributes.uv && (ie || L),
			fog: !!_,
			useFog: i.fog === !0,
			fogExp2: !!_ && _.isFogExp2,
			flatShading: i.wireframe === !1 && (i.flatShading === !0 || v.attributes.normal === void 0 && ue === !1 && (i.isMeshLambertMaterial || i.isMeshPhongMaterial || i.isMeshStandardMaterial || i.isMeshPhysicalMaterial)),
			sizeAttenuation: i.sizeAttenuation === !0,
			logarithmicDepthBuffer: d,
			reversedDepthBuffer: te,
			skinning: h.isSkinnedMesh === !0,
			morphTargets: v.morphAttributes.position !== void 0,
			morphNormals: v.morphAttributes.normal !== void 0,
			morphColors: v.morphAttributes.color !== void 0,
			morphTargetsCount: T,
			morphTextureStride: E,
			numDirLights: o.directional.length,
			numPointLights: o.point.length,
			numSpotLights: o.spot.length,
			numSpotLightMaps: o.spotLightMap.length,
			numRectAreaLights: o.rectArea.length,
			numHemiLights: o.hemi.length,
			numDirLightShadows: o.directionalShadowMap.length,
			numPointLightShadows: o.pointShadowMap.length,
			numSpotLightShadows: o.spotShadowMap.length,
			numSpotLightShadowsWithMaps: o.numSpotLightShadowsWithMaps,
			numLightProbes: o.numLightProbes,
			numLightProbeGrids: g.length,
			numClippingPlanes: a.numPlanes,
			numClipIntersection: a.numIntersection,
			dithering: i.dithering,
			shadowMapEnabled: e.shadowMap.enabled && l.length > 0,
			shadowMapType: e.shadowMap.type,
			toneMapping: Me,
			decodeVideoTexture: ie && i.map.isVideoTexture === !0 && Y.getTransfer(i.map.colorSpace) === "srgb",
			decodeVideoTextureEmissive: fe && i.emissiveMap.isVideoTexture === !0 && Y.getTransfer(i.emissiveMap.colorSpace) === "srgb",
			premultipliedAlpha: i.premultipliedAlpha,
			doubleSided: i.side === 2,
			flipSided: i.side === 1,
			useDepthPacking: i.depthPacking >= 0,
			depthPacking: i.depthPacking || 0,
			index0AttributeName: i.index0AttributeName,
			extensionClipCullDistance: je && i.extensions.clipCullDistance === !0 && n.has("WEBGL_clip_cull_distance"),
			extensionMultiDraw: (je && i.extensions.multiDraw === !0 || re) && n.has("WEBGL_multi_draw"),
			rendererExtensionParallelShaderCompile: n.has("KHR_parallel_shader_compile"),
			customProgramCacheKey: i.customProgramCacheKey()
		};
		return Ne.vertexUv1s = c.has(1), Ne.vertexUv2s = c.has(2), Ne.vertexUv3s = c.has(3), c.clear(), Ne;
	}
	function g(t) {
		let n = [];
		if (t.shaderID ? n.push(t.shaderID) : (n.push(t.customVertexShaderID), n.push(t.customFragmentShaderID)), t.defines !== void 0) for (let e in t.defines) n.push(e), n.push(t.defines[e]);
		return t.isRawShaderMaterial === !1 && (_(n, t), v(n, t), n.push(e.outputColorSpace)), n.push(t.customProgramCacheKey), n.join();
	}
	function _(e, t) {
		e.push(t.precision), e.push(t.outputColorSpace), e.push(t.envMapMode), e.push(t.envMapCubeUVHeight), e.push(t.mapUv), e.push(t.alphaMapUv), e.push(t.lightMapUv), e.push(t.aoMapUv), e.push(t.bumpMapUv), e.push(t.normalMapUv), e.push(t.displacementMapUv), e.push(t.emissiveMapUv), e.push(t.metalnessMapUv), e.push(t.roughnessMapUv), e.push(t.anisotropyMapUv), e.push(t.clearcoatMapUv), e.push(t.clearcoatNormalMapUv), e.push(t.clearcoatRoughnessMapUv), e.push(t.iridescenceMapUv), e.push(t.iridescenceThicknessMapUv), e.push(t.sheenColorMapUv), e.push(t.sheenRoughnessMapUv), e.push(t.specularMapUv), e.push(t.specularColorMapUv), e.push(t.specularIntensityMapUv), e.push(t.transmissionMapUv), e.push(t.thicknessMapUv), e.push(t.combine), e.push(t.fogExp2), e.push(t.sizeAttenuation), e.push(t.morphTargetsCount), e.push(t.morphAttributeCount), e.push(t.numDirLights), e.push(t.numPointLights), e.push(t.numSpotLights), e.push(t.numSpotLightMaps), e.push(t.numHemiLights), e.push(t.numRectAreaLights), e.push(t.numDirLightShadows), e.push(t.numPointLightShadows), e.push(t.numSpotLightShadows), e.push(t.numSpotLightShadowsWithMaps), e.push(t.numLightProbes), e.push(t.shadowMapType), e.push(t.toneMapping), e.push(t.numClippingPlanes), e.push(t.numClipIntersection), e.push(t.depthPacking);
	}
	function v(e, t) {
		o.disableAll(), t.instancing && o.enable(0), t.instancingColor && o.enable(1), t.instancingMorph && o.enable(2), t.matcap && o.enable(3), t.envMap && o.enable(4), t.normalMapObjectSpace && o.enable(5), t.normalMapTangentSpace && o.enable(6), t.clearcoat && o.enable(7), t.iridescence && o.enable(8), t.alphaTest && o.enable(9), t.vertexColors && o.enable(10), t.vertexAlphas && o.enable(11), t.vertexUv1s && o.enable(12), t.vertexUv2s && o.enable(13), t.vertexUv3s && o.enable(14), t.vertexTangents && o.enable(15), t.anisotropy && o.enable(16), t.alphaHash && o.enable(17), t.batching && o.enable(18), t.dispersion && o.enable(19), t.batchingColor && o.enable(20), t.gradientMap && o.enable(21), t.packedNormalMap && o.enable(22), t.vertexNormals && o.enable(23), e.push(o.mask), o.disableAll(), t.fog && o.enable(0), t.useFog && o.enable(1), t.flatShading && o.enable(2), t.logarithmicDepthBuffer && o.enable(3), t.reversedDepthBuffer && o.enable(4), t.skinning && o.enable(5), t.morphTargets && o.enable(6), t.morphNormals && o.enable(7), t.morphColors && o.enable(8), t.premultipliedAlpha && o.enable(9), t.shadowMapEnabled && o.enable(10), t.doubleSided && o.enable(11), t.flipSided && o.enable(12), t.useDepthPacking && o.enable(13), t.dithering && o.enable(14), t.transmission && o.enable(15), t.sheen && o.enable(16), t.opaque && o.enable(17), t.pointsUvs && o.enable(18), t.decodeVideoTexture && o.enable(19), t.decodeVideoTextureEmissive && o.enable(20), t.alphaToCoverage && o.enable(21), t.numLightProbeGrids > 0 && o.enable(22), e.push(o.mask);
	}
	function y(e) {
		let t = p[e.type], n;
		if (t) {
			let e = el[t];
			n = Os.clone(e.uniforms);
		} else n = e.uniforms;
		return n;
	}
	function b(t, n) {
		let r = u.get(n);
		return r === void 0 ? (r = new Dd(e, n, t, i), l.push(r), u.set(n, r)) : ++r.usedTimes, r;
	}
	function x(e) {
		if (--e.usedTimes === 0) {
			let t = l.indexOf(e);
			l[t] = l[l.length - 1], l.pop(), u.delete(e.cacheKey), e.destroy();
		}
	}
	function S(e) {
		s.remove(e);
	}
	function C() {
		s.dispose();
	}
	return {
		getParameters: h,
		getProgramCacheKey: g,
		getUniforms: y,
		acquireProgram: b,
		releaseProgram: x,
		releaseShaderCache: S,
		programs: l,
		dispose: C
	};
}
function Nd() {
	let e = /* @__PURE__ */ new WeakMap();
	function t(t) {
		return e.has(t);
	}
	function n(t) {
		let n = e.get(t);
		return n === void 0 && (n = {}, e.set(t, n)), n;
	}
	function r(t) {
		e.delete(t);
	}
	function i(t, n, r) {
		e.get(t)[n] = r;
	}
	function a() {
		e = /* @__PURE__ */ new WeakMap();
	}
	return {
		has: t,
		get: n,
		remove: r,
		update: i,
		dispose: a
	};
}
function Pd(e, t) {
	return e.groupOrder === t.groupOrder ? e.renderOrder === t.renderOrder ? e.material.id === t.material.id ? e.materialVariant === t.materialVariant ? e.z === t.z ? e.id - t.id : e.z - t.z : e.materialVariant - t.materialVariant : e.material.id - t.material.id : e.renderOrder - t.renderOrder : e.groupOrder - t.groupOrder;
}
function Fd(e, t) {
	return e.groupOrder === t.groupOrder ? e.renderOrder === t.renderOrder ? e.z === t.z ? e.id - t.id : t.z - e.z : e.renderOrder - t.renderOrder : e.groupOrder - t.groupOrder;
}
function Id() {
	let e = [], t = 0, n = [], r = [], i = [];
	function a() {
		t = 0, n.length = 0, r.length = 0, i.length = 0;
	}
	function o(e) {
		let t = 0;
		return e.isInstancedMesh && (t += 2), e.isSkinnedMesh && (t += 1), t;
	}
	function s(n, r, i, a, s, c) {
		let l = e[t];
		return l === void 0 ? (l = {
			id: n.id,
			object: n,
			geometry: r,
			material: i,
			materialVariant: o(n),
			groupOrder: a,
			renderOrder: n.renderOrder,
			z: s,
			group: c
		}, e[t] = l) : (l.id = n.id, l.object = n, l.geometry = r, l.material = i, l.materialVariant = o(n), l.groupOrder = a, l.renderOrder = n.renderOrder, l.z = s, l.group = c), t++, l;
	}
	function c(e, t, a, o, c, l) {
		let u = s(e, t, a, o, c, l);
		a.transmission > 0 ? r.push(u) : a.transparent === !0 ? i.push(u) : n.push(u);
	}
	function l(e, t, a, o, c, l) {
		let u = s(e, t, a, o, c, l);
		a.transmission > 0 ? r.unshift(u) : a.transparent === !0 ? i.unshift(u) : n.unshift(u);
	}
	function u(e, t) {
		n.length > 1 && n.sort(e || Pd), r.length > 1 && r.sort(t || Fd), i.length > 1 && i.sort(t || Fd);
	}
	function d() {
		for (let n = t, r = e.length; n < r; n++) {
			let t = e[n];
			if (t.id === null) break;
			t.id = null, t.object = null, t.geometry = null, t.material = null, t.group = null;
		}
	}
	return {
		opaque: n,
		transmissive: r,
		transparent: i,
		init: a,
		push: c,
		unshift: l,
		finish: d,
		sort: u
	};
}
function Ld() {
	let e = /* @__PURE__ */ new WeakMap();
	function t(t, n) {
		let r = e.get(t), i;
		return r === void 0 ? (i = new Id(), e.set(t, [i])) : n >= r.length ? (i = new Id(), r.push(i)) : i = r[n], i;
	}
	function n() {
		e = /* @__PURE__ */ new WeakMap();
	}
	return {
		get: t,
		dispose: n
	};
}
function Rd() {
	let e = {};
	return { get: function(t) {
		if (e[t.id] !== void 0) return e[t.id];
		let n;
		switch (t.type) {
			case "DirectionalLight":
				n = {
					direction: new q(),
					color: new X()
				};
				break;
			case "SpotLight":
				n = {
					position: new q(),
					direction: new q(),
					color: new X(),
					distance: 0,
					coneCos: 0,
					penumbraCos: 0,
					decay: 0
				};
				break;
			case "PointLight":
				n = {
					position: new q(),
					color: new X(),
					distance: 0,
					decay: 0
				};
				break;
			case "HemisphereLight":
				n = {
					direction: new q(),
					skyColor: new X(),
					groundColor: new X()
				};
				break;
			case "RectAreaLight":
				n = {
					color: new X(),
					position: new q(),
					halfWidth: new q(),
					halfHeight: new q()
				};
				break;
		}
		return e[t.id] = n, n;
	} };
}
function zd() {
	let e = {};
	return { get: function(t) {
		if (e[t.id] !== void 0) return e[t.id];
		let n;
		switch (t.type) {
			case "DirectionalLight":
				n = {
					shadowIntensity: 1,
					shadowBias: 0,
					shadowNormalBias: 0,
					shadowRadius: 1,
					shadowMapSize: new Yr()
				};
				break;
			case "SpotLight":
				n = {
					shadowIntensity: 1,
					shadowBias: 0,
					shadowNormalBias: 0,
					shadowRadius: 1,
					shadowMapSize: new Yr()
				};
				break;
			case "PointLight":
				n = {
					shadowIntensity: 1,
					shadowBias: 0,
					shadowNormalBias: 0,
					shadowRadius: 1,
					shadowMapSize: new Yr(),
					shadowCameraNear: 1,
					shadowCameraFar: 1e3
				};
				break;
		}
		return e[t.id] = n, n;
	} };
}
var Bd = 0;
function Vd(e, t) {
	return (t.castShadow ? 2 : 0) - (e.castShadow ? 2 : 0) + +!!t.map - !!e.map;
}
function Hd(e) {
	let t = new Rd(), n = zd(), r = {
		version: 0,
		hash: {
			directionalLength: -1,
			pointLength: -1,
			spotLength: -1,
			rectAreaLength: -1,
			hemiLength: -1,
			numDirectionalShadows: -1,
			numPointShadows: -1,
			numSpotShadows: -1,
			numSpotMaps: -1,
			numLightProbes: -1
		},
		ambient: [
			0,
			0,
			0
		],
		probe: [],
		directional: [],
		directionalShadow: [],
		directionalShadowMap: [],
		directionalShadowMatrix: [],
		spot: [],
		spotLightMap: [],
		spotShadow: [],
		spotShadowMap: [],
		spotLightMatrix: [],
		rectArea: [],
		rectAreaLTC1: null,
		rectAreaLTC2: null,
		point: [],
		pointShadow: [],
		pointShadowMap: [],
		pointShadowMatrix: [],
		hemi: [],
		numSpotLightShadowsWithMaps: 0,
		numLightProbes: 0
	};
	for (let e = 0; e < 9; e++) r.probe.push(new q());
	let i = new q(), a = new vi(), o = new vi();
	function s(i) {
		let a = 0, o = 0, s = 0;
		for (let e = 0; e < 9; e++) r.probe[e].set(0, 0, 0);
		let c = 0, l = 0, u = 0, d = 0, f = 0, p = 0, m = 0, h = 0, g = 0, _ = 0, v = 0;
		i.sort(Vd);
		for (let e = 0, y = i.length; e < y; e++) {
			let y = i[e], b = y.color, x = y.intensity, S = y.distance, C = null;
			if (y.shadow && y.shadow.map && (C = y.shadow.map.texture.format === 1030 ? y.shadow.map.texture : y.shadow.map.depthTexture || y.shadow.map.texture), y.isAmbientLight) a += b.r * x, o += b.g * x, s += b.b * x;
			else if (y.isLightProbe) {
				for (let e = 0; e < 9; e++) r.probe[e].addScaledVector(y.sh.coefficients[e], x);
				v++;
			} else if (y.isDirectionalLight) {
				let e = t.get(y);
				if (e.color.copy(y.color).multiplyScalar(y.intensity), y.castShadow) {
					let e = y.shadow, t = n.get(y);
					t.shadowIntensity = e.intensity, t.shadowBias = e.bias, t.shadowNormalBias = e.normalBias, t.shadowRadius = e.radius, t.shadowMapSize = e.mapSize, r.directionalShadow[c] = t, r.directionalShadowMap[c] = C, r.directionalShadowMatrix[c] = y.shadow.matrix, p++;
				}
				r.directional[c] = e, c++;
			} else if (y.isSpotLight) {
				let e = t.get(y);
				e.position.setFromMatrixPosition(y.matrixWorld), e.color.copy(b).multiplyScalar(x), e.distance = S, e.coneCos = Math.cos(y.angle), e.penumbraCos = Math.cos(y.angle * (1 - y.penumbra)), e.decay = y.decay, r.spot[u] = e;
				let i = y.shadow;
				if (y.map && (r.spotLightMap[g] = y.map, g++, i.updateMatrices(y), y.castShadow && _++), r.spotLightMatrix[u] = i.matrix, y.castShadow) {
					let e = n.get(y);
					e.shadowIntensity = i.intensity, e.shadowBias = i.bias, e.shadowNormalBias = i.normalBias, e.shadowRadius = i.radius, e.shadowMapSize = i.mapSize, r.spotShadow[u] = e, r.spotShadowMap[u] = C, h++;
				}
				u++;
			} else if (y.isRectAreaLight) {
				let e = t.get(y);
				e.color.copy(b).multiplyScalar(x), e.halfWidth.set(y.width * .5, 0, 0), e.halfHeight.set(0, y.height * .5, 0), r.rectArea[d] = e, d++;
			} else if (y.isPointLight) {
				let e = t.get(y);
				if (e.color.copy(y.color).multiplyScalar(y.intensity), e.distance = y.distance, e.decay = y.decay, y.castShadow) {
					let e = y.shadow, t = n.get(y);
					t.shadowIntensity = e.intensity, t.shadowBias = e.bias, t.shadowNormalBias = e.normalBias, t.shadowRadius = e.radius, t.shadowMapSize = e.mapSize, t.shadowCameraNear = e.camera.near, t.shadowCameraFar = e.camera.far, r.pointShadow[l] = t, r.pointShadowMap[l] = C, r.pointShadowMatrix[l] = y.shadow.matrix, m++;
				}
				r.point[l] = e, l++;
			} else if (y.isHemisphereLight) {
				let e = t.get(y);
				e.skyColor.copy(y.color).multiplyScalar(x), e.groundColor.copy(y.groundColor).multiplyScalar(x), r.hemi[f] = e, f++;
			}
		}
		d > 0 && (e.has("OES_texture_float_linear") === !0 ? (r.rectAreaLTC1 = Q.LTC_FLOAT_1, r.rectAreaLTC2 = Q.LTC_FLOAT_2) : (r.rectAreaLTC1 = Q.LTC_HALF_1, r.rectAreaLTC2 = Q.LTC_HALF_2)), r.ambient[0] = a, r.ambient[1] = o, r.ambient[2] = s;
		let y = r.hash;
		(y.directionalLength !== c || y.pointLength !== l || y.spotLength !== u || y.rectAreaLength !== d || y.hemiLength !== f || y.numDirectionalShadows !== p || y.numPointShadows !== m || y.numSpotShadows !== h || y.numSpotMaps !== g || y.numLightProbes !== v) && (r.directional.length = c, r.spot.length = u, r.rectArea.length = d, r.point.length = l, r.hemi.length = f, r.directionalShadow.length = p, r.directionalShadowMap.length = p, r.pointShadow.length = m, r.pointShadowMap.length = m, r.spotShadow.length = h, r.spotShadowMap.length = h, r.directionalShadowMatrix.length = p, r.pointShadowMatrix.length = m, r.spotLightMatrix.length = h + g - _, r.spotLightMap.length = g, r.numSpotLightShadowsWithMaps = _, r.numLightProbes = v, y.directionalLength = c, y.pointLength = l, y.spotLength = u, y.rectAreaLength = d, y.hemiLength = f, y.numDirectionalShadows = p, y.numPointShadows = m, y.numSpotShadows = h, y.numSpotMaps = g, y.numLightProbes = v, r.version = Bd++);
	}
	function c(e, t) {
		let n = 0, s = 0, c = 0, l = 0, u = 0, d = t.matrixWorldInverse;
		for (let t = 0, f = e.length; t < f; t++) {
			let f = e[t];
			if (f.isDirectionalLight) {
				let e = r.directional[n];
				e.direction.setFromMatrixPosition(f.matrixWorld), i.setFromMatrixPosition(f.target.matrixWorld), e.direction.sub(i), e.direction.transformDirection(d), n++;
			} else if (f.isSpotLight) {
				let e = r.spot[c];
				e.position.setFromMatrixPosition(f.matrixWorld), e.position.applyMatrix4(d), e.direction.setFromMatrixPosition(f.matrixWorld), i.setFromMatrixPosition(f.target.matrixWorld), e.direction.sub(i), e.direction.transformDirection(d), c++;
			} else if (f.isRectAreaLight) {
				let e = r.rectArea[l];
				e.position.setFromMatrixPosition(f.matrixWorld), e.position.applyMatrix4(d), o.identity(), a.copy(f.matrixWorld), a.premultiply(d), o.extractRotation(a), e.halfWidth.set(f.width * .5, 0, 0), e.halfHeight.set(0, f.height * .5, 0), e.halfWidth.applyMatrix4(o), e.halfHeight.applyMatrix4(o), l++;
			} else if (f.isPointLight) {
				let e = r.point[s];
				e.position.setFromMatrixPosition(f.matrixWorld), e.position.applyMatrix4(d), s++;
			} else if (f.isHemisphereLight) {
				let e = r.hemi[u];
				e.direction.setFromMatrixPosition(f.matrixWorld), e.direction.transformDirection(d), u++;
			}
		}
	}
	return {
		setup: s,
		setupView: c,
		state: r
	};
}
function Ud(e) {
	let t = new Hd(e), n = [], r = [], i = [];
	function a(e) {
		d.camera = e, n.length = 0, r.length = 0, i.length = 0;
	}
	function o(e) {
		n.push(e);
	}
	function s(e) {
		r.push(e);
	}
	function c(e) {
		i.push(e);
	}
	function l() {
		t.setup(n);
	}
	function u(e) {
		t.setupView(n, e);
	}
	let d = {
		lightsArray: n,
		shadowsArray: r,
		lightProbeGridArray: i,
		camera: null,
		lights: t,
		transmissionRenderTarget: {},
		textureUnits: 0
	};
	return {
		init: a,
		state: d,
		setupLights: l,
		setupLightsView: u,
		pushLight: o,
		pushShadow: s,
		pushLightProbeGrid: c
	};
}
function Wd(e) {
	let t = /* @__PURE__ */ new WeakMap();
	function n(n, r = 0) {
		let i = t.get(n), a;
		return i === void 0 ? (a = new Ud(e), t.set(n, [a])) : r >= i.length ? (a = new Ud(e), i.push(a)) : a = i[r], a;
	}
	function r() {
		t = /* @__PURE__ */ new WeakMap();
	}
	return {
		get: n,
		dispose: r
	};
}
var Gd = "void main() {\n	gl_Position = vec4( position, 1.0 );\n}", Kd = "uniform sampler2D shadow_pass;\nuniform vec2 resolution;\nuniform float radius;\nvoid main() {\n	const float samples = float( VSM_SAMPLES );\n	float mean = 0.0;\n	float squared_mean = 0.0;\n	float uvStride = samples <= 1.0 ? 0.0 : 2.0 / ( samples - 1.0 );\n	float uvStart = samples <= 1.0 ? 0.0 : - 1.0;\n	for ( float i = 0.0; i < samples; i ++ ) {\n		float uvOffset = uvStart + i * uvStride;\n		#ifdef HORIZONTAL_PASS\n			vec2 distribution = texture2D( shadow_pass, ( gl_FragCoord.xy + vec2( uvOffset, 0.0 ) * radius ) / resolution ).rg;\n			mean += distribution.x;\n			squared_mean += distribution.y * distribution.y + distribution.x * distribution.x;\n		#else\n			float depth = texture2D( shadow_pass, ( gl_FragCoord.xy + vec2( 0.0, uvOffset ) * radius ) / resolution ).r;\n			mean += depth;\n			squared_mean += depth * depth;\n		#endif\n	}\n	mean = mean / samples;\n	squared_mean = squared_mean / samples;\n	float std_dev = sqrt( max( 0.0, squared_mean - mean * mean ) );\n	gl_FragColor = vec4( mean, std_dev, 0.0, 1.0 );\n}", qd = [
	/*@__PURE__*/ new q(1, 0, 0),
	/*@__PURE__*/ new q(-1, 0, 0),
	/*@__PURE__*/ new q(0, 1, 0),
	/*@__PURE__*/ new q(0, -1, 0),
	/*@__PURE__*/ new q(0, 0, 1),
	/*@__PURE__*/ new q(0, 0, -1)
], Jd = [
	/*@__PURE__*/ new q(0, -1, 0),
	/*@__PURE__*/ new q(0, -1, 0),
	/*@__PURE__*/ new q(0, 0, 1),
	/*@__PURE__*/ new q(0, 0, -1),
	/*@__PURE__*/ new q(0, -1, 0),
	/*@__PURE__*/ new q(0, -1, 0)
], Yd = /*@__PURE__*/ new vi(), Xd = /*@__PURE__*/ new q(), Zd = /*@__PURE__*/ new q();
function Qd(e, t, n) {
	let r = new Yo(), i = new Yr(), a = new Yr(), o = new pi(), s = new Ps(), c = new Fs(), l = {}, u = n.maxTextureSize, d = {
		0: 1,
		1: 0,
		2: 2
	}, f = new js({
		defines: { VSM_SAMPLES: 8 },
		uniforms: {
			shadow_pass: { value: null },
			resolution: { value: new Yr() },
			radius: { value: 4 }
		},
		vertexShader: Gd,
		fragmentShader: Kd
	}), p = f.clone();
	p.defines.HORIZONTAL_PASS = 1;
	let m = new Ja();
	m.setAttribute("position", new Na(new Float32Array([
		-1,
		-1,
		.5,
		3,
		-1,
		.5,
		-1,
		3,
		.5
	]), 3));
	let h = new vo(m, f), g = this;
	this.enabled = !1, this.autoUpdate = !0, this.needsUpdate = !1, this.type = 1;
	let _ = this.type;
	this.render = function(t, n, s) {
		if (g.enabled === !1 || g.autoUpdate === !1 && g.needsUpdate === !1 || t.length === 0) return;
		this.type === 2 && (W("WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead."), this.type = 1);
		let c = e.getRenderTarget(), l = e.getActiveCubeFace(), d = e.getActiveMipmapLevel(), f = e.state;
		f.setBlending(0), f.buffers.depth.getReversed() === !0 ? f.buffers.color.setClear(0, 0, 0, 0) : f.buffers.color.setClear(1, 1, 1, 1), f.buffers.depth.setTest(!0), f.setScissorTest(!1);
		let p = _ !== this.type;
		p && n.traverse(function(e) {
			e.material && (Array.isArray(e.material) ? e.material.forEach((e) => e.needsUpdate = !0) : e.material.needsUpdate = !0);
		});
		for (let c = 0, l = t.length; c < l; c++) {
			let l = t[c], d = l.shadow;
			if (d === void 0) {
				W("WebGLShadowMap:", l, "has no shadow.");
				continue;
			}
			if (d.autoUpdate === !1 && d.needsUpdate === !1) continue;
			i.copy(d.mapSize);
			let m = d.getFrameExtents();
			i.multiply(m), a.copy(d.mapSize), (i.x > u || i.y > u) && (i.x > u && (a.x = Math.floor(u / m.x), i.x = a.x * m.x, d.mapSize.x = a.x), i.y > u && (a.y = Math.floor(u / m.y), i.y = a.y * m.y, d.mapSize.y = a.y));
			let h = e.state.buffers.depth.getReversed();
			if (d.camera._reversedDepth = h, d.map === null || p === !0) {
				if (d.map !== null && (d.map.depthTexture !== null && (d.map.depthTexture.dispose(), d.map.depthTexture = null), d.map.dispose()), this.type === 3) {
					if (l.isPointLight) {
						W("WebGLShadowMap: VSM shadow maps are not supported for PointLights. Use PCF or BasicShadowMap instead.");
						continue;
					}
					d.map = new hi(i.x, i.y, {
						format: on,
						type: Kt,
						minFilter: It,
						magFilter: It,
						generateMipmaps: !1
					}), d.map.texture.name = l.name + ".shadowMap", d.map.depthTexture = new _s(i.x, i.y, Gt), d.map.depthTexture.name = l.name + ".shadowMapDepth", d.map.depthTexture.format = tn, d.map.depthTexture.compareFunction = null, d.map.depthTexture.minFilter = Nt, d.map.depthTexture.magFilter = Nt;
				} else l.isPointLight ? (d.map = new kl(i.x), d.map.depthTexture = new vs(i.x, Wt)) : (d.map = new hi(i.x, i.y), d.map.depthTexture = new _s(i.x, i.y, Wt)), d.map.depthTexture.name = l.name + ".shadowMap", d.map.depthTexture.format = tn, this.type === 1 ? (d.map.depthTexture.compareFunction = h ? 518 : 515, d.map.depthTexture.minFilter = It, d.map.depthTexture.magFilter = It) : (d.map.depthTexture.compareFunction = null, d.map.depthTexture.minFilter = Nt, d.map.depthTexture.magFilter = Nt);
				d.camera.updateProjectionMatrix();
			}
			let g = d.map.isWebGLCubeRenderTarget ? 6 : 1;
			for (let t = 0; t < g; t++) {
				if (d.map.isWebGLCubeRenderTarget) e.setRenderTarget(d.map, t), e.clear();
				else {
					t === 0 && (e.setRenderTarget(d.map), e.clear());
					let n = d.getViewport(t);
					o.set(a.x * n.x, a.y * n.y, a.x * n.z, a.y * n.w), f.viewport(o);
				}
				if (l.isPointLight) {
					let e = d.camera, n = d.matrix, r = l.distance || e.far;
					r !== e.far && (e.far = r, e.updateProjectionMatrix()), Xd.setFromMatrixPosition(l.matrixWorld), e.position.copy(Xd), Zd.copy(e.position), Zd.add(qd[t]), e.up.copy(Jd[t]), e.lookAt(Zd), e.updateMatrixWorld(), n.makeTranslation(-Xd.x, -Xd.y, -Xd.z), Yd.multiplyMatrices(e.projectionMatrix, e.matrixWorldInverse), d._frustum.setFromProjectionMatrix(Yd, e.coordinateSystem, e.reversedDepth);
				} else d.updateMatrices(l);
				r = d.getFrustum(), b(n, s, d.camera, l, this.type);
			}
			d.isPointLightShadow !== !0 && this.type === 3 && v(d, s), d.needsUpdate = !1;
		}
		_ = this.type, g.needsUpdate = !1, e.setRenderTarget(c, l, d);
	};
	function v(n, r) {
		let a = t.update(h);
		f.defines.VSM_SAMPLES !== n.blurSamples && (f.defines.VSM_SAMPLES = n.blurSamples, p.defines.VSM_SAMPLES = n.blurSamples, f.needsUpdate = !0, p.needsUpdate = !0), n.mapPass === null && (n.mapPass = new hi(i.x, i.y, {
			format: on,
			type: Kt
		})), f.uniforms.shadow_pass.value = n.map.depthTexture, f.uniforms.resolution.value = n.mapSize, f.uniforms.radius.value = n.radius, e.setRenderTarget(n.mapPass), e.clear(), e.renderBufferDirect(r, null, a, f, h, null), p.uniforms.shadow_pass.value = n.mapPass.texture, p.uniforms.resolution.value = n.mapSize, p.uniforms.radius.value = n.radius, e.setRenderTarget(n.map), e.clear(), e.renderBufferDirect(r, null, a, p, h, null);
	}
	function y(t, n, r, i) {
		let a = null, o = r.isPointLight === !0 ? t.customDistanceMaterial : t.customDepthMaterial;
		if (o !== void 0) a = o;
		else if (a = r.isPointLight === !0 ? c : s, e.localClippingEnabled && n.clipShadows === !0 && Array.isArray(n.clippingPlanes) && n.clippingPlanes.length !== 0 || n.displacementMap && n.displacementScale !== 0 || n.alphaMap && n.alphaTest > 0 || n.map && n.alphaTest > 0 || n.alphaToCoverage === !0) {
			let e = a.uuid, t = n.uuid, r = l[e];
			r === void 0 && (r = {}, l[e] = r);
			let i = r[t];
			i === void 0 && (i = a.clone(), r[t] = i, n.addEventListener("dispose", x)), a = i;
		}
		if (a.visible = n.visible, a.wireframe = n.wireframe, i === 3 ? a.side = n.shadowSide === null ? n.side : n.shadowSide : a.side = n.shadowSide === null ? d[n.side] : n.shadowSide, a.alphaMap = n.alphaMap, a.alphaTest = n.alphaToCoverage === !0 ? .5 : n.alphaTest, a.map = n.map, a.clipShadows = n.clipShadows, a.clippingPlanes = n.clippingPlanes, a.clipIntersection = n.clipIntersection, a.displacementMap = n.displacementMap, a.displacementScale = n.displacementScale, a.displacementBias = n.displacementBias, a.wireframeLinewidth = n.wireframeLinewidth, a.linewidth = n.linewidth, r.isPointLight === !0 && a.isMeshDistanceMaterial === !0) {
			let t = e.properties.get(a);
			t.light = r;
		}
		return a;
	}
	function b(n, i, a, o, s) {
		if (n.visible === !1) return;
		if (n.layers.test(i.layers) && (n.isMesh || n.isLine || n.isPoints) && (n.castShadow || n.receiveShadow && s === 3) && (!n.frustumCulled || r.intersectsObject(n))) {
			n.modelViewMatrix.multiplyMatrices(a.matrixWorldInverse, n.matrixWorld);
			let r = t.update(n), c = n.material;
			if (Array.isArray(c)) {
				let t = r.groups;
				for (let l = 0, u = t.length; l < u; l++) {
					let u = t[l], d = c[u.materialIndex];
					if (d && d.visible) {
						let t = y(n, d, o, s);
						n.onBeforeShadow(e, n, i, a, r, t, u), e.renderBufferDirect(a, null, r, t, n, u), n.onAfterShadow(e, n, i, a, r, t, u);
					}
				}
			} else if (c.visible) {
				let t = y(n, c, o, s);
				n.onBeforeShadow(e, n, i, a, r, t, null), e.renderBufferDirect(a, null, r, t, n, null), n.onAfterShadow(e, n, i, a, r, t, null);
			}
		}
		let c = n.children;
		for (let e = 0, t = c.length; e < t; e++) b(c[e], i, a, o, s);
	}
	function x(e) {
		e.target.removeEventListener("dispose", x);
		for (let t in l) {
			let n = l[t], r = e.target.uuid;
			r in n && (n[r].dispose(), delete n[r]);
		}
	}
}
function $d(e, t) {
	function n() {
		let t = !1, n = new pi(), r = null, i = new pi(0, 0, 0, 0);
		return {
			setMask: function(n) {
				r !== n && !t && (e.colorMask(n, n, n, n), r = n);
			},
			setLocked: function(e) {
				t = e;
			},
			setClear: function(t, r, a, o, s) {
				s === !0 && (t *= o, r *= o, a *= o), n.set(t, r, a, o), i.equals(n) === !1 && (e.clearColor(t, r, a, o), i.copy(n));
			},
			reset: function() {
				t = !1, r = null, i.set(-1, 0, 0, 0);
			}
		};
	}
	function r() {
		let n = !1, r = !1, i = null, a = null, o = null;
		return {
			setReversed: function(e) {
				if (r !== e) {
					let n = t.get("EXT_clip_control");
					e ? n.clipControlEXT(n.LOWER_LEFT_EXT, n.ZERO_TO_ONE_EXT) : n.clipControlEXT(n.LOWER_LEFT_EXT, n.NEGATIVE_ONE_TO_ONE_EXT), r = e;
					let i = o;
					o = null, this.setClear(i);
				}
			},
			getReversed: function() {
				return r;
			},
			setTest: function(t) {
				t ? pe(e.DEPTH_TEST) : me(e.DEPTH_TEST);
			},
			setMask: function(t) {
				i !== t && !n && (e.depthMask(t), i = t);
			},
			setFunc: function(t) {
				if (r && (t = xr[t]), a !== t) {
					switch (t) {
						case 0:
							e.depthFunc(e.NEVER);
							break;
						case 1:
							e.depthFunc(e.ALWAYS);
							break;
						case 2:
							e.depthFunc(e.LESS);
							break;
						case 3:
							e.depthFunc(e.LEQUAL);
							break;
						case 4:
							e.depthFunc(e.EQUAL);
							break;
						case 5:
							e.depthFunc(e.GEQUAL);
							break;
						case 6:
							e.depthFunc(e.GREATER);
							break;
						case 7:
							e.depthFunc(e.NOTEQUAL);
							break;
						default: e.depthFunc(e.LEQUAL);
					}
					a = t;
				}
			},
			setLocked: function(e) {
				n = e;
			},
			setClear: function(t) {
				o !== t && (o = t, r && (t = 1 - t), e.clearDepth(t));
			},
			reset: function() {
				n = !1, i = null, a = null, o = null, r = !1;
			}
		};
	}
	function i() {
		let t = !1, n = null, r = null, i = null, a = null, o = null, s = null, c = null, l = null;
		return {
			setTest: function(n) {
				t || (n ? pe(e.STENCIL_TEST) : me(e.STENCIL_TEST));
			},
			setMask: function(r) {
				n !== r && !t && (e.stencilMask(r), n = r);
			},
			setFunc: function(t, n, o) {
				(r !== t || i !== n || a !== o) && (e.stencilFunc(t, n, o), r = t, i = n, a = o);
			},
			setOp: function(t, n, r) {
				(o !== t || s !== n || c !== r) && (e.stencilOp(t, n, r), o = t, s = n, c = r);
			},
			setLocked: function(e) {
				t = e;
			},
			setClear: function(t) {
				l !== t && (e.clearStencil(t), l = t);
			},
			reset: function() {
				t = !1, n = null, r = null, i = null, a = null, o = null, s = null, c = null, l = null;
			}
		};
	}
	let a = new n(), o = new r(), s = new i(), c = /* @__PURE__ */ new WeakMap(), l = /* @__PURE__ */ new WeakMap(), u = {}, d = {}, f = {}, p = /* @__PURE__ */ new WeakMap(), m = [], h = null, g = !1, _ = null, v = null, y = null, b = null, x = null, S = null, C = null, w = new X(0, 0, 0), T = 0, E = !1, D = null, O = null, k = null, A = null, ee = null, te = e.getParameter(e.MAX_COMBINED_TEXTURE_IMAGE_UNITS), ne = !1, re = 0, ie = e.getParameter(e.VERSION);
	ie.indexOf("WebGL") === -1 ? ie.indexOf("OpenGL ES") !== -1 && (re = parseFloat(/^OpenGL ES (\d)/.exec(ie)[1]), ne = re >= 2) : (re = parseFloat(/^WebGL (\d)/.exec(ie)[1]), ne = re >= 1);
	let ae = null, oe = {}, se = e.getParameter(e.SCISSOR_BOX), ce = e.getParameter(e.VIEWPORT), le = new pi().fromArray(se), ue = new pi().fromArray(ce);
	function de(t, n, r, i) {
		let a = /* @__PURE__ */ new Uint8Array(4), o = e.createTexture();
		e.bindTexture(t, o), e.texParameteri(t, e.TEXTURE_MIN_FILTER, e.NEAREST), e.texParameteri(t, e.TEXTURE_MAG_FILTER, e.NEAREST);
		for (let o = 0; o < r; o++) t === e.TEXTURE_3D || t === e.TEXTURE_2D_ARRAY ? e.texImage3D(n, 0, e.RGBA, 1, 1, i, 0, e.RGBA, e.UNSIGNED_BYTE, a) : e.texImage2D(n + o, 0, e.RGBA, 1, 1, 0, e.RGBA, e.UNSIGNED_BYTE, a);
		return o;
	}
	let fe = {};
	fe[e.TEXTURE_2D] = de(e.TEXTURE_2D, e.TEXTURE_2D, 1), fe[e.TEXTURE_CUBE_MAP] = de(e.TEXTURE_CUBE_MAP, e.TEXTURE_CUBE_MAP_POSITIVE_X, 6), fe[e.TEXTURE_2D_ARRAY] = de(e.TEXTURE_2D_ARRAY, e.TEXTURE_2D_ARRAY, 1, 1), fe[e.TEXTURE_3D] = de(e.TEXTURE_3D, e.TEXTURE_3D, 1, 1), a.setClear(0, 0, 0, 1), o.setClear(1), s.setClear(0), pe(e.DEPTH_TEST), o.setFunc(3), Se(!1), Ce(1), pe(e.CULL_FACE), be(0);
	function pe(t) {
		u[t] !== !0 && (e.enable(t), u[t] = !0);
	}
	function me(t) {
		u[t] !== !1 && (e.disable(t), u[t] = !1);
	}
	function he(t, n) {
		return f[t] === n ? !1 : (e.bindFramebuffer(t, n), f[t] = n, t === e.DRAW_FRAMEBUFFER && (f[e.FRAMEBUFFER] = n), t === e.FRAMEBUFFER && (f[e.DRAW_FRAMEBUFFER] = n), !0);
	}
	function ge(t, n) {
		let r = m, i = !1;
		if (t) {
			r = p.get(n), r === void 0 && (r = [], p.set(n, r));
			let a = t.textures;
			if (r.length !== a.length || r[0] !== e.COLOR_ATTACHMENT0) {
				for (let t = 0, n = a.length; t < n; t++) r[t] = e.COLOR_ATTACHMENT0 + t;
				r.length = a.length, i = !0;
			}
		} else r[0] !== e.BACK && (r[0] = e.BACK, i = !0);
		i && e.drawBuffers(r);
	}
	function _e(t) {
		return h === t ? !1 : (e.useProgram(t), h = t, !0);
	}
	let ve = {
		100: e.FUNC_ADD,
		101: e.FUNC_SUBTRACT,
		102: e.FUNC_REVERSE_SUBTRACT
	};
	ve[103] = e.MIN, ve[104] = e.MAX;
	let ye = {
		200: e.ZERO,
		201: e.ONE,
		202: e.SRC_COLOR,
		204: e.SRC_ALPHA,
		210: e.SRC_ALPHA_SATURATE,
		208: e.DST_COLOR,
		206: e.DST_ALPHA,
		203: e.ONE_MINUS_SRC_COLOR,
		205: e.ONE_MINUS_SRC_ALPHA,
		209: e.ONE_MINUS_DST_COLOR,
		207: e.ONE_MINUS_DST_ALPHA,
		211: e.CONSTANT_COLOR,
		212: e.ONE_MINUS_CONSTANT_COLOR,
		213: e.CONSTANT_ALPHA,
		214: e.ONE_MINUS_CONSTANT_ALPHA
	};
	function be(t, n, r, i, a, o, s, c, l, u) {
		if (t === 0) {
			g === !0 && (me(e.BLEND), g = !1);
			return;
		}
		if (g === !1 && (pe(e.BLEND), g = !0), t !== 5) {
			if (t !== _ || u !== E) {
				if ((v !== 100 || x !== 100) && (e.blendEquation(e.FUNC_ADD), v = 100, x = 100), u) switch (t) {
					case 1:
						e.blendFuncSeparate(e.ONE, e.ONE_MINUS_SRC_ALPHA, e.ONE, e.ONE_MINUS_SRC_ALPHA);
						break;
					case 2:
						e.blendFunc(e.ONE, e.ONE);
						break;
					case 3:
						e.blendFuncSeparate(e.ZERO, e.ONE_MINUS_SRC_COLOR, e.ZERO, e.ONE);
						break;
					case 4:
						e.blendFuncSeparate(e.DST_COLOR, e.ONE_MINUS_SRC_ALPHA, e.ZERO, e.ONE);
						break;
					default:
						G("WebGLState: Invalid blending: ", t);
						break;
				}
				else switch (t) {
					case 1:
						e.blendFuncSeparate(e.SRC_ALPHA, e.ONE_MINUS_SRC_ALPHA, e.ONE, e.ONE_MINUS_SRC_ALPHA);
						break;
					case 2:
						e.blendFuncSeparate(e.SRC_ALPHA, e.ONE, e.ONE, e.ONE);
						break;
					case 3:
						G("WebGLState: SubtractiveBlending requires material.premultipliedAlpha = true");
						break;
					case 4:
						G("WebGLState: MultiplyBlending requires material.premultipliedAlpha = true");
						break;
					default:
						G("WebGLState: Invalid blending: ", t);
						break;
				}
				y = null, b = null, S = null, C = null, w.set(0, 0, 0), T = 0, _ = t, E = u;
			}
			return;
		}
		a ||= n, o ||= r, s ||= i, (n !== v || a !== x) && (e.blendEquationSeparate(ve[n], ve[a]), v = n, x = a), (r !== y || i !== b || o !== S || s !== C) && (e.blendFuncSeparate(ye[r], ye[i], ye[o], ye[s]), y = r, b = i, S = o, C = s), (c.equals(w) === !1 || l !== T) && (e.blendColor(c.r, c.g, c.b, l), w.copy(c), T = l), _ = t, E = !1;
	}
	function xe(t, n) {
		t.side === 2 ? me(e.CULL_FACE) : pe(e.CULL_FACE);
		let r = t.side === 1;
		n && (r = !r), Se(r), t.blending === 1 && t.transparent === !1 ? be(0) : be(t.blending, t.blendEquation, t.blendSrc, t.blendDst, t.blendEquationAlpha, t.blendSrcAlpha, t.blendDstAlpha, t.blendColor, t.blendAlpha, t.premultipliedAlpha), o.setFunc(t.depthFunc), o.setTest(t.depthTest), o.setMask(t.depthWrite), a.setMask(t.colorWrite);
		let i = t.stencilWrite;
		s.setTest(i), i && (s.setMask(t.stencilWriteMask), s.setFunc(t.stencilFunc, t.stencilRef, t.stencilFuncMask), s.setOp(t.stencilFail, t.stencilZFail, t.stencilZPass)), M(t.polygonOffset, t.polygonOffsetFactor, t.polygonOffsetUnits), t.alphaToCoverage === !0 ? pe(e.SAMPLE_ALPHA_TO_COVERAGE) : me(e.SAMPLE_ALPHA_TO_COVERAGE);
	}
	function Se(t) {
		D !== t && (t ? e.frontFace(e.CW) : e.frontFace(e.CCW), D = t);
	}
	function Ce(t) {
		t === 0 ? me(e.CULL_FACE) : (pe(e.CULL_FACE), t !== O && (t === 1 ? e.cullFace(e.BACK) : t === 2 ? e.cullFace(e.FRONT) : e.cullFace(e.FRONT_AND_BACK))), O = t;
	}
	function j(t) {
		t !== k && (ne && e.lineWidth(t), k = t);
	}
	function M(t, n, r) {
		t ? (pe(e.POLYGON_OFFSET_FILL), (A !== n || ee !== r) && (A = n, ee = r, o.getReversed() && (n = -n), e.polygonOffset(n, r))) : me(e.POLYGON_OFFSET_FILL);
	}
	function we(t) {
		t ? pe(e.SCISSOR_TEST) : me(e.SCISSOR_TEST);
	}
	function N(t) {
		t === void 0 && (t = e.TEXTURE0 + te - 1), ae !== t && (e.activeTexture(t), ae = t);
	}
	function P(t, n, r) {
		r === void 0 && (r = ae === null ? e.TEXTURE0 + te - 1 : ae);
		let i = oe[r];
		i === void 0 && (i = {
			type: void 0,
			texture: void 0
		}, oe[r] = i), (i.type !== t || i.texture !== n) && (ae !== r && (e.activeTexture(r), ae = r), e.bindTexture(t, n || fe[t]), i.type = t, i.texture = n);
	}
	function Te() {
		let t = oe[ae];
		t !== void 0 && t.type !== void 0 && (e.bindTexture(t.type, null), t.type = void 0, t.texture = void 0);
	}
	function F() {
		try {
			e.compressedTexImage2D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function I() {
		try {
			e.compressedTexImage3D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function Ee() {
		try {
			e.texSubImage2D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function De() {
		try {
			e.texSubImage3D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function Oe() {
		try {
			e.compressedTexSubImage2D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function L() {
		try {
			e.compressedTexSubImage3D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function ke() {
		try {
			e.texStorage2D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function Ae() {
		try {
			e.texStorage3D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function je() {
		try {
			e.texImage2D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function Me() {
		try {
			e.texImage3D(...arguments);
		} catch (e) {
			G("WebGLState:", e);
		}
	}
	function Ne(t) {
		return d[t] === void 0 ? e.getParameter(t) : d[t];
	}
	function R(t, n) {
		d[t] !== n && (e.pixelStorei(t, n), d[t] = n);
	}
	function Pe(t) {
		le.equals(t) === !1 && (e.scissor(t.x, t.y, t.z, t.w), le.copy(t));
	}
	function Fe(t) {
		ue.equals(t) === !1 && (e.viewport(t.x, t.y, t.z, t.w), ue.copy(t));
	}
	function Ie(t, n) {
		let r = l.get(n);
		r === void 0 && (r = /* @__PURE__ */ new WeakMap(), l.set(n, r));
		let i = r.get(t);
		i === void 0 && (i = e.getUniformBlockIndex(n, t.name), r.set(t, i));
	}
	function Le(t, n) {
		let r = l.get(n).get(t);
		c.get(n) !== r && (e.uniformBlockBinding(n, r, t.__bindingPointIndex), c.set(n, r));
	}
	function Re() {
		e.disable(e.BLEND), e.disable(e.CULL_FACE), e.disable(e.DEPTH_TEST), e.disable(e.POLYGON_OFFSET_FILL), e.disable(e.SCISSOR_TEST), e.disable(e.STENCIL_TEST), e.disable(e.SAMPLE_ALPHA_TO_COVERAGE), e.blendEquation(e.FUNC_ADD), e.blendFunc(e.ONE, e.ZERO), e.blendFuncSeparate(e.ONE, e.ZERO, e.ONE, e.ZERO), e.blendColor(0, 0, 0, 0), e.colorMask(!0, !0, !0, !0), e.clearColor(0, 0, 0, 0), e.depthMask(!0), e.depthFunc(e.LESS), o.setReversed(!1), e.clearDepth(1), e.stencilMask(4294967295), e.stencilFunc(e.ALWAYS, 0, 4294967295), e.stencilOp(e.KEEP, e.KEEP, e.KEEP), e.clearStencil(0), e.cullFace(e.BACK), e.frontFace(e.CCW), e.polygonOffset(0, 0), e.activeTexture(e.TEXTURE0), e.bindFramebuffer(e.FRAMEBUFFER, null), e.bindFramebuffer(e.DRAW_FRAMEBUFFER, null), e.bindFramebuffer(e.READ_FRAMEBUFFER, null), e.useProgram(null), e.lineWidth(1), e.scissor(0, 0, e.canvas.width, e.canvas.height), e.viewport(0, 0, e.canvas.width, e.canvas.height), e.pixelStorei(e.PACK_ALIGNMENT, 4), e.pixelStorei(e.UNPACK_ALIGNMENT, 4), e.pixelStorei(e.UNPACK_FLIP_Y_WEBGL, !1), e.pixelStorei(e.UNPACK_PREMULTIPLY_ALPHA_WEBGL, !1), e.pixelStorei(e.UNPACK_COLORSPACE_CONVERSION_WEBGL, e.BROWSER_DEFAULT_WEBGL), e.pixelStorei(e.PACK_ROW_LENGTH, 0), e.pixelStorei(e.PACK_SKIP_PIXELS, 0), e.pixelStorei(e.PACK_SKIP_ROWS, 0), e.pixelStorei(e.UNPACK_ROW_LENGTH, 0), e.pixelStorei(e.UNPACK_IMAGE_HEIGHT, 0), e.pixelStorei(e.UNPACK_SKIP_PIXELS, 0), e.pixelStorei(e.UNPACK_SKIP_ROWS, 0), e.pixelStorei(e.UNPACK_SKIP_IMAGES, 0), u = {}, d = {}, ae = null, oe = {}, f = {}, p = /* @__PURE__ */ new WeakMap(), m = [], h = null, g = !1, _ = null, v = null, y = null, b = null, x = null, S = null, C = null, w = new X(0, 0, 0), T = 0, E = !1, D = null, O = null, k = null, A = null, ee = null, le.set(0, 0, e.canvas.width, e.canvas.height), ue.set(0, 0, e.canvas.width, e.canvas.height), a.reset(), o.reset(), s.reset();
	}
	return {
		buffers: {
			color: a,
			depth: o,
			stencil: s
		},
		enable: pe,
		disable: me,
		bindFramebuffer: he,
		drawBuffers: ge,
		useProgram: _e,
		setBlending: be,
		setMaterial: xe,
		setFlipSided: Se,
		setCullFace: Ce,
		setLineWidth: j,
		setPolygonOffset: M,
		setScissorTest: we,
		activeTexture: N,
		bindTexture: P,
		unbindTexture: Te,
		compressedTexImage2D: F,
		compressedTexImage3D: I,
		texImage2D: je,
		texImage3D: Me,
		pixelStorei: R,
		getParameter: Ne,
		updateUBOMapping: Ie,
		uniformBlockBinding: Le,
		texStorage2D: ke,
		texStorage3D: Ae,
		texSubImage2D: Ee,
		texSubImage3D: De,
		compressedTexSubImage2D: Oe,
		compressedTexSubImage3D: L,
		scissor: Pe,
		viewport: Fe,
		reset: Re
	};
}
function ef(e, t, n, r, i, a, o) {
	let s = t.has("WEBGL_multisampled_render_to_texture") ? t.get("WEBGL_multisampled_render_to_texture") : null, c = typeof navigator > "u" ? !1 : /OculusBrowser/g.test(navigator.userAgent), l = new Yr(), u = /* @__PURE__ */ new WeakMap(), d = /* @__PURE__ */ new Set(), f, p = /* @__PURE__ */ new WeakMap(), m = !1;
	try {
		m = typeof OffscreenCanvas < "u" && new OffscreenCanvas(1, 1).getContext("2d") !== null;
	} catch {}
	function h(e, t) {
		return m ? new OffscreenCanvas(e, t) : mr("canvas");
	}
	function g(e, t, n) {
		let r = 1, i = F(e);
		if ((i.width > n || i.height > n) && (r = n / Math.max(i.width, i.height)), r < 1) if (typeof HTMLImageElement < "u" && e instanceof HTMLImageElement || typeof HTMLCanvasElement < "u" && e instanceof HTMLCanvasElement || typeof ImageBitmap < "u" && e instanceof ImageBitmap || typeof VideoFrame < "u" && e instanceof VideoFrame) {
			let n = Math.floor(r * i.width), a = Math.floor(r * i.height);
			f === void 0 && (f = h(n, a));
			let o = t ? h(n, a) : f;
			return o.width = n, o.height = a, o.getContext("2d").drawImage(e, 0, 0, n, a), W("WebGLRenderer: Texture has been resized from (" + i.width + "x" + i.height + ") to (" + n + "x" + a + ")."), o;
		} else return "data" in e && W("WebGLRenderer: Image in DataTexture is too big (" + i.width + "x" + i.height + ")."), e;
		return e;
	}
	function _(e) {
		return e.generateMipmaps;
	}
	function v(t) {
		e.generateMipmap(t);
	}
	function y(t) {
		return t.isWebGLCubeRenderTarget ? e.TEXTURE_CUBE_MAP : t.isWebGL3DRenderTarget ? e.TEXTURE_3D : t.isWebGLArrayRenderTarget || t.isCompressedArrayTexture ? e.TEXTURE_2D_ARRAY : e.TEXTURE_2D;
	}
	function b(n, r, i, a, o, s = !1) {
		if (n !== null) {
			if (e[n] !== void 0) return e[n];
			W("WebGLRenderer: Attempt to use non-existing WebGL internal format '" + n + "'");
		}
		let c;
		a && (c = t.get("EXT_texture_norm16"), c || W("WebGLRenderer: Unable to use normalized textures without EXT_texture_norm16 extension"));
		let l = r;
		if (r === e.RED && (i === e.FLOAT && (l = e.R32F), i === e.HALF_FLOAT && (l = e.R16F), i === e.UNSIGNED_BYTE && (l = e.R8), i === e.UNSIGNED_SHORT && c && (l = c.R16_EXT), i === e.SHORT && c && (l = c.R16_SNORM_EXT)), r === e.RED_INTEGER && (i === e.UNSIGNED_BYTE && (l = e.R8UI), i === e.UNSIGNED_SHORT && (l = e.R16UI), i === e.UNSIGNED_INT && (l = e.R32UI), i === e.BYTE && (l = e.R8I), i === e.SHORT && (l = e.R16I), i === e.INT && (l = e.R32I)), r === e.RG && (i === e.FLOAT && (l = e.RG32F), i === e.HALF_FLOAT && (l = e.RG16F), i === e.UNSIGNED_BYTE && (l = e.RG8), i === e.UNSIGNED_SHORT && c && (l = c.RG16_EXT), i === e.SHORT && c && (l = c.RG16_SNORM_EXT)), r === e.RG_INTEGER && (i === e.UNSIGNED_BYTE && (l = e.RG8UI), i === e.UNSIGNED_SHORT && (l = e.RG16UI), i === e.UNSIGNED_INT && (l = e.RG32UI), i === e.BYTE && (l = e.RG8I), i === e.SHORT && (l = e.RG16I), i === e.INT && (l = e.RG32I)), r === e.RGB_INTEGER && (i === e.UNSIGNED_BYTE && (l = e.RGB8UI), i === e.UNSIGNED_SHORT && (l = e.RGB16UI), i === e.UNSIGNED_INT && (l = e.RGB32UI), i === e.BYTE && (l = e.RGB8I), i === e.SHORT && (l = e.RGB16I), i === e.INT && (l = e.RGB32I)), r === e.RGBA_INTEGER && (i === e.UNSIGNED_BYTE && (l = e.RGBA8UI), i === e.UNSIGNED_SHORT && (l = e.RGBA16UI), i === e.UNSIGNED_INT && (l = e.RGBA32UI), i === e.BYTE && (l = e.RGBA8I), i === e.SHORT && (l = e.RGBA16I), i === e.INT && (l = e.RGBA32I)), r === e.RGB && (i === e.UNSIGNED_SHORT && c && (l = c.RGB16_EXT), i === e.SHORT && c && (l = c.RGB16_SNORM_EXT), i === e.UNSIGNED_INT_5_9_9_9_REV && (l = e.RGB9_E5), i === e.UNSIGNED_INT_10F_11F_11F_REV && (l = e.R11F_G11F_B10F)), r === e.RGBA) {
			let t = s ? or : Y.getTransfer(o);
			i === e.FLOAT && (l = e.RGBA32F), i === e.HALF_FLOAT && (l = e.RGBA16F), i === e.UNSIGNED_BYTE && (l = t === "srgb" ? e.SRGB8_ALPHA8 : e.RGBA8), i === e.UNSIGNED_SHORT && c && (l = c.RGBA16_EXT), i === e.SHORT && c && (l = c.RGBA16_SNORM_EXT), i === e.UNSIGNED_SHORT_4_4_4_4 && (l = e.RGBA4), i === e.UNSIGNED_SHORT_5_5_5_1 && (l = e.RGB5_A1);
		}
		return (l === e.R16F || l === e.R32F || l === e.RG16F || l === e.RG32F || l === e.RGBA16F || l === e.RGBA32F) && t.get("EXT_color_buffer_float"), l;
	}
	function x(t, n) {
		let r;
		return t ? n === null || n === 1014 || n === 1020 ? r = e.DEPTH24_STENCIL8 : n === 1015 ? r = e.DEPTH32F_STENCIL8 : n === 1012 && (r = e.DEPTH24_STENCIL8, W("DepthTexture: 16 bit depth attachment is not supported with stencil. Using 24-bit attachment.")) : n === null || n === 1014 || n === 1020 ? r = e.DEPTH_COMPONENT24 : n === 1015 ? r = e.DEPTH_COMPONENT32F : n === 1012 && (r = e.DEPTH_COMPONENT16), r;
	}
	function S(e, t) {
		return _(e) === !0 || e.isFramebufferTexture && e.minFilter !== 1003 && e.minFilter !== 1006 ? Math.log2(Math.max(t.width, t.height)) + 1 : e.mipmaps !== void 0 && e.mipmaps.length > 0 ? e.mipmaps.length : e.isCompressedTexture && Array.isArray(e.image) ? t.mipmaps.length : 1;
	}
	function C(e) {
		let t = e.target;
		t.removeEventListener("dispose", C), T(t), t.isVideoTexture && u.delete(t), t.isHTMLTexture && d.delete(t);
	}
	function w(e) {
		let t = e.target;
		t.removeEventListener("dispose", w), D(t);
	}
	function T(e) {
		let t = r.get(e);
		if (t.__webglInit === void 0) return;
		let n = e.source, i = p.get(n);
		if (i) {
			let r = i[t.__cacheKey];
			r.usedTimes--, r.usedTimes === 0 && E(e), Object.keys(i).length === 0 && p.delete(n);
		}
		r.remove(e);
	}
	function E(t) {
		let n = r.get(t);
		e.deleteTexture(n.__webglTexture);
		let i = t.source, a = p.get(i);
		delete a[n.__cacheKey], o.memory.textures--;
	}
	function D(t) {
		let n = r.get(t);
		if (t.depthTexture && (t.depthTexture.dispose(), r.remove(t.depthTexture)), t.isWebGLCubeRenderTarget) for (let t = 0; t < 6; t++) {
			if (Array.isArray(n.__webglFramebuffer[t])) for (let r = 0; r < n.__webglFramebuffer[t].length; r++) e.deleteFramebuffer(n.__webglFramebuffer[t][r]);
			else e.deleteFramebuffer(n.__webglFramebuffer[t]);
			n.__webglDepthbuffer && e.deleteRenderbuffer(n.__webglDepthbuffer[t]);
		}
		else {
			if (Array.isArray(n.__webglFramebuffer)) for (let t = 0; t < n.__webglFramebuffer.length; t++) e.deleteFramebuffer(n.__webglFramebuffer[t]);
			else e.deleteFramebuffer(n.__webglFramebuffer);
			if (n.__webglDepthbuffer && e.deleteRenderbuffer(n.__webglDepthbuffer), n.__webglMultisampledFramebuffer && e.deleteFramebuffer(n.__webglMultisampledFramebuffer), n.__webglColorRenderbuffer) for (let t = 0; t < n.__webglColorRenderbuffer.length; t++) n.__webglColorRenderbuffer[t] && e.deleteRenderbuffer(n.__webglColorRenderbuffer[t]);
			n.__webglDepthRenderbuffer && e.deleteRenderbuffer(n.__webglDepthRenderbuffer);
		}
		let i = t.textures;
		for (let t = 0, n = i.length; t < n; t++) {
			let n = r.get(i[t]);
			n.__webglTexture && (e.deleteTexture(n.__webglTexture), o.memory.textures--), r.remove(i[t]);
		}
		r.remove(t);
	}
	let O = 0;
	function k() {
		O = 0;
	}
	function A() {
		return O;
	}
	function ee(e) {
		O = e;
	}
	function te() {
		let e = O;
		return e >= i.maxTextures && W("WebGLTextures: Trying to use " + e + " texture units while this GPU supports only " + i.maxTextures), O += 1, e;
	}
	function ne(e) {
		let t = [];
		return t.push(e.wrapS), t.push(e.wrapT), t.push(e.wrapR || 0), t.push(e.magFilter), t.push(e.minFilter), t.push(e.anisotropy), t.push(e.internalFormat), t.push(e.format), t.push(e.type), t.push(e.generateMipmaps), t.push(e.premultiplyAlpha), t.push(e.flipY), t.push(e.unpackAlignment), t.push(e.colorSpace), t.join();
	}
	function re(t, i) {
		let a = r.get(t);
		if (t.isVideoTexture && P(t), t.isRenderTargetTexture === !1 && t.isExternalTexture !== !0 && t.version > 0 && a.__version !== t.version) {
			let e = t.image;
			if (e === null) W("WebGLRenderer: Texture marked for update but no image data found.");
			else if (e.complete === !1) W("WebGLRenderer: Texture marked for update but image is incomplete");
			else {
				me(a, t, i);
				return;
			}
		} else t.isExternalTexture && (a.__webglTexture = t.sourceTexture ? t.sourceTexture : null);
		n.bindTexture(e.TEXTURE_2D, a.__webglTexture, e.TEXTURE0 + i);
	}
	function ie(t, i) {
		let a = r.get(t);
		if (t.isRenderTargetTexture === !1 && t.version > 0 && a.__version !== t.version) {
			me(a, t, i);
			return;
		} else t.isExternalTexture && (a.__webglTexture = t.sourceTexture ? t.sourceTexture : null);
		n.bindTexture(e.TEXTURE_2D_ARRAY, a.__webglTexture, e.TEXTURE0 + i);
	}
	function ae(t, i) {
		let a = r.get(t);
		if (t.isRenderTargetTexture === !1 && t.version > 0 && a.__version !== t.version) {
			me(a, t, i);
			return;
		}
		n.bindTexture(e.TEXTURE_3D, a.__webglTexture, e.TEXTURE0 + i);
	}
	function oe(t, i) {
		let a = r.get(t);
		if (t.isCubeDepthTexture !== !0 && t.version > 0 && a.__version !== t.version) {
			he(a, t, i);
			return;
		}
		n.bindTexture(e.TEXTURE_CUBE_MAP, a.__webglTexture, e.TEXTURE0 + i);
	}
	let se = {
		[At]: e.REPEAT,
		[jt]: e.CLAMP_TO_EDGE,
		[Mt]: e.MIRRORED_REPEAT
	}, ce = {
		[Nt]: e.NEAREST,
		[Pt]: e.NEAREST_MIPMAP_NEAREST,
		[Ft]: e.NEAREST_MIPMAP_LINEAR,
		[It]: e.LINEAR,
		[Lt]: e.LINEAR_MIPMAP_NEAREST,
		[Rt]: e.LINEAR_MIPMAP_LINEAR
	}, le = {
		512: e.NEVER,
		519: e.ALWAYS,
		513: e.LESS,
		515: e.LEQUAL,
		514: e.EQUAL,
		518: e.GEQUAL,
		516: e.GREATER,
		517: e.NOTEQUAL
	};
	function ue(n, a) {
		if (a.type === 1015 && t.has("OES_texture_float_linear") === !1 && (a.magFilter === 1006 || a.magFilter === 1007 || a.magFilter === 1005 || a.magFilter === 1008 || a.minFilter === 1006 || a.minFilter === 1007 || a.minFilter === 1005 || a.minFilter === 1008) && W("WebGLRenderer: Unable to use linear filtering with floating point textures. OES_texture_float_linear not supported on this device."), e.texParameteri(n, e.TEXTURE_WRAP_S, se[a.wrapS]), e.texParameteri(n, e.TEXTURE_WRAP_T, se[a.wrapT]), (n === e.TEXTURE_3D || n === e.TEXTURE_2D_ARRAY) && e.texParameteri(n, e.TEXTURE_WRAP_R, se[a.wrapR]), e.texParameteri(n, e.TEXTURE_MAG_FILTER, ce[a.magFilter]), e.texParameteri(n, e.TEXTURE_MIN_FILTER, ce[a.minFilter]), a.compareFunction && (e.texParameteri(n, e.TEXTURE_COMPARE_MODE, e.COMPARE_REF_TO_TEXTURE), e.texParameteri(n, e.TEXTURE_COMPARE_FUNC, le[a.compareFunction])), t.has("EXT_texture_filter_anisotropic") === !0) {
			if (a.magFilter === 1003 || a.minFilter !== 1005 && a.minFilter !== 1008 || a.type === 1015 && t.has("OES_texture_float_linear") === !1) return;
			if (a.anisotropy > 1 || r.get(a).__currentAnisotropy) {
				let o = t.get("EXT_texture_filter_anisotropic");
				e.texParameterf(n, o.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(a.anisotropy, i.getMaxAnisotropy())), r.get(a).__currentAnisotropy = a.anisotropy;
			}
		}
	}
	function de(t, n) {
		let r = !1;
		t.__webglInit === void 0 && (t.__webglInit = !0, n.addEventListener("dispose", C));
		let i = n.source, a = p.get(i);
		a === void 0 && (a = {}, p.set(i, a));
		let s = ne(n);
		if (s !== t.__cacheKey) {
			a[s] === void 0 && (a[s] = {
				texture: e.createTexture(),
				usedTimes: 0
			}, o.memory.textures++, r = !0), a[s].usedTimes++;
			let i = a[t.__cacheKey];
			i !== void 0 && (a[t.__cacheKey].usedTimes--, i.usedTimes === 0 && E(n)), t.__cacheKey = s, t.__webglTexture = a[s].texture;
		}
		return r;
	}
	function fe(e, t, n) {
		return Math.floor(Math.floor(e / n) / t);
	}
	function pe(t, r, i, a) {
		let o = t.updateRanges;
		if (o.length === 0) n.texSubImage2D(e.TEXTURE_2D, 0, 0, 0, r.width, r.height, i, a, r.data);
		else {
			o.sort((e, t) => e.start - t.start);
			let s = 0;
			for (let e = 1; e < o.length; e++) {
				let t = o[s], n = o[e], i = t.start + t.count, a = fe(n.start, r.width, 4), c = fe(t.start, r.width, 4);
				n.start <= i + 1 && a === c && fe(n.start + n.count - 1, r.width, 4) === a ? t.count = Math.max(t.count, n.start + n.count - t.start) : (++s, o[s] = n);
			}
			o.length = s + 1;
			let c = n.getParameter(e.UNPACK_ROW_LENGTH), l = n.getParameter(e.UNPACK_SKIP_PIXELS), u = n.getParameter(e.UNPACK_SKIP_ROWS);
			n.pixelStorei(e.UNPACK_ROW_LENGTH, r.width);
			for (let t = 0, s = o.length; t < s; t++) {
				let s = o[t], c = Math.floor(s.start / 4), l = Math.ceil(s.count / 4), u = c % r.width, d = Math.floor(c / r.width), f = l;
				n.pixelStorei(e.UNPACK_SKIP_PIXELS, u), n.pixelStorei(e.UNPACK_SKIP_ROWS, d), n.texSubImage2D(e.TEXTURE_2D, 0, u, d, f, 1, i, a, r.data);
			}
			t.clearUpdateRanges(), n.pixelStorei(e.UNPACK_ROW_LENGTH, c), n.pixelStorei(e.UNPACK_SKIP_PIXELS, l), n.pixelStorei(e.UNPACK_SKIP_ROWS, u);
		}
	}
	function me(t, o, s) {
		let c = e.TEXTURE_2D;
		(o.isDataArrayTexture || o.isCompressedArrayTexture) && (c = e.TEXTURE_2D_ARRAY), o.isData3DTexture && (c = e.TEXTURE_3D);
		let l = de(t, o), u = o.source;
		n.bindTexture(c, t.__webglTexture, e.TEXTURE0 + s);
		let f = r.get(u);
		if (u.version !== f.__version || l === !0) {
			if (n.activeTexture(e.TEXTURE0 + s), !(typeof ImageBitmap < "u" && o.image instanceof ImageBitmap)) {
				let t = Y.getPrimaries(Y.workingColorSpace), r = o.colorSpace === "" ? null : Y.getPrimaries(o.colorSpace), i = o.colorSpace === "" || t === r ? e.NONE : e.BROWSER_DEFAULT_WEBGL;
				n.pixelStorei(e.UNPACK_FLIP_Y_WEBGL, o.flipY), n.pixelStorei(e.UNPACK_PREMULTIPLY_ALPHA_WEBGL, o.premultiplyAlpha), n.pixelStorei(e.UNPACK_COLORSPACE_CONVERSION_WEBGL, i);
			}
			n.pixelStorei(e.UNPACK_ALIGNMENT, o.unpackAlignment);
			let t = g(o.image, !1, i.maxTextureSize);
			t = Te(o, t);
			let r = a.convert(o.format, o.colorSpace), p = a.convert(o.type), m = b(o.internalFormat, r, p, o.normalized, o.colorSpace, o.isVideoTexture);
			ue(c, o);
			let h, y = o.mipmaps, C = o.isVideoTexture !== !0, w = f.__version === void 0 || l === !0, T = u.dataReady, E = S(o, t);
			if (o.isDepthTexture) m = x(o.format === nn, o.type), w && (C ? n.texStorage2D(e.TEXTURE_2D, 1, m, t.width, t.height) : n.texImage2D(e.TEXTURE_2D, 0, m, t.width, t.height, 0, r, p, null));
			else if (o.isDataTexture) if (y.length > 0) {
				C && w && n.texStorage2D(e.TEXTURE_2D, E, m, y[0].width, y[0].height);
				for (let t = 0, i = y.length; t < i; t++) h = y[t], C ? T && n.texSubImage2D(e.TEXTURE_2D, t, 0, 0, h.width, h.height, r, p, h.data) : n.texImage2D(e.TEXTURE_2D, t, m, h.width, h.height, 0, r, p, h.data);
				o.generateMipmaps = !1;
			} else C ? (w && n.texStorage2D(e.TEXTURE_2D, E, m, t.width, t.height), T && pe(o, t, r, p)) : n.texImage2D(e.TEXTURE_2D, 0, m, t.width, t.height, 0, r, p, t.data);
			else if (o.isCompressedTexture) if (o.isCompressedArrayTexture) {
				C && w && n.texStorage3D(e.TEXTURE_2D_ARRAY, E, m, y[0].width, y[0].height, t.depth);
				for (let i = 0, a = y.length; i < a; i++) if (h = y[i], o.format !== 1023) if (r !== null) if (C) {
					if (T) if (o.layerUpdates.size > 0) {
						let t = Xc(h.width, h.height, o.format, o.type);
						for (let a of o.layerUpdates) {
							let o = h.data.subarray(a * t / h.data.BYTES_PER_ELEMENT, (a + 1) * t / h.data.BYTES_PER_ELEMENT);
							n.compressedTexSubImage3D(e.TEXTURE_2D_ARRAY, i, 0, 0, a, h.width, h.height, 1, r, o);
						}
						o.clearLayerUpdates();
					} else n.compressedTexSubImage3D(e.TEXTURE_2D_ARRAY, i, 0, 0, 0, h.width, h.height, t.depth, r, h.data);
				} else n.compressedTexImage3D(e.TEXTURE_2D_ARRAY, i, m, h.width, h.height, t.depth, 0, h.data, 0, 0);
				else W("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()");
				else C ? T && n.texSubImage3D(e.TEXTURE_2D_ARRAY, i, 0, 0, 0, h.width, h.height, t.depth, r, p, h.data) : n.texImage3D(e.TEXTURE_2D_ARRAY, i, m, h.width, h.height, t.depth, 0, r, p, h.data);
			} else {
				C && w && n.texStorage2D(e.TEXTURE_2D, E, m, y[0].width, y[0].height);
				for (let t = 0, i = y.length; t < i; t++) h = y[t], o.format === 1023 ? C ? T && n.texSubImage2D(e.TEXTURE_2D, t, 0, 0, h.width, h.height, r, p, h.data) : n.texImage2D(e.TEXTURE_2D, t, m, h.width, h.height, 0, r, p, h.data) : r === null ? W("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()") : C ? T && n.compressedTexSubImage2D(e.TEXTURE_2D, t, 0, 0, h.width, h.height, r, h.data) : n.compressedTexImage2D(e.TEXTURE_2D, t, m, h.width, h.height, 0, h.data);
			}
			else if (o.isDataArrayTexture) if (C) {
				if (w && n.texStorage3D(e.TEXTURE_2D_ARRAY, E, m, t.width, t.height, t.depth), T) if (o.layerUpdates.size > 0) {
					let i = Xc(t.width, t.height, o.format, o.type);
					for (let a of o.layerUpdates) {
						let o = t.data.subarray(a * i / t.data.BYTES_PER_ELEMENT, (a + 1) * i / t.data.BYTES_PER_ELEMENT);
						n.texSubImage3D(e.TEXTURE_2D_ARRAY, 0, 0, 0, a, t.width, t.height, 1, r, p, o);
					}
					o.clearLayerUpdates();
				} else n.texSubImage3D(e.TEXTURE_2D_ARRAY, 0, 0, 0, 0, t.width, t.height, t.depth, r, p, t.data);
			} else n.texImage3D(e.TEXTURE_2D_ARRAY, 0, m, t.width, t.height, t.depth, 0, r, p, t.data);
			else if (o.isData3DTexture) C ? (w && n.texStorage3D(e.TEXTURE_3D, E, m, t.width, t.height, t.depth), T && n.texSubImage3D(e.TEXTURE_3D, 0, 0, 0, 0, t.width, t.height, t.depth, r, p, t.data)) : n.texImage3D(e.TEXTURE_3D, 0, m, t.width, t.height, t.depth, 0, r, p, t.data);
			else if (o.isFramebufferTexture) {
				if (w) if (C) n.texStorage2D(e.TEXTURE_2D, E, m, t.width, t.height);
				else {
					let i = t.width, a = t.height;
					for (let t = 0; t < E; t++) n.texImage2D(e.TEXTURE_2D, t, m, i, a, 0, r, p, null), i >>= 1, a >>= 1;
				}
			} else if (o.isHTMLTexture) {
				if ("texElementImage2D" in e) {
					let n = e.canvas;
					if (n.hasAttribute("layoutsubtree") || n.setAttribute("layoutsubtree", "true"), t.parentNode !== n) {
						n.appendChild(t), d.add(o), n.onpaint = (e) => {
							let t = e.changedElements;
							for (let e of d) t.includes(e.image) && (e.needsUpdate = !0);
						}, n.requestPaint();
						return;
					}
					let r = e.RGBA, i = e.RGBA, a = e.UNSIGNED_BYTE;
					e.texElementImage2D(e.TEXTURE_2D, 0, r, i, a, t), e.texParameteri(e.TEXTURE_2D, e.TEXTURE_MIN_FILTER, e.LINEAR), e.texParameteri(e.TEXTURE_2D, e.TEXTURE_WRAP_S, e.CLAMP_TO_EDGE), e.texParameteri(e.TEXTURE_2D, e.TEXTURE_WRAP_T, e.CLAMP_TO_EDGE);
				}
			} else if (y.length > 0) {
				if (C && w) {
					let t = F(y[0]);
					n.texStorage2D(e.TEXTURE_2D, E, m, t.width, t.height);
				}
				for (let t = 0, i = y.length; t < i; t++) h = y[t], C ? T && n.texSubImage2D(e.TEXTURE_2D, t, 0, 0, r, p, h) : n.texImage2D(e.TEXTURE_2D, t, m, r, p, h);
				o.generateMipmaps = !1;
			} else if (C) {
				if (w) {
					let r = F(t);
					n.texStorage2D(e.TEXTURE_2D, E, m, r.width, r.height);
				}
				T && n.texSubImage2D(e.TEXTURE_2D, 0, 0, 0, r, p, t);
			} else n.texImage2D(e.TEXTURE_2D, 0, m, r, p, t);
			_(o) && v(c), f.__version = u.version, o.onUpdate && o.onUpdate(o);
		}
		t.__version = o.version;
	}
	function he(t, o, s) {
		if (o.image.length !== 6) return;
		let c = de(t, o), l = o.source;
		n.bindTexture(e.TEXTURE_CUBE_MAP, t.__webglTexture, e.TEXTURE0 + s);
		let u = r.get(l);
		if (l.version !== u.__version || c === !0) {
			n.activeTexture(e.TEXTURE0 + s);
			let t = Y.getPrimaries(Y.workingColorSpace), r = o.colorSpace === "" ? null : Y.getPrimaries(o.colorSpace), d = o.colorSpace === "" || t === r ? e.NONE : e.BROWSER_DEFAULT_WEBGL;
			n.pixelStorei(e.UNPACK_FLIP_Y_WEBGL, o.flipY), n.pixelStorei(e.UNPACK_PREMULTIPLY_ALPHA_WEBGL, o.premultiplyAlpha), n.pixelStorei(e.UNPACK_ALIGNMENT, o.unpackAlignment), n.pixelStorei(e.UNPACK_COLORSPACE_CONVERSION_WEBGL, d);
			let f = o.isCompressedTexture || o.image[0].isCompressedTexture, p = o.image[0] && o.image[0].isDataTexture, m = [];
			for (let e = 0; e < 6; e++) !f && !p ? m[e] = g(o.image[e], !0, i.maxCubemapSize) : m[e] = p ? o.image[e].image : o.image[e], m[e] = Te(o, m[e]);
			let h = m[0], y = a.convert(o.format, o.colorSpace), x = a.convert(o.type), C = b(o.internalFormat, y, x, o.normalized, o.colorSpace), w = o.isVideoTexture !== !0, T = u.__version === void 0 || c === !0, E = l.dataReady, D = S(o, h);
			ue(e.TEXTURE_CUBE_MAP, o);
			let O;
			if (f) {
				w && T && n.texStorage2D(e.TEXTURE_CUBE_MAP, D, C, h.width, h.height);
				for (let t = 0; t < 6; t++) {
					O = m[t].mipmaps;
					for (let r = 0; r < O.length; r++) {
						let i = O[r];
						o.format === 1023 ? w ? E && n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r, 0, 0, i.width, i.height, y, x, i.data) : n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r, C, i.width, i.height, 0, y, x, i.data) : y === null ? W("WebGLRenderer: Attempt to load unsupported compressed texture format in .setTextureCube()") : w ? E && n.compressedTexSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r, 0, 0, i.width, i.height, y, i.data) : n.compressedTexImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r, C, i.width, i.height, 0, i.data);
					}
				}
			} else {
				if (O = o.mipmaps, w && T) {
					O.length > 0 && D++;
					let t = F(m[0]);
					n.texStorage2D(e.TEXTURE_CUBE_MAP, D, C, t.width, t.height);
				}
				for (let t = 0; t < 6; t++) if (p) {
					w ? E && n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, 0, 0, 0, m[t].width, m[t].height, y, x, m[t].data) : n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, 0, C, m[t].width, m[t].height, 0, y, x, m[t].data);
					for (let r = 0; r < O.length; r++) {
						let i = O[r].image[t].image;
						w ? E && n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r + 1, 0, 0, i.width, i.height, y, x, i.data) : n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r + 1, C, i.width, i.height, 0, y, x, i.data);
					}
				} else {
					w ? E && n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, 0, 0, 0, y, x, m[t]) : n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, 0, C, y, x, m[t]);
					for (let r = 0; r < O.length; r++) {
						let i = O[r];
						w ? E && n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r + 1, 0, 0, y, x, i.image[t]) : n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + t, r + 1, C, y, x, i.image[t]);
					}
				}
			}
			_(o) && v(e.TEXTURE_CUBE_MAP), u.__version = l.version, o.onUpdate && o.onUpdate(o);
		}
		t.__version = o.version;
	}
	function ge(t, i, o, c, l, u) {
		let d = a.convert(o.format, o.colorSpace), f = a.convert(o.type), p = b(o.internalFormat, d, f, o.normalized, o.colorSpace), m = r.get(i), h = r.get(o);
		if (h.__renderTarget = i, !m.__hasExternalTextures) {
			let t = Math.max(1, i.width >> u), r = Math.max(1, i.height >> u);
			l === e.TEXTURE_3D || l === e.TEXTURE_2D_ARRAY ? n.texImage3D(l, u, p, t, r, i.depth, 0, d, f, null) : n.texImage2D(l, u, p, t, r, 0, d, f, null);
		}
		n.bindFramebuffer(e.FRAMEBUFFER, t), N(i) ? s.framebufferTexture2DMultisampleEXT(e.FRAMEBUFFER, c, l, h.__webglTexture, 0, we(i)) : (l === e.TEXTURE_2D || l >= e.TEXTURE_CUBE_MAP_POSITIVE_X && l <= e.TEXTURE_CUBE_MAP_NEGATIVE_Z) && e.framebufferTexture2D(e.FRAMEBUFFER, c, l, h.__webglTexture, u), n.bindFramebuffer(e.FRAMEBUFFER, null);
	}
	function _e(t, n, r) {
		if (e.bindRenderbuffer(e.RENDERBUFFER, t), n.depthBuffer) {
			let i = n.depthTexture, a = i && i.isDepthTexture ? i.type : null, o = x(n.stencilBuffer, a), c = n.stencilBuffer ? e.DEPTH_STENCIL_ATTACHMENT : e.DEPTH_ATTACHMENT;
			N(n) ? s.renderbufferStorageMultisampleEXT(e.RENDERBUFFER, we(n), o, n.width, n.height) : r ? e.renderbufferStorageMultisample(e.RENDERBUFFER, we(n), o, n.width, n.height) : e.renderbufferStorage(e.RENDERBUFFER, o, n.width, n.height), e.framebufferRenderbuffer(e.FRAMEBUFFER, c, e.RENDERBUFFER, t);
		} else {
			let t = n.textures;
			for (let i = 0; i < t.length; i++) {
				let o = t[i], c = a.convert(o.format, o.colorSpace), l = a.convert(o.type), u = b(o.internalFormat, c, l, o.normalized, o.colorSpace);
				N(n) ? s.renderbufferStorageMultisampleEXT(e.RENDERBUFFER, we(n), u, n.width, n.height) : r ? e.renderbufferStorageMultisample(e.RENDERBUFFER, we(n), u, n.width, n.height) : e.renderbufferStorage(e.RENDERBUFFER, u, n.width, n.height);
			}
		}
		e.bindRenderbuffer(e.RENDERBUFFER, null);
	}
	function ve(t, i, o) {
		let c = i.isWebGLCubeRenderTarget === !0;
		if (n.bindFramebuffer(e.FRAMEBUFFER, t), !(i.depthTexture && i.depthTexture.isDepthTexture)) throw Error("renderTarget.depthTexture must be an instance of THREE.DepthTexture");
		let l = r.get(i.depthTexture);
		if (l.__renderTarget = i, (!l.__webglTexture || i.depthTexture.image.width !== i.width || i.depthTexture.image.height !== i.height) && (i.depthTexture.image.width = i.width, i.depthTexture.image.height = i.height, i.depthTexture.needsUpdate = !0), c) {
			if (l.__webglInit === void 0 && (l.__webglInit = !0, i.depthTexture.addEventListener("dispose", C)), l.__webglTexture === void 0) {
				l.__webglTexture = e.createTexture(), n.bindTexture(e.TEXTURE_CUBE_MAP, l.__webglTexture), ue(e.TEXTURE_CUBE_MAP, i.depthTexture);
				let t = a.convert(i.depthTexture.format), r = a.convert(i.depthTexture.type), o;
				i.depthTexture.format === 1026 ? o = e.DEPTH_COMPONENT24 : i.depthTexture.format === 1027 && (o = e.DEPTH24_STENCIL8);
				for (let n = 0; n < 6; n++) e.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X + n, 0, o, i.width, i.height, 0, t, r, null);
			}
		} else re(i.depthTexture, 0);
		let u = l.__webglTexture, d = we(i), f = c ? e.TEXTURE_CUBE_MAP_POSITIVE_X + o : e.TEXTURE_2D, p = i.depthTexture.format === 1027 ? e.DEPTH_STENCIL_ATTACHMENT : e.DEPTH_ATTACHMENT;
		if (i.depthTexture.format === 1026) N(i) ? s.framebufferTexture2DMultisampleEXT(e.FRAMEBUFFER, p, f, u, 0, d) : e.framebufferTexture2D(e.FRAMEBUFFER, p, f, u, 0);
		else if (i.depthTexture.format === 1027) N(i) ? s.framebufferTexture2DMultisampleEXT(e.FRAMEBUFFER, p, f, u, 0, d) : e.framebufferTexture2D(e.FRAMEBUFFER, p, f, u, 0);
		else throw Error("Unknown depthTexture format");
	}
	function ye(t) {
		let i = r.get(t), a = t.isWebGLCubeRenderTarget === !0;
		if (i.__boundDepthTexture !== t.depthTexture) {
			let e = t.depthTexture;
			if (i.__depthDisposeCallback && i.__depthDisposeCallback(), e) {
				let t = () => {
					delete i.__boundDepthTexture, delete i.__depthDisposeCallback, e.removeEventListener("dispose", t);
				};
				e.addEventListener("dispose", t), i.__depthDisposeCallback = t;
			}
			i.__boundDepthTexture = e;
		}
		if (t.depthTexture && !i.__autoAllocateDepthBuffer) if (a) for (let e = 0; e < 6; e++) ve(i.__webglFramebuffer[e], t, e);
		else {
			let e = t.texture.mipmaps;
			e && e.length > 0 ? ve(i.__webglFramebuffer[0], t, 0) : ve(i.__webglFramebuffer, t, 0);
		}
		else if (a) {
			i.__webglDepthbuffer = [];
			for (let r = 0; r < 6; r++) if (n.bindFramebuffer(e.FRAMEBUFFER, i.__webglFramebuffer[r]), i.__webglDepthbuffer[r] === void 0) i.__webglDepthbuffer[r] = e.createRenderbuffer(), _e(i.__webglDepthbuffer[r], t, !1);
			else {
				let n = t.stencilBuffer ? e.DEPTH_STENCIL_ATTACHMENT : e.DEPTH_ATTACHMENT, a = i.__webglDepthbuffer[r];
				e.bindRenderbuffer(e.RENDERBUFFER, a), e.framebufferRenderbuffer(e.FRAMEBUFFER, n, e.RENDERBUFFER, a);
			}
		} else {
			let r = t.texture.mipmaps;
			if (r && r.length > 0 ? n.bindFramebuffer(e.FRAMEBUFFER, i.__webglFramebuffer[0]) : n.bindFramebuffer(e.FRAMEBUFFER, i.__webglFramebuffer), i.__webglDepthbuffer === void 0) i.__webglDepthbuffer = e.createRenderbuffer(), _e(i.__webglDepthbuffer, t, !1);
			else {
				let n = t.stencilBuffer ? e.DEPTH_STENCIL_ATTACHMENT : e.DEPTH_ATTACHMENT, r = i.__webglDepthbuffer;
				e.bindRenderbuffer(e.RENDERBUFFER, r), e.framebufferRenderbuffer(e.FRAMEBUFFER, n, e.RENDERBUFFER, r);
			}
		}
		n.bindFramebuffer(e.FRAMEBUFFER, null);
	}
	function be(t, n, i) {
		let a = r.get(t);
		n !== void 0 && ge(a.__webglFramebuffer, t, t.texture, e.COLOR_ATTACHMENT0, e.TEXTURE_2D, 0), i !== void 0 && ye(t);
	}
	function xe(t) {
		let i = t.texture, s = r.get(t), c = r.get(i);
		t.addEventListener("dispose", w);
		let l = t.textures, u = t.isWebGLCubeRenderTarget === !0, d = l.length > 1;
		if (d || (c.__webglTexture === void 0 && (c.__webglTexture = e.createTexture()), c.__version = i.version, o.memory.textures++), u) {
			s.__webglFramebuffer = [];
			for (let t = 0; t < 6; t++) if (i.mipmaps && i.mipmaps.length > 0) {
				s.__webglFramebuffer[t] = [];
				for (let n = 0; n < i.mipmaps.length; n++) s.__webglFramebuffer[t][n] = e.createFramebuffer();
			} else s.__webglFramebuffer[t] = e.createFramebuffer();
		} else {
			if (i.mipmaps && i.mipmaps.length > 0) {
				s.__webglFramebuffer = [];
				for (let t = 0; t < i.mipmaps.length; t++) s.__webglFramebuffer[t] = e.createFramebuffer();
			} else s.__webglFramebuffer = e.createFramebuffer();
			if (d) for (let t = 0, n = l.length; t < n; t++) {
				let n = r.get(l[t]);
				n.__webglTexture === void 0 && (n.__webglTexture = e.createTexture(), o.memory.textures++);
			}
			if (t.samples > 0 && N(t) === !1) {
				s.__webglMultisampledFramebuffer = e.createFramebuffer(), s.__webglColorRenderbuffer = [], n.bindFramebuffer(e.FRAMEBUFFER, s.__webglMultisampledFramebuffer);
				for (let n = 0; n < l.length; n++) {
					let r = l[n];
					s.__webglColorRenderbuffer[n] = e.createRenderbuffer(), e.bindRenderbuffer(e.RENDERBUFFER, s.__webglColorRenderbuffer[n]);
					let i = a.convert(r.format, r.colorSpace), o = a.convert(r.type), c = b(r.internalFormat, i, o, r.normalized, r.colorSpace, t.isXRRenderTarget === !0), u = we(t);
					e.renderbufferStorageMultisample(e.RENDERBUFFER, u, c, t.width, t.height), e.framebufferRenderbuffer(e.FRAMEBUFFER, e.COLOR_ATTACHMENT0 + n, e.RENDERBUFFER, s.__webglColorRenderbuffer[n]);
				}
				e.bindRenderbuffer(e.RENDERBUFFER, null), t.depthBuffer && (s.__webglDepthRenderbuffer = e.createRenderbuffer(), _e(s.__webglDepthRenderbuffer, t, !0)), n.bindFramebuffer(e.FRAMEBUFFER, null);
			}
		}
		if (u) {
			n.bindTexture(e.TEXTURE_CUBE_MAP, c.__webglTexture), ue(e.TEXTURE_CUBE_MAP, i);
			for (let n = 0; n < 6; n++) if (i.mipmaps && i.mipmaps.length > 0) for (let r = 0; r < i.mipmaps.length; r++) ge(s.__webglFramebuffer[n][r], t, i, e.COLOR_ATTACHMENT0, e.TEXTURE_CUBE_MAP_POSITIVE_X + n, r);
			else ge(s.__webglFramebuffer[n], t, i, e.COLOR_ATTACHMENT0, e.TEXTURE_CUBE_MAP_POSITIVE_X + n, 0);
			_(i) && v(e.TEXTURE_CUBE_MAP), n.unbindTexture();
		} else if (d) {
			for (let i = 0, a = l.length; i < a; i++) {
				let a = l[i], o = r.get(a), c = e.TEXTURE_2D;
				(t.isWebGL3DRenderTarget || t.isWebGLArrayRenderTarget) && (c = t.isWebGL3DRenderTarget ? e.TEXTURE_3D : e.TEXTURE_2D_ARRAY), n.bindTexture(c, o.__webglTexture), ue(c, a), ge(s.__webglFramebuffer, t, a, e.COLOR_ATTACHMENT0 + i, c, 0), _(a) && v(c);
			}
			n.unbindTexture();
		} else {
			let r = e.TEXTURE_2D;
			if ((t.isWebGL3DRenderTarget || t.isWebGLArrayRenderTarget) && (r = t.isWebGL3DRenderTarget ? e.TEXTURE_3D : e.TEXTURE_2D_ARRAY), n.bindTexture(r, c.__webglTexture), ue(r, i), i.mipmaps && i.mipmaps.length > 0) for (let n = 0; n < i.mipmaps.length; n++) ge(s.__webglFramebuffer[n], t, i, e.COLOR_ATTACHMENT0, r, n);
			else ge(s.__webglFramebuffer, t, i, e.COLOR_ATTACHMENT0, r, 0);
			_(i) && v(r), n.unbindTexture();
		}
		t.depthBuffer && ye(t);
	}
	function Se(e) {
		let t = e.textures;
		for (let i = 0, a = t.length; i < a; i++) {
			let a = t[i];
			if (_(a)) {
				let t = y(e), i = r.get(a).__webglTexture;
				n.bindTexture(t, i), v(t), n.unbindTexture();
			}
		}
	}
	let Ce = [], j = [];
	function M(t) {
		if (t.samples > 0) {
			if (N(t) === !1) {
				let i = t.textures, a = t.width, o = t.height, s = e.COLOR_BUFFER_BIT, l = t.stencilBuffer ? e.DEPTH_STENCIL_ATTACHMENT : e.DEPTH_ATTACHMENT, u = r.get(t), d = i.length > 1;
				if (d) for (let t = 0; t < i.length; t++) n.bindFramebuffer(e.FRAMEBUFFER, u.__webglMultisampledFramebuffer), e.framebufferRenderbuffer(e.FRAMEBUFFER, e.COLOR_ATTACHMENT0 + t, e.RENDERBUFFER, null), n.bindFramebuffer(e.FRAMEBUFFER, u.__webglFramebuffer), e.framebufferTexture2D(e.DRAW_FRAMEBUFFER, e.COLOR_ATTACHMENT0 + t, e.TEXTURE_2D, null, 0);
				n.bindFramebuffer(e.READ_FRAMEBUFFER, u.__webglMultisampledFramebuffer);
				let f = t.texture.mipmaps;
				f && f.length > 0 ? n.bindFramebuffer(e.DRAW_FRAMEBUFFER, u.__webglFramebuffer[0]) : n.bindFramebuffer(e.DRAW_FRAMEBUFFER, u.__webglFramebuffer);
				for (let n = 0; n < i.length; n++) {
					if (t.resolveDepthBuffer && (t.depthBuffer && (s |= e.DEPTH_BUFFER_BIT), t.stencilBuffer && t.resolveStencilBuffer && (s |= e.STENCIL_BUFFER_BIT)), d) {
						e.framebufferRenderbuffer(e.READ_FRAMEBUFFER, e.COLOR_ATTACHMENT0, e.RENDERBUFFER, u.__webglColorRenderbuffer[n]);
						let t = r.get(i[n]).__webglTexture;
						e.framebufferTexture2D(e.DRAW_FRAMEBUFFER, e.COLOR_ATTACHMENT0, e.TEXTURE_2D, t, 0);
					}
					e.blitFramebuffer(0, 0, a, o, 0, 0, a, o, s, e.NEAREST), c === !0 && (Ce.length = 0, j.length = 0, Ce.push(e.COLOR_ATTACHMENT0 + n), t.depthBuffer && t.resolveDepthBuffer === !1 && (Ce.push(l), j.push(l), e.invalidateFramebuffer(e.DRAW_FRAMEBUFFER, j)), e.invalidateFramebuffer(e.READ_FRAMEBUFFER, Ce));
				}
				if (n.bindFramebuffer(e.READ_FRAMEBUFFER, null), n.bindFramebuffer(e.DRAW_FRAMEBUFFER, null), d) for (let t = 0; t < i.length; t++) {
					n.bindFramebuffer(e.FRAMEBUFFER, u.__webglMultisampledFramebuffer), e.framebufferRenderbuffer(e.FRAMEBUFFER, e.COLOR_ATTACHMENT0 + t, e.RENDERBUFFER, u.__webglColorRenderbuffer[t]);
					let a = r.get(i[t]).__webglTexture;
					n.bindFramebuffer(e.FRAMEBUFFER, u.__webglFramebuffer), e.framebufferTexture2D(e.DRAW_FRAMEBUFFER, e.COLOR_ATTACHMENT0 + t, e.TEXTURE_2D, a, 0);
				}
				n.bindFramebuffer(e.DRAW_FRAMEBUFFER, u.__webglMultisampledFramebuffer);
			} else if (t.depthBuffer && t.resolveDepthBuffer === !1 && c) {
				let n = t.stencilBuffer ? e.DEPTH_STENCIL_ATTACHMENT : e.DEPTH_ATTACHMENT;
				e.invalidateFramebuffer(e.DRAW_FRAMEBUFFER, [n]);
			}
		}
	}
	function we(e) {
		return Math.min(i.maxSamples, e.samples);
	}
	function N(e) {
		let n = r.get(e);
		return e.samples > 0 && t.has("WEBGL_multisampled_render_to_texture") === !0 && n.__useRenderToTexture !== !1;
	}
	function P(e) {
		let t = o.render.frame;
		u.get(e) !== t && (u.set(e, t), e.update());
	}
	function Te(e, t) {
		let n = e.colorSpace, r = e.format, i = e.type;
		return e.isCompressedTexture === !0 || e.isVideoTexture === !0 || n !== "srgb-linear" && n !== "" && (Y.getTransfer(n) === "srgb" ? (r !== 1023 || i !== 1009) && W("WebGLTextures: sRGB encoded textures have to use RGBAFormat and UnsignedByteType.") : G("WebGLTextures: Unsupported texture color space:", n)), t;
	}
	function F(e) {
		return typeof HTMLImageElement < "u" && e instanceof HTMLImageElement ? (l.width = e.naturalWidth || e.width, l.height = e.naturalHeight || e.height) : typeof VideoFrame < "u" && e instanceof VideoFrame ? (l.width = e.displayWidth, l.height = e.displayHeight) : (l.width = e.width, l.height = e.height), l;
	}
	this.allocateTextureUnit = te, this.resetTextureUnits = k, this.getTextureUnits = A, this.setTextureUnits = ee, this.setTexture2D = re, this.setTexture2DArray = ie, this.setTexture3D = ae, this.setTextureCube = oe, this.rebindTextures = be, this.setupRenderTarget = xe, this.updateRenderTargetMipmap = Se, this.updateMultisampleRenderTarget = M, this.setupDepthRenderbuffer = ye, this.setupFrameBufferTexture = ge, this.useMultisampledRTT = N, this.isReversedDepthBuffer = function() {
		return n.buffers.depth.getReversed();
	};
}
function tf(e, t) {
	function n(n, r = "") {
		let i, a = Y.getTransfer(r);
		if (n === 1009) return e.UNSIGNED_BYTE;
		if (n === 1017) return e.UNSIGNED_SHORT_4_4_4_4;
		if (n === 1018) return e.UNSIGNED_SHORT_5_5_5_1;
		if (n === 35902) return e.UNSIGNED_INT_5_9_9_9_REV;
		if (n === 35899) return e.UNSIGNED_INT_10F_11F_11F_REV;
		if (n === 1010) return e.BYTE;
		if (n === 1011) return e.SHORT;
		if (n === 1012) return e.UNSIGNED_SHORT;
		if (n === 1013) return e.INT;
		if (n === 1014) return e.UNSIGNED_INT;
		if (n === 1015) return e.FLOAT;
		if (n === 1016) return e.HALF_FLOAT;
		if (n === 1021) return e.ALPHA;
		if (n === 1022) return e.RGB;
		if (n === 1023) return e.RGBA;
		if (n === 1026) return e.DEPTH_COMPONENT;
		if (n === 1027) return e.DEPTH_STENCIL;
		if (n === 1028) return e.RED;
		if (n === 1029) return e.RED_INTEGER;
		if (n === 1030) return e.RG;
		if (n === 1031) return e.RG_INTEGER;
		if (n === 1033) return e.RGBA_INTEGER;
		if (n === 33776 || n === 33777 || n === 33778 || n === 33779) if (a === "srgb") if (i = t.get("WEBGL_compressed_texture_s3tc_srgb"), i !== null) {
			if (n === 33776) return i.COMPRESSED_SRGB_S3TC_DXT1_EXT;
			if (n === 33777) return i.COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT;
			if (n === 33778) return i.COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT;
			if (n === 33779) return i.COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT;
		} else return null;
		else if (i = t.get("WEBGL_compressed_texture_s3tc"), i !== null) {
			if (n === 33776) return i.COMPRESSED_RGB_S3TC_DXT1_EXT;
			if (n === 33777) return i.COMPRESSED_RGBA_S3TC_DXT1_EXT;
			if (n === 33778) return i.COMPRESSED_RGBA_S3TC_DXT3_EXT;
			if (n === 33779) return i.COMPRESSED_RGBA_S3TC_DXT5_EXT;
		} else return null;
		if (n === 35840 || n === 35841 || n === 35842 || n === 35843) if (i = t.get("WEBGL_compressed_texture_pvrtc"), i !== null) {
			if (n === 35840) return i.COMPRESSED_RGB_PVRTC_4BPPV1_IMG;
			if (n === 35841) return i.COMPRESSED_RGB_PVRTC_2BPPV1_IMG;
			if (n === 35842) return i.COMPRESSED_RGBA_PVRTC_4BPPV1_IMG;
			if (n === 35843) return i.COMPRESSED_RGBA_PVRTC_2BPPV1_IMG;
		} else return null;
		if (n === 36196 || n === 37492 || n === 37496 || n === 37488 || n === 37489 || n === 37490 || n === 37491) if (i = t.get("WEBGL_compressed_texture_etc"), i !== null) {
			if (n === 36196 || n === 37492) return a === "srgb" ? i.COMPRESSED_SRGB8_ETC2 : i.COMPRESSED_RGB8_ETC2;
			if (n === 37496) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ETC2_EAC : i.COMPRESSED_RGBA8_ETC2_EAC;
			if (n === 37488) return i.COMPRESSED_R11_EAC;
			if (n === 37489) return i.COMPRESSED_SIGNED_R11_EAC;
			if (n === 37490) return i.COMPRESSED_RG11_EAC;
			if (n === 37491) return i.COMPRESSED_SIGNED_RG11_EAC;
		} else return null;
		if (n === 37808 || n === 37809 || n === 37810 || n === 37811 || n === 37812 || n === 37813 || n === 37814 || n === 37815 || n === 37816 || n === 37817 || n === 37818 || n === 37819 || n === 37820 || n === 37821) if (i = t.get("WEBGL_compressed_texture_astc"), i !== null) {
			if (n === 37808) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_4x4_KHR : i.COMPRESSED_RGBA_ASTC_4x4_KHR;
			if (n === 37809) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_5x4_KHR : i.COMPRESSED_RGBA_ASTC_5x4_KHR;
			if (n === 37810) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_5x5_KHR : i.COMPRESSED_RGBA_ASTC_5x5_KHR;
			if (n === 37811) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_6x5_KHR : i.COMPRESSED_RGBA_ASTC_6x5_KHR;
			if (n === 37812) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_6x6_KHR : i.COMPRESSED_RGBA_ASTC_6x6_KHR;
			if (n === 37813) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_8x5_KHR : i.COMPRESSED_RGBA_ASTC_8x5_KHR;
			if (n === 37814) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_8x6_KHR : i.COMPRESSED_RGBA_ASTC_8x6_KHR;
			if (n === 37815) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_8x8_KHR : i.COMPRESSED_RGBA_ASTC_8x8_KHR;
			if (n === 37816) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x5_KHR : i.COMPRESSED_RGBA_ASTC_10x5_KHR;
			if (n === 37817) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x6_KHR : i.COMPRESSED_RGBA_ASTC_10x6_KHR;
			if (n === 37818) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x8_KHR : i.COMPRESSED_RGBA_ASTC_10x8_KHR;
			if (n === 37819) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x10_KHR : i.COMPRESSED_RGBA_ASTC_10x10_KHR;
			if (n === 37820) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_12x10_KHR : i.COMPRESSED_RGBA_ASTC_12x10_KHR;
			if (n === 37821) return a === "srgb" ? i.COMPRESSED_SRGB8_ALPHA8_ASTC_12x12_KHR : i.COMPRESSED_RGBA_ASTC_12x12_KHR;
		} else return null;
		if (n === 36492 || n === 36494 || n === 36495) if (i = t.get("EXT_texture_compression_bptc"), i !== null) {
			if (n === 36492) return a === "srgb" ? i.COMPRESSED_SRGB_ALPHA_BPTC_UNORM_EXT : i.COMPRESSED_RGBA_BPTC_UNORM_EXT;
			if (n === 36494) return i.COMPRESSED_RGB_BPTC_SIGNED_FLOAT_EXT;
			if (n === 36495) return i.COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT_EXT;
		} else return null;
		if (n === 36283 || n === 36284 || n === 36285 || n === 36286) if (i = t.get("EXT_texture_compression_rgtc"), i !== null) {
			if (n === 36283) return i.COMPRESSED_RED_RGTC1_EXT;
			if (n === 36284) return i.COMPRESSED_SIGNED_RED_RGTC1_EXT;
			if (n === 36285) return i.COMPRESSED_RED_GREEN_RGTC2_EXT;
			if (n === 36286) return i.COMPRESSED_SIGNED_RED_GREEN_RGTC2_EXT;
		} else return null;
		return n === 1020 ? e.UNSIGNED_INT_24_8 : e[n] === void 0 ? null : e[n];
	}
	return { convert: n };
}
var nf = "\nvoid main() {\n\n	gl_Position = vec4( position, 1.0 );\n\n}", rf = "\nuniform sampler2DArray depthColor;\nuniform float depthWidth;\nuniform float depthHeight;\n\nvoid main() {\n\n	vec2 coord = vec2( gl_FragCoord.x / depthWidth, gl_FragCoord.y / depthHeight );\n\n	if ( coord.x >= 1.0 ) {\n\n		gl_FragDepth = texture( depthColor, vec3( coord.x - 1.0, coord.y, 1 ) ).r;\n\n	} else {\n\n		gl_FragDepth = texture( depthColor, vec3( coord.x, coord.y, 0 ) ).r;\n\n	}\n\n}", af = class {
	constructor() {
		this.texture = null, this.mesh = null, this.depthNear = 0, this.depthFar = 0;
	}
	init(e, t) {
		if (this.texture === null) {
			let n = new ys(e.texture);
			(e.depthNear !== t.depthNear || e.depthFar !== t.depthFar) && (this.depthNear = e.depthNear, this.depthFar = e.depthFar), this.texture = n;
		}
	}
	getMesh(e) {
		if (this.texture !== null && this.mesh === null) {
			let t = e.cameras[0].viewport, n = new js({
				vertexShader: nf,
				fragmentShader: rf,
				uniforms: {
					depthColor: { value: this.texture },
					depthWidth: { value: t.z },
					depthHeight: { value: t.w }
				}
			});
			this.mesh = new vo(new xs(20, 20), n);
		}
		return this.mesh;
	}
	reset() {
		this.texture = null, this.mesh = null;
	}
	getDepthTexture() {
		return this.texture;
	}
}, of = class extends Sr {
	constructor(e, t) {
		super();
		let n = this, r = null, i = 1, a = null, o = "local-floor", s = 1, c = null, l = null, u = null, d = null, f = null, p = null, m = typeof XRWebGLBinding < "u", h = new af(), g = {}, _ = t.getContextAttributes(), v = null, y = null, b = [], x = [], S = new Yr(), C = null, w = new _c();
		w.viewport = new pi();
		let T = new _c();
		T.viewport = new pi();
		let E = [w, T], D = new kc(), O = null, k = null;
		this.cameraAutoUpdate = !0, this.enabled = !1, this.isPresenting = !1, this.getController = function(e) {
			let t = b[e];
			return t === void 0 && (t = new Ji(), b[e] = t), t.getTargetRaySpace();
		}, this.getControllerGrip = function(e) {
			let t = b[e];
			return t === void 0 && (t = new Ji(), b[e] = t), t.getGripSpace();
		}, this.getHand = function(e) {
			let t = b[e];
			return t === void 0 && (t = new Ji(), b[e] = t), t.getHandSpace();
		};
		function A(e) {
			let t = x.indexOf(e.inputSource);
			if (t === -1) return;
			let n = b[t];
			n !== void 0 && (n.update(e.inputSource, e.frame, c || a), n.dispatchEvent({
				type: e.type,
				data: e.inputSource
			}));
		}
		function ee() {
			r.removeEventListener("select", A), r.removeEventListener("selectstart", A), r.removeEventListener("selectend", A), r.removeEventListener("squeeze", A), r.removeEventListener("squeezestart", A), r.removeEventListener("squeezeend", A), r.removeEventListener("end", ee), r.removeEventListener("inputsourceschange", te);
			for (let e = 0; e < b.length; e++) {
				let t = x[e];
				t !== null && (x[e] = null, b[e].disconnect(t));
			}
			O = null, k = null, h.reset();
			for (let e in g) delete g[e];
			e.setRenderTarget(v), f = null, d = null, u = null, r = null, y = null, le.stop(), n.isPresenting = !1, e.setPixelRatio(C), e.setSize(S.width, S.height, !1), n.dispatchEvent({ type: "sessionend" });
		}
		this.setFramebufferScaleFactor = function(e) {
			i = e, n.isPresenting === !0 && W("WebXRManager: Cannot change framebuffer scale while presenting.");
		}, this.setReferenceSpaceType = function(e) {
			o = e, n.isPresenting === !0 && W("WebXRManager: Cannot change reference space type while presenting.");
		}, this.getReferenceSpace = function() {
			return c || a;
		}, this.setReferenceSpace = function(e) {
			c = e;
		}, this.getBaseLayer = function() {
			return d === null ? f : d;
		}, this.getBinding = function() {
			return u === null && m && (u = new XRWebGLBinding(r, t)), u;
		}, this.getFrame = function() {
			return p;
		}, this.getSession = function() {
			return r;
		}, this.setSession = async function(l) {
			if (r = l, r !== null) {
				if (v = e.getRenderTarget(), r.addEventListener("select", A), r.addEventListener("selectstart", A), r.addEventListener("selectend", A), r.addEventListener("squeeze", A), r.addEventListener("squeezestart", A), r.addEventListener("squeezeend", A), r.addEventListener("end", ee), r.addEventListener("inputsourceschange", te), _.xrCompatible !== !0 && await t.makeXRCompatible(), C = e.getPixelRatio(), e.getSize(S), m && "createProjectionLayer" in XRWebGLBinding.prototype) {
					let n = null, a = null, o = null;
					_.depth && (o = _.stencil ? t.DEPTH24_STENCIL8 : t.DEPTH_COMPONENT24, n = _.stencil ? nn : tn, a = _.stencil ? Yt : Wt);
					let s = {
						colorFormat: t.RGBA8,
						depthFormat: o,
						scaleFactor: i
					};
					u = this.getBinding(), d = u.createProjectionLayer(s), r.updateRenderState({ layers: [d] }), e.setPixelRatio(1), e.setSize(d.textureWidth, d.textureHeight, !1), y = new hi(d.textureWidth, d.textureHeight, {
						format: en,
						type: zt,
						depthTexture: new _s(d.textureWidth, d.textureHeight, a, void 0, void 0, void 0, void 0, void 0, void 0, n),
						stencilBuffer: _.stencil,
						colorSpace: e.outputColorSpace,
						samples: _.antialias ? 4 : 0,
						resolveDepthBuffer: d.ignoreDepthValues === !1,
						resolveStencilBuffer: d.ignoreDepthValues === !1
					});
				} else {
					let n = {
						antialias: _.antialias,
						alpha: !0,
						depth: _.depth,
						stencil: _.stencil,
						framebufferScaleFactor: i
					};
					f = new XRWebGLLayer(r, t, n), r.updateRenderState({ baseLayer: f }), e.setPixelRatio(1), e.setSize(f.framebufferWidth, f.framebufferHeight, !1), y = new hi(f.framebufferWidth, f.framebufferHeight, {
						format: en,
						type: zt,
						colorSpace: e.outputColorSpace,
						stencilBuffer: _.stencil,
						resolveDepthBuffer: f.ignoreDepthValues === !1,
						resolveStencilBuffer: f.ignoreDepthValues === !1
					});
				}
				y.isXRRenderTarget = !0, this.setFoveation(s), c = null, a = await r.requestReferenceSpace(o), le.setContext(r), le.start(), n.isPresenting = !0, n.dispatchEvent({ type: "sessionstart" });
			}
		}, this.getEnvironmentBlendMode = function() {
			if (r !== null) return r.environmentBlendMode;
		}, this.getDepthTexture = function() {
			return h.getDepthTexture();
		};
		function te(e) {
			for (let t = 0; t < e.removed.length; t++) {
				let n = e.removed[t], r = x.indexOf(n);
				r >= 0 && (x[r] = null, b[r].disconnect(n));
			}
			for (let t = 0; t < e.added.length; t++) {
				let n = e.added[t], r = x.indexOf(n);
				if (r === -1) {
					for (let e = 0; e < b.length; e++) if (e >= x.length) {
						x.push(n), r = e;
						break;
					} else if (x[e] === null) {
						x[e] = n, r = e;
						break;
					}
					if (r === -1) break;
				}
				let i = b[r];
				i && i.connect(n);
			}
		}
		let ne = new q(), re = new q();
		function ie(e, t, n) {
			ne.setFromMatrixPosition(t.matrixWorld), re.setFromMatrixPosition(n.matrixWorld);
			let r = ne.distanceTo(re), i = t.projectionMatrix.elements, a = n.projectionMatrix.elements, o = i[14] / (i[10] - 1), s = i[14] / (i[10] + 1), c = (i[9] + 1) / i[5], l = (i[9] - 1) / i[5], u = (i[8] - 1) / i[0], d = (a[8] + 1) / a[0], f = o * u, p = o * d, m = r / (-u + d), h = m * -u;
			if (t.matrixWorld.decompose(e.position, e.quaternion, e.scale), e.translateX(h), e.translateZ(m), e.matrixWorld.compose(e.position, e.quaternion, e.scale), e.matrixWorldInverse.copy(e.matrixWorld).invert(), i[10] === -1) e.projectionMatrix.copy(t.projectionMatrix), e.projectionMatrixInverse.copy(t.projectionMatrixInverse);
			else {
				let t = o + m, n = s + m, i = f - h, a = p + (r - h), u = c * s / n * t, d = l * s / n * t;
				e.projectionMatrix.makePerspective(i, a, u, d, t, n), e.projectionMatrixInverse.copy(e.projectionMatrix).invert();
			}
		}
		function ae(e, t) {
			t === null ? e.matrixWorld.copy(e.matrix) : e.matrixWorld.multiplyMatrices(t.matrixWorld, e.matrix), e.matrixWorldInverse.copy(e.matrixWorld).invert();
		}
		this.updateCamera = function(e) {
			if (r === null) return;
			let t = e.near, n = e.far;
			h.texture !== null && (h.depthNear > 0 && (t = h.depthNear), h.depthFar > 0 && (n = h.depthFar)), D.near = T.near = w.near = t, D.far = T.far = w.far = n, (O !== D.near || k !== D.far) && (r.updateRenderState({
				depthNear: D.near,
				depthFar: D.far
			}), O = D.near, k = D.far), D.layers.mask = e.layers.mask | 6, w.layers.mask = D.layers.mask & -5, T.layers.mask = D.layers.mask & -3;
			let i = e.parent, a = D.cameras;
			ae(D, i);
			for (let e = 0; e < a.length; e++) ae(a[e], i);
			a.length === 2 ? ie(D, w, T) : D.projectionMatrix.copy(w.projectionMatrix), oe(e, D, i);
		};
		function oe(e, t, n) {
			n === null ? e.matrix.copy(t.matrixWorld) : (e.matrix.copy(n.matrixWorld), e.matrix.invert(), e.matrix.multiply(t.matrixWorld)), e.matrix.decompose(e.position, e.quaternion, e.scale), e.updateMatrixWorld(!0), e.projectionMatrix.copy(t.projectionMatrix), e.projectionMatrixInverse.copy(t.projectionMatrixInverse), e.isPerspectiveCamera && (e.fov = Er * 2 * Math.atan(1 / e.projectionMatrix.elements[5]), e.zoom = 1);
		}
		this.getCamera = function() {
			return D;
		}, this.getFoveation = function() {
			if (!(d === null && f === null)) return s;
		}, this.setFoveation = function(e) {
			s = e, d !== null && (d.fixedFoveation = e), f !== null && f.fixedFoveation !== void 0 && (f.fixedFoveation = e);
		}, this.hasDepthSensing = function() {
			return h.texture !== null;
		}, this.getDepthSensingMesh = function() {
			return h.getMesh(D);
		}, this.getCameraTexture = function(e) {
			return g[e];
		};
		let se = null;
		function ce(t, i) {
			if (l = i.getViewerPose(c || a), p = i, l !== null) {
				let t = l.views;
				f !== null && (e.setRenderTargetFramebuffer(y, f.framebuffer), e.setRenderTarget(y));
				let i = !1;
				t.length !== D.cameras.length && (D.cameras.length = 0, i = !0);
				for (let n = 0; n < t.length; n++) {
					let r = t[n], a = null;
					if (f !== null) a = f.getViewport(r);
					else {
						let t = u.getViewSubImage(d, r);
						a = t.viewport, n === 0 && (e.setRenderTargetTextures(y, t.colorTexture, t.depthStencilTexture), e.setRenderTarget(y));
					}
					let o = E[n];
					o === void 0 && (o = new _c(), o.layers.enable(n), o.viewport = new pi(), E[n] = o), o.matrix.fromArray(r.transform.matrix), o.matrix.decompose(o.position, o.quaternion, o.scale), o.projectionMatrix.fromArray(r.projectionMatrix), o.projectionMatrixInverse.copy(o.projectionMatrix).invert(), o.viewport.set(a.x, a.y, a.width, a.height), n === 0 && (D.matrix.copy(o.matrix), D.matrix.decompose(D.position, D.quaternion, D.scale)), i === !0 && D.cameras.push(o);
				}
				let a = r.enabledFeatures;
				if (a && a.includes("depth-sensing") && r.depthUsage == "gpu-optimized" && m) {
					u = n.getBinding();
					let e = u.getDepthInformation(t[0]);
					e && e.isValid && e.texture && h.init(e, r.renderState);
				}
				if (a && a.includes("camera-access") && m) {
					e.state.unbindTexture(), u = n.getBinding();
					for (let e = 0; e < t.length; e++) {
						let n = t[e].camera;
						if (n) {
							let e = g[n];
							e || (e = new ys(), g[n] = e);
							let t = u.getCameraImage(n);
							e.sourceTexture = t;
						}
					}
				}
			}
			for (let e = 0; e < b.length; e++) {
				let t = x[e], n = b[e];
				t !== null && n !== void 0 && n.update(t, i, c || a);
			}
			se && se(t, i), i.detectedPlanes && n.dispatchEvent({
				type: "planesdetected",
				data: i
			}), p = null;
		}
		let le = new Qc();
		le.setAnimationLoop(ce), this.setAnimationLoop = function(e) {
			se = e;
		}, this.dispose = function() {};
	}
}, sf = /*@__PURE__*/ new vi(), cf = /*@__PURE__*/ new J();
cf.set(-1, 0, 0, 0, 1, 0, 0, 0, 1);
function lf(e, t) {
	function n(e, t) {
		e.matrixAutoUpdate === !0 && e.updateMatrix(), t.value.copy(e.matrix);
	}
	function r(t, n) {
		n.color.getRGB(t.fogColor.value, Ds(e)), n.isFog ? (t.fogNear.value = n.near, t.fogFar.value = n.far) : n.isFogExp2 && (t.fogDensity.value = n.density);
	}
	function i(e, t, n, r, i) {
		t.isNodeMaterial ? t.uniformsNeedUpdate = !1 : t.isMeshBasicMaterial ? a(e, t) : t.isMeshLambertMaterial ? (a(e, t), t.envMap && (e.envMapIntensity.value = t.envMapIntensity)) : t.isMeshToonMaterial ? (a(e, t), d(e, t)) : t.isMeshPhongMaterial ? (a(e, t), u(e, t), t.envMap && (e.envMapIntensity.value = t.envMapIntensity)) : t.isMeshStandardMaterial ? (a(e, t), f(e, t), t.isMeshPhysicalMaterial && p(e, t, i)) : t.isMeshMatcapMaterial ? (a(e, t), m(e, t)) : t.isMeshDepthMaterial ? a(e, t) : t.isMeshDistanceMaterial ? (a(e, t), h(e, t)) : t.isMeshNormalMaterial ? a(e, t) : t.isLineBasicMaterial ? (o(e, t), t.isLineDashedMaterial && s(e, t)) : t.isPointsMaterial ? c(e, t, n, r) : t.isSpriteMaterial ? l(e, t) : t.isShadowMaterial ? (e.color.value.copy(t.color), e.opacity.value = t.opacity) : t.isShaderMaterial && (t.uniformsNeedUpdate = !1);
	}
	function a(e, r) {
		e.opacity.value = r.opacity, r.color && e.diffuse.value.copy(r.color), r.emissive && e.emissive.value.copy(r.emissive).multiplyScalar(r.emissiveIntensity), r.map && (e.map.value = r.map, n(r.map, e.mapTransform)), r.alphaMap && (e.alphaMap.value = r.alphaMap, n(r.alphaMap, e.alphaMapTransform)), r.bumpMap && (e.bumpMap.value = r.bumpMap, n(r.bumpMap, e.bumpMapTransform), e.bumpScale.value = r.bumpScale, r.side === 1 && (e.bumpScale.value *= -1)), r.normalMap && (e.normalMap.value = r.normalMap, n(r.normalMap, e.normalMapTransform), e.normalScale.value.copy(r.normalScale), r.side === 1 && e.normalScale.value.negate()), r.displacementMap && (e.displacementMap.value = r.displacementMap, n(r.displacementMap, e.displacementMapTransform), e.displacementScale.value = r.displacementScale, e.displacementBias.value = r.displacementBias), r.emissiveMap && (e.emissiveMap.value = r.emissiveMap, n(r.emissiveMap, e.emissiveMapTransform)), r.specularMap && (e.specularMap.value = r.specularMap, n(r.specularMap, e.specularMapTransform)), r.alphaTest > 0 && (e.alphaTest.value = r.alphaTest);
		let i = t.get(r), a = i.envMap, o = i.envMapRotation;
		a && (e.envMap.value = a, e.envMapRotation.value.setFromMatrix4(sf.makeRotationFromEuler(o)).transpose(), a.isCubeTexture && a.isRenderTargetTexture === !1 && e.envMapRotation.value.premultiply(cf), e.reflectivity.value = r.reflectivity, e.ior.value = r.ior, e.refractionRatio.value = r.refractionRatio), r.lightMap && (e.lightMap.value = r.lightMap, e.lightMapIntensity.value = r.lightMapIntensity, n(r.lightMap, e.lightMapTransform)), r.aoMap && (e.aoMap.value = r.aoMap, e.aoMapIntensity.value = r.aoMapIntensity, n(r.aoMap, e.aoMapTransform));
	}
	function o(e, t) {
		e.diffuse.value.copy(t.color), e.opacity.value = t.opacity, t.map && (e.map.value = t.map, n(t.map, e.mapTransform));
	}
	function s(e, t) {
		e.dashSize.value = t.dashSize, e.totalSize.value = t.dashSize + t.gapSize, e.scale.value = t.scale;
	}
	function c(e, t, r, i) {
		e.diffuse.value.copy(t.color), e.opacity.value = t.opacity, e.size.value = t.size * r, e.scale.value = i * .5, t.map && (e.map.value = t.map, n(t.map, e.uvTransform)), t.alphaMap && (e.alphaMap.value = t.alphaMap, n(t.alphaMap, e.alphaMapTransform)), t.alphaTest > 0 && (e.alphaTest.value = t.alphaTest);
	}
	function l(e, t) {
		e.diffuse.value.copy(t.color), e.opacity.value = t.opacity, e.rotation.value = t.rotation, t.map && (e.map.value = t.map, n(t.map, e.mapTransform)), t.alphaMap && (e.alphaMap.value = t.alphaMap, n(t.alphaMap, e.alphaMapTransform)), t.alphaTest > 0 && (e.alphaTest.value = t.alphaTest);
	}
	function u(e, t) {
		e.specular.value.copy(t.specular), e.shininess.value = Math.max(t.shininess, 1e-4);
	}
	function d(e, t) {
		t.gradientMap && (e.gradientMap.value = t.gradientMap);
	}
	function f(e, t) {
		e.metalness.value = t.metalness, t.metalnessMap && (e.metalnessMap.value = t.metalnessMap, n(t.metalnessMap, e.metalnessMapTransform)), e.roughness.value = t.roughness, t.roughnessMap && (e.roughnessMap.value = t.roughnessMap, n(t.roughnessMap, e.roughnessMapTransform)), t.envMap && (e.envMapIntensity.value = t.envMapIntensity);
	}
	function p(e, t, r) {
		e.ior.value = t.ior, t.sheen > 0 && (e.sheenColor.value.copy(t.sheenColor).multiplyScalar(t.sheen), e.sheenRoughness.value = t.sheenRoughness, t.sheenColorMap && (e.sheenColorMap.value = t.sheenColorMap, n(t.sheenColorMap, e.sheenColorMapTransform)), t.sheenRoughnessMap && (e.sheenRoughnessMap.value = t.sheenRoughnessMap, n(t.sheenRoughnessMap, e.sheenRoughnessMapTransform))), t.clearcoat > 0 && (e.clearcoat.value = t.clearcoat, e.clearcoatRoughness.value = t.clearcoatRoughness, t.clearcoatMap && (e.clearcoatMap.value = t.clearcoatMap, n(t.clearcoatMap, e.clearcoatMapTransform)), t.clearcoatRoughnessMap && (e.clearcoatRoughnessMap.value = t.clearcoatRoughnessMap, n(t.clearcoatRoughnessMap, e.clearcoatRoughnessMapTransform)), t.clearcoatNormalMap && (e.clearcoatNormalMap.value = t.clearcoatNormalMap, n(t.clearcoatNormalMap, e.clearcoatNormalMapTransform), e.clearcoatNormalScale.value.copy(t.clearcoatNormalScale), t.side === 1 && e.clearcoatNormalScale.value.negate())), t.dispersion > 0 && (e.dispersion.value = t.dispersion), t.iridescence > 0 && (e.iridescence.value = t.iridescence, e.iridescenceIOR.value = t.iridescenceIOR, e.iridescenceThicknessMinimum.value = t.iridescenceThicknessRange[0], e.iridescenceThicknessMaximum.value = t.iridescenceThicknessRange[1], t.iridescenceMap && (e.iridescenceMap.value = t.iridescenceMap, n(t.iridescenceMap, e.iridescenceMapTransform)), t.iridescenceThicknessMap && (e.iridescenceThicknessMap.value = t.iridescenceThicknessMap, n(t.iridescenceThicknessMap, e.iridescenceThicknessMapTransform))), t.transmission > 0 && (e.transmission.value = t.transmission, e.transmissionSamplerMap.value = r.texture, e.transmissionSamplerSize.value.set(r.width, r.height), t.transmissionMap && (e.transmissionMap.value = t.transmissionMap, n(t.transmissionMap, e.transmissionMapTransform)), e.thickness.value = t.thickness, t.thicknessMap && (e.thicknessMap.value = t.thicknessMap, n(t.thicknessMap, e.thicknessMapTransform)), e.attenuationDistance.value = t.attenuationDistance, e.attenuationColor.value.copy(t.attenuationColor)), t.anisotropy > 0 && (e.anisotropyVector.value.set(t.anisotropy * Math.cos(t.anisotropyRotation), t.anisotropy * Math.sin(t.anisotropyRotation)), t.anisotropyMap && (e.anisotropyMap.value = t.anisotropyMap, n(t.anisotropyMap, e.anisotropyMapTransform))), e.specularIntensity.value = t.specularIntensity, e.specularColor.value.copy(t.specularColor), t.specularColorMap && (e.specularColorMap.value = t.specularColorMap, n(t.specularColorMap, e.specularColorMapTransform)), t.specularIntensityMap && (e.specularIntensityMap.value = t.specularIntensityMap, n(t.specularIntensityMap, e.specularIntensityMapTransform));
	}
	function m(e, t) {
		t.matcap && (e.matcap.value = t.matcap);
	}
	function h(e, n) {
		let r = t.get(n).light;
		e.referencePosition.value.setFromMatrixPosition(r.matrixWorld), e.nearDistance.value = r.shadow.camera.near, e.farDistance.value = r.shadow.camera.far;
	}
	return {
		refreshFogUniforms: r,
		refreshMaterialUniforms: i
	};
}
function uf(e, t, n, r) {
	let i = {}, a = {}, o = [], s = e.getParameter(e.MAX_UNIFORM_BUFFER_BINDINGS);
	function c(e, t) {
		let n = t.program;
		r.uniformBlockBinding(e, n);
	}
	function l(e, n) {
		let o = i[e.id];
		o === void 0 && (m(e), o = u(e), i[e.id] = o, e.addEventListener("dispose", g));
		let s = n.program;
		r.updateUBOMapping(e, s);
		let c = t.render.frame;
		a[e.id] !== c && (f(e), a[e.id] = c);
	}
	function u(t) {
		let n = d();
		t.__bindingPointIndex = n;
		let r = e.createBuffer(), i = t.__size, a = t.usage;
		return e.bindBuffer(e.UNIFORM_BUFFER, r), e.bufferData(e.UNIFORM_BUFFER, i, a), e.bindBuffer(e.UNIFORM_BUFFER, null), e.bindBufferBase(e.UNIFORM_BUFFER, n, r), r;
	}
	function d() {
		for (let e = 0; e < s; e++) if (o.indexOf(e) === -1) return o.push(e), e;
		return G("WebGLRenderer: Maximum number of simultaneously usable uniforms groups reached."), 0;
	}
	function f(t) {
		let n = i[t.id], r = t.uniforms, a = t.__cache;
		e.bindBuffer(e.UNIFORM_BUFFER, n);
		for (let t = 0, n = r.length; t < n; t++) {
			let n = Array.isArray(r[t]) ? r[t] : [r[t]];
			for (let r = 0, i = n.length; r < i; r++) {
				let i = n[r];
				if (p(i, t, r, a) === !0) {
					let t = i.__offset, n = Array.isArray(i.value) ? i.value : [i.value], r = 0;
					for (let a = 0; a < n.length; a++) {
						let o = n[a], s = h(o);
						typeof o == "number" || typeof o == "boolean" ? (i.__data[0] = o, e.bufferSubData(e.UNIFORM_BUFFER, t + r, i.__data)) : o.isMatrix3 ? (i.__data[0] = o.elements[0], i.__data[1] = o.elements[1], i.__data[2] = o.elements[2], i.__data[3] = 0, i.__data[4] = o.elements[3], i.__data[5] = o.elements[4], i.__data[6] = o.elements[5], i.__data[7] = 0, i.__data[8] = o.elements[6], i.__data[9] = o.elements[7], i.__data[10] = o.elements[8], i.__data[11] = 0) : ArrayBuffer.isView(o) ? i.__data.set(new o.constructor(o.buffer, o.byteOffset, i.__data.length)) : (o.toArray(i.__data, r), r += s.storage / Float32Array.BYTES_PER_ELEMENT);
					}
					e.bufferSubData(e.UNIFORM_BUFFER, t, i.__data);
				}
			}
		}
		e.bindBuffer(e.UNIFORM_BUFFER, null);
	}
	function p(e, t, n, r) {
		let i = e.value, a = t + "_" + n;
		if (r[a] === void 0) return typeof i == "number" || typeof i == "boolean" ? r[a] = i : ArrayBuffer.isView(i) ? r[a] = i.slice() : r[a] = i.clone(), !0;
		{
			let e = r[a];
			if (typeof i == "number" || typeof i == "boolean") {
				if (e !== i) return r[a] = i, !0;
			} else if (ArrayBuffer.isView(i)) return !0;
			else if (e.equals(i) === !1) return e.copy(i), !0;
		}
		return !1;
	}
	function m(e) {
		let t = e.uniforms, n = 0;
		for (let e = 0, r = t.length; e < r; e++) {
			let r = Array.isArray(t[e]) ? t[e] : [t[e]];
			for (let e = 0, t = r.length; e < t; e++) {
				let t = r[e], i = Array.isArray(t.value) ? t.value : [t.value];
				for (let e = 0, r = i.length; e < r; e++) {
					let r = i[e], a = h(r), o = n % 16, s = o % a.boundary, c = o + s;
					n += s, c !== 0 && 16 - c < a.storage && (n += 16 - c), t.__data = new Float32Array(a.storage / Float32Array.BYTES_PER_ELEMENT), t.__offset = n, n += a.storage;
				}
			}
		}
		let r = n % 16;
		return r > 0 && (n += 16 - r), e.__size = n, e.__cache = {}, this;
	}
	function h(e) {
		let t = {
			boundary: 0,
			storage: 0
		};
		return typeof e == "number" || typeof e == "boolean" ? (t.boundary = 4, t.storage = 4) : e.isVector2 ? (t.boundary = 8, t.storage = 8) : e.isVector3 || e.isColor ? (t.boundary = 16, t.storage = 12) : e.isVector4 ? (t.boundary = 16, t.storage = 16) : e.isMatrix3 ? (t.boundary = 48, t.storage = 48) : e.isMatrix4 ? (t.boundary = 64, t.storage = 64) : e.isTexture ? W("WebGLRenderer: Texture samplers can not be part of an uniforms group.") : ArrayBuffer.isView(e) ? (t.boundary = 16, t.storage = e.byteLength) : W("WebGLRenderer: Unsupported uniform value type.", e), t;
	}
	function g(t) {
		let n = t.target;
		n.removeEventListener("dispose", g);
		let r = o.indexOf(n.__bindingPointIndex);
		o.splice(r, 1), e.deleteBuffer(i[n.id]), delete i[n.id], delete a[n.id];
	}
	function _() {
		for (let t in i) e.deleteBuffer(i[t]);
		o = [], i = {}, a = {};
	}
	return {
		bind: c,
		update: l,
		dispose: _
	};
}
var df = new Uint16Array([
	12469,
	15057,
	12620,
	14925,
	13266,
	14620,
	13807,
	14376,
	14323,
	13990,
	14545,
	13625,
	14713,
	13328,
	14840,
	12882,
	14931,
	12528,
	14996,
	12233,
	15039,
	11829,
	15066,
	11525,
	15080,
	11295,
	15085,
	10976,
	15082,
	10705,
	15073,
	10495,
	13880,
	14564,
	13898,
	14542,
	13977,
	14430,
	14158,
	14124,
	14393,
	13732,
	14556,
	13410,
	14702,
	12996,
	14814,
	12596,
	14891,
	12291,
	14937,
	11834,
	14957,
	11489,
	14958,
	11194,
	14943,
	10803,
	14921,
	10506,
	14893,
	10278,
	14858,
	9960,
	14484,
	14039,
	14487,
	14025,
	14499,
	13941,
	14524,
	13740,
	14574,
	13468,
	14654,
	13106,
	14743,
	12678,
	14818,
	12344,
	14867,
	11893,
	14889,
	11509,
	14893,
	11180,
	14881,
	10751,
	14852,
	10428,
	14812,
	10128,
	14765,
	9754,
	14712,
	9466,
	14764,
	13480,
	14764,
	13475,
	14766,
	13440,
	14766,
	13347,
	14769,
	13070,
	14786,
	12713,
	14816,
	12387,
	14844,
	11957,
	14860,
	11549,
	14868,
	11215,
	14855,
	10751,
	14825,
	10403,
	14782,
	10044,
	14729,
	9651,
	14666,
	9352,
	14599,
	9029,
	14967,
	12835,
	14966,
	12831,
	14963,
	12804,
	14954,
	12723,
	14936,
	12564,
	14917,
	12347,
	14900,
	11958,
	14886,
	11569,
	14878,
	11247,
	14859,
	10765,
	14828,
	10401,
	14784,
	10011,
	14727,
	9600,
	14660,
	9289,
	14586,
	8893,
	14508,
	8533,
	15111,
	12234,
	15110,
	12234,
	15104,
	12216,
	15092,
	12156,
	15067,
	12010,
	15028,
	11776,
	14981,
	11500,
	14942,
	11205,
	14902,
	10752,
	14861,
	10393,
	14812,
	9991,
	14752,
	9570,
	14682,
	9252,
	14603,
	8808,
	14519,
	8445,
	14431,
	8145,
	15209,
	11449,
	15208,
	11451,
	15202,
	11451,
	15190,
	11438,
	15163,
	11384,
	15117,
	11274,
	15055,
	10979,
	14994,
	10648,
	14932,
	10343,
	14871,
	9936,
	14803,
	9532,
	14729,
	9218,
	14645,
	8742,
	14556,
	8381,
	14461,
	8020,
	14365,
	7603,
	15273,
	10603,
	15272,
	10607,
	15267,
	10619,
	15256,
	10631,
	15231,
	10614,
	15182,
	10535,
	15118,
	10389,
	15042,
	10167,
	14963,
	9787,
	14883,
	9447,
	14800,
	9115,
	14710,
	8665,
	14615,
	8318,
	14514,
	7911,
	14411,
	7507,
	14279,
	7198,
	15314,
	9675,
	15313,
	9683,
	15309,
	9712,
	15298,
	9759,
	15277,
	9797,
	15229,
	9773,
	15166,
	9668,
	15084,
	9487,
	14995,
	9274,
	14898,
	8910,
	14800,
	8539,
	14697,
	8234,
	14590,
	7790,
	14479,
	7409,
	14367,
	7067,
	14178,
	6621,
	15337,
	8619,
	15337,
	8631,
	15333,
	8677,
	15325,
	8769,
	15305,
	8871,
	15264,
	8940,
	15202,
	8909,
	15119,
	8775,
	15022,
	8565,
	14916,
	8328,
	14804,
	8009,
	14688,
	7614,
	14569,
	7287,
	14448,
	6888,
	14321,
	6483,
	14088,
	6171,
	15350,
	7402,
	15350,
	7419,
	15347,
	7480,
	15340,
	7613,
	15322,
	7804,
	15287,
	7973,
	15229,
	8057,
	15148,
	8012,
	15046,
	7846,
	14933,
	7611,
	14810,
	7357,
	14682,
	7069,
	14552,
	6656,
	14421,
	6316,
	14251,
	5948,
	14007,
	5528,
	15356,
	5942,
	15356,
	5977,
	15353,
	6119,
	15348,
	6294,
	15332,
	6551,
	15302,
	6824,
	15249,
	7044,
	15171,
	7122,
	15070,
	7050,
	14949,
	6861,
	14818,
	6611,
	14679,
	6349,
	14538,
	6067,
	14398,
	5651,
	14189,
	5311,
	13935,
	4958,
	15359,
	4123,
	15359,
	4153,
	15356,
	4296,
	15353,
	4646,
	15338,
	5160,
	15311,
	5508,
	15263,
	5829,
	15188,
	6042,
	15088,
	6094,
	14966,
	6001,
	14826,
	5796,
	14678,
	5543,
	14527,
	5287,
	14377,
	4985,
	14133,
	4586,
	13869,
	4257,
	15360,
	1563,
	15360,
	1642,
	15358,
	2076,
	15354,
	2636,
	15341,
	3350,
	15317,
	4019,
	15273,
	4429,
	15203,
	4732,
	15105,
	4911,
	14981,
	4932,
	14836,
	4818,
	14679,
	4621,
	14517,
	4386,
	14359,
	4156,
	14083,
	3795,
	13808,
	3437,
	15360,
	122,
	15360,
	137,
	15358,
	285,
	15355,
	636,
	15344,
	1274,
	15322,
	2177,
	15281,
	2765,
	15215,
	3223,
	15120,
	3451,
	14995,
	3569,
	14846,
	3567,
	14681,
	3466,
	14511,
	3305,
	14344,
	3121,
	14037,
	2800,
	13753,
	2467,
	15360,
	0,
	15360,
	1,
	15359,
	21,
	15355,
	89,
	15346,
	253,
	15325,
	479,
	15287,
	796,
	15225,
	1148,
	15133,
	1492,
	15008,
	1749,
	14856,
	1882,
	14685,
	1886,
	14506,
	1783,
	14324,
	1608,
	13996,
	1398,
	13702,
	1183
]), ff = null;
function pf() {
	return ff === null && (ff = new Mo(df, 16, 16, on, Kt), ff.name = "DFG_LUT", ff.minFilter = It, ff.magFilter = It, ff.wrapS = jt, ff.wrapT = jt, ff.generateMipmaps = !1, ff.needsUpdate = !0), ff;
}
var mf = class {
	constructor(e = {}) {
		let { canvas: t = hr(), context: n = null, depth: r = !0, stencil: i = !1, alpha: a = !1, antialias: o = !1, premultipliedAlpha: s = !0, preserveDrawingBuffer: c = !1, powerPreference: l = "default", failIfMajorPerformanceCaveat: u = !1, reversedDepthBuffer: d = !1, outputBufferType: f = zt } = e;
		this.isWebGLRenderer = !0;
		let p;
		if (n !== null) {
			if (typeof WebGLRenderingContext < "u" && n instanceof WebGLRenderingContext) throw Error("THREE.WebGLRenderer: WebGL 1 is not supported since r163.");
			p = n.getContextAttributes().alpha;
		} else p = a;
		let m = f, h = /* @__PURE__ */ new Set([
			cn,
			sn,
			an
		]), g = /* @__PURE__ */ new Set([
			zt,
			Wt,
			Ht,
			Yt,
			qt,
			Jt
		]), _ = /* @__PURE__ */ new Uint32Array(4), v = /* @__PURE__ */ new Int32Array(4), y = new q(), b = null, x = null, S = [], C = [], w = null;
		this.domElement = t, this.debug = {
			checkShaderErrors: !0,
			onShaderError: null
		}, this.autoClear = !0, this.autoClearColor = !0, this.autoClearDepth = !0, this.autoClearStencil = !0, this.sortObjects = !0, this.clippingPlanes = [], this.localClippingEnabled = !1, this.toneMapping = 0, this.toneMappingExposure = 1, this.transmissionResolutionScale = 1;
		let T = this, E = !1, D = null;
		this._outputColorSpace = ir;
		let O = 0, k = 0, A = null, ee = -1, te = null, ne = new pi(), re = new pi(), ie = null, ae = new X(0), oe = 0, se = t.width, ce = t.height, le = 1, ue = null, de = null, fe = new pi(0, 0, se, ce), pe = new pi(0, 0, se, ce), me = !1, he = new Yo(), ge = !1, _e = !1, ve = new vi(), ye = new q(), be = new pi(), xe = {
			background: null,
			fog: null,
			environment: null,
			overrideMaterial: null,
			isScene: !0
		}, Se = !1;
		function Ce() {
			return A === null ? le : 1;
		}
		let j = n;
		function M(e, n) {
			return t.getContext(e, n);
		}
		try {
			let e = {
				alpha: !0,
				depth: r,
				stencil: i,
				antialias: o,
				premultipliedAlpha: s,
				preserveDrawingBuffer: c,
				powerPreference: l,
				failIfMajorPerformanceCaveat: u
			};
			if ("setAttribute" in t && t.setAttribute("data-engine", "three.js r184"), t.addEventListener("webglcontextlost", Be, !1), t.addEventListener("webglcontextrestored", Ve, !1), t.addEventListener("webglcontextcreationerror", He, !1), j === null) {
				let t = "webgl2";
				if (j = M(t, e), j === null) throw M(t) ? Error("Error creating WebGL context with your selected attributes.") : Error("Error creating WebGL context.");
			}
		} catch (e) {
			throw G("WebGLRenderer: " + e.message), e;
		}
		let we, N, P, Te, F, I, Ee, De, Oe, L, ke, Ae, je, Me, Ne, R, Pe, Fe, Ie, Le, Re, z, ze;
		function B() {
			we = new jl(j), we.init(), Re = new tf(j, we), N = new sl(j, we, e, Re), P = new $d(j, we), N.reversedDepthBuffer && d && P.buffers.depth.setReversed(!0), Te = new Pl(j), F = new Nd(), I = new ef(j, we, P, F, N, Re, Te), Ee = new Al(T), De = new $c(j), z = new al(j, De), Oe = new Ml(j, De, Te, z), L = new Il(j, Oe, De, z, Te), Fe = new Fl(j, N, I), Ne = new cl(F), ke = new Md(T, Ee, we, N, z, Ne), Ae = new lf(T, F), je = new Ld(), Me = new Wd(we), Pe = new il(T, Ee, P, L, p, s), R = new Qd(T, L, N), ze = new uf(j, Te, N, P), Ie = new ol(j, we, Te), Le = new Nl(j, we, Te), Te.programs = ke.programs, T.capabilities = N, T.extensions = we, T.properties = F, T.renderLists = je, T.shadowMap = R, T.state = P, T.info = Te;
		}
		B(), m !== 1009 && (w = new Rl(m, t.width, t.height, r, i));
		let V = new of(T, j);
		this.xr = V, this.getContext = function() {
			return j;
		}, this.getContextAttributes = function() {
			return j.getContextAttributes();
		}, this.forceContextLoss = function() {
			let e = we.get("WEBGL_lose_context");
			e && e.loseContext();
		}, this.forceContextRestore = function() {
			let e = we.get("WEBGL_lose_context");
			e && e.restoreContext();
		}, this.getPixelRatio = function() {
			return le;
		}, this.setPixelRatio = function(e) {
			e !== void 0 && (le = e, this.setSize(se, ce, !1));
		}, this.getSize = function(e) {
			return e.set(se, ce);
		}, this.setSize = function(e, n, r = !0) {
			if (V.isPresenting) {
				W("WebGLRenderer: Can't change size while VR device is presenting.");
				return;
			}
			se = e, ce = n, t.width = Math.floor(e * le), t.height = Math.floor(n * le), r === !0 && (t.style.width = e + "px", t.style.height = n + "px"), w !== null && w.setSize(t.width, t.height), this.setViewport(0, 0, e, n);
		}, this.getDrawingBufferSize = function(e) {
			return e.set(se * le, ce * le).floor();
		}, this.setDrawingBufferSize = function(e, n, r) {
			se = e, ce = n, le = r, t.width = Math.floor(e * r), t.height = Math.floor(n * r), this.setViewport(0, 0, e, n);
		}, this.setEffects = function(e) {
			if (m === 1009) {
				G("THREE.WebGLRenderer: setEffects() requires outputBufferType set to HalfFloatType or FloatType.");
				return;
			}
			if (e) {
				for (let t = 0; t < e.length; t++) if (e[t].isOutputPass === !0) {
					W("THREE.WebGLRenderer: OutputPass is not needed in setEffects(). Tone mapping and color space conversion are applied automatically.");
					break;
				}
			}
			w.setEffects(e || []);
		}, this.getCurrentViewport = function(e) {
			return e.copy(ne);
		}, this.getViewport = function(e) {
			return e.copy(fe);
		}, this.setViewport = function(e, t, n, r) {
			e.isVector4 ? fe.set(e.x, e.y, e.z, e.w) : fe.set(e, t, n, r), P.viewport(ne.copy(fe).multiplyScalar(le).round());
		}, this.getScissor = function(e) {
			return e.copy(pe);
		}, this.setScissor = function(e, t, n, r) {
			e.isVector4 ? pe.set(e.x, e.y, e.z, e.w) : pe.set(e, t, n, r), P.scissor(re.copy(pe).multiplyScalar(le).round());
		}, this.getScissorTest = function() {
			return me;
		}, this.setScissorTest = function(e) {
			P.setScissorTest(me = e);
		}, this.setOpaqueSort = function(e) {
			ue = e;
		}, this.setTransparentSort = function(e) {
			de = e;
		}, this.getClearColor = function(e) {
			return e.copy(Pe.getClearColor());
		}, this.setClearColor = function() {
			Pe.setClearColor(...arguments);
		}, this.getClearAlpha = function() {
			return Pe.getClearAlpha();
		}, this.setClearAlpha = function() {
			Pe.setClearAlpha(...arguments);
		}, this.clear = function(e = !0, t = !0, n = !0) {
			let r = 0;
			if (e) {
				let e = !1;
				if (A !== null) {
					let t = A.texture.format;
					e = h.has(t);
				}
				if (e) {
					let e = A.texture.type, t = g.has(e), n = Pe.getClearColor(), r = Pe.getClearAlpha(), i = n.r, a = n.g, o = n.b;
					t ? (_[0] = i, _[1] = a, _[2] = o, _[3] = r, j.clearBufferuiv(j.COLOR, 0, _)) : (v[0] = i, v[1] = a, v[2] = o, v[3] = r, j.clearBufferiv(j.COLOR, 0, v));
				} else r |= j.COLOR_BUFFER_BIT;
			}
			t && (r |= j.DEPTH_BUFFER_BIT, this.state.buffers.depth.setMask(!0)), n && (r |= j.STENCIL_BUFFER_BIT, this.state.buffers.stencil.setMask(4294967295)), r !== 0 && j.clear(r);
		}, this.clearColor = function() {
			this.clear(!0, !1, !1);
		}, this.clearDepth = function() {
			this.clear(!1, !0, !1);
		}, this.clearStencil = function() {
			this.clear(!1, !1, !0);
		}, this.setNodesHandler = function(e) {
			e.setRenderer(this), D = e;
		}, this.dispose = function() {
			t.removeEventListener("webglcontextlost", Be, !1), t.removeEventListener("webglcontextrestored", Ve, !1), t.removeEventListener("webglcontextcreationerror", He, !1), Pe.dispose(), je.dispose(), Me.dispose(), F.dispose(), Ee.dispose(), L.dispose(), z.dispose(), ze.dispose(), ke.dispose(), V.dispose(), V.removeEventListener("sessionstart", Ye), V.removeEventListener("sessionend", Xe), Ze.stop();
		};
		function Be(e) {
			e.preventDefault(), _r("WebGLRenderer: Context Lost."), E = !0;
		}
		function Ve() {
			_r("WebGLRenderer: Context Restored."), E = !1;
			let e = Te.autoReset, t = R.enabled, n = R.autoUpdate, r = R.needsUpdate, i = R.type;
			B(), Te.autoReset = e, R.enabled = t, R.autoUpdate = n, R.needsUpdate = r, R.type = i;
		}
		function He(e) {
			G("WebGLRenderer: A WebGL context could not be created. Reason: ", e.statusMessage);
		}
		function Ue(e) {
			let t = e.target;
			t.removeEventListener("dispose", Ue), We(t);
		}
		function We(e) {
			Ge(e), F.remove(e);
		}
		function Ge(e) {
			let t = F.get(e).programs;
			t !== void 0 && (t.forEach(function(e) {
				ke.releaseProgram(e);
			}), e.isShaderMaterial && ke.releaseShaderCache(e));
		}
		this.renderBufferDirect = function(e, t, n, r, i, a) {
			t === null && (t = xe);
			let o = i.isMesh && i.matrixWorld.determinant() < 0, s = ot(e, t, n, r, i);
			P.setMaterial(r, o);
			let c = n.index, l = 1;
			if (r.wireframe === !0) {
				if (c = Oe.getWireframeAttribute(n), c === void 0) return;
				l = 2;
			}
			let u = n.drawRange, d = n.attributes.position, f = u.start * l, p = (u.start + u.count) * l;
			a !== null && (f = Math.max(f, a.start * l), p = Math.min(p, (a.start + a.count) * l)), c === null ? d != null && (f = Math.max(f, 0), p = Math.min(p, d.count)) : (f = Math.max(f, 0), p = Math.min(p, c.count));
			let m = p - f;
			if (m < 0 || m === Infinity) return;
			z.setup(i, r, s, n, c);
			let h, g = Ie;
			if (c !== null && (h = De.get(c), g = Le, g.setIndex(h)), i.isMesh) r.wireframe === !0 ? (P.setLineWidth(r.wireframeLinewidth * Ce()), g.setMode(j.LINES)) : g.setMode(j.TRIANGLES);
			else if (i.isLine) {
				let e = r.linewidth;
				e === void 0 && (e = 1), P.setLineWidth(e * Ce()), i.isLineSegments ? g.setMode(j.LINES) : i.isLineLoop ? g.setMode(j.LINE_LOOP) : g.setMode(j.LINE_STRIP);
			} else i.isPoints ? g.setMode(j.POINTS) : i.isSprite && g.setMode(j.TRIANGLES);
			if (i.isBatchedMesh) if (we.get("WEBGL_multi_draw")) g.renderMultiDraw(i._multiDrawStarts, i._multiDrawCounts, i._multiDrawCount);
			else {
				let e = i._multiDrawStarts, t = i._multiDrawCounts, n = i._multiDrawCount, a = c ? De.get(c).bytesPerElement : 1, o = F.get(r).currentProgram.getUniforms();
				for (let r = 0; r < n; r++) o.setValue(j, "_gl_DrawID", r), g.render(e[r] / a, t[r]);
			}
			else if (i.isInstancedMesh) g.renderInstances(f, m, i.count);
			else if (n.isInstancedBufferGeometry) {
				let e = n._maxInstanceCount === void 0 ? Infinity : n._maxInstanceCount, t = Math.min(n.instanceCount, e);
				g.renderInstances(f, m, t);
			} else g.render(f, m);
		};
		function Ke(e, t, n) {
			e.transparent === !0 && e.side === 2 && e.forceSinglePass === !1 ? (e.side = 1, e.needsUpdate = !0, nt(e, t, n), e.side = 0, e.needsUpdate = !0, nt(e, t, n), e.side = 2) : nt(e, t, n);
		}
		this.compile = function(e, t, n = null) {
			n === null && (n = e), x = Me.get(n), x.init(t), C.push(x), n.traverseVisible(function(e) {
				e.isLight && e.layers.test(t.layers) && (x.pushLight(e), e.castShadow && x.pushShadow(e));
			}), e !== n && e.traverseVisible(function(e) {
				e.isLight && e.layers.test(t.layers) && (x.pushLight(e), e.castShadow && x.pushShadow(e));
			}), x.setupLights();
			let r = /* @__PURE__ */ new Set();
			return e.traverse(function(e) {
				if (!(e.isMesh || e.isPoints || e.isLine || e.isSprite)) return;
				let t = e.material;
				if (t) if (Array.isArray(t)) for (let i = 0; i < t.length; i++) {
					let a = t[i];
					Ke(a, n, e), r.add(a);
				}
				else Ke(t, n, e), r.add(t);
			}), x = C.pop(), r;
		}, this.compileAsync = function(e, t, n = null) {
			let r = this.compile(e, t, n);
			return new Promise((t) => {
				function n() {
					if (r.forEach(function(e) {
						F.get(e).currentProgram.isReady() && r.delete(e);
					}), r.size === 0) {
						t(e);
						return;
					}
					setTimeout(n, 10);
				}
				we.get("KHR_parallel_shader_compile") === null ? setTimeout(n, 10) : n();
			});
		};
		let qe = null;
		function Je(e) {
			qe && qe(e);
		}
		function Ye() {
			Ze.stop();
		}
		function Xe() {
			Ze.start();
		}
		let Ze = new Qc();
		Ze.setAnimationLoop(Je), typeof self < "u" && Ze.setContext(self), this.setAnimationLoop = function(e) {
			qe = e, V.setAnimationLoop(e), e === null ? Ze.stop() : Ze.start();
		}, V.addEventListener("sessionstart", Ye), V.addEventListener("sessionend", Xe), this.render = function(e, t) {
			if (t !== void 0 && t.isCamera !== !0) {
				G("WebGLRenderer.render: camera is not an instance of THREE.Camera.");
				return;
			}
			if (E === !0) return;
			D !== null && D.renderStart(e, t);
			let n = V.enabled === !0 && V.isPresenting === !0, r = w !== null && (A === null || n) && w.begin(T, A);
			if (e.matrixWorldAutoUpdate === !0 && e.updateMatrixWorld(), t.parent === null && t.matrixWorldAutoUpdate === !0 && t.updateMatrixWorld(), V.enabled === !0 && V.isPresenting === !0 && (w === null || w.isCompositing() === !1) && (V.cameraAutoUpdate === !0 && V.updateCamera(t), t = V.getCamera()), e.isScene === !0 && e.onBeforeRender(T, e, t, A), x = Me.get(e, C.length), x.init(t), x.state.textureUnits = I.getTextureUnits(), C.push(x), ve.multiplyMatrices(t.projectionMatrix, t.matrixWorldInverse), he.setFromProjectionMatrix(ve, dr, t.reversedDepth), _e = this.localClippingEnabled, ge = Ne.init(this.clippingPlanes, _e), b = je.get(e, S.length), b.init(), S.push(b), V.enabled === !0 && V.isPresenting === !0) {
				let e = T.xr.getDepthSensingMesh();
				e !== null && Qe(e, t, -Infinity, T.sortObjects);
			}
			Qe(e, t, 0, T.sortObjects), b.finish(), T.sortObjects === !0 && b.sort(ue, de), Se = V.enabled === !1 || V.isPresenting === !1 || V.hasDepthSensing() === !1, Se && Pe.addToRenderList(b, e), this.info.render.frame++, ge === !0 && Ne.beginShadows();
			let i = x.state.shadowsArray;
			if (R.render(i, e, t), ge === !0 && Ne.endShadows(), this.info.autoReset === !0 && this.info.reset(), (r && w.hasRenderPass()) === !1) {
				let n = b.opaque, r = b.transmissive;
				if (x.setupLights(), t.isArrayCamera) {
					let i = t.cameras;
					if (r.length > 0) for (let t = 0, a = i.length; t < a; t++) {
						let a = i[t];
						et(n, r, e, a);
					}
					Se && Pe.render(e);
					for (let t = 0, n = i.length; t < n; t++) {
						let n = i[t];
						$e(b, e, n, n.viewport);
					}
				} else r.length > 0 && et(n, r, e, t), Se && Pe.render(e), $e(b, e, t);
			}
			A !== null && k === 0 && (I.updateMultisampleRenderTarget(A), I.updateRenderTargetMipmap(A)), r && w.end(T), e.isScene === !0 && e.onAfterRender(T, e, t), z.resetDefaultState(), ee = -1, te = null, C.pop(), C.length > 0 ? (x = C[C.length - 1], I.setTextureUnits(x.state.textureUnits), ge === !0 && Ne.setGlobalState(T.clippingPlanes, x.state.camera)) : x = null, S.pop(), b = S.length > 0 ? S[S.length - 1] : null, D !== null && D.renderEnd();
		};
		function Qe(e, t, n, r) {
			if (e.visible === !1) return;
			if (e.layers.test(t.layers)) {
				if (e.isGroup) n = e.renderOrder;
				else if (e.isLOD) e.autoUpdate === !0 && e.update(t);
				else if (e.isLightProbeGrid) x.pushLightProbeGrid(e);
				else if (e.isLight) x.pushLight(e), e.castShadow && x.pushShadow(e);
				else if (e.isSprite) {
					if (!e.frustumCulled || he.intersectsSprite(e)) {
						r && be.setFromMatrixPosition(e.matrixWorld).applyMatrix4(ve);
						let t = L.update(e), i = e.material;
						i.visible && b.push(e, t, i, n, be.z, null);
					}
				} else if ((e.isMesh || e.isLine || e.isPoints) && (!e.frustumCulled || he.intersectsObject(e))) {
					let t = L.update(e), i = e.material;
					if (r && (e.boundingSphere === void 0 ? (t.boundingSphere === null && t.computeBoundingSphere(), be.copy(t.boundingSphere.center)) : (e.boundingSphere === null && e.computeBoundingSphere(), be.copy(e.boundingSphere.center)), be.applyMatrix4(e.matrixWorld).applyMatrix4(ve)), Array.isArray(i)) {
						let r = t.groups;
						for (let a = 0, o = r.length; a < o; a++) {
							let o = r[a], s = i[o.materialIndex];
							s && s.visible && b.push(e, t, s, n, be.z, o);
						}
					} else i.visible && b.push(e, t, i, n, be.z, null);
				}
			}
			let i = e.children;
			for (let e = 0, a = i.length; e < a; e++) Qe(i[e], t, n, r);
		}
		function $e(e, t, n, r) {
			let { opaque: i, transmissive: a, transparent: o } = e;
			x.setupLightsView(n), ge === !0 && Ne.setGlobalState(T.clippingPlanes, n), r && P.viewport(ne.copy(r)), i.length > 0 && H(i, t, n), a.length > 0 && H(a, t, n), o.length > 0 && H(o, t, n), P.buffers.depth.setTest(!0), P.buffers.depth.setMask(!0), P.buffers.color.setMask(!0), P.setPolygonOffset(!1);
		}
		function et(e, t, n, r) {
			if ((n.isScene === !0 ? n.overrideMaterial : null) !== null) return;
			if (x.state.transmissionRenderTarget[r.id] === void 0) {
				let e = we.has("EXT_color_buffer_half_float") || we.has("EXT_color_buffer_float");
				x.state.transmissionRenderTarget[r.id] = new hi(1, 1, {
					generateMipmaps: !0,
					type: e ? Kt : zt,
					minFilter: Rt,
					samples: Math.max(4, N.samples),
					stencilBuffer: i,
					resolveDepthBuffer: !1,
					resolveStencilBuffer: !1,
					colorSpace: Y.workingColorSpace
				});
			}
			let a = x.state.transmissionRenderTarget[r.id], o = r.viewport || ne;
			a.setSize(o.z * T.transmissionResolutionScale, o.w * T.transmissionResolutionScale);
			let s = T.getRenderTarget(), c = T.getActiveCubeFace(), l = T.getActiveMipmapLevel();
			T.setRenderTarget(a), T.getClearColor(ae), oe = T.getClearAlpha(), oe < 1 && T.setClearColor(16777215, .5), T.clear(), Se && Pe.render(n);
			let u = T.toneMapping;
			T.toneMapping = 0;
			let d = r.viewport;
			if (r.viewport !== void 0 && (r.viewport = void 0), x.setupLightsView(r), ge === !0 && Ne.setGlobalState(T.clippingPlanes, r), H(e, n, r), I.updateMultisampleRenderTarget(a), I.updateRenderTargetMipmap(a), we.has("WEBGL_multisampled_render_to_texture") === !1) {
				let e = !1;
				for (let i = 0, a = t.length; i < a; i++) {
					let { object: a, geometry: o, material: s, group: c } = t[i];
					if (s.side === 2 && a.layers.test(r.layers)) {
						let t = s.side;
						s.side = 1, s.needsUpdate = !0, tt(a, n, r, o, s, c), s.side = t, s.needsUpdate = !0, e = !0;
					}
				}
				e === !0 && (I.updateMultisampleRenderTarget(a), I.updateRenderTargetMipmap(a));
			}
			T.setRenderTarget(s, c, l), T.setClearColor(ae, oe), d !== void 0 && (r.viewport = d), T.toneMapping = u;
		}
		function H(e, t, n) {
			let r = t.isScene === !0 ? t.overrideMaterial : null;
			for (let i = 0, a = e.length; i < a; i++) {
				let a = e[i], { object: o, geometry: s, group: c } = a, l = a.material;
				l.allowOverride === !0 && r !== null && (l = r), o.layers.test(n.layers) && tt(o, t, n, s, l, c);
			}
		}
		function tt(e, t, n, r, i, a) {
			e.onBeforeRender(T, t, n, r, i, a), e.modelViewMatrix.multiplyMatrices(n.matrixWorldInverse, e.matrixWorld), e.normalMatrix.getNormalMatrix(e.modelViewMatrix), i.onBeforeRender(T, t, n, r, e, a), i.transparent === !0 && i.side === 2 && i.forceSinglePass === !1 ? (i.side = 1, i.needsUpdate = !0, T.renderBufferDirect(n, t, r, i, e, a), i.side = 0, i.needsUpdate = !0, T.renderBufferDirect(n, t, r, i, e, a), i.side = 2) : T.renderBufferDirect(n, t, r, i, e, a), e.onAfterRender(T, t, n, r, i, a);
		}
		function nt(e, t, n) {
			t.isScene !== !0 && (t = xe);
			let r = F.get(e), i = x.state.lights, a = x.state.shadowsArray, o = i.state.version, s = ke.getParameters(e, i.state, a, t, n, x.state.lightProbeGridArray), c = ke.getProgramCacheKey(s), l = r.programs;
			r.environment = e.isMeshStandardMaterial || e.isMeshLambertMaterial || e.isMeshPhongMaterial ? t.environment : null, r.fog = t.fog;
			let u = e.isMeshStandardMaterial || e.isMeshLambertMaterial && !e.envMap || e.isMeshPhongMaterial && !e.envMap;
			r.envMap = Ee.get(e.envMap || r.environment, u), r.envMapRotation = r.environment !== null && e.envMap === null ? t.environmentRotation : e.envMapRotation, l === void 0 && (e.addEventListener("dispose", Ue), l = /* @__PURE__ */ new Map(), r.programs = l);
			let d = l.get(c);
			if (d !== void 0) {
				if (r.currentProgram === d && r.lightsStateVersion === o) return it(e, s), d;
			} else s.uniforms = ke.getUniforms(e), D !== null && e.isNodeMaterial && D.build(e, n, s), e.onBeforeCompile(s, T), d = ke.acquireProgram(s, c), l.set(c, d), r.uniforms = s.uniforms;
			let f = r.uniforms;
			return (!e.isShaderMaterial && !e.isRawShaderMaterial || e.clipping === !0) && (f.clippingPlanes = Ne.uniform), it(e, s), r.needsLights = ct(e), r.lightsStateVersion = o, r.needsLights && (f.ambientLightColor.value = i.state.ambient, f.lightProbe.value = i.state.probe, f.directionalLights.value = i.state.directional, f.directionalLightShadows.value = i.state.directionalShadow, f.spotLights.value = i.state.spot, f.spotLightShadows.value = i.state.spotShadow, f.rectAreaLights.value = i.state.rectArea, f.ltc_1.value = i.state.rectAreaLTC1, f.ltc_2.value = i.state.rectAreaLTC2, f.pointLights.value = i.state.point, f.pointLightShadows.value = i.state.pointShadow, f.hemisphereLights.value = i.state.hemi, f.directionalShadowMatrix.value = i.state.directionalShadowMatrix, f.spotLightMatrix.value = i.state.spotLightMatrix, f.spotLightMap.value = i.state.spotLightMap, f.pointShadowMatrix.value = i.state.pointShadowMatrix), r.lightProbeGrid = x.state.lightProbeGridArray.length > 0, r.currentProgram = d, r.uniformsList = null, d;
		}
		function rt(e) {
			if (e.uniformsList === null) {
				let t = e.currentProgram.getUniforms();
				e.uniformsList = Gu.seqWithValue(t.seq, e.uniforms);
			}
			return e.uniformsList;
		}
		function it(e, t) {
			let n = F.get(e);
			n.outputColorSpace = t.outputColorSpace, n.batching = t.batching, n.batchingColor = t.batchingColor, n.instancing = t.instancing, n.instancingColor = t.instancingColor, n.instancingMorph = t.instancingMorph, n.skinning = t.skinning, n.morphTargets = t.morphTargets, n.morphNormals = t.morphNormals, n.morphColors = t.morphColors, n.morphTargetsCount = t.morphTargetsCount, n.numClippingPlanes = t.numClippingPlanes, n.numIntersection = t.numClipIntersection, n.vertexAlphas = t.vertexAlphas, n.vertexTangents = t.vertexTangents, n.toneMapping = t.toneMapping;
		}
		function at(e, t) {
			if (e.length === 0) return null;
			if (e.length === 1) return e[0].texture === null ? null : e[0];
			y.setFromMatrixPosition(t.matrixWorld);
			for (let t = 0, n = e.length; t < n; t++) {
				let n = e[t];
				if (n.texture !== null && n.boundingBox.containsPoint(y)) return n;
			}
			return null;
		}
		function ot(e, t, n, r, i) {
			t.isScene !== !0 && (t = xe), I.resetTextureUnits();
			let a = t.fog, o = r.isMeshStandardMaterial || r.isMeshLambertMaterial || r.isMeshPhongMaterial ? t.environment : null, s = A === null ? T.outputColorSpace : A.isXRRenderTarget === !0 ? A.texture.colorSpace : Y.workingColorSpace, c = r.isMeshStandardMaterial || r.isMeshLambertMaterial && !r.envMap || r.isMeshPhongMaterial && !r.envMap, l = Ee.get(r.envMap || o, c), u = r.vertexColors === !0 && !!n.attributes.color && n.attributes.color.itemSize === 4, d = !!n.attributes.tangent && (!!r.normalMap || r.anisotropy > 0), f = !!n.morphAttributes.position, p = !!n.morphAttributes.normal, m = !!n.morphAttributes.color, h = 0;
			r.toneMapped && (A === null || A.isXRRenderTarget === !0) && (h = T.toneMapping);
			let g = n.morphAttributes.position || n.morphAttributes.normal || n.morphAttributes.color, _ = g === void 0 ? 0 : g.length, v = F.get(r), y = x.state.lights;
			if (ge === !0 && (_e === !0 || e !== te)) {
				let t = e === te && r.id === ee;
				Ne.setState(r, e, t);
			}
			let b = !1;
			r.version === v.__version ? v.needsLights && v.lightsStateVersion !== y.state.version ? b = !0 : v.outputColorSpace === s ? i.isBatchedMesh && v.batching === !1 || !i.isBatchedMesh && v.batching === !0 || i.isBatchedMesh && v.batchingColor === !0 && i.colorTexture === null || i.isBatchedMesh && v.batchingColor === !1 && i.colorTexture !== null || i.isInstancedMesh && v.instancing === !1 || !i.isInstancedMesh && v.instancing === !0 || i.isSkinnedMesh && v.skinning === !1 || !i.isSkinnedMesh && v.skinning === !0 || i.isInstancedMesh && v.instancingColor === !0 && i.instanceColor === null || i.isInstancedMesh && v.instancingColor === !1 && i.instanceColor !== null || i.isInstancedMesh && v.instancingMorph === !0 && i.morphTexture === null || i.isInstancedMesh && v.instancingMorph === !1 && i.morphTexture !== null ? b = !0 : v.envMap === l ? r.fog === !0 && v.fog !== a || v.numClippingPlanes !== void 0 && (v.numClippingPlanes !== Ne.numPlanes || v.numIntersection !== Ne.numIntersection) ? b = !0 : v.vertexAlphas === u && v.vertexTangents === d && v.morphTargets === f && v.morphNormals === p && v.morphColors === m && v.toneMapping === h && v.morphTargetsCount === _ ? !!v.lightProbeGrid != x.state.lightProbeGridArray.length > 0 && (b = !0) : b = !0 : b = !0 : b = !0 : (b = !0, v.__version = r.version);
			let S = v.currentProgram;
			b === !0 && (S = nt(r, t, i), D && r.isNodeMaterial && D.onUpdateProgram(r, S, v));
			let C = !1, w = !1, E = !1, O = S.getUniforms(), k = v.uniforms;
			if (P.useProgram(S.program) && (C = !0, w = !0, E = !0), r.id !== ee && (ee = r.id, w = !0), v.needsLights) {
				let e = at(x.state.lightProbeGridArray, i);
				v.lightProbeGrid !== e && (v.lightProbeGrid = e, w = !0);
			}
			if (C || te !== e) {
				P.buffers.depth.getReversed() && e.reversedDepth !== !0 && (e._reversedDepth = !0, e.updateProjectionMatrix()), O.setValue(j, "projectionMatrix", e.projectionMatrix), O.setValue(j, "viewMatrix", e.matrixWorldInverse);
				let t = O.map.cameraPosition;
				t !== void 0 && t.setValue(j, ye.setFromMatrixPosition(e.matrixWorld)), N.logarithmicDepthBuffer && O.setValue(j, "logDepthBufFC", 2 / (Math.log(e.far + 1) / Math.LN2)), (r.isMeshPhongMaterial || r.isMeshToonMaterial || r.isMeshLambertMaterial || r.isMeshBasicMaterial || r.isMeshStandardMaterial || r.isShaderMaterial) && O.setValue(j, "isOrthographic", e.isOrthographicCamera === !0), te !== e && (te = e, w = !0, E = !0);
			}
			if (v.needsLights && (y.state.directionalShadowMap.length > 0 && O.setValue(j, "directionalShadowMap", y.state.directionalShadowMap, I), y.state.spotShadowMap.length > 0 && O.setValue(j, "spotShadowMap", y.state.spotShadowMap, I), y.state.pointShadowMap.length > 0 && O.setValue(j, "pointShadowMap", y.state.pointShadowMap, I)), i.isSkinnedMesh) {
				O.setOptional(j, i, "bindMatrix"), O.setOptional(j, i, "bindMatrixInverse");
				let e = i.skeleton;
				e && (e.boneTexture === null && e.computeBoneTexture(), O.setValue(j, "boneTexture", e.boneTexture, I));
			}
			i.isBatchedMesh && (O.setOptional(j, i, "batchingTexture"), O.setValue(j, "batchingTexture", i._matricesTexture, I), O.setOptional(j, i, "batchingIdTexture"), O.setValue(j, "batchingIdTexture", i._indirectTexture, I), O.setOptional(j, i, "batchingColorTexture"), i._colorsTexture !== null && O.setValue(j, "batchingColorTexture", i._colorsTexture, I));
			let ne = n.morphAttributes;
			if ((ne.position !== void 0 || ne.normal !== void 0 || ne.color !== void 0) && Fe.update(i, n, S), (w || v.receiveShadow !== i.receiveShadow) && (v.receiveShadow = i.receiveShadow, O.setValue(j, "receiveShadow", i.receiveShadow)), (r.isMeshStandardMaterial || r.isMeshLambertMaterial || r.isMeshPhongMaterial) && r.envMap === null && t.environment !== null && (k.envMapIntensity.value = t.environmentIntensity), k.dfgLUT !== void 0 && (k.dfgLUT.value = pf()), w) {
				if (O.setValue(j, "toneMappingExposure", T.toneMappingExposure), v.needsLights && st(k, E), a && r.fog === !0 && Ae.refreshFogUniforms(k, a), Ae.refreshMaterialUniforms(k, r, le, ce, x.state.transmissionRenderTarget[e.id]), v.needsLights && v.lightProbeGrid) {
					let e = v.lightProbeGrid;
					k.probesSH.value = e.texture, k.probesMin.value.copy(e.boundingBox.min), k.probesMax.value.copy(e.boundingBox.max), k.probesResolution.value.copy(e.resolution);
				}
				Gu.upload(j, rt(v), k, I);
			}
			if (r.isShaderMaterial && r.uniformsNeedUpdate === !0 && (Gu.upload(j, rt(v), k, I), r.uniformsNeedUpdate = !1), r.isSpriteMaterial && O.setValue(j, "center", i.center), O.setValue(j, "modelViewMatrix", i.modelViewMatrix), O.setValue(j, "normalMatrix", i.normalMatrix), O.setValue(j, "modelMatrix", i.matrixWorld), r.uniformsGroups !== void 0) {
				let e = r.uniformsGroups;
				for (let t = 0, n = e.length; t < n; t++) {
					let n = e[t];
					ze.update(n, S), ze.bind(n, S);
				}
			}
			return S;
		}
		function st(e, t) {
			e.ambientLightColor.needsUpdate = t, e.lightProbe.needsUpdate = t, e.directionalLights.needsUpdate = t, e.directionalLightShadows.needsUpdate = t, e.pointLights.needsUpdate = t, e.pointLightShadows.needsUpdate = t, e.spotLights.needsUpdate = t, e.spotLightShadows.needsUpdate = t, e.rectAreaLights.needsUpdate = t, e.hemisphereLights.needsUpdate = t;
		}
		function ct(e) {
			return e.isMeshLambertMaterial || e.isMeshToonMaterial || e.isMeshPhongMaterial || e.isMeshStandardMaterial || e.isShadowMaterial || e.isShaderMaterial && e.lights === !0;
		}
		this.getActiveCubeFace = function() {
			return O;
		}, this.getActiveMipmapLevel = function() {
			return k;
		}, this.getRenderTarget = function() {
			return A;
		}, this.setRenderTargetTextures = function(e, t, n) {
			let r = F.get(e);
			r.__autoAllocateDepthBuffer = e.resolveDepthBuffer === !1, r.__autoAllocateDepthBuffer === !1 && (r.__useRenderToTexture = !1), F.get(e.texture).__webglTexture = t, F.get(e.depthTexture).__webglTexture = r.__autoAllocateDepthBuffer ? void 0 : n, r.__hasExternalTextures = !0;
		}, this.setRenderTargetFramebuffer = function(e, t) {
			let n = F.get(e);
			n.__webglFramebuffer = t, n.__useDefaultFramebuffer = t === void 0;
		};
		let lt = j.createFramebuffer();
		this.setRenderTarget = function(e, t = 0, n = 0) {
			A = e, O = t, k = n;
			let r = null, i = !1, a = !1;
			if (e) {
				let o = F.get(e);
				if (o.__useDefaultFramebuffer !== void 0) {
					P.bindFramebuffer(j.FRAMEBUFFER, o.__webglFramebuffer), ne.copy(e.viewport), re.copy(e.scissor), ie = e.scissorTest, P.viewport(ne), P.scissor(re), P.setScissorTest(ie), ee = -1;
					return;
				} else if (o.__webglFramebuffer === void 0) I.setupRenderTarget(e);
				else if (o.__hasExternalTextures) I.rebindTextures(e, F.get(e.texture).__webglTexture, F.get(e.depthTexture).__webglTexture);
				else if (e.depthBuffer) {
					let t = e.depthTexture;
					if (o.__boundDepthTexture !== t) {
						if (t !== null && F.has(t) && (e.width !== t.image.width || e.height !== t.image.height)) throw Error("WebGLRenderTarget: Attached DepthTexture is initialized to the incorrect size.");
						I.setupDepthRenderbuffer(e);
					}
				}
				let s = e.texture;
				(s.isData3DTexture || s.isDataArrayTexture || s.isCompressedArrayTexture) && (a = !0);
				let c = F.get(e).__webglFramebuffer;
				e.isWebGLCubeRenderTarget ? (r = Array.isArray(c[t]) ? c[t][n] : c[t], i = !0) : r = e.samples > 0 && I.useMultisampledRTT(e) === !1 ? F.get(e).__webglMultisampledFramebuffer : Array.isArray(c) ? c[n] : c, ne.copy(e.viewport), re.copy(e.scissor), ie = e.scissorTest;
			} else ne.copy(fe).multiplyScalar(le).floor(), re.copy(pe).multiplyScalar(le).floor(), ie = me;
			if (n !== 0 && (r = lt), P.bindFramebuffer(j.FRAMEBUFFER, r) && P.drawBuffers(e, r), P.viewport(ne), P.scissor(re), P.setScissorTest(ie), i) {
				let r = F.get(e.texture);
				j.framebufferTexture2D(j.FRAMEBUFFER, j.COLOR_ATTACHMENT0, j.TEXTURE_CUBE_MAP_POSITIVE_X + t, r.__webglTexture, n);
			} else if (a) {
				let r = t;
				for (let t = 0; t < e.textures.length; t++) {
					let i = F.get(e.textures[t]);
					j.framebufferTextureLayer(j.FRAMEBUFFER, j.COLOR_ATTACHMENT0 + t, i.__webglTexture, n, r);
				}
			} else if (e !== null && n !== 0) {
				let t = F.get(e.texture);
				j.framebufferTexture2D(j.FRAMEBUFFER, j.COLOR_ATTACHMENT0, j.TEXTURE_2D, t.__webglTexture, n);
			}
			ee = -1;
		}, this.readRenderTargetPixels = function(e, t, n, r, i, a, o, s = 0) {
			if (!(e && e.isWebGLRenderTarget)) {
				G("WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");
				return;
			}
			let c = F.get(e).__webglFramebuffer;
			if (e.isWebGLCubeRenderTarget && o !== void 0 && (c = c[o]), c) {
				P.bindFramebuffer(j.FRAMEBUFFER, c);
				try {
					let o = e.textures[s], c = o.format, l = o.type;
					if (e.textures.length > 1 && j.readBuffer(j.COLOR_ATTACHMENT0 + s), !N.textureFormatReadable(c)) {
						G("WebGLRenderer.readRenderTargetPixels: renderTarget is not in RGBA or implementation defined format.");
						return;
					}
					if (!N.textureTypeReadable(l)) {
						G("WebGLRenderer.readRenderTargetPixels: renderTarget is not in UnsignedByteType or implementation defined type.");
						return;
					}
					t >= 0 && t <= e.width - r && n >= 0 && n <= e.height - i && j.readPixels(t, n, r, i, Re.convert(c), Re.convert(l), a);
				} finally {
					let e = A === null ? null : F.get(A).__webglFramebuffer;
					P.bindFramebuffer(j.FRAMEBUFFER, e);
				}
			}
		}, this.readRenderTargetPixelsAsync = async function(e, t, n, r, i, a, o, s = 0) {
			if (!(e && e.isWebGLRenderTarget)) throw Error("THREE.WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");
			let c = F.get(e).__webglFramebuffer;
			if (e.isWebGLCubeRenderTarget && o !== void 0 && (c = c[o]), c) if (t >= 0 && t <= e.width - r && n >= 0 && n <= e.height - i) {
				P.bindFramebuffer(j.FRAMEBUFFER, c);
				let o = e.textures[s], l = o.format, u = o.type;
				if (e.textures.length > 1 && j.readBuffer(j.COLOR_ATTACHMENT0 + s), !N.textureFormatReadable(l)) throw Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in RGBA or implementation defined format.");
				if (!N.textureTypeReadable(u)) throw Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in UnsignedByteType or implementation defined type.");
				let d = j.createBuffer();
				j.bindBuffer(j.PIXEL_PACK_BUFFER, d), j.bufferData(j.PIXEL_PACK_BUFFER, a.byteLength, j.STREAM_READ), j.readPixels(t, n, r, i, Re.convert(l), Re.convert(u), 0);
				let f = A === null ? null : F.get(A).__webglFramebuffer;
				P.bindFramebuffer(j.FRAMEBUFFER, f);
				let p = j.fenceSync(j.SYNC_GPU_COMMANDS_COMPLETE, 0);
				return j.flush(), await br(j, p, 4), j.bindBuffer(j.PIXEL_PACK_BUFFER, d), j.getBufferSubData(j.PIXEL_PACK_BUFFER, 0, a), j.deleteBuffer(d), j.deleteSync(p), a;
			} else throw Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: requested read bounds are out of range.");
		}, this.copyFramebufferToTexture = function(e, t = null, n = 0) {
			let r = 2 ** -n, i = Math.floor(e.image.width * r), a = Math.floor(e.image.height * r), o = t === null ? 0 : t.x, s = t === null ? 0 : t.y;
			I.setTexture2D(e, 0), j.copyTexSubImage2D(j.TEXTURE_2D, n, 0, 0, o, s, i, a), P.unbindTexture();
		};
		let ut = j.createFramebuffer(), dt = j.createFramebuffer();
		this.copyTextureToTexture = function(e, t, n = null, r = null, i = 0, a = 0) {
			let o, s, c, l, u, d, f, p, m, h = e.isCompressedTexture ? e.mipmaps[a] : e.image;
			if (n !== null) o = n.max.x - n.min.x, s = n.max.y - n.min.y, c = n.isBox3 ? n.max.z - n.min.z : 1, l = n.min.x, u = n.min.y, d = n.isBox3 ? n.min.z : 0;
			else {
				let t = 2 ** -i;
				o = Math.floor(h.width * t), s = Math.floor(h.height * t), c = e.isDataArrayTexture ? h.depth : e.isData3DTexture ? Math.floor(h.depth * t) : 1, l = 0, u = 0, d = 0;
			}
			r === null ? (f = 0, p = 0, m = 0) : (f = r.x, p = r.y, m = r.z);
			let g = Re.convert(t.format), _ = Re.convert(t.type), v;
			t.isData3DTexture ? (I.setTexture3D(t, 0), v = j.TEXTURE_3D) : t.isDataArrayTexture || t.isCompressedArrayTexture ? (I.setTexture2DArray(t, 0), v = j.TEXTURE_2D_ARRAY) : (I.setTexture2D(t, 0), v = j.TEXTURE_2D), P.activeTexture(j.TEXTURE0), P.pixelStorei(j.UNPACK_FLIP_Y_WEBGL, t.flipY), P.pixelStorei(j.UNPACK_PREMULTIPLY_ALPHA_WEBGL, t.premultiplyAlpha), P.pixelStorei(j.UNPACK_ALIGNMENT, t.unpackAlignment);
			let y = P.getParameter(j.UNPACK_ROW_LENGTH), b = P.getParameter(j.UNPACK_IMAGE_HEIGHT), x = P.getParameter(j.UNPACK_SKIP_PIXELS), S = P.getParameter(j.UNPACK_SKIP_ROWS), C = P.getParameter(j.UNPACK_SKIP_IMAGES);
			P.pixelStorei(j.UNPACK_ROW_LENGTH, h.width), P.pixelStorei(j.UNPACK_IMAGE_HEIGHT, h.height), P.pixelStorei(j.UNPACK_SKIP_PIXELS, l), P.pixelStorei(j.UNPACK_SKIP_ROWS, u), P.pixelStorei(j.UNPACK_SKIP_IMAGES, d);
			let w = e.isDataArrayTexture || e.isData3DTexture, T = t.isDataArrayTexture || t.isData3DTexture;
			if (e.isDepthTexture) {
				let n = F.get(e), r = F.get(t), h = F.get(n.__renderTarget), g = F.get(r.__renderTarget);
				P.bindFramebuffer(j.READ_FRAMEBUFFER, h.__webglFramebuffer), P.bindFramebuffer(j.DRAW_FRAMEBUFFER, g.__webglFramebuffer);
				for (let n = 0; n < c; n++) w && (j.framebufferTextureLayer(j.READ_FRAMEBUFFER, j.COLOR_ATTACHMENT0, F.get(e).__webglTexture, i, d + n), j.framebufferTextureLayer(j.DRAW_FRAMEBUFFER, j.COLOR_ATTACHMENT0, F.get(t).__webglTexture, a, m + n)), j.blitFramebuffer(l, u, o, s, f, p, o, s, j.DEPTH_BUFFER_BIT, j.NEAREST);
				P.bindFramebuffer(j.READ_FRAMEBUFFER, null), P.bindFramebuffer(j.DRAW_FRAMEBUFFER, null);
			} else if (i !== 0 || e.isRenderTargetTexture || F.has(e)) {
				let n = F.get(e), r = F.get(t);
				P.bindFramebuffer(j.READ_FRAMEBUFFER, ut), P.bindFramebuffer(j.DRAW_FRAMEBUFFER, dt);
				for (let e = 0; e < c; e++) w ? j.framebufferTextureLayer(j.READ_FRAMEBUFFER, j.COLOR_ATTACHMENT0, n.__webglTexture, i, d + e) : j.framebufferTexture2D(j.READ_FRAMEBUFFER, j.COLOR_ATTACHMENT0, j.TEXTURE_2D, n.__webglTexture, i), T ? j.framebufferTextureLayer(j.DRAW_FRAMEBUFFER, j.COLOR_ATTACHMENT0, r.__webglTexture, a, m + e) : j.framebufferTexture2D(j.DRAW_FRAMEBUFFER, j.COLOR_ATTACHMENT0, j.TEXTURE_2D, r.__webglTexture, a), i === 0 ? T ? j.copyTexSubImage3D(v, a, f, p, m + e, l, u, o, s) : j.copyTexSubImage2D(v, a, f, p, l, u, o, s) : j.blitFramebuffer(l, u, o, s, f, p, o, s, j.COLOR_BUFFER_BIT, j.NEAREST);
				P.bindFramebuffer(j.READ_FRAMEBUFFER, null), P.bindFramebuffer(j.DRAW_FRAMEBUFFER, null);
			} else T ? e.isDataTexture || e.isData3DTexture ? j.texSubImage3D(v, a, f, p, m, o, s, c, g, _, h.data) : t.isCompressedArrayTexture ? j.compressedTexSubImage3D(v, a, f, p, m, o, s, c, g, h.data) : j.texSubImage3D(v, a, f, p, m, o, s, c, g, _, h) : e.isDataTexture ? j.texSubImage2D(j.TEXTURE_2D, a, f, p, o, s, g, _, h.data) : e.isCompressedTexture ? j.compressedTexSubImage2D(j.TEXTURE_2D, a, f, p, h.width, h.height, g, h.data) : j.texSubImage2D(j.TEXTURE_2D, a, f, p, o, s, g, _, h);
			P.pixelStorei(j.UNPACK_ROW_LENGTH, y), P.pixelStorei(j.UNPACK_IMAGE_HEIGHT, b), P.pixelStorei(j.UNPACK_SKIP_PIXELS, x), P.pixelStorei(j.UNPACK_SKIP_ROWS, S), P.pixelStorei(j.UNPACK_SKIP_IMAGES, C), a === 0 && t.generateMipmaps && j.generateMipmap(v), P.unbindTexture();
		}, this.initRenderTarget = function(e) {
			F.get(e).__webglFramebuffer === void 0 && I.setupRenderTarget(e);
		}, this.initTexture = function(e) {
			e.isCubeTexture ? I.setTextureCube(e, 0) : e.isData3DTexture ? I.setTexture3D(e, 0) : e.isDataArrayTexture || e.isCompressedArrayTexture ? I.setTexture2DArray(e, 0) : I.setTexture2D(e, 0), P.unbindTexture();
		}, this.resetState = function() {
			O = 0, k = 0, A = null, P.reset(), z.reset();
		}, typeof __THREE_DEVTOOLS__ < "u" && __THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe", { detail: this }));
	}
	get coordinateSystem() {
		return dr;
	}
	get outputColorSpace() {
		return this._outputColorSpace;
	}
	set outputColorSpace(e) {
		this._outputColorSpace = e;
		let t = this.getContext();
		t.drawingBufferColorSpace = Y._getDrawingBufferColorSpace(e), t.unpackColorSpace = Y._getUnpackColorSpace();
	}
};
//#endregion
//#region node_modules/.pnpm/three@0.184.0/node_modules/three/examples/jsm/utils/SkeletonUtils.js
function hf(e) {
	let t = /* @__PURE__ */ new Map(), n = /* @__PURE__ */ new Map(), r = e.clone();
	return gf(e, r, function(e, r) {
		t.set(r, e), n.set(e, r);
	}), r.traverse(function(e) {
		if (!e.isSkinnedMesh) return;
		let r = e, i = t.get(e), a = i.skeleton.bones;
		r.skeleton = i.skeleton.clone(), r.bindMatrix.copy(i.bindMatrix), r.skeleton.bones = a.map(function(e) {
			return n.get(e);
		}), r.bind(r.skeleton, r.bindMatrix);
	}), r;
}
function gf(e, t, n) {
	n(e, t);
	for (let r = 0; r < e.children.length; r++) gf(e.children[r], t.children[r], n);
}
//#endregion
//#region packages/renderer-three/dist/animated-mesh.js
var _f = class extends Error {
	constructor(e) {
		super(e), this.name = "AnimatedMeshApplyError";
	}
}, vf = class {
	#e;
	#t = /* @__PURE__ */ new Map();
	#n = /* @__PURE__ */ new Map();
	constructor(e) {
		this.#e = e;
	}
	get instanceCount() {
		return this.#n.size;
	}
	define(e) {
		let t = this.#t.get(e.asset);
		if (t && t.refCount > 0) throw new _f(`defineAnimatedMesh: asset ${e.asset} is in use by ${t.refCount} instance(s)`);
		let n = this.#r(e), r = yf(n.scene);
		t && xf(t.scene), this.#t.set(e.asset, {
			asset: e,
			resource: n,
			scene: r,
			refCount: 0
		});
	}
	validateDefinition(e) {
		this.#r(e);
	}
	#r(e) {
		if (e.runtimeFormat !== "glb") throw new _f(`defineAnimatedMesh: unsupported runtime format ${e.runtimeFormat}`);
		let t = this.#e?.getAnimatedMeshResource(e);
		if (!t) throw new _f(`defineAnimatedMesh: missing animated mesh resource ${e.asset}`);
		if (t.contentHash !== void 0 && t.contentHash !== e.contentHash) throw new _f(`defineAnimatedMesh: content hash mismatch for ${e.asset}; expected ${t.contentHash}, received ${e.contentHash}`);
		return Sf(e, t), t;
	}
	create(e, t) {
		let n = this.#t.get(t.asset);
		if (!n) throw new _f(`createAnimatedMeshInstance: undefined animated mesh asset ${t.asset}`);
		if (t.materialOverrides.length > 0) throw new _f(`createAnimatedMeshInstance: material overrides are not implemented for animated mesh ${t.asset}`);
		let r = hf(n.scene), i = new Gc(r), a = /* @__PURE__ */ new Map();
		for (let e of n.asset.clips) a.set(e.id, i.clipAction(Cf(n.resource, e.id, e.name)));
		let o = {
			handle: e,
			asset: t.asset,
			object: r,
			mixer: i,
			actions: a,
			currentClip: null,
			commandSelected: !1,
			status: "not_started",
			loop: null,
			speed: null,
			weight: null,
			controllerClips: []
		};
		return this.#n.set(e, o), n.refCount += 1, t.playback && this.setPlayback(e, t.playback), o;
	}
	setPlayback(e, t) {
		wf(this.#i(e, "setAnimatedMeshPlayback"), t);
	}
	setControllerWeights(e, t) {
		Tf(this.#i(e, "setAnimationControllerWeights"), t);
	}
	hasClips(e, t) {
		let n = this.#n.get(e);
		return n !== void 0 && t.every((e) => n.actions.has(e));
	}
	clearControllerWeights(e) {
		let t = this.#i(e, "clearAnimationControllerWeights");
		t.mixer.stopAllAction(), t.currentClip = null, t.controllerClips = [], t.commandSelected = !1, t.status = "stopped", t.loop = null, t.speed = null, t.weight = null;
	}
	advance(e) {
		if (!Number.isFinite(e) || e < 0) throw new _f("advanceAnimation: deltaSeconds must be finite and non-negative");
		for (let t of this.#n.values()) t.mixer.update(e);
	}
	playback(e) {
		let t = this.#n.get(e);
		if (!t) return;
		let n = t.currentClip === null ? null : t.actions.get(t.currentClip) ?? null;
		return {
			handle: e,
			asset: t.asset,
			status: t.status,
			currentClip: t.currentClip,
			mixerTimeSeconds: t.mixer.time,
			actionTimeSeconds: n?.time ?? null,
			running: n?.isRunning() ?? !1,
			paused: n?.paused ?? !1,
			loop: t.loop,
			speed: t.speed,
			weight: t.weight,
			commandSelected: t.commandSelected,
			poseSample: Af(t.object),
			diagnostics: jf(t, n),
			controllerClips: t.controllerClips
		};
	}
	sample(e, t, n) {
		if (!Number.isFinite(n) || n < 0 || n > 1) throw new _f("sampleAnimatedMesh: normalizedTime must be finite and between 0 and 1");
		let r = this.#i(e, "sampleAnimatedMesh"), i = r.actions.get(t);
		if (i === void 0) throw new _f(`sampleAnimatedMesh: missing clip ${t} on ${r.asset}`);
		let a = i.getClip().duration;
		if (!Number.isFinite(a) || a <= 0) throw new _f(`sampleAnimatedMesh: clip ${t} has an invalid duration`);
		let o = this.#t.get(r.asset);
		if (o === void 0) throw new _f(`sampleAnimatedMesh: missing defined asset ${r.asset}`);
		let s = Lf(r.object, o.scene, i.getClip());
		return r.mixer.stopAllAction(), i.reset(), i.enabled = !0, i.paused = !1, i.clampWhenFinished = !0, i.setLoop(Gn, 1), i.setEffectiveTimeScale(1), i.setEffectiveWeight(1), i.play(), r.mixer.setTime(a * n), i.paused = !0, r.currentClip = t, r.commandSelected = !0, r.status = "paused", r.loop = "once", r.speed = 1, r.weight = 1, r.controllerClips = [], Pf(r, s, t, n, a, o.asset.bounds, o.asset.contentHash);
	}
	release(e) {
		let t = this.#n.get(e);
		if (!t) return;
		t.mixer.stopAllAction(), t.mixer.uncacheRoot(t.object), this.#n.delete(e);
		let n = this.#t.get(t.asset);
		n && --n.refCount;
	}
	dispose() {
		for (let e of [...this.#n.keys()]) this.release(e);
		for (let e of this.#t.values()) xf(e.scene);
		this.#t.clear();
	}
	#i(e, t) {
		let n = this.#n.get(e);
		if (!n) throw new _f(`${t}: handle ${e} is not an animated mesh`);
		return n;
	}
};
function yf(e) {
	let t = hf(e), n = /* @__PURE__ */ new Map(), r = /* @__PURE__ */ new Map();
	return t.traverse((e) => {
		let t = e;
		if (t.geometry instanceof Ja) {
			let e = t.geometry, r = n.get(e);
			r === void 0 && (r = e.clone(), n.set(e, r)), t.geometry = r;
		}
		Array.isArray(t.material) ? t.material = t.material.map((e) => bf(e, r)) : t.material instanceof Xa && (t.material = bf(t.material, r));
	}), t;
}
function bf(e, t) {
	let n = t.get(e);
	return n === void 0 && (n = e.clone(), t.set(e, n)), n;
}
function xf(e) {
	let t = /* @__PURE__ */ new Set(), n = /* @__PURE__ */ new Set();
	e.traverse((e) => {
		let r = e;
		r.geometry instanceof Ja && t.add(r.geometry), Array.isArray(r.material) ? r.material.forEach((e) => n.add(e)) : r.material instanceof Xa && n.add(r.material);
	}), t.forEach((e) => e.dispose()), n.forEach((e) => e.dispose());
}
function Sf(e, t) {
	for (let n of e.clips) Cf(t, n.id, n.name);
}
function Cf(e, t, n) {
	let r = e.clips.find((e) => e.name === t || n !== null && e.name === n);
	if (!r) throw new _f(`animated mesh ${e.asset} does not contain clip ${t}`);
	return r;
}
function wf(e, t) {
	switch (t.kind) {
		case "play":
			Ef(e, t);
			return;
		case "stop":
			Df(e, t.fadeSeconds), e.currentClip = null, e.commandSelected = !0, e.status = "stopped", e.loop = null, e.speed = null, e.weight = null;
			return;
		case "pause":
			Of(e, "pause").paused = !0, e.commandSelected = !0, e.status = "paused";
			return;
		case "resume": {
			let t = Of(e, "resume");
			t.paused = !1, t.play(), e.commandSelected = !0, e.status = "playing";
			return;
		}
	}
}
function Tf(e, t) {
	if (t.length === 0 || t.length > 4) throw new _f("setAnimationControllerWeights: expected one to four clips");
	let n = /* @__PURE__ */ new Map(), r = 0;
	for (let i of t) {
		if (n.has(i.clip) || !Number.isFinite(i.weight) || i.weight < 0 || i.weight > 1 || !Number.isFinite(i.speed) || i.speed <= 0) throw new _f("setAnimationControllerWeights: invalid clip sample");
		if (!e.actions.has(i.clip)) throw new _f(`setAnimationControllerWeights: missing clip ${i.clip} on ${e.asset}`);
		n.set(i.clip, i), r += i.weight;
	}
	if (Math.abs(r - 1) > .001) throw new _f(`setAnimationControllerWeights: weights must sum to 1, received ${r}`);
	for (let [t, r] of e.actions) {
		let e = n.get(t);
		if (e === void 0) {
			r.stop();
			continue;
		}
		r.enabled = !0, r.paused = !1, r.setLoop(Kn, Infinity), r.setEffectiveTimeScale(e.speed), r.setEffectiveWeight(e.weight), r.play();
	}
	e.currentClip = t.reduce((e, t) => e === null || t.weight > e.weight ? t : e, null)?.clip ?? null, e.commandSelected = !1, e.status = "playing", e.loop = "repeat", e.speed = null, e.weight = null, e.controllerClips = t.map((e) => ({ ...e }));
}
function Ef(e, t) {
	let n = e.actions.get(t.clip);
	if (!n) throw new _f(`setAnimatedMeshPlayback: missing clip ${t.clip} on ${e.asset}`);
	let r = e.currentClip === null ? null : e.actions.get(e.currentClip) ?? null;
	t.restart && n.reset(), n.enabled = !0, n.paused = !1, n.clampWhenFinished = t.loop === "once", n.setLoop(kf(t.loop), t.loop === "once" ? 1 : Infinity), n.setEffectiveTimeScale(t.speed), n.setEffectiveWeight(t.weight), r && r !== n && (t.fadeSeconds !== null && t.fadeSeconds > 0 ? n.crossFadeFrom(r, t.fadeSeconds, !1) : r.stop()), n.play(), e.currentClip = t.clip, e.controllerClips = [], e.commandSelected = !0, e.status = "playing", e.loop = t.loop, e.speed = t.speed, e.weight = t.weight;
}
function Df(e, t) {
	let n = e.currentClip === null ? null : e.actions.get(e.currentClip) ?? null;
	n && (t !== null && t > 0 ? n.fadeOut(t) : n.stop());
}
function Of(e, t) {
	let n = e.currentClip === null ? null : e.actions.get(e.currentClip) ?? null;
	if (!n) throw new _f(`setAnimatedMeshPlayback.${t}: no current clip on ${e.asset}`);
	return n;
}
function kf(e) {
	switch (e) {
		case "once": return Gn;
		case "repeat": return Kn;
		case "pingPong": return qn;
	}
}
function Af(e) {
	let t = [
		0,
		0,
		0
	], n = [
		0,
		0,
		0,
		0
	], r = [
		0,
		0,
		0
	], i = 0;
	return e.traverse((e) => {
		i += 1, t[0] += e.position.x, t[1] += e.position.y, t[2] += e.position.z, n[0] += e.quaternion.x, n[1] += e.quaternion.y, n[2] += e.quaternion.z, n[3] += e.quaternion.w, r[0] += e.scale.x, r[1] += e.scale.y, r[2] += e.scale.z;
	}), {
		rootTranslation: [
			e.position.x,
			e.position.y,
			e.position.z
		],
		rootRotation: [
			e.quaternion.x,
			e.quaternion.y,
			e.quaternion.z,
			e.quaternion.w
		],
		rootScale: [
			e.scale.x,
			e.scale.y,
			e.scale.z
		],
		hierarchyNodeCount: i,
		hierarchyTranslationSum: t,
		hierarchyRotationSum: n,
		hierarchyScaleSum: r
	};
}
function jf(e, t) {
	return e.commandSelected ? e.status === "stopped" ? ["animation_stopped"] : t?.paused || e.status === "paused" ? ["animation_paused"] : [] : ["animation_not_started"];
}
var Mf = 1e6, Nf = 64;
function Pf(e, t, n, r, i, a, o) {
	let s = [], c = (e, t, n) => {
		s.length < Nf && s.push({
			code: e,
			message: t,
			node: zf(n)
		});
	}, l = 0, u = 0, d = !1, f = new ha(), p = new q(), m = new vi();
	e.object.updateMatrixWorld(!0), e.object.traverse((e) => {
		Rf(e) || c("node_transform_non_finite", "node transform contains a non-finite value", e);
		let t = e.quaternion.lengthSq();
		if ((!Number.isFinite(t) || t < 1e-12) && c("node_quaternion_invalid", "node quaternion is non-finite or has zero length", e), (!Number.isFinite(e.scale.x) || !Number.isFinite(e.scale.y) || !Number.isFinite(e.scale.z) || Math.abs(e.scale.x) < 1e-12 || Math.abs(e.scale.y) < 1e-12 || Math.abs(e.scale.z) < 1e-12) && c("node_scale_invalid", "node scale is non-finite or singular", e), e instanceof jo && (l += 1), !(e instanceof vo)) return;
		let n = e.geometry.getAttribute("position");
		if (n !== void 0) {
			if (u + n.count > Mf) {
				d = !0;
				return;
			}
			if (e instanceof Ao) {
				e.skeleton.update();
				for (let t = 0; t < e.skeleton.bones.length; t += 1) {
					let n = e.skeleton.bones[t], r = e.skeleton.boneInverses[t];
					n === void 0 || r === void 0 || (m.multiplyMatrices(n.matrixWorld, r), m.elements.every(Number.isFinite) ? Math.abs(m.determinant()) < 1e-12 && c("bone_matrix_singular", "bone skin matrix is singular", n) : c("bone_matrix_non_finite", "bone skin matrix contains a non-finite value", n));
				}
			}
			for (let t = 0; t < n.count; t += 1) p.fromBufferAttribute(n, t), e instanceof Ao && e.applyBoneTransform(t, p), e.localToWorld(p), f.expandByPoint(p);
			u += n.count;
		}
	}), d && c("vertex_budget_exceeded", `sample contains more than ${Mf} vertices`, null);
	let h = !f.isEmpty() && !d ? Bf(f) : null;
	return h !== null && Vf(a, h, e.object) && c("sampled_bounds_implausible", "sampled world bounds expand beyond eight times the admitted asset extent", null), {
		handle: e.handle,
		asset: e.asset,
		contentHash: o,
		clip: n,
		normalizedTime: r,
		durationSeconds: i,
		assetBounds: {
			min: [...a.min],
			max: [...a.max]
		},
		sampledWorldBounds: h,
		sampledVertexCount: u,
		boneCount: l,
		skinningFacts: t,
		diagnostics: s
	};
}
var Ff = 256, If = 1e-4;
function Lf(e, t, n) {
	let r = /* @__PURE__ */ new Map(), i = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Set(), o = /* @__PURE__ */ new Map();
	if (t.updateMatrixWorld(!0), t.traverse((e) => {
		e instanceof jo && r.set(e.name, e), e instanceof vo && o.set(e.name, e), e instanceof Ao && (a.add(e.skeleton), e.skeleton.bones.forEach((t, n) => {
			let r = e.skeleton.boneInverses[n];
			r !== void 0 && i.set(t.name, r);
		}));
	}), r.size > Ff) throw new _f(`sampleAnimatedMesh: joint count exceeds ${Ff}`);
	let s = 0, c = 0, l = !0, u = 0, d = 0, f = 0, p = !0, m = 0, h = 0;
	e.updateMatrixWorld(!0), e.traverse((e) => {
		if (!(e instanceof vo)) return;
		let t = o.get(e.name);
		if (t?.geometry === e.geometry && (m += 1), t?.material === e.material && (h += 1), !(e instanceof Ao)) return;
		s += 1, a.has(e.skeleton) && (p = !1), e.skeleton.bones.forEach((t, n) => {
			r.get(t.name) === t && (p = !1);
			let i = e.skeleton.boneInverses[n];
			i !== void 0 && (c += 1, i.elements.every(Number.isFinite) || (l = !1));
		});
		let n = e.geometry.getAttribute("skinWeight");
		if (n !== void 0) for (let e = 0; e < n.count; e += 1) {
			let t = n.getX(e) + (n.itemSize > 1 ? n.getY(e) : 0) + (n.itemSize > 2 ? n.getZ(e) : 0) + (n.itemSize > 3 ? n.getW(e) : 0);
			if (u += 1, !Number.isFinite(t) || t <= 0) {
				d += 1;
				continue;
			}
			f = Math.max(f, Math.abs(t - 1));
		}
	});
	let g = [...new Set(n.tracks.map((e) => {
		switch (e.getInterpolation()) {
			case Jn: return "discrete";
			case Xn: return "smooth";
			default: return "linear";
		}
	}))].sort();
	return {
		joints: [...r.values()].map((e) => ({
			name: e.name,
			parent: e.parent instanceof jo ? e.parent.name : null,
			restLocalMatrix: [...e.matrix.elements],
			inverseBindMatrix: i.has(e.name) ? [...i.get(e.name).elements] : null
		})),
		skinnedMeshCount: s,
		inverseBindMatrixCount: c,
		inverseBindMatricesFinite: l,
		weightedVertexCount: u,
		invalidWeightVertexCount: d,
		maximumWeightSumError: f,
		weightsNormalized: u > 0 && d === 0 && f <= If,
		interpolationModes: g,
		instanceRootDistinctFromTemplate: e !== t,
		skeletonsIndependentFromTemplate: p,
		sharedGeometryCount: m,
		sharedMaterialCount: h
	};
}
function Rf(e) {
	return [
		...e.position.toArray(),
		...e.quaternion.toArray(),
		...e.scale.toArray(),
		...e.matrix.elements,
		...e.matrixWorld.elements
	].every(Number.isFinite);
}
function zf(e) {
	return e === null ? null : e.name.length > 0 ? e.name : `${e.type}:${e.id}`;
}
function Bf(e) {
	return {
		min: e.min.toArray(),
		max: e.max.toArray()
	};
}
function Vf(e, t, n) {
	let r = Math.max(e.max[0] - e.min[0], e.max[1] - e.min[1], e.max[2] - e.min[2], 1e-6), i = Math.max(t.max[0] - t.min[0], t.max[1] - t.min[1], t.max[2] - t.min[2]), a = n.getWorldScale(new q());
	return i > r * Math.max(Math.abs(a.x), Math.abs(a.y), Math.abs(a.z), 1e-6) * 8;
}
//#endregion
//#region packages/renderer-three/dist/lighting.js
var Hf = class extends Error {
	code;
	constructor(e, t) {
		super(t), this.code = e, this.name = "RendererLightingPolicyError";
	}
};
function Uf(e, t) {
	let n = new X(...e.color), r;
	switch (e.kind) {
		case "ambient":
			r = new Tc(n, e.intensity);
			break;
		case "directional": {
			let t = new wc(n, e.intensity);
			t.add(t.target), t.target.position.set(...Xf(e.direction)), r = t;
			break;
		}
		case "point":
			r = new xc(n, e.intensity, e.range ?? 0, e.decay), r.position.set(...e.position);
			break;
		case "spot": {
			let t = new yc(n, e.intensity, e.range ?? 0, e.outerAngleRadians, e.penumbra, e.decay);
			t.position.set(...e.position), t.add(t.target), t.target.position.set(...Xf(e.direction)), r = t;
			break;
		}
	}
	return r.visible = e.enabled, Yf(r, e, t), r;
}
function Wf(e, t, n) {
	let r = e;
	if (r.color.setRGB(t.color[0], t.color[1], t.color[2]), r.intensity = t.intensity, r.visible = t.enabled, t.kind === "directional") r.target.position.set(...Xf(t.direction));
	else if (t.kind === "point") {
		let e = r;
		e.position.set(...t.position), e.distance = t.range ?? 0, e.decay = t.decay;
	} else if (t.kind === "spot") {
		let e = r;
		e.position.set(...t.position), e.target.position.set(...Xf(t.direction)), e.distance = t.range ?? 0, e.decay = t.decay, e.angle = t.outerAngleRadians, e.penumbra = t.penumbra;
	}
	Yf(r, t, n);
}
function Gf(e, t) {
	return !e.enabled || e.shadowIntent === "disabled" ? "disabled" : t && e.kind !== "ambient" ? "active" : "requested_unsupported";
}
function Kf(e, t) {
	if (e === null) return null;
	for (let [n, r] of t) if (r.object === e) return n;
	return null;
}
function qf(e) {
	e.clear(), e.removeFromParent();
}
function Jf(e, t, r) {
	if (e.color.forEach((e, n) => {
		if (!Number.isFinite(e) || e < 0 || e > 1) throw r(`${t}.color[${n}] must be finite and in 0..=1`);
	}), !Number.isFinite(e.intensity) || e.intensity < 0 || e.intensity > 1e4) throw r(`${t}.intensity must be finite and in 0..=${String(n)}`);
	if ((e.kind === "directional" || e.kind === "spot") && (e.direction.forEach((e, n) => {
		if (!Number.isFinite(e)) throw r(`${t}.direction[${n}] must be finite`);
	}), e.direction.reduce((e, t) => e + t * t, 0) <= 2 ** -52)) throw r(`${t}.direction must be non-zero`);
	if (e.kind === "point" || e.kind === "spot") {
		if (e.position.forEach((e, n) => {
			if (!Number.isFinite(e)) throw r(`${t}.position[${n}] must be finite`);
		}), e.range !== null && (!Number.isFinite(e.range) || e.range <= 0)) throw r(`${t}.range must be null or finite and positive`);
		if (!Number.isFinite(e.decay) || e.decay < 0) throw r(`${t}.decay must be finite and non-negative`);
	}
	if (e.kind === "spot") {
		if (!Number.isFinite(e.outerAngleRadians) || e.outerAngleRadians <= 0 || e.outerAngleRadians > Math.PI / 2) throw r(`${t}.outerAngleRadians must be in (0, pi/2]`);
		if (!Number.isFinite(e.penumbra) || e.penumbra < 0 || e.penumbra > 1) throw r(`${t}.penumbra must be in 0..=1`);
	}
}
function Yf(e, t, n) {
	t.kind !== "ambient" && "castShadow" in e && (e.castShadow = n && t.enabled && t.shadowIntent === "requested");
}
function Xf(e) {
	let t = new q(...e).normalize();
	return [
		t.x,
		t.y,
		t.z
	];
}
//#endregion
//#region packages/renderer-three/dist/mesh-presentation.js
var Zf = {
	color: [
		1,
		1,
		1,
		1
	],
	wireframe: !1
};
function Qf(e) {
	let t = e.material;
	return Array.isArray(t) ? t : [t];
}
//#endregion
//#region node_modules/.pnpm/@noble+hashes@2.2.0/node_modules/@noble/hashes/utils.js
function $f(e) {
	return e instanceof Uint8Array || ArrayBuffer.isView(e) && e.constructor.name === "Uint8Array" && "BYTES_PER_ELEMENT" in e && e.BYTES_PER_ELEMENT === 1;
}
function ep(e, t, n = "") {
	let r = $f(e), i = e?.length, a = t !== void 0;
	if (!r || a && i !== t) {
		let o = n && `"${n}" `, s = a ? ` of length ${t}` : "", c = r ? `length=${i}` : `type=${typeof e}`, l = o + "expected Uint8Array" + s + ", got " + c;
		throw r ? RangeError(l) : TypeError(l);
	}
	return e;
}
function tp(e, t = !0) {
	if (e.destroyed) throw Error("Hash instance has been destroyed");
	if (t && e.finished) throw Error("Hash#digest() has already been called");
}
function np(e, t) {
	ep(e, void 0, "digestInto() output");
	let n = t.outputLen;
	if (e.length < n) throw RangeError("\"digestInto() output\" expected to be of length >=" + n);
}
function rp(...e) {
	for (let t = 0; t < e.length; t++) e[t].fill(0);
}
function ip(e) {
	return new DataView(e.buffer, e.byteOffset, e.byteLength);
}
function ap(e, t) {
	return e << 32 - t | e >>> t;
}
var op = typeof Uint8Array.from([]).toHex == "function" && typeof Uint8Array.fromHex == "function", sp = /* @__PURE__ */ Array.from({ length: 256 }, (e, t) => t.toString(16).padStart(2, "0"));
function cp(e) {
	if (ep(e), op) return e.toHex();
	let t = "";
	for (let n = 0; n < e.length; n++) t += sp[e[n]];
	return t;
}
function lp(e, t = {}) {
	let n = (t, n) => e(n).update(t).digest(), r = e(void 0);
	return n.outputLen = r.outputLen, n.blockLen = r.blockLen, n.canXOF = r.canXOF, n.create = (t) => e(t), Object.assign(n, t), Object.freeze(n);
}
var up = (e) => ({ oid: Uint8Array.from([
	6,
	9,
	96,
	134,
	72,
	1,
	101,
	3,
	4,
	2,
	e
]) });
//#endregion
//#region node_modules/.pnpm/@noble+hashes@2.2.0/node_modules/@noble/hashes/_md.js
function dp(e, t, n) {
	return e & t ^ ~e & n;
}
function fp(e, t, n) {
	return e & t ^ e & n ^ t & n;
}
var pp = class {
	blockLen;
	outputLen;
	canXOF = !1;
	padOffset;
	isLE;
	buffer;
	view;
	finished = !1;
	length = 0;
	pos = 0;
	destroyed = !1;
	constructor(e, t, n, r) {
		this.blockLen = e, this.outputLen = t, this.padOffset = n, this.isLE = r, this.buffer = new Uint8Array(e), this.view = ip(this.buffer);
	}
	update(e) {
		tp(this), ep(e);
		let { view: t, buffer: n, blockLen: r } = this, i = e.length;
		for (let a = 0; a < i;) {
			let o = Math.min(r - this.pos, i - a);
			if (o === r) {
				let t = ip(e);
				for (; r <= i - a; a += r) this.process(t, a);
				continue;
			}
			n.set(e.subarray(a, a + o), this.pos), this.pos += o, a += o, this.pos === r && (this.process(t, 0), this.pos = 0);
		}
		return this.length += e.length, this.roundClean(), this;
	}
	digestInto(e) {
		tp(this), np(e, this), this.finished = !0;
		let { buffer: t, view: n, blockLen: r, isLE: i } = this, { pos: a } = this;
		t[a++] = 128, rp(this.buffer.subarray(a)), this.padOffset > r - a && (this.process(n, 0), a = 0);
		for (let e = a; e < r; e++) t[e] = 0;
		n.setBigUint64(r - 8, BigInt(this.length * 8), i), this.process(n, 0);
		let o = ip(e), s = this.outputLen;
		if (s % 4) throw Error("_sha2: outputLen must be aligned to 32bit");
		let c = s / 4, l = this.get();
		if (c > l.length) throw Error("_sha2: outputLen bigger than state");
		for (let e = 0; e < c; e++) o.setUint32(4 * e, l[e], i);
	}
	digest() {
		let { buffer: e, outputLen: t } = this;
		this.digestInto(e);
		let n = e.slice(0, t);
		return this.destroy(), n;
	}
	_cloneInto(e) {
		e ||= new this.constructor(), e.set(...this.get());
		let { blockLen: t, buffer: n, length: r, finished: i, destroyed: a, pos: o } = this;
		return e.destroyed = a, e.finished = i, e.length = r, e.pos = o, r % t && e.buffer.set(n), e;
	}
	clone() {
		return this._cloneInto();
	}
}, mp = /* @__PURE__ */ Uint32Array.from([
	1779033703,
	3144134277,
	1013904242,
	2773480762,
	1359893119,
	2600822924,
	528734635,
	1541459225
]), hp = /* @__PURE__ */ Uint32Array.from([
	1116352408,
	1899447441,
	3049323471,
	3921009573,
	961987163,
	1508970993,
	2453635748,
	2870763221,
	3624381080,
	310598401,
	607225278,
	1426881987,
	1925078388,
	2162078206,
	2614888103,
	3248222580,
	3835390401,
	4022224774,
	264347078,
	604807628,
	770255983,
	1249150122,
	1555081692,
	1996064986,
	2554220882,
	2821834349,
	2952996808,
	3210313671,
	3336571891,
	3584528711,
	113926993,
	338241895,
	666307205,
	773529912,
	1294757372,
	1396182291,
	1695183700,
	1986661051,
	2177026350,
	2456956037,
	2730485921,
	2820302411,
	3259730800,
	3345764771,
	3516065817,
	3600352804,
	4094571909,
	275423344,
	430227734,
	506948616,
	659060556,
	883997877,
	958139571,
	1322822218,
	1537002063,
	1747873779,
	1955562222,
	2024104815,
	2227730452,
	2361852424,
	2428436474,
	2756734187,
	3204031479,
	3329325298
]), gp = /* @__PURE__ */ new Uint32Array(64), _p = class extends pp {
	constructor(e) {
		super(64, e, 8, !1);
	}
	get() {
		let { A: e, B: t, C: n, D: r, E: i, F: a, G: o, H: s } = this;
		return [
			e,
			t,
			n,
			r,
			i,
			a,
			o,
			s
		];
	}
	set(e, t, n, r, i, a, o, s) {
		this.A = e | 0, this.B = t | 0, this.C = n | 0, this.D = r | 0, this.E = i | 0, this.F = a | 0, this.G = o | 0, this.H = s | 0;
	}
	process(e, t) {
		for (let n = 0; n < 16; n++, t += 4) gp[n] = e.getUint32(t, !1);
		for (let e = 16; e < 64; e++) {
			let t = gp[e - 15], n = gp[e - 2], r = ap(t, 7) ^ ap(t, 18) ^ t >>> 3;
			gp[e] = (ap(n, 17) ^ ap(n, 19) ^ n >>> 10) + gp[e - 7] + r + gp[e - 16] | 0;
		}
		let { A: n, B: r, C: i, D: a, E: o, F: s, G: c, H: l } = this;
		for (let e = 0; e < 64; e++) {
			let t = ap(o, 6) ^ ap(o, 11) ^ ap(o, 25), u = l + t + dp(o, s, c) + hp[e] + gp[e] | 0, d = (ap(n, 2) ^ ap(n, 13) ^ ap(n, 22)) + fp(n, r, i) | 0;
			l = c, c = s, s = o, o = a + u | 0, a = i, i = r, r = n, n = u + d | 0;
		}
		n = n + this.A | 0, r = r + this.B | 0, i = i + this.C | 0, a = a + this.D | 0, o = o + this.E | 0, s = s + this.F | 0, c = c + this.G | 0, l = l + this.H | 0, this.set(n, r, i, a, o, s, c, l);
	}
	roundClean() {
		rp(gp);
	}
	destroy() {
		this.destroyed = !0, this.set(0, 0, 0, 0, 0, 0, 0, 0), rp(this.buffer);
	}
}, vp = class extends _p {
	A = mp[0] | 0;
	B = mp[1] | 0;
	C = mp[2] | 0;
	D = mp[3] | 0;
	E = mp[4] | 0;
	F = mp[5] | 0;
	G = mp[6] | 0;
	H = mp[7] | 0;
	constructor() {
		super(32);
	}
}, yp = /* @__PURE__ */ lp(() => new vp(), /* @__PURE__ */ up(1)), bp = Uint8Array, xp = Uint16Array, Sp = Int32Array, Cp = new bp([
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	1,
	1,
	1,
	1,
	2,
	2,
	2,
	2,
	3,
	3,
	3,
	3,
	4,
	4,
	4,
	4,
	5,
	5,
	5,
	5,
	0,
	0,
	0,
	0
]), wp = new bp([
	0,
	0,
	0,
	0,
	1,
	1,
	2,
	2,
	3,
	3,
	4,
	4,
	5,
	5,
	6,
	6,
	7,
	7,
	8,
	8,
	9,
	9,
	10,
	10,
	11,
	11,
	12,
	12,
	13,
	13,
	0,
	0
]), Tp = new bp([
	16,
	17,
	18,
	0,
	8,
	7,
	9,
	6,
	10,
	5,
	11,
	4,
	12,
	3,
	13,
	2,
	14,
	1,
	15
]), Ep = function(e, t) {
	for (var n = new xp(31), r = 0; r < 31; ++r) n[r] = t += 1 << e[r - 1];
	for (var i = new Sp(n[30]), r = 1; r < 30; ++r) for (var a = n[r]; a < n[r + 1]; ++a) i[a] = a - n[r] << 5 | r;
	return {
		b: n,
		r: i
	};
}, Dp = Ep(Cp, 2), Op = Dp.b, kp = Dp.r;
Op[28] = 258, kp[258] = 28;
var Ap = Ep(wp, 0), jp = Ap.b;
Ap.r;
for (var Mp = new xp(32768), Np = 0; Np < 32768; ++Np) {
	var Pp = (Np & 43690) >> 1 | (Np & 21845) << 1;
	Pp = (Pp & 52428) >> 2 | (Pp & 13107) << 2, Pp = (Pp & 61680) >> 4 | (Pp & 3855) << 4, Mp[Np] = ((Pp & 65280) >> 8 | (Pp & 255) << 8) >> 1;
}
for (var Fp = (function(e, t, n) {
	for (var r = e.length, i = 0, a = new xp(t); i < r; ++i) e[i] && ++a[e[i] - 1];
	var o = new xp(t);
	for (i = 1; i < t; ++i) o[i] = o[i - 1] + a[i - 1] << 1;
	var s;
	if (n) {
		s = new xp(1 << t);
		var c = 15 - t;
		for (i = 0; i < r; ++i) if (e[i]) for (var l = i << 4 | e[i], u = t - e[i], d = o[e[i] - 1]++ << u, f = d | (1 << u) - 1; d <= f; ++d) s[Mp[d] >> c] = l;
	} else for (s = new xp(r), i = 0; i < r; ++i) e[i] && (s[i] = Mp[o[e[i] - 1]++] >> 15 - e[i]);
	return s;
}), Ip = new bp(288), Np = 0; Np < 144; ++Np) Ip[Np] = 8;
for (var Np = 144; Np < 256; ++Np) Ip[Np] = 9;
for (var Np = 256; Np < 280; ++Np) Ip[Np] = 7;
for (var Np = 280; Np < 288; ++Np) Ip[Np] = 8;
for (var Lp = new bp(32), Np = 0; Np < 32; ++Np) Lp[Np] = 5;
var Rp = /*#__PURE__*/ Fp(Ip, 9, 1), zp = /*#__PURE__*/ Fp(Lp, 5, 1), Bp = function(e) {
	for (var t = e[0], n = 1; n < e.length; ++n) e[n] > t && (t = e[n]);
	return t;
}, Vp = function(e, t, n) {
	var r = t / 8 | 0;
	return (e[r] | e[r + 1] << 8) >> (t & 7) & n;
}, Hp = function(e, t) {
	var n = t / 8 | 0;
	return (e[n] | e[n + 1] << 8 | e[n + 2] << 16) >> (t & 7);
}, Up = function(e) {
	return (e + 7) / 8 | 0;
}, Wp = function(e, t, n) {
	return (t == null || t < 0) && (t = 0), (n == null || n > e.length) && (n = e.length), new bp(e.subarray(t, n));
}, Gp = [
	"unexpected EOF",
	"invalid block type",
	"invalid length/literal",
	"invalid distance",
	"stream finished",
	"no stream handler",
	,
	"no callback",
	"invalid UTF-8 data",
	"extra field too long",
	"date not in range 1980-2099",
	"filename too long",
	"stream finishing",
	"invalid zip data"
], Kp = function(e, t, n) {
	var r = Error(t || Gp[e]);
	if (r.code = e, Error.captureStackTrace && Error.captureStackTrace(r, Kp), !n) throw r;
	return r;
}, qp = function(e, t, n, r) {
	var i = e.length, a = r ? r.length : 0;
	if (!i || t.f && !t.l) return n || new bp(0);
	var o = !n, s = o || t.i != 2, c = t.i;
	o && (n = new bp(i * 3));
	var l = function(e) {
		var t = n.length;
		if (e > t) {
			var r = new bp(Math.max(t * 2, e));
			r.set(n), n = r;
		}
	}, u = t.f || 0, d = t.p || 0, f = t.b || 0, p = t.l, m = t.d, h = t.m, g = t.n, _ = i * 8;
	do {
		if (!p) {
			u = Vp(e, d, 1);
			var v = Vp(e, d + 1, 3);
			if (d += 3, !v) {
				var y = Up(d) + 4, b = e[y - 4] | e[y - 3] << 8, x = y + b;
				if (x > i) {
					c && Kp(0);
					break;
				}
				s && l(f + b), n.set(e.subarray(y, x), f), t.b = f += b, t.p = d = x * 8, t.f = u;
				continue;
			} else if (v == 1) p = Rp, m = zp, h = 9, g = 5;
			else if (v == 2) {
				var S = Vp(e, d, 31) + 257, C = Vp(e, d + 10, 15) + 4, w = S + Vp(e, d + 5, 31) + 1;
				d += 14;
				for (var T = new bp(w), E = new bp(19), D = 0; D < C; ++D) E[Tp[D]] = Vp(e, d + D * 3, 7);
				d += C * 3;
				for (var O = Bp(E), k = (1 << O) - 1, A = Fp(E, O, 1), D = 0; D < w;) {
					var ee = A[Vp(e, d, k)];
					d += ee & 15;
					var y = ee >> 4;
					if (y < 16) T[D++] = y;
					else {
						var te = 0, ne = 0;
						for (y == 16 ? (ne = 3 + Vp(e, d, 3), d += 2, te = T[D - 1]) : y == 17 ? (ne = 3 + Vp(e, d, 7), d += 3) : y == 18 && (ne = 11 + Vp(e, d, 127), d += 7); ne--;) T[D++] = te;
					}
				}
				var re = T.subarray(0, S), ie = T.subarray(S);
				h = Bp(re), g = Bp(ie), p = Fp(re, h, 1), m = Fp(ie, g, 1);
			} else Kp(1);
			if (d > _) {
				c && Kp(0);
				break;
			}
		}
		s && l(f + 131072);
		for (var ae = (1 << h) - 1, oe = (1 << g) - 1, se = d;; se = d) {
			var te = p[Hp(e, d) & ae], ce = te >> 4;
			if (d += te & 15, d > _) {
				c && Kp(0);
				break;
			}
			if (te || Kp(2), ce < 256) n[f++] = ce;
			else if (ce == 256) {
				se = d, p = null;
				break;
			} else {
				var le = ce - 254;
				if (ce > 264) {
					var D = ce - 257, ue = Cp[D];
					le = Vp(e, d, (1 << ue) - 1) + Op[D], d += ue;
				}
				var de = m[Hp(e, d) & oe], fe = de >> 4;
				de || Kp(3), d += de & 15;
				var ie = jp[fe];
				if (fe > 3) {
					var ue = wp[fe];
					ie += Hp(e, d) & (1 << ue) - 1, d += ue;
				}
				if (d > _) {
					c && Kp(0);
					break;
				}
				s && l(f + 131072);
				var pe = f + le;
				if (f < ie) {
					var me = a - ie, he = Math.min(ie, pe);
					for (me + f < 0 && Kp(3); f < he; ++f) n[f] = r[me + f];
				}
				for (; f < pe; ++f) n[f] = n[f - ie];
			}
		}
		t.l = p, t.p = se, t.b = f, t.f = u, p && (u = 1, t.m = h, t.d = m, t.n = g);
	} while (!u);
	return f != n.length && o ? Wp(n, 0, f) : n.subarray(0, f);
}, Jp = /*#__PURE__*/ new bp(0), Yp = function(e, t) {
	return ((e[0] & 15) != 8 || e[0] >> 4 > 7 || (e[0] << 8 | e[1]) % 31) && Kp(6, "invalid zlib data"), (e[1] >> 5 & 1) == +!t && Kp(6, "invalid zlib data: " + (e[1] & 32 ? "need" : "unexpected") + " dictionary"), (e[1] >> 3 & 4) + 2;
};
function Xp(e, t) {
	return qp(e.subarray(Yp(e, t && t.dictionary), -4), { i: 2 }, t && t.out, t && t.dictionary);
}
var Zp = typeof TextDecoder < "u" && /*#__PURE__*/ new TextDecoder();
try {
	Zp.decode(Jp, { stream: !0 });
} catch {}
//#endregion
//#region packages/renderer-three/dist/png-texture.js
var Qp = class extends Error {
	constructor(e) {
		super(e), this.name = "PngTextureError";
	}
};
function $p(e, t) {
	let n = e.payload;
	if (n === void 0) throw new Qp("texture has no retained payload");
	if (t.byteLength !== n.byteLength) throw new Qp(`encoded byte length ${String(t.byteLength)} does not match ${String(n.byteLength)}`);
	let r = `sha256:${cp(yp(t))}`;
	if (r !== n.contentHash || e.contentHash !== r) throw new Qp(`content hash mismatch: expected ${n.contentHash}, received ${r}`);
	return em(t, e.width, e.height);
}
function em(e, t, n) {
	if (e.byteLength < 45 || [
		137,
		80,
		78,
		71,
		13,
		10,
		26,
		10
	].some((t, n) => e[n] !== t)) throw new Qp("invalid PNG signature or truncated stream");
	let r = new DataView(e.buffer, e.byteOffset, e.byteLength), i = [], a = 8, o = !1, s = !1;
	for (; a < e.byteLength;) {
		if (a + 12 > e.byteLength) throw new Qp("truncated PNG chunk");
		let c = r.getUint32(a, !1), l = a + 4, u = l + 4, d = u + c, f = d + 4;
		if (!Number.isSafeInteger(f) || f > e.byteLength) throw new Qp("PNG chunk exceeds encoded bytes");
		let p = String.fromCharCode(...e.subarray(l, u)), m = r.getUint32(d, !1);
		if (nm(e.subarray(l, d)) !== m) throw new Qp(`PNG ${p} CRC mismatch`);
		if (p === "IHDR") {
			if (o || a !== 8 || c !== 13) throw new Qp("invalid PNG IHDR");
			let i = r.getUint32(u, !1), s = r.getUint32(u + 4, !1);
			if (i !== t || s !== n) throw new Qp("PNG dimensions do not match the descriptor");
			if (e[u + 8] !== 8 || e[u + 9] !== 6 || e[u + 10] !== 0 || e[u + 11] !== 0 || e[u + 12] !== 0) throw new Qp("only non-interlaced RGBA8 PNG is supported");
			o = !0;
		} else if (p === "IDAT") {
			if (!o || s) throw new Qp("PNG IDAT ordering is invalid");
			i.push(e.slice(u, d));
		} else if (p === "IEND") {
			if (!o || i.length === 0 || s || c !== 0 || f !== e.byteLength) throw new Qp("invalid PNG IEND");
			s = !0;
		} else if (e[l] >= 65 && e[l] <= 90) throw new Qp(`unsupported critical PNG chunk ${p}`);
		a = f;
	}
	if (!o || !s || i.length === 0) throw new Qp("incomplete PNG stream");
	let c = i.reduce((e, t) => e + t.byteLength, 0), l = new Uint8Array(c), u = 0;
	for (let e of i) l.set(e, u), u += e.byteLength;
	let d;
	try {
		d = Xp(l);
	} catch (e) {
		throw new Qp(`PNG deflate stream is invalid: ${e instanceof Error ? e.message : String(e)}`);
	}
	let f = t * 4, p = n * (f + 1);
	if (d.byteLength !== p) throw new Qp(`decoded PNG length ${String(d.byteLength)} does not match ${String(p)}`);
	let m = new Uint8Array(t * n * 4);
	for (let e = 0; e < n; e++) {
		let t = e * (f + 1), n = d[t];
		if (n > 4) throw new Qp(`unsupported PNG row filter ${String(n)}`);
		let r = t + 1, i = e * f;
		for (let t = 0; t < f; t++) {
			let a = d[r + t], o = t >= 4 ? m[i + t - 4] : 0, s = e > 0 ? m[i + t - f] : 0, c = e > 0 && t >= 4 ? m[i + t - f - 4] : 0, l = n === 0 ? 0 : n === 1 ? o : n === 2 ? s : n === 3 ? Math.floor((o + s) / 2) : tm(o, s, c);
			m[i + t] = a + l & 255;
		}
	}
	return {
		pixels: m,
		width: t,
		height: n
	};
}
function tm(e, t, n) {
	let r = e + t - n, i = Math.abs(r - e), a = Math.abs(r - t), o = Math.abs(r - n);
	return i <= a && i <= o ? e : a <= o ? t : n;
}
function nm(e) {
	let t = 4294967295;
	for (let n of e) {
		t ^= n;
		for (let e = 0; e < 8; e++) t = t & 1 ? t >>> 1 ^ 3988292384 : t >>> 1;
	}
	return (t ^ 4294967295) >>> 0;
}
//#endregion
//#region packages/renderer-three/dist/voxel-surface-material.js
var rm = class extends Error {
	constructor(e) {
		super(e), this.name = "VoxelSurfaceMaterialError";
	}
};
function im(e, t) {
	let n = e.voxelSurface;
	if (n === void 0) throw new rm(`material ${e.id} has no voxel surface`);
	let r = n.mapping;
	if (e.texture !== r.texture || t.id !== r.texture) throw new rm(`material ${e.id} resolved texture ${r.texture} does not match ${t.id}`);
	if (t.version !== r.textureVersion) throw new rm(`material ${e.id} needs texture ${t.id} version ${String(r.textureVersion)}`);
	if (t.contentHash !== r.textureContentHash) throw new rm(`material ${e.id} needs texture ${t.id} hash ${r.textureContentHash}`);
	if (t.payload === void 0) throw new rm(`material ${e.id} needs retained texture payload ${t.id}`);
	if (t.filter !== n.filter || t.wrap !== n.wrap) throw new rm(`material ${e.id} texture sampling policy does not match ${t.id}`);
	let i = [0, 0], a = [1, 1];
	if (r.kind === "atlas") {
		let [n, o] = r.region.contentMin, [s, c] = r.region.contentExtent;
		if (n + s > t.width || o + c > t.height) throw new rm(`material ${e.id} atlas region ${r.region.id} exceeds ${t.id}`);
		i = [(n + .5) / t.width, (o + .5) / t.height], a = [(n + s - .5) / t.width, (o + c - .5) / t.height];
	}
	return Object.freeze({
		material: e.id,
		texture: t.id,
		mapping: r.kind,
		tileScaleCells: Object.freeze([...r.tileScaleCells]),
		tileOriginCells: Object.freeze([...r.tileOriginCells]),
		sampleUvMin: Object.freeze([...i]),
		sampleUvMax: Object.freeze([...a]),
		alphaMode: n.alphaMode.kind,
		alphaCutoff: n.alphaMode.kind === "mask" ? n.alphaMode.cutoff : null
	});
}
function am(e, t, n) {
	let r = im(t, n);
	return e.userData.rustyVoxelSurface = r, e.customProgramCacheKey = () => [
		"rusty-engine.voxel-surface.v1",
		r.mapping,
		t.voxelSurface.filter,
		r.alphaMode
	].join(":"), e.onBeforeCompile = (e) => {
		e.uniforms.rustyVoxelTileScale = { value: new Yr(...r.tileScaleCells) }, e.uniforms.rustyVoxelTileOrigin = { value: new Yr(...r.tileOriginCells) }, e.uniforms.rustyVoxelUvMin = { value: new Yr(...r.sampleUvMin) }, e.uniforms.rustyVoxelUvMax = { value: new Yr(...r.sampleUvMax) }, e.fragmentShader = e.fragmentShader.replace("#include <map_pars_fragment>", [
			"#include <map_pars_fragment>",
			"#ifdef USE_MAP",
			"uniform vec2 rustyVoxelTileScale;",
			"uniform vec2 rustyVoxelTileOrigin;",
			"uniform vec2 rustyVoxelUvMin;",
			"uniform vec2 rustyVoxelUvMax;",
			"#endif"
		].join("\n")).replace("#include <map_fragment>", [
			"#ifdef USE_MAP",
			"vec2 rustyVoxelRepeat = fract((vMapUv - rustyVoxelTileOrigin) / rustyVoxelTileScale);",
			"vec2 rustyVoxelUv = mix(rustyVoxelUvMin, rustyVoxelUvMax, rustyVoxelRepeat);",
			"vec4 sampledDiffuseColor = texture2D(map, rustyVoxelUv);",
			"#ifdef DECODE_VIDEO_TEXTURE",
			"sampledDiffuseColor = sRGBTransferEOTF(sampledDiffuseColor);",
			"#endif",
			"diffuseColor *= sampledDiffuseColor;",
			"#endif"
		].join("\n"));
	}, om(e, t.voxelSurface), e.needsUpdate = !0, r;
}
function om(e, t) {
	switch (t.alphaMode.kind) {
		case "opaque":
			e.alphaTest = 0, e.transparent = !1, e.depthWrite = !0;
			break;
		case "mask":
			e.alphaTest = t.alphaMode.cutoff, e.transparent = !1, e.depthWrite = !0;
			break;
		case "blend":
			e.alphaTest = 0, e.transparent = !0, e.depthWrite = !1;
			break;
	}
}
//#endregion
//#region packages/renderer-three/dist/three-renderer.js
var $ = class extends Error {
	constructor(e) {
		super(e), this.name = "RenderApplyError";
	}
}, sm = class extends Error {
	code;
	resource;
	constructor(e, t, n) {
		super(n), this.code = e, this.resource = t, this.name = "RenderResourceError";
	}
};
function cm(e, t, n) {
	let r = e.count - (t === void 0 ? 0 : 1) + (n === void 0 ? 0 : 1), i = e.encodedBytes - (t?.encodedBytes ?? 0) + (n?.encodedBytes ?? 0), a = e.decodedBytes - (t?.decodedBytes ?? 0) + (n?.decodedBytes ?? 0);
	if (![
		r,
		i,
		a
	].every(Number.isSafeInteger) || r < 0 || i < 0 || a < 0) throw new $("defineTexture: texture resource budget arithmetic is invalid");
	if (r > 256) throw new $("defineTexture: retained texture quota exceeded");
	if (i > 134217728) throw new $("defineTexture: aggregate encoded texture byte quota exceeded");
	if (a > 268435456) throw new $("defineTexture: aggregate decoded texture byte quota exceeded");
	return {
		count: r,
		encodedBytes: i,
		decodedBytes: a
	};
}
var lm = 31, um = 4096, dm = 2, fm = class {
	scene = new ea();
	viewmodelScene = new ea();
	#e = new Ki();
	#t = new Ki();
	#n = new Ki();
	#r = new Ki();
	#i = /* @__PURE__ */ new Map();
	#a = /* @__PURE__ */ new Set();
	#o = /* @__PURE__ */ new Map();
	#s = /* @__PURE__ */ new Map();
	#c = /* @__PURE__ */ new Map();
	#l = /* @__PURE__ */ new Map();
	#u = 0;
	#d = /* @__PURE__ */ new Set();
	#f = /* @__PURE__ */ new Map();
	#p = /* @__PURE__ */ new Map();
	#m = 0;
	#h;
	#g;
	#_;
	#v;
	#y;
	#b;
	#x;
	#S = new tt();
	#C = /* @__PURE__ */ new Set();
	#w = /* @__PURE__ */ new Set();
	#T = /* @__PURE__ */ new Map();
	#E = /* @__PURE__ */ new Set();
	#D = /* @__PURE__ */ new Map();
	#O = /* @__PURE__ */ new Map();
	#k = /* @__PURE__ */ new Map();
	#A = /* @__PURE__ */ new WeakSet();
	#j = !1;
	constructor(e = {}) {
		if (this.#h = e.meshBufferSource, this.#g = e.meshResourceSource, this.#_ = e.textureResourceSource, this.#v = e.animatedMeshSource, this.#y = new vf(this.#v), this.#b = e.shadowsEnabled ?? !1, this.#x = e.maximumActiveShadowLights ?? 8, !Number.isSafeInteger(this.#x) || this.#x < 0 || this.#x > 8) throw new Hf("invalid_shadow_limit", "maximumActiveShadowLights must be an integer in 0..=8");
		this.#e.name = "scene", this.#t.name = "debug", this.#n.name = "ui", this.#r.name = "viewmodel", this.viewmodelScene.name = "viewmodel", this.scene.add(this.#e, this.#t, this.#n), this.viewmodelScene.add(this.#r);
	}
	#M(e) {
		switch (e) {
			case "scene": return this.#e;
			case "debug": return this.#t;
			case "ui": return this.#n;
			case "viewmodel": return this.#r;
		}
	}
	applyFrame(e) {
		if (this.#j) throw new $("renderer is disposed");
		try {
			let t = this.#S.validateFrame(e);
			this.#N(t);
		} catch (e) {
			throw e instanceof H ? new $(e.message) : e;
		}
		let t = this.#F(e), n = this.#oe(e), r = /* @__PURE__ */ new Set(), i = /* @__PURE__ */ new Set(), a = /* @__PURE__ */ new Set(), o = /* @__PURE__ */ new Set();
		try {
			for (let n = 0; n < e.ops.length; n += 1) {
				let s = e.ops[n];
				if (s.op === "destroy") {
					if (!this.#i.has(s.handle) && r.has(s.handle)) continue;
					this.#W(s, r);
				} else this.#P(s, t.geometries.get(n), t.textures.get(n), i, a, o), t.geometries.delete(n), t.textures.delete(n);
			}
			for (let e of this.#l.values()) e.texture !== null && a.has(e.texture) && i.add(e.id);
			for (let e of [...i].sort()) this.#fe(e);
			this.#xe(a, o);
		} catch (e) {
			throw ih(t), e;
		}
		ih(t), this.#S.applyFrame(e), n && this.#ie(), this.#b && this.#e.traverse((e) => {
			e instanceof vo && (e.castShadow = !0, e.receiveShadow = !0);
		});
	}
	#N(e) {
		if (!this.#b) return;
		let t = new Set(this.#S.snapshot().lights.filter(({ light: e }) => ah(e)).map(({ handle: e }) => e));
		for (let n of e) if (n.op === "removeLight" ? t.delete(n.handle) : n.op === "upsertLight" && (ah(n.light.light) ? t.add(n.light.handle) : t.delete(n.light.handle)), t.size > this.#x) throw new Hf("shadow_budget_exceeded", `active shadow light quota ${String(this.#x)} exceeded`);
	}
	applyEncodedFrame(e) {
		this.applyFrame(a(e));
	}
	applyDiff(e) {
		this.applyFrame({
			schemaVersion: 1,
			ops: [e]
		});
	}
	#P(e, t, n, r, i, a) {
		switch (e.op) {
			case "create":
				this.#V(e);
				break;
			case "update":
				this.#U(e);
				break;
			case "destroy":
				this.#W(e);
				break;
			case "replaceMeshPayload":
				this.#Se(e, t?.[0]);
				break;
			case "createLight":
				this.#Te(e);
				break;
			case "updateLight":
				this.#Ee(e);
				break;
			case "defineMaterial":
				this.#ue(e.material, r);
				break;
			case "setMaterialInstanceParameters":
				this.#pe(e);
				break;
			case "defineTexture":
				this.#de(e.texture, n, i);
				break;
			case "defineSpriteAtlas":
				this.#p.set(e.atlas.id, e.atlas), a?.add(e.atlas.id);
				break;
			case "defineStaticMesh":
				this.#G(e.asset, t?.[0]);
				break;
			case "defineAnimatedMesh":
				this.#J(e);
				break;
			case "createAnimatedMeshInstance":
				this.#Y(e);
				break;
			case "setAnimatedMeshPlayback":
				this.#X(e);
				break;
			case "defineVoxelObject":
				this.#Q(e.asset, t);
				break;
			case "releaseVoxelObject":
				this.#$(e.asset);
				break;
			case "createVoxelObjectInstance":
				this.#ee(e);
				break;
			case "setVoxelObjectFrame":
				this.#te(e);
				break;
			case "createStaticMeshInstance":
				this.#K(e);
				break;
			case "createSprite":
				this.#ve(e);
				break;
			case "updateSprite":
				this.#ye(e);
				break;
		}
	}
	#F(e) {
		let t = {
			geometries: /* @__PURE__ */ new Map(),
			textures: /* @__PURE__ */ new Map()
		}, n = /* @__PURE__ */ new Map(), r = new Map([...this.#f].map(([e, t]) => [e, t.version])), i = new Map([...this.#f].map(([e, t]) => [e, structuredClone(t)])), a = new Map([...this.#l].map(([e, t]) => [e, structuredClone(t)])), o = new Map([...this.#D].map(([e, t]) => [e, t.readout])), s = {
			count: o.size,
			encodedBytes: [...o.values()].reduce((e, t) => e + t.encodedBytes, 0),
			decodedBytes: [...o.values()].reduce((e, t) => e + t.decodedBytes, 0)
		};
		try {
			for (let c = 0; c < e.ops.length; c += 1) {
				let l = e.ops[c];
				if (l.op === "defineStaticMesh") t.geometries.set(c, [Sm(l.asset.payload, l.asset.materialSlots, this.#h, this.#g, "defineStaticMesh")]);
				else if (l.op === "replaceMeshPayload") t.geometries.set(c, [Sm(l.payload, void 0, this.#h, this.#g, "replaceMeshPayload")]);
				else if (l.op === "defineVoxelObject") t.geometries.set(c, xm(l.asset, this.#h, this.#g));
				else if (l.op === "defineTexture") {
					let e = r.get(l.texture.id);
					if (e !== void 0 && l.texture.version <= e) throw new $(`defineTexture: stale or duplicate version ${String(l.texture.version)} for ${l.texture.id}`);
					let n = o.get(l.texture.id), a = l.texture.payload;
					if (a === void 0) s = cm(s, n, void 0), o.delete(l.texture.id), t.textures.set(c, null);
					else {
						let e = l.texture.width * l.texture.height * 4, r = {
							encodedBytes: a.byteLength,
							decodedBytes: e
						};
						s = cm(s, n, r);
						let i = vm(l.texture, this.#_, "defineTexture");
						o.set(l.texture.id, i.readout), t.textures.set(c, i);
					}
					r.set(l.texture.id, l.texture.version), i.set(l.texture.id, structuredClone(l.texture));
				} else if (l.op === "defineMaterial") a.set(l.material.id, structuredClone(l.material));
				else if (l.op === "defineAnimatedMesh") this.#y.validateDefinition(l.asset);
				else if (l.op === "createAnimatedMeshInstance" && l.instance.materialOverrides.length > 0) throw new $(`createAnimatedMeshInstance: material overrides are not implemented for animated mesh ${l.instance.asset}`);
				else if (l.op === "createAnimatedMeshInstance") {
					let e = l.instance.playback;
					if (e?.kind === "pause" || e?.kind === "resume") throw new $(`createAnimatedMeshInstance.${e.kind}: no current clip on ${l.instance.asset}`);
					n.set(l.handle, e?.kind === "play" ? e.clip : null);
				} else if (l.op === "setAnimatedMeshPlayback") {
					let e = n.has(l.handle) ? n.get(l.handle) ?? null : this.#y.playback(l.handle)?.currentClip ?? null;
					if ((l.playback.kind === "pause" || l.playback.kind === "resume") && e === null) throw new $(`setAnimatedMeshPlayback.${l.playback.kind}: no current clip`);
					l.playback.kind === "play" ? n.set(l.handle, l.playback.clip) : l.playback.kind === "stop" && n.set(l.handle, null);
				}
			}
			for (let e of a.values()) {
				if (e.schemaVersion >= 3 && e.texture !== null && !o.has(e.texture)) throw new $(`defineMaterial: texture ${e.texture} has no admitted retained payload`);
				if (e.voxelSurface !== void 0) {
					let t = i.get(e.voxelSurface.mapping.texture);
					if (t === void 0) throw new $(`defineMaterial: missing voxel surface texture ${e.voxelSurface.mapping.texture}`);
					try {
						im(e, t);
					} catch (e) {
						throw e instanceof rm ? new $(`defineMaterial: ${e.message}`) : e;
					}
				}
			}
			return t;
		} catch (e) {
			throw ih(t), nh(e);
		}
	}
	registerSlotColor(e, t, n, r) {
		this.#c.set(e, new X(t, n, r));
	}
	#I(e) {
		let t = this.#c.get(e);
		if (t) return t.clone();
		let n = e * .61803398875 % 1;
		return new X().setHSL(n, .7, .5);
	}
	has(e) {
		return this.#i.has(e);
	}
	get handleCount() {
		return this.#i.size;
	}
	resourceStatistics() {
		return Object.freeze({
			renderHandleCount: this.#i.size,
			geometryResourceCount: this.#C.size,
			materialResourceCount: this.#w.size,
			textureResourceCount: this.#E.size,
			animatedInstanceCount: this.#y.instanceCount
		});
	}
	#L(e) {
		e.traverse((e) => {
			let t = e;
			t.geometry instanceof Ja && this.#R(t.geometry), Array.isArray(t.material) ? t.material.forEach((e) => this.#z(e)) : t.material instanceof Xa && this.#z(t.material);
		});
	}
	#R(e) {
		this.#C.has(e) || (this.#C.add(e), e.addEventListener("dispose", () => this.#C.delete(e)));
	}
	#z(e) {
		if (this.#w.has(e)) return;
		this.#w.add(e);
		let t = eh(e);
		for (let e of t) {
			let t = this.#T.get(e) ?? 0;
			this.#B(e), this.#T.set(e, t + 1);
		}
		e.addEventListener("dispose", () => {
			if (this.#w.delete(e)) for (let e of t) {
				let t = this.#T.get(e);
				t === void 0 || t <= 1 ? this.#T.delete(e) : this.#T.set(e, t - 1);
			}
		});
	}
	#B(e) {
		this.#E.has(e) || (this.#E.add(e), e.addEventListener("dispose", () => {
			this.#E.delete(e), this.#T.delete(e);
		}));
	}
	lightReadout() {
		return [...this.#i.entries()].filter((e) => e[1].kind === "light" && e[1].light !== void 0).sort(([e], [t]) => e - t).map(([e, t]) => ({
			descriptor: structuredClone(t.light),
			handle: e,
			parent: Kf(t.object.parent, this.#i),
			shadowStatus: Gf(t.light, this.#b)
		}));
	}
	meshPresentationReadout() {
		return [...this.#i.entries()].filter(([, e]) => e.meshProvenance !== void 0).sort(([e], [t]) => e - t).map(([e, t]) => ({
			handle: e,
			lit: Qf(t.object).every((e) => e instanceof Ns),
			materialSlots: [...t.meshMaterialSlots ?? []],
			opacity: t.viewMaterial?.color[3] ?? 1,
			wireframe: t.viewMaterial?.wireframe ?? !1
		}));
	}
	dispose() {
		if (this.#j) return;
		this.#le();
		let e = [...this.#i.entries()].sort((e, t) => th(t[1].object) - th(e[1].object)).map(([e]) => e);
		for (let t of e) this.#i.has(t) && this.#W({
			op: "destroy",
			handle: t
		});
		this.#a.clear();
		for (let e of this.#o.values()) e.geometry.dispose(), e.materials.forEach((e) => e.dispose());
		this.#o.clear();
		for (let e of this.#s.values()) e.geometries.forEach((e) => e.dispose()), e.materials.forEach((e) => e.dispose());
		this.#s.clear(), this.#y.dispose(), this.#c.clear(), this.#l.clear(), this.#d.clear();
		for (let e of this.#D.values()) e.texture.dispose();
		this.#D.clear(), this.#f.clear(), this.#p.clear(), this.scene.clear(), this.viewmodelScene.clear(), this.#C.clear(), this.#w.clear(), this.#T.clear(), this.#E.clear(), this.#j = !0;
	}
	objectFor(e) {
		return this.#i.get(e)?.object;
	}
	projectionIdentityForObject(e, t) {
		if (e instanceof Vo && t !== void 0) {
			let n = this.#k.get(e)?.handles[t], r = n === void 0 ? void 0 : this.#i.get(n);
			if (n !== void 0 && r !== void 0) return {
				handle: n,
				layer: this.#H(r.object),
				metadata: Zm(r.object)
			};
		}
		let n = e;
		for (; n !== null;) {
			for (let [e, t] of this.#i.entries()) if (t.object === n) return {
				handle: e,
				layer: this.#H(t.object),
				metadata: Zm(t.object)
			};
			n = n.parent;
		}
	}
	projectionWorldNormalForObject(e, t, n) {
		if (e instanceof Vo && t !== void 0 && this.#k.has(e)) {
			let r = new vi();
			return e.getMatrixAt(t, r), r.premultiply(e.matrixWorld), n.clone().applyNormalMatrix(new J().getNormalMatrix(r));
		}
		return n.clone().transformDirection(e.matrixWorld);
	}
	prepareStaticInstanceBatches(e) {
		if (this.#j) throw new $("renderer is disposed");
		this.scene.updateMatrixWorld(!0), e.updateMatrixWorld(!0);
		let t = new vi().multiplyMatrices(e.projectionMatrix, e.matrixWorldInverse), n = new Yo().setFromProjectionMatrix(t);
		for (let e of this.#O.values()) {
			let t = e.candidateHandles.filter((e) => {
				let t = this.#i.get(e);
				return t !== void 0 && t.object instanceof vo && n.intersectsObject(t.object);
			});
			this.#ae(e, t);
		}
	}
	visibilityReadout(e, t = this.scene) {
		if (this.#j) throw new $("renderer is disposed");
		e.updateMatrixWorld(!0), t.updateMatrixWorld(!0), this.prepareSpritesForCamera(e, t);
		let n = new vi().multiplyMatrices(e.projectionMatrix, e.matrixWorldInverse), r = new Yo().setFromProjectionMatrix(n), i = [...this.#i.entries()].filter(([, e]) => pm(e.object, t)).sort(([e], [t]) => e - t).map(([e, n]) => {
			let i = Wm(n.object, t), a = Gm(n), o = a && Km(r, n.object);
			return Object.freeze({
				handle: e,
				state: a ? i ? o ? "frustumVisible" : "outsideFrustum" : "hidden" : "notDrawable",
				inFrustum: o,
				effectivelyVisible: i,
				occlusion: "notMeasured"
			});
		});
		return Object.freeze({
			schemaVersion: 1,
			basis: "cpuFrustum",
			occlusion: "notMeasured",
			handles: Object.freeze(i)
		});
	}
	prepareSpritesForCamera(e, t = this.scene) {
		if (this.#j) throw new $("renderer is disposed");
		e.updateMatrixWorld(!0), t.updateMatrixWorld(!0);
		let n = new q().setFromMatrixPosition(e.matrixWorld), r = e.getWorldQuaternion(new Xr()), i = new q(), a = new q(), o = new Xr(), s = new Xr(), c = new Xr(), l = new Xr(), u = new q(), d = new q(), f = new q(0, 1, 0), p = new vi(), m = [...this.#a].map((e) => this.#i.get(e)).filter((e) => e !== void 0 && e.kind === "sprite" && e.sprite !== void 0 && pm(e.object, t)).sort((e, t) => th(e.object) - th(t.object));
		for (let e of m) {
			let t = e.sprite;
			t !== void 0 && e.object.quaternion.set(...t.transform.rotation);
		}
		t.updateMatrixWorld(!0);
		for (let t of m) {
			let m = t.sprite;
			if (m === void 0 || m.billboard === "none") continue;
			let h = t.object;
			h.updateMatrixWorld(!0), h.getWorldPosition(a), m.billboard === "spherical" ? o.copy(r) : (e instanceof Sc ? (e.getWorldDirection(i), u.copy(i).negate()) : u.subVectors(n, a), u.y = 0, u.lengthSq() <= 2 ** -52 && (h.getWorldQuaternion(s), u.set(0, 0, 1).applyQuaternion(s), u.y = 0, u.lengthSq() <= 2 ** -52 && u.set(0, 0, 1)), u.normalize(), d.crossVectors(f, u).normalize(), p.makeBasis(d, f, u), o.setFromRotationMatrix(p).normalize()), h.parent === null ? h.quaternion.copy(o) : (h.parent.getWorldQuaternion(c), l.copy(c).invert().multiply(o).normalize(), h.quaternion.copy(l)), h.updateMatrixWorld(!0);
		}
		t.updateMatrixWorld(!0);
	}
	prepareStaticInstanceBatchesForPicking() {
		if (this.#j) throw new $("renderer is disposed");
		this.scene.updateMatrixWorld(!0);
		for (let e of this.#O.values()) this.#ae(e, e.candidateHandles);
	}
	advanceAnimation(e) {
		try {
			this.#y.advance(e);
		} catch (e) {
			throw nh(e);
		}
		for (let [e, t] of this.#i.entries()) t.kind === "animatedMesh" && this.#Z(e, t);
	}
	animatedMeshPlayback(e) {
		return this.#y.playback(e);
	}
	sampleAnimatedMesh(e, t, n) {
		try {
			let r = this.#y.sample(e, t, n);
			return this.#Z(e, this.#De(e, "sampleAnimatedMesh")), r;
		} catch (e) {
			throw nh(e);
		}
	}
	setAnimationControllerWeights(e, t) {
		try {
			this.#y.setControllerWeights(e, t), this.#Z(e, this.#De(e, "setAnimationControllerWeights"));
		} catch (e) {
			throw nh(e);
		}
	}
	hasAnimationControllerClips(e, t) {
		return this.#y.hasClips(e, t);
	}
	clearAnimationControllerWeights(e) {
		try {
			this.#y.clearControllerWeights(e), this.#Z(e, this.#De(e, "clearAnimationControllerWeights"));
		} catch (e) {
			throw nh(e);
		}
	}
	snapshot() {
		let e = [...this.#i.entries()].sort((e, t) => e[0] - t[0]);
		return e.length === 0 ? "(empty scene)\n" : e.map(([e, t]) => mm(e, t, this.#H(t.object))).join("\n") + "\n";
	}
	#V(e) {
		if (this.#i.has(e.handle)) throw new $(`create: handle ${e.handle} already exists`);
		let t = ym(e.node);
		this.#L(t), (e.parent === null ? this.#M(e.node.layer) : this.#De(e.parent, "create.parent").object).add(t), this.#i.set(e.handle, {
			object: t,
			kind: "primitive",
			shape: e.node.geometry.kind,
			ownsGeometry: e.node.geometry.kind !== "group",
			viewMaterial: e.node.material
		});
	}
	#H(e) {
		return pm(e, this.#r) ? "viewmodel" : pm(e, this.#t) ? "debug" : pm(e, this.#n) ? "ui" : "scene";
	}
	#U(e) {
		let t = this.#De(e.handle, "update");
		e.transform && Ym(t.object, e.transform), e.material && (t.meshProvenance === void 0 ? Qm(t, e.material) : this.#we(t, e.material), t.viewMaterial = e.material, this.#L(t.object)), e.visible !== null && (t.object.visible = e.visible), e.metadata && Xm(t.object, e.metadata);
	}
	#W(e, t) {
		let n = this.#De(e.handle, "destroy"), r = [...this.#i.entries()].filter(([, e]) => e.object.parent === n.object).map(([e]) => e).sort((e, t) => e - t);
		for (let e of r) this.#W({
			op: "destroy",
			handle: e
		}, t);
		if (n.object.parent?.remove(n.object), n.kind === "staticMesh" && n.asset !== void 0) gm(n), this.#q(n.asset);
		else if (n.kind === "animatedMesh") this.#y.release(e.handle);
		else if (n.kind === "voxelObject" && n.asset !== void 0) {
			gm(n);
			let e = this.#s.get(n.asset);
			e !== void 0 && --e.refCount;
		} else n.kind === "light" ? qf(n.object) : $m(n.object);
		this.#i.delete(e.handle), this.#a.delete(e.handle), t?.add(e.handle);
	}
	#G(e, t) {
		let n = this.#o.get(e.asset);
		if (n) {
			if (n.refCount > 0) throw new $(`defineStaticMesh: asset ${e.asset} is in use by ${n.refCount} instance(s)`);
			n.geometry.dispose(), n.materials.forEach((e) => e.dispose());
		}
		let r = t ?? Sm(e.payload, e.materialSlots, this.#h, this.#g, "defineStaticMesh");
		this.#R(r);
		let i = /* @__PURE__ */ new Map(), a = e.materialSlots.map((e, t) => (i.set(e.slot, t), this.#me(e)));
		this.#o.set(e.asset, {
			geometry: r,
			materials: a,
			slotIndex: i,
			materialSlots: e.materialSlots,
			collision: e.collision,
			refCount: 0
		});
	}
	#K(e) {
		if (this.#i.has(e.handle)) throw new $(`createStaticMeshInstance: handle ${e.handle} already exists`);
		let t = this.#o.get(e.instance.asset);
		if (!t) throw new $(`createStaticMeshInstance: undefined static mesh asset ${e.instance.asset}`);
		let n = t.materials.slice(), r = t.materialSlots.map((e) => e.material), i = /* @__PURE__ */ new Set();
		for (let a of e.instance.materialOverrides) {
			let o = t.slotIndex.get(a.slot);
			if (o === void 0) throw new $(`createStaticMeshInstance: override for unbound slot ${a.slot} on ${e.instance.asset}`);
			n[o] = this.#me(a), r[o] = a.material, i.add(o);
		}
		let a = new vo(t.geometry, n.length === 1 ? n[0] : n);
		this.#A.add(a), Ym(a, e.instance.transform), Xm(a, e.instance.metadata), a.visible = e.instance.visible, (e.parent === null ? this.#e : this.#De(e.parent, "createStaticMeshInstance.parent").object).add(a), t.refCount += 1, this.#i.set(e.handle, {
			object: a,
			kind: "staticMesh",
			shape: "quad",
			asset: e.instance.asset,
			ownsGeometry: !1,
			materialIds: r,
			ownedMaterialIndices: i,
			materialParameterOverrides: /* @__PURE__ */ new Map()
		});
	}
	#q(e) {
		let t = this.#o.get(e);
		t && --t.refCount;
	}
	#J(e) {
		try {
			this.#y.define(e.asset);
		} catch (e) {
			throw nh(e);
		}
	}
	#Y(e) {
		if (this.#i.has(e.handle)) throw new $(`createAnimatedMeshInstance: handle ${e.handle} already exists`);
		let t;
		try {
			t = this.#y.create(e.handle, e.instance);
		} catch (e) {
			throw nh(e);
		}
		Ym(t.object, e.instance.transform), Xm(t.object, e.instance.metadata), t.object.visible = e.instance.visible, this.#L(t.object), (e.parent === null ? this.#e : this.#De(e.parent, "createAnimatedMeshInstance.parent").object).add(t.object), this.#i.set(e.handle, {
			object: t.object,
			kind: "animatedMesh",
			shape: "quad",
			asset: e.instance.asset,
			ownsGeometry: !1
		}), this.#Z(e.handle, this.#De(e.handle, "createAnimatedMeshInstance"));
	}
	#X(e) {
		let t = this.#De(e.handle, "setAnimatedMeshPlayback");
		try {
			this.#y.setPlayback(e.handle, e.playback);
		} catch (e) {
			throw nh(e);
		}
		this.#Z(e.handle, t);
	}
	#Z(e, t) {
		t.object.userData.animatedMeshPlayback = this.#y.playback(e);
	}
	#Q(e, t) {
		let n = t === void 0 ? xm(e, this.#h, this.#g) : [...t];
		if (n.length !== e.meshes.length) throw new $(`defineVoxelObject: prepared ${n.length} meshes for ${e.meshes.length} descriptors`);
		let r = /* @__PURE__ */ new Map(), i = e.materialSlots.map((e, t) => (r.set(e.slot, t), this.#me(e)));
		n.forEach((e) => this.#R(e));
		let a = this.#s.get(e.asset), o = {
			geometries: n,
			frames: e.frames,
			meshMaterialSlots: e.meshes.map((e) => e.payload.groups.map((e) => e.materialSlot)),
			materials: i,
			slotIndex: r,
			materialSlots: e.materialSlots,
			refCount: a?.refCount ?? 0
		};
		if (a !== void 0) {
			for (let t of this.#i.values()) {
				if (t.kind !== "voxelObject" || t.asset !== e.asset) continue;
				let r = t.voxelFrame ?? 0, a = o.frames[r], s = a === void 0 ? void 0 : o.geometries[a.mesh];
				if (a === void 0 || s === void 0) throw n.forEach((e) => e.dispose()), i.forEach((e) => e.dispose()), new $(`defineVoxelObject: live frame ${r} is unavailable on ${e.asset}`);
				let c = this.#re(o, t.voxelMaterialOverrides ?? []);
				gm(t);
				let l = t.object;
				l.geometry = s, l.material = c.materials.length === 1 ? c.materials[0] : c.materials, t.materialIds = c.materialIds, t.ownedMaterialIndices = c.ownedMaterialIndices, t.meshMaterialSlots = e.meshes[a.mesh].payload.groups.map((e) => e.materialSlot);
			}
			a.geometries.forEach((e) => e.dispose()), a.materials.forEach((e) => e.dispose());
		}
		this.#s.set(e.asset, o);
	}
	#$(e) {
		let t = this.#s.get(e);
		if (t === void 0) throw new $(`releaseVoxelObject: undefined voxel object ${e}`);
		if (t.refCount !== 0) throw new $(`releaseVoxelObject: ${e} is in use by ${t.refCount} instance(s)`);
		t.geometries.forEach((e) => e.dispose()), t.materials.forEach((e) => e.dispose()), this.#s.delete(e);
	}
	#ee(e) {
		if (this.#i.has(e.handle)) throw new $(`createVoxelObjectInstance: handle ${e.handle} already exists`);
		let t = this.#s.get(e.instance.asset);
		if (t === void 0) throw new $(`createVoxelObjectInstance: undefined voxel object ${e.instance.asset}`);
		let n = t.frames[e.instance.frame], r = n === void 0 ? void 0 : t.geometries[n.mesh];
		if (r === void 0) throw new $(`createVoxelObjectInstance: frame ${e.instance.frame} unavailable on ${e.instance.asset}`);
		let i = this.#re(t, e.instance.materialOverrides), a = new vo(r, i.materials.length === 1 ? i.materials[0] : i.materials);
		this.#A.add(a), Ym(a, e.instance.transform), Xm(a, e.instance.metadata), a.visible = e.instance.visible, (e.parent === null ? this.#e : this.#De(e.parent, "createVoxelObjectInstance.parent").object).add(a), t.refCount += 1, this.#i.set(e.handle, {
			object: a,
			kind: "voxelObject",
			shape: "quad",
			asset: e.instance.asset,
			ownsGeometry: !1,
			materialIds: i.materialIds,
			ownedMaterialIndices: i.ownedMaterialIndices,
			meshProvenance: "voxelObject",
			meshMaterialSlots: this.#ne(e.instance.asset, e.instance.frame),
			voxelFrame: e.instance.frame,
			voxelMaterialOverrides: structuredClone(e.instance.materialOverrides)
		});
	}
	#te(e) {
		let t = this.#De(e.handle, "setVoxelObjectFrame");
		if (t.kind !== "voxelObject" || t.asset === void 0) throw new $(`setVoxelObjectFrame: handle ${e.handle} is not a voxel object`);
		let n = this.#s.get(t.asset), r = n?.frames[e.frame], i = r === void 0 ? void 0 : n?.geometries[r.mesh];
		if (n === void 0 || r === void 0 || i === void 0) throw new $(`setVoxelObjectFrame: frame ${e.frame} unavailable on ${t.asset}`);
		t.object.geometry = i, t.voxelFrame = e.frame, t.meshMaterialSlots = this.#ne(t.asset, e.frame), t.object.userData.voxelObjectFrame = e.frame;
	}
	#ne(e, t) {
		let n = this.#s.get(e), r = n?.frames[t];
		return n === void 0 || r === void 0 ? [] : [...n.meshMaterialSlots[r.mesh] ?? []];
	}
	#re(e, t) {
		let n = e.materials.slice(), r = e.materialSlots.map((e) => e.material), i = /* @__PURE__ */ new Set();
		for (let a of t) {
			let t = e.slotIndex.get(a.slot);
			if (t === void 0) throw new $(`voxel object material override uses unbound slot ${a.slot}`);
			n[t] = this.#me(a), r[t] = a.material, i.add(t);
		}
		return {
			materials: n,
			materialIds: r,
			ownedMaterialIndices: i
		};
	}
	voxelObjectFrame(e) {
		let t = this.#i.get(e);
		if (t?.kind !== "voxelObject" || t.asset === void 0 || t.voxelFrame === void 0) return;
		let n = this.#s.get(t.asset)?.frames[t.voxelFrame];
		if (n !== void 0) return {
			handle: e,
			asset: t.asset,
			frame: t.voxelFrame,
			frameId: n.id,
			mesh: n.mesh
		};
	}
	instanceCountFor(e) {
		return this.#o.get(e)?.refCount ?? 0;
	}
	#ie() {
		let e = /* @__PURE__ */ new Map();
		for (let e of this.#i.values()) (e.kind === "staticMesh" || e.kind === "voxelObject") && e.object instanceof vo && e.object.layers.set(0);
		this.scene.updateMatrixWorld(!0);
		let t = [...this.#i.entries()].sort(([e], [t]) => e - t);
		for (let [n, r] of t) {
			if (r.kind !== "staticMesh" && r.kind !== "voxelObject" || !(r.object instanceof vo) || r.object instanceof Vo || this.#H(r.object) !== "scene" || !Wm(r.object, this.#e) || r.object.matrixWorld.determinant() <= 0 || !Jm(r.object.matrixWorld) || r.object.customDepthMaterial !== void 0 || r.object.customDistanceMaterial !== void 0 || this.#b && r.object.castShadow) continue;
			let t = Array.isArray(r.object.material) ? r.object.material : [r.object.material];
			if (t.length === 0 || t.some((e) => e.transparent || e.opacity < 1)) continue;
			let i = Um(r.object, t), a = e.get(i) ?? [];
			a.push({
				handle: n,
				mesh: r.object
			}), e.set(i, a);
		}
		let n = /* @__PURE__ */ new Set();
		for (let [t, r] of e.entries()) if (!(r.length < dm)) for (let e = 0; e < r.length; e += um) {
			let i = r.slice(e, e + um);
			if (i.length < dm) continue;
			let a = `${t}|chunk:${String(Math.floor(e / um))}`;
			n.add(a);
			let o = i[0].mesh, s = Array.isArray(o.material) ? o.material : [o.material], c = this.#O.get(a);
			if (c === void 0 || c.mesh.instanceMatrix.count < i.length) {
				c !== void 0 && this.#ce(a, c);
				let e = new Vo(o.geometry, s.length === 1 ? s[0] : s, i.length);
				e.name = `static-instance-batch:${t}`, e.castShadow = o.castShadow, e.receiveShadow = o.receiveShadow, e.renderOrder = o.renderOrder, e.frustumCulled = !0, e.instanceMatrix.setUsage(ur), e.layers.set(0), this.#e.add(e), c = {
					mesh: e,
					candidateHandles: [],
					handles: []
				}, this.#O.set(a, c), this.#k.set(e, c);
			}
			c.candidateHandles = i.map(({ handle: e }) => e), this.#ae(c, c.candidateHandles);
		}
		for (let [e, t] of [...this.#O.entries()]) n.has(e) || this.#ce(e, t);
	}
	#ae(e, t) {
		for (let t of e.candidateHandles) {
			let e = this.#i.get(t);
			e?.object instanceof vo && e.object.layers.set(lm);
		}
		if (t.length < dm) {
			if (e.handles = [], e.mesh.count = 0, e.mesh.visible = !1, t.length === 1) {
				let e = this.#i.get(t[0]);
				e?.object instanceof vo && e.object.layers.set(0);
			}
			return;
		}
		e.handles = [...t], e.mesh.visible = !0, e.mesh.count = t.length;
		for (let n = 0; n < t.length; n += 1) {
			let r = this.#i.get(t[n]);
			if (r === void 0) throw new $(`static instance batch references missing handle ${t[n]}`);
			e.mesh.setMatrixAt(n, r.object.matrixWorld);
		}
		e.mesh.instanceMatrix.needsUpdate = !0, e.mesh.boundingBox = null, e.mesh.boundingSphere = null, e.mesh.computeBoundingBox(), e.mesh.computeBoundingSphere();
	}
	#oe(e) {
		return e.ops.some((e) => {
			switch (e.op) {
				case "defineMaterial":
				case "defineStaticMesh":
				case "defineVoxelObject":
				case "releaseVoxelObject":
				case "createStaticMeshInstance":
				case "createVoxelObjectInstance":
				case "setVoxelObjectFrame":
				case "setMaterialInstanceParameters": return !0;
				case "destroy":
				case "replaceMeshPayload": {
					let t = this.#i.get(e.handle);
					return t !== void 0 && this.#se(t.object);
				}
				case "update": {
					if (e.transform === null && e.material === null && e.visible === null) return !1;
					let t = this.#i.get(e.handle);
					return t !== void 0 && this.#se(t.object);
				}
				default: return !1;
			}
		});
	}
	#se(e) {
		let t = !1;
		return e.traverse((e) => {
			t ||= this.#A.has(e);
		}), t;
	}
	#ce(e, t) {
		t.mesh.parent?.remove(t.mesh), t.mesh.dispose(), this.#k.delete(t.mesh), this.#O.delete(e);
	}
	#le() {
		for (let [e, t] of [...this.#O.entries()]) this.#ce(e, t);
	}
	#ue(e, t) {
		this.#l.set(e.id, e), t === void 0 ? this.#fe(e.id) : t.add(e.id);
	}
	#de(e, t, n) {
		if (e.payload !== void 0 && t === void 0) throw new $(`defineTexture: missing prepared payload for ${e.id}`);
		let r = this.#D.get(e.id);
		if (this.#f.set(e.id, structuredClone(e)), t === null || e.payload === void 0 ? this.#D.delete(e.id) : t !== void 0 && (this.#D.set(e.id, t), this.#B(t.texture)), n === void 0) for (let t of this.#l.values()) t.texture === e.id && this.#fe(t.id);
		else n.add(e.id);
		r?.texture.dispose();
	}
	#fe(e) {
		let t = /* @__PURE__ */ new Set();
		for (let n of this.#o.values()) for (let r = 0; r < n.materialSlots.length; r += 1) {
			let i = n.materialSlots[r];
			i.material === e && (t.add(n.materials[r]), n.materials[r] = this.#me(i));
		}
		for (let n of this.#s.values()) for (let r = 0; r < n.materialSlots.length; r += 1) {
			let i = n.materialSlots[r];
			i.material === e && (t.add(n.materials[r]), n.materials[r] = this.#me(i));
		}
		for (let t of this.#i.values()) {
			if (t.meshMaterialSlots?.some((t) => `voxel-material/${String(t)}` === e)) {
				this.#we(t, t.viewMaterial ?? Zf);
				continue;
			}
			if (t.kind !== "staticMesh" && t.kind !== "voxelObject" || !t.materialIds || t.asset === void 0) continue;
			let n = t.kind === "staticMesh" ? this.#o.get(t.asset) : this.#s.get(t.asset);
			if (n === void 0) continue;
			let r = t.object, i = Array.isArray(r.material) ? r.material : [r.material], a = !1;
			for (let r = 0; r < t.materialIds.length; r += 1) {
				if (t.materialIds[r] !== e) continue;
				t.ownedMaterialIndices?.has(r) && i[r]?.dispose();
				let o = t.kind === "staticMesh" ? t.materialParameterOverrides?.get(r) : void 0, s = n.materialSlots[r], c = o === void 0 && s?.material === e;
				i[r] = c ? n.materials[r] : this.#me({
					slot: s?.slot ?? r,
					material: e
				}, o), c ? t.ownedMaterialIndices?.delete(r) : t.ownedMaterialIndices?.add(r), a = !0;
			}
			a && (r.material = i.length === 1 ? i[0] : i);
		}
		t.forEach((e) => e.dispose());
	}
	#pe(e) {
		let t = this.#De(e.handle, "setMaterialInstanceParameters");
		if (t.kind !== "staticMesh" || t.asset === void 0 || t.materialIds === void 0) throw new $(`setMaterialInstanceParameters: handle ${e.handle} is not a static-mesh instance`);
		let n = this.#o.get(t.asset), r = n?.slotIndex.get(e.slot);
		if (n === void 0 || r === void 0) throw new $(`setMaterialInstanceParameters: unbound slot ${e.slot} on ${t.asset}`);
		let i = t.materialIds[r];
		if (i == null) throw new $(`setMaterialInstanceParameters: slot ${e.slot} on ${t.asset} has no material`);
		let a = t.object, o = Array.isArray(a.material) ? a.material : [a.material];
		t.ownedMaterialIndices?.has(r) && o[r]?.dispose();
		let s = n.materialSlots[r];
		e.parameters === null ? (t.materialParameterOverrides?.delete(r), s.material === i ? (o[r] = n.materials[r], t.ownedMaterialIndices?.delete(r)) : (o[r] = this.#me({
			slot: e.slot,
			material: i
		}), t.ownedMaterialIndices?.add(r))) : (t.materialParameterOverrides?.set(r, e.parameters), o[r] = this.#me({
			slot: e.slot,
			material: i
		}, e.parameters), t.ownedMaterialIndices?.add(r)), a.material = o.length === 1 ? o[0] : o;
	}
	materialDescriptor(e) {
		return this.#l.get(e);
	}
	get fallbackMaterialCount() {
		return this.#u;
	}
	fallbackMaterials() {
		return [...this.#d].sort();
	}
	#me(e, t) {
		let n = this.#l.get(e.material);
		if (n) {
			let e = _m(n, t, n.texture === null ? void 0 : this.#D.get(n.texture)?.texture, n.texture === null ? void 0 : this.#f.get(n.texture));
			return this.#z(e), e;
		}
		this.#u += 1, this.#d.add(e.material);
		let r = new Ns({
			color: this.#I(e.slot),
			roughness: 1,
			metalness: 0
		});
		return this.#z(r), r;
	}
	textureDescriptor(e) {
		let t = this.#f.get(e);
		return t === void 0 ? void 0 : structuredClone(t);
	}
	textureResourceReadout() {
		return Object.freeze([...this.#D.values()].map((e) => Object.freeze({ ...e.readout })).sort((e, t) => e.id.localeCompare(t.id)));
	}
	voxelSurfaceMaterialReadout() {
		return Object.freeze([...this.#w].map((e) => e.userData.rustyVoxelSurface).filter((e) => e !== void 0).map((e) => Object.freeze(structuredClone(e))).sort((e, t) => e.material.localeCompare(t.material)));
	}
	spriteAtlas(e) {
		return this.#p.get(e);
	}
	get spriteFallbackCount() {
		return this.#m;
	}
	#he(e, t, n) {
		let r = this.#p.get(t), i = r?.frames.find((e) => e.frame === n);
		if (!i) return (r !== void 0 || this.#f.size > 0 || n !== 0) && (this.#m += 1), [
			0,
			0,
			1,
			1
		];
		let [a, o] = i.uvMin, [s, c] = i.uvMax, l = e.getAttribute("uv");
		return l.setXY(0, a, c), l.setXY(1, s, c), l.setXY(2, a, o), l.setXY(3, s, o), l.needsUpdate = !0, [
			a,
			o,
			s,
			c
		];
	}
	#ge(e, t, n) {
		return this.#p.get(e)?.frames.find((e) => e.frame === t)?.size ?? n;
	}
	#_e(e, t) {
		let n = this.#ge(e.asset, t, e.size), r = new xs(n[0], n[1]);
		return r.translate((.5 - e.pivot[0]) * n[0], (.5 - e.pivot[1]) * n[1], 0), r;
	}
	#ve(e) {
		if (this.#i.has(e.handle)) throw new $(`createSprite: handle ${e.handle} already exists`);
		let t = e.sprite, n = this.#_e(t, t.frame), r = new vo(n, this.#be(t));
		this.#L(r), r.renderOrder = t.renderOrder, Ym(r, t.transform), Xm(r, t.metadata), r.visible = t.visible, r.userData.frame = t.frame, r.userData.billboard = t.billboard, r.userData.uv = this.#he(n, t.asset, t.frame), (e.parent === null ? this.#e : this.#De(e.parent, "createSprite.parent").object).add(r), this.#i.set(e.handle, {
			object: r,
			kind: "sprite",
			shape: "quad",
			asset: t.asset,
			ownsGeometry: !0,
			sprite: t
		}), t.billboard !== "none" && this.#a.add(e.handle);
	}
	#ye(e) {
		let t = this.#De(e.handle, "updateSprite");
		if (t.kind !== "sprite" || !t.sprite) throw new $(`updateSprite: handle ${e.handle} is not a sprite`);
		let n = t.object, r = n.material;
		if (e.frame !== null) {
			t.sprite = {
				...t.sprite,
				frame: e.frame
			}, n.userData.frame = e.frame;
			let r = n.geometry, i = this.#_e(t.sprite, e.frame);
			n.geometry = i, this.#R(i), n.userData.uv = this.#he(i, t.sprite.asset, e.frame), r.dispose();
		}
		e.tint !== null && (t.sprite = {
			...t.sprite,
			tint: e.tint
		}, r.color.setRGB(e.tint[0], e.tint[1], e.tint[2]), r.opacity = e.tint[3], r.transparent = e.tint[3] < 1 || r.map !== null), e.renderOrder !== null && (t.sprite = {
			...t.sprite,
			renderOrder: e.renderOrder
		}, n.renderOrder = e.renderOrder), e.visible !== null && (n.visible = e.visible, t.sprite = {
			...t.sprite,
			visible: e.visible
		});
	}
	#be(e) {
		let t = this.#p.get(e.asset), n = t === void 0 ? void 0 : this.#D.get(t.texture)?.texture, r = new ao({
			color: new X(e.tint[0], e.tint[1], e.tint[2]),
			map: n ?? null,
			opacity: e.tint[3],
			transparent: e.tint[3] < 1 || n !== void 0,
			depthTest: e.depth !== "depthTestOff",
			depthWrite: e.depth === "default"
		});
		return this.#z(r), r;
	}
	#xe(e, t) {
		if (!(e.size === 0 && t.size === 0)) for (let n of this.#i.values()) {
			if (n.kind !== "sprite" || n.sprite === void 0) continue;
			let r = this.#p.get(n.sprite.asset);
			if (r === void 0 || !t.has(n.sprite.asset) && !e.has(r.texture)) continue;
			let i = n.object, a = i.material;
			i.material = this.#be(n.sprite), t.has(n.sprite.asset) && (i.userData.uv = this.#he(i.geometry, n.sprite.asset, n.sprite.frame)), a.dispose();
		}
	}
	pickSprite(e) {
		let t = this.#i.get(e);
		if (!t || t.kind !== "sprite" || !t.sprite) return;
		let n = t.sprite.attachment;
		return {
			handle: e,
			sourceEntity: n.sourceEntity,
			sourceSceneNode: n.sourceSceneNode,
			asset: t.sprite.asset,
			attachmentPoint: n.attachmentPoint
		};
	}
	#Se(e, t) {
		let n = this.#De(e.handle, "replaceMeshPayload"), r = n.object;
		if (!(r instanceof vo)) throw new $(`replaceMeshPayload: handle ${e.handle} is not a mesh`);
		let i = t ?? Sm(e.payload, void 0, this.#h, this.#g, "replaceMeshPayload");
		this.#R(i);
		let a = n.viewMaterial ?? Zf, o = e.payload.groups.map((e) => this.#Ce(e.materialSlot, a)), s = r.geometry, c = r.material;
		r.geometry = i, r.material = o.length === 1 ? o[0] : o, s.dispose(), Array.isArray(c) ? c.forEach((e) => e.dispose()) : c.dispose(), n.meshProvenance = e.payload.provenance, n.meshMaterialSlots = e.payload.groups.map((e) => e.materialSlot), n.viewMaterial = a;
	}
	#Ce(e, t) {
		let n = this.#l.get(`voxel-material/${String(e)}`);
		if (n !== void 0) {
			let e = _m(n, void 0, n.texture === null ? void 0 : this.#D.get(n.texture)?.texture);
			return e.color.multiply(new X(t.color[0], t.color[1], t.color[2])), e.opacity *= t.color[3], e.transparent = e.opacity < 1, e.wireframe = t.wireframe, this.#z(e), e;
		}
		let r = this.#I(e), i = new Ns({
			color: new X(r.r * t.color[0], r.g * t.color[1], r.b * t.color[2]),
			opacity: t.color[3],
			transparent: t.color[3] < 1,
			wireframe: t.wireframe,
			roughness: 1,
			metalness: 0
		});
		return this.#z(i), i;
	}
	#we(e, t) {
		let n = e.object, r = Qf(n), i = (e.meshMaterialSlots ?? []).map((e) => this.#Ce(e, t));
		n.material = i.length === 1 ? i[0] : i, r.forEach((e) => e.dispose());
	}
	#Te(e) {
		if (this.#i.has(e.handle)) throw new $(`createLight: handle ${e.handle} already exists`);
		Jf(e.light, "createLight.light", (e) => new $(e));
		let t = Uf(e.light, this.#b);
		(e.parent === null ? this.#e : this.#De(e.parent, "createLight.parent").object).add(t), this.#i.set(e.handle, {
			object: t,
			kind: "light",
			shape: "point",
			ownsGeometry: !1,
			light: structuredClone(e.light)
		});
	}
	#Ee(e) {
		let t = this.#De(e.handle, "updateLight");
		if (t.kind !== "light" || t.light === void 0) throw new $(`updateLight: handle ${e.handle} is not a light`);
		if (Jf(e.light, "updateLight.light", (e) => new $(e)), t.light.kind !== e.light.kind) throw new $(`updateLight: handle ${e.handle} cannot change kind from ${t.light.kind} to ${e.light.kind}`);
		Wf(t.object, e.light, this.#b), t.light = structuredClone(e.light);
	}
	pickMesh(e) {
		let t = this.#i.get(e);
		if (!t || t.meshProvenance === void 0) return;
		let n = Zm(t.object);
		return {
			handle: e,
			provenance: t.meshProvenance,
			sourceEntity: n.sourceEntity,
			sourceSceneNode: n.sourceSceneNode
		};
	}
	#De(e, t) {
		let n = this.#i.get(e);
		if (n === void 0) throw new $(`${t}: unknown handle ${e}`);
		return n;
	}
};
function pm(e, t) {
	let n = e.parent;
	for (; n !== null;) {
		if (n === t) return !0;
		n = n.parent;
	}
	return !1;
}
function mm(e, t, n) {
	let r = t.object, i = `handle ${e}  layer ${n}`;
	if (t.kind === "light" && t.light !== void 0) return [
		i,
		`kind light/${t.light.kind}`,
		`enabled ${t.light.enabled}`,
		`intensity ${Bm(t.light.intensity)}`,
		`color ${t.light.color.map(Bm).join(",")}`,
		`shadow ${t.light.shadowIntent}`
	].join("  ");
	if (t.kind === "staticMesh") return [
		i,
		"kind staticMesh",
		`asset ${t.asset}`,
		`pos ${Vm(r.position)}`,
		`scale ${Vm(r.scale)}`,
		`visible ${r.visible}`,
		`materials ${hm(r)}`,
		`label ${JSON.stringify(r.name)}`
	].join("  ");
	if (t.kind === "sprite" && t.sprite) {
		let e = t.sprite, n = e.attachment;
		return [
			i,
			"kind sprite",
			`asset ${e.asset}`,
			`frame ${e.frame}`,
			`uv ${(r.userData.uv ?? [
				0,
				0,
				1,
				1
			]).map(Bm).join(",")}`,
			`pos ${Vm(r.position)}`,
			`size ${Bm(e.size[0])},${Bm(e.size[1])}`,
			`pivot ${Bm(e.pivot[0])},${Bm(e.pivot[1])}`,
			`billboard ${e.billboard}`,
			`tint ${e.tint.map(Bm).join(",")}`,
			`order ${r.renderOrder}`,
			`depth ${e.depth}`,
			`shading ${e.shading}`,
			`visible ${r.visible}`,
			`attach ${n.sourceEntity ?? "-"}/${n.sourceSceneNode ?? "-"}/${n.attachmentPoint ?? "-"}`,
			`label ${JSON.stringify(r.name)}`
		].join("  ");
	}
	if (t.kind === "animatedMesh") {
		let e = r.userData.animatedMeshPlayback ?? null;
		return [
			i,
			"kind animatedMesh",
			`asset ${t.asset}`,
			`clip ${e?.currentClip ?? "-"}`,
			`time ${Bm(e?.actionTimeSeconds ?? 0)}`,
			`pos ${Vm(r.position)}`,
			`scale ${Vm(r.scale)}`,
			`visible ${r.visible}`,
			`label ${JSON.stringify(r.name)}`
		].join("  ");
	}
	return t.kind === "voxelObject" ? [
		i,
		"kind voxelObject",
		`asset ${t.asset}`,
		`frame ${t.voxelFrame ?? 0}`,
		`pos ${Vm(r.position)}`,
		`scale ${Vm(r.scale)}`,
		`visible ${r.visible}`,
		`materials ${hm(r)}`,
		`label ${JSON.stringify(r.name)}`
	].join("  ") : [
		i,
		`shape ${t.shape}`,
		`pos ${Vm(r.position)}`,
		`scale ${Vm(r.scale)}`,
		`visible ${r.visible}`,
		`color ${Hm(r)}`,
		`label ${JSON.stringify(r.name)}`
	].join("  ");
}
function hm(e) {
	let t = e.material;
	return "[" + (Array.isArray(t) ? t : [t]).map((e) => {
		let t = e.color;
		if (!t) return "none";
		let n = `${Bm(t.r)},${Bm(t.g)},${Bm(t.b)}`;
		return !(e instanceof Ns) || e.emissiveIntensity === 0 || e.emissive.r === 0 && e.emissive.g === 0 && e.emissive.b === 0 ? n : `${n}~emit(${`${Bm(e.emissive.r)},${Bm(e.emissive.g)},${Bm(e.emissive.b)}`}*${Bm(e.emissiveIntensity)})`;
	}).join(" ") + "]";
}
function gm(e) {
	let t = e.object, n = Array.isArray(t.material) ? t.material : [t.material];
	e.ownedMaterialIndices?.forEach((e) => n[e]?.dispose());
}
function _m(e, t, n, r) {
	let i = t?.textureTint ?? e.textureTint, a = t?.emissionColor ?? e.emissionColor, o = t?.emissionIntensity ?? e.emissionIntensity, s = new X(e.color[0] * i[0], e.color[1] * i[1], e.color[2] * i[2]), c = e.color[3] * i[3], l = new Ns({
		color: s,
		emissive: new X(a[0], a[1], a[2]),
		emissiveIntensity: o,
		metalness: 0,
		map: n ?? null,
		opacity: c,
		roughness: e.roughness,
		transparent: c < 1
	});
	if (e.voxelSurface !== void 0) {
		if (n === void 0 || r === void 0) throw new $(`material ${e.id} has no realized voxel texture ${e.voxelSurface.mapping.texture}`);
		am(l, e, r);
	}
	return l;
}
function vm(e, t, n) {
	let r = e.payload;
	if (r === void 0) throw new $(`${n}: texture ${e.id} has no retained payload`);
	let i, a;
	if (r.source.kind === "inline") i = Uint8Array.from(r.source.encodedBytes);
	else {
		if (t === void 0) throw new $(`${n}: resource texture needs a texture resource provider (${r.source.resource})`);
		try {
			i = t.acquireResource(r.source.resource, r.contentHash, r.byteLength).bytes.slice(), a = r.source.resource;
		} catch (e) {
			throw Om(e, r.source.resource, n, "unavailable");
		}
	}
	let o;
	try {
		o = $p(e, i);
	} catch (r) {
		if (a !== void 0 && t !== void 0) try {
			t.releaseResource(a);
		} catch {}
		throw r instanceof Qp ? new $(`${n}: texture ${e.id} rejected: ${r.message}`) : r;
	}
	if (a !== void 0 && t !== void 0) try {
		t.releaseResource(a);
	} catch (e) {
		throw Om(e, a, n, "release failed");
	}
	let s = new Mo(o.pixels, o.width, o.height, en, zt);
	return s.colorSpace = ir, s.flipY = !1, s.generateMipmaps = !1, s.magFilter = e.filter === "nearest" ? Nt : It, s.minFilter = e.filter === "nearest" ? Nt : It, s.wrapS = e.wrap === "repeat" ? At : jt, s.wrapT = e.wrap === "repeat" ? At : jt, s.unpackAlignment = 1, s.needsUpdate = !0, {
		texture: s,
		readout: Object.freeze({
			id: e.id,
			resource: r.source.kind === "resource" ? r.source.resource : null,
			contentHash: r.contentHash,
			encodedBytes: r.byteLength,
			decodedBytes: o.pixels.byteLength
		})
	};
}
function ym(e) {
	let t;
	switch (e.geometry.kind) {
		case "group":
			t = new Ki();
			break;
		case "cube":
			t = new vo(new bs(1, 1, 1), bm("cube", e.material));
			break;
		case "sphere":
			t = new vo(new Ss(.5, 8, 8), bm("sphere", e.material));
			break;
		case "quad":
			t = new vo(new xs(1, 1), bm("quad", e.material));
			break;
		case "point":
			t = new ms(Rm(), bm("point", e.material));
			break;
		case "line":
			t = new cs(zm(e.geometry.a, e.geometry.b), bm("line", e.material));
			break;
		default: {
			let t = e.geometry;
			throw new $(`unhandled geometry ${JSON.stringify(t)}`);
		}
	}
	return Ym(t, e.transform), t.visible = e.visible, Xm(t, e.metadata), t;
}
function bm(e, t) {
	let n = new X(t.color[0], t.color[1], t.color[2]), r = t.color[3], i = r < 1;
	switch (e) {
		case "point": return new ls({
			color: n,
			opacity: r,
			transparent: i,
			size: .1
		});
		case "line": return new Xo({
			color: n,
			opacity: r,
			transparent: i
		});
		default: return new ao({
			color: n,
			opacity: r,
			transparent: i,
			wireframe: t.wireframe
		});
	}
}
function xm(e, t, n) {
	let r = [], i = new Map(e.materialSlots.map((e, t) => [e.slot, t]));
	try {
		return e.meshes.forEach((e, a) => {
			let o = Sm(e.payload, void 0, t, n, `defineVoxelObject.meshes[${String(a)}]`);
			o.clearGroups(), e.payload.groups.forEach((e) => {
				let t = i.get(e.materialSlot);
				if (t === void 0) throw o.dispose(), new $(`defineVoxelObject.meshes[${String(a)}]: unbound material slot ${e.materialSlot}`);
				o.addGroup(e.start, e.count, t);
			}), r.push(o);
		}), r;
	} catch (e) {
		throw r.forEach((e) => e.dispose()), e;
	}
}
function Sm(e, t, n, r, i) {
	let a = e.source.kind === "inline" ? Cm(e.source) : e.source.kind === "sharedBuffer" ? wm(e, e.source, n, i) : Tm(e, e.source, r, i), o = Pm(e, "position"), s = Pm(e, "normal"), c = new Ja();
	c.setAttribute("position", new Na(a.positions, o)), c.setAttribute("normal", new Na(a.normals, s)), a.uvs !== void 0 && c.setAttribute("uv", new Na(a.uvs, 2)), c.setIndex(new Na(a.indices, 1));
	let l = t === void 0 ? void 0 : new Map(t.map((e, t) => [e.slot, t]));
	for (let t = 0; t < e.groups.length; t += 1) {
		let n = e.groups[t], r = l?.get(n.materialSlot) ?? (l === void 0 ? t : void 0);
		if (r === void 0) throw c.dispose(), new $(`${i}: unbound material slot ${n.materialSlot}`);
		c.addGroup(n.start, n.count, r);
	}
	return c.boundingBox = new ha(new q(e.bounds.min[0], e.bounds.min[1], e.bounds.min[2]), new q(e.bounds.max[0], e.bounds.max[1], e.bounds.max[2])), c;
}
function Cm(e) {
	return {
		positions: new Float32Array(e.positions),
		normals: new Float32Array(e.normals),
		uvs: e.uvs === void 0 ? void 0 : new Float32Array(e.uvs),
		indices: new Uint32Array(e.indices)
	};
}
function wm(e, t, n, r) {
	if (n === void 0) throw new $(`${r}: shared-buffer payload needs a mesh buffer provider (buffer ${t.buffer})`);
	let i = t.buffer, a;
	try {
		a = n.acquireBuffer(i);
	} catch (e) {
		throw jm(e, t.buffer, r, "unavailable");
	}
	let o;
	try {
		o = km(a, e, t, r);
	} catch (e) {
		throw Nm(n, i), e;
	}
	return Mm(n, i, r), o;
}
function Tm(e, t, n, r) {
	if (n === void 0) throw new $(`${r}: resource payload needs a mesh resource provider (${t.resource})`);
	let i;
	try {
		i = n.acquireResource(t.resource, t.contentHash, t.byteLength);
	} catch (e) {
		throw Om(e, t.resource, r, "unavailable");
	}
	let a;
	try {
		Em(i.bytes, t, r), a = Dm(i, e, t, r);
	} catch (e) {
		try {
			n.releaseResource(t.resource);
		} catch {}
		throw e;
	}
	try {
		n.releaseResource(t.resource);
	} catch (e) {
		throw Om(e, t.resource, r, "release failed");
	}
	return a;
}
function Em(e, t, n) {
	let r = t.encoding === "packedStreamsLeV1" ? 49 : 50, i = t.encoding === "packedStreamsLeV1" ? "v1" : "v2", a = [
		82,
		77,
		83,
		72,
		76,
		69,
		48,
		r
	];
	if (e.byteLength !== t.byteLength || a.some((t, n) => e[n] !== t) || e.byteLength < 16) throw new $(`${n}: mesh resource ${t.resource} has an invalid ${i} header`);
	let o = new DataView(e.buffer, e.byteOffset, 16);
	if (o.getUint32(8, !0) !== e.byteLength || o.getUint32(12, !0) === 0) throw new $(`${n}: mesh resource ${t.resource} has an invalid ${i} header`);
}
function Dm(e, t, n, r) {
	let { vertexCount: i, indexCount: a } = t.layout, o = Fm(e, n.positionsByteOffset, i * Pm(t, "position"), "positions", n.resource, r), s = Fm(e, n.normalsByteOffset, i * Pm(t, "normal"), "normals", n.resource, r), c = n.uvsByteOffset === void 0 ? void 0 : Fm(e, n.uvsByteOffset, i * Pm(t, "uv"), "uvs", n.resource, r);
	Am(t, c, n.resource, r);
	let l = Im(e, n.indicesByteOffset, a, n.resource, r);
	for (let e of l) if (e >= i) throw new $(`${r}: index ${e} out of range for ${i} vertices (resource ${n.resource})`);
	return {
		positions: o,
		normals: s,
		uvs: c,
		indices: l
	};
}
function Om(e, t, n, r) {
	return e instanceof sm ? new $(`${n}: resource ${t} ${r} [${e.code}]: ${e.message}`) : new $(`${n}: resource ${t} ${r} [providerFailure]: ${e instanceof Error ? e.message : String(e)}`);
}
function km(e, t, n, r) {
	let { vertexCount: i, indexCount: a } = t.layout, o = Pm(t, "position"), s = Pm(t, "normal"), c = Fm(e, n.positionsByteOffset, i * o, "positions", n.buffer, r), l = Fm(e, n.normalsByteOffset, i * s, "normals", n.buffer, r), u = n.uvsByteOffset === void 0 ? void 0 : Fm(e, n.uvsByteOffset, i * Pm(t, "uv"), "uvs", n.buffer, r);
	Am(t, u, `buffer ${n.buffer}`, r);
	let d = Im(e, n.indicesByteOffset, a, n.buffer, r);
	for (let e = 0; e < d.length; e++) if (d[e] >= i) throw new $(`${r}: index ${d[e]} out of range for ${i} vertices (buffer ${n.buffer})`);
	return {
		positions: c,
		normals: l,
		uvs: u,
		indices: d
	};
}
function Am(e, t, n, r) {
	if (t === void 0) return;
	let i = e.provenance === "voxelChunk" || e.provenance === "voxelObject";
	for (let e = 0; e < t.length; e++) {
		let a = t[e];
		if (!Number.isFinite(a) || i && Math.abs(a) > 16777216) throw new $(`${r}: invalid voxel tile coordinate ${a} at uvs[${e}] (${n})`);
	}
}
function jm(e, t, n, r) {
	return e instanceof sm ? new $(`${n}: buffer ${t} ${r} [${e.code}]: ${e.message}`) : new $(`${n}: buffer ${t} ${r} [providerFailure]: ${e instanceof Error ? e.message : String(e)}`);
}
function Mm(e, t, n) {
	try {
		e.releaseBuffer(t);
	} catch (e) {
		throw jm(e, t, n, "release failed");
	}
}
function Nm(e, t) {
	try {
		e.releaseBuffer(t);
	} catch {}
}
function Pm(e, t) {
	return e.layout.attributes.find((e) => e.name === t)?.components ?? (t === "uv" ? 2 : 3);
}
function Fm(e, t, n, r, i, a) {
	let o = Lm(e, t, n * Float32Array.BYTES_PER_ELEMENT, r, i, a);
	return new Float32Array(o.buffer, o.byteOffset, n);
}
function Im(e, t, n, r, i) {
	let a = Lm(e, t, n * Uint32Array.BYTES_PER_ELEMENT, "indices", r, i);
	return new Uint32Array(a.buffer, a.byteOffset, n);
}
function Lm(e, t, n, r, i, a) {
	if (t < 0 || t + n > e.bytes.length) throw new $(`${a}: ${r} window [${t}, ${t + n}) exceeds buffer ${i} length ${e.bytes.length}`);
	return e.bytes.slice(t, t + n);
}
function Rm() {
	let e = new Ja();
	return e.setAttribute("position", new Ia([
		0,
		0,
		0
	], 3)), e;
}
function zm(e, t) {
	let n = new Ja();
	return n.setAttribute("position", new Ia([
		e[0],
		e[1],
		e[2],
		t[0],
		t[1],
		t[2]
	], 3)), n;
}
function Bm(e) {
	return String(Number(e.toFixed(4)));
}
function Vm(e) {
	return `${Bm(e.x)},${Bm(e.y)},${Bm(e.z)}`;
}
function Hm(e) {
	let t = e.material, n = (Array.isArray(t) ? t[0] : t)?.color;
	return n ? `${Bm(n.r)},${Bm(n.g)},${Bm(n.b)}` : "none";
}
function Um(e, t) {
	return [
		e.geometry.uuid,
		t.map((e) => e.uuid).join(","),
		String(e.renderOrder),
		e.castShadow ? "cast" : "no-cast",
		e.receiveShadow ? "receive" : "no-receive"
	].join("|");
}
function Wm(e, t) {
	let n = e;
	for (; n !== null;) {
		if (!n.visible) return !1;
		if (n === t) return !0;
		n = n.parent;
	}
	return !1;
}
function Gm(e) {
	return e.kind !== "light" && !(e.kind === "primitive" && e.shape === "group");
}
function Km(e, t) {
	let n = !1, r = !1;
	return t.traverse((t) => {
		n && r || qm(t) && (n = !0, r ||= e.intersectsObject(t));
	}), n && r;
}
function qm(e) {
	return e instanceof vo || e instanceof is || e instanceof ms;
}
function Jm(e) {
	return e.elements.every(Number.isFinite);
}
function Ym(e, t) {
	e.position.set(t.translation[0], t.translation[1], t.translation[2]), e.quaternion.set(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]), e.scale.set(t.scale[0], t.scale[1], t.scale[2]);
}
function Xm(e, t) {
	e.name = t.label ?? "", e.userData.renderMetadata = structuredClone(t);
}
function Zm(e) {
	let t = e.userData.renderMetadata;
	return t === void 0 ? {
		sourceEntity: null,
		sourceSceneNode: null,
		tags: [],
		label: null
	} : structuredClone(t);
}
function Qm(e, t) {
	if (e.shape === "group") return;
	let n = e.object, r = n.material;
	n.material = bm(e.shape, t), Array.isArray(r) ? r.forEach((e) => e.dispose()) : r.dispose();
}
function $m(e) {
	let t = e;
	t.geometry?.dispose(), Array.isArray(t.material) ? t.material.forEach((e) => e.dispose()) : t.material?.dispose();
}
function eh(e) {
	let t = /* @__PURE__ */ new Set();
	for (let n of Object.values(e)) if (n instanceof fi) t.add(n);
	else if (Array.isArray(n)) for (let e of n) e instanceof fi && t.add(e);
	return t;
}
function th(e) {
	let t = 0, n = e.parent;
	for (; n !== null;) t += 1, n = n.parent;
	return t;
}
function nh(e) {
	if (e instanceof _f) return new $(e.message);
	throw e;
}
function rh(e) {
	for (let t of e.values()) t.forEach((e) => e.dispose());
}
function ih(e) {
	rh(e.geometries);
	for (let t of e.textures.values()) t?.texture.dispose();
}
function ah(e) {
	return e.enabled && e.kind !== "ambient" && e.shadowIntent === "requested";
}
//#endregion
//#region packages/renderer-three/dist/browser-surface-render-pass.js
function oh(e, t, n, r, i) {
	e.clear(!0, !0, !0), r.advanceAnimation(i), r.prepareSpritesForCamera(t, r.scene), r.prepareStaticInstanceBatches(t), e.render(r.scene, t), e.clearDepth(), r.prepareSpritesForCamera(n, r.viewmodelScene), e.render(r.viewmodelScene, n);
}
//#endregion
//#region packages/renderer-three/dist/gpu-submission-fence.js
var sh = class {
	#e;
	#t;
	#n = !1;
	#r = [];
	constructor(e, t = {}) {
		this.#e = e, this.#t = ch(t.maximumPendingSubmissions ?? 1, "maximum pending GPU submissions");
	}
	ready(e = this.#t) {
		let t = Math.min(this.#t, ch(e, "automatic pending GPU submission limit"));
		if (this.#e === null || this.#n) return !0;
		for (let e = this.#r.length - 1; e >= 0; --e) {
			let t = this.#r[e];
			if (t === void 0) continue;
			let n;
			try {
				n = this.#e.poll(t);
			} catch {
				return this.#i(), !0;
			}
			if (n === "failed") return this.#i(), !0;
			n === "signaled" && (this.#a(t), this.#r.splice(e, 1));
		}
		return this.#r.length < t;
	}
	submitted() {
		if (!(this.#e === null || this.#n)) try {
			for (; this.#r.length >= this.#t;) {
				let e = this.#r.shift();
				e !== void 0 && this.#a(e);
			}
			let e = this.#e.create();
			if (e === null) {
				this.#n = !0;
				return;
			}
			this.#r.push(e), this.#e.flush();
		} catch {
			this.#i();
		}
	}
	sample() {
		return Object.freeze({
			schemaVersion: 1,
			mode: this.#e === null ? "unsupported" : this.#n ? "disabled" : "active",
			maximumPendingSubmissions: this.#t,
			pendingSubmissionCount: this.#r.length
		});
	}
	dispose() {
		this.#i();
	}
	#i() {
		for (let e of this.#r) this.#a(e);
		this.#r.length = 0, this.#n = !0;
	}
	#a(e) {
		if (this.#e !== null) try {
			this.#e.delete(e);
		} catch {}
	}
};
function ch(e, t) {
	if (!Number.isSafeInteger(e) || e < 1) throw RangeError(`${t} must be a positive safe integer`);
	return e;
}
//#endregion
//#region packages/renderer-three/dist/gpu-submission-duty.js
var lh = 8, uh = 17, dh = .5, fh = 100, ph = .2, mh = class {
	#e;
	#t;
	#n;
	#r;
	#i;
	#a = null;
	#o = !1;
	#s = null;
	#c = 0;
	#l = 0;
	#u = [];
	#d;
	#f = !1;
	constructor(e, t = {}) {
		this.#n = e, this.#e = t.clock ?? e ?? hh(), this.#i = t.rendererClass ?? "unknown", this.#r = yh(t.maximumPendingMeasurements ?? 1, "maximum pending GPU measurements"), this.#t = this.#i === "software" ? 0 : uh, this.#d = _h(e === null ? "completionOnly" : "timerQuery", "idle", this.#i, this.#t);
	}
	begin(e) {
		if (!this.#o) {
			for (this.#p(); this.#u.length >= this.#r;) this.#h();
			if (this.#s = null, this.#d = vh(this.#d, {
				mode: this.#b(),
				state: this.#u.length === 0 ? "idle" : "measuring"
			}), !(this.#n === null || this.#f)) try {
				let t = this.#x(), n = gh(this.#i, e, t), r = this.#n.begin();
				r === null ? this.#_() : this.#a = {
					query: r,
					deadlineOriginMs: n
				};
			} catch {
				this.#_();
			}
		}
	}
	submitted() {
		if (this.#o) return;
		let e = this.#x();
		if (e === null) {
			this.#v();
			return;
		}
		let t = this.#a, n = gh(this.#i, t?.deadlineOriginMs, e);
		if (this.#l = Math.max(this.#l, n + this.#c), this.#d = vh(this.#d, {
			mode: this.#b(),
			state: "measuring"
		}), this.#n === null || this.#f || t === null) {
			this.#s = e;
			return;
		}
		let { query: r } = t;
		this.#a = null;
		try {
			this.#n.end(r), this.#u.push({
				deadlineOriginMs: gh(this.#i, t.deadlineOriginMs, e),
				query: r,
				submittedAtMs: e
			});
		} catch {
			this.#g(r), this.#_(), this.#s = e;
		}
	}
	aborted() {
		if (this.#n === null || this.#a === null) return;
		let { query: e } = this.#a;
		this.#a = null;
		try {
			this.#n.end(e);
		} catch {}
		this.#g(e);
	}
	ready(e) {
		if (this.#o) return !0;
		let t = this.#x();
		if (t === null) return this.#v(), !0;
		for (let e = 0; e < this.#u.length;) {
			let n = this.#u[e];
			if (n === void 0) {
				e += 1;
				continue;
			}
			let r;
			if (this.#n === null) r = { status: "failed" };
			else try {
				r = this.#n.poll(n.query);
			} catch {
				r = { status: "failed" };
			}
			if (r.status === "pending") {
				e += 1;
				continue;
			}
			if (r.status === "failed" || !Number.isFinite(r.durationMs) || r.durationMs < 0) {
				let e = Math.max(n.submittedAtMs, ...this.#u.map((e) => e.submittedAtMs));
				this.#_(), this.#s = e;
				break;
			}
			this.#u.splice(e, 1), this.#g(n.query), this.#y(t, r.durationMs, n.deadlineOriginMs, n.submittedAtMs);
		}
		if (this.#s !== null) {
			let e = this.#s;
			this.#s = null, this.#y(t, null, e, e);
		}
		let n = this.#b() === "timerQuery" ? this.#r : 1, r = this.#u.length < n, i = gh(this.#i, e, t), a = r && i >= this.#l;
		return this.#d = vh(this.#d, {
			mode: this.#b(),
			state: a ? "ready" : r ? "waiting" : "measuring",
			...a ? { admissionObservedAtMs: t } : {}
		}), a;
	}
	sample() {
		return Object.freeze({
			...this.#d,
			maximumPendingMeasurements: this.#b() === "timerQuery" ? this.#r : 1,
			pendingMeasurementCount: this.#u.length
		});
	}
	dispose() {
		this.#o || (this.#p(), this.#m(), this.#s = null, this.#c = 0, this.#l = 0, this.#o = !0, this.#d = vh(this.#d, { state: "disposed" }));
	}
	#p() {
		if (this.#n === null || this.#a === null) return;
		let { query: e } = this.#a;
		this.#a = null;
		try {
			this.#n.end(e);
		} catch {}
		this.#g(e);
	}
	#m() {
		for (let e of this.#u) this.#g(e.query);
		this.#u.length = 0;
	}
	#h() {
		let e = this.#u.shift();
		e !== void 0 && this.#g(e.query);
	}
	#g(e) {
		if (this.#n !== null) try {
			this.#n.delete(e);
		} catch {
			this.#f = !0;
		}
	}
	#_() {
		this.#p(), this.#m(), this.#f = !0, this.#d = vh(this.#d, { mode: "timerFailed" });
	}
	#v() {
		this.#p(), this.#m(), this.#s = null, this.#c = 0, this.#l = 0, this.#f = !0, this.#d = vh(this.#d, {
			mode: "timerFailed",
			state: "ready"
		});
	}
	#y(e, t, n, r) {
		let i = Math.max(0, e - r), a = Math.max(0, i - this.#t), o = this.#i === "accelerated" && t !== null, s = o ? t : Math.max(t ?? 0, a), c = s * (1 / Math.min(dh, Math.max(ph, dh * lh / Math.max(s, 2 ** -52))) - 1), l = s + Math.min(fh, Math.max(0, c - s)), u = s <= 2 ** -52 ? dh : s / (s + l);
		this.#c = s + l;
		let d = o ? n : r;
		this.#l = Math.max(this.#l, d + this.#c), this.#d = Object.freeze({
			schemaVersion: 1,
			mode: this.#b(),
			state: e >= this.#l ? "ready" : "waiting",
			rendererClass: this.#i,
			timerDurationMs: t,
			completionAgeMs: i,
			completionAllowanceMs: this.#t,
			effectiveDurationMs: s,
			targetDutyFraction: u,
			admittedAtMs: this.#l,
			admissionObservedAtMs: null,
			observedAtMs: e
		});
	}
	#b() {
		return this.#f ? "timerFailed" : this.#n === null ? "completionOnly" : "timerQuery";
	}
	#x() {
		try {
			let e = this.#e.now();
			return Number.isFinite(e) && e >= 0 ? e : null;
		} catch {
			return null;
		}
	}
};
function hh() {
	return { now: () => globalThis.performance?.now() ?? 0 };
}
function gh(e, t, n) {
	return e === "accelerated" && t != null && Number.isFinite(t) && t >= 0 ? t : n ?? 0;
}
function _h(e, t, n, r) {
	return Object.freeze({
		schemaVersion: 1,
		mode: e,
		state: t,
		rendererClass: n,
		timerDurationMs: null,
		completionAgeMs: null,
		completionAllowanceMs: r,
		effectiveDurationMs: null,
		targetDutyFraction: null,
		admittedAtMs: null,
		admissionObservedAtMs: null,
		observedAtMs: null
	});
}
function vh(e, t) {
	return Object.freeze({
		...e,
		...t
	});
}
function yh(e, t) {
	if (!Number.isSafeInteger(e) || e < 1) throw RangeError(`${t} must be a positive safe integer`);
	return e;
}
//#endregion
//#region packages/renderer-three/dist/gpu-submission-class.js
function bh(e) {
	return typeof e != "string" || e.length === 0 ? "unknown" : /swiftshader|llvmpipe|software rasterizer|software renderer|microsoft basic render/iu.test(e) ? "software" : "accelerated";
}
function xh(e, t) {
	return e === "accelerated" && t ? 8 : 1;
}
//#endregion
//#region packages/renderer-three/dist/software-renderer-resolution.js
var Sh = .25;
function Ch(e, t) {
	if (!Number.isFinite(e) || e <= 0) throw RangeError("renderer pixel ratio must be finite and greater than zero");
	return t === "software" ? Math.min(e, Sh) : e;
}
//#endregion
//#region packages/renderer-three/dist/view-composition.js
var wh = class extends Error {
	code = "invalid_view_composition";
	constructor(e) {
		super(e), this.name = "RendererViewCompositionPolicyError";
	}
}, Th = Object.freeze({
	schemaVersion: 1,
	cameras: Object.freeze([]),
	targets: Object.freeze([]),
	views: Object.freeze([]),
	presentations: Object.freeze([])
}), Eh = class {
	#e = /* @__PURE__ */ new Map();
	#t;
	#n;
	#r = /* @__PURE__ */ new Map();
	#i = Th;
	#a = !1;
	#o = /* @__PURE__ */ new Map();
	#s = 0;
	#c = /* @__PURE__ */ new Map();
	constructor(e, t) {
		this.#n = e, this.#t = t;
	}
	configure(e) {
		if (this.#a) return this.#d("surface_disposed", "renderer view composition is disposed");
		let t = null;
		try {
			let n = Ah(e);
			return Ue(n), this.#m(n), t = this.#l(n), this.#u(t), Object.freeze({
				applied: !0,
				diagnostics: Object.freeze([]),
				revision: this.#s
			});
		} catch (e) {
			t !== null && Lh(t, this.#c);
			let n = Hh(e);
			return this.#d(n.code, n.message);
		}
	}
	readout() {
		let e = this.#i.targets.map((e) => {
			let t = this.#c.get(e.id), n = t?.lastRefreshedSubmission ?? null;
			return Object.freeze({
				...e,
				lastRefreshedSubmission: n,
				status: n === null ? "never_rendered" : t?.stale === !0 ? "stale" : "current"
			});
		});
		return Object.freeze({
			schemaVersion: 1,
			revision: this.#s,
			cameras: this.#i.cameras,
			targets: Object.freeze(e),
			views: this.#i.views,
			presentations: this.#i.presentations,
			resources: Object.freeze({
				presentationCount: this.#o.size,
				targetCount: this.#c.size
			})
		});
	}
	visibilityReadout() {
		let e = this.#i.views.map((e) => {
			let t = this.#r.get(e.cameraId);
			return t === void 0 ? null : Object.freeze({
				viewId: e.id,
				cameraId: e.cameraId,
				target: e.target.kind,
				visibility: this.#t.visibilityReadout(t, this.#t.scene)
			});
		}).filter((e) => e !== null).sort((e, t) => e.viewId.localeCompare(t.viewId));
		return Object.freeze({
			schemaVersion: 1,
			views: Object.freeze(e)
		});
	}
	render(e, t, n) {
		if (this.#a || this.#i.views.length === 0) return;
		let r = this.#i.views.filter((e) => e.target.kind === "offscreen").sort(zh);
		for (let t of r) this.#f(t, e);
		let i = [...this.#i.views.filter((e) => e.target.kind === "primary").map((e) => ({
			id: e.id,
			kind: "view",
			order: e.order
		})), ...this.#i.presentations.map((e) => ({
			id: e.id,
			kind: "presentation",
			order: e.order
		}))].sort(zh);
		this.#n.setRenderTarget(null), this.#n.setScissorTest(!0);
		try {
			for (let e of i) if (e.kind === "view") {
				let r = this.#i.views.find((t) => t.id === e.id);
				r !== void 0 && this.#p(r, t, n);
			} else {
				let r = this.#i.presentations.find((t) => t.id === e.id), i = this.#o.get(e.id);
				if (r !== void 0 && i !== void 0) {
					let e = Bh(r.destination.viewport, t, n);
					Vh(this.#n, e), this.#n.clear(!1, !0, !1), this.#n.render(i.scene, kh);
				}
			}
		} finally {
			this.#n.setRenderTarget(null), this.#n.setScissorTest(!1), Vh(this.#n, {
				x: 0,
				y: 0,
				width: t,
				height: n
			});
		}
	}
	invalidate() {
		if (!this.#a) for (let e of this.#c.values()) e.stale = !0;
	}
	dispose() {
		if (!this.#a) {
			Ih(this.#o);
			for (let e of this.#c.values()) e.target.dispose();
			this.#r = /* @__PURE__ */ new Map(), this.#i = Th, this.#o = /* @__PURE__ */ new Map(), this.#c = /* @__PURE__ */ new Map(), this.#a = !0;
		}
	}
	#l(e) {
		let t = new Map(e.cameras.map((e) => [e.id, Mh(e)])), n = /* @__PURE__ */ new Map(), r = [], i = /* @__PURE__ */ new Map();
		try {
			for (let t of e.targets) {
				let e = this.#c.get(t.id);
				if (e !== void 0 && e.descriptor.revision === t.revision && Rh(e.descriptor, t)) {
					n.set(t.id, e);
					continue;
				}
				let i = Ph(t), a = {
					descriptor: t,
					target: i,
					lastRefreshedSubmission: null,
					stale: !0
				};
				r.push(a), this.#n.initRenderTarget(i), n.set(t.id, a);
			}
			for (let t of e.presentations) {
				let e = n.get(t.sourceTargetId);
				if (e === void 0) throw Error("validated presentation source is missing");
				i.set(t.id, Fh(e.target.texture));
			}
			return {
				cameras: t,
				composition: e,
				createdTargets: r,
				presentations: i,
				targets: n
			};
		} catch (e) {
			Ih(i);
			for (let e of r) e.target.dispose();
			throw new Oh(e instanceof Error ? e.message : String(e));
		}
	}
	#u(e) {
		let t = this.#c, n = this.#o;
		this.#r = e.cameras, this.#i = e.composition, this.#o = e.presentations, this.#c = e.targets, this.invalidate(), this.#s += 1;
		for (let t of e.composition.targets) this.#e.set(t.id, t.revision);
		Ih(n);
		for (let [n, r] of t) e.targets.get(n) !== r && r.target.dispose();
	}
	#d(e, t) {
		return Object.freeze({
			applied: !1,
			diagnostics: Object.freeze([Object.freeze({
				code: e,
				message: t
			})]),
			revision: this.#s
		});
	}
	#f(e, t) {
		if (e.target.kind !== "offscreen") return;
		let n = this.#c.get(e.target.targetId), r = this.#r.get(e.cameraId);
		if (n === void 0 || r === void 0) return;
		let i = Bh(e.viewport, n.descriptor.width, n.descriptor.height);
		Nh(r, i.width / i.height), r.updateMatrixWorld(!0), this.#t.scene.updateMatrixWorld(!0), this.#n.setRenderTarget(n.target), this.#n.setScissorTest(!1), Vh(this.#n, i), this.#n.setScissorTest(!0), this.#n.clear(!0, !0, !0), this.#t.prepareSpritesForCamera(r, this.#t.scene), this.#t.prepareStaticInstanceBatches(r), this.#n.render(this.#t.scene, r), n.lastRefreshedSubmission = t, n.stale = !1;
	}
	#p(e, t, n) {
		let r = this.#r.get(e.cameraId);
		if (r === void 0) return;
		let i = Bh(e.viewport, t, n);
		Nh(r, i.width / i.height), Vh(this.#n, i), this.#n.clear(!0, !0, !0), this.#t.prepareSpritesForCamera(r, this.#t.scene), this.#t.prepareStaticInstanceBatches(r), this.#n.render(this.#t.scene, r);
	}
	#m(e) {
		for (let t of e.targets) {
			let e = this.#c.get(t.id)?.descriptor, n = this.#e.get(t.id);
			if (e !== void 0 && t.revision === e.revision) {
				if (!Rh(e, t)) throw new Dh(`${t.id} revision ${String(t.revision)} cannot change target facts`);
				continue;
			}
			if (n !== void 0 && t.revision <= n) throw new Dh(`${t.id} revision must be greater than ${String(n)}`);
		}
	}
}, Dh = class extends Error {}, Oh = class extends Error {}, kh = new Sc(-1, 1, 1, -1, .1, 10);
kh.position.z = 1, kh.updateMatrixWorld(!0);
function Ah(e) {
	return jh(structuredClone(e));
}
function jh(e) {
	if (typeof e != "object" || !e || Object.isFrozen(e)) return e;
	for (let t of Object.values(e)) jh(t);
	return Object.freeze(e);
}
function Mh(e) {
	let t = e.projection.kind === "perspective" ? new _c(e.projection.fovYDegrees, 1, e.projection.near, e.projection.far) : new Sc(-e.projection.verticalSize / 2, e.projection.verticalSize / 2, e.projection.verticalSize / 2, -e.projection.verticalSize / 2, e.projection.near, e.projection.far);
	return t.name = e.id, t.position.set(...e.pose.position), t.up.set(0, 1, 0), t.rotation.order = "YXZ", t.rotation.x = Jr.degToRad(e.pose.pitchDegrees), t.rotation.y = Jr.degToRad(e.pose.yawDegrees), t.rotation.z = 0, t.updateMatrixWorld(!0), t;
}
function Nh(e, t) {
	if (e instanceof _c) {
		e.aspect = t, e.updateProjectionMatrix();
		return;
	}
	if (e instanceof Sc) {
		let n = e.top - e.bottom;
		e.left = -(n * t) / 2, e.right = n * t / 2, e.updateProjectionMatrix();
	}
}
function Ph(e) {
	let t = e.sampling === "nearest" ? Nt : It, n = new hi(e.width, e.height, {
		depthBuffer: e.depth === "depth24",
		generateMipmaps: !1,
		magFilter: t,
		minFilter: t,
		stencilBuffer: !1
	});
	return n.texture.colorSpace = ir, n.texture.name = e.id, n;
}
function Fh(e) {
	let t = new js({
		depthTest: !1,
		depthWrite: !1,
		fragmentShader: "\n      uniform sampler2D sourceTarget;\n      varying vec2 sourceUv;\n      void main() {\n        gl_FragColor = texture2D(sourceTarget, sourceUv);\n      }\n    ",
		toneMapped: !1,
		uniforms: { sourceTarget: { value: e } },
		vertexShader: "\n      varying vec2 sourceUv;\n      void main() {\n        sourceUv = uv;\n        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);\n      }\n    "
	}), n = new vo(new xs(2, 2), t), r = new ea();
	return r.add(n), {
		material: t,
		scene: r
	};
}
function Ih(e) {
	for (let t of e.values()) {
		for (let e of t.scene.children) e instanceof vo && e.geometry.dispose();
		t.material.dispose();
	}
}
function Lh(e, t) {
	Ih(e.presentations);
	for (let n of e.createdTargets) t.get(n.descriptor.id) !== n && n.target.dispose();
}
function Rh(e, t) {
	return e.id === t.id && e.revision === t.revision && e.width === t.width && e.height === t.height && e.color === t.color && e.depth === t.depth && e.sampling === t.sampling;
}
function zh(e, t) {
	return e.order - t.order || e.id.localeCompare(t.id);
}
function Bh(e, t, n) {
	let r = Math.round(e.x * t), i = Math.round(e.y * n);
	return {
		x: r,
		y: i,
		width: Math.max(1, Math.min(t - r, Math.round(e.width * t))),
		height: Math.max(1, Math.min(n - i, Math.round(e.height * n)))
	};
}
function Vh(e, t) {
	let n = 1 / e.getPixelRatio();
	e.setViewport(t.x * n, t.y * n, t.width * n, t.height * n), e.setScissor(t.x * n, t.y * n, t.width * n, t.height * n);
}
function Hh(e) {
	let t = e instanceof Error ? e.message : String(e);
	return e instanceof Dh ? {
		code: "stale_target_revision",
		message: t
	} : e instanceof Oh ? {
		code: "target_allocation_failed",
		message: t
	} : {
		code: "invalid_view_composition",
		message: t
	};
}
//#endregion
//#region packages/renderer-three/dist/browser-surface.js
function Uh(e, t, n, r, i, a = 0, o = 0) {
	let s = e > 0 ? e : a > 0 ? a : Number.isFinite(n) ? n / i : 0, c = t > 0 ? t : o > 0 ? o : Number.isFinite(r) ? r / i : 0;
	return {
		width: Math.max(1, Math.round(s) || 800),
		height: Math.max(1, Math.round(c) || 450)
	};
}
function Wh(e, t = {}) {
	let n = Gh(t.lighting), r = new fm({
		...t.animatedMeshSource === void 0 ? {} : { animatedMeshSource: t.animatedMeshSource },
		...t.meshBufferSource === void 0 ? {} : { meshBufferSource: t.meshBufferSource },
		...t.meshResourceSource === void 0 ? {} : { meshResourceSource: t.meshResourceSource },
		...t.textureResourceSource === void 0 ? {} : { textureResourceSource: t.textureResourceSource },
		shadowsEnabled: n.shadows.enabled,
		maximumActiveShadowLights: n.shadows.maximumActiveLights
	}), i = n.defaultLights.world === "neutral" ? Kh([
		5,
		8,
		6
	]) : [];
	i.length > 0 && r.scene.add(...i);
	let a = n.defaultLights.viewmodel === "neutral" ? Kh([
		2,
		3,
		2
	]) : [];
	a.length > 0 && r.viewmodelScene.add(...a);
	let o = t.frame ?? Qh();
	try {
		r.applyFrame(o);
	} catch (e) {
		throw r.dispose(), e;
	}
	let s = new mf({
		canvas: e,
		antialias: !0
	});
	s.shadowMap.enabled = n.shadows.enabled;
	let c = s.getContext(), l = Yh(c), u = qh(c), d = Jh(c), f = xh(l, d !== null), p = new sh(u, { maximumPendingSubmissions: f }), m = new mh(d, {
		maximumPendingMeasurements: f,
		rendererClass: l
	});
	s.autoClear = !1, s.info.autoReset = !1, s.setClearColor(t.clearColor ?? 1054752, 1);
	let h = t.pixelRatio ?? globalThis.devicePixelRatio ?? 1, g = Ch(h, l);
	s.setPixelRatio(g);
	let _ = Zh(t.camera?.projection ?? {
		fovYDegrees: 55,
		near: .1,
		far: 100
	}), v = new _c(_.fovYDegrees, 1, _.near, _.far);
	v.name = "world-camera";
	let y = new _c(_.fovYDegrees, 1, _.near, _.far);
	y.name = "viewmodel-camera";
	let b = new qc(), x = new Yr(0, 0), S = new q(), C = t.camera?.initialPose ?? {
		position: [
			0,
			1.62,
			8
		],
		pitchDegrees: 0,
		yawDegrees: 0
	}, w = t.camera?.initialBasis ?? null, T = null, E = null, D = null, O = {
		width: 0,
		height: 0
	}, k = 0, A = !1, ee = new Eh(s, r);
	if (t.viewComposition !== void 0) {
		let e = ee.configure(t.viewComposition);
		if (!e.applied) throw ee.dispose(), s.dispose(), r.dispose(), new wh(e.diagnostics[0]?.message ?? "view composition was rejected");
	}
	let te = (e, t) => {
		if (C = e, w = t ?? null, v.position.set(e.position[0], e.position[1], e.position[2]), w === null) {
			v.up.set(0, 1, 0), v.rotation.order = "YXZ", v.rotation.x = cg(e.pitchDegrees), v.rotation.y = cg(e.yawDegrees), v.rotation.z = 0;
			return;
		}
		v.up.set(w.up[0], w.up[1], w.up[2]), S.set(v.position.x + w.forward[0], v.position.y + w.forward[1], v.position.z + w.forward[2]), v.lookAt(S);
	}, ne = () => {
		let { width: t, height: n } = Uh(e.clientWidth, e.clientHeight, e.width, e.height, h, O.width, O.height);
		(O.width !== t || O.height !== n) && (s.setSize(t, n, !1), O = {
			width: t,
			height: n
		}), v.aspect = t / n, v.updateProjectionMatrix(), y.aspect = t / n, y.updateProjectionMatrix();
	}, re = (t = globalThis.performance?.now() ?? 0) => {
		if (A) throw Error("renderer browser surface is disposed");
		let n = E;
		E = null, ne();
		let i = D === null ? 0 : Math.min(.05, Math.max(0, (t - D) / 1e3));
		D = t, s.info.reset(), m.begin(n ?? void 0);
		try {
			oh(s, v, y, r, i), k += 1, ee.render(k, e.width, e.height);
		} catch (e) {
			throw m.aborted(), e;
		}
		return m.submitted(), p.submitted(), Object.freeze({
			schemaVersion: 1,
			drawCallCount: s.info.render.calls,
			triangleCount: s.info.render.triangles,
			...r.resourceStatistics()
		});
	}, ie = (e) => {
		E = null;
		let t = m.ready(e), n = p.ready(m.sample().mode === "timerQuery" ? f : 1) && t;
		return n && e !== void 0 && Number.isFinite(e) && e >= 0 && (E = e), n;
	}, ae = (e) => (ne(), v.updateMatrixWorld(!0), Xh(v, O, e)), oe = (e) => {
		T = globalThis.requestAnimationFrame(oe), ie(e) && re(e);
	}, se = () => {
		if (A) throw Error("renderer browser surface is disposed");
		T === null && (T = globalThis.requestAnimationFrame(oe));
	}, ce = () => {
		E = null, T !== null && (globalThis.cancelAnimationFrame(T), T = null);
	};
	return te(C, w ?? void 0), re(0), t.autoStart !== !1 && se(), {
		kind: "rusty_renderer_browser_surface.v1",
		canvas: e,
		renderer: r,
		frame: o,
		automaticSubmissionPacing: () => {
			let e = m.sample(), t = p.sample();
			return Object.freeze({
				...e,
				automaticSubmissionCapacity: f,
				automaticSubmissionLimit: e.maximumPendingMeasurements,
				completionFenceMode: t.mode,
				maximumPendingSubmissions: t.maximumPendingSubmissions,
				pendingSubmissionCount: t.pendingSubmissionCount
			});
		},
		automaticSubmissionReady: ie,
		animatedMeshPlayback: (e) => r.animatedMeshPlayback(e),
		sampleAnimatedMesh: (e, t, n) => r.sampleAnimatedMesh(e, t, n),
		applyFrame: (e) => {
			r.applyFrame(e), ee.invalidate();
		},
		configureViews: (e) => ee.configure(e),
		cameraPose: () => C,
		cameraProjection: () => _,
		lightingReadout: () => {
			let e = r.lightReadout();
			return Object.freeze({
				schemaVersion: 1,
				defaultLights: Object.freeze({ ...n.defaultLights }),
				neutralLightCounts: Object.freeze({
					world: i.length,
					viewmodel: a.length
				}),
				shadows: Object.freeze({
					enabled: n.shadows.enabled,
					maximumActiveLights: n.shadows.maximumActiveLights,
					activeLights: e.filter((e) => e.shadowStatus === "active").length,
					requestedUnsupportedLights: e.filter((e) => e.shadowStatus === "requested_unsupported").length
				}),
				retainedLights: e
			});
		},
		visibilityReadout: () => Object.freeze({
			schemaVersion: 1,
			world: r.visibilityReadout(v, r.scene),
			viewmodel: r.visibilityReadout(y, r.viewmodelScene),
			views: ee.visibilityReadout().views
		}),
		viewCompositionReadout: () => ee.readout(),
		projectWorldPoint: ae,
		pick: (e) => eg(r, v, b, x, e),
		snapshot: () => r.snapshot(),
		renderOnce: re,
		setCameraPose: te,
		start: se,
		stop: ce,
		dispose: () => {
			A ||= (ce(), p.dispose(), m.dispose(), ee.dispose(), s.dispose(), r.dispose(), !0);
		}
	};
}
function Gh(e) {
	let t = e ?? {
		schemaVersion: 1,
		defaultLights: {
			world: "neutral",
			viewmodel: "neutral"
		},
		shadows: {
			enabled: !1,
			maximumActiveLights: 8
		}
	};
	if (t.schemaVersion !== 1) throw new Hf("invalid_shadow_limit", "lighting.schemaVersion must equal 1");
	for (let [e, n] of Object.entries(t.defaultLights)) if (n !== "neutral" && n !== "disabled") throw new Hf("invalid_shadow_limit", `lighting.defaultLights.${e} must be neutral or disabled`);
	let n = t.shadows.maximumActiveLights;
	if (!Number.isSafeInteger(n) || n < 0 || n > 8) throw new Hf("invalid_shadow_limit", "lighting.shadows.maximumActiveLights must be in 0..=8");
	return t;
}
function Kh(e) {
	let t = new ac(16777215, 2503224, 2.4), n = new wc(16777215, 2.2);
	return n.position.set(...e), [t, n];
}
function qh(e) {
	if (!("fenceSync" in e)) return null;
	let t = e;
	return {
		create: () => t.fenceSync(t.SYNC_GPU_COMMANDS_COMPLETE, 0),
		delete: (e) => t.deleteSync(e),
		flush: () => t.flush(),
		poll: (e) => {
			let n = t.clientWaitSync(e, 0, 0);
			return n === t.TIMEOUT_EXPIRED ? "pending" : n === t.ALREADY_SIGNALED || n === t.CONDITION_SATISFIED ? "signaled" : "failed";
		}
	};
}
function Jh(e) {
	if (!("createQuery" in e)) return null;
	let t = e, n = t.getExtension("EXT_disjoint_timer_query_webgl2");
	return n === null ? null : {
		begin: () => {
			let e = t.createQuery();
			return e === null ? null : (t.beginQuery(n.TIME_ELAPSED_EXT, e), e);
		},
		delete: (e) => t.deleteQuery(e),
		end: () => t.endQuery(n.TIME_ELAPSED_EXT),
		now: () => globalThis.performance?.now() ?? 0,
		poll: (e) => {
			if (t.getParameter(n.GPU_DISJOINT_EXT) === !0) return { status: "failed" };
			if (t.getQueryParameter(e, t.QUERY_RESULT_AVAILABLE) !== !0) return { status: "pending" };
			let r = t.getQueryParameter(e, t.QUERY_RESULT);
			return typeof r == "number" ? {
				durationMs: r / 1e6,
				status: "complete"
			} : { status: "failed" };
		}
	};
}
function Yh(e) {
	let t;
	try {
		let n = e.getExtension("WEBGL_debug_renderer_info");
		if (n === null) return "unknown";
		t = e.getParameter(n.UNMASKED_RENDERER_WEBGL);
	} catch {
		return "unknown";
	}
	return bh(t);
}
function Xh(e, t, n) {
	let r = new q(...n).project(e), i = e.position.distanceTo(new q(...n)), a = r.x >= -1 && r.x <= 1 && r.y >= -1 && r.y <= 1 && r.z >= -1 && r.z <= 1;
	return {
		xPixels: (r.x + 1) / 2 * t.width,
		yPixels: (1 - r.y) / 2 * t.height,
		depth: Math.max(0, Math.min(1, (r.z + 1) / 2)),
		distance: i,
		insideViewport: a,
		occluded: !1
	};
}
function Zh(e) {
	if (![
		e.fovYDegrees,
		e.near,
		e.far
	].every(Number.isFinite) || e.fovYDegrees <= 0 || e.fovYDegrees >= 180 || e.near <= 0 || e.far <= e.near) throw RangeError("camera projection must have a finite FOV in (0, 180) and 0 < near < far");
	return {
		fovYDegrees: e.fovYDegrees,
		near: e.near,
		far: e.far
	};
}
function Qh() {
	let e = ig();
	return {
		schemaVersion: 1,
		ops: [
			{
				op: "create",
				handle: t(4103001),
				parent: null,
				node: ag("rusty-renderer-flat-plane", "cube", [
					0,
					-.08,
					0
				], [
					18,
					.16,
					18
				], [
					.16,
					.22,
					.2,
					1
				])
			},
			{
				op: "create",
				handle: t(4103002),
				parent: null,
				node: ag("rusty-renderer-collision-wall-north", "cube", [
					0,
					.5,
					-2.5
				], [
					6,
					3,
					1
				], [
					.32,
					.38,
					.42,
					1
				])
			},
			{
				op: "create",
				handle: t(4103003),
				parent: null,
				node: ag("rusty-renderer-collision-wall-south", "cube", [
					0,
					.5,
					2.5
				], [
					6,
					3,
					1
				], [
					.32,
					.38,
					.42,
					1
				])
			},
			{
				op: "create",
				handle: t(4103004),
				parent: null,
				node: ag("rusty-renderer-collision-wall-west", "cube", [
					-2.5,
					.5,
					0
				], [
					1,
					3,
					6
				], [
					.27,
					.34,
					.37,
					1
				])
			},
			{
				op: "create",
				handle: t(4103005),
				parent: null,
				node: ag("rusty-renderer-collision-wall-east", "cube", [
					2.5,
					.5,
					0
				], [
					1,
					3,
					6
				], [
					.27,
					.34,
					.37,
					1
				])
			},
			...e.map((e, n) => ({
				op: "create",
				handle: t(4103100 + n),
				parent: null,
				node: ag(`rusty-renderer-random-cube-${String(n + 1).padStart(2, "0")}`, "cube", [
					e.position[0],
					e.size[1] / 2,
					e.position[1]
				], e.size, e.color)
			}))
		]
	};
}
var $h = 128;
function eg(e, t, n, r, i) {
	let a = ng(i);
	if (a.length > 0) return {
		diagnostics: a,
		hit: null,
		kind: "rusty_renderer_browser_surface_pick.v1"
	};
	e.prepareSpritesForCamera(t, e.scene), e.prepareStaticInstanceBatchesForPicking(), e.scene.updateMatrixWorld(!0), tg(n, t, r, i.ray), n.far = i.maxDistance ?? Infinity;
	let o = n.intersectObjects(e.scene.children, !0);
	for (let t of o) {
		let n = e.projectionIdentityForObject(t.object, t.instanceId);
		if (n === void 0 || !rg(n, i.filter)) continue;
		let r = t.face?.normal.clone() ?? new q(0, 0, 0);
		return t.face !== null && t.face !== void 0 && r.copy(e.projectionWorldNormalForObject(t.object, t.instanceId, t.face.normal)), {
			diagnostics: [],
			hit: {
				channel: "render_projection",
				distance: Number(t.distance.toFixed(4)),
				handle: n.handle,
				label: n.metadata.label,
				layer: n.layer,
				normal: [
					r.x,
					r.y,
					r.z
				],
				position: [
					t.point.x,
					t.point.y,
					t.point.z
				],
				sourceTrace: n.metadata.sourceEntity === null ? null : {
					entity: n.metadata.sourceEntity,
					kind: "render_metadata_entity"
				},
				tags: [...n.metadata.tags]
			},
			kind: "rusty_renderer_browser_surface_pick.v1"
		};
	}
	return {
		diagnostics: [],
		hit: null,
		kind: "rusty_renderer_browser_surface_pick.v1"
	};
}
function tg(e, t, n, r) {
	if (r.kind === "viewport") {
		n.set(r.point[0], r.point[1]), e.setFromCamera(n, t);
		return;
	}
	e.set(new q(...r.origin), new q(...r.direction).normalize());
}
function ng(e) {
	if (e.maxDistance !== void 0 && (!Number.isFinite(e.maxDistance) || e.maxDistance <= 0)) return [{
		code: "invalid_max_distance",
		message: "maxDistance must be finite and greater than zero"
	}];
	if ([
		e.filter?.handles?.length ?? 0,
		e.filter?.labels?.length ?? 0,
		e.filter?.layers?.length ?? 0,
		e.filter?.tags?.length ?? 0
	].some((e) => e > $h)) return [{
		code: "filter_limit_exceeded",
		message: `pick filters may contain at most ${$h} values`
	}];
	if (e.ray.kind === "viewport") {
		let [t, n] = e.ray.point;
		return ![t, n].every(Number.isFinite) || t < -1 || t > 1 || n < -1 || n > 1 ? [{
			code: "invalid_viewport_point",
			message: "viewport coordinates must be finite and within [-1, 1]"
		}] : [];
	}
	let t = [...e.ray.origin, ...e.ray.direction], n = Math.hypot(...e.ray.direction);
	return !t.every(Number.isFinite) || n === 0 ? [{
		code: "invalid_world_ray",
		message: "world ray values must be finite and direction must be non-zero"
	}] : [];
}
function rg(e, t) {
	return t === void 0 || !(t.handles !== void 0 && !t.handles.includes(e.handle) || t.labels !== void 0 && (e.metadata.label === null || !t.labels.includes(e.metadata.label)) || t.layers !== void 0 && !t.layers.includes(e.layer) || t.tags !== void 0 && !t.tags.every((t) => e.metadata.tags.some((e) => e === t)));
}
function ig() {
	let e = sg(1090765022), t = [
		[
			.28,
			.66,
			.92,
			1
		],
		[
			.92,
			.54,
			.32,
			1
		],
		[
			.46,
			.78,
			.42,
			1
		],
		[
			.82,
			.58,
			.92,
			1
		],
		[
			.92,
			.76,
			.28,
			1
		]
	], n = [
		{
			color: t[0],
			position: [0, -1.35],
			size: [
				.62,
				2.2,
				.62
			]
		},
		{
			color: t[1],
			position: [1.25, -.65],
			size: [
				.48,
				.85,
				.48
			]
		},
		{
			color: t[2],
			position: [-1.15, -.9],
			size: [
				.52,
				1.05,
				.52
			]
		},
		{
			color: t[3],
			position: [.85, 1.1],
			size: [
				.44,
				.75,
				.44
			]
		}
	];
	for (let r = n.length; r < 28; r += 1) {
		let i = lg(.55 + e() * 1.55), a = lg(.65 + e() * 2.8), o = lg(.55 + e() * 1.55), s = lg(-7 + e() * 14), c = lg(-7 + e() * 14);
		s > -3.5 && s < 3.5 && c > -3.5 && c < 3.5 && (c = lg(c < 0 ? c - 3.75 : c + 3.75)), n.push({
			color: t[r % t.length],
			position: [s, c],
			size: [
				i,
				a,
				o
			]
		});
	}
	return n;
}
function ag(e, t, n, r, i) {
	return {
		geometry: { kind: t },
		material: {
			color: i,
			wireframe: !1
		},
		transform: og(n, r),
		visible: !0,
		layer: "scene",
		metadata: {
			sourceEntity: null,
			sourceSceneNode: null,
			tags: [],
			label: e
		}
	};
}
function og(e, t) {
	return {
		translation: e,
		rotation: [
			0,
			0,
			0,
			1
		],
		scale: t
	};
}
function sg(e) {
	let t = e >>> 0;
	return () => (t = Math.imul(t, 1664525) + 1013904223 >>> 0, t / 4294967296);
}
function cg(e) {
	return e * Math.PI / 180;
}
function lg(e) {
	return Number(e.toFixed(2));
}
//#endregion
//#region packages/renderer-host/dist/resource-content-hash.js
async function ug(e, t) {
	if (/^[0-9a-f]{16}$/u.test(t)) return hg(e);
	let n = t.startsWith("sha256:");
	if (!/^(?:sha256:)?[0-9a-f]{64}$/u.test(t)) throw Error(`unsupported renderer resource content hash ${t}`);
	let r = pg(e);
	return n ? `sha256:${r}` : r;
}
var dg = [
	1779033703,
	3144134277,
	1013904242,
	2773480762,
	1359893119,
	2600822924,
	528734635,
	1541459225
], fg = [
	1116352408,
	1899447441,
	3049323471,
	3921009573,
	961987163,
	1508970993,
	2453635748,
	2870763221,
	3624381080,
	310598401,
	607225278,
	1426881987,
	1925078388,
	2162078206,
	2614888103,
	3248222580,
	3835390401,
	4022224774,
	264347078,
	604807628,
	770255983,
	1249150122,
	1555081692,
	1996064986,
	2554220882,
	2821834349,
	2952996808,
	3210313671,
	3336571891,
	3584528711,
	113926993,
	338241895,
	666307205,
	773529912,
	1294757372,
	1396182291,
	1695183700,
	1986661051,
	2177026350,
	2456956037,
	2730485921,
	2820302411,
	3259730800,
	3345764771,
	3516065817,
	3600352804,
	4094571909,
	275423344,
	430227734,
	506948616,
	659060556,
	883997877,
	958139571,
	1322822218,
	1537002063,
	1747873779,
	1955562222,
	2024104815,
	2227730452,
	2361852424,
	2428436474,
	2756734187,
	3204031479,
	3329325298
];
function pg(e) {
	let t = new Uint8Array(e), n = Math.ceil((t.byteLength + 9) / 64) * 64, r = new Uint8Array(n);
	r.set(t), r[t.byteLength] = 128;
	let i = BigInt(t.byteLength) * 8n;
	for (let e = 0; e < 8; e += 1) r[n - 1 - e] = Number(i >> BigInt(e * 8) & 255n);
	let a = dg[0], o = dg[1], s = dg[2], c = dg[3], l = dg[4], u = dg[5], d = dg[6], f = dg[7], p = /* @__PURE__ */ new Uint32Array(64);
	for (let e = 0; e < r.byteLength; e += 64) {
		for (let t = 0; t < 16; t += 1) {
			let n = e + t * 4;
			p[t] = (r[n] << 24 | r[n + 1] << 16 | r[n + 2] << 8 | r[n + 3]) >>> 0;
		}
		for (let e = 16; e < p.length; e += 1) {
			let t = p[e - 15], n = p[e - 2], r = mg(t, 7) ^ mg(t, 18) ^ t >>> 3, i = mg(n, 17) ^ mg(n, 19) ^ n >>> 10;
			p[e] = p[e - 16] + r + p[e - 7] + i >>> 0;
		}
		let t = a, n = o, i = s, m = c, h = l, g = u, _ = d, v = f;
		for (let e = 0; e < p.length; e += 1) {
			let r = mg(h, 6) ^ mg(h, 11) ^ mg(h, 25), a = h & g ^ ~h & _, o = v + r + a + fg[e] + p[e] >>> 0, s = (mg(t, 2) ^ mg(t, 13) ^ mg(t, 22)) + (t & n ^ t & i ^ n & i) >>> 0;
			v = _, _ = g, g = h, h = m + o >>> 0, m = i, i = n, n = t, t = o + s >>> 0;
		}
		a = a + t >>> 0, o = o + n >>> 0, s = s + i >>> 0, c = c + m >>> 0, l = l + h >>> 0, u = u + g >>> 0, d = d + _ >>> 0, f = f + v >>> 0;
	}
	return [
		a,
		o,
		s,
		c,
		l,
		u,
		d,
		f
	].map((e) => e.toString(16).padStart(8, "0")).join("");
}
function mg(e, t) {
	return e >>> t | e << 32 - t;
}
function hg(e) {
	let t = 14695981039346656037n;
	for (let n of new Uint8Array(e)) t ^= BigInt(n), t = BigInt.asUintN(64, t * 1099511628211n);
	return t.toString(16).padStart(16, "0");
}
//#endregion
//#region packages/renderer-host/dist/animated-mesh-host.js
function gg(e, t, n = null) {
	return t === void 0 ? {
		handle: e,
		asset: null,
		contentHash: null,
		status: "unavailable",
		selectedClip: null,
		mixerTimeSeconds: 0,
		actionTimeSeconds: null,
		commandSelected: !1,
		running: !1,
		paused: !1,
		loop: null,
		speed: null,
		weight: null,
		poseSample: null,
		diagnostics: [vg("animated_mesh_handle_unavailable", null, e, `animated mesh handle ${e} is unavailable`)],
		projectionOnly: !0,
		controllerClips: []
	} : {
		handle: e,
		asset: t.asset,
		contentHash: n,
		status: t.status,
		selectedClip: t.currentClip,
		mixerTimeSeconds: t.mixerTimeSeconds,
		actionTimeSeconds: t.actionTimeSeconds,
		commandSelected: t.commandSelected,
		running: t.running,
		paused: t.paused,
		loop: t.loop,
		speed: t.speed,
		weight: t.weight,
		poseSample: t.poseSample,
		diagnostics: t.diagnostics.map((n) => vg(_g(n), t.asset, e, n)),
		projectionOnly: !0,
		controllerClips: t.controllerClips
	};
}
function _g(e) {
	switch (e) {
		case "animation_not_started":
		case "animation_paused":
		case "animation_stopped": return e;
		default: return "animated_mesh_frame_rejected";
	}
}
function vg(e, t, n, r) {
	return {
		code: e,
		message: r,
		asset: t,
		handle: n
	};
}
var yg = class extends Error {
	code;
	resource;
	constructor(e, t, n) {
		super(n), this.code = e, this.resource = t, this.name = "RendererMeshResourceError";
	}
};
async function bg(e, t) {
	xg(e);
	let n = await Promise.all(e.resources.map(async (e) => {
		let n;
		try {
			n = await t(e);
		} catch (t) {
			throw Sg("mesh_resource_unavailable", e.resource, t);
		}
		let r = n.slice(0);
		if (r.byteLength !== e.byteLength) throw Sg("mesh_resource_byte_length_mismatch", e.resource, `expected ${String(e.byteLength)} bytes, received ${String(r.byteLength)}`);
		let i = await ug(r, e.contentHash);
		if (i !== e.contentHash) throw Sg("mesh_resource_content_hash_mismatch", e.resource, `expected ${e.contentHash}, received ${i}`);
		return [e.resource, {
			descriptor: e,
			bytes: new Uint8Array(r)
		}];
	})), r = new Map(n);
	return {
		acquireResource: (e, t, n) => {
			let i = r.get(e);
			if (i === void 0) throw Sg("mesh_resource_unavailable", e, "resource was not preloaded");
			if (i.descriptor.contentHash !== t || i.descriptor.byteLength !== n) throw Sg("mesh_resource_manifest_invalid", e, "retained descriptor does not match the admitted resource manifest");
			return { bytes: i.bytes };
		},
		releaseResource: () => {}
	};
}
function xg(e) {
	if (e.kind !== "rusty_renderer_mesh_resources.v1" || e.resources.length === 0 || e.resources.length > 1024) throw Sg("mesh_resource_manifest_invalid", null, "mesh resource manifest is empty, oversized, or unsupported");
	let t = /* @__PURE__ */ new Set(), n = 0;
	for (let r of e.resources) {
		let e = /^sha256:([0-9a-f]{64})$/u.exec(r.contentHash)?.[1];
		if (e === void 0 || r.resource !== `mesh-resource/${e}` || !Number.isSafeInteger(r.byteLength) || r.byteLength < 16 || r.byteLength > 67108864 || t.has(r.resource)) throw Sg("mesh_resource_manifest_invalid", r.resource || null, "mesh resource descriptor is invalid or duplicated");
		if (t.add(r.resource), n += r.byteLength, n > 268435456) throw Sg("mesh_resource_manifest_invalid", r.resource, "mesh resource manifest exceeds the aggregate byte bound");
	}
}
function Sg(e, t, n) {
	return new yg(e, t, n instanceof Error ? n.message : String(n));
}
var Cg = class extends Error {
	code;
	resource;
	constructor(e, t, n) {
		super(n), this.code = e, this.resource = t, this.name = "RendererTextureResourceError";
	}
};
async function wg(e, t) {
	Tg(e);
	let n = await Promise.all(e.resources.map(async (e) => {
		let n;
		try {
			n = await t(e);
		} catch (t) {
			throw Eg("texture_resource_unavailable", e.resource, t);
		}
		let r = n.slice(0);
		if (r.byteLength !== e.byteLength) throw Eg("texture_resource_byte_length_mismatch", e.resource, `expected ${String(e.byteLength)} bytes, received ${String(r.byteLength)}`);
		let i = await ug(r, e.contentHash);
		if (i !== e.contentHash) throw Eg("texture_resource_content_hash_mismatch", e.resource, `expected ${e.contentHash}, received ${i}`);
		return [e.resource, {
			descriptor: e,
			bytes: new Uint8Array(r)
		}];
	})), r = new Map(n);
	return {
		acquireResource: (e, t, n) => {
			let i = r.get(e);
			if (i === void 0) throw Eg("texture_resource_unavailable", e, "resource was not preloaded");
			if (i.descriptor.contentHash !== t || i.descriptor.byteLength !== n) throw Eg("texture_resource_manifest_invalid", e, "retained descriptor does not match the admitted resource manifest");
			return { bytes: i.bytes };
		},
		releaseResource: () => {}
	};
}
function Tg(e) {
	if (e.kind !== "rusty_renderer_texture_resources.v1" || e.resources.length === 0 || e.resources.length > 256) throw Eg("texture_resource_manifest_invalid", null, "texture resource manifest is empty, oversized, or unsupported");
	let t = /* @__PURE__ */ new Set(), n = 0;
	for (let r of e.resources) {
		let e = /^sha256:([0-9a-f]{64})$/u.exec(r.contentHash)?.[1];
		if (e === void 0 || r.resource !== `texture-resource/${e}` || !Number.isSafeInteger(r.byteLength) || r.byteLength <= 0 || r.byteLength > 16777216 || t.has(r.resource)) throw Eg("texture_resource_manifest_invalid", r.resource || null, "texture resource descriptor is invalid or duplicated");
		if (t.add(r.resource), n += r.byteLength, n > 134217728) throw Eg("texture_resource_manifest_invalid", r.resource, "texture resource manifest exceeds the aggregate byte bound");
	}
}
function Eg(e, t, n) {
	return new Cg(e, t, n instanceof Error ? n.message : String(n));
}
//#endregion
//#region packages/renderer-host/dist/presentation-host-set.js
var Dg = class {
	#e;
	constructor(e) {
		this.#e = { ...e };
	}
	async apply(e) {
		o(e);
		let t = [];
		for (let n of Og) {
			let r = e.ops.filter((e) => e.domain === n), i = this.#e[n];
			if (i === void 0) {
				let e = r.map((e) => Ag(e));
				t.push({
					domain: n,
					configured: !1,
					requested: r.length,
					applied: 0,
					diagnostics: e
				});
				continue;
			}
			if (r.length === 0) {
				t.push({
					domain: n,
					configured: !0,
					requested: 0,
					applied: 0,
					diagnostics: []
				});
				continue;
			}
			let a = await i.applyPresentation({
				schemaVersion: 1,
				ops: r
			});
			t.push({
				domain: n,
				configured: !0,
				requested: r.length,
				applied: a.applied,
				diagnostics: a.diagnostics.map((e) => ({
					domain: n,
					...e
				}))
			});
		}
		return Mg(t);
	}
	advance(e) {
		if (!Number.isFinite(e) || e < 0) throw RangeError("presentation deltaSeconds must be finite and non-negative");
		let t = [], n = [], r = 0;
		for (let i of kg) {
			let a = this.#e[i];
			if (a === void 0) continue;
			let o = a.advance(e);
			t.push(i), r += o.applied, n.push(...o.diagnostics.map((e) => ({
				domain: i,
				...e
			})));
		}
		return {
			schemaVersion: 1,
			advancedDomains: t,
			applied: r,
			diagnostics: n
		};
	}
	requiresAnimationFrame() {
		return kg.some((e) => {
			let t = this.#e[e];
			return t !== void 0 && (t.requiresAnimationFrame?.() ?? !0);
		});
	}
}, Og = [
	"animation",
	"audio",
	"billboard",
	"particle",
	"telemetryOverlay"
], kg = ["animation", "particle"];
function Ag(e) {
	return {
		domain: e.domain,
		code: "unavailableHost",
		sequence: e.meta.sequence,
		handle: jg(e),
		message: `${e.domain} presentation was requested without a configured host`
	};
}
function jg(e) {
	let t = e.op;
	return "handle" in t ? t.handle : null;
}
function Mg(e) {
	return {
		schemaVersion: 1,
		applied: e.reduce((e, t) => e + t.applied, 0),
		domains: e,
		diagnostics: e.flatMap((e) => e.diagnostics)
	};
}
var Ng = class {
	#e = null;
	#t = 0;
	#n = null;
	record(e) {
		if (Pg(e.sourceTimeMs), this.#t === 2 ** 53 - 1) throw Error("renderer surface timing sequence is exhausted");
		let t = Fg(this.#e, e.sourceTimeMs), n = Ig(e.backendSubmissionStartedMs, e.backendSubmissionEndedMs), r = Object.freeze({
			schemaVersion: 1,
			renderSequence: this.#t + 1,
			source: e.source,
			sourceTimeMs: e.sourceTimeMs,
			frameIntervalMs: t.value,
			frameIntervalStatus: t.status,
			backendSubmissionDurationMs: n.value,
			backendSubmissionDurationStatus: n.status
		});
		return this.#e = e.sourceTimeMs, this.#t = r.renderSequence, this.#n = r, r;
	}
	read() {
		if (this.#n === null) throw Error("renderer surface has not submitted a frame");
		return this.#n;
	}
};
function Pg(e) {
	if (!Number.isFinite(e) || e < 0 || e > 2 ** 53 - 1) throw Error("renderer surface source time must be finite and in 0..=Number.MAX_SAFE_INTEGER");
}
function Fg(e, t) {
	if (e === null) return {
		value: null,
		status: "firstFrame"
	};
	let n = t - e;
	return n < 0 ? {
		value: null,
		status: "sourceTimeRegressed"
	} : n > 6e4 ? {
		value: null,
		status: "sourceTimeGapExceeded"
	} : {
		value: n,
		status: "available"
	};
}
function Ig(e, t) {
	if (!Number.isFinite(e) || !Number.isFinite(t) || e < 0 || t < 0) return {
		value: null,
		status: "clockUnavailable"
	};
	let n = t - e;
	return n < 0 ? {
		value: null,
		status: "clockRegressed"
	} : n > 6e4 ? {
		value: null,
		status: "durationExceeded"
	} : {
		value: n,
		status: "available"
	};
}
[...Object.keys({
	drawCallCount: "perSubmission",
	renderHandleCount: "liveResident",
	geometryResourceCount: "liveResident",
	materialResourceCount: "liveResident",
	textureResourceCount: "liveResident",
	animatedInstanceCount: "liveResident",
	triangleCount: "perSubmission"
})];
function Lg(e, t) {
	return Object.freeze({
		...e,
		statistics: Rg(t)
	});
}
function Rg(e) {
	return Object.freeze({
		schemaVersion: 1,
		drawCallCount: zg("perSubmission", e.drawCallCount),
		renderHandleCount: zg("liveResident", e.renderHandleCount),
		geometryResourceCount: zg("liveResident", e.geometryResourceCount),
		materialResourceCount: zg("liveResident", e.materialResourceCount),
		textureResourceCount: zg("liveResident", e.textureResourceCount),
		animatedInstanceCount: zg("liveResident", e.animatedInstanceCount),
		triangleCount: zg("perSubmission", e.triangleCount)
	});
}
function zg(e, t) {
	return Object.freeze(t === void 0 ? {
		scope: e,
		status: "unsupported",
		value: null
	} : t === null || !Number.isSafeInteger(t) || t < 0 ? {
		scope: e,
		status: "unavailable",
		value: null
	} : {
		scope: e,
		status: "available",
		value: t
	});
}
var Bg = class {
	#e = 0;
	#t = 0;
	#n = 0;
	#r = 0;
	#i = [];
	record(e, t, n, r, i) {
		switch (this.#t += 1, t) {
			case "admitted":
				this.#e += 1;
				break;
			case "backendBlocked":
				this.#n += 1;
				break;
			case "noDemand":
				this.#r += 1;
				break;
		}
		let a = Object.freeze({
			schemaVersion: 1,
			sequence: this.#t,
			sourceTimeMs: e,
			outcome: t,
			demand: n,
			callback: Object.freeze({ ...i }),
			backend: Object.freeze({
				mode: r.mode,
				state: r.state,
				rendererClass: r.rendererClass,
				timerDurationMs: r.timerDurationMs,
				effectiveDurationMs: r.effectiveDurationMs,
				admittedAtMs: r.admittedAtMs,
				admissionObservedAtMs: r.admissionObservedAtMs,
				observedAtMs: r.observedAtMs,
				automaticSubmissionLimit: r.automaticSubmissionLimit,
				pendingMeasurementCount: r.pendingMeasurementCount,
				completionFenceMode: r.completionFenceMode,
				maximumPendingSubmissions: r.maximumPendingSubmissions,
				pendingSubmissionCount: r.pendingSubmissionCount
			})
		});
		this.#i.push(a), this.#i.length > 64 && this.#i.shift();
	}
	sample() {
		return Object.freeze({
			schemaVersion: 1,
			attemptCount: this.#t,
			admittedCount: this.#e,
			backendBlockedCount: this.#n,
			noDemandCount: this.#r,
			recentAttempts: Object.freeze([...this.#i])
		});
	}
}, Vg = class {
	#e = !1;
	#t;
	constructor(e) {
		this.#t = e;
	}
	request() {
		this.#e = !0;
	}
	consume(e, t) {
		return this.consumeDecision(e, t).shouldSubmit;
	}
	consumeDecision(e, t) {
		let n = !Hg(this.#t, e);
		this.#t = e;
		let r = this.#e, i = r || n || t.controls || t.presentation || t.retainedAnimation;
		return this.#e = !1, Object.freeze({
			schemaVersion: 1,
			requested: r,
			viewportChanged: n,
			controls: t.controls,
			presentation: t.presentation,
			retainedAnimation: t.retainedAnimation,
			shouldSubmit: i
		});
	}
	submitted(e) {
		this.#t = e, this.#e = !1;
	}
};
function Hg(e, t) {
	return e.bufferHeight === t.bufferHeight && e.bufferWidth === t.bufferWidth && e.clientHeight === t.clientHeight && e.clientWidth === t.clientWidth;
}
var Ug = class extends Error {
	code = "invalid_lighting_policy";
	constructor(e) {
		super(e), this.name = "RendererSurfaceLightingError";
	}
}, Wg = {
	family: "threejs",
	implementation: "rusty-engine-renderer-backend",
	publicContract: "rusty-renderer-surface.v1"
};
function Gg() {
	return Qh();
}
function Kg(e) {
	let t = e;
	return t.meshResourceManifest !== void 0 || t.resolveMeshResource !== void 0 || t.textureResourceManifest !== void 0 || t.resolveTextureResource !== void 0;
}
async function qg(e) {
	if (e.meshResourceManifest === void 0 != (e.resolveMeshResource === void 0)) throw Error("meshResourceManifest requires an explicit resource resolver");
	if (e.textureResourceManifest === void 0 != (e.resolveTextureResource === void 0)) throw Error("textureResourceManifest requires an explicit resource resolver");
	let t = e.meshResourceManifest === void 0 ? void 0 : await bg(e.meshResourceManifest, e.resolveMeshResource), n = e.textureResourceManifest === void 0 ? void 0 : await wg(e.textureResourceManifest, e.resolveTextureResource);
	return {
		...t === void 0 ? {} : { meshResourceSource: t },
		...n === void 0 ? {} : { textureResourceSource: n }
	};
}
function Jg(e, t = {}) {
	return Kg(t) ? Yg(e, t) : Xg(e, t);
}
async function Yg(e, t) {
	return Xg(e, t, await qg(t));
}
function Xg(e, t, n = {}) {
	let r = d_(t.lighting), i = t.frame ?? Gg(), a = new tt();
	a.applyFrame(i);
	let o = t_(e, t.controls), s;
	try {
		s = Wh(e, {
			autoStart: !1,
			...n.animatedMeshSource === void 0 ? {} : { animatedMeshSource: n.animatedMeshSource },
			...t.meshBufferSource === void 0 ? {} : { meshBufferSource: t.meshBufferSource },
			...n.meshResourceSource === void 0 ? {} : { meshResourceSource: n.meshResourceSource },
			...n.textureResourceSource === void 0 ? {} : { textureResourceSource: n.textureResourceSource },
			camera: {
				initialPose: o.cameraPose(),
				...t.projection === void 0 ? {} : { projection: t.projection }
			},
			...t.clearColor === void 0 ? {} : { clearColor: t.clearColor },
			...t.pixelRatio === void 0 ? {} : { pixelRatio: t.pixelRatio },
			lighting: r,
			frame: i,
			...t.viewComposition === void 0 ? {} : { viewComposition: t.viewComposition }
		});
	} catch (e) {
		throw o.dispose(), e;
	}
	let c = e_(s, n.contentHashes ?? /* @__PURE__ */ new Map()), l = t.presentationHosts ?? null, u = null, d = null, f = new Ng(), p = null, m = new Vg(i_(e)), h = new Bg(), g = !1, _ = () => ({
		controls: o.requiresAnimationFrame(),
		presentation: l?.requiresAnimationFrame() ?? !1,
		retainedAnimation: r_(p)
	}), v = () => {
		m.request();
	}, y = (t, n) => {
		if (g) throw Error("renderer surface is disposed");
		Pg(t);
		let r = d === null ? 0 : Math.min(.05, Math.max(0, (t - d) / 1e3));
		d = t, o.update(r);
		let i = Qg(), a = o.cameraSnapshot();
		s.setCameraPose(a.pose, a.basis);
		let c = Qg();
		l?.advance(r);
		let u = Qg(), h = Qg(), _ = s.renderOnce(t), v = Qg();
		return p = Zg(f.record({
			source: n,
			sourceTimeMs: t,
			backendSubmissionStartedMs: h,
			backendSubmissionEndedMs: v
		}), _), m.submitted(i_(e)), {
			submission: p,
			controlsUpdatedAtMs: i,
			cameraUpdatedAtMs: c,
			presentationAdvancedAtMs: u,
			backendSubmittedAtMs: v
		};
	}, b = (e = globalThis.performance?.now() ?? 0) => y(e, "explicit").submission, x = (t) => {
		let n = Qg();
		u = globalThis.requestAnimationFrame(x);
		let r = Qg(), i = m.consumeDecision(i_(e), _()), a = Qg();
		if (i.shouldSubmit) {
			let e = s.automaticSubmissionReady(t), o = Qg(), c = s.automaticSubmissionPacing();
			if (e) {
				let e = y(t, "animationFrame"), s = Qg();
				h.record(t, "admitted", i, c, $g({
					callbackStartedAtMs: n,
					successorQueuedAtMs: r,
					demandObservedAtMs: a,
					backendReadinessObservedAtMs: o,
					controlsUpdatedAtMs: e.controlsUpdatedAtMs,
					cameraUpdatedAtMs: e.cameraUpdatedAtMs,
					presentationAdvancedAtMs: e.presentationAdvancedAtMs,
					backendSubmittedAtMs: e.backendSubmittedAtMs,
					callbackEndedAtMs: s
				}));
			} else {
				let e = Qg();
				h.record(t, "backendBlocked", i, c, $g({
					callbackStartedAtMs: n,
					successorQueuedAtMs: r,
					demandObservedAtMs: a,
					backendReadinessObservedAtMs: o,
					callbackEndedAtMs: e
				})), v();
			}
		} else {
			let e = Qg();
			h.record(t, "noDemand", i, s.automaticSubmissionPacing(), $g({
				callbackStartedAtMs: n,
				successorQueuedAtMs: r,
				demandObservedAtMs: a,
				callbackEndedAtMs: e
			}));
		}
	}, S = () => {
		if (g) throw Error("renderer surface is disposed");
		u === null && (u = globalThis.requestAnimationFrame(x), v());
	}, C = () => {
		u !== null && (globalThis.cancelAnimationFrame(u), u = null);
	};
	return y(0, "mount"), t.autoStart !== !1 && S(), {
		kind: "rusty_renderer_surface.v1",
		backend: Wg,
		canvas: e,
		animationProjection: c,
		animatedMeshPlayback: (e) => c.playback(e),
		sampleAnimatedMesh: (e, t, n) => s.sampleAnimatedMesh(e, t, n),
		applyFrame: (e) => {
			try {
				return a.validateFrame(e), s.applyFrame(e), a.applyFrame(e), v(), {
					applied: !0,
					diagnostics: []
				};
			} catch (e) {
				return {
					applied: !1,
					diagnostics: [{
						code: e instanceof Hf ? "renderer_lighting_policy_rejected" : "animated_mesh_frame_rejected",
						message: e instanceof Error ? e.message : String(e),
						asset: null,
						handle: null
					}]
				};
			}
		},
		applyPresentation: async (e) => {
			let t = await (l ?? new Dg({})).apply(e);
			return t.applied > 0 && v(), t;
		},
		automaticSubmissionPacing: () => Object.freeze({
			...s.automaticSubmissionPacing(),
			hostAdmission: h.sample()
		}),
		cameraPose: o.cameraPose,
		cameraProjection: s.cameraProjection,
		inputReadout: o.inputReadout,
		lightingReadout: s.lightingReadout,
		visibilityReadout: s.visibilityReadout,
		configureViews: (e) => {
			let t = s.configureViews(e);
			return t.applied && v(), t;
		},
		viewCompositionReadout: s.viewCompositionReadout,
		lockPointer: o.lockPointer,
		movementState: o.movementState,
		pick: (e) => {
			let t = s.pick(e);
			return {
				diagnostics: t.diagnostics,
				hint: t.hit,
				kind: "rusty_renderer_surface_pick.v1"
			};
		},
		pointerLocked: o.pointerLocked,
		projectWorldPoint: s.projectWorldPoint,
		projectionSnapshot: () => a.snapshot(),
		releaseInput: o.releaseInput,
		renderOnce: b,
		resetCamera: () => {
			o.resetCamera(), d = null, y(0, "cameraReset");
		},
		setCameraPose: (e, t) => {
			let n = o.cameraSnapshot();
			o.setCameraPose(e, t), s.setCameraPose(e, t), a_(n, o.cameraSnapshot()) || v();
		},
		setPresentationHosts: (e) => {
			l = e, v();
		},
		snapshot: s.snapshot,
		start: S,
		stop: C,
		submission: () => {
			if (p === null) throw Error("renderer surface has not submitted a frame");
			return p;
		},
		timing: f.read.bind(f),
		dispose: () => {
			g ||= (C(), o.dispose(), s.dispose(), !0);
		}
	};
}
function Zg(e, t) {
	return Lg(e, {
		drawCallCount: t.drawCallCount,
		renderHandleCount: t.renderHandleCount,
		geometryResourceCount: t.geometryResourceCount,
		materialResourceCount: t.materialResourceCount,
		textureResourceCount: t.textureResourceCount,
		animatedInstanceCount: t.animatedInstanceCount,
		triangleCount: t.triangleCount
	});
}
function Qg() {
	return globalThis.performance?.now() ?? 0;
}
function $g(e) {
	return Object.freeze({
		schemaVersion: 1,
		callbackStartedAtMs: e.callbackStartedAtMs,
		successorQueuedAtMs: e.successorQueuedAtMs,
		demandObservedAtMs: e.demandObservedAtMs,
		backendReadinessObservedAtMs: e.backendReadinessObservedAtMs ?? null,
		controlsUpdatedAtMs: e.controlsUpdatedAtMs ?? null,
		cameraUpdatedAtMs: e.cameraUpdatedAtMs ?? null,
		presentationAdvancedAtMs: e.presentationAdvancedAtMs ?? null,
		backendSubmittedAtMs: e.backendSubmittedAtMs ?? null,
		callbackEndedAtMs: e.callbackEndedAtMs
	});
}
function e_(e, t) {
	return {
		kind: "rusty_renderer_animated_mesh_projection.v1",
		applyFrame: (t) => {
			try {
				return e.applyFrame(t), {
					applied: !0,
					diagnostics: []
				};
			} catch (e) {
				return {
					applied: !1,
					diagnostics: [{
						code: "animated_mesh_frame_rejected",
						message: e instanceof Error ? e.message : String(e),
						asset: null,
						handle: null
					}]
				};
			}
		},
		advance: () => ({
			applied: !0,
			diagnostics: []
		}),
		playback: (n) => {
			let r = e.animatedMeshPlayback(n);
			return gg(n, r, r === void 0 ? null : t.get(r.asset) ?? null);
		},
		snapshot: e.snapshot,
		hasAnimationTarget: (t) => e.renderer.has(t),
		setAnimationControllerWeights: (t, n) => {
			e.renderer.setAnimationControllerWeights(t, n);
		},
		hasAnimationClips: (t, n) => e.renderer.hasAnimationControllerClips(t, n),
		clearAnimationControllerWeights: (t) => {
			e.renderer.clearAnimationControllerWeights(t);
		}
	};
}
function t_(e, t) {
	let n = t?.enabled === !0, r = e.ownerDocument, i = g_(t?.moveSpeed ?? 5.8, "moveSpeed"), a = g_(t?.mouseSensitivity ?? .0021, "mouseSensitivity"), o = h_(t?.eyeHeight ?? 1.62, "eyeHeight"), s = m_(t?.initialPosition ?? [
		0,
		o,
		8
	], "initialPosition"), c = t?.resolveMovement, l = /* @__PURE__ */ new Set(), u = [0, 0], d, f = 0, p = v_(h_(t?.initialPitchDegrees ?? 0, "initialPitchDegrees")), m = v_(h_(t?.initialYawDegrees ?? 0, "initialYawDegrees")), h = [...s], g = u_(c), _ = e.tabIndex, v = e.style.touchAction;
	e.tabIndex < 0 && (e.tabIndex = 0), e.style.touchAction = "none";
	let y = () => r.pointerLockElement === e, b = () => y() || r.activeElement === e, x = () => {
		l.clear(), u = [0, 0];
	}, S = () => {
		x(), y() && r.exitPointerLock();
	}, C = (t) => {
		!n || t.button !== 0 || (t.preventDefault(), e.focus({ preventScroll: !0 }), y() || e.requestPointerLock());
	}, w = () => {
		y() || x();
	}, T = (e) => {
		!n || !y() || (u = [u[0] + e.movementX, u[1] + e.movementY]);
	}, E = (e) => {
		!n || !b() || !n_.has(e.code) || (e.preventDefault(), l.add(e.code));
	}, D = (e) => {
		n_.has(e.code) && l.delete(e.code);
	};
	e.addEventListener("pointerdown", C), r.addEventListener("pointerlockchange", w), r.addEventListener("mousemove", T), r.addEventListener("keydown", E), r.addEventListener("keyup", D), r.defaultView?.addEventListener("blur", x);
	let O = () => ({
		position: [
			x_(h[0]),
			x_(h[1]),
			x_(h[2])
		],
		pitchDegrees: b_(y_(p)),
		yawDegrees: b_(y_(m))
	});
	return {
		cameraPose: O,
		cameraSnapshot: () => ({
			...d === void 0 ? {} : { basis: d },
			pose: O()
		}),
		inputReadout: () => ({
			enabled: n,
			pointerLocked: y(),
			pressedCodes: [...l].sort()
		}),
		lockPointer: () => {
			n && !y() && e.requestPointerLock();
		},
		movementState: () => g,
		pointerLocked: y,
		releaseInput: S,
		requiresAnimationFrame: () => n && (l.size > 0 || u[0] !== 0 || u[1] !== 0),
		resetCamera: () => {
			x(), d = void 0, f = 0, p = v_(t?.initialPitchDegrees ?? 0), m = v_(t?.initialYawDegrees ?? 0), h = [...s], g = u_(c);
		},
		setCameraPose: (e, t) => {
			f_(e), t !== void 0 && p_(t), h = [...e.position], p = v_(e.pitchDegrees), m = v_(e.yawDegrees), d = t;
		},
		update: (e) => {
			if (!n) return;
			let t = Math.max(0, h_(e, "deltaSeconds")), r = c_(l, "KeyW", "KeyS"), s = c_(l, "KeyD", "KeyA"), _ = u[0] * y_(a), v = -u[1] * y_(a);
			if (u = [0, 0], r === 0 && s === 0 && _ === 0 && v === 0) return;
			if (c !== void 0) {
				f += 1;
				let e = c({
					deltaSeconds: t,
					moveForward: r,
					moveRight: s,
					moveSpeedUnitsPerSecond: i,
					pitchDeltaDegrees: v,
					poseBefore: O(),
					sequence: f,
					yawDeltaDegrees: _
				});
				f_(e.pose), h = [...e.pose.position], p = v_(e.pose.pitchDegrees), m = v_(e.pose.yawDegrees), d = e.basis, g = {
					mode: "caller_resolved",
					blockedAxes: [...e.blockedAxes ?? []],
					collided: e.collided ?? !1,
					resolutionId: e.resolutionId ?? null
				};
				return;
			}
			m += v_(_), p = __(p + v_(v), v_(-85), v_(85)), d = void 0;
			let y = l_(m, r, s);
			if (y !== null && t > 0) {
				let e = i * t;
				h = [
					h[0] + y[0] * e,
					o,
					h[2] + y[2] * e
				];
			}
			g = u_(void 0);
		},
		dispose: () => {
			S(), e.removeEventListener("pointerdown", C), r.removeEventListener("pointerlockchange", w), r.removeEventListener("mousemove", T), r.removeEventListener("keydown", E), r.removeEventListener("keyup", D), r.defaultView?.removeEventListener("blur", x), e.tabIndex = _, e.style.touchAction = v;
		}
	};
}
var n_ = /* @__PURE__ */ new Set([
	"KeyA",
	"KeyD",
	"KeyS",
	"KeyW"
]);
function r_(e) {
	let t = e?.statistics.animatedInstanceCount;
	return t?.status === "available" && t.value > 0;
}
function i_(e) {
	return {
		bufferHeight: e.height,
		bufferWidth: e.width,
		clientHeight: e.clientHeight,
		clientWidth: e.clientWidth
	};
}
function a_(e, t) {
	return s_(e.pose.position, t.pose.position) && e.pose.pitchDegrees === t.pose.pitchDegrees && e.pose.yawDegrees === t.pose.yawDegrees && o_(e.basis, t.basis);
}
function o_(e, t) {
	return e === void 0 || t === void 0 ? e === t : s_(e.forward, t.forward) && s_(e.right, t.right) && s_(e.up, t.up);
}
function s_(e, t) {
	return e[0] === t[0] && e[1] === t[1] && e[2] === t[2];
}
function c_(e, t, n) {
	return Number(e.has(t)) - Number(e.has(n));
}
function l_(e, t, n) {
	let r = [
		-Math.sin(e),
		0,
		-Math.cos(e)
	], i = [
		Math.cos(e),
		0,
		-Math.sin(e)
	], a = [
		r[0] * t + i[0] * n,
		0,
		r[2] * t + i[2] * n
	], o = Math.hypot(a[0], a[2]);
	return o === 0 ? null : [
		a[0] / o,
		0,
		a[2] / o
	];
}
function u_(e) {
	return {
		mode: e === void 0 ? "free_camera" : "caller_resolved",
		blockedAxes: [],
		collided: !1,
		resolutionId: null
	};
}
function d_(e) {
	if (e !== void 0 && e.schemaVersion !== 1) throw new Ug("lighting.schemaVersion must equal 1");
	let t = e?.defaultLights?.world ?? "neutral", n = e?.defaultLights?.viewmodel ?? "neutral";
	if (t !== "neutral" && t !== "disabled" || n !== "neutral" && n !== "disabled") throw new Ug("default lighting mode must be neutral or disabled");
	let r = e?.shadows?.enabled ?? !1;
	if (typeof r != "boolean") throw new Ug("lighting.shadows.enabled must be boolean");
	let i = e?.shadows?.maximumActiveLights ?? 8;
	if (!Number.isSafeInteger(i) || i < 0 || i > 8) throw new Ug("lighting.shadows.maximumActiveLights must be in 0..=8");
	return {
		schemaVersion: 1,
		defaultLights: {
			world: t,
			viewmodel: n
		},
		shadows: {
			enabled: r,
			maximumActiveLights: i
		}
	};
}
function f_(e) {
	m_(e.position, "resolved camera position"), h_(e.pitchDegrees, "resolved camera pitch"), h_(e.yawDegrees, "resolved camera yaw");
}
function p_(e) {
	m_(e.forward, "camera basis forward"), m_(e.right, "camera basis right"), m_(e.up, "camera basis up");
}
function m_(e, t) {
	return e.forEach((e, n) => h_(e, `${t}[${n}]`)), e;
}
function h_(e, t) {
	if (!Number.isFinite(e)) throw RangeError(`${t} must be finite`);
	return e;
}
function g_(e, t) {
	if (!Number.isFinite(e) || e <= 0) throw RangeError(`${t} must be finite and greater than zero`);
	return e;
}
function __(e, t, n) {
	return Math.min(n, Math.max(t, e));
}
function v_(e) {
	return e * Math.PI / 180;
}
function y_(e) {
	return e * 180 / Math.PI;
}
function b_(e) {
	return Number(e.toFixed(2));
}
function x_(e) {
	return Number(e.toFixed(4));
}
180 / Math.PI;
//#endregion
//#region packages/renderer-host/dist/telemetry-host.js
var S_ = /* @__PURE__ */ new Set([
	"renderHandleCount",
	"drawCallCount",
	"geometryResourceCount",
	"materialResourceCount",
	"textureResourceCount",
	"animatedInstanceCount",
	"triangleCount"
]);
new Set([
	"entityCount",
	"activeCapabilityCount",
	"residentChunkCount",
	"dirtyChunkCount",
	"renderDiffCount",
	"renderHandleCount",
	"drawCallCount",
	"geometryResourceCount",
	"materialResourceCount",
	"textureResourceCount",
	"animatedInstanceCount",
	"triangleCount",
	"activeAudioSourceCount",
	"activeBillboardCount",
	"activeParticleCount",
	"droppedFeedbackCount"
].filter((e) => !S_.has(e)));
//#endregion
//#region packages/application-host/src/application-content.ts
var C_ = class extends Error {
	code;
	resource;
	constructor(e, t, n) {
		super(n), this.code = e, this.resource = t, this.name = "RustyApplicationContentError";
	}
}, w_ = /^(mesh|texture)-resource\/([0-9a-f]{64})$/u;
function T_(e) {
	if (typeof e != "object" || !e || typeof e.frame != "object" || e.frame === null) throw k_("content_invalid", null, "application content must include one frame");
	if (e.resources !== void 0 && !Array.isArray(e.resources)) throw k_("content_invalid", null, "application content resources must be an array");
	let t = structuredClone(e.frame), n = /* @__PURE__ */ new Set(), r = 0, i = 0, a = 0, o = 0, s = (e.resources ?? []).map((e, t) => {
		if (typeof e != "object" || !e || typeof e.identity != "string" || typeof e.contentHash != "string" || typeof e.mediaType != "string" || !(e.bytes instanceof Uint8Array)) throw k_("content_invalid", null, `application content resource ${String(t)} is malformed`);
		let s = w_.exec(e.identity), c = /^sha256:([0-9a-f]{64})$/u.exec(e.contentHash)?.[1];
		if (s === null || c === void 0 || s[2] !== c) throw k_("resource_identity_invalid", e.identity || null, "application resource identity must match its lowercase SHA-256 content hash");
		if (n.has(e.identity)) throw k_("resource_duplicate", e.identity, "application resource identity is duplicated");
		n.add(e.identity);
		let l = s[1];
		if (l === "texture") {
			if (e.mediaType !== "image/png") throw k_("resource_media_type_unsupported", e.identity, "texture resources must use image/png");
			if (a += 1, o += e.bytes.byteLength, a > 256 || e.bytes.byteLength === 0 || e.bytes.byteLength > 16777216 || o > 134217728) throw k_("resource_limit_exceeded", e.identity, "texture resource count or byte length exceeds the application-host bound");
		} else {
			if (e.mediaType !== "application/octet-stream") throw k_("resource_media_type_unsupported", e.identity, "mesh resources must use application/octet-stream");
			if (r += 1, i += e.bytes.byteLength, r > 1024 || e.bytes.byteLength < 16 || e.bytes.byteLength > 67108864 || i > 268435456) throw k_("resource_limit_exceeded", e.identity, "mesh resource count or byte length exceeds the application-host bound");
		}
		return Object.freeze({
			identity: e.identity,
			contentHash: e.contentHash,
			mediaType: e.mediaType,
			bytes: e.bytes.slice().buffer,
			kind: l
		});
	});
	return Object.freeze({
		frame: t,
		resources: Object.freeze(s),
		resourceBytes: i + o
	});
}
function E_(e) {
	let t = new Map(e.resources.map((e) => [e.identity, e])), n = e.resources.filter((e) => e.kind === "mesh"), r = e.resources.filter((e) => e.kind === "texture");
	return Object.freeze({
		...n.length === 0 ? {} : {
			meshResourceManifest: {
				kind: "rusty_renderer_mesh_resources.v1",
				resources: Object.freeze(n.map(D_))
			},
			resolveMeshResource: (e) => O_(t, e.resource)
		},
		...r.length === 0 ? {} : {
			textureResourceManifest: {
				kind: "rusty_renderer_texture_resources.v1",
				resources: Object.freeze(r.map(D_))
			},
			resolveTextureResource: (e) => O_(t, e.resource)
		}
	});
}
function D_(e) {
	return Object.freeze({
		resource: e.identity,
		contentHash: e.contentHash,
		byteLength: e.bytes.byteLength
	});
}
function O_(e, t) {
	let n = e.get(t);
	return n === void 0 ? Promise.reject(/* @__PURE__ */ Error(`resource ${t} is unavailable`)) : Promise.resolve(n.bytes.slice(0));
}
function k_(e, t, n) {
	return new C_(e, t, n);
}
//#endregion
//#region packages/application-host/src/application-host.ts
var A_ = "rusty_application_host.v1", j_ = class extends Error {
	code;
	constructor(e, t, n) {
		super(t, n), this.name = "RustyApplicationHostError", this.code = e;
	}
}, M_ = { mountSurface: Jg };
async function N_(e) {
	return P_(e, M_);
}
async function P_(e, t) {
	let { root: n } = e;
	if (G_(n), n.childNodes.length > 0) throw new j_("invalid_root", "Rusty Application Host requires an empty downstream mount root");
	let r = n.ownerDocument, i = L_(r, e.loadingLabel ?? "Starting application…");
	n.append(i.host), n.dataset.rustyApplicationState = "mounting";
	let a = null, o = null, s = () => void 0, c = !1, l = !1, u = null, d = e.initialInteractionMode ?? "interface", f = i.canvas, p = null, m = 0, h = 0, g = Promise.resolve(), _ = () => {
		if (l || c || a === null) throw new j_("disposed", "Rusty Application Host is disposed");
		return a;
	}, v = () => {
		a?.releaseInput();
	}, y = (e) => {
		if (c) throw new j_("disposed", "Rusty Application Host is disposed");
		d = e, i.host.dataset.interactionMode = e, e !== "gameplay" && v();
	}, b = () => {
		if (d !== "gameplay") return;
		let e = _();
		e.canvas.focus({ preventScroll: !0 }), U_(e.canvas);
	}, x = (n, r) => t.mountSurface(n, {
		autoStart: !0,
		controls: { enabled: !1 },
		frame: r.frame,
		...e.renderer?.clearColor === void 0 ? {} : { clearColor: e.renderer.clearColor },
		...e.renderer?.pixelRatio === void 0 ? {} : { pixelRatio: e.renderer.pixelRatio },
		...E_(r)
	}), S = (e) => {
		_(), h += 1;
		let t = Object.freeze({
			applied: !1,
			diagnostics: []
		});
		return g = g.then(async () => {
			let n = a;
			if (n === null || p === null || c) {
				t = F_(new j_("disposed", "Rusty Application Host is disposed"));
				return;
			}
			let o = f, l = R_(r);
			i.host.insertBefore(l, i.ui);
			let u = null, h = null;
			try {
				let r = e();
				u = await x(l, r), u.setCameraPose(n.cameraPose()), h = z_(i.host, i.ui, () => u, () => d, b), a = u, p = r, m += 1, f = l;
				let c = s;
				s = h, h = null;
				try {
					c();
				} catch {}
				try {
					n.dispose();
				} catch {}
				o.remove(), t = Object.freeze({
					applied: !0,
					diagnostics: []
				});
			} catch (e) {
				try {
					h?.();
				} catch {}
				try {
					u?.dispose();
				} catch {}
				l.remove(), t = F_(e);
			}
		}), g.then(() => t).finally(() => {
			--h;
		});
	}, C = (e) => {
		_();
		let t;
		try {
			t = T_(e);
		} catch (e) {
			return Promise.resolve(F_(e));
		}
		return S(() => t);
	}, w = Object.freeze({
		applyFrame: (e) => {
			if (h > 0) return Object.freeze({
				applied: !1,
				diagnostics: Object.freeze([Object.freeze({
					code: "content_replacement_in_progress",
					message: "incremental frames are rejected while complete content replacement is pending"
				})])
			});
			let t = _().applyFrame(e);
			return Object.freeze({
				applied: t.applied,
				diagnostics: Object.freeze(t.diagnostics.map((e) => Object.freeze({
					code: e.code,
					message: e.message
				})))
			});
		},
		clear: async () => {
			let e = await C({
				frame: Gg(),
				resources: []
			});
			if (!e.applied) throw Error(`Engine default renderer frame was rejected: ${e.diagnostics.map((e) => e.message).join("; ")}`);
		},
		renderOnce: (e) => {
			e === void 0 ? _().renderOnce() : _().renderOnce(e);
		},
		replaceContent: C,
		replaceFrame: (e) => {
			_();
			let t;
			try {
				t = T_({ frame: e }).frame;
			} catch (e) {
				return Promise.resolve(F_(e));
			}
			return S(() => {
				let e = p;
				if (e === null) throw new j_("disposed", "Rusty Application Host is disposed");
				return Object.freeze({
					frame: t,
					resources: e.resources,
					resourceBytes: e.resourceBytes
				});
			});
		},
		setCameraPose: (e) => _().setCameraPose(e)
	}), T = Object.freeze({
		active: () => !l && !c,
		allowsGameplayInput: (e) => !l && !c && !e.defaultPrevented && d === "gameplay" && !B_(e, i.ui),
		focusGameplay: b,
		interactionMode: () => d,
		setInteractionMode: y
	});
	try {
		if (e.renderer?.initialContent !== void 0 && e.renderer.initialFrame !== void 0) throw new C_("content_invalid", null, "initialContent and initialFrame are mutually exclusive");
		let t = T_(e.renderer?.initialContent ?? {
			frame: e.renderer?.initialFrame ?? Gg(),
			resources: []
		});
		a = await x(i.canvas, t), p = t, m = 1, s = z_(i.host, i.ui, () => _(), () => d, b), y(d), o = await e.mountUi(i.ui, {
			renderer: w,
			ui: T
		}) ?? null, i.loading.remove(), i.host.dataset.state = "ready", n.dataset.rustyApplicationState = "ready";
	} catch (t) {
		c = !0;
		let r = await W_(o, s, a, i.host);
		delete n.dataset.rustyApplicationState;
		let l = t instanceof Error ? t : Error(String(t));
		throw K_(n, e.failureLabel ?? "Application failed to start", l.message), new j_("mount_failed", r.length === 0 ? `Rusty Application Host mount failed: ${l.message}` : `Rusty Application Host mount failed: ${l.message}; cleanup also failed`, { cause: l });
	}
	return Object.freeze({
		kind: "rusty_application_host.v1",
		renderer: w,
		ui: T,
		readout: () => Object.freeze({
			compatibilityVersion: A_,
			contentRevision: m,
			interactionMode: d,
			pointerLocked: a?.pointerLocked() ?? !1,
			resourceBytes: p?.resourceBytes ?? 0,
			resourceCount: p?.resources.length ?? 0,
			state: c ? "disposed" : "ready"
		}),
		dispose: async () => u === null ? (l = !0, u = (async () => {
			await g, c = !0;
			let e = await W_(o, s, a, i.host);
			if (o = null, a = null, delete n.dataset.rustyApplicationState, e.length > 0) throw AggregateError(e, "Rusty Application Host disposal failed");
		})(), u) : u
	});
}
function F_(e) {
	return Object.freeze({
		applied: !1,
		diagnostics: Object.freeze([Object.freeze({
			code: I_(e),
			message: e instanceof Error ? e.message : String(e)
		})])
	});
}
function I_(e) {
	return e instanceof C_ ? e.code : typeof e == "object" && e && "code" in e && typeof e.code == "string" && e.code.includes("resource") || e instanceof Error && e.message.toLowerCase().includes("resource") ? "resource_admission_failed" : "retained_frame_replacement_failed";
}
function L_(e, t) {
	let n = e.createElement("div");
	n.dataset.rustyApplicationHost = A_, n.style.cssText = "isolation:isolate;min-height:100dvh;position:relative;width:100%;";
	let r = R_(e), i = e.createElement("div");
	i.dataset.rustyApplicationUi = "downstream", i.style.cssText = "min-height:100dvh;position:relative;width:100%;z-index:1;";
	let a = e.createElement("div");
	return a.dataset.rustyApplicationLoading = "", a.setAttribute("role", "status"), a.textContent = t, a.style.cssText = "align-items:center;background:#071012;color:#d9eee7;display:flex;font:14px system-ui;inset:0;justify-content:center;position:absolute;z-index:2;", n.append(r, i, a), {
		host: n,
		canvas: r,
		ui: i,
		loading: a
	};
}
function R_(e) {
	let t = e.createElement("canvas");
	return t.dataset.rustyApplicationRenderer = "engine-owned", t.setAttribute("aria-label", "Engine-rendered game world"), t.style.cssText = "display:block;height:100%;inset:0;position:absolute;width:100%;z-index:0;", t;
}
function z_(e, t, n, r, i) {
	let a = e.ownerDocument, o = (e) => {
		if (B_(e, t)) {
			n().releaseInput();
			return;
		}
		r() === "gameplay" && i();
	}, s = (e) => {
		H_(e.target) && n().releaseInput();
	}, c = () => {
		e.dataset.pointerLocked = String(a.pointerLockElement === n().canvas);
	}, l = () => n().releaseInput();
	return t.addEventListener("pointerdown", o, !0), t.addEventListener("focusin", s, !0), a.addEventListener("pointerlockchange", c), a.defaultView?.addEventListener("blur", l), c(), () => {
		t.removeEventListener("pointerdown", o, !0), t.removeEventListener("focusin", s, !0), a.removeEventListener("pointerlockchange", c), a.defaultView?.removeEventListener("blur", l);
	};
}
function B_(e, t) {
	return e.composedPath().some((e) => V_(e, t));
}
function V_(e, t) {
	return !(e instanceof Element) || !t.contains(e) ? !1 : e.closest("a,button,input,select,textarea,summary,[contenteditable=\"true\"],[data-rusty-ui-interactive],[role=\"dialog\"]") !== null;
}
function H_(e) {
	return e instanceof HTMLInputElement || e instanceof HTMLTextAreaElement || e instanceof HTMLSelectElement || e instanceof HTMLElement && e.isContentEditable;
}
function U_(e) {
	try {
		e.requestPointerLock().catch(() => void 0);
	} catch {}
}
async function W_(e, t, n, r) {
	let i = [];
	try {
		await e?.dispose();
	} catch (e) {
		i.push(e);
	}
	try {
		t();
	} catch (e) {
		i.push(e);
	}
	try {
		n?.dispose();
	} catch (e) {
		i.push(e);
	}
	return r.remove(), i;
}
function G_(e) {
	e.querySelector(":scope > [data-rusty-application-failure]")?.remove();
}
function K_(e, t, n) {
	let r = e.ownerDocument.createElement("section");
	r.dataset.rustyApplicationFailure = "", r.setAttribute("role", "alert"), r.style.cssText = "background:#1b0b0d;color:#ffe8e8;font:14px system-ui;margin:0;min-height:100dvh;padding:2rem;";
	let i = e.ownerDocument.createElement("h1");
	i.textContent = t;
	let a = e.ownerDocument.createElement("p");
	a.textContent = n, r.append(i, a), e.append(r);
}
//#endregion
export { A_ as RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION, C_ as RustyApplicationContentError, j_ as RustyApplicationHostError, N_ as mountRustyApplication };
