(function(){var e=(e,t,n)=>()=>{if(n)throw n[0];try{return e&&(t=e(e=0)),t}catch(e){throw n=[e],e}};function t(e,t){if(!Number.isSafeInteger(e)||e<0)throw RangeError(`${t} must be an unsigned JSON-safe integer`);return e}var n,r,i=e((()=>{n=e=>t(e,`render handle`),r=1e4})),a=e((()=>{}));function o(e){let t=j(e,`$`,[`schemaVersion`,`ops`]);return t.schemaVersion!==1&&V(`$.schemaVersion`,`must equal 1`),De(t.ops,`$.ops`).forEach((e,t)=>c(e,`$.ops[${String(t)}]`)),e}function s(e){let t=j(e,`$`,[`schemaVersion`,`ops`]);return t.schemaVersion!==1&&V(`$.schemaVersion`,`must equal 1`),De(t.ops,`$.ops`).forEach((e,t)=>{let n=`$.ops[${String(t)}]`,r=j(e,n,[`domain`,`meta`,`op`]),i=j(r.meta,`${n}.meta`,[`sequence`]);L(i.sequence,`${n}.meta.sequence`,0,4294967295),i.sequence!==t&&V(`${n}.meta.sequence`,`must equal ordered index ${String(t)}`),ae(B(r.domain,`${n}.domain`,[`audio`,`billboard`,`particle`,`telemetryOverlay`,`animation`]),r.op,`${n}.op`)}),e}function c(e,t){let n=ze(M(e,t).op,`${t}.op`);switch(n){case`create`:{let n=j(e,t,[`op`,`handle`,`parent`,`node`]);Ne(n.handle,`${t}.handle`),Me(n.parent,`${t}.parent`),l(n.node,`${t}.node`);return}case`update`:{let n=j(e,t,[`op`,`handle`,`transform`,`material`,`visible`,`metadata`]);Ne(n.handle,`${t}.handle`),F(n.transform,`${t}.transform`,f),F(n.material,`${t}.material`,d),F(n.visible,`${t}.visible`,Be),F(n.metadata,`${t}.metadata`,p);return}case`destroy`:Ne(j(e,t,[`op`,`handle`]).handle,`${t}.handle`);return;case`replaceMeshPayload`:{let n=j(e,t,[`op`,`handle`,`payload`]);Ne(n.handle,`${t}.handle`),m(n.payload,`${t}.payload`);return}case`createLight`:{let n=j(e,t,[`op`,`handle`,`parent`,`light`]);Ne(n.handle,`${t}.handle`),Me(n.parent,`${t}.parent`),re(n.light,`${t}.light`);return}case`updateLight`:{let n=j(e,t,[`op`,`handle`,`light`]);Ne(n.handle,`${t}.handle`),re(n.light,`${t}.light`);return}case`defineMaterial`:T(j(e,t,[`op`,`material`]).material,`${t}.material`);return;case`setMaterialInstanceParameters`:{let n=j(e,t,[`op`,`handle`,`slot`,`parameters`]);Ne(n.handle,`${t}.handle`),L(n.slot,`${t}.slot`,0,65535),F(n.parameters,`${t}.parameters`,ee);return}case`defineTexture`:k(j(e,t,[`op`,`texture`]).texture,`${t}.texture`);return;case`defineSpriteAtlas`:te(j(e,t,[`op`,`atlas`]).atlas,`${t}.atlas`);return;case`defineStaticMesh`:v(j(e,t,[`op`,`asset`]).asset,`${t}.asset`);return;case`defineAnimatedMesh`:b(j(e,t,[`op`,`asset`]).asset,`${t}.asset`);return;case`defineVoxelObject`:S(j(e,t,[`op`,`asset`]).asset,`${t}.asset`);return;case`releaseVoxelObject`:z(j(e,t,[`op`,`asset`]).asset,`${t}.asset`);return;case`createStaticMeshInstance`:{let n=j(e,t,[`op`,`handle`,`parent`,`instance`]);Ne(n.handle,`${t}.handle`),Me(n.parent,`${t}.parent`),y(n.instance,`${t}.instance`);return}case`createAnimatedMeshInstance`:{let n=j(e,t,[`op`,`handle`,`parent`,`instance`]);Ne(n.handle,`${t}.handle`),Me(n.parent,`${t}.parent`),x(n.instance,`${t}.instance`);return}case`setAnimatedMeshPlayback`:{let n=j(e,t,[`op`,`handle`,`playback`]);Ne(n.handle,`${t}.handle`),w(n.playback,`${t}.playback`);return}case`createVoxelObjectInstance`:{let n=j(e,t,[`op`,`handle`,`parent`,`instance`]);Ne(n.handle,`${t}.handle`),Me(n.parent,`${t}.parent`),C(n.instance,`${t}.instance`);return}case`setVoxelObjectFrame`:{let n=j(e,t,[`op`,`handle`,`frame`]);Ne(n.handle,`${t}.handle`),L(n.frame,`${t}.frame`,0,4294967295);return}case`createSprite`:{let n=j(e,t,[`op`,`handle`,`parent`,`sprite`]);Ne(n.handle,`${t}.handle`),Me(n.parent,`${t}.parent`),ne(n.sprite,`${t}.sprite`);return}case`updateSprite`:{let n=j(e,t,[`op`,`handle`,`frame`,`tint`,`renderOrder`,`visible`]);Ne(n.handle,`${t}.handle`),F(n.frame,`${t}.frame`,I),F(n.tint,`${t}.tint`,ke),F(n.renderOrder,`${t}.renderOrder`,Fe),F(n.visible,`${t}.visible`,Be);return}default:V(`${t}.op`,`unsupported operation ${JSON.stringify(n)}`)}}function l(e,t){let n=j(e,t,[`geometry`,`material`,`transform`,`visible`,`layer`,`metadata`]);u(n.geometry,`${t}.geometry`),d(n.material,`${t}.material`),f(n.transform,`${t}.transform`),Be(n.visible,`${t}.visible`),B(n.layer,`${t}.layer`,[`scene`,`debug`,`ui`,`viewmodel`]),p(n.metadata,`${t}.metadata`)}function u(e,t){if(B(M(e,t).kind,`${t}.kind`,[`group`,`cube`,`sphere`,`quad`,`point`,`line`])===`line`){let n=j(e,t,[`kind`,`a`,`b`]);P(n.a,`${t}.a`),P(n.b,`${t}.b`)}else j(e,t,[`kind`])}function d(e,t){let n=j(e,t,[`color`,`wireframe`]);ke(n.color,`${t}.color`),Be(n.wireframe,`${t}.wireframe`)}function f(e,t){let n=j(e,t,[`translation`,`rotation`,`scale`]);P(n.translation,`${t}.translation`);let r=N(n.rotation,`${t}.rotation`,4);r.forEach((e,n)=>Ie(e,`${t}.rotation[${String(n)}]`)),r.every(e=>e===0)&&V(`${t}.rotation`,`must be non-zero`),P(n.scale,`${t}.scale`)}function p(e,t){let n=j(e,t,[`sourceEntity`,`sourceSceneNode`,`tags`,`label`]);F(n.sourceEntity,`${t}.sourceEntity`,Pe),F(n.sourceSceneNode,`${t}.sourceSceneNode`,Pe);let r=De(n.tags,`${t}.tags`),i;r.forEach((e,n)=>{let r=z(e,`${t}.tags[${String(n)}]`);i!==void 0&&i>=r&&V(`${t}.tags`,`must be strictly sorted and unique`),i=r}),F(n.label,`${t}.label`,z)}function m(e,t){let n=j(e,t,[`layout`,`groups`,`bounds`,`source`,`provenance`]),r=j(n.layout,`${t}.layout`,[`vertexCount`,`indexCount`,`indexWidth`,`attributes`]),i=L(r.vertexCount,`${t}.layout.vertexCount`,0,4294967295),a=L(r.indexCount,`${t}.layout.indexCount`,0,4294967295);B(r.indexWidth,`${t}.layout.indexWidth`,[`u32`]);let o=De(r.attributes,`${t}.layout.attributes`),s=new Set;o.forEach((e,n)=>{let r=`${t}.layout.attributes[${String(n)}]`,i=j(e,r,[`name`,`components`,`kind`]),a=B(i.name,`${r}.name`,[`position`,`normal`,`uv`,`color`]);s.has(a)&&V(`${r}.name`,`is duplicated`),s.add(a);let o=a===`uv`?2:a===`color`?4:3;i.components!==o&&V(`${r}.components`,`must equal ${String(o)}`),B(i.kind,`${r}.kind`,[`f32`])}),(!s.has(`position`)||!s.has(`normal`))&&V(`${t}.layout.attributes`,`must declare position and normal`),h(n.bounds,`${t}.bounds`),B(n.provenance,`${t}.provenance`,[`voxelChunk`,`voxelObject`,`staticAsset`,`generated`,`debug`]);let c=B(M(n.source,`${t}.source`).kind,`${t}.source.kind`,[`inline`,`sharedBuffer`,`resource`]);if(c===`inline`){let e=Ee(n.source,`${t}.source`,[`kind`,`positions`,`normals`,`indices`],[`uvs`]);if(je(e.positions,`${t}.source.positions`,i*3,!1),je(e.normals,`${t}.source.normals`,i*3,!1),s.has(`uv`)!==Object.hasOwn(e,`uvs`)&&V(`${t}.source.uvs`,`must be present exactly when the uv attribute is declared`),Object.hasOwn(e,`uvs`)){let r=je(e.uvs,`${t}.source.uvs`,i*2,!1);(n.provenance===`voxelChunk`||n.provenance===`voxelObject`)&&r.some(e=>Math.abs(e)>16777216)&&V(`${t}.source.uvs`,`voxel tile coordinate exceeds the exact f32 integer range`)}je(e.indices,`${t}.source.indices`,a,!0).forEach((e,n)=>{e>=i&&V(`${t}.source.indices[${String(n)}]`,`is outside vertex range`)})}else if(c===`sharedBuffer`){let e=Ee(n.source,`${t}.source`,[`kind`,`buffer`,`positionsByteOffset`,`normalsByteOffset`,`indicesByteOffset`],[`uvsByteOffset`]);Pe(e.buffer,`${t}.source.buffer`),I(e.positionsByteOffset,`${t}.source.positionsByteOffset`),I(e.normalsByteOffset,`${t}.source.normalsByteOffset`),s.has(`uv`)!==Object.hasOwn(e,`uvsByteOffset`)&&V(`${t}.source.uvsByteOffset`,`must be present exactly when the uv attribute is declared`),Object.hasOwn(e,`uvsByteOffset`)&&I(e.uvsByteOffset,`${t}.source.uvsByteOffset`),I(e.indicesByteOffset,`${t}.source.indicesByteOffset`)}else{let e=Ee(n.source,`${t}.source`,[`kind`,`resource`,`contentHash`,`byteLength`,`encoding`,`positionsByteOffset`,`normalsByteOffset`,`indicesByteOffset`],[`uvsByteOffset`]),r=z(e.resource,`${t}.source.resource`),o=z(e.contentHash,`${t}.source.contentHash`),c=/^sha256:([0-9a-f]{64})$/u.exec(o)?.[1];c===void 0&&V(`${t}.source.contentHash`,`must be a lowercase SHA-256 identity`),r!==`mesh-resource/${c}`&&V(`${t}.source.resource`,`must be the content-addressed mesh-resource identity`);let l=L(e.byteLength,`${t}.source.byteLength`,16,64*1024*1024),u=B(e.encoding,`${t}.source.encoding`,[`packedStreamsLeV1`,`packedStreamsLeV2`]);(s.has(`uv`)!==Object.hasOwn(e,`uvsByteOffset`)||u===`packedStreamsLeV1`&&Object.hasOwn(e,`uvsByteOffset`)||u===`packedStreamsLeV2`&&!Object.hasOwn(e,`uvsByteOffset`))&&V(`${t}.source`,`mesh resource encoding and uv stream must agree`);let d=L(e.positionsByteOffset,`${t}.source.positionsByteOffset`,16,4294967295),f=L(e.normalsByteOffset,`${t}.source.normalsByteOffset`,16,4294967295),p=Object.hasOwn(e,`uvsByteOffset`)?L(e.uvsByteOffset,`${t}.source.uvsByteOffset`,16,4294967295):void 0,m=L(e.indicesByteOffset,`${t}.source.indicesByteOffset`,16,4294967295);for(let[e,n]of[[`positionsByteOffset`,d],[`normalsByteOffset`,f],...p===void 0?[]:[[`uvsByteOffset`,p]],[`indicesByteOffset`,m]])n%4!=0&&V(`${t}.source.${e}`,`must be four-byte aligned`);let h=d+i*3*4,g=f+i*3*4,_=p===void 0?g:p+i*2*4,v=m+a*4;(h>l||g>l||_>l||v>l)&&V(`${t}.source`,`declares a mesh stream outside the resource byte length`),(h>f||(p===void 0?g:_)>m||p!==void 0&&g>p)&&V(`${t}.source`,`mesh resource streams must not overlap`)}let l=De(n.groups,`${t}.groups`),u=0;l.forEach((e,n)=>{let r=`${t}.groups[${String(n)}]`,i=j(e,r,[`materialSlot`,`start`,`count`]);L(i.materialSlot,`${r}.materialSlot`,0,65535);let o=I(i.start,`${r}.start`),s=I(i.count,`${r}.count`);o!==u&&V(`${r}.start`,`must tile from ${String(u)}`),u+=s,u>a&&V(r,`extends beyond index count`)}),u!==a&&V(`${t}.groups`,`must cover the complete index buffer`)}function h(e,t){let n=j(e,t,[`min`,`max`]),r=P(n.min,`${t}.min`),i=P(n.max,`${t}.max`);r.forEach((e,n)=>{e>i[n]&&V(t,`minimum exceeds maximum`)})}function g(e,t){let n=j(e,t,[`slot`,`material`]),r=L(n.slot,`${t}.slot`,0,65535);return z(n.material,`${t}.material`),r}function _(e,t){let n=new Set;return De(e,t).forEach((e,r)=>{let i=g(e,`${t}[${String(r)}]`);n.has(i)&&V(`${t}[${String(r)}].slot`,`is duplicated`),n.add(i)}),n}function v(e,t){let n=j(e,t,[`asset`,`payload`,`materialSlots`,`collision`]);z(n.asset,`${t}.asset`),m(n.payload,`${t}.payload`);let r=_(n.materialSlots,`${t}.materialSlots`);De(M(n.payload,`${t}.payload`).groups,`${t}.payload.groups`).forEach((e,n)=>{let i=M(e,`${t}.payload.groups[${String(n)}]`);r.has(i.materialSlot)||V(`${t}.payload.groups[${String(n)}].materialSlot`,`is not bound`)}),B(M(n.collision,`${t}.collision`).kind,`${t}.collision.kind`,[`visualOnly`,`proxy`,`aabbFallback`,`trimesh`])===`proxy`?z(j(n.collision,`${t}.collision`,[`kind`,`proxyAsset`]).proxyAsset,`${t}.collision.proxyAsset`):j(n.collision,`${t}.collision`,[`kind`])}function y(e,t){let n=j(e,t,[`asset`,`transform`,`visible`,`materialOverrides`,`metadata`]);z(n.asset,`${t}.asset`),f(n.transform,`${t}.transform`),Be(n.visible,`${t}.visible`),_(n.materialOverrides,`${t}.materialOverrides`),p(n.metadata,`${t}.metadata`)}function b(e,t){let n=j(e,t,[`asset`,`runtimeFormat`,`contentHash`,`clips`,`defaultClip`,`materialSlots`,`bounds`]);z(n.asset,`${t}.asset`),B(n.runtimeFormat,`${t}.runtimeFormat`,[`glb`]),F(n.contentHash,`${t}.contentHash`,z);let r=new Set;if(De(n.clips,`${t}.clips`).forEach((e,n)=>{let i=`${t}.clips[${String(n)}]`,a=j(e,i,[`id`,`name`,`durationSeconds`]),o=z(a.id,`${i}.id`);r.has(o)&&V(`${i}.id`,`is duplicated`),r.add(o),F(a.name,`${i}.name`,z),F(a.durationSeconds,`${i}.durationSeconds`,Le)}),n.defaultClip!==null){let e=z(n.defaultClip,`${t}.defaultClip`);r.has(e)||V(`${t}.defaultClip`,`is not declared`)}_(n.materialSlots,`${t}.materialSlots`),h(n.bounds,`${t}.bounds`)}function x(e,t){let n=j(e,t,[`asset`,`transform`,`visible`,`materialOverrides`,`playback`,`metadata`]);z(n.asset,`${t}.asset`),f(n.transform,`${t}.transform`),Be(n.visible,`${t}.visible`),_(n.materialOverrides,`${t}.materialOverrides`),F(n.playback,`${t}.playback`,w),p(n.metadata,`${t}.metadata`)}function S(e,t){let n=j(e,t,[`asset`,`contentHash`,`meshes`,`frames`,`materialSlots`]);z(n.asset,`${t}.asset`),z(n.contentHash,`${t}.contentHash`);let r=_(n.materialSlots,`${t}.materialSlots`),i=De(n.meshes,`${t}.meshes`);(i.length===0||i.length>8193)&&V(`${t}.meshes`,`must contain 1..=8193 entries`);let a=0,o=0;i.forEach((e,n)=>{let i=`${t}.meshes[${String(n)}]`,s=j(e,i,[`payload`]);m(s.payload,`${i}.payload`);let c=M(s.payload,`${i}.payload`),l=M(c.layout,`${i}.payload.layout`);a+=l.vertexCount,o+=l.indexCount,De(c.groups,`${i}.payload.groups`).forEach((e,t)=>{let n=M(e,`${i}.payload.groups[${String(t)}]`);r.has(n.materialSlot)||V(`${i}.payload.groups[${String(t)}].materialSlot`,`is not bound`)})}),(a>8e6||o>12e6)&&V(`${t}.meshes`,`exceeds aggregate vertex/index work limits`);let s=De(n.frames,`${t}.frames`);(s.length===0||s.length>8193)&&V(`${t}.frames`,`must contain 1..=8193 entries`);let c=new Set;s.forEach((e,n)=>{let r=`${t}.frames[${String(n)}]`,a=j(e,r,[`id`,`mesh`]),o=z(a.id,`${r}.id`);c.has(o)&&V(`${r}.id`,`is duplicated`),c.add(o),L(a.mesh,`${r}.mesh`,0,i.length-1)})}function C(e,t){let n=j(e,t,[`asset`,`frame`,`transform`,`visible`,`materialOverrides`,`metadata`]);z(n.asset,`${t}.asset`),L(n.frame,`${t}.frame`,0,4294967295),f(n.transform,`${t}.transform`),Be(n.visible,`${t}.visible`),_(n.materialOverrides,`${t}.materialOverrides`),p(n.metadata,`${t}.metadata`)}function w(e,t){let n=B(M(e,t).kind,`${t}.kind`,[`play`,`stop`,`pause`,`resume`]);if(n===`play`){let n=j(e,t,[`kind`,`clip`,`loop`,`speed`,`weight`,`restart`,`fadeSeconds`]);z(n.clip,`${t}.clip`),B(n.loop,`${t}.loop`,[`once`,`repeat`,`pingPong`]),Le(n.speed,`${t}.speed`),R(n.weight,`${t}.weight`,0,1),Be(n.restart,`${t}.restart`),F(n.fadeSeconds,`${t}.fadeSeconds`,Re)}else n===`stop`?F(j(e,t,[`kind`,`fadeSeconds`]).fadeSeconds,`${t}.fadeSeconds`,Re):j(e,t,[`kind`])}function T(e,t){let n=Ee(e,t,[`schemaVersion`,`id`,`color`,`texture`,`roughness`,`textureTint`,`emissionColor`,`emissionIntensity`,`uvStrategy`],[`voxelSurface`]);L(n.schemaVersion,`${t}.schemaVersion`,1,4294967295),z(n.id,`${t}.id`),ke(n.color,`${t}.color`),F(n.texture,`${t}.texture`,z),R(n.roughness,`${t}.roughness`,0,1),ke(n.textureTint,`${t}.textureTint`),Oe(n.emissionColor,`${t}.emissionColor`),Re(n.emissionIntensity,`${t}.emissionIntensity`),B(n.uvStrategy,`${t}.uvStrategy`,[`flat`,`planar`,`atlas`]),Object.hasOwn(n,`voxelSurface`)&&E(n.voxelSurface,`${t}.voxelSurface`,n.texture)}function E(e,t,n){let r=j(e,t,[`schemaVersion`,`filter`,`wrap`,`alphaMode`,`mapping`]);L(r.schemaVersion,`${t}.schemaVersion`,1,1);let i=B(r.filter,`${t}.filter`,[`nearest`,`linear`]),a=B(r.wrap,`${t}.wrap`,[`clamp`,`repeat`]);B(M(r.alphaMode,`${t}.alphaMode`).kind,`${t}.alphaMode.kind`,[`opaque`,`mask`,`blend`])===`mask`?R(j(r.alphaMode,`${t}.alphaMode`,[`kind`,`cutoff`]).cutoff,`${t}.alphaMode.cutoff`,0,1):j(r.alphaMode,`${t}.alphaMode`,[`kind`]);let o=B(M(r.mapping,`${t}.mapping`).kind,`${t}.mapping.kind`,[`repeat`,`atlas`]),s=o===`repeat`?j(r.mapping,`${t}.mapping`,[`kind`,`texture`,`textureVersion`,`textureContentHash`,`tileScaleCells`,`tileOriginCells`]):j(r.mapping,`${t}.mapping`,[`kind`,`atlas`,`atlasVersion`,`atlasContentHash`,`texture`,`textureVersion`,`textureContentHash`,`region`,`tileScaleCells`,`tileOriginCells`]);if(n!==z(s.texture,`${t}.mapping.texture`)&&V(`${t}.mapping.texture`,`must match material texture`),L(s.textureVersion,`${t}.mapping.textureVersion`,1,4294967295),z(s.textureContentHash,`${t}.mapping.textureContentHash`),D(s.tileScaleCells,`${t}.mapping.tileScaleCells`,1/256,4096),D(s.tileOriginCells,`${t}.mapping.tileOriginCells`,-16777216,16777216),o===`repeat`){a!==`repeat`&&V(`${t}.wrap`,`repeat mapping requires repeat wrap`);return}a!==`clamp`&&V(`${t}.wrap`,`atlas mapping requires clamp wrap`),z(s.atlas,`${t}.mapping.atlas`),L(s.atlasVersion,`${t}.mapping.atlasVersion`,1,4294967295),z(s.atlasContentHash,`${t}.mapping.atlasContentHash`);let c=j(s.region,`${t}.mapping.region`,[`id`,`contentMin`,`contentExtent`,`padding`,`inset`]);z(c.id,`${t}.mapping.region.id`),O(c.contentMin,`${t}.mapping.region.contentMin`,0,4294967295),O(c.contentExtent,`${t}.mapping.region.contentExtent`,1,4294967295),B(c.inset,`${t}.mapping.region.inset`,[`halfTexel`]);let l=j(c.padding,`${t}.mapping.region.padding`,[`left`,`right`,`bottom`,`top`]);for(let e of[`left`,`right`,`bottom`,`top`]){let n=+(i===`linear`);L(l[e],`${t}.mapping.region.padding.${e}`,n,32)}}function D(e,t,n,r){let i=N(e,t,2);R(i[0],`${t}[0]`,n,r),R(i[1],`${t}[1]`,n,r)}function O(e,t,n,r){let i=N(e,t,2);L(i[0],`${t}[0]`,n,r),L(i[1],`${t}[1]`,n,r)}function ee(e,t){let n=j(e,t,[`textureTint`,`emissionColor`,`emissionIntensity`]);ke(n.textureTint,`${t}.textureTint`),Oe(n.emissionColor,`${t}.emissionColor`),Re(n.emissionIntensity,`${t}.emissionIntensity`)}function k(e,t){let n=Ee(e,t,[`id`,`width`,`height`,`filter`,`wrap`,`contentHash`,`version`],[`payload`]);if(z(n.id,`${t}.id`),L(n.width,`${t}.width`,1,4096)*L(n.height,`${t}.height`,1,4096)>16777216&&V(t,`texture texel quota exceeded`),B(n.filter,`${t}.filter`,[`nearest`,`linear`]),B(n.wrap,`${t}.wrap`,[`clamp`,`repeat`]),F(n.contentHash,`${t}.contentHash`,z),L(n.version,`${t}.version`,1,4294967295),Object.hasOwn(n,`payload`)){let e=j(n.payload,`${t}.payload`,[`encoding`,`colorSpace`,`contentHash`,`byteLength`,`source`]);B(e.encoding,`${t}.payload.encoding`,[`pngRgba8`]),B(e.colorSpace,`${t}.payload.colorSpace`,[`srgb`]);let r=z(e.contentHash,`${t}.payload.contentHash`),i=/^sha256:([0-9a-f]{64})$/u.exec(r)?.[1];(i===void 0||n.contentHash!==r)&&V(`${t}.payload.contentHash`,`must be the canonical texture content hash`);let a=L(e.byteLength,`${t}.payload.byteLength`,1,16*1024*1024),o=M(e.source,`${t}.payload.source`);B(o.kind,`${t}.payload.source.kind`,[`inline`,`resource`])===`inline`?je(j(o,`${t}.payload.source`,[`kind`,`encodedBytes`]).encodedBytes,`${t}.payload.source.encodedBytes`,a,!0).forEach((e,n)=>{e>255&&V(`${t}.payload.source.encodedBytes[${String(n)}]`,`must be a byte`)}):j(o,`${t}.payload.source`,[`kind`,`resource`]).resource!==`texture-resource/${i}`&&V(`${t}.payload.source.resource`,`must match the content hash`)}}function te(e,t){let n=j(e,t,[`id`,`texture`,`frames`]);z(n.id,`${t}.id`),z(n.texture,`${t}.texture`);let r=De(n.frames,`${t}.frames`);r.length===0&&V(`${t}.frames`,`must not be empty`);let i=new Set;r.forEach((e,n)=>{let r=`${t}.frames[${String(n)}]`,a=Ee(e,r,[`frame`,`uvMin`,`uvMax`],[`size`]),o=I(a.frame,`${r}.frame`);i.has(o)&&V(`${r}.frame`,`is duplicated`),i.add(o);let s=Ae(a.uvMin,`${r}.uvMin`,2,0,1),c=Ae(a.uvMax,`${r}.uvMax`,2,0,1);(c[0]<=s[0]||c[1]<=s[1])&&V(r,`UV rectangle is degenerate`),a.size!==void 0&&N(a.size,`${r}.size`,2).forEach((e,t)=>Le(e,`${r}.size[${String(t)}]`))})}function ne(e,t){let n=j(e,t,[`asset`,`frame`,`pivot`,`size`,`sizeMode`,`billboard`,`tint`,`renderOrder`,`depth`,`shading`,`visible`,`transform`,`attachment`,`metadata`]);z(n.asset,`${t}.asset`),I(n.frame,`${t}.frame`),Ae(n.pivot,`${t}.pivot`,2,0,1),N(n.size,`${t}.size`,2).forEach((e,n)=>Le(e,`${t}.size[${String(n)}]`)),B(n.sizeMode,`${t}.sizeMode`,[`world`,`pixel`]),B(n.billboard,`${t}.billboard`,[`none`,`spherical`,`cylindrical`]),ke(n.tint,`${t}.tint`),Fe(n.renderOrder,`${t}.renderOrder`),B(n.depth,`${t}.depth`,[`default`,`depthTestOff`,`depthWriteOff`]),B(n.shading,`${t}.shading`,[`unlit`,`lit`,`shadowed`,`custom`]),Be(n.visible,`${t}.visible`),f(n.transform,`${t}.transform`);let r=j(n.attachment,`${t}.attachment`,[`sourceEntity`,`sourceSceneNode`,`attachmentPoint`]);F(r.sourceEntity,`${t}.attachment.sourceEntity`,Pe),F(r.sourceSceneNode,`${t}.attachment.sourceSceneNode`,Pe),F(r.attachmentPoint,`${t}.attachment.attachmentPoint`,z),p(n.metadata,`${t}.metadata`)}function re(e,t){let n=B(M(e,t).kind,`${t}.kind`,[`ambient`,`directional`,`point`,`spot`]),i=[`kind`,`color`,`intensity`,`enabled`,`shadowIntent`],a=j(e,t,n===`ambient`?i:n===`directional`?[...i,`direction`]:n===`point`?[...i,`position`,`range`,`decay`]:[...i,`position`,`direction`,`range`,`decay`,`outerAngleRadians`,`penumbra`]);Oe(a.color,`${t}.color`),R(a.intensity,`${t}.intensity`,0,r),Be(a.enabled,`${t}.enabled`),B(a.shadowIntent,`${t}.shadowIntent`,[`disabled`,`requested`]),(n===`directional`||n===`spot`)&&ie(a.direction,`${t}.direction`),(n===`point`||n===`spot`)&&(P(a.position,`${t}.position`),F(a.range,`${t}.range`,Le),Re(a.decay,`${t}.decay`)),n===`spot`&&(R(a.outerAngleRadians,`${t}.outerAngleRadians`,Number.MIN_VALUE,Math.PI/2),R(a.penumbra,`${t}.penumbra`,0,1))}function ie(e,t){P(e,t).every(e=>e===0)&&V(t,`must be non-zero`)}function ae(e,t,n){let r=ze(M(t,n).op,`${n}.op`);if(e===`audio`)return oe(r,t,n);if(e===`billboard`)return ce(r,t,n);if(e===`particle`)return pe(r,t,n);if(e===`telemetryOverlay`)return ye(r,t,n);Se(r,t,n)}function oe(e,t,n){if(e===`emit`){let e=j(t,n,[`op`,`signalId`,`descriptor`]);z(e.signalId,`${n}.signalId`),se(e.descriptor,`${n}.descriptor`)}else if(e===`create`){let e=j(t,n,[`op`,`handle`,`descriptor`]);Ne(e.handle,`${n}.handle`),se(e.descriptor,`${n}.descriptor`)}else if(e===`update`){let e=j(t,n,[`op`,`handle`,`patch`]);Ne(e.handle,`${n}.handle`);let r=j(e.patch,`${n}.patch`,[`volume`,`pitch`,`looping`,`spatialBlend`,`attenuation`,`pan`,`emitter`]);F(r.volume,`${n}.patch.volume`,(e,t)=>R(e,t,0,1)),F(r.pitch,`${n}.patch.pitch`,(e,t)=>R(e,t,.25,4)),F(r.looping,`${n}.patch.looping`,Be),F(r.spatialBlend,`${n}.patch.spatialBlend`,(e,t)=>R(e,t,0,1)),F(r.attenuation,`${n}.patch.attenuation`,Le),F(r.pan,`${n}.patch.pan`,(e,t)=>R(e,t,-1,1)),F(r.emitter,`${n}.patch.emitter`,(e,t)=>Te(e,t,!0))}else e===`destroy`?Ne(j(t,n,[`op`,`handle`]).handle,`${n}.handle`):V(`${n}.op`,`is unsupported for audio`)}function se(e,t){let n=j(e,t,[`clip`,`bus`,`volume`,`pitch`,`looping`,`spatialBlend`,`attenuation`,`pan`,`emitter`]),r=j(n.clip,`${t}.clip`,[`asset`,`contentHash`]);z(r.asset,`${t}.clip.asset`),z(r.contentHash,`${t}.clip.contentHash`),B(n.bus,`${t}.bus`,[`sfx`,`ambient`,`ui`]),R(n.volume,`${t}.volume`,0,1),R(n.pitch,`${t}.pitch`,.25,4),R(n.spatialBlend,`${t}.spatialBlend`,0,1),Le(n.attenuation,`${t}.attenuation`),R(n.pan,`${t}.pan`,-1,1),Be(n.looping,`${t}.looping`),Te(n.emitter,`${t}.emitter`,!0)}function ce(e,t,n){A(e,t,n,le,ue)}function le(e,t){let n=j(e,t,[`anchor`,`content`,`font`,`heightPixels`,`color`,`background`,`maxDistance`,`layer`,`visible`]);Te(n.anchor,`${t}.anchor`,!1),de(n.content,`${t}.content`),fe(n.font,`${t}.font`),R(n.heightPixels,`${t}.heightPixels`,8,256),ke(n.color,`${t}.color`),ke(n.background,`${t}.background`),R(n.maxDistance,`${t}.maxDistance`,Number.MIN_VALUE,1e4),B(n.layer,`${t}.layer`,[`alwaysOnTop`,`depthTested`,`occluded`]),Be(n.visible,`${t}.visible`)}function ue(e,t){let n=j(e,t,[`anchor`,`content`,`font`,`heightPixels`,`color`,`background`,`maxDistance`,`layer`,`visible`]);F(n.anchor,`${t}.anchor`,(e,t)=>Te(e,t,!1)),F(n.content,`${t}.content`,de),F(n.font,`${t}.font`,fe),F(n.heightPixels,`${t}.heightPixels`,(e,t)=>R(e,t,8,256)),F(n.color,`${t}.color`,ke),F(n.background,`${t}.background`,ke),F(n.maxDistance,`${t}.maxDistance`,(e,t)=>R(e,t,Number.MIN_VALUE,1e4)),F(n.layer,`${t}.layer`,(e,t)=>B(e,t,[`alwaysOnTop`,`depthTested`,`occluded`])),F(n.visible,`${t}.visible`,Be)}function de(e,t){let n=B(M(e,t).kind,`${t}.kind`,[`text`,`value`,`icon`]);if(n===`text`){let n=j(e,t,[`kind`,`localizationKey`,`fallbackText`,`arguments`]);z(n.localizationKey,`${t}.localizationKey`),z(n.fallbackText,`${t}.fallbackText`);let r=new Set,i=De(n.arguments,`${t}.arguments`);i.length>8&&V(`${t}.arguments`,`must contain at most 8 entries`),i.forEach((e,n)=>{let i=`${t}.arguments[${String(n)}]`,a=j(e,i,[`name`,`value`]),o=z(a.name,`${i}.name`);z(a.value,`${i}.value`),r.has(o)&&V(`${i}.name`,`is duplicated`),r.add(o)})}else if(n===`value`){let n=j(e,t,[`kind`,`labelKey`,`fallbackLabel`,`value`,`unitKey`,`fallbackUnit`]);z(n.labelKey,`${t}.labelKey`),z(n.fallbackLabel,`${t}.fallbackLabel`),z(n.value,`${t}.value`),F(n.unitKey,`${t}.unitKey`,z),F(n.fallbackUnit,`${t}.fallbackUnit`,z)}else{let n=j(e,t,[`kind`,`texture`,`altKey`,`fallbackAlt`]),r=j(n.texture,`${t}.texture`,[`asset`,`contentHash`]);z(r.asset,`${t}.texture.asset`),z(r.contentHash,`${t}.texture.contentHash`),z(n.altKey,`${t}.altKey`),z(n.fallbackAlt,`${t}.fallbackAlt`)}}function fe(e,t){if(B(M(e,t).kind,`${t}.kind`,[`system`,`asset`])===`system`)z(j(e,t,[`kind`,`family`]).family,`${t}.family`);else{let n=j(e,t,[`kind`,`asset`,`contentHash`,`family`]);z(n.asset,`${t}.asset`),z(n.contentHash,`${t}.contentHash`),z(n.family,`${t}.family`)}}function pe(e,t,n){if(e===`emit`){let e=j(t,n,[`op`,`signalId`,`descriptor`]);z(e.signalId,`${n}.signalId`),me(e.descriptor,`${n}.descriptor`)}else A(e,t,n,me,he)}function me(e,t){let n=j(e,t,[`anchor`,`sprite`,`ratePerSecond`,`burstCount`,`lifetimeSeconds`,`velocityMin`,`velocityMax`,`acceleration`,`sizeCurve`,`colorCurve`,`flipbookFramesPerSecond`,`seed`,`maxParticles`,`visible`]);Te(n.anchor,`${t}.anchor`,!1),ge(n.sprite,`${t}.sprite`),R(n.ratePerSecond,`${t}.ratePerSecond`,0,1e4),R(n.flipbookFramesPerSecond,`${t}.flipbookFramesPerSecond`,0,120),I(n.burstCount,`${t}.burstCount`),Ae(n.lifetimeSeconds,`${t}.lifetimeSeconds`,2,0,Number.MAX_VALUE),P(n.velocityMin,`${t}.velocityMin`),P(n.velocityMax,`${t}.velocityMax`),P(n.acceleration,`${t}.acceleration`),_e(n.sizeCurve,`${t}.sizeCurve`),ve(n.colorCurve,`${t}.colorCurve`),Pe(n.seed,`${t}.seed`),I(n.maxParticles,`${t}.maxParticles`),Be(n.visible,`${t}.visible`)}function he(e,t){let n=j(e,t,[`anchor`,`sprite`,`ratePerSecond`,`burstCount`,`lifetimeSeconds`,`velocityMin`,`velocityMax`,`acceleration`,`sizeCurve`,`colorCurve`,`flipbookFramesPerSecond`,`maxParticles`,`visible`]);F(n.anchor,`${t}.anchor`,(e,t)=>Te(e,t,!1)),F(n.sprite,`${t}.sprite`,ge),F(n.ratePerSecond,`${t}.ratePerSecond`,Re),F(n.burstCount,`${t}.burstCount`,I),F(n.lifetimeSeconds,`${t}.lifetimeSeconds`,(e,t)=>Ae(e,t,2,0,60)),F(n.velocityMin,`${t}.velocityMin`,P),F(n.velocityMax,`${t}.velocityMax`,P),F(n.acceleration,`${t}.acceleration`,P),F(n.sizeCurve,`${t}.sizeCurve`,_e),F(n.colorCurve,`${t}.colorCurve`,ve),F(n.flipbookFramesPerSecond,`${t}.flipbookFramesPerSecond`,(e,t)=>R(e,t,0,120)),F(n.maxParticles,`${t}.maxParticles`,I),F(n.visible,`${t}.visible`,Be)}function ge(e,t){let n=j(e,t,[`asset`,`contentHash`,`frameCount`]);z(n.asset,`${t}.asset`),z(n.contentHash,`${t}.contentHash`),L(n.frameCount,`${t}.frameCount`,1,65535)}function _e(e,t){let n=De(e,t);(n.length<2||n.length>8)&&V(t,`must contain 2 to 8 keys`);let r=-1;n.forEach((e,n)=>{let i=`${t}[${String(n)}]`,a=j(e,i,[`age`,`value`]),o=R(a.age,`${i}.age`,0,1);Re(a.value,`${i}.value`),o<=r&&V(`${i}.age`,`must be strictly increasing`),r=o}),(M(n[0],`${t}[0]`).age!==0||M(n[n.length-1],`${t}[${String(n.length-1)}]`).age!==1)&&V(t,`must start at age 0 and end at age 1`)}function ve(e,t){let n=De(e,t);(n.length<2||n.length>8)&&V(t,`must contain 2 to 8 keys`);let r=-1;n.forEach((e,n)=>{let i=`${t}[${String(n)}]`,a=j(e,i,[`age`,`color`]),o=R(a.age,`${i}.age`,0,1);ke(a.color,`${i}.color`),o<=r&&V(`${i}.age`,`must be strictly increasing`),r=o}),(M(n[0],`${t}[0]`).age!==0||M(n[n.length-1],`${t}[${String(n.length-1)}]`).age!==1)&&V(t,`must start at age 0 and end at age 1`)}function ye(e,t,n){A(e,t,n,be,xe)}function be(e,t){let n=j(e,t,[`title`,`corner`,`refreshIntervalMs`,`maxFrameTimeSamples`,`visible`]);z(n.title,`${t}.title`),B(n.corner,`${t}.corner`,[`topLeft`,`topRight`,`bottomLeft`,`bottomRight`]),L(n.refreshIntervalMs,`${t}.refreshIntervalMs`,100,5e3),L(n.maxFrameTimeSamples,`${t}.maxFrameTimeSamples`,1,240),Be(n.visible,`${t}.visible`)}function xe(e,t){let n=j(e,t,[`title`,`corner`,`refreshIntervalMs`,`maxFrameTimeSamples`,`visible`]);F(n.title,`${t}.title`,z),F(n.corner,`${t}.corner`,(e,t)=>B(e,t,[`topLeft`,`topRight`,`bottomLeft`,`bottomRight`])),F(n.refreshIntervalMs,`${t}.refreshIntervalMs`,(e,t)=>L(e,t,100,5e3)),F(n.maxFrameTimeSamples,`${t}.maxFrameTimeSamples`,(e,t)=>L(e,t,1,240)),F(n.visible,`${t}.visible`,Be)}function Se(e,t,n){if(e===`create`){let e=j(t,n,[`op`,`handle`,`descriptor`]);Ne(e.handle,`${n}.handle`);let r=j(e.descriptor,`${n}.descriptor`,[`target`,`asset`,`contentHash`,`tickDurationMillis`,`controller`]);Ne(r.target,`${n}.descriptor.target`),z(r.asset,`${n}.descriptor.asset`),z(r.contentHash,`${n}.descriptor.contentHash`),I(r.tickDurationMillis,`${n}.descriptor.tickDurationMillis`),Ce(r.controller,`${n}.descriptor.controller`)}else if(e===`update`){let e=j(t,n,[`op`,`handle`,`controller`]);Ne(e.handle,`${n}.handle`),Ce(e.controller,`${n}.controller`)}else e===`destroy`?Ne(j(t,n,[`op`,`handle`]).handle,`${n}.handle`):V(`${n}.op`,`is unsupported for animation`)}function Ce(e,t){let n=j(e,t,[`entity`,`graphId`,`graphVersion`,`stateId`,`revision`,`controllerTick`,`motion`,`transition`,`transitionFact`]);Pe(n.entity,`${t}.entity`),z(n.graphId,`${t}.graphId`),I(n.graphVersion,`${t}.graphVersion`),z(n.stateId,`${t}.stateId`),Pe(n.revision,`${t}.revision`),Pe(n.controllerTick,`${t}.controllerTick`),we(n.motion,`${t}.motion`),F(n.transition,`${t}.transition`,(e,t)=>{let n=j(e,t,[`transitionId`,`fromStateId`,`toStateId`,`elapsedTicks`,`durationTicks`,`targetMotion`]);z(n.transitionId,`${t}.transitionId`),z(n.fromStateId,`${t}.fromStateId`),z(n.toStateId,`${t}.toStateId`),I(n.elapsedTicks,`${t}.elapsedTicks`),I(n.durationTicks,`${t}.durationTicks`),we(n.targetMotion,`${t}.targetMotion`)}),F(n.transitionFact,`${t}.transitionFact`,(e,t)=>{let n=j(e,t,[`controllerTick`,`transitionId`,`fromStateId`,`toStateId`,`moment`,`durationTicks`]);Pe(n.controllerTick,`${t}.controllerTick`),z(n.transitionId,`${t}.transitionId`),z(n.fromStateId,`${t}.fromStateId`),z(n.toStateId,`${t}.toStateId`),B(n.moment,`${t}.moment`,[`started`,`completed`]),I(n.durationTicks,`${t}.durationTicks`)})}function we(e,t){let n=j(e,t,[`clipA`,`clipB`,`blendWeightMilli`,`speedMilli`]);z(n.clipA,`${t}.clipA`),F(n.clipB,`${t}.clipB`,z),Fe(n.blendWeightMilli,`${t}.blendWeightMilli`),Fe(n.speedMilli,`${t}.speedMilli`)}function A(e,t,n,r,i){if(e===`create`){let e=j(t,n,[`op`,`handle`,`descriptor`]);Ne(e.handle,`${n}.handle`),r(e.descriptor,`${n}.descriptor`)}else if(e===`update`){let e=j(t,n,[`op`,`handle`,`patch`]);Ne(e.handle,`${n}.handle`),i(e.patch,`${n}.patch`)}else e===`destroy`?Ne(j(t,n,[`op`,`handle`]).handle,`${n}.handle`):V(`${n}.op`,`is unsupported for retained presentation`)}function Te(e,t,n){let r=M(e,t),i=n?[`global2d`,`world3d`,`entityAttached`]:[`world`,`entityAttached`],a=B(r.kind,`${t}.kind`,i);if(a===`global2d`)j(e,t,[`kind`]);else if(a===`world`||a===`world3d`)P(j(e,t,[`kind`,`position`]).position,`${t}.position`);else{let n=j(e,t,[`kind`,`entity`,`offset`]);Pe(n.entity,`${t}.entity`),P(n.offset,`${t}.offset`)}}function j(e,t,n){let r=M(e,t),i=new Set(n);return Object.keys(r).forEach(e=>{i.has(e)||V(`${t}.${e}`,`is unknown`)}),n.forEach(e=>{Object.hasOwn(r,e)||V(`${t}.${e}`,`is required`)}),r}function Ee(e,t,n,r){let i=M(e,t),a=new Set([...n,...r]);return Object.keys(i).forEach(e=>{a.has(e)||V(`${t}.${e}`,`is unknown`)}),n.forEach(e=>{Object.hasOwn(i,e)||V(`${t}.${e}`,`is required`)}),i}function M(e,t){return(typeof e!=`object`||!e||Array.isArray(e))&&V(t,`must be an object`),e}function De(e,t){return Array.isArray(e)||V(t,`must be an array`),e}function N(e,t,n){let r=De(e,t);return r.length!==n&&V(t,`must contain ${String(n)} values`),r}function P(e,t){return N(e,t,3).map((e,n)=>Ie(e,`${t}[${String(n)}]`))}function Oe(e,t){Ae(e,t,3,0,1)}function ke(e,t){Ae(e,t,4,0,1)}function Ae(e,t,n,r,i){return N(e,t,n).map((e,n)=>R(e,`${t}[${String(n)}]`,r,i))}function je(e,t,n,r){let i=De(e,t);return i.length!==n&&V(t,`must contain ${String(n)} values`),i.map((e,n)=>r?I(e,`${t}[${String(n)}]`):Ie(e,`${t}[${String(n)}]`))}function F(e,t,n){e!==null&&n(e,t)}function Me(e,t){F(e,t,Ne)}function Ne(e,t){return Pe(e,t)}function Pe(e,t){return L(e,t,0,Ve)}function I(e,t){return L(e,t,0,2**53-1)}function Fe(e,t){return(typeof e!=`number`||!Number.isSafeInteger(e))&&V(t,`must be a safe integer`),e}function L(e,t,n,r){let i=Fe(e,t);return(i<n||i>r)&&V(t,`must be in ${String(n)}..=${String(r)}`),i}function Ie(e,t){return(typeof e!=`number`||!Number.isFinite(e))&&V(t,`must be finite`),e}function Le(e,t){let n=Ie(e,t);return n<=0&&V(t,`must be positive`),n}function Re(e,t){let n=Ie(e,t);return n<0&&V(t,`must be non-negative`),n}function R(e,t,n,r){let i=Ie(e,t);return(i<n||i>r)&&V(t,`must be in ${String(n)}..=${String(r)}`),i}function ze(e,t){return typeof e!=`string`&&V(t,`must be a string`),e}function z(e,t){let n=ze(e,t);return n.trim()===``&&V(t,`must be non-empty`),n}function Be(e,t){return typeof e!=`boolean`&&V(t,`must be a boolean`),e}function B(e,t,n){let r=ze(e,t);return n.includes(r)||V(t,`must be one of ${n.join(`, `)}`),r}function V(e,t){throw new He(`${e} ${t}`)}var Ve,He,Ue=e((()=>{i(),Ve=9007199254740991,He=class extends Error{constructor(e){super(e),this.name=`ContractDecodeError`}}}));function We(e){e.schemaVersion!==1&&et(`composition.schemaVersion`,`must equal 1`),Ye(e.cameras,`composition.cameras`,4),Ye(e.targets,`composition.targets`,4),Ye(e.views,`composition.views`,8),Ye(e.presentations,`composition.presentations`,4);let t=new Map;for(let[n,r]of e.cameras.entries()){let e=`composition.cameras[${String(n)}]`;qe(r.id,`${e}.id`,t),Xe(r.pose.position,`${e}.pose.position`),Ze(r.pose.pitchDegrees,`${e}.pose.pitchDegrees`),Ze(r.pose.yawDegrees,`${e}.pose.yawDegrees`),Ge(r.projection,`${e}.projection`),t.set(r.id,r)}let n=new Map,r=0;for(let[t,i]of e.targets.entries()){let e=`composition.targets[${String(t)}]`;qe(i.id,`${e}.id`,n),Qe(i.revision,`${e}.revision`,1,2**53-1),Qe(i.width,`${e}.width`,1,tt),Qe(i.height,`${e}.height`,1,tt),i.color!==`rgba8_srgb`&&et(`${e}.color`,`must equal rgba8_srgb`),i.depth!==`depth24`&&i.depth!==`none`&&et(`${e}.depth`,`must equal depth24 or none`),i.sampling!==`linear`&&i.sampling!==`nearest`&&et(`${e}.sampling`,`must equal linear or nearest`),r=$e(r,i.width*i.height,`composition.targets`),r>8388608&&et(`composition.targets`,`aggregate pixels must not exceed ${String(nt)}`),n.set(i.id,i)}let i=new Set,a=new Set;for(let[r,o]of e.views.entries()){let e=`composition.views[${String(r)}]`;if(qe(o.id,`${e}.id`,i),Je(o.cameraId,`${e}.cameraId`),t.has(o.cameraId)||et(`${e}.cameraId`,`does not name an admitted camera ${JSON.stringify(o.cameraId)}`),Ke(o.viewport,`${e}.viewport`),Qe(o.order,`${e}.order`,0,65535),o.target.kind===`primary`)continue;o.target.kind!==`offscreen`&&et(`${e}.target.kind`,`must equal primary or offscreen`);let s=n.get(o.target.targetId);s===void 0&&et(`${e}.target.targetId`,`does not name an admitted target`),s.revision!==o.target.targetRevision&&et(`${e}.target.targetRevision`,`must equal the admitted target revision`),a.has(s.id)&&et(`${e}.target.targetId`,`already has a producing view`),a.add(s.id)}let o=new Set;for(let[t,r]of e.presentations.entries()){let e=`composition.presentations[${String(t)}]`;qe(r.id,`${e}.id`,o);let i=n.get(r.sourceTargetId);i===void 0&&et(`${e}.sourceTargetId`,`does not name an admitted target`),i.revision!==r.sourceTargetRevision&&et(`${e}.sourceTargetRevision`,`must equal the admitted target revision`),a.has(i.id)||et(`${e}.sourceTargetId`,`must have one producing view in the same composition`),r.destination.kind!==`primary`&&et(`${e}.destination.kind`,`must equal primary; render-target feedback is unsupported`),Ke(r.destination.viewport,`${e}.destination.viewport`),Qe(r.order,`${e}.order`,0,65535)}return e}function Ge(e,t){if(Ze(e.near,`${t}.near`),Ze(e.far,`${t}.far`),(e.near<=0||e.far<=e.near)&&et(t,`must have 0 < near < far`),e.kind===`perspective`){Ze(e.fovYDegrees,`${t}.fovYDegrees`),(e.fovYDegrees<=0||e.fovYDegrees>=180)&&et(`${t}.fovYDegrees`,`must be greater than 0 and less than 180`);return}if(e.kind===`orthographic`){Ze(e.verticalSize,`${t}.verticalSize`),e.verticalSize<=0&&et(`${t}.verticalSize`,`must be greater than 0`);return}et(`${t}.kind`,`must equal perspective or orthographic`)}function Ke(e,t){Ze(e.x,`${t}.x`),Ze(e.y,`${t}.y`),Ze(e.width,`${t}.width`),Ze(e.height,`${t}.height`),(e.x<0||e.y<0||e.width<=0||e.height<=0)&&et(t,`must have non-negative origin and positive extent`),(e.x+e.width>1||e.y+e.height>1)&&et(t,`must fit inside normalized destination bounds`)}function qe(e,t,n){Je(e,t),n.has(e)&&et(t,`duplicates ${JSON.stringify(e)}`)}function Je(e,t){/^[a-z][a-z0-9._-]{0,63}$/u.test(e)||et(t,`must be a lowercase stable identifier of at most 64 characters`)}function Ye(e,t,n){Array.isArray(e)||et(t,`must be an array`),e.length>n&&et(t,`must contain at most ${String(n)} entries`)}function Xe(e,t){(!Array.isArray(e)||e.length!==3)&&et(t,`must contain exactly 3 values`),e.forEach((e,n)=>Ze(e,`${t}[${String(n)}]`))}function Ze(e,t){Number.isFinite(e)||et(t,`must be finite`)}function Qe(e,t,n,r){(!Number.isSafeInteger(e)||e<n||e>r)&&et(t,`must be a safe integer in ${String(n)}..=${String(r)}`)}function $e(e,t,n){let r=e+t;return Number.isSafeInteger(r)||et(n,`aggregate size overflowed`),r}function et(e,t){throw new rt(e,t)}var tt,nt,rt,it=e((()=>{tt=2048,nt=8388608,rt=class extends Error{path;code=`invalid_view_composition`;constructor(e,t){super(`${e} ${t}`),this.path=e,this.name=`RendererViewCompositionValidationError`}}})),at=e((()=>{i(),a(),Ue(),it()}));function ot(e){let t=new Set(e.children);return e.kind===`staticMesh`?{...e,children:t,materialParameters:new Map(e.materialParameters)}:{...e,children:t}}function st(e,t){if(e.translation.some(e=>Math.abs(e)>16))throw new U(`${t}: viewmodel translation components must be within +/−16`);if(e.rotation.some(e=>Math.abs(e)>1))throw new U(`${t}: viewmodel rotation components must be within +/−1`);if(e.scale.some(e=>Math.abs(e)>64))throw new U(`${t}: viewmodel scale components must be within +/−64`)}function ct(e,t){lt([e.min,e.max],t)}function lt(e,t){if(e.some(e=>e.some(e=>Math.abs(e)>16)))throw new U(`${t}: viewmodel asset coordinates must be within +/−16`)}function ut(e){switch(e.kind){case`primitive`:return null;case`staticMesh`:return`staticMesh:${e.asset}`;case`animatedMesh`:return`animatedMesh:${e.asset}`;case`voxelObject`:return`voxelObject:${e.asset}`;case`sprite`:return`sprite:${e.sprite.asset}`}}function dt(){return{copiedNodeRecords:0,copiedLightRecords:0,copiedResourceRecords:0,sharedDefinitionRecords:0}}function ft(e){return{handle:e.handle,parent:e.parent,light:H(e.light)}}function pt(e){let t={handle:e.handle,parent:e.parent,children:[...e.children].sort(Mt),layer:e.layer,transform:H(e.transform),visible:e.visible,metadata:H(e.metadata),material:H(e.material),meshPayload:H(e.meshPayload)};return e.kind===`primitive`?{...t,kind:`primitive`,node:H(e.node)}:e.kind===`staticMesh`?{...t,kind:`staticMesh`,asset:e.asset,instance:H(e.instance),materialParameters:[...e.materialParameters.entries()].sort(([e],[t])=>e-t).map(([e,t])=>({slot:e,parameters:H(t)}))}:e.kind===`animatedMesh`?{...t,kind:`animatedMesh`,asset:e.asset,instance:H(e.instance),playback:H(e.playback)}:e.kind===`voxelObject`?{...t,kind:`voxelObject`,asset:e.asset,instance:H(e.instance),frame:e.frame}:{...t,kind:`sprite`,sprite:H(e.sprite),frameUv:H(e.frameUv),frameSize:H(e.frameSize),renderOrder:e.renderOrder}}function mt(e,t){if(e.asset.length===0)throw new U(`${t}.asset must be non-empty`);if(e.runtimeFormat!==`glb`)throw new U(`${t}.runtimeFormat unsupported: ${e.runtimeFormat}`);let n=new Set;for(let r=0;r<e.clips.length;r+=1){let i=e.clips[r];if(i.id.length===0)throw new U(`${t}.clips[${r}].id must be non-empty`);if(n.has(i.id))throw new U(`${t}.clips duplicate clip ${i.id}`);n.add(i.id)}if(e.defaultClip!==null&&!n.has(e.defaultClip))throw new U(`${t}.defaultClip ${e.defaultClip} is not declared`);let r=new Set;for(let n=0;n<e.materialSlots.length;n+=1){let i=kt(e.materialSlots[n].slot,`${t}.materialSlots[${n}].slot`);if(r.has(i))throw new U(`${t}.materialSlots duplicate slot ${i}`);r.add(i)}}function ht(e,t){if(e.asset.length===0||e.contentHash.length===0)throw new U(`${t} asset and contentHash must be non-empty`);if(e.meshes.length===0||e.meshes.length>8193)throw new U(`${t}.meshes must contain 1..=8193 entries`);if(e.frames.length===0||e.frames.length>8193)throw new U(`${t}.frames must contain 1..=8193 entries`);let n=new Set;e.materialSlots.forEach((e,r)=>{let i=kt(e.slot,`${t}.materialSlots[${r}].slot`);if(n.has(i))throw new U(`${t}.materialSlots duplicate slot ${i}`);n.add(i)});let r=0,i=0;if(e.meshes.forEach((e,a)=>{wt(e.payload,`${t}.meshes[${a}].payload`),r+=e.payload.layout.vertexCount,i+=e.payload.layout.indexCount,e.payload.groups.forEach((e,r)=>{if(!n.has(e.materialSlot))throw new U(`${t}.meshes[${a}].payload.groups[${r}] uses unbound slot ${e.materialSlot}`)})}),r>8e6||i>12e6)throw new U(`${t}.meshes exceeds aggregate vertex/index work limits`);let a=new Set;e.frames.forEach((n,r)=>{if(n.id.length===0||a.has(n.id))throw new U(`${t}.frames[${r}].id must be non-empty and unique`);a.add(n.id),gt(e,r,`${t}.frames[${r}]`)})}function gt(e,t,n){let r=kt(t,n),i=e.frames[r];if(i===void 0||e.meshes[i.mesh]===void 0)throw new U(`${n} ${r} is outside voxel object ${e.asset} frame resources`)}function _t(e,t,n){let r=new Set(e.materialSlots.map(e=>e.slot)),i=new Set;t.forEach((e,t)=>{if(i.has(e.slot))throw new U(`${n}[${t}] duplicates slot ${e.slot}`);if(!r.has(e.slot))throw new U(`${n}[${t}] uses unbound slot ${e.slot}`);i.add(e.slot)})}function vt(e,t,n){if(t.kind===`play`){if(!e.clips.some(e=>e.id===t.clip))throw new U(`${n}.clip ${t.clip} is not defined on ${e.asset}`);if(t.speed<=0)throw new U(`${n}.speed must be positive`);if(t.weight<0||t.weight>1)throw new U(`${n}.weight must be in 0..=1`)}}function yt(e,t){if(bt(e.color,`${t}.color`),Ct(e.intensity,`${t}.intensity`),e.intensity>1e4)throw new U(`${t}.intensity must not exceed ${String(r)}`);if(e.kind===`directional`){xt(e.direction,`${t}.direction`);return}if(e.kind===`point`||e.kind===`spot`){if(e.position.forEach((e,n)=>St(e,`${t}.position[${n}]`)),e.range!==null&&(!Number.isFinite(e.range)||e.range<=0))throw new U(`${t}.range must be null or finite and positive`);Ct(e.decay,`${t}.decay`)}if(e.kind===`spot`){if(xt(e.direction,`${t}.direction`),!Number.isFinite(e.outerAngleRadians)||e.outerAngleRadians<=0||e.outerAngleRadians>Math.PI/2)throw new U(`${t}.outerAngleRadians must be in (0, pi/2]`);if(!Number.isFinite(e.penumbra)||e.penumbra<0||e.penumbra>1)throw new U(`${t}.penumbra must be in 0..=1`)}}function bt(e,t){e.forEach((e,n)=>{if(!Number.isFinite(e)||e<0||e>1)throw new U(`${t}[${n}] must be finite and in 0..=1`)})}function xt(e,t){if(e.forEach((e,n)=>St(e,`${t}[${n}]`)),e.reduce((e,t)=>e+t*t,0)<=2**-52)throw new U(`${t} must be non-zero`)}function St(e,t){if(!Number.isFinite(e))throw new U(`${t} must be finite`)}function Ct(e,t){if(!Number.isFinite(e)||e<0)throw new U(`${t} must be finite and non-negative`)}function wt(e,t){let n=kt(e.layout.vertexCount,`${t}.layout.vertexCount`),r=kt(e.layout.indexCount,`${t}.layout.indexCount`),i=Dt(e,`position`,t),a=Dt(e,`normal`,t),o=e.layout.attributes.find(e=>e.name===`uv`)!==void 0;if(e.source.kind===`inline`){if(Ot(e.source.positions,n*i,`${t}.source.positions`),Ot(e.source.normals,n*a,`${t}.source.normals`),o!==(e.source.uvs!==void 0))throw new U(`${t}.source.uvs must match the declared uv attribute`);if(e.source.uvs!==void 0&&(Ot(e.source.uvs,n*2,`${t}.source.uvs`),e.source.uvs.forEach((e,n)=>St(e,`${t}.source.uvs[${n}]`)),(e.provenance===`voxelChunk`||e.provenance===`voxelObject`)&&e.source.uvs.some(e=>Math.abs(e)>16777216)))throw new U(`${t}.source.uvs exceeds the voxel tile-coordinate range`);Ot(e.source.indices,r,`${t}.source.indices`),e.source.indices.forEach((e,r)=>{let i=kt(e,`${t}.source.indices[${r}]`);if(i>=n)throw new U(`${t}.source.indices[${r}] ${i} is out of range for ${n} vertices`)})}else if(e.source.kind===`sharedBuffer`){if(kt(e.source.buffer,`${t}.source.buffer`),kt(e.source.positionsByteOffset,`${t}.source.positionsByteOffset`),kt(e.source.normalsByteOffset,`${t}.source.normalsByteOffset`),o!==(e.source.uvsByteOffset!==void 0))throw new U(`${t}.source.uvsByteOffset must match the declared uv attribute`);e.source.uvsByteOffset!==void 0&&kt(e.source.uvsByteOffset,`${t}.source.uvsByteOffset`),kt(e.source.indicesByteOffset,`${t}.source.indicesByteOffset`)}else{let s=/^sha256:([0-9a-f]{64})$/u.exec(e.source.contentHash)?.[1];if(s===void 0||e.source.resource!==`mesh-resource/${s}`)throw new U(`${t}.source has an invalid content-addressed identity`);let c=kt(e.source.byteLength,`${t}.source.byteLength`);if(c<16||c>64*1024*1024)throw new U(`${t}.source.byteLength exceeds the resource bounds`);let l=kt(e.source.positionsByteOffset,`${t}.source.positionsByteOffset`),u=kt(e.source.normalsByteOffset,`${t}.source.normalsByteOffset`),d=e.source.uvsByteOffset===void 0?void 0:kt(e.source.uvsByteOffset,`${t}.source.uvsByteOffset`);if(o!==(d!==void 0)||e.source.encoding===`packedStreamsLeV1`&&d!==void 0||e.source.encoding===`packedStreamsLeV2`&&d===void 0)throw new U(`${t}.source encoding and uv stream must agree`);let f=kt(e.source.indicesByteOffset,`${t}.source.indicesByteOffset`);if([l,u,d,f].filter(e=>e!==void 0).some(e=>e<16||e%4!=0))throw new U(`${t}.source offsets must be aligned after the header`);let p=l+n*i*4,m=u+n*a*4,h=d===void 0?m:d+n*2*4,g=f+r*4;if(p>c||m>c||h>c||g>c||p>u||(d===void 0?m:h)>f||d!==void 0&&m>d)throw new U(`${t}.source streams exceed or overlap the resource`)}for(let n=0;n<e.groups.length;n+=1){let i=e.groups[n],a=kt(i.start,`${t}.groups[${n}].start`),o=kt(i.count,`${t}.groups[${n}].count`);if(kt(i.materialSlot,`${t}.groups[${n}].materialSlot`),a+o>r)throw new U(`${t}.groups[${n}] window [${a}, ${a+o}) exceeds indexCount ${r}`);let s=n===0?0:e.groups[n-1].start+e.groups[n-1].count;if(a!==s)throw new U(`${t}.groups[${n}] starts at ${a}; contiguous coverage requires ${s}`)}if(e.groups.length>0){let n=e.groups[e.groups.length-1];if(n.start+n.count!==r)throw new U(`${t}.groups must cover all ${r} indices`)}}function Tt(e){switch(e.op){case`create`:case`createLight`:case`createStaticMeshInstance`:case`createAnimatedMeshInstance`:case`createVoxelObjectInstance`:case`createSprite`:Et(e.handle,`${e.op}.handle`),e.parent!==null&&Et(e.parent,`${e.op}.parent`);return;case`update`:case`destroy`:case`replaceMeshPayload`:case`updateLight`:case`setMaterialInstanceParameters`:case`setAnimatedMeshPlayback`:case`setVoxelObjectFrame`:case`updateSprite`:Et(e.handle,`${e.op}.handle`);return;case`defineMaterial`:case`defineTexture`:case`defineSpriteAtlas`:case`defineStaticMesh`:case`defineAnimatedMesh`:case`defineVoxelObject`:case`releaseVoxelObject`:return}}function Et(e,t){if(!Number.isSafeInteger(e)||e<0)throw new U(`${t} must be a non-negative JSON-safe integer`)}function Dt(e,t,n){let r=e.layout.attributes.find(e=>e.name===t);if(r===void 0)throw new U(`${n}.layout.attributes missing ${t}`);return kt(r.components,`${n}.layout.attributes.${t}.components`)}function Ot(e,t,n){if(e.length!==t)throw new U(`${n} expected length ${t}, got ${e.length}`)}function kt(e,t){if(!Number.isInteger(e)||e<0)throw new U(`${t} must be a non-negative integer`);return e}function At(e){return[...e.keys()].sort(Mt)}function jt(e){return[...e.values()].map(e=>H(e)).sort((e,t)=>e.id.localeCompare(t.id))}function Mt(e,t){return e-t}function H(e){return e===void 0?e:JSON.parse(JSON.stringify(e))}var Nt,U,Pt,Ft=e((()=>{at(),U=class extends Error{constructor(e){super(e),this.name=`RenderProjectionError`}},Pt=class{#e=new Map;#t=new Map;#n=new Map;#r=new Map;#i=new Map;#a=new Map;#o=new Map;#s=new Map;#c=dt();#l=!1;applyFrame(e){let{staged:t,instructions:n}=this.#q(e);return this.#K(t),n}validateFrame(e){return this.#q(e).instructions}applyDiff(e){switch(Tt(e),e.op){case`create`:return[this.#u(e)];case`update`:return[this.#d(e)];case`destroy`:return this.#f(e.handle);case`replaceMeshPayload`:return[this.#p(e)];case`createLight`:return[this.#m(e)];case`updateLight`:return[this.#h(e)];case`defineMaterial`:return[this.#g(e.material)];case`setMaterialInstanceParameters`:return[this.#S(e)];case`defineTexture`:return[this.#_(e.texture)];case`defineSpriteAtlas`:return[this.#v(e.atlas)];case`defineStaticMesh`:return[this.#y(e.asset)];case`defineAnimatedMesh`:return[this.#b(e.asset)];case`defineVoxelObject`:return[this.#T(e.asset)];case`releaseVoxelObject`:return[this.#E(e.asset)];case`createStaticMeshInstance`:return[this.#x(e)];case`createAnimatedMeshInstance`:return[this.#C(e)];case`setAnimatedMeshPlayback`:return[this.#w(e)];case`createVoxelObjectInstance`:return[this.#D(e)];case`setVoxelObjectFrame`:return[this.#O(e)];case`createSprite`:return[this.#k(e)];case`updateSprite`:return[this.#A(e)];default:throw new U(`unsupported render diff op ${JSON.stringify(e.op)}`)}}has(e){return this.#e.has(e)||this.#t.has(e)}get handleCount(){return this.#e.size+this.#t.size}lastFrameStagingStatistics(){return{...this.#c}}node(e){let t=this.#e.get(e);return t===void 0?void 0:pt(t)}light(e){let t=this.#t.get(e);return t===void 0?void 0:ft(t)}materialDescriptor(e){return H(this.#n.get(e))}textureDescriptor(e){return H(this.#r.get(e))}spriteAtlas(e){return H(this.#i.get(e))}staticMesh(e){return H(this.#a.get(e)?.asset)}animatedMesh(e){return H(this.#o.get(e)?.asset)}voxelObject(e){return H(this.#s.get(e)?.asset)}staticMeshRefCount(e){return this.#a.get(e)?.refCount??0}animatedMeshRefCount(e){return this.#o.get(e)?.refCount??0}voxelObjectRefCount(e){return this.#s.get(e)?.refCount??0}snapshot(){return{nodes:At(this.#e).map(e=>pt(this.#R(e,`snapshot`))),lights:At(this.#t).map(e=>ft(this.#z(e,`snapshot`))),materials:jt(this.#n),textures:jt(this.#r),spriteAtlases:jt(this.#i),staticMeshes:[...this.#a.values()].map(e=>H(e.asset)).sort((e,t)=>e.asset.localeCompare(t.asset)),animatedMeshes:[...this.#o.values()].map(e=>H(e.asset)).sort((e,t)=>e.asset.localeCompare(t.asset)),voxelObjects:[...this.#s.values()].map(e=>H(e.asset)).sort((e,t)=>e.asset.localeCompare(t.asset))}}pickMesh(e){let t=this.#e.get(e),n=t?.meshPayload;if(!(t===void 0||n==null))return{handle:e,provenance:n.provenance,sourceEntity:t.metadata.sourceEntity,sourceSceneNode:t.metadata.sourceSceneNode}}pickSprite(e){let t=this.#e.get(e);if(t?.kind!==`sprite`)return;let n=t.sprite.attachment;return{handle:e,sourceEntity:n.sourceEntity,sourceSceneNode:n.sourceSceneNode,asset:t.sprite.asset,attachmentPoint:n.attachmentPoint}}#u(e){this.#I(e.handle,`create`);let t=this.#L(e.parent,`create.parent`),n=H(e.node),r={handle:e.handle,parent:t,children:new Set,kind:`primitive`,layer:t===null?n.layer:this.#R(t,`create.parent`).layer,transform:H(n.transform),visible:n.visible,metadata:H(n.metadata),material:H(n.material),meshPayload:null,node:n};return this.#P(r,`create`),this.#N(r),{op:`upsertNode`,node:pt(r)}}#d(e){this.#R(e.handle,`update`).layer===`viewmodel`&&e.transform!==null&&st(e.transform,`update.transform`);let t=this.#B(e.handle,`update`);return e.transform!==null&&(t.transform=H(e.transform),t.kind===`primitive`?t.node={...t.node,transform:H(e.transform)}:t.kind===`staticMesh`||t.kind===`animatedMesh`||t.kind===`voxelObject`?t.instance={...t.instance,transform:H(e.transform)}:t.sprite={...t.sprite,transform:H(e.transform)}),e.material!==null&&(t.material=H(e.material),t.kind===`primitive`&&(t.node={...t.node,material:H(e.material)})),e.visible!==null&&(t.visible=e.visible,t.kind===`primitive`?t.node={...t.node,visible:e.visible}:t.kind===`staticMesh`||t.kind===`animatedMesh`||t.kind===`voxelObject`?t.instance={...t.instance,visible:e.visible}:t.sprite={...t.sprite,visible:e.visible}),e.metadata!==null&&(t.metadata=H(e.metadata),t.kind===`primitive`?t.node={...t.node,metadata:H(e.metadata)}:t.kind===`staticMesh`||t.kind===`animatedMesh`||t.kind===`voxelObject`?t.instance={...t.instance,metadata:H(e.metadata)}:t.sprite={...t.sprite,metadata:H(e.metadata)}),{op:`upsertNode`,node:pt(t)}}#f(e){let t=this.#t.get(e);if(t!==void 0)return this.#t.delete(e),t.parent!==null&&this.#B(t.parent,`destroyLight.parent`).children.delete(e),[{op:`removeLight`,handle:e}];let n=this.#R(e,`destroy`),r=[];for(let e of[...n.children].sort(Mt))r.push(...this.#f(e));if(this.#e.delete(e),n.parent!==null&&this.#B(n.parent,`destroy.parent`).children.delete(e),n.kind===`staticMesh`){let e=this.#H(n.asset);e!==void 0&&--e.refCount}else if(n.kind===`animatedMesh`){let e=this.#U(n.asset);e!==void 0&&--e.refCount}else if(n.kind===`voxelObject`){let e=this.#W(n.asset);e!==void 0&&--e.refCount}return r.push({op:`removeNode`,handle:e}),r}#p(e){let t=this.#R(e.handle,`replaceMeshPayload`);if(t.kind!==`primitive`||t.node.geometry.kind===`group`)throw new U(`replaceMeshPayload: handle ${e.handle} is not a primitive mesh`);wt(e.payload,`replaceMeshPayload.payload`),t.layer===`viewmodel`&&ct(e.payload.bounds,`replaceMeshPayload.payload.bounds`);let n=this.#B(e.handle,`replaceMeshPayload`);if(n.kind!==`primitive`)throw new U(`replaceMeshPayload: handle ${e.handle} is not a primitive mesh`);return n.meshPayload=H(e.payload),{op:`upsertNode`,node:pt(n)}}#m(e){if(this.#I(e.handle,`createLight`),this.#t.size>=256)throw new U(`createLight: retained light quota 256 exceeded`);let t=this.#L(e.parent,`createLight.parent`);if(t!==null&&this.#R(t,`createLight.parent`).layer===`viewmodel`)throw new U(`createLight: camera-relative presentation uses the backend-owned neutral light rig`);yt(e.light,`createLight.light`);let n={handle:e.handle,parent:t,light:H(e.light)};return this.#t.set(e.handle,n),t!==null&&this.#B(t,`createLight.parent`).children.add(e.handle),{op:`upsertLight`,light:ft(n)}}#h(e){let t=this.#z(e.handle,`updateLight`);if(yt(e.light,`updateLight.light`),t.light.kind!==e.light.kind)throw new U(`updateLight: handle ${e.handle} cannot change kind from ${t.light.kind} to ${e.light.kind}`);let n=this.#V(e.handle,`updateLight`);return n.light=H(e.light),{op:`upsertLight`,light:ft(n)}}#g(e){return this.#n.set(e.id,H(e)),{op:`defineMaterial`,material:H(e)}}#_(e){return this.#r.set(e.id,H(e)),{op:`defineTexture`,texture:H(e)}}#v(e){return this.#i.set(e.id,H(e)),{op:`defineSpriteAtlas`,atlas:H(e)}}#y(e){wt(e.payload,`defineStaticMesh(${e.asset}).payload`);let t=this.#a.get(e.asset);if(t!==void 0&&t.refCount>0)throw new U(`defineStaticMesh: asset ${e.asset} is in use by ${t.refCount} instance(s)`);return this.#a.set(e.asset,{asset:H(e),refCount:0}),{op:`defineStaticMesh`,asset:H(e)}}#b(e){mt(e,`defineAnimatedMesh(${e.asset})`);let t=this.#o.get(e.asset);if(t!==void 0&&t.refCount>0)throw new U(`defineAnimatedMesh: asset ${e.asset} is in use by ${t.refCount} instance(s)`);return this.#o.set(e.asset,{asset:H(e),refCount:0}),{op:`defineAnimatedMesh`,asset:H(e)}}#x(e){this.#I(e.handle,`createStaticMeshInstance`);let t=this.#a.get(e.instance.asset);if(t===void 0)throw new U(`createStaticMeshInstance: undefined static mesh asset ${e.instance.asset}`);let n=this.#L(e.parent,`createStaticMeshInstance.parent`),r=H(e.instance),i=new Set(t.asset.materialSlots.map(e=>e.slot));for(let e of r.materialOverrides)if(!i.has(e.slot))throw new U(`createStaticMeshInstance: override for unbound slot ${e.slot} on ${r.asset}`);let a={handle:e.handle,parent:n,children:new Set,kind:`staticMesh`,layer:n===null?`scene`:this.#R(n,`createStaticMeshInstance.parent`).layer,transform:H(r.transform),visible:r.visible,metadata:H(r.metadata),material:null,meshPayload:H(t.asset.payload),asset:r.asset,instance:r,materialParameters:new Map};return this.#P(a,`createStaticMeshInstance`),this.#H(r.asset).refCount+=1,this.#N(a),{op:`upsertNode`,node:pt(a)}}#S(e){let t=this.#R(e.handle,`setMaterialInstanceParameters`);if(t.kind!==`staticMesh`)throw new U(`setMaterialInstanceParameters: handle ${e.handle} is not a static mesh`);let n=this.#a.get(t.asset);if(n===void 0||!n.asset.materialSlots.some(t=>t.slot===e.slot))throw new U(`setMaterialInstanceParameters: unbound slot ${e.slot} on ${t.asset}`);let r=this.#B(e.handle,`setMaterialInstanceParameters`);if(r.kind!==`staticMesh`)throw new U(`setMaterialInstanceParameters: handle ${e.handle} is not a static mesh`);return e.parameters===null?r.materialParameters.delete(e.slot):r.materialParameters.set(e.slot,H(e.parameters)),{op:`upsertNode`,node:pt(r)}}#C(e){this.#I(e.handle,`createAnimatedMeshInstance`);let t=this.#o.get(e.instance.asset);if(t===void 0)throw new U(`createAnimatedMeshInstance: undefined animated mesh asset ${e.instance.asset}`);e.instance.playback!==null&&vt(t.asset,e.instance.playback,`createAnimatedMeshInstance.playback`);let n=this.#L(e.parent,`createAnimatedMeshInstance.parent`),r=H(e.instance),i={handle:e.handle,parent:n,children:new Set,kind:`animatedMesh`,layer:n===null?`scene`:this.#R(n,`createAnimatedMeshInstance.parent`).layer,transform:H(r.transform),visible:r.visible,metadata:H(r.metadata),material:null,meshPayload:null,asset:r.asset,instance:r,playback:H(r.playback)};return this.#P(i,`createAnimatedMeshInstance`),this.#U(r.asset).refCount+=1,this.#N(i),{op:`upsertNode`,node:pt(i)}}#w(e){let t=this.#R(e.handle,`setAnimatedMeshPlayback`);if(t.kind!==`animatedMesh`)throw new U(`setAnimatedMeshPlayback: handle ${e.handle} is not an animated mesh`);let n=this.#o.get(t.asset);if(n===void 0)throw new U(`setAnimatedMeshPlayback: missing animated mesh asset ${t.asset}`);vt(n.asset,e.playback,`setAnimatedMeshPlayback.playback`);let r=this.#B(e.handle,`setAnimatedMeshPlayback`);if(r.kind!==`animatedMesh`)throw new U(`setAnimatedMeshPlayback: handle ${e.handle} is not an animated mesh`);return r.playback=H(e.playback),r.instance={...r.instance,playback:H(e.playback)},{op:`upsertNode`,node:pt(r)}}#T(e){ht(e,`defineVoxelObject(${e.asset})`);let t=this.#s.get(e.asset),n=[];if(t!==void 0)for(let t of this.#e.values()){if(t.kind!==`voxelObject`||t.asset!==e.asset)continue;gt(e,t.frame,`defineVoxelObject.liveInstance`),_t(e,t.instance.materialOverrides,`defineVoxelObject.liveInstance`);let r=e.meshes[e.frames[t.frame].mesh].payload;t.layer===`viewmodel`&&ct(r.bounds,`defineVoxelObject.liveInstance.bounds`),n.push({payload:r,handle:t.handle})}for(let e of n){let t=this.#B(e.handle,`defineVoxelObject.liveInstance`);if(t.kind!==`voxelObject`)throw new U(`defineVoxelObject.liveInstance: handle ${e.handle} is not a voxel object`);t.meshPayload=H(e.payload)}return this.#s.set(e.asset,{asset:H(e),refCount:t?.refCount??0}),{op:`defineVoxelObject`,asset:H(e)}}#E(e){let t=this.#s.get(e);if(t===void 0)throw new U(`releaseVoxelObject: undefined voxel object ${e}`);if(t.refCount!==0)throw new U(`releaseVoxelObject: ${e} is in use by ${t.refCount} instance(s)`);return this.#s.delete(e),{op:`releaseVoxelObject`,asset:e}}#D(e){this.#I(e.handle,`createVoxelObjectInstance`);let t=this.#s.get(e.instance.asset);if(t===void 0)throw new U(`createVoxelObjectInstance: undefined voxel object ${e.instance.asset}`);gt(t.asset,e.instance.frame,`createVoxelObjectInstance.frame`),_t(t.asset,e.instance.materialOverrides,`createVoxelObjectInstance.materialOverrides`);let n=this.#L(e.parent,`createVoxelObjectInstance.parent`),r=H(e.instance),i={handle:e.handle,parent:n,children:new Set,kind:`voxelObject`,layer:n===null?`scene`:this.#R(n,`createVoxelObjectInstance.parent`).layer,transform:H(r.transform),visible:r.visible,metadata:H(r.metadata),material:null,meshPayload:H(t.asset.meshes[t.asset.frames[r.frame].mesh].payload),asset:r.asset,instance:r,frame:r.frame};return this.#P(i,`createVoxelObjectInstance`),this.#W(r.asset).refCount+=1,this.#N(i),{op:`upsertNode`,node:pt(i)}}#O(e){let t=this.#R(e.handle,`setVoxelObjectFrame`);if(t.kind!==`voxelObject`)throw new U(`setVoxelObjectFrame: handle ${e.handle} is not a voxel object`);let n=this.#s.get(t.asset);if(n===void 0)throw new U(`setVoxelObjectFrame: missing voxel object ${t.asset}`);gt(n.asset,e.frame,`setVoxelObjectFrame.frame`);let r=n.asset.meshes[n.asset.frames[e.frame].mesh].payload;t.layer===`viewmodel`&&ct(r.bounds,`setVoxelObjectFrame.bounds`);let i=this.#B(e.handle,`setVoxelObjectFrame`);if(i.kind!==`voxelObject`)throw new U(`setVoxelObjectFrame: handle ${e.handle} is not a voxel object`);return i.frame=e.frame,i.instance={...i.instance,frame:e.frame},i.meshPayload=H(r),{op:`upsertNode`,node:pt(i)}}#k(e){this.#I(e.handle,`createSprite`);let t=this.#L(e.parent,`createSprite.parent`),n=H(e.sprite),r={handle:e.handle,parent:t,children:new Set,kind:`sprite`,layer:t===null?`scene`:this.#R(t,`createSprite.parent`).layer,transform:H(n.transform),visible:n.visible,metadata:H(n.metadata),material:null,meshPayload:null,sprite:n,frameUv:this.#j(n.asset,n.frame),frameSize:this.#M(n.asset,n.frame,n.size),renderOrder:n.renderOrder};return this.#P(r,`createSprite`),this.#N(r),{op:`upsertNode`,node:pt(r)}}#A(e){if(this.#R(e.handle,`updateSprite`).kind!==`sprite`)throw new U(`updateSprite: handle ${e.handle} is not a sprite`);let t=this.#B(e.handle,`updateSprite`);if(t.kind!==`sprite`)throw new U(`updateSprite: handle ${e.handle} is not a sprite`);return e.frame!==null&&(t.sprite={...t.sprite,frame:e.frame},t.frameUv=this.#j(t.sprite.asset,e.frame),t.frameSize=this.#M(t.sprite.asset,e.frame,t.sprite.size)),e.tint!==null&&(t.sprite={...t.sprite,tint:H(e.tint)}),e.renderOrder!==null&&(t.sprite={...t.sprite,renderOrder:e.renderOrder},t.renderOrder=e.renderOrder),e.visible!==null&&(t.visible=e.visible,t.sprite={...t.sprite,visible:e.visible}),{op:`upsertNode`,node:pt(t)}}#j(e,t){let n=this.#i.get(e)?.frames.find(e=>e.frame===t);return n===void 0?[0,0,1,1]:[n.uvMin[0],n.uvMin[1],n.uvMax[0],n.uvMax[1]]}#M(e,t,n){let r=this.#i.get(e)?.frames.find(e=>e.frame===t);return r?.size===void 0?[n[0],n[1]]:[r.size[0],r.size[1]]}#N(e){this.#e.set(e.handle,e),e.parent!==null&&this.#B(e.parent,`insert.parent`).children.add(e.handle)}#P(e,t){if(e.layer!==`viewmodel`)return;st(e.transform,`${t}.transform`),this.#F(e,t);let n=[...this.#e.values()].filter(e=>e.layer===`viewmodel`);if(n.length>=128)throw new U(`${t}: viewmodel node capacity 128 is exhausted`);let r=ut(e);if(r===null)return;let i=new Set(n.map(ut).filter(e=>e!==null));if(!i.has(r)&&i.size>=16)throw new U(`${t}: viewmodel asset capacity 16 is exhausted`)}#F(e,t){if(e.kind===`primitive`){e.node.geometry.kind===`line`&&lt([e.node.geometry.a,e.node.geometry.b],`${t}.geometry`),e.meshPayload!==null&&ct(e.meshPayload.bounds,`${t}.meshPayload.bounds`);return}if(e.kind===`animatedMesh`){let n=this.#o.get(e.asset);if(n===void 0)throw new U(`${t}: missing animated mesh asset ${e.asset}`);ct(n.asset.bounds,`${t}.asset.bounds`);return}if(e.kind===`sprite`){if(e.sprite.size.some(e=>e>16))throw new U(`${t}.sprite.size: viewmodel dimensions must not exceed 16`);return}e.meshPayload!==null&&ct(e.meshPayload.bounds,`${t}.asset.bounds`)}#I(e,t){if(this.#e.has(e)||this.#t.has(e))throw new U(`${t}: handle ${e} already exists`)}#L(e,t){return e!==null&&this.#R(e,t),e}#R(e,t){let n=this.#e.get(e);if(n===void 0)throw new U(`${t}: unknown handle ${e}`);return n}#z(e,t){let n=this.#t.get(e);if(n===void 0)throw new U(`${t}: unknown light handle ${e}`);return n}#B(e,t){let n=ot(this.#R(e,t));return this.#e.set(e,n),this.#l&&(this.#c.copiedNodeRecords+=1),n}#V(e,t){let n={...this.#z(e,t)};return this.#t.set(e,n),this.#l&&(this.#c.copiedLightRecords+=1),n}#H(e){let t=this.#a.get(e);if(t===void 0)return;let n={...t};return this.#a.set(e,n),this.#l&&(this.#c.copiedResourceRecords+=1),n}#U(e){let t=this.#o.get(e);if(t===void 0)return;let n={...t};return this.#o.set(e,n),this.#l&&(this.#c.copiedResourceRecords+=1),n}#W(e){let t=this.#s.get(e);if(t===void 0)return;let n={...t};return this.#s.set(e,n),this.#l&&(this.#c.copiedResourceRecords+=1),n}#G(){let e=new Nt;return e.#e=new Map(this.#e),e.#t=new Map(this.#t),e.#n=new Map(this.#n),e.#r=new Map(this.#r),e.#i=new Map(this.#i),e.#a=new Map(this.#a),e.#o=new Map(this.#o),e.#s=new Map(this.#s),e.#c={...dt(),sharedDefinitionRecords:this.#n.size+this.#r.size+this.#i.size+this.#a.size+this.#o.size+this.#s.size},e.#l=!0,e}#K(e){this.#e=e.#e,this.#t=e.#t,this.#n=e.#n,this.#r=e.#r,this.#i=e.#i,this.#a=e.#a,this.#o=e.#o,this.#s=e.#s,this.#c=e.#c,this.#l=!1}#q(e){let t=this.#G(),n=[];for(let r of e.ops)n.push(...t.applyDiff(r));return{staged:t,instructions:n}}},Nt=Pt})),It=e((()=>{Ft()}));function Lt(e){for(let t=e.length-1;t>=0;--t)if(e[t]>=65535)return!0;return!1}function Rt(e){return ArrayBuffer.isView(e)&&!(e instanceof DataView)}function zt(e){return document.createElementNS(`http://www.w3.org/1999/xhtml`,e)}function Bt(){let e=zt(`canvas`);return e.style.display=`block`,e}function Vt(...e){let t=`THREE.`+e.shift();oi?oi(`log`,t,...e):console.log(t,...e)}function Ht(e){let t=e[0];if(typeof t==`string`&&t.startsWith(`TSL:`)){let t=e[1];t&&t.isStackTrace?e[0]+=` `+t.getLocation():e[1]=`Stack trace not available. Enable "THREE.Node.captureStackTrace" to capture stack traces.`}return e}function W(...e){e=Ht(e);let t=`THREE.`+e.shift();if(oi)oi(`warn`,t,...e);else{let n=e[0];n&&n.isStackTrace?console.warn(n.getError(t)):console.warn(t,...e)}}function G(...e){e=Ht(e);let t=`THREE.`+e.shift();if(oi)oi(`error`,t,...e);else{let n=e[0];n&&n.isStackTrace?console.error(n.getError(t)):console.error(t,...e)}}function Ut(...e){let t=e.join(` `);t in ai||(ai[t]=!0,W(...e))}function Wt(e,t,n){return new Promise(function(r,i){function a(){switch(e.clientWaitSync(t,e.SYNC_FLUSH_COMMANDS_BIT,0)){case e.WAIT_FAILED:i();break;case e.TIMEOUT_EXPIRED:setTimeout(a,n);break;default:r()}}setTimeout(a,n)})}function Gt(){let e=Math.random()*4294967295|0,t=Math.random()*4294967295|0,n=Math.random()*4294967295|0,r=Math.random()*4294967295|0;return(li[e&255]+li[e>>8&255]+li[e>>16&255]+li[e>>24&255]+`-`+li[t&255]+li[t>>8&255]+`-`+li[t>>16&15|64]+li[t>>24&255]+`-`+li[n&63|128]+li[n>>8&255]+`-`+li[n>>16&255]+li[n>>24&255]+li[r&255]+li[r>>8&255]+li[r>>16&255]+li[r>>24&255]).toLowerCase()}function Kt(e,t,n){return Math.max(t,Math.min(n,e))}function qt(e,t){return(e%t+t)%t}function Jt(e,t,n){return(1-n)*e+n*t}function Yt(e,t){switch(t.constructor){case Float32Array:return e;case Uint32Array:return e/4294967295;case Uint16Array:return e/65535;case Uint8Array:return e/255;case Int32Array:return Math.max(e/2147483647,-1);case Int16Array:return Math.max(e/32767,-1);case Int8Array:return Math.max(e/127,-1);default:throw Error(`Invalid component type.`)}}function Xt(e,t){switch(t.constructor){case Float32Array:return e;case Uint32Array:return Math.round(e*4294967295);case Uint16Array:return Math.round(e*65535);case Uint8Array:return Math.round(e*255);case Int32Array:return Math.round(e*2147483647);case Int16Array:return Math.round(e*32767);case Int8Array:return Math.round(e*127);default:throw Error(`Invalid component type.`)}}function Zt(){let e={enabled:!0,workingColorSpace:Qr,spaces:{},convert:function(e,t,n){return this.enabled===!1||t===n||!t||!n?e:(this.spaces[t].transfer===`srgb`&&(e.r=Qt(e.r),e.g=Qt(e.g),e.b=Qt(e.b)),this.spaces[t].primaries!==this.spaces[n].primaries&&(e.applyMatrix3(this.spaces[t].toXYZ),e.applyMatrix3(this.spaces[n].fromXYZ)),this.spaces[n].transfer===`srgb`&&(e.r=$t(e.r),e.g=$t(e.g),e.b=$t(e.b)),e)},workingToColorSpace:function(e,t){return this.convert(e,this.workingColorSpace,t)},colorSpaceToWorking:function(e,t){return this.convert(e,t,this.workingColorSpace)},getPrimaries:function(e){return this.spaces[e].primaries},getTransfer:function(e){return e===``?$r:this.spaces[e].transfer},getToneMappingMode:function(e){return this.spaces[e].outputColorSpaceConfig.toneMappingMode||`standard`},getLuminanceCoefficients:function(e,t=this.workingColorSpace){return e.fromArray(this.spaces[t].luminanceCoefficients)},define:function(e){Object.assign(this.spaces,e)},_getMatrix:function(e,t,n){return e.copy(this.spaces[t].toXYZ).multiply(this.spaces[n].fromXYZ)},_getDrawingBufferColorSpace:function(e){return this.spaces[e].outputColorSpaceConfig.drawingBufferColorSpace},_getUnpackColorSpace:function(e=this.workingColorSpace){return this.spaces[e].workingColorSpaceConfig.unpackColorSpace},fromWorkingColorSpace:function(t,n){return Ut(`ColorManagement: .fromWorkingColorSpace() has been renamed to .workingToColorSpace().`),e.workingToColorSpace(t,n)},toWorkingColorSpace:function(t,n){return Ut(`ColorManagement: .toWorkingColorSpace() has been renamed to .colorSpaceToWorking().`),e.colorSpaceToWorking(t,n)}},t=[.64,.33,.3,.6,.15,.06],n=[.2126,.7152,.0722],r=[.3127,.329];return e.define({[Qr]:{primaries:t,whitePoint:r,transfer:$r,toXYZ:_i,fromXYZ:vi,luminanceCoefficients:n,workingColorSpaceConfig:{unpackColorSpace:Zr},outputColorSpaceConfig:{drawingBufferColorSpace:Zr}},[Zr]:{primaries:t,whitePoint:r,transfer:ei,toXYZ:_i,fromXYZ:vi,luminanceCoefficients:n,outputColorSpaceConfig:{drawingBufferColorSpace:Zr}}}),e}function Qt(e){return e<.04045?e*.0773993808:(e*.9478672986+.0521327014)**2.4}function $t(e){return e<.0031308?e*12.92:1.055*e**.41666-.055}function en(e){return typeof HTMLImageElement<`u`&&e instanceof HTMLImageElement||typeof HTMLCanvasElement<`u`&&e instanceof HTMLCanvasElement||typeof ImageBitmap<`u`&&e instanceof ImageBitmap?bi.getDataURL(e):e.data?{data:Array.from(e.data),width:e.width,height:e.height,type:e.data.constructor.name}:(W(`Texture: Unable to serialize Texture.`),{})}function tn(e,t,n){return n<0&&(n+=1),n>1&&--n,n<1/6?e+(t-e)*6*n:n<1/2?t:n<2/3?e+(t-e)*6*(2/3-n):e}function nn(e,t,n,r,i){for(let a=0,o=e.length-3;a<=o;a+=3){za.fromArray(e,a);let o=i.x*Math.abs(za.x)+i.y*Math.abs(za.y)+i.z*Math.abs(za.z),s=t.dot(za),c=n.dot(za),l=r.dot(za);if(Math.max(-Math.max(s,c,l),Math.min(s,c,l))>o)return!1}return!0}function rn(e,t,n,r,i,a,o,s){let c;if(c=t.side===1?r.intersectTriangle(o,a,i,!0,s):r.intersectTriangle(i,a,o,t.side===0,s),c===null)return null;Do.copy(s),Do.applyMatrix4(e.matrixWorld);let l=n.ray.origin.distanceTo(Do);return l<n.near||l>n.far?null:{distance:l,point:Do.clone(),object:e}}function an(e,t,n,r,i,a,o,s,c,l){e.getVertexPosition(s,xo),e.getVertexPosition(c,So),e.getVertexPosition(l,Co);let u=rn(e,t,n,r,xo,So,Co,Eo);if(u){let e=new K;Ta.getBarycoord(Eo,xo,So,Co,e),i&&(u.uv=Ta.getInterpolatedAttribute(i,s,c,l,e,new fi)),a&&(u.uv1=Ta.getInterpolatedAttribute(a,s,c,l,e,new fi)),o&&(u.normal=Ta.getInterpolatedAttribute(o,s,c,l,e,new K),u.normal.dot(r.direction)>0&&u.normal.multiplyScalar(-1));let t={a:s,b:c,c:l,normal:new K,materialIndex:0};Ta.getNormal(xo,So,Co,t.normal),u.face=t,u.barycoord=e}return u}function on(e,t,n,r,i,a,o){let s=e.geometry.attributes.position;if(as.fromBufferAttribute(s,i),os.fromBufferAttribute(s,a),n.distanceSqToSegment(as,os,us,ds)>r)return;us.applyMatrix4(e.matrixWorld);let c=t.ray.origin.distanceTo(us);if(!(c<t.near||c>t.far))return{distance:c,point:ds.clone().applyMatrix4(e.matrixWorld),index:o,face:null,faceIndex:null,barycoord:null,object:e}}function sn(e,t,n,r,i,a,o){let s=vs.distanceSqToPoint(e);if(s<n){let n=new K;vs.closestPointToPoint(e,n),n.applyMatrix4(r);let c=i.ray.origin.distanceTo(n);if(c<i.near||c>i.far)return;a.push({distance:c,distanceToRay:Math.sqrt(s),point:n,index:t,face:null,faceIndex:null,barycoord:null,object:o})}}function cn(e){let t={};for(let n in e){t[n]={};for(let r in e[n]){let i=e[n][r];if(un(i))i.isRenderTargetTexture?(W(`UniformsUtils: Textures of render targets cannot be cloned via cloneUniforms() or mergeUniforms().`),t[n][r]=null):t[n][r]=i.clone();else if(Array.isArray(i))if(un(i[0])){let e=[];for(let t=0,n=i.length;t<n;t++)e[t]=i[t].clone();t[n][r]=e}else t[n][r]=i.slice();else t[n][r]=i}}return t}function ln(e){let t={};for(let n=0;n<e.length;n++){let r=cn(e[n]);for(let e in r)t[e]=r[e]}return t}function un(e){return e&&(e.isColor||e.isMatrix3||e.isMatrix4||e.isVector2||e.isVector3||e.isVector4||e.isTexture||e.isQuaternion)}function dn(e){let t=[];for(let n=0;n<e.length;n++)t.push(e[n].clone());return t}function fn(e){let t=e.getRenderTarget();return t===null?e.outputColorSpace:t.isXRRenderTarget===!0?t.texture.colorSpace:J.workingColorSpace}function pn(e,t){return!e||e.constructor===t?e:typeof t.BYTES_PER_ELEMENT==`number`?new t(e):Array.prototype.slice.call(e)}function mn(e){function t(t,n){return e[t]-e[n]}let n=e.length,r=Array(n);for(let e=0;e!==n;++e)r[e]=e;return r.sort(t),r}function hn(e,t,n){let r=e.length,i=new e.constructor(r);for(let a=0,o=0;o!==r;++a){let r=n[a]*t;for(let n=0;n!==t;++n)i[o++]=e[r+n]}return i}function gn(e,t,n,r){let i=1,a=e[0];for(;a!==void 0&&a[r]===void 0;)a=e[i++];if(a===void 0)return;let o=a[r];if(o!==void 0)if(Array.isArray(o))do o=a[r],o!==void 0&&(t.push(a.time),n.push(...o)),a=e[i++];while(a!==void 0);else if(o.toArray!==void 0)do o=a[r],o!==void 0&&(t.push(a.time),o.toArray(n,n.length)),a=e[i++];while(a!==void 0);else do o=a[r],o!==void 0&&(t.push(a.time),n.push(o)),a=e[i++];while(a!==void 0)}function _n(e){switch(e.toLowerCase()){case`scalar`:case`double`:case`float`:case`number`:case`integer`:return Gs;case`vector`:case`vector2`:case`vector3`:case`vector4`:return Ys;case`color`:return Ws;case`quaternion`:return qs;case`bool`:case`boolean`:return Us;case`string`:return Js}throw Error(`THREE.KeyframeTrack: Unsupported typeName: `+e)}function vn(e){if(e.type===void 0)throw Error(`THREE.KeyframeTrack: track type undefined, can not parse`);let t=_n(e.type);if(e.times===void 0){let t=[],n=[];gn(e.keys,t,n,`value`),e.times=t,e.values=n}return t.parse===void 0?new t(e.name,e.times,e.values,e.interpolation):t.parse(e)}function yn(e,t){return e.distance-t.distance}function bn(e,t,n,r){let i=!0;if(e.layers.test(t.layers)&&e.raycast(t,n)===!1&&(i=!1),i===!0&&r===!0){let r=e.children;for(let e=0,i=r.length;e<i;e++)bn(r[e],t,n,!0)}}function xn(e,t,n,r){let i=Sn(r);switch(n){case Kn:return e*t;case Zn:return e*t/i.components*i.byteLength;case Qn:return e*t/i.components*i.byteLength;case $n:return e*t*2/i.components*i.byteLength;case er:return e*t*2/i.components*i.byteLength;case qn:return e*t*3/i.components*i.byteLength;case Jn:return e*t*4/i.components*i.byteLength;case tr:return e*t*4/i.components*i.byteLength;case nr:case rr:return Math.floor((e+3)/4)*Math.floor((t+3)/4)*8;case ir:case ar:return Math.floor((e+3)/4)*Math.floor((t+3)/4)*16;case sr:case lr:return Math.max(e,16)*Math.max(t,8)/4;case or:case cr:return Math.max(e,8)*Math.max(t,8)/2;case ur:case dr:case pr:case mr:return Math.floor((e+3)/4)*Math.floor((t+3)/4)*8;case fr:case hr:case gr:return Math.floor((e+3)/4)*Math.floor((t+3)/4)*16;case _r:return Math.floor((e+3)/4)*Math.floor((t+3)/4)*16;case vr:return Math.floor((e+4)/5)*Math.floor((t+3)/4)*16;case yr:return Math.floor((e+4)/5)*Math.floor((t+4)/5)*16;case br:return Math.floor((e+5)/6)*Math.floor((t+4)/5)*16;case xr:return Math.floor((e+5)/6)*Math.floor((t+5)/6)*16;case Sr:return Math.floor((e+7)/8)*Math.floor((t+4)/5)*16;case Cr:return Math.floor((e+7)/8)*Math.floor((t+5)/6)*16;case wr:return Math.floor((e+7)/8)*Math.floor((t+7)/8)*16;case Tr:return Math.floor((e+9)/10)*Math.floor((t+4)/5)*16;case Er:return Math.floor((e+9)/10)*Math.floor((t+5)/6)*16;case Dr:return Math.floor((e+9)/10)*Math.floor((t+7)/8)*16;case Or:return Math.floor((e+9)/10)*Math.floor((t+9)/10)*16;case kr:return Math.floor((e+11)/12)*Math.floor((t+9)/10)*16;case Ar:return Math.floor((e+11)/12)*Math.floor((t+11)/12)*16;case jr:case Mr:case Nr:return Math.ceil(e/4)*Math.ceil(t/4)*16;case Pr:case Fr:return Math.ceil(e/4)*Math.ceil(t/4)*8;case Ir:case Lr:return Math.ceil(e/4)*Math.ceil(t/4)*16}throw Error(`Unable to determine texture byte length for ${n} format.`)}function Sn(e){switch(e){case Nn:case Pn:return{byteLength:1,components:1};case In:case Fn:case Bn:return{byteLength:2,components:1};case Vn:case Hn:return{byteLength:2,components:4};case Rn:case Ln:case zn:return{byteLength:4,components:1};case Wn:case Gn:return{byteLength:4,components:3}}throw Error(`Unknown texture type ${e}.`)}var Cn,wn,Tn,En,Dn,On,kn,An,jn,Mn,Nn,Pn,Fn,In,Ln,Rn,zn,Bn,Vn,Hn,Un,Wn,Gn,Kn,qn,Jn,Yn,Xn,Zn,Qn,$n,er,tr,nr,rr,ir,ar,or,sr,cr,lr,ur,dr,fr,pr,mr,hr,gr,_r,vr,yr,br,xr,Sr,Cr,wr,Tr,Er,Dr,Or,kr,Ar,jr,Mr,Nr,Pr,Fr,Ir,Lr,Rr,zr,Br,Vr,Hr,Ur,Wr,Gr,Kr,qr,Jr,Yr,Xr,Zr,Qr,$r,ei,ti,ni,ri,ii,ai,oi,si,ci,li,ui,di,fi,pi,K,mi,hi,q,gi,_i,vi,J,yi,bi,xi,Si,Ci,wi,Ti,Ei,Di,Oi,ki,Ai,Y,ji,Mi,Ni,Pi,Fi,Ii,Li,Ri,zi,Bi,Vi,Hi,Ui,Wi,Gi,Ki,qi,Ji,Yi,Xi,Zi,Qi,$i,ea,ta,na,ra,ia,aa,oa,sa,ca,la,X,ua,da,fa,pa,ma,ha,ga,_a,va,ya,ba,xa,Sa,Ca,wa,Ta,Ea,Da,Oa,ka,Aa,ja,Ma,Na,Pa,Fa,Ia,La,Ra,za,Ba,Va,Ha,Ua,Wa,Ga,Ka,qa,Ja,Ya,Xa,Za,Qa,$a,eo,to,no,ro,io,ao,oo,so,co,lo,uo,fo,po,mo,ho,go,_o,vo,yo,bo,xo,So,Co,wo,To,Eo,Do,Oo,ko,Ao,jo,Mo,No,Po,Fo,Io,Lo,Ro,zo,Bo,Vo,Ho,Uo,Wo,Go,Ko,qo,Jo,Yo,Xo,Zo,Qo,$o,es,ts,ns,rs,is,as,os,ss,cs,ls,us,ds,fs,ps,ms,hs,gs,_s,vs,ys,bs,xs,Ss,Cs,ws,Ts,Es,Ds,Os,ks,As,js,Ms,Ns,Ps,Fs,Is,Ls,Rs,zs,Bs,Vs,Hs,Us,Ws,Gs,Ks,qs,Js,Ys,Xs,Zs,Qs,$s,ec,tc,nc,rc,ic,ac,oc,sc,cc,lc,uc,dc,fc,pc,mc,hc,gc,_c,vc,yc,bc,xc,Sc,Cc,wc,Tc,Ec,Dc,Oc,kc,Ac,jc,Mc,Nc,Pc,Fc,Ic,Lc,Rc,zc,Bc,Vc,Hc,Uc,Wc=e((()=>{Cn=`attached`,wn=1e3,Tn=1001,En=1002,Dn=1003,On=1004,kn=1005,An=1006,jn=1007,Mn=1008,Nn=1009,Pn=1010,Fn=1011,In=1012,Ln=1013,Rn=1014,zn=1015,Bn=1016,Vn=1017,Hn=1018,Un=1020,Wn=35902,Gn=35899,Kn=1021,qn=1022,Jn=1023,Yn=1026,Xn=1027,Zn=1028,Qn=1029,$n=1030,er=1031,tr=1033,nr=33776,rr=33777,ir=33778,ar=33779,or=35840,sr=35841,cr=35842,lr=35843,ur=36196,dr=37492,fr=37496,pr=37488,mr=37489,hr=37490,gr=37491,_r=37808,vr=37809,yr=37810,br=37811,xr=37812,Sr=37813,Cr=37814,wr=37815,Tr=37816,Er=37817,Dr=37818,Or=37819,kr=37820,Ar=37821,jr=36492,Mr=36494,Nr=36495,Pr=36283,Fr=36284,Ir=36285,Lr=36286,Rr=2200,zr=2201,Br=2202,Vr=2300,Hr=2301,Ur=2302,Wr=2303,Gr=2400,Kr=2401,qr=2402,Jr=2500,Yr=2501,Xr=3200,Zr=`srgb`,Qr=`srgb-linear`,$r=`linear`,ei=`srgb`,ti=7680,ni=35044,ri=35048,ii=2e3,ai={},oi=null,si={0:1,2:6,4:7,3:5,1:0,6:2,7:4,5:3},ci=class{addEventListener(e,t){this._listeners===void 0&&(this._listeners={});let n=this._listeners;n[e]===void 0&&(n[e]=[]),n[e].indexOf(t)===-1&&n[e].push(t)}hasEventListener(e,t){let n=this._listeners;return n!==void 0&&n[e]!==void 0&&n[e].indexOf(t)!==-1}removeEventListener(e,t){let n=this._listeners;if(n===void 0)return;let r=n[e];if(r!==void 0){let e=r.indexOf(t);e!==-1&&r.splice(e,1)}}dispatchEvent(e){let t=this._listeners;if(t===void 0)return;let n=t[e.type];if(n!==void 0){e.target=this;let t=n.slice(0);for(let n=0,r=t.length;n<r;n++)t[n].call(this,e);e.target=null}}},li=`00.01.02.03.04.05.06.07.08.09.0a.0b.0c.0d.0e.0f.10.11.12.13.14.15.16.17.18.19.1a.1b.1c.1d.1e.1f.20.21.22.23.24.25.26.27.28.29.2a.2b.2c.2d.2e.2f.30.31.32.33.34.35.36.37.38.39.3a.3b.3c.3d.3e.3f.40.41.42.43.44.45.46.47.48.49.4a.4b.4c.4d.4e.4f.50.51.52.53.54.55.56.57.58.59.5a.5b.5c.5d.5e.5f.60.61.62.63.64.65.66.67.68.69.6a.6b.6c.6d.6e.6f.70.71.72.73.74.75.76.77.78.79.7a.7b.7c.7d.7e.7f.80.81.82.83.84.85.86.87.88.89.8a.8b.8c.8d.8e.8f.90.91.92.93.94.95.96.97.98.99.9a.9b.9c.9d.9e.9f.a0.a1.a2.a3.a4.a5.a6.a7.a8.a9.aa.ab.ac.ad.ae.af.b0.b1.b2.b3.b4.b5.b6.b7.b8.b9.ba.bb.bc.bd.be.bf.c0.c1.c2.c3.c4.c5.c6.c7.c8.c9.ca.cb.cc.cd.ce.cf.d0.d1.d2.d3.d4.d5.d6.d7.d8.d9.da.db.dc.dd.de.df.e0.e1.e2.e3.e4.e5.e6.e7.e8.e9.ea.eb.ec.ed.ee.ef.f0.f1.f2.f3.f4.f5.f6.f7.f8.f9.fa.fb.fc.fd.fe.ff`.split(`.`),ui=Math.PI/180,di=180/Math.PI,fi=class e{static{e.prototype.isVector2=!0}constructor(e=0,t=0){this.x=e,this.y=t}get width(){return this.x}set width(e){this.x=e}get height(){return this.y}set height(e){this.y=e}set(e,t){return this.x=e,this.y=t,this}setScalar(e){return this.x=e,this.y=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;default:throw Error(`index is out of range: `+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;default:throw Error(`index is out of range: `+e)}}clone(){return new this.constructor(this.x,this.y)}copy(e){return this.x=e.x,this.y=e.y,this}add(e){return this.x+=e.x,this.y+=e.y,this}addScalar(e){return this.x+=e,this.y+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this}subScalar(e){return this.x-=e,this.y-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this}multiply(e){return this.x*=e.x,this.y*=e.y,this}multiplyScalar(e){return this.x*=e,this.y*=e,this}divide(e){return this.x/=e.x,this.y/=e.y,this}divideScalar(e){return this.multiplyScalar(1/e)}applyMatrix3(e){let t=this.x,n=this.y,r=e.elements;return this.x=r[0]*t+r[3]*n+r[6],this.y=r[1]*t+r[4]*n+r[7],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this}clamp(e,t){return this.x=Kt(this.x,e.x,t.x),this.y=Kt(this.y,e.y,t.y),this}clampScalar(e,t){return this.x=Kt(this.x,e,t),this.y=Kt(this.y,e,t),this}clampLength(e,t){let n=this.length();return this.divideScalar(n||1).multiplyScalar(Kt(n,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this}negate(){return this.x=-this.x,this.y=-this.y,this}dot(e){return this.x*e.x+this.y*e.y}cross(e){return this.x*e.y-this.y*e.x}lengthSq(){return this.x*this.x+this.y*this.y}length(){return Math.sqrt(this.x*this.x+this.y*this.y)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)}normalize(){return this.divideScalar(this.length()||1)}angle(){return Math.atan2(-this.y,-this.x)+Math.PI}angleTo(e){let t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;let n=this.dot(e)/t;return Math.acos(Kt(n,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){let t=this.x-e.x,n=this.y-e.y;return t*t+n*n}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this}lerpVectors(e,t,n){return this.x=e.x+(t.x-e.x)*n,this.y=e.y+(t.y-e.y)*n,this}equals(e){return e.x===this.x&&e.y===this.y}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this}rotateAround(e,t){let n=Math.cos(t),r=Math.sin(t),i=this.x-e.x,a=this.y-e.y;return this.x=i*n-a*r+e.x,this.y=i*r+a*n+e.y,this}random(){return this.x=Math.random(),this.y=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y}},pi=class{constructor(e=0,t=0,n=0,r=1){this.isQuaternion=!0,this._x=e,this._y=t,this._z=n,this._w=r}static slerpFlat(e,t,n,r,i,a,o){let s=n[r+0],c=n[r+1],l=n[r+2],u=n[r+3],d=i[a+0],f=i[a+1],p=i[a+2],m=i[a+3];if(u!==m||s!==d||c!==f||l!==p){let e=s*d+c*f+l*p+u*m;e<0&&(d=-d,f=-f,p=-p,m=-m,e=-e);let t=1-o;if(e<.9995){let n=Math.acos(e),r=Math.sin(n);t=Math.sin(t*n)/r,o=Math.sin(o*n)/r,s=s*t+d*o,c=c*t+f*o,l=l*t+p*o,u=u*t+m*o}else{s=s*t+d*o,c=c*t+f*o,l=l*t+p*o,u=u*t+m*o;let e=1/Math.sqrt(s*s+c*c+l*l+u*u);s*=e,c*=e,l*=e,u*=e}}e[t]=s,e[t+1]=c,e[t+2]=l,e[t+3]=u}static multiplyQuaternionsFlat(e,t,n,r,i,a){let o=n[r],s=n[r+1],c=n[r+2],l=n[r+3],u=i[a],d=i[a+1],f=i[a+2],p=i[a+3];return e[t]=o*p+l*u+s*f-c*d,e[t+1]=s*p+l*d+c*u-o*f,e[t+2]=c*p+l*f+o*d-s*u,e[t+3]=l*p-o*u-s*d-c*f,e}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get w(){return this._w}set w(e){this._w=e,this._onChangeCallback()}set(e,t,n,r){return this._x=e,this._y=t,this._z=n,this._w=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._w)}copy(e){return this._x=e.x,this._y=e.y,this._z=e.z,this._w=e.w,this._onChangeCallback(),this}setFromEuler(e,t=!0){let n=e._x,r=e._y,i=e._z,a=e._order,o=Math.cos,s=Math.sin,c=o(n/2),l=o(r/2),u=o(i/2),d=s(n/2),f=s(r/2),p=s(i/2);switch(a){case`XYZ`:this._x=d*l*u+c*f*p,this._y=c*f*u-d*l*p,this._z=c*l*p+d*f*u,this._w=c*l*u-d*f*p;break;case`YXZ`:this._x=d*l*u+c*f*p,this._y=c*f*u-d*l*p,this._z=c*l*p-d*f*u,this._w=c*l*u+d*f*p;break;case`ZXY`:this._x=d*l*u-c*f*p,this._y=c*f*u+d*l*p,this._z=c*l*p+d*f*u,this._w=c*l*u-d*f*p;break;case`ZYX`:this._x=d*l*u-c*f*p,this._y=c*f*u+d*l*p,this._z=c*l*p-d*f*u,this._w=c*l*u+d*f*p;break;case`YZX`:this._x=d*l*u+c*f*p,this._y=c*f*u+d*l*p,this._z=c*l*p-d*f*u,this._w=c*l*u-d*f*p;break;case`XZY`:this._x=d*l*u-c*f*p,this._y=c*f*u-d*l*p,this._z=c*l*p+d*f*u,this._w=c*l*u+d*f*p;break;default:W(`Quaternion: .setFromEuler() encountered an unknown order: `+a)}return t===!0&&this._onChangeCallback(),this}setFromAxisAngle(e,t){let n=t/2,r=Math.sin(n);return this._x=e.x*r,this._y=e.y*r,this._z=e.z*r,this._w=Math.cos(n),this._onChangeCallback(),this}setFromRotationMatrix(e){let t=e.elements,n=t[0],r=t[4],i=t[8],a=t[1],o=t[5],s=t[9],c=t[2],l=t[6],u=t[10],d=n+o+u;if(d>0){let e=.5/Math.sqrt(d+1);this._w=.25/e,this._x=(l-s)*e,this._y=(i-c)*e,this._z=(a-r)*e}else if(n>o&&n>u){let e=2*Math.sqrt(1+n-o-u);this._w=(l-s)/e,this._x=.25*e,this._y=(r+a)/e,this._z=(i+c)/e}else if(o>u){let e=2*Math.sqrt(1+o-n-u);this._w=(i-c)/e,this._x=(r+a)/e,this._y=.25*e,this._z=(s+l)/e}else{let e=2*Math.sqrt(1+u-n-o);this._w=(a-r)/e,this._x=(i+c)/e,this._y=(s+l)/e,this._z=.25*e}return this._onChangeCallback(),this}setFromUnitVectors(e,t){let n=e.dot(t)+1;return n<1e-8?(n=0,Math.abs(e.x)>Math.abs(e.z)?(this._x=-e.y,this._y=e.x,this._z=0,this._w=n):(this._x=0,this._y=-e.z,this._z=e.y,this._w=n)):(this._x=e.y*t.z-e.z*t.y,this._y=e.z*t.x-e.x*t.z,this._z=e.x*t.y-e.y*t.x,this._w=n),this.normalize()}angleTo(e){return 2*Math.acos(Math.abs(Kt(this.dot(e),-1,1)))}rotateTowards(e,t){let n=this.angleTo(e);if(n===0)return this;let r=Math.min(1,t/n);return this.slerp(e,r),this}identity(){return this.set(0,0,0,1)}invert(){return this.conjugate()}conjugate(){return this._x*=-1,this._y*=-1,this._z*=-1,this._onChangeCallback(),this}dot(e){return this._x*e._x+this._y*e._y+this._z*e._z+this._w*e._w}lengthSq(){return this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w}length(){return Math.sqrt(this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w)}normalize(){let e=this.length();return e===0?(this._x=0,this._y=0,this._z=0,this._w=1):(e=1/e,this._x*=e,this._y*=e,this._z*=e,this._w*=e),this._onChangeCallback(),this}multiply(e){return this.multiplyQuaternions(this,e)}premultiply(e){return this.multiplyQuaternions(e,this)}multiplyQuaternions(e,t){let n=e._x,r=e._y,i=e._z,a=e._w,o=t._x,s=t._y,c=t._z,l=t._w;return this._x=n*l+a*o+r*c-i*s,this._y=r*l+a*s+i*o-n*c,this._z=i*l+a*c+n*s-r*o,this._w=a*l-n*o-r*s-i*c,this._onChangeCallback(),this}slerp(e,t){let n=e._x,r=e._y,i=e._z,a=e._w,o=this.dot(e);o<0&&(n=-n,r=-r,i=-i,a=-a,o=-o);let s=1-t;if(o<.9995){let e=Math.acos(o),c=Math.sin(e);s=Math.sin(s*e)/c,t=Math.sin(t*e)/c,this._x=this._x*s+n*t,this._y=this._y*s+r*t,this._z=this._z*s+i*t,this._w=this._w*s+a*t,this._onChangeCallback()}else this._x=this._x*s+n*t,this._y=this._y*s+r*t,this._z=this._z*s+i*t,this._w=this._w*s+a*t,this.normalize();return this}slerpQuaternions(e,t,n){return this.copy(e).slerp(t,n)}random(){let e=2*Math.PI*Math.random(),t=2*Math.PI*Math.random(),n=Math.random(),r=Math.sqrt(1-n),i=Math.sqrt(n);return this.set(r*Math.sin(e),r*Math.cos(e),i*Math.sin(t),i*Math.cos(t))}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._w===this._w}fromArray(e,t=0){return this._x=e[t],this._y=e[t+1],this._z=e[t+2],this._w=e[t+3],this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._w,e}fromBufferAttribute(e,t){return this._x=e.getX(t),this._y=e.getY(t),this._z=e.getZ(t),this._w=e.getW(t),this._onChangeCallback(),this}toJSON(){return this.toArray()}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._w}},K=class e{static{e.prototype.isVector3=!0}constructor(e=0,t=0,n=0){this.x=e,this.y=t,this.z=n}set(e,t,n){return n===void 0&&(n=this.z),this.x=e,this.y=t,this.z=n,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;default:throw Error(`index is out of range: `+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;default:throw Error(`index is out of range: `+e)}}clone(){return new this.constructor(this.x,this.y,this.z)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this}multiplyVectors(e,t){return this.x=e.x*t.x,this.y=e.y*t.y,this.z=e.z*t.z,this}applyEuler(e){return this.applyQuaternion(hi.setFromEuler(e))}applyAxisAngle(e,t){return this.applyQuaternion(hi.setFromAxisAngle(e,t))}applyMatrix3(e){let t=this.x,n=this.y,r=this.z,i=e.elements;return this.x=i[0]*t+i[3]*n+i[6]*r,this.y=i[1]*t+i[4]*n+i[7]*r,this.z=i[2]*t+i[5]*n+i[8]*r,this}applyNormalMatrix(e){return this.applyMatrix3(e).normalize()}applyMatrix4(e){let t=this.x,n=this.y,r=this.z,i=e.elements,a=1/(i[3]*t+i[7]*n+i[11]*r+i[15]);return this.x=(i[0]*t+i[4]*n+i[8]*r+i[12])*a,this.y=(i[1]*t+i[5]*n+i[9]*r+i[13])*a,this.z=(i[2]*t+i[6]*n+i[10]*r+i[14])*a,this}applyQuaternion(e){let t=this.x,n=this.y,r=this.z,i=e.x,a=e.y,o=e.z,s=e.w,c=2*(a*r-o*n),l=2*(o*t-i*r),u=2*(i*n-a*t);return this.x=t+s*c+a*u-o*l,this.y=n+s*l+o*c-i*u,this.z=r+s*u+i*l-a*c,this}project(e){return this.applyMatrix4(e.matrixWorldInverse).applyMatrix4(e.projectionMatrix)}unproject(e){return this.applyMatrix4(e.projectionMatrixInverse).applyMatrix4(e.matrixWorld)}transformDirection(e){let t=this.x,n=this.y,r=this.z,i=e.elements;return this.x=i[0]*t+i[4]*n+i[8]*r,this.y=i[1]*t+i[5]*n+i[9]*r,this.z=i[2]*t+i[6]*n+i[10]*r,this.normalize()}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this}divideScalar(e){return this.multiplyScalar(1/e)}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this}clamp(e,t){return this.x=Kt(this.x,e.x,t.x),this.y=Kt(this.y,e.y,t.y),this.z=Kt(this.z,e.z,t.z),this}clampScalar(e,t){return this.x=Kt(this.x,e,t),this.y=Kt(this.y,e,t),this.z=Kt(this.z,e,t),this}clampLength(e,t){let n=this.length();return this.divideScalar(n||1).multiplyScalar(Kt(n,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this}lerpVectors(e,t,n){return this.x=e.x+(t.x-e.x)*n,this.y=e.y+(t.y-e.y)*n,this.z=e.z+(t.z-e.z)*n,this}cross(e){return this.crossVectors(this,e)}crossVectors(e,t){let n=e.x,r=e.y,i=e.z,a=t.x,o=t.y,s=t.z;return this.x=r*s-i*o,this.y=i*a-n*s,this.z=n*o-r*a,this}projectOnVector(e){let t=e.lengthSq();if(t===0)return this.set(0,0,0);let n=e.dot(this)/t;return this.copy(e).multiplyScalar(n)}projectOnPlane(e){return mi.copy(this).projectOnVector(e),this.sub(mi)}reflect(e){return this.sub(mi.copy(e).multiplyScalar(2*this.dot(e)))}angleTo(e){let t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;let n=this.dot(e)/t;return Math.acos(Kt(n,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){let t=this.x-e.x,n=this.y-e.y,r=this.z-e.z;return t*t+n*n+r*r}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)+Math.abs(this.z-e.z)}setFromSpherical(e){return this.setFromSphericalCoords(e.radius,e.phi,e.theta)}setFromSphericalCoords(e,t,n){let r=Math.sin(t)*e;return this.x=r*Math.sin(n),this.y=Math.cos(t)*e,this.z=r*Math.cos(n),this}setFromCylindrical(e){return this.setFromCylindricalCoords(e.radius,e.theta,e.y)}setFromCylindricalCoords(e,t,n){return this.x=e*Math.sin(t),this.y=n,this.z=e*Math.cos(t),this}setFromMatrixPosition(e){let t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this}setFromMatrixScale(e){let t=this.setFromMatrixColumn(e,0).length(),n=this.setFromMatrixColumn(e,1).length(),r=this.setFromMatrixColumn(e,2).length();return this.x=t,this.y=n,this.z=r,this}setFromMatrixColumn(e,t){return this.fromArray(e.elements,t*4)}setFromMatrix3Column(e,t){return this.fromArray(e.elements,t*3)}setFromEuler(e){return this.x=e._x,this.y=e._y,this.z=e._z,this}setFromColor(e){return this.x=e.r,this.y=e.g,this.z=e.b,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this}randomDirection(){let e=Math.random()*Math.PI*2,t=Math.random()*2-1,n=Math.sqrt(1-t*t);return this.x=n*Math.cos(e),this.y=t,this.z=n*Math.sin(e),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z}},mi=new K,hi=new pi,q=class e{static{e.prototype.isMatrix3=!0}constructor(e,t,n,r,i,a,o,s,c){this.elements=[1,0,0,0,1,0,0,0,1],e!==void 0&&this.set(e,t,n,r,i,a,o,s,c)}set(e,t,n,r,i,a,o,s,c){let l=this.elements;return l[0]=e,l[1]=r,l[2]=o,l[3]=t,l[4]=i,l[5]=s,l[6]=n,l[7]=a,l[8]=c,this}identity(){return this.set(1,0,0,0,1,0,0,0,1),this}copy(e){let t=this.elements,n=e.elements;return t[0]=n[0],t[1]=n[1],t[2]=n[2],t[3]=n[3],t[4]=n[4],t[5]=n[5],t[6]=n[6],t[7]=n[7],t[8]=n[8],this}extractBasis(e,t,n){return e.setFromMatrix3Column(this,0),t.setFromMatrix3Column(this,1),n.setFromMatrix3Column(this,2),this}setFromMatrix4(e){let t=e.elements;return this.set(t[0],t[4],t[8],t[1],t[5],t[9],t[2],t[6],t[10]),this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){let n=e.elements,r=t.elements,i=this.elements,a=n[0],o=n[3],s=n[6],c=n[1],l=n[4],u=n[7],d=n[2],f=n[5],p=n[8],m=r[0],h=r[3],g=r[6],_=r[1],v=r[4],y=r[7],b=r[2],x=r[5],S=r[8];return i[0]=a*m+o*_+s*b,i[3]=a*h+o*v+s*x,i[6]=a*g+o*y+s*S,i[1]=c*m+l*_+u*b,i[4]=c*h+l*v+u*x,i[7]=c*g+l*y+u*S,i[2]=d*m+f*_+p*b,i[5]=d*h+f*v+p*x,i[8]=d*g+f*y+p*S,this}multiplyScalar(e){let t=this.elements;return t[0]*=e,t[3]*=e,t[6]*=e,t[1]*=e,t[4]*=e,t[7]*=e,t[2]*=e,t[5]*=e,t[8]*=e,this}determinant(){let e=this.elements,t=e[0],n=e[1],r=e[2],i=e[3],a=e[4],o=e[5],s=e[6],c=e[7],l=e[8];return t*a*l-t*o*c-n*i*l+n*o*s+r*i*c-r*a*s}invert(){let e=this.elements,t=e[0],n=e[1],r=e[2],i=e[3],a=e[4],o=e[5],s=e[6],c=e[7],l=e[8],u=l*a-o*c,d=o*s-l*i,f=c*i-a*s,p=t*u+n*d+r*f;if(p===0)return this.set(0,0,0,0,0,0,0,0,0);let m=1/p;return e[0]=u*m,e[1]=(r*c-l*n)*m,e[2]=(o*n-r*a)*m,e[3]=d*m,e[4]=(l*t-r*s)*m,e[5]=(r*i-o*t)*m,e[6]=f*m,e[7]=(n*s-c*t)*m,e[8]=(a*t-n*i)*m,this}transpose(){let e,t=this.elements;return e=t[1],t[1]=t[3],t[3]=e,e=t[2],t[2]=t[6],t[6]=e,e=t[5],t[5]=t[7],t[7]=e,this}getNormalMatrix(e){return this.setFromMatrix4(e).invert().transpose()}transposeIntoArray(e){let t=this.elements;return e[0]=t[0],e[1]=t[3],e[2]=t[6],e[3]=t[1],e[4]=t[4],e[5]=t[7],e[6]=t[2],e[7]=t[5],e[8]=t[8],this}setUvTransform(e,t,n,r,i,a,o){let s=Math.cos(i),c=Math.sin(i);return this.set(n*s,n*c,-n*(s*a+c*o)+a+e,-r*c,r*s,-r*(-c*a+s*o)+o+t,0,0,1),this}scale(e,t){return this.premultiply(gi.makeScale(e,t)),this}rotate(e){return this.premultiply(gi.makeRotation(-e)),this}translate(e,t){return this.premultiply(gi.makeTranslation(e,t)),this}makeTranslation(e,t){return e.isVector2?this.set(1,0,e.x,0,1,e.y,0,0,1):this.set(1,0,e,0,1,t,0,0,1),this}makeRotation(e){let t=Math.cos(e),n=Math.sin(e);return this.set(t,-n,0,n,t,0,0,0,1),this}makeScale(e,t){return this.set(e,0,0,0,t,0,0,0,1),this}equals(e){let t=this.elements,n=e.elements;for(let e=0;e<9;e++)if(t[e]!==n[e])return!1;return!0}fromArray(e,t=0){for(let n=0;n<9;n++)this.elements[n]=e[n+t];return this}toArray(e=[],t=0){let n=this.elements;return e[t]=n[0],e[t+1]=n[1],e[t+2]=n[2],e[t+3]=n[3],e[t+4]=n[4],e[t+5]=n[5],e[t+6]=n[6],e[t+7]=n[7],e[t+8]=n[8],e}clone(){return new this.constructor().fromArray(this.elements)}},gi=new q,_i=new q().set(.4123908,.3575843,.1804808,.212639,.7151687,.0721923,.0193308,.1191948,.9505322),vi=new q().set(3.2409699,-1.5373832,-.4986108,-.9692436,1.8759675,.0415551,.0556301,-.203977,1.0569715),J=Zt(),bi=class{static getDataURL(e,t=`image/png`){if(/^data:/i.test(e.src)||typeof HTMLCanvasElement>`u`)return e.src;let n;if(e instanceof HTMLCanvasElement)n=e;else{yi===void 0&&(yi=zt(`canvas`)),yi.width=e.width,yi.height=e.height;let t=yi.getContext(`2d`);e instanceof ImageData?t.putImageData(e,0,0):t.drawImage(e,0,0,e.width,e.height),n=yi}return n.toDataURL(t)}static sRGBToLinear(e){if(typeof HTMLImageElement<`u`&&e instanceof HTMLImageElement||typeof HTMLCanvasElement<`u`&&e instanceof HTMLCanvasElement||typeof ImageBitmap<`u`&&e instanceof ImageBitmap){let t=zt(`canvas`);t.width=e.width,t.height=e.height;let n=t.getContext(`2d`);n.drawImage(e,0,0,e.width,e.height);let r=n.getImageData(0,0,e.width,e.height),i=r.data;for(let e=0;e<i.length;e++)i[e]=Qt(i[e]/255)*255;return n.putImageData(r,0,0),t}else if(e.data){let t=e.data.slice(0);for(let e=0;e<t.length;e++)t instanceof Uint8Array||t instanceof Uint8ClampedArray?t[e]=Math.floor(Qt(t[e]/255)*255):t[e]=Qt(t[e]);return{data:t,width:e.width,height:e.height}}else return W(`ImageUtils.sRGBToLinear(): Unsupported image type. No color space conversion applied.`),e}},xi=0,Si=class{constructor(e=null){this.isSource=!0,Object.defineProperty(this,"id",{value:xi++}),this.uuid=Gt(),this.data=e,this.dataReady=!0,this.version=0}getSize(e){let t=this.data;return typeof HTMLVideoElement<`u`&&t instanceof HTMLVideoElement?e.set(t.videoWidth,t.videoHeight,0):typeof VideoFrame<`u`&&t instanceof VideoFrame?e.set(t.displayWidth,t.displayHeight,0):t===null?e.set(0,0,0):e.set(t.width,t.height,t.depth||0),e}set needsUpdate(e){e===!0&&this.version++}toJSON(e){let t=e===void 0||typeof e==`string`;if(!t&&e.images[this.uuid]!==void 0)return e.images[this.uuid];let n={uuid:this.uuid,url:``},r=this.data;if(r!==null){let e;if(Array.isArray(r)){e=[];for(let t=0,n=r.length;t<n;t++)r[t].isDataTexture?e.push(en(r[t].image)):e.push(en(r[t]))}else e=en(r);n.url=e}return t||(e.images[this.uuid]=n),n}},Ci=0,wi=new K,Ti=class e extends ci{constructor(t=e.DEFAULT_IMAGE,n=e.DEFAULT_MAPPING,r=Tn,i=Tn,a=An,o=Mn,s=Jn,c=Nn,l=e.DEFAULT_ANISOTROPY,u=``){super(),this.isTexture=!0,Object.defineProperty(this,"id",{value:Ci++}),this.uuid=Gt(),this.name=``,this.source=new Si(t),this.mipmaps=[],this.mapping=n,this.channel=0,this.wrapS=r,this.wrapT=i,this.magFilter=a,this.minFilter=o,this.anisotropy=l,this.format=s,this.internalFormat=null,this.type=c,this.offset=new fi(0,0),this.repeat=new fi(1,1),this.center=new fi(0,0),this.rotation=0,this.matrixAutoUpdate=!0,this.matrix=new q,this.generateMipmaps=!0,this.premultiplyAlpha=!1,this.flipY=!0,this.unpackAlignment=4,this.colorSpace=u,this.userData={},this.updateRanges=[],this.version=0,this.onUpdate=null,this.renderTarget=null,this.isRenderTargetTexture=!1,this.isArrayTexture=!!(t&&t.depth&&t.depth>1),this.pmremVersion=0,this.normalized=!1}get width(){return this.source.getSize(wi).x}get height(){return this.source.getSize(wi).y}get depth(){return this.source.getSize(wi).z}get image(){return this.source.data}set image(e){this.source.data=e}updateMatrix(){this.matrix.setUvTransform(this.offset.x,this.offset.y,this.repeat.x,this.repeat.y,this.rotation,this.center.x,this.center.y)}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}clone(){return new this.constructor().copy(this)}copy(e){return this.name=e.name,this.source=e.source,this.mipmaps=e.mipmaps.slice(0),this.mapping=e.mapping,this.channel=e.channel,this.wrapS=e.wrapS,this.wrapT=e.wrapT,this.magFilter=e.magFilter,this.minFilter=e.minFilter,this.anisotropy=e.anisotropy,this.format=e.format,this.internalFormat=e.internalFormat,this.type=e.type,this.normalized=e.normalized,this.offset.copy(e.offset),this.repeat.copy(e.repeat),this.center.copy(e.center),this.rotation=e.rotation,this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrix.copy(e.matrix),this.generateMipmaps=e.generateMipmaps,this.premultiplyAlpha=e.premultiplyAlpha,this.flipY=e.flipY,this.unpackAlignment=e.unpackAlignment,this.colorSpace=e.colorSpace,this.renderTarget=e.renderTarget,this.isRenderTargetTexture=e.isRenderTargetTexture,this.isArrayTexture=e.isArrayTexture,this.userData=JSON.parse(JSON.stringify(e.userData)),this.needsUpdate=!0,this}setValues(e){for(let t in e){let n=e[t];if(n===void 0){W(`Texture.setValues(): parameter '${t}' has value of undefined.`);continue}let r=this[t];if(r===void 0){W(`Texture.setValues(): property '${t}' does not exist.`);continue}r&&n&&r.isVector2&&n.isVector2||r&&n&&r.isVector3&&n.isVector3||r&&n&&r.isMatrix3&&n.isMatrix3?r.copy(n):this[t]=n}}toJSON(e){let t=e===void 0||typeof e==`string`;if(!t&&e.textures[this.uuid]!==void 0)return e.textures[this.uuid];let n={metadata:{version:4.7,type:`Texture`,generator:`Texture.toJSON`},uuid:this.uuid,name:this.name,image:this.source.toJSON(e).uuid,mapping:this.mapping,channel:this.channel,repeat:[this.repeat.x,this.repeat.y],offset:[this.offset.x,this.offset.y],center:[this.center.x,this.center.y],rotation:this.rotation,wrap:[this.wrapS,this.wrapT],format:this.format,internalFormat:this.internalFormat,type:this.type,normalized:this.normalized,colorSpace:this.colorSpace,minFilter:this.minFilter,magFilter:this.magFilter,anisotropy:this.anisotropy,flipY:this.flipY,generateMipmaps:this.generateMipmaps,premultiplyAlpha:this.premultiplyAlpha,unpackAlignment:this.unpackAlignment};return Object.keys(this.userData).length>0&&(n.userData=this.userData),t||(e.textures[this.uuid]=n),n}dispose(){this.dispatchEvent({type:`dispose`})}transformUv(e){if(this.mapping!==300)return e;if(e.applyMatrix3(this.matrix),e.x<0||e.x>1)switch(this.wrapS){case wn:e.x-=Math.floor(e.x);break;case Tn:e.x=e.x<0?0:1;break;case En:Math.abs(Math.floor(e.x)%2)===1?e.x=Math.ceil(e.x)-e.x:e.x-=Math.floor(e.x);break}if(e.y<0||e.y>1)switch(this.wrapT){case wn:e.y-=Math.floor(e.y);break;case Tn:e.y=e.y<0?0:1;break;case En:Math.abs(Math.floor(e.y)%2)===1?e.y=Math.ceil(e.y)-e.y:e.y-=Math.floor(e.y);break}return this.flipY&&(e.y=1-e.y),e}set needsUpdate(e){e===!0&&(this.version++,this.source.needsUpdate=!0)}set needsPMREMUpdate(e){e===!0&&this.pmremVersion++}},Ti.DEFAULT_IMAGE=null,Ti.DEFAULT_MAPPING=300,Ti.DEFAULT_ANISOTROPY=1,Ei=class e{static{e.prototype.isVector4=!0}constructor(e=0,t=0,n=0,r=1){this.x=e,this.y=t,this.z=n,this.w=r}get width(){return this.z}set width(e){this.z=e}get height(){return this.w}set height(e){this.w=e}set(e,t,n,r){return this.x=e,this.y=t,this.z=n,this.w=r,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this.w=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setW(e){return this.w=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;case 3:this.w=t;break;default:throw Error(`index is out of range: `+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;case 3:return this.w;default:throw Error(`index is out of range: `+e)}}clone(){return new this.constructor(this.x,this.y,this.z,this.w)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this.w=e.w===void 0?1:e.w,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this.w+=e.w,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this.w+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this.w=e.w+t.w,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this.w+=e.w*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this.w-=e.w,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this.w-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this.w=e.w-t.w,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this.w*=e.w,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this.w*=e,this}applyMatrix4(e){let t=this.x,n=this.y,r=this.z,i=this.w,a=e.elements;return this.x=a[0]*t+a[4]*n+a[8]*r+a[12]*i,this.y=a[1]*t+a[5]*n+a[9]*r+a[13]*i,this.z=a[2]*t+a[6]*n+a[10]*r+a[14]*i,this.w=a[3]*t+a[7]*n+a[11]*r+a[15]*i,this}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this.w/=e.w,this}divideScalar(e){return this.multiplyScalar(1/e)}setAxisAngleFromQuaternion(e){this.w=2*Math.acos(e.w);let t=Math.sqrt(1-e.w*e.w);return t<1e-4?(this.x=1,this.y=0,this.z=0):(this.x=e.x/t,this.y=e.y/t,this.z=e.z/t),this}setAxisAngleFromRotationMatrix(e){let t,n,r,i,a=.01,o=.1,s=e.elements,c=s[0],l=s[4],u=s[8],d=s[1],f=s[5],p=s[9],m=s[2],h=s[6],g=s[10];if(Math.abs(l-d)<a&&Math.abs(u-m)<a&&Math.abs(p-h)<a){if(Math.abs(l+d)<o&&Math.abs(u+m)<o&&Math.abs(p+h)<o&&Math.abs(c+f+g-3)<o)return this.set(1,0,0,0),this;t=Math.PI;let e=(c+1)/2,s=(f+1)/2,_=(g+1)/2,v=(l+d)/4,y=(u+m)/4,b=(p+h)/4;return e>s&&e>_?e<a?(n=0,r=.707106781,i=.707106781):(n=Math.sqrt(e),r=v/n,i=y/n):s>_?s<a?(n=.707106781,r=0,i=.707106781):(r=Math.sqrt(s),n=v/r,i=b/r):_<a?(n=.707106781,r=.707106781,i=0):(i=Math.sqrt(_),n=y/i,r=b/i),this.set(n,r,i,t),this}let _=Math.sqrt((h-p)*(h-p)+(u-m)*(u-m)+(d-l)*(d-l));return Math.abs(_)<.001&&(_=1),this.x=(h-p)/_,this.y=(u-m)/_,this.z=(d-l)/_,this.w=Math.acos((c+f+g-1)/2),this}setFromMatrixPosition(e){let t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this.w=t[15],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this.w=Math.min(this.w,e.w),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this.w=Math.max(this.w,e.w),this}clamp(e,t){return this.x=Kt(this.x,e.x,t.x),this.y=Kt(this.y,e.y,t.y),this.z=Kt(this.z,e.z,t.z),this.w=Kt(this.w,e.w,t.w),this}clampScalar(e,t){return this.x=Kt(this.x,e,t),this.y=Kt(this.y,e,t),this.z=Kt(this.z,e,t),this.w=Kt(this.w,e,t),this}clampLength(e,t){let n=this.length();return this.divideScalar(n||1).multiplyScalar(Kt(n,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this.w=Math.floor(this.w),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this.w=Math.ceil(this.w),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this.w=Math.round(this.w),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this.w=Math.trunc(this.w),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this.w=-this.w,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z+this.w*e.w}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)+Math.abs(this.w)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this.w+=(e.w-this.w)*t,this}lerpVectors(e,t,n){return this.x=e.x+(t.x-e.x)*n,this.y=e.y+(t.y-e.y)*n,this.z=e.z+(t.z-e.z)*n,this.w=e.w+(t.w-e.w)*n,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z&&e.w===this.w}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this.w=e[t+3],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e[t+3]=this.w,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this.w=e.getW(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this.w=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z,yield this.w}},Di=class extends ci{constructor(e=1,t=1,n={}){super(),n=Object.assign({generateMipmaps:!1,internalFormat:null,minFilter:An,depthBuffer:!0,stencilBuffer:!1,resolveDepthBuffer:!0,resolveStencilBuffer:!0,depthTexture:null,samples:0,count:1,depth:1,multiview:!1},n),this.isRenderTarget=!0,this.width=e,this.height=t,this.depth=n.depth,this.scissor=new Ei(0,0,e,t),this.scissorTest=!1,this.viewport=new Ei(0,0,e,t),this.textures=[];let r={width:e,height:t,depth:n.depth},i=new Ti(r),a=n.count;for(let e=0;e<a;e++)this.textures[e]=i.clone(),this.textures[e].isRenderTargetTexture=!0,this.textures[e].renderTarget=this;this._setTextureOptions(n),this.depthBuffer=n.depthBuffer,this.stencilBuffer=n.stencilBuffer,this.resolveDepthBuffer=n.resolveDepthBuffer,this.resolveStencilBuffer=n.resolveStencilBuffer,this._depthTexture=null,this.depthTexture=n.depthTexture,this.samples=n.samples,this.multiview=n.multiview}_setTextureOptions(e={}){let t={minFilter:An,generateMipmaps:!1,flipY:!1,internalFormat:null};e.mapping!==void 0&&(t.mapping=e.mapping),e.wrapS!==void 0&&(t.wrapS=e.wrapS),e.wrapT!==void 0&&(t.wrapT=e.wrapT),e.wrapR!==void 0&&(t.wrapR=e.wrapR),e.magFilter!==void 0&&(t.magFilter=e.magFilter),e.minFilter!==void 0&&(t.minFilter=e.minFilter),e.format!==void 0&&(t.format=e.format),e.type!==void 0&&(t.type=e.type),e.anisotropy!==void 0&&(t.anisotropy=e.anisotropy),e.colorSpace!==void 0&&(t.colorSpace=e.colorSpace),e.flipY!==void 0&&(t.flipY=e.flipY),e.generateMipmaps!==void 0&&(t.generateMipmaps=e.generateMipmaps),e.internalFormat!==void 0&&(t.internalFormat=e.internalFormat);for(let e=0;e<this.textures.length;e++)this.textures[e].setValues(t)}get texture(){return this.textures[0]}set texture(e){this.textures[0]=e}set depthTexture(e){this._depthTexture!==null&&(this._depthTexture.renderTarget=null),e!==null&&(e.renderTarget=this),this._depthTexture=e}get depthTexture(){return this._depthTexture}setSize(e,t,n=1){if(this.width!==e||this.height!==t||this.depth!==n){this.width=e,this.height=t,this.depth=n;for(let r=0,i=this.textures.length;r<i;r++)this.textures[r].image.width=e,this.textures[r].image.height=t,this.textures[r].image.depth=n,this.textures[r].isData3DTexture!==!0&&(this.textures[r].isArrayTexture=this.textures[r].image.depth>1);this.dispose()}this.viewport.set(0,0,e,t),this.scissor.set(0,0,e,t)}clone(){return new this.constructor().copy(this)}copy(e){this.width=e.width,this.height=e.height,this.depth=e.depth,this.scissor.copy(e.scissor),this.scissorTest=e.scissorTest,this.viewport.copy(e.viewport),this.textures.length=0;for(let t=0,n=e.textures.length;t<n;t++){this.textures[t]=e.textures[t].clone(),this.textures[t].isRenderTargetTexture=!0,this.textures[t].renderTarget=this;let n=Object.assign({},e.textures[t].image);this.textures[t].source=new Si(n)}return this.depthBuffer=e.depthBuffer,this.stencilBuffer=e.stencilBuffer,this.resolveDepthBuffer=e.resolveDepthBuffer,this.resolveStencilBuffer=e.resolveStencilBuffer,e.depthTexture!==null&&(this.depthTexture=e.depthTexture.clone()),this.samples=e.samples,this.multiview=e.multiview,this}dispose(){this.dispatchEvent({type:`dispose`})}},Oi=class extends Di{constructor(e=1,t=1,n={}){super(e,t,n),this.isWebGLRenderTarget=!0}},ki=class extends Ti{constructor(e=null,t=1,n=1,r=1){super(null),this.isDataArrayTexture=!0,this.image={data:e,width:t,height:n,depth:r},this.magFilter=Dn,this.minFilter=Dn,this.wrapR=Tn,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1,this.layerUpdates=new Set}addLayerUpdate(e){this.layerUpdates.add(e)}clearLayerUpdates(){this.layerUpdates.clear()}},Ai=class extends Ti{constructor(e=null,t=1,n=1,r=1){super(null),this.isData3DTexture=!0,this.image={data:e,width:t,height:n,depth:r},this.magFilter=Dn,this.minFilter=Dn,this.wrapR=Tn,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}},Y=class e{static{e.prototype.isMatrix4=!0}constructor(e,t,n,r,i,a,o,s,c,l,u,d,f,p,m,h){this.elements=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],e!==void 0&&this.set(e,t,n,r,i,a,o,s,c,l,u,d,f,p,m,h)}set(e,t,n,r,i,a,o,s,c,l,u,d,f,p,m,h){let g=this.elements;return g[0]=e,g[4]=t,g[8]=n,g[12]=r,g[1]=i,g[5]=a,g[9]=o,g[13]=s,g[2]=c,g[6]=l,g[10]=u,g[14]=d,g[3]=f,g[7]=p,g[11]=m,g[15]=h,this}identity(){return this.set(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1),this}clone(){return new e().fromArray(this.elements)}copy(e){let t=this.elements,n=e.elements;return t[0]=n[0],t[1]=n[1],t[2]=n[2],t[3]=n[3],t[4]=n[4],t[5]=n[5],t[6]=n[6],t[7]=n[7],t[8]=n[8],t[9]=n[9],t[10]=n[10],t[11]=n[11],t[12]=n[12],t[13]=n[13],t[14]=n[14],t[15]=n[15],this}copyPosition(e){let t=this.elements,n=e.elements;return t[12]=n[12],t[13]=n[13],t[14]=n[14],this}setFromMatrix3(e){let t=e.elements;return this.set(t[0],t[3],t[6],0,t[1],t[4],t[7],0,t[2],t[5],t[8],0,0,0,0,1),this}extractBasis(e,t,n){return this.determinant()===0?(e.set(1,0,0),t.set(0,1,0),n.set(0,0,1),this):(e.setFromMatrixColumn(this,0),t.setFromMatrixColumn(this,1),n.setFromMatrixColumn(this,2),this)}makeBasis(e,t,n){return this.set(e.x,t.x,n.x,0,e.y,t.y,n.y,0,e.z,t.z,n.z,0,0,0,0,1),this}extractRotation(e){if(e.determinant()===0)return this.identity();let t=this.elements,n=e.elements,r=1/ji.setFromMatrixColumn(e,0).length(),i=1/ji.setFromMatrixColumn(e,1).length(),a=1/ji.setFromMatrixColumn(e,2).length();return t[0]=n[0]*r,t[1]=n[1]*r,t[2]=n[2]*r,t[3]=0,t[4]=n[4]*i,t[5]=n[5]*i,t[6]=n[6]*i,t[7]=0,t[8]=n[8]*a,t[9]=n[9]*a,t[10]=n[10]*a,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromEuler(e){let t=this.elements,n=e.x,r=e.y,i=e.z,a=Math.cos(n),o=Math.sin(n),s=Math.cos(r),c=Math.sin(r),l=Math.cos(i),u=Math.sin(i);if(e.order===`XYZ`){let e=a*l,n=a*u,r=o*l,i=o*u;t[0]=s*l,t[4]=-s*u,t[8]=c,t[1]=n+r*c,t[5]=e-i*c,t[9]=-o*s,t[2]=i-e*c,t[6]=r+n*c,t[10]=a*s}else if(e.order===`YXZ`){let e=s*l,n=s*u,r=c*l,i=c*u;t[0]=e+i*o,t[4]=r*o-n,t[8]=a*c,t[1]=a*u,t[5]=a*l,t[9]=-o,t[2]=n*o-r,t[6]=i+e*o,t[10]=a*s}else if(e.order===`ZXY`){let e=s*l,n=s*u,r=c*l,i=c*u;t[0]=e-i*o,t[4]=-a*u,t[8]=r+n*o,t[1]=n+r*o,t[5]=a*l,t[9]=i-e*o,t[2]=-a*c,t[6]=o,t[10]=a*s}else if(e.order===`ZYX`){let e=a*l,n=a*u,r=o*l,i=o*u;t[0]=s*l,t[4]=r*c-n,t[8]=e*c+i,t[1]=s*u,t[5]=i*c+e,t[9]=n*c-r,t[2]=-c,t[6]=o*s,t[10]=a*s}else if(e.order===`YZX`){let e=a*s,n=a*c,r=o*s,i=o*c;t[0]=s*l,t[4]=i-e*u,t[8]=r*u+n,t[1]=u,t[5]=a*l,t[9]=-o*l,t[2]=-c*l,t[6]=n*u+r,t[10]=e-i*u}else if(e.order===`XZY`){let e=a*s,n=a*c,r=o*s,i=o*c;t[0]=s*l,t[4]=-u,t[8]=c*l,t[1]=e*u+i,t[5]=a*l,t[9]=n*u-r,t[2]=r*u-n,t[6]=o*l,t[10]=i*u+e}return t[3]=0,t[7]=0,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromQuaternion(e){return this.compose(Ni,e,Pi)}lookAt(e,t,n){let r=this.elements;return Li.subVectors(e,t),Li.lengthSq()===0&&(Li.z=1),Li.normalize(),Fi.crossVectors(n,Li),Fi.lengthSq()===0&&(Math.abs(n.z)===1?Li.x+=1e-4:Li.z+=1e-4,Li.normalize(),Fi.crossVectors(n,Li)),Fi.normalize(),Ii.crossVectors(Li,Fi),r[0]=Fi.x,r[4]=Ii.x,r[8]=Li.x,r[1]=Fi.y,r[5]=Ii.y,r[9]=Li.y,r[2]=Fi.z,r[6]=Ii.z,r[10]=Li.z,this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){let n=e.elements,r=t.elements,i=this.elements,a=n[0],o=n[4],s=n[8],c=n[12],l=n[1],u=n[5],d=n[9],f=n[13],p=n[2],m=n[6],h=n[10],g=n[14],_=n[3],v=n[7],y=n[11],b=n[15],x=r[0],S=r[4],C=r[8],w=r[12],T=r[1],E=r[5],D=r[9],O=r[13],ee=r[2],k=r[6],te=r[10],ne=r[14],re=r[3],ie=r[7],ae=r[11],oe=r[15];return i[0]=a*x+o*T+s*ee+c*re,i[4]=a*S+o*E+s*k+c*ie,i[8]=a*C+o*D+s*te+c*ae,i[12]=a*w+o*O+s*ne+c*oe,i[1]=l*x+u*T+d*ee+f*re,i[5]=l*S+u*E+d*k+f*ie,i[9]=l*C+u*D+d*te+f*ae,i[13]=l*w+u*O+d*ne+f*oe,i[2]=p*x+m*T+h*ee+g*re,i[6]=p*S+m*E+h*k+g*ie,i[10]=p*C+m*D+h*te+g*ae,i[14]=p*w+m*O+h*ne+g*oe,i[3]=_*x+v*T+y*ee+b*re,i[7]=_*S+v*E+y*k+b*ie,i[11]=_*C+v*D+y*te+b*ae,i[15]=_*w+v*O+y*ne+b*oe,this}multiplyScalar(e){let t=this.elements;return t[0]*=e,t[4]*=e,t[8]*=e,t[12]*=e,t[1]*=e,t[5]*=e,t[9]*=e,t[13]*=e,t[2]*=e,t[6]*=e,t[10]*=e,t[14]*=e,t[3]*=e,t[7]*=e,t[11]*=e,t[15]*=e,this}determinant(){let e=this.elements,t=e[0],n=e[4],r=e[8],i=e[12],a=e[1],o=e[5],s=e[9],c=e[13],l=e[2],u=e[6],d=e[10],f=e[14],p=e[3],m=e[7],h=e[11],g=e[15],_=s*f-c*d,v=o*f-c*u,y=o*d-s*u,b=a*f-c*l,x=a*d-s*l,S=a*u-o*l;return t*(m*_-h*v+g*y)-n*(p*_-h*b+g*x)+r*(p*v-m*b+g*S)-i*(p*y-m*x+h*S)}transpose(){let e=this.elements,t;return t=e[1],e[1]=e[4],e[4]=t,t=e[2],e[2]=e[8],e[8]=t,t=e[6],e[6]=e[9],e[9]=t,t=e[3],e[3]=e[12],e[12]=t,t=e[7],e[7]=e[13],e[13]=t,t=e[11],e[11]=e[14],e[14]=t,this}setPosition(e,t,n){let r=this.elements;return e.isVector3?(r[12]=e.x,r[13]=e.y,r[14]=e.z):(r[12]=e,r[13]=t,r[14]=n),this}invert(){let e=this.elements,t=e[0],n=e[1],r=e[2],i=e[3],a=e[4],o=e[5],s=e[6],c=e[7],l=e[8],u=e[9],d=e[10],f=e[11],p=e[12],m=e[13],h=e[14],g=e[15],_=t*o-n*a,v=t*s-r*a,y=t*c-i*a,b=n*s-r*o,x=n*c-i*o,S=r*c-i*s,C=l*m-u*p,w=l*h-d*p,T=l*g-f*p,E=u*h-d*m,D=u*g-f*m,O=d*g-f*h,ee=_*O-v*D+y*E+b*T-x*w+S*C;if(ee===0)return this.set(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);let k=1/ee;return e[0]=(o*O-s*D+c*E)*k,e[1]=(r*D-n*O-i*E)*k,e[2]=(m*S-h*x+g*b)*k,e[3]=(d*x-u*S-f*b)*k,e[4]=(s*T-a*O-c*w)*k,e[5]=(t*O-r*T+i*w)*k,e[6]=(h*y-p*S-g*v)*k,e[7]=(l*S-d*y+f*v)*k,e[8]=(a*D-o*T+c*C)*k,e[9]=(n*T-t*D-i*C)*k,e[10]=(p*x-m*y+g*_)*k,e[11]=(u*y-l*x-f*_)*k,e[12]=(o*w-a*E-s*C)*k,e[13]=(t*E-n*w+r*C)*k,e[14]=(m*v-p*b-h*_)*k,e[15]=(l*b-u*v+d*_)*k,this}scale(e){let t=this.elements,n=e.x,r=e.y,i=e.z;return t[0]*=n,t[4]*=r,t[8]*=i,t[1]*=n,t[5]*=r,t[9]*=i,t[2]*=n,t[6]*=r,t[10]*=i,t[3]*=n,t[7]*=r,t[11]*=i,this}getMaxScaleOnAxis(){let e=this.elements,t=e[0]*e[0]+e[1]*e[1]+e[2]*e[2],n=e[4]*e[4]+e[5]*e[5]+e[6]*e[6],r=e[8]*e[8]+e[9]*e[9]+e[10]*e[10];return Math.sqrt(Math.max(t,n,r))}makeTranslation(e,t,n){return e.isVector3?this.set(1,0,0,e.x,0,1,0,e.y,0,0,1,e.z,0,0,0,1):this.set(1,0,0,e,0,1,0,t,0,0,1,n,0,0,0,1),this}makeRotationX(e){let t=Math.cos(e),n=Math.sin(e);return this.set(1,0,0,0,0,t,-n,0,0,n,t,0,0,0,0,1),this}makeRotationY(e){let t=Math.cos(e),n=Math.sin(e);return this.set(t,0,n,0,0,1,0,0,-n,0,t,0,0,0,0,1),this}makeRotationZ(e){let t=Math.cos(e),n=Math.sin(e);return this.set(t,-n,0,0,n,t,0,0,0,0,1,0,0,0,0,1),this}makeRotationAxis(e,t){let n=Math.cos(t),r=Math.sin(t),i=1-n,a=e.x,o=e.y,s=e.z,c=i*a,l=i*o;return this.set(c*a+n,c*o-r*s,c*s+r*o,0,c*o+r*s,l*o+n,l*s-r*a,0,c*s-r*o,l*s+r*a,i*s*s+n,0,0,0,0,1),this}makeScale(e,t,n){return this.set(e,0,0,0,0,t,0,0,0,0,n,0,0,0,0,1),this}makeShear(e,t,n,r,i,a){return this.set(1,n,i,0,e,1,a,0,t,r,1,0,0,0,0,1),this}compose(e,t,n){let r=this.elements,i=t._x,a=t._y,o=t._z,s=t._w,c=i+i,l=a+a,u=o+o,d=i*c,f=i*l,p=i*u,m=a*l,h=a*u,g=o*u,_=s*c,v=s*l,y=s*u,b=n.x,x=n.y,S=n.z;return r[0]=(1-(m+g))*b,r[1]=(f+y)*b,r[2]=(p-v)*b,r[3]=0,r[4]=(f-y)*x,r[5]=(1-(d+g))*x,r[6]=(h+_)*x,r[7]=0,r[8]=(p+v)*S,r[9]=(h-_)*S,r[10]=(1-(d+m))*S,r[11]=0,r[12]=e.x,r[13]=e.y,r[14]=e.z,r[15]=1,this}decompose(e,t,n){let r=this.elements;e.x=r[12],e.y=r[13],e.z=r[14];let i=this.determinant();if(i===0)return n.set(1,1,1),t.identity(),this;let a=ji.set(r[0],r[1],r[2]).length(),o=ji.set(r[4],r[5],r[6]).length(),s=ji.set(r[8],r[9],r[10]).length();i<0&&(a=-a),Mi.copy(this);let c=1/a,l=1/o,u=1/s;return Mi.elements[0]*=c,Mi.elements[1]*=c,Mi.elements[2]*=c,Mi.elements[4]*=l,Mi.elements[5]*=l,Mi.elements[6]*=l,Mi.elements[8]*=u,Mi.elements[9]*=u,Mi.elements[10]*=u,t.setFromRotationMatrix(Mi),n.x=a,n.y=o,n.z=s,this}makePerspective(e,t,n,r,i,a,o=ii,s=!1){let c=this.elements,l=2*i/(t-e),u=2*i/(n-r),d=(t+e)/(t-e),f=(n+r)/(n-r),p,m;if(s)p=i/(a-i),m=a*i/(a-i);else if(o===2e3)p=-(a+i)/(a-i),m=-2*a*i/(a-i);else if(o===2001)p=-a/(a-i),m=-a*i/(a-i);else throw Error(`THREE.Matrix4.makePerspective(): Invalid coordinate system: `+o);return c[0]=l,c[4]=0,c[8]=d,c[12]=0,c[1]=0,c[5]=u,c[9]=f,c[13]=0,c[2]=0,c[6]=0,c[10]=p,c[14]=m,c[3]=0,c[7]=0,c[11]=-1,c[15]=0,this}makeOrthographic(e,t,n,r,i,a,o=ii,s=!1){let c=this.elements,l=2/(t-e),u=2/(n-r),d=-(t+e)/(t-e),f=-(n+r)/(n-r),p,m;if(s)p=1/(a-i),m=a/(a-i);else if(o===2e3)p=-2/(a-i),m=-(a+i)/(a-i);else if(o===2001)p=-1/(a-i),m=-i/(a-i);else throw Error(`THREE.Matrix4.makeOrthographic(): Invalid coordinate system: `+o);return c[0]=l,c[4]=0,c[8]=0,c[12]=d,c[1]=0,c[5]=u,c[9]=0,c[13]=f,c[2]=0,c[6]=0,c[10]=p,c[14]=m,c[3]=0,c[7]=0,c[11]=0,c[15]=1,this}equals(e){let t=this.elements,n=e.elements;for(let e=0;e<16;e++)if(t[e]!==n[e])return!1;return!0}fromArray(e,t=0){for(let n=0;n<16;n++)this.elements[n]=e[n+t];return this}toArray(e=[],t=0){let n=this.elements;return e[t]=n[0],e[t+1]=n[1],e[t+2]=n[2],e[t+3]=n[3],e[t+4]=n[4],e[t+5]=n[5],e[t+6]=n[6],e[t+7]=n[7],e[t+8]=n[8],e[t+9]=n[9],e[t+10]=n[10],e[t+11]=n[11],e[t+12]=n[12],e[t+13]=n[13],e[t+14]=n[14],e[t+15]=n[15],e}},ji=new K,Mi=new Y,Ni=new K(0,0,0),Pi=new K(1,1,1),Fi=new K,Ii=new K,Li=new K,Ri=new Y,zi=new pi,Bi=class e{constructor(t=0,n=0,r=0,i=e.DEFAULT_ORDER){this.isEuler=!0,this._x=t,this._y=n,this._z=r,this._order=i}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get order(){return this._order}set order(e){this._order=e,this._onChangeCallback()}set(e,t,n,r=this._order){return this._x=e,this._y=t,this._z=n,this._order=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._order)}copy(e){return this._x=e._x,this._y=e._y,this._z=e._z,this._order=e._order,this._onChangeCallback(),this}setFromRotationMatrix(e,t=this._order,n=!0){let r=e.elements,i=r[0],a=r[4],o=r[8],s=r[1],c=r[5],l=r[9],u=r[2],d=r[6],f=r[10];switch(t){case`XYZ`:this._y=Math.asin(Kt(o,-1,1)),Math.abs(o)<.9999999?(this._x=Math.atan2(-l,f),this._z=Math.atan2(-a,i)):(this._x=Math.atan2(d,c),this._z=0);break;case`YXZ`:this._x=Math.asin(-Kt(l,-1,1)),Math.abs(l)<.9999999?(this._y=Math.atan2(o,f),this._z=Math.atan2(s,c)):(this._y=Math.atan2(-u,i),this._z=0);break;case`ZXY`:this._x=Math.asin(Kt(d,-1,1)),Math.abs(d)<.9999999?(this._y=Math.atan2(-u,f),this._z=Math.atan2(-a,c)):(this._y=0,this._z=Math.atan2(s,i));break;case`ZYX`:this._y=Math.asin(-Kt(u,-1,1)),Math.abs(u)<.9999999?(this._x=Math.atan2(d,f),this._z=Math.atan2(s,i)):(this._x=0,this._z=Math.atan2(-a,c));break;case`YZX`:this._z=Math.asin(Kt(s,-1,1)),Math.abs(s)<.9999999?(this._x=Math.atan2(-l,c),this._y=Math.atan2(-u,i)):(this._x=0,this._y=Math.atan2(o,f));break;case`XZY`:this._z=Math.asin(-Kt(a,-1,1)),Math.abs(a)<.9999999?(this._x=Math.atan2(d,c),this._y=Math.atan2(o,i)):(this._x=Math.atan2(-l,f),this._y=0);break;default:W(`Euler: .setFromRotationMatrix() encountered an unknown order: `+t)}return this._order=t,n===!0&&this._onChangeCallback(),this}setFromQuaternion(e,t,n){return Ri.makeRotationFromQuaternion(e),this.setFromRotationMatrix(Ri,t,n)}setFromVector3(e,t=this._order){return this.set(e.x,e.y,e.z,t)}reorder(e){return zi.setFromEuler(this),this.setFromQuaternion(zi,e)}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._order===this._order}fromArray(e){return this._x=e[0],this._y=e[1],this._z=e[2],e[3]!==void 0&&(this._order=e[3]),this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._order,e}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._order}},Bi.DEFAULT_ORDER=`XYZ`,Vi=class{constructor(){this.mask=1}set(e){this.mask=(1<<e|0)>>>0}enable(e){this.mask|=1<<e|0}enableAll(){this.mask=-1}toggle(e){this.mask^=1<<e|0}disable(e){this.mask&=~(1<<e|0)}disableAll(){this.mask=0}test(e){return(this.mask&e.mask)!==0}isEnabled(e){return(this.mask&(1<<e|0))!=0}},Hi=0,Ui=new K,Wi=new pi,Gi=new Y,Ki=new K,qi=new K,Ji=new K,Yi=new pi,Xi=new K(1,0,0),Zi=new K(0,1,0),Qi=new K(0,0,1),$i={type:`added`},ea={type:`removed`},ta={type:`childadded`,child:null},na={type:`childremoved`,child:null},ra=class e extends ci{constructor(){super(),this.isObject3D=!0,Object.defineProperty(this,"id",{value:Hi++}),this.uuid=Gt(),this.name=``,this.type=`Object3D`,this.parent=null,this.children=[],this.up=e.DEFAULT_UP.clone();let t=new K,n=new Bi,r=new pi,i=new K(1,1,1);function a(){r.setFromEuler(n,!1)}function o(){n.setFromQuaternion(r,void 0,!1)}n._onChange(a),r._onChange(o),Object.defineProperties(this,{position:{configurable:!0,enumerable:!0,value:t},rotation:{configurable:!0,enumerable:!0,value:n},quaternion:{configurable:!0,enumerable:!0,value:r},scale:{configurable:!0,enumerable:!0,value:i},modelViewMatrix:{value:new Y},normalMatrix:{value:new q}}),this.matrix=new Y,this.matrixWorld=new Y,this.matrixAutoUpdate=e.DEFAULT_MATRIX_AUTO_UPDATE,this.matrixWorldAutoUpdate=e.DEFAULT_MATRIX_WORLD_AUTO_UPDATE,this.matrixWorldNeedsUpdate=!1,this.layers=new Vi,this.visible=!0,this.castShadow=!1,this.receiveShadow=!1,this.frustumCulled=!0,this.renderOrder=0,this.animations=[],this.customDepthMaterial=void 0,this.customDistanceMaterial=void 0,this.static=!1,this.userData={},this.pivot=null}onBeforeShadow(){}onAfterShadow(){}onBeforeRender(){}onAfterRender(){}applyMatrix4(e){this.matrixAutoUpdate&&this.updateMatrix(),this.matrix.premultiply(e),this.matrix.decompose(this.position,this.quaternion,this.scale)}applyQuaternion(e){return this.quaternion.premultiply(e),this}setRotationFromAxisAngle(e,t){this.quaternion.setFromAxisAngle(e,t)}setRotationFromEuler(e){this.quaternion.setFromEuler(e,!0)}setRotationFromMatrix(e){this.quaternion.setFromRotationMatrix(e)}setRotationFromQuaternion(e){this.quaternion.copy(e)}rotateOnAxis(e,t){return Wi.setFromAxisAngle(e,t),this.quaternion.multiply(Wi),this}rotateOnWorldAxis(e,t){return Wi.setFromAxisAngle(e,t),this.quaternion.premultiply(Wi),this}rotateX(e){return this.rotateOnAxis(Xi,e)}rotateY(e){return this.rotateOnAxis(Zi,e)}rotateZ(e){return this.rotateOnAxis(Qi,e)}translateOnAxis(e,t){return Ui.copy(e).applyQuaternion(this.quaternion),this.position.add(Ui.multiplyScalar(t)),this}translateX(e){return this.translateOnAxis(Xi,e)}translateY(e){return this.translateOnAxis(Zi,e)}translateZ(e){return this.translateOnAxis(Qi,e)}localToWorld(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(this.matrixWorld)}worldToLocal(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(Gi.copy(this.matrixWorld).invert())}lookAt(e,t,n){e.isVector3?Ki.copy(e):Ki.set(e,t,n);let r=this.parent;this.updateWorldMatrix(!0,!1),qi.setFromMatrixPosition(this.matrixWorld),this.isCamera||this.isLight?Gi.lookAt(qi,Ki,this.up):Gi.lookAt(Ki,qi,this.up),this.quaternion.setFromRotationMatrix(Gi),r&&(Gi.extractRotation(r.matrixWorld),Wi.setFromRotationMatrix(Gi),this.quaternion.premultiply(Wi.invert()))}add(e){if(arguments.length>1){for(let e=0;e<arguments.length;e++)this.add(arguments[e]);return this}return e===this?(G(`Object3D.add: object can't be added as a child of itself.`,e),this):(e&&e.isObject3D?(e.removeFromParent(),e.parent=this,this.children.push(e),e.dispatchEvent($i),ta.child=e,this.dispatchEvent(ta),ta.child=null):G(`Object3D.add: object not an instance of THREE.Object3D.`,e),this)}remove(e){if(arguments.length>1){for(let e=0;e<arguments.length;e++)this.remove(arguments[e]);return this}let t=this.children.indexOf(e);return t!==-1&&(e.parent=null,this.children.splice(t,1),e.dispatchEvent(ea),na.child=e,this.dispatchEvent(na),na.child=null),this}removeFromParent(){let e=this.parent;return e!==null&&e.remove(this),this}clear(){return this.remove(...this.children)}attach(e){return this.updateWorldMatrix(!0,!1),Gi.copy(this.matrixWorld).invert(),e.parent!==null&&(e.parent.updateWorldMatrix(!0,!1),Gi.multiply(e.parent.matrixWorld)),e.applyMatrix4(Gi),e.removeFromParent(),e.parent=this,this.children.push(e),e.updateWorldMatrix(!1,!0),e.dispatchEvent($i),ta.child=e,this.dispatchEvent(ta),ta.child=null,this}getObjectById(e){return this.getObjectByProperty(`id`,e)}getObjectByName(e){return this.getObjectByProperty(`name`,e)}getObjectByProperty(e,t){if(this[e]===t)return this;for(let n=0,r=this.children.length;n<r;n++){let r=this.children[n].getObjectByProperty(e,t);if(r!==void 0)return r}}getObjectsByProperty(e,t,n=[]){this[e]===t&&n.push(this);let r=this.children;for(let i=0,a=r.length;i<a;i++)r[i].getObjectsByProperty(e,t,n);return n}getWorldPosition(e){return this.updateWorldMatrix(!0,!1),e.setFromMatrixPosition(this.matrixWorld)}getWorldQuaternion(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(qi,e,Ji),e}getWorldScale(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(qi,Yi,e),e}getWorldDirection(e){this.updateWorldMatrix(!0,!1);let t=this.matrixWorld.elements;return e.set(t[8],t[9],t[10]).normalize()}raycast(){}traverse(e){e(this);let t=this.children;for(let n=0,r=t.length;n<r;n++)t[n].traverse(e)}traverseVisible(e){if(this.visible===!1)return;e(this);let t=this.children;for(let n=0,r=t.length;n<r;n++)t[n].traverseVisible(e)}traverseAncestors(e){let t=this.parent;t!==null&&(e(t),t.traverseAncestors(e))}updateMatrix(){this.matrix.compose(this.position,this.quaternion,this.scale);let e=this.pivot;if(e!==null){let t=e.x,n=e.y,r=e.z,i=this.matrix.elements;i[12]+=t-i[0]*t-i[4]*n-i[8]*r,i[13]+=n-i[1]*t-i[5]*n-i[9]*r,i[14]+=r-i[2]*t-i[6]*n-i[10]*r}this.matrixWorldNeedsUpdate=!0}updateMatrixWorld(e){this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||e)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,e=!0);let t=this.children;for(let n=0,r=t.length;n<r;n++)t[n].updateMatrixWorld(e)}updateWorldMatrix(e,t){let n=this.parent;if(e===!0&&n!==null&&n.updateWorldMatrix(!0,!1),this.matrixAutoUpdate&&this.updateMatrix(),this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),t===!0){let e=this.children;for(let t=0,n=e.length;t<n;t++)e[t].updateWorldMatrix(!1,!0)}}toJSON(e){let t=e===void 0||typeof e==`string`,n={};t&&(e={geometries:{},materials:{},textures:{},images:{},shapes:{},skeletons:{},animations:{},nodes:{}},n.metadata={version:4.7,type:`Object`,generator:`Object3D.toJSON`});let r={};r.uuid=this.uuid,r.type=this.type,this.name!==``&&(r.name=this.name),this.castShadow===!0&&(r.castShadow=!0),this.receiveShadow===!0&&(r.receiveShadow=!0),this.visible===!1&&(r.visible=!1),this.frustumCulled===!1&&(r.frustumCulled=!1),this.renderOrder!==0&&(r.renderOrder=this.renderOrder),this.static!==!1&&(r.static=this.static),Object.keys(this.userData).length>0&&(r.userData=this.userData),r.layers=this.layers.mask,r.matrix=this.matrix.toArray(),r.up=this.up.toArray(),this.pivot!==null&&(r.pivot=this.pivot.toArray()),this.matrixAutoUpdate===!1&&(r.matrixAutoUpdate=!1),this.morphTargetDictionary!==void 0&&(r.morphTargetDictionary=Object.assign({},this.morphTargetDictionary)),this.morphTargetInfluences!==void 0&&(r.morphTargetInfluences=this.morphTargetInfluences.slice()),this.isInstancedMesh&&(r.type=`InstancedMesh`,r.count=this.count,r.instanceMatrix=this.instanceMatrix.toJSON(),this.instanceColor!==null&&(r.instanceColor=this.instanceColor.toJSON())),this.isBatchedMesh&&(r.type=`BatchedMesh`,r.perObjectFrustumCulled=this.perObjectFrustumCulled,r.sortObjects=this.sortObjects,r.drawRanges=this._drawRanges,r.reservedRanges=this._reservedRanges,r.geometryInfo=this._geometryInfo.map(e=>({...e,boundingBox:e.boundingBox?e.boundingBox.toJSON():void 0,boundingSphere:e.boundingSphere?e.boundingSphere.toJSON():void 0})),r.instanceInfo=this._instanceInfo.map(e=>({...e})),r.availableInstanceIds=this._availableInstanceIds.slice(),r.availableGeometryIds=this._availableGeometryIds.slice(),r.nextIndexStart=this._nextIndexStart,r.nextVertexStart=this._nextVertexStart,r.geometryCount=this._geometryCount,r.maxInstanceCount=this._maxInstanceCount,r.maxVertexCount=this._maxVertexCount,r.maxIndexCount=this._maxIndexCount,r.geometryInitialized=this._geometryInitialized,r.matricesTexture=this._matricesTexture.toJSON(e),r.indirectTexture=this._indirectTexture.toJSON(e),this._colorsTexture!==null&&(r.colorsTexture=this._colorsTexture.toJSON(e)),this.boundingSphere!==null&&(r.boundingSphere=this.boundingSphere.toJSON()),this.boundingBox!==null&&(r.boundingBox=this.boundingBox.toJSON()));function i(t,n){return t[n.uuid]===void 0&&(t[n.uuid]=n.toJSON(e)),n.uuid}if(this.isScene)this.background&&(this.background.isColor?r.background=this.background.toJSON():this.background.isTexture&&(r.background=this.background.toJSON(e).uuid)),this.environment&&this.environment.isTexture&&this.environment.isRenderTargetTexture!==!0&&(r.environment=this.environment.toJSON(e).uuid);else if(this.isMesh||this.isLine||this.isPoints){r.geometry=i(e.geometries,this.geometry);let t=this.geometry.parameters;if(t!==void 0&&t.shapes!==void 0){let n=t.shapes;if(Array.isArray(n))for(let t=0,r=n.length;t<r;t++){let r=n[t];i(e.shapes,r)}else i(e.shapes,n)}}if(this.isSkinnedMesh&&(r.bindMode=this.bindMode,r.bindMatrix=this.bindMatrix.toArray(),this.skeleton!==void 0&&(i(e.skeletons,this.skeleton),r.skeleton=this.skeleton.uuid)),this.material!==void 0)if(Array.isArray(this.material)){let t=[];for(let n=0,r=this.material.length;n<r;n++)t.push(i(e.materials,this.material[n]));r.material=t}else r.material=i(e.materials,this.material);if(this.children.length>0){r.children=[];for(let t=0;t<this.children.length;t++)r.children.push(this.children[t].toJSON(e).object)}if(this.animations.length>0){r.animations=[];for(let t=0;t<this.animations.length;t++){let n=this.animations[t];r.animations.push(i(e.animations,n))}}if(t){let t=a(e.geometries),r=a(e.materials),i=a(e.textures),o=a(e.images),s=a(e.shapes),c=a(e.skeletons),l=a(e.animations),u=a(e.nodes);t.length>0&&(n.geometries=t),r.length>0&&(n.materials=r),i.length>0&&(n.textures=i),o.length>0&&(n.images=o),s.length>0&&(n.shapes=s),c.length>0&&(n.skeletons=c),l.length>0&&(n.animations=l),u.length>0&&(n.nodes=u)}return n.object=r,n;function a(e){let t=[];for(let n in e){let r=e[n];delete r.metadata,t.push(r)}return t}}clone(e){return new this.constructor().copy(this,e)}copy(e,t=!0){if(this.name=e.name,this.up.copy(e.up),this.position.copy(e.position),this.rotation.order=e.rotation.order,this.quaternion.copy(e.quaternion),this.scale.copy(e.scale),this.pivot=e.pivot===null?null:e.pivot.clone(),this.matrix.copy(e.matrix),this.matrixWorld.copy(e.matrixWorld),this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrixWorldAutoUpdate=e.matrixWorldAutoUpdate,this.matrixWorldNeedsUpdate=e.matrixWorldNeedsUpdate,this.layers.mask=e.layers.mask,this.visible=e.visible,this.castShadow=e.castShadow,this.receiveShadow=e.receiveShadow,this.frustumCulled=e.frustumCulled,this.renderOrder=e.renderOrder,this.static=e.static,this.animations=e.animations.slice(),this.userData=JSON.parse(JSON.stringify(e.userData)),t===!0)for(let t=0;t<e.children.length;t++){let n=e.children[t];this.add(n.clone())}return this}},ra.DEFAULT_UP=new K(0,1,0),ra.DEFAULT_MATRIX_AUTO_UPDATE=!0,ra.DEFAULT_MATRIX_WORLD_AUTO_UPDATE=!0,ia=class extends ra{constructor(){super(),this.isGroup=!0,this.type=`Group`}},aa={type:`move`},oa=class{constructor(){this._targetRay=null,this._grip=null,this._hand=null}getHandSpace(){return this._hand===null&&(this._hand=new ia,this._hand.matrixAutoUpdate=!1,this._hand.visible=!1,this._hand.joints={},this._hand.inputState={pinching:!1}),this._hand}getTargetRaySpace(){return this._targetRay===null&&(this._targetRay=new ia,this._targetRay.matrixAutoUpdate=!1,this._targetRay.visible=!1,this._targetRay.hasLinearVelocity=!1,this._targetRay.linearVelocity=new K,this._targetRay.hasAngularVelocity=!1,this._targetRay.angularVelocity=new K),this._targetRay}getGripSpace(){return this._grip===null&&(this._grip=new ia,this._grip.matrixAutoUpdate=!1,this._grip.visible=!1,this._grip.hasLinearVelocity=!1,this._grip.linearVelocity=new K,this._grip.hasAngularVelocity=!1,this._grip.angularVelocity=new K,this._grip.eventsEnabled=!1),this._grip}dispatchEvent(e){return this._targetRay!==null&&this._targetRay.dispatchEvent(e),this._grip!==null&&this._grip.dispatchEvent(e),this._hand!==null&&this._hand.dispatchEvent(e),this}connect(e){if(e&&e.hand){let t=this._hand;if(t)for(let n of e.hand.values())this._getHandJoint(t,n)}return this.dispatchEvent({type:`connected`,data:e}),this}disconnect(e){return this.dispatchEvent({type:`disconnected`,data:e}),this._targetRay!==null&&(this._targetRay.visible=!1),this._grip!==null&&(this._grip.visible=!1),this._hand!==null&&(this._hand.visible=!1),this}update(e,t,n){let r=null,i=null,a=null,o=this._targetRay,s=this._grip,c=this._hand;if(e&&t.session.visibilityState!==`visible-blurred`){if(c&&e.hand){a=!0;for(let r of e.hand.values()){let e=t.getJointPose(r,n),i=this._getHandJoint(c,r);e!==null&&(i.matrix.fromArray(e.transform.matrix),i.matrix.decompose(i.position,i.rotation,i.scale),i.matrixWorldNeedsUpdate=!0,i.jointRadius=e.radius),i.visible=e!==null}let r=c.joints[`index-finger-tip`],i=c.joints[`thumb-tip`],o=r.position.distanceTo(i.position);c.inputState.pinching&&o>.025?(c.inputState.pinching=!1,this.dispatchEvent({type:`pinchend`,handedness:e.handedness,target:this})):!c.inputState.pinching&&o<=.015&&(c.inputState.pinching=!0,this.dispatchEvent({type:`pinchstart`,handedness:e.handedness,target:this}))}else s!==null&&e.gripSpace&&(i=t.getPose(e.gripSpace,n),i!==null&&(s.matrix.fromArray(i.transform.matrix),s.matrix.decompose(s.position,s.rotation,s.scale),s.matrixWorldNeedsUpdate=!0,i.linearVelocity?(s.hasLinearVelocity=!0,s.linearVelocity.copy(i.linearVelocity)):s.hasLinearVelocity=!1,i.angularVelocity?(s.hasAngularVelocity=!0,s.angularVelocity.copy(i.angularVelocity)):s.hasAngularVelocity=!1,s.eventsEnabled&&s.dispatchEvent({type:`gripUpdated`,data:e,target:this})));o!==null&&(r=t.getPose(e.targetRaySpace,n),r===null&&i!==null&&(r=i),r!==null&&(o.matrix.fromArray(r.transform.matrix),o.matrix.decompose(o.position,o.rotation,o.scale),o.matrixWorldNeedsUpdate=!0,r.linearVelocity?(o.hasLinearVelocity=!0,o.linearVelocity.copy(r.linearVelocity)):o.hasLinearVelocity=!1,r.angularVelocity?(o.hasAngularVelocity=!0,o.angularVelocity.copy(r.angularVelocity)):o.hasAngularVelocity=!1,this.dispatchEvent(aa)))}return o!==null&&(o.visible=r!==null),s!==null&&(s.visible=i!==null),c!==null&&(c.visible=a!==null),this}_getHandJoint(e,t){if(e.joints[t.jointName]===void 0){let n=new ia;n.matrixAutoUpdate=!1,n.visible=!1,e.joints[t.jointName]=n,e.add(n)}return e.joints[t.jointName]}},sa={aliceblue:15792383,antiquewhite:16444375,aqua:65535,aquamarine:8388564,azure:15794175,beige:16119260,bisque:16770244,black:0,blanchedalmond:16772045,blue:255,blueviolet:9055202,brown:10824234,burlywood:14596231,cadetblue:6266528,chartreuse:8388352,chocolate:13789470,coral:16744272,cornflowerblue:6591981,cornsilk:16775388,crimson:14423100,cyan:65535,darkblue:139,darkcyan:35723,darkgoldenrod:12092939,darkgray:11119017,darkgreen:25600,darkgrey:11119017,darkkhaki:12433259,darkmagenta:9109643,darkolivegreen:5597999,darkorange:16747520,darkorchid:10040012,darkred:9109504,darksalmon:15308410,darkseagreen:9419919,darkslateblue:4734347,darkslategray:3100495,darkslategrey:3100495,darkturquoise:52945,darkviolet:9699539,deeppink:16716947,deepskyblue:49151,dimgray:6908265,dimgrey:6908265,dodgerblue:2003199,firebrick:11674146,floralwhite:16775920,forestgreen:2263842,fuchsia:16711935,gainsboro:14474460,ghostwhite:16316671,gold:16766720,goldenrod:14329120,gray:8421504,green:32768,greenyellow:11403055,grey:8421504,honeydew:15794160,hotpink:16738740,indianred:13458524,indigo:4915330,ivory:16777200,khaki:15787660,lavender:15132410,lavenderblush:16773365,lawngreen:8190976,lemonchiffon:16775885,lightblue:11393254,lightcoral:15761536,lightcyan:14745599,lightgoldenrodyellow:16448210,lightgray:13882323,lightgreen:9498256,lightgrey:13882323,lightpink:16758465,lightsalmon:16752762,lightseagreen:2142890,lightskyblue:8900346,lightslategray:7833753,lightslategrey:7833753,lightsteelblue:11584734,lightyellow:16777184,lime:65280,limegreen:3329330,linen:16445670,magenta:16711935,maroon:8388608,mediumaquamarine:6737322,mediumblue:205,mediumorchid:12211667,mediumpurple:9662683,mediumseagreen:3978097,mediumslateblue:8087790,mediumspringgreen:64154,mediumturquoise:4772300,mediumvioletred:13047173,midnightblue:1644912,mintcream:16121850,mistyrose:16770273,moccasin:16770229,navajowhite:16768685,navy:128,oldlace:16643558,olive:8421376,olivedrab:7048739,orange:16753920,orangered:16729344,orchid:14315734,palegoldenrod:15657130,palegreen:10025880,paleturquoise:11529966,palevioletred:14381203,papayawhip:16773077,peachpuff:16767673,peru:13468991,pink:16761035,plum:14524637,powderblue:11591910,purple:8388736,rebeccapurple:6697881,red:16711680,rosybrown:12357519,royalblue:4286945,saddlebrown:9127187,salmon:16416882,sandybrown:16032864,seagreen:3050327,seashell:16774638,sienna:10506797,silver:12632256,skyblue:8900331,slateblue:6970061,slategray:7372944,slategrey:7372944,snow:16775930,springgreen:65407,steelblue:4620980,tan:13808780,teal:32896,thistle:14204888,tomato:16737095,turquoise:4251856,violet:15631086,wheat:16113331,white:16777215,whitesmoke:16119285,yellow:16776960,yellowgreen:10145074},ca={h:0,s:0,l:0},la={h:0,s:0,l:0},X=class{constructor(e,t,n){return this.isColor=!0,this.r=1,this.g=1,this.b=1,this.set(e,t,n)}set(e,t,n){if(t===void 0&&n===void 0){let t=e;t&&t.isColor?this.copy(t):typeof t==`number`?this.setHex(t):typeof t==`string`&&this.setStyle(t)}else this.setRGB(e,t,n);return this}setScalar(e){return this.r=e,this.g=e,this.b=e,this}setHex(e,t=Zr){return e=Math.floor(e),this.r=(e>>16&255)/255,this.g=(e>>8&255)/255,this.b=(e&255)/255,J.colorSpaceToWorking(this,t),this}setRGB(e,t,n,r=J.workingColorSpace){return this.r=e,this.g=t,this.b=n,J.colorSpaceToWorking(this,r),this}setHSL(e,t,n,r=J.workingColorSpace){if(e=qt(e,1),t=Kt(t,0,1),n=Kt(n,0,1),t===0)this.r=this.g=this.b=n;else{let r=n<=.5?n*(1+t):n+t-n*t,i=2*n-r;this.r=tn(i,r,e+1/3),this.g=tn(i,r,e),this.b=tn(i,r,e-1/3)}return J.colorSpaceToWorking(this,r),this}setStyle(e,t=Zr){function n(t){t!==void 0&&parseFloat(t)<1&&W(`Color: Alpha component of `+e+` will be ignored.`)}let r;if(r=/^(\w+)\(([^\)]*)\)/.exec(e)){let i,a=r[1],o=r[2];switch(a){case`rgb`:case`rgba`:if(i=/^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return n(i[4]),this.setRGB(Math.min(255,parseInt(i[1],10))/255,Math.min(255,parseInt(i[2],10))/255,Math.min(255,parseInt(i[3],10))/255,t);if(i=/^\s*(\d+)\%\s*,\s*(\d+)\%\s*,\s*(\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return n(i[4]),this.setRGB(Math.min(100,parseInt(i[1],10))/100,Math.min(100,parseInt(i[2],10))/100,Math.min(100,parseInt(i[3],10))/100,t);break;case`hsl`:case`hsla`:if(i=/^\s*(\d*\.?\d+)\s*,\s*(\d*\.?\d+)\%\s*,\s*(\d*\.?\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return n(i[4]),this.setHSL(parseFloat(i[1])/360,parseFloat(i[2])/100,parseFloat(i[3])/100,t);break;default:W(`Color: Unknown color model `+e)}}else if(r=/^\#([A-Fa-f\d]+)$/.exec(e)){let n=r[1],i=n.length;if(i===3)return this.setRGB(parseInt(n.charAt(0),16)/15,parseInt(n.charAt(1),16)/15,parseInt(n.charAt(2),16)/15,t);if(i===6)return this.setHex(parseInt(n,16),t);W(`Color: Invalid hex color `+e)}else if(e&&e.length>0)return this.setColorName(e,t);return this}setColorName(e,t=Zr){let n=sa[e.toLowerCase()];return n===void 0?W(`Color: Unknown color `+e):this.setHex(n,t),this}clone(){return new this.constructor(this.r,this.g,this.b)}copy(e){return this.r=e.r,this.g=e.g,this.b=e.b,this}copySRGBToLinear(e){return this.r=Qt(e.r),this.g=Qt(e.g),this.b=Qt(e.b),this}copyLinearToSRGB(e){return this.r=$t(e.r),this.g=$t(e.g),this.b=$t(e.b),this}convertSRGBToLinear(){return this.copySRGBToLinear(this),this}convertLinearToSRGB(){return this.copyLinearToSRGB(this),this}getHex(e=Zr){return J.workingToColorSpace(ua.copy(this),e),Math.round(Kt(ua.r*255,0,255))*65536+Math.round(Kt(ua.g*255,0,255))*256+Math.round(Kt(ua.b*255,0,255))}getHexString(e=Zr){return(`000000`+this.getHex(e).toString(16)).slice(-6)}getHSL(e,t=J.workingColorSpace){J.workingToColorSpace(ua.copy(this),t);let n=ua.r,r=ua.g,i=ua.b,a=Math.max(n,r,i),o=Math.min(n,r,i),s,c,l=(o+a)/2;if(o===a)s=0,c=0;else{let e=a-o;switch(c=l<=.5?e/(a+o):e/(2-a-o),a){case n:s=(r-i)/e+(r<i?6:0);break;case r:s=(i-n)/e+2;break;case i:s=(n-r)/e+4;break}s/=6}return e.h=s,e.s=c,e.l=l,e}getRGB(e,t=J.workingColorSpace){return J.workingToColorSpace(ua.copy(this),t),e.r=ua.r,e.g=ua.g,e.b=ua.b,e}getStyle(e=Zr){J.workingToColorSpace(ua.copy(this),e);let t=ua.r,n=ua.g,r=ua.b;return e===`srgb`?`rgb(${Math.round(t*255)},${Math.round(n*255)},${Math.round(r*255)})`:`color(${e} ${t.toFixed(3)} ${n.toFixed(3)} ${r.toFixed(3)})`}offsetHSL(e,t,n){return this.getHSL(ca),this.setHSL(ca.h+e,ca.s+t,ca.l+n)}add(e){return this.r+=e.r,this.g+=e.g,this.b+=e.b,this}addColors(e,t){return this.r=e.r+t.r,this.g=e.g+t.g,this.b=e.b+t.b,this}addScalar(e){return this.r+=e,this.g+=e,this.b+=e,this}sub(e){return this.r=Math.max(0,this.r-e.r),this.g=Math.max(0,this.g-e.g),this.b=Math.max(0,this.b-e.b),this}multiply(e){return this.r*=e.r,this.g*=e.g,this.b*=e.b,this}multiplyScalar(e){return this.r*=e,this.g*=e,this.b*=e,this}lerp(e,t){return this.r+=(e.r-this.r)*t,this.g+=(e.g-this.g)*t,this.b+=(e.b-this.b)*t,this}lerpColors(e,t,n){return this.r=e.r+(t.r-e.r)*n,this.g=e.g+(t.g-e.g)*n,this.b=e.b+(t.b-e.b)*n,this}lerpHSL(e,t){this.getHSL(ca),e.getHSL(la);let n=Jt(ca.h,la.h,t),r=Jt(ca.s,la.s,t),i=Jt(ca.l,la.l,t);return this.setHSL(n,r,i),this}setFromVector3(e){return this.r=e.x,this.g=e.y,this.b=e.z,this}applyMatrix3(e){let t=this.r,n=this.g,r=this.b,i=e.elements;return this.r=i[0]*t+i[3]*n+i[6]*r,this.g=i[1]*t+i[4]*n+i[7]*r,this.b=i[2]*t+i[5]*n+i[8]*r,this}equals(e){return e.r===this.r&&e.g===this.g&&e.b===this.b}fromArray(e,t=0){return this.r=e[t],this.g=e[t+1],this.b=e[t+2],this}toArray(e=[],t=0){return e[t]=this.r,e[t+1]=this.g,e[t+2]=this.b,e}fromBufferAttribute(e,t){return this.r=e.getX(t),this.g=e.getY(t),this.b=e.getZ(t),this}toJSON(){return this.getHex()}*[Symbol.iterator](){yield this.r,yield this.g,yield this.b}},ua=new X,X.NAMES=sa,da=class extends ra{constructor(){super(),this.isScene=!0,this.type=`Scene`,this.background=null,this.environment=null,this.fog=null,this.backgroundBlurriness=0,this.backgroundIntensity=1,this.backgroundRotation=new Bi,this.environmentIntensity=1,this.environmentRotation=new Bi,this.overrideMaterial=null,typeof __THREE_DEVTOOLS__<`u`&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent(`observe`,{detail:this}))}copy(e,t){return super.copy(e,t),e.background!==null&&(this.background=e.background.clone()),e.environment!==null&&(this.environment=e.environment.clone()),e.fog!==null&&(this.fog=e.fog.clone()),this.backgroundBlurriness=e.backgroundBlurriness,this.backgroundIntensity=e.backgroundIntensity,this.backgroundRotation.copy(e.backgroundRotation),this.environmentIntensity=e.environmentIntensity,this.environmentRotation.copy(e.environmentRotation),e.overrideMaterial!==null&&(this.overrideMaterial=e.overrideMaterial.clone()),this.matrixAutoUpdate=e.matrixAutoUpdate,this}toJSON(e){let t=super.toJSON(e);return this.fog!==null&&(t.object.fog=this.fog.toJSON()),this.backgroundBlurriness>0&&(t.object.backgroundBlurriness=this.backgroundBlurriness),this.backgroundIntensity!==1&&(t.object.backgroundIntensity=this.backgroundIntensity),t.object.backgroundRotation=this.backgroundRotation.toArray(),this.environmentIntensity!==1&&(t.object.environmentIntensity=this.environmentIntensity),t.object.environmentRotation=this.environmentRotation.toArray(),t}},fa=new K,pa=new K,ma=new K,ha=new K,ga=new K,_a=new K,va=new K,ya=new K,ba=new K,xa=new K,Sa=new Ei,Ca=new Ei,wa=new Ei,Ta=class e{constructor(e=new K,t=new K,n=new K){this.a=e,this.b=t,this.c=n}static getNormal(e,t,n,r){r.subVectors(n,t),fa.subVectors(e,t),r.cross(fa);let i=r.lengthSq();return i>0?r.multiplyScalar(1/Math.sqrt(i)):r.set(0,0,0)}static getBarycoord(e,t,n,r,i){fa.subVectors(r,t),pa.subVectors(n,t),ma.subVectors(e,t);let a=fa.dot(fa),o=fa.dot(pa),s=fa.dot(ma),c=pa.dot(pa),l=pa.dot(ma),u=a*c-o*o;if(u===0)return i.set(0,0,0),null;let d=1/u,f=(c*s-o*l)*d,p=(a*l-o*s)*d;return i.set(1-f-p,p,f)}static containsPoint(e,t,n,r){return this.getBarycoord(e,t,n,r,ha)!==null&&ha.x>=0&&ha.y>=0&&ha.x+ha.y<=1}static getInterpolation(e,t,n,r,i,a,o,s){return this.getBarycoord(e,t,n,r,ha)===null?(s.x=0,s.y=0,`z`in s&&(s.z=0),`w`in s&&(s.w=0),null):(s.setScalar(0),s.addScaledVector(i,ha.x),s.addScaledVector(a,ha.y),s.addScaledVector(o,ha.z),s)}static getInterpolatedAttribute(e,t,n,r,i,a){return Sa.setScalar(0),Ca.setScalar(0),wa.setScalar(0),Sa.fromBufferAttribute(e,t),Ca.fromBufferAttribute(e,n),wa.fromBufferAttribute(e,r),a.setScalar(0),a.addScaledVector(Sa,i.x),a.addScaledVector(Ca,i.y),a.addScaledVector(wa,i.z),a}static isFrontFacing(e,t,n,r){return fa.subVectors(n,t),pa.subVectors(e,t),fa.cross(pa).dot(r)<0}set(e,t,n){return this.a.copy(e),this.b.copy(t),this.c.copy(n),this}setFromPointsAndIndices(e,t,n,r){return this.a.copy(e[t]),this.b.copy(e[n]),this.c.copy(e[r]),this}setFromAttributeAndIndices(e,t,n,r){return this.a.fromBufferAttribute(e,t),this.b.fromBufferAttribute(e,n),this.c.fromBufferAttribute(e,r),this}clone(){return new this.constructor().copy(this)}copy(e){return this.a.copy(e.a),this.b.copy(e.b),this.c.copy(e.c),this}getArea(){return fa.subVectors(this.c,this.b),pa.subVectors(this.a,this.b),fa.cross(pa).length()*.5}getMidpoint(e){return e.addVectors(this.a,this.b).add(this.c).multiplyScalar(1/3)}getNormal(t){return e.getNormal(this.a,this.b,this.c,t)}getPlane(e){return e.setFromCoplanarPoints(this.a,this.b,this.c)}getBarycoord(t,n){return e.getBarycoord(t,this.a,this.b,this.c,n)}getInterpolation(t,n,r,i,a){return e.getInterpolation(t,this.a,this.b,this.c,n,r,i,a)}containsPoint(t){return e.containsPoint(t,this.a,this.b,this.c)}isFrontFacing(t){return e.isFrontFacing(this.a,this.b,this.c,t)}intersectsBox(e){return e.intersectsTriangle(this)}closestPointToPoint(e,t){let n=this.a,r=this.b,i=this.c,a,o;ga.subVectors(r,n),_a.subVectors(i,n),ya.subVectors(e,n);let s=ga.dot(ya),c=_a.dot(ya);if(s<=0&&c<=0)return t.copy(n);ba.subVectors(e,r);let l=ga.dot(ba),u=_a.dot(ba);if(l>=0&&u<=l)return t.copy(r);let d=s*u-l*c;if(d<=0&&s>=0&&l<=0)return a=s/(s-l),t.copy(n).addScaledVector(ga,a);xa.subVectors(e,i);let f=ga.dot(xa),p=_a.dot(xa);if(p>=0&&f<=p)return t.copy(i);let m=f*c-s*p;if(m<=0&&c>=0&&p<=0)return o=c/(c-p),t.copy(n).addScaledVector(_a,o);let h=l*p-f*u;if(h<=0&&u-l>=0&&f-p>=0)return va.subVectors(i,r),o=(u-l)/(u-l+(f-p)),t.copy(r).addScaledVector(va,o);let g=1/(h+m+d);return a=m*g,o=d*g,t.copy(n).addScaledVector(ga,a).addScaledVector(_a,o)}equals(e){return e.a.equals(this.a)&&e.b.equals(this.b)&&e.c.equals(this.c)}},Ea=class{constructor(e=new K(1/0,1/0,1/0),t=new K(-1/0,-1/0,-1/0)){this.isBox3=!0,this.min=e,this.max=t}set(e,t){return this.min.copy(e),this.max.copy(t),this}setFromArray(e){this.makeEmpty();for(let t=0,n=e.length;t<n;t+=3)this.expandByPoint(Oa.fromArray(e,t));return this}setFromBufferAttribute(e){this.makeEmpty();for(let t=0,n=e.count;t<n;t++)this.expandByPoint(Oa.fromBufferAttribute(e,t));return this}setFromPoints(e){this.makeEmpty();for(let t=0,n=e.length;t<n;t++)this.expandByPoint(e[t]);return this}setFromCenterAndSize(e,t){let n=Oa.copy(t).multiplyScalar(.5);return this.min.copy(e).sub(n),this.max.copy(e).add(n),this}setFromObject(e,t=!1){return this.makeEmpty(),this.expandByObject(e,t)}clone(){return new this.constructor().copy(this)}copy(e){return this.min.copy(e.min),this.max.copy(e.max),this}makeEmpty(){return this.min.x=this.min.y=this.min.z=1/0,this.max.x=this.max.y=this.max.z=-1/0,this}isEmpty(){return this.max.x<this.min.x||this.max.y<this.min.y||this.max.z<this.min.z}getCenter(e){return this.isEmpty()?e.set(0,0,0):e.addVectors(this.min,this.max).multiplyScalar(.5)}getSize(e){return this.isEmpty()?e.set(0,0,0):e.subVectors(this.max,this.min)}expandByPoint(e){return this.min.min(e),this.max.max(e),this}expandByVector(e){return this.min.sub(e),this.max.add(e),this}expandByScalar(e){return this.min.addScalar(-e),this.max.addScalar(e),this}expandByObject(e,t=!1){e.updateWorldMatrix(!1,!1);let n=e.geometry;if(n!==void 0){let r=n.getAttribute(`position`);if(t===!0&&r!==void 0&&e.isInstancedMesh!==!0)for(let t=0,n=r.count;t<n;t++)e.isMesh===!0?e.getVertexPosition(t,Oa):Oa.fromBufferAttribute(r,t),Oa.applyMatrix4(e.matrixWorld),this.expandByPoint(Oa);else e.boundingBox===void 0?(n.boundingBox===null&&n.computeBoundingBox(),ka.copy(n.boundingBox)):(e.boundingBox===null&&e.computeBoundingBox(),ka.copy(e.boundingBox)),ka.applyMatrix4(e.matrixWorld),this.union(ka)}let r=e.children;for(let e=0,n=r.length;e<n;e++)this.expandByObject(r[e],t);return this}containsPoint(e){return e.x>=this.min.x&&e.x<=this.max.x&&e.y>=this.min.y&&e.y<=this.max.y&&e.z>=this.min.z&&e.z<=this.max.z}containsBox(e){return this.min.x<=e.min.x&&e.max.x<=this.max.x&&this.min.y<=e.min.y&&e.max.y<=this.max.y&&this.min.z<=e.min.z&&e.max.z<=this.max.z}getParameter(e,t){return t.set((e.x-this.min.x)/(this.max.x-this.min.x),(e.y-this.min.y)/(this.max.y-this.min.y),(e.z-this.min.z)/(this.max.z-this.min.z))}intersectsBox(e){return e.max.x>=this.min.x&&e.min.x<=this.max.x&&e.max.y>=this.min.y&&e.min.y<=this.max.y&&e.max.z>=this.min.z&&e.min.z<=this.max.z}intersectsSphere(e){return this.clampPoint(e.center,Oa),Oa.distanceToSquared(e.center)<=e.radius*e.radius}intersectsPlane(e){let t,n;return e.normal.x>0?(t=e.normal.x*this.min.x,n=e.normal.x*this.max.x):(t=e.normal.x*this.max.x,n=e.normal.x*this.min.x),e.normal.y>0?(t+=e.normal.y*this.min.y,n+=e.normal.y*this.max.y):(t+=e.normal.y*this.max.y,n+=e.normal.y*this.min.y),e.normal.z>0?(t+=e.normal.z*this.min.z,n+=e.normal.z*this.max.z):(t+=e.normal.z*this.max.z,n+=e.normal.z*this.min.z),t<=-e.constant&&n>=-e.constant}intersectsTriangle(e){if(this.isEmpty())return!1;this.getCenter(Ia),La.subVectors(this.max,Ia),Aa.subVectors(e.a,Ia),ja.subVectors(e.b,Ia),Ma.subVectors(e.c,Ia),Na.subVectors(ja,Aa),Pa.subVectors(Ma,ja),Fa.subVectors(Aa,Ma);let t=[0,-Na.z,Na.y,0,-Pa.z,Pa.y,0,-Fa.z,Fa.y,Na.z,0,-Na.x,Pa.z,0,-Pa.x,Fa.z,0,-Fa.x,-Na.y,Na.x,0,-Pa.y,Pa.x,0,-Fa.y,Fa.x,0];return!nn(t,Aa,ja,Ma,La)||(t=[1,0,0,0,1,0,0,0,1],!nn(t,Aa,ja,Ma,La))?!1:(Ra.crossVectors(Na,Pa),t=[Ra.x,Ra.y,Ra.z],nn(t,Aa,ja,Ma,La))}clampPoint(e,t){return t.copy(e).clamp(this.min,this.max)}distanceToPoint(e){return this.clampPoint(e,Oa).distanceTo(e)}getBoundingSphere(e){return this.isEmpty()?e.makeEmpty():(this.getCenter(e.center),e.radius=this.getSize(Oa).length()*.5),e}intersect(e){return this.min.max(e.min),this.max.min(e.max),this.isEmpty()&&this.makeEmpty(),this}union(e){return this.min.min(e.min),this.max.max(e.max),this}applyMatrix4(e){return this.isEmpty()?this:(Da[0].set(this.min.x,this.min.y,this.min.z).applyMatrix4(e),Da[1].set(this.min.x,this.min.y,this.max.z).applyMatrix4(e),Da[2].set(this.min.x,this.max.y,this.min.z).applyMatrix4(e),Da[3].set(this.min.x,this.max.y,this.max.z).applyMatrix4(e),Da[4].set(this.max.x,this.min.y,this.min.z).applyMatrix4(e),Da[5].set(this.max.x,this.min.y,this.max.z).applyMatrix4(e),Da[6].set(this.max.x,this.max.y,this.min.z).applyMatrix4(e),Da[7].set(this.max.x,this.max.y,this.max.z).applyMatrix4(e),this.setFromPoints(Da),this)}translate(e){return this.min.add(e),this.max.add(e),this}equals(e){return e.min.equals(this.min)&&e.max.equals(this.max)}toJSON(){return{min:this.min.toArray(),max:this.max.toArray()}}fromJSON(e){return this.min.fromArray(e.min),this.max.fromArray(e.max),this}},Da=[new K,new K,new K,new K,new K,new K,new K,new K],Oa=new K,ka=new Ea,Aa=new K,ja=new K,Ma=new K,Na=new K,Pa=new K,Fa=new K,Ia=new K,La=new K,Ra=new K,za=new K,Ba=new K,Va=new fi,Ha=0,Ua=class extends ci{constructor(e,t,n=!1){if(super(),Array.isArray(e))throw TypeError(`THREE.BufferAttribute: array should be a Typed Array.`);this.isBufferAttribute=!0,Object.defineProperty(this,"id",{value:Ha++}),this.name=``,this.array=e,this.itemSize=t,this.count=e===void 0?0:e.length/t,this.normalized=n,this.usage=ni,this.updateRanges=[],this.gpuType=zn,this.version=0}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.name=e.name,this.array=new e.array.constructor(e.array),this.itemSize=e.itemSize,this.count=e.count,this.normalized=e.normalized,this.usage=e.usage,this.gpuType=e.gpuType,this}copyAt(e,t,n){e*=this.itemSize,n*=t.itemSize;for(let r=0,i=this.itemSize;r<i;r++)this.array[e+r]=t.array[n+r];return this}copyArray(e){return this.array.set(e),this}applyMatrix3(e){if(this.itemSize===2)for(let t=0,n=this.count;t<n;t++)Va.fromBufferAttribute(this,t),Va.applyMatrix3(e),this.setXY(t,Va.x,Va.y);else if(this.itemSize===3)for(let t=0,n=this.count;t<n;t++)Ba.fromBufferAttribute(this,t),Ba.applyMatrix3(e),this.setXYZ(t,Ba.x,Ba.y,Ba.z);return this}applyMatrix4(e){for(let t=0,n=this.count;t<n;t++)Ba.fromBufferAttribute(this,t),Ba.applyMatrix4(e),this.setXYZ(t,Ba.x,Ba.y,Ba.z);return this}applyNormalMatrix(e){for(let t=0,n=this.count;t<n;t++)Ba.fromBufferAttribute(this,t),Ba.applyNormalMatrix(e),this.setXYZ(t,Ba.x,Ba.y,Ba.z);return this}transformDirection(e){for(let t=0,n=this.count;t<n;t++)Ba.fromBufferAttribute(this,t),Ba.transformDirection(e),this.setXYZ(t,Ba.x,Ba.y,Ba.z);return this}set(e,t=0){return this.array.set(e,t),this}getComponent(e,t){let n=this.array[e*this.itemSize+t];return this.normalized&&(n=Yt(n,this.array)),n}setComponent(e,t,n){return this.normalized&&(n=Xt(n,this.array)),this.array[e*this.itemSize+t]=n,this}getX(e){let t=this.array[e*this.itemSize];return this.normalized&&(t=Yt(t,this.array)),t}setX(e,t){return this.normalized&&(t=Xt(t,this.array)),this.array[e*this.itemSize]=t,this}getY(e){let t=this.array[e*this.itemSize+1];return this.normalized&&(t=Yt(t,this.array)),t}setY(e,t){return this.normalized&&(t=Xt(t,this.array)),this.array[e*this.itemSize+1]=t,this}getZ(e){let t=this.array[e*this.itemSize+2];return this.normalized&&(t=Yt(t,this.array)),t}setZ(e,t){return this.normalized&&(t=Xt(t,this.array)),this.array[e*this.itemSize+2]=t,this}getW(e){let t=this.array[e*this.itemSize+3];return this.normalized&&(t=Yt(t,this.array)),t}setW(e,t){return this.normalized&&(t=Xt(t,this.array)),this.array[e*this.itemSize+3]=t,this}setXY(e,t,n){return e*=this.itemSize,this.normalized&&(t=Xt(t,this.array),n=Xt(n,this.array)),this.array[e+0]=t,this.array[e+1]=n,this}setXYZ(e,t,n,r){return e*=this.itemSize,this.normalized&&(t=Xt(t,this.array),n=Xt(n,this.array),r=Xt(r,this.array)),this.array[e+0]=t,this.array[e+1]=n,this.array[e+2]=r,this}setXYZW(e,t,n,r,i){return e*=this.itemSize,this.normalized&&(t=Xt(t,this.array),n=Xt(n,this.array),r=Xt(r,this.array),i=Xt(i,this.array)),this.array[e+0]=t,this.array[e+1]=n,this.array[e+2]=r,this.array[e+3]=i,this}onUpload(e){return this.onUploadCallback=e,this}clone(){return new this.constructor(this.array,this.itemSize).copy(this)}toJSON(){let e={itemSize:this.itemSize,type:this.array.constructor.name,array:Array.from(this.array),normalized:this.normalized};return this.name!==``&&(e.name=this.name),this.usage!==35044&&(e.usage=this.usage),e}dispose(){this.dispatchEvent({type:`dispose`})}},Wa=class extends Ua{constructor(e,t,n){super(new Uint16Array(e),t,n)}},Ga=class extends Ua{constructor(e,t,n){super(new Uint32Array(e),t,n)}},Ka=class extends Ua{constructor(e,t,n){super(new Float32Array(e),t,n)}},qa=new Ea,Ja=new K,Ya=new K,Xa=class{constructor(e=new K,t=-1){this.isSphere=!0,this.center=e,this.radius=t}set(e,t){return this.center.copy(e),this.radius=t,this}setFromPoints(e,t){let n=this.center;t===void 0?qa.setFromPoints(e).getCenter(n):n.copy(t);let r=0;for(let t=0,i=e.length;t<i;t++)r=Math.max(r,n.distanceToSquared(e[t]));return this.radius=Math.sqrt(r),this}copy(e){return this.center.copy(e.center),this.radius=e.radius,this}isEmpty(){return this.radius<0}makeEmpty(){return this.center.set(0,0,0),this.radius=-1,this}containsPoint(e){return e.distanceToSquared(this.center)<=this.radius*this.radius}distanceToPoint(e){return e.distanceTo(this.center)-this.radius}intersectsSphere(e){let t=this.radius+e.radius;return e.center.distanceToSquared(this.center)<=t*t}intersectsBox(e){return e.intersectsSphere(this)}intersectsPlane(e){return Math.abs(e.distanceToPoint(this.center))<=this.radius}clampPoint(e,t){let n=this.center.distanceToSquared(e);return t.copy(e),n>this.radius*this.radius&&(t.sub(this.center).normalize(),t.multiplyScalar(this.radius).add(this.center)),t}getBoundingBox(e){return this.isEmpty()?(e.makeEmpty(),e):(e.set(this.center,this.center),e.expandByScalar(this.radius),e)}applyMatrix4(e){return this.center.applyMatrix4(e),this.radius*=e.getMaxScaleOnAxis(),this}translate(e){return this.center.add(e),this}expandByPoint(e){if(this.isEmpty())return this.center.copy(e),this.radius=0,this;Ja.subVectors(e,this.center);let t=Ja.lengthSq();if(t>this.radius*this.radius){let e=Math.sqrt(t),n=(e-this.radius)*.5;this.center.addScaledVector(Ja,n/e),this.radius+=n}return this}union(e){return e.isEmpty()?this:this.isEmpty()?(this.copy(e),this):(this.center.equals(e.center)===!0?this.radius=Math.max(this.radius,e.radius):(Ya.subVectors(e.center,this.center).setLength(e.radius),this.expandByPoint(Ja.copy(e.center).add(Ya)),this.expandByPoint(Ja.copy(e.center).sub(Ya))),this)}equals(e){return e.center.equals(this.center)&&e.radius===this.radius}clone(){return new this.constructor().copy(this)}toJSON(){return{radius:this.radius,center:this.center.toArray()}}fromJSON(e){return this.radius=e.radius,this.center.fromArray(e.center),this}},Za=0,Qa=new Y,$a=new ra,eo=new K,to=new Ea,no=new Ea,ro=new K,io=class e extends ci{constructor(){super(),this.isBufferGeometry=!0,Object.defineProperty(this,"id",{value:Za++}),this.uuid=Gt(),this.name=``,this.type=`BufferGeometry`,this.index=null,this.indirect=null,this.indirectOffset=0,this.attributes={},this.morphAttributes={},this.morphTargetsRelative=!1,this.groups=[],this.boundingBox=null,this.boundingSphere=null,this.drawRange={start:0,count:1/0},this.userData={}}getIndex(){return this.index}setIndex(e){return Array.isArray(e)?this.index=new(Lt(e)?Ga:Wa)(e,1):this.index=e,this}setIndirect(e,t=0){return this.indirect=e,this.indirectOffset=t,this}getIndirect(){return this.indirect}getAttribute(e){return this.attributes[e]}setAttribute(e,t){return this.attributes[e]=t,this}deleteAttribute(e){return delete this.attributes[e],this}hasAttribute(e){return this.attributes[e]!==void 0}addGroup(e,t,n=0){this.groups.push({start:e,count:t,materialIndex:n})}clearGroups(){this.groups=[]}setDrawRange(e,t){this.drawRange.start=e,this.drawRange.count=t}applyMatrix4(e){let t=this.attributes.position;t!==void 0&&(t.applyMatrix4(e),t.needsUpdate=!0);let n=this.attributes.normal;if(n!==void 0){let t=new q().getNormalMatrix(e);n.applyNormalMatrix(t),n.needsUpdate=!0}let r=this.attributes.tangent;return r!==void 0&&(r.transformDirection(e),r.needsUpdate=!0),this.boundingBox!==null&&this.computeBoundingBox(),this.boundingSphere!==null&&this.computeBoundingSphere(),this}applyQuaternion(e){return Qa.makeRotationFromQuaternion(e),this.applyMatrix4(Qa),this}rotateX(e){return Qa.makeRotationX(e),this.applyMatrix4(Qa),this}rotateY(e){return Qa.makeRotationY(e),this.applyMatrix4(Qa),this}rotateZ(e){return Qa.makeRotationZ(e),this.applyMatrix4(Qa),this}translate(e,t,n){return Qa.makeTranslation(e,t,n),this.applyMatrix4(Qa),this}scale(e,t,n){return Qa.makeScale(e,t,n),this.applyMatrix4(Qa),this}lookAt(e){return $a.lookAt(e),$a.updateMatrix(),this.applyMatrix4($a.matrix),this}center(){return this.computeBoundingBox(),this.boundingBox.getCenter(eo).negate(),this.translate(eo.x,eo.y,eo.z),this}setFromPoints(e){let t=this.getAttribute(`position`);if(t===void 0){let t=[];for(let n=0,r=e.length;n<r;n++){let r=e[n];t.push(r.x,r.y,r.z||0)}this.setAttribute(`position`,new Ka(t,3))}else{let n=Math.min(e.length,t.count);for(let r=0;r<n;r++){let n=e[r];t.setXYZ(r,n.x,n.y,n.z||0)}e.length>t.count&&W(`BufferGeometry: Buffer size too small for points data. Use .dispose() and create a new geometry.`),t.needsUpdate=!0}return this}computeBoundingBox(){this.boundingBox===null&&(this.boundingBox=new Ea);let e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){G(`BufferGeometry.computeBoundingBox(): GLBufferAttribute requires a manual bounding box.`,this),this.boundingBox.set(new K(-1/0,-1/0,-1/0),new K(1/0,1/0,1/0));return}if(e!==void 0){if(this.boundingBox.setFromBufferAttribute(e),t)for(let e=0,n=t.length;e<n;e++){let n=t[e];to.setFromBufferAttribute(n),this.morphTargetsRelative?(ro.addVectors(this.boundingBox.min,to.min),this.boundingBox.expandByPoint(ro),ro.addVectors(this.boundingBox.max,to.max),this.boundingBox.expandByPoint(ro)):(this.boundingBox.expandByPoint(to.min),this.boundingBox.expandByPoint(to.max))}}else this.boundingBox.makeEmpty();(isNaN(this.boundingBox.min.x)||isNaN(this.boundingBox.min.y)||isNaN(this.boundingBox.min.z))&&G(`BufferGeometry.computeBoundingBox(): Computed min/max have NaN values. The "position" attribute is likely to have NaN values.`,this)}computeBoundingSphere(){this.boundingSphere===null&&(this.boundingSphere=new Xa);let e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){G(`BufferGeometry.computeBoundingSphere(): GLBufferAttribute requires a manual bounding sphere.`,this),this.boundingSphere.set(new K,1/0);return}if(e){let n=this.boundingSphere.center;if(to.setFromBufferAttribute(e),t)for(let e=0,n=t.length;e<n;e++){let n=t[e];no.setFromBufferAttribute(n),this.morphTargetsRelative?(ro.addVectors(to.min,no.min),to.expandByPoint(ro),ro.addVectors(to.max,no.max),to.expandByPoint(ro)):(to.expandByPoint(no.min),to.expandByPoint(no.max))}to.getCenter(n);let r=0;for(let t=0,i=e.count;t<i;t++)ro.fromBufferAttribute(e,t),r=Math.max(r,n.distanceToSquared(ro));if(t)for(let i=0,a=t.length;i<a;i++){let a=t[i],o=this.morphTargetsRelative;for(let t=0,i=a.count;t<i;t++)ro.fromBufferAttribute(a,t),o&&(eo.fromBufferAttribute(e,t),ro.add(eo)),r=Math.max(r,n.distanceToSquared(ro))}this.boundingSphere.radius=Math.sqrt(r),isNaN(this.boundingSphere.radius)&&G(`BufferGeometry.computeBoundingSphere(): Computed radius is NaN. The "position" attribute is likely to have NaN values.`,this)}}computeTangents(){let e=this.index,t=this.attributes;if(e===null||t.position===void 0||t.normal===void 0||t.uv===void 0){G(`BufferGeometry: .computeTangents() failed. Missing required attributes (index, position, normal or uv)`);return}let n=t.position,r=t.normal,i=t.uv;this.hasAttribute(`tangent`)===!1&&this.setAttribute(`tangent`,new Ua(new Float32Array(4*n.count),4));let a=this.getAttribute(`tangent`),o=[],s=[];for(let e=0;e<n.count;e++)o[e]=new K,s[e]=new K;let c=new K,l=new K,u=new K,d=new fi,f=new fi,p=new fi,m=new K,h=new K;function g(e,t,r){c.fromBufferAttribute(n,e),l.fromBufferAttribute(n,t),u.fromBufferAttribute(n,r),d.fromBufferAttribute(i,e),f.fromBufferAttribute(i,t),p.fromBufferAttribute(i,r),l.sub(c),u.sub(c),f.sub(d),p.sub(d);let a=1/(f.x*p.y-p.x*f.y);isFinite(a)&&(m.copy(l).multiplyScalar(p.y).addScaledVector(u,-f.y).multiplyScalar(a),h.copy(u).multiplyScalar(f.x).addScaledVector(l,-p.x).multiplyScalar(a),o[e].add(m),o[t].add(m),o[r].add(m),s[e].add(h),s[t].add(h),s[r].add(h))}let _=this.groups;_.length===0&&(_=[{start:0,count:e.count}]);for(let t=0,n=_.length;t<n;++t){let n=_[t],r=n.start,i=n.count;for(let t=r,n=r+i;t<n;t+=3)g(e.getX(t+0),e.getX(t+1),e.getX(t+2))}let v=new K,y=new K,b=new K,x=new K;function S(e){b.fromBufferAttribute(r,e),x.copy(b);let t=o[e];v.copy(t),v.sub(b.multiplyScalar(b.dot(t))).normalize(),y.crossVectors(x,t);let n=y.dot(s[e])<0?-1:1;a.setXYZW(e,v.x,v.y,v.z,n)}for(let t=0,n=_.length;t<n;++t){let n=_[t],r=n.start,i=n.count;for(let t=r,n=r+i;t<n;t+=3)S(e.getX(t+0)),S(e.getX(t+1)),S(e.getX(t+2))}}computeVertexNormals(){let e=this.index,t=this.getAttribute(`position`);if(t!==void 0){let n=this.getAttribute(`normal`);if(n===void 0)n=new Ua(new Float32Array(t.count*3),3),this.setAttribute(`normal`,n);else for(let e=0,t=n.count;e<t;e++)n.setXYZ(e,0,0,0);let r=new K,i=new K,a=new K,o=new K,s=new K,c=new K,l=new K,u=new K;if(e)for(let d=0,f=e.count;d<f;d+=3){let f=e.getX(d+0),p=e.getX(d+1),m=e.getX(d+2);r.fromBufferAttribute(t,f),i.fromBufferAttribute(t,p),a.fromBufferAttribute(t,m),l.subVectors(a,i),u.subVectors(r,i),l.cross(u),o.fromBufferAttribute(n,f),s.fromBufferAttribute(n,p),c.fromBufferAttribute(n,m),o.add(l),s.add(l),c.add(l),n.setXYZ(f,o.x,o.y,o.z),n.setXYZ(p,s.x,s.y,s.z),n.setXYZ(m,c.x,c.y,c.z)}else for(let e=0,o=t.count;e<o;e+=3)r.fromBufferAttribute(t,e+0),i.fromBufferAttribute(t,e+1),a.fromBufferAttribute(t,e+2),l.subVectors(a,i),u.subVectors(r,i),l.cross(u),n.setXYZ(e+0,l.x,l.y,l.z),n.setXYZ(e+1,l.x,l.y,l.z),n.setXYZ(e+2,l.x,l.y,l.z);this.normalizeNormals(),n.needsUpdate=!0}}normalizeNormals(){let e=this.attributes.normal;for(let t=0,n=e.count;t<n;t++)ro.fromBufferAttribute(e,t),ro.normalize(),e.setXYZ(t,ro.x,ro.y,ro.z)}toNonIndexed(){function t(e,t){let n=e.array,r=e.itemSize,i=e.normalized,a=new n.constructor(t.length*r),o=0,s=0;for(let i=0,c=t.length;i<c;i++){o=e.isInterleavedBufferAttribute?t[i]*e.data.stride+e.offset:t[i]*r;for(let e=0;e<r;e++)a[s++]=n[o++]}return new Ua(a,r,i)}if(this.index===null)return W(`BufferGeometry.toNonIndexed(): BufferGeometry is already non-indexed.`),this;let n=new e,r=this.index.array,i=this.attributes;for(let e in i){let a=i[e],o=t(a,r);n.setAttribute(e,o)}let a=this.morphAttributes;for(let e in a){let i=[],o=a[e];for(let e=0,n=o.length;e<n;e++){let n=o[e],a=t(n,r);i.push(a)}n.morphAttributes[e]=i}n.morphTargetsRelative=this.morphTargetsRelative;let o=this.groups;for(let e=0,t=o.length;e<t;e++){let t=o[e];n.addGroup(t.start,t.count,t.materialIndex)}return n}toJSON(){let e={metadata:{version:4.7,type:`BufferGeometry`,generator:`BufferGeometry.toJSON`}};if(e.uuid=this.uuid,e.type=this.type,this.name!==``&&(e.name=this.name),Object.keys(this.userData).length>0&&(e.userData=this.userData),this.parameters!==void 0){let t=this.parameters;for(let n in t)t[n]!==void 0&&(e[n]=t[n]);return e}e.data={attributes:{}};let t=this.index;t!==null&&(e.data.index={type:t.array.constructor.name,array:Array.prototype.slice.call(t.array)});let n=this.attributes;for(let t in n){let r=n[t];e.data.attributes[t]=r.toJSON(e.data)}let r={},i=!1;for(let t in this.morphAttributes){let n=this.morphAttributes[t],a=[];for(let t=0,r=n.length;t<r;t++){let r=n[t];a.push(r.toJSON(e.data))}a.length>0&&(r[t]=a,i=!0)}i&&(e.data.morphAttributes=r,e.data.morphTargetsRelative=this.morphTargetsRelative);let a=this.groups;a.length>0&&(e.data.groups=JSON.parse(JSON.stringify(a)));let o=this.boundingSphere;return o!==null&&(e.data.boundingSphere=o.toJSON()),e}clone(){return new this.constructor().copy(this)}copy(e){this.index=null,this.attributes={},this.morphAttributes={},this.groups=[],this.boundingBox=null,this.boundingSphere=null;let t={};this.name=e.name;let n=e.index;n!==null&&this.setIndex(n.clone());let r=e.attributes;for(let e in r){let n=r[e];this.setAttribute(e,n.clone(t))}let i=e.morphAttributes;for(let e in i){let n=[],r=i[e];for(let e=0,i=r.length;e<i;e++)n.push(r[e].clone(t));this.morphAttributes[e]=n}this.morphTargetsRelative=e.morphTargetsRelative;let a=e.groups;for(let e=0,t=a.length;e<t;e++){let t=a[e];this.addGroup(t.start,t.count,t.materialIndex)}let o=e.boundingBox;o!==null&&(this.boundingBox=o.clone());let s=e.boundingSphere;return s!==null&&(this.boundingSphere=s.clone()),this.drawRange.start=e.drawRange.start,this.drawRange.count=e.drawRange.count,this.userData=e.userData,this}dispose(){this.dispatchEvent({type:`dispose`})}},ao=0,oo=class extends ci{constructor(){super(),this.isMaterial=!0,Object.defineProperty(this,"id",{value:ao++}),this.uuid=Gt(),this.name=``,this.type=`Material`,this.blending=1,this.side=0,this.vertexColors=!1,this.opacity=1,this.transparent=!1,this.alphaHash=!1,this.blendSrc=204,this.blendDst=205,this.blendEquation=100,this.blendSrcAlpha=null,this.blendDstAlpha=null,this.blendEquationAlpha=null,this.blendColor=new X(0,0,0),this.blendAlpha=0,this.depthFunc=3,this.depthTest=!0,this.depthWrite=!0,this.stencilWriteMask=255,this.stencilFunc=519,this.stencilRef=0,this.stencilFuncMask=255,this.stencilFail=ti,this.stencilZFail=ti,this.stencilZPass=ti,this.stencilWrite=!1,this.clippingPlanes=null,this.clipIntersection=!1,this.clipShadows=!1,this.shadowSide=null,this.colorWrite=!0,this.precision=null,this.polygonOffset=!1,this.polygonOffsetFactor=0,this.polygonOffsetUnits=0,this.dithering=!1,this.alphaToCoverage=!1,this.premultipliedAlpha=!1,this.forceSinglePass=!1,this.allowOverride=!0,this.visible=!0,this.toneMapped=!0,this.userData={},this.version=0,this._alphaTest=0}get alphaTest(){return this._alphaTest}set alphaTest(e){this._alphaTest>0!=e>0&&this.version++,this._alphaTest=e}onBeforeRender(){}onBeforeCompile(){}customProgramCacheKey(){return this.onBeforeCompile.toString()}setValues(e){if(e!==void 0)for(let t in e){let n=e[t];if(n===void 0){W(`Material: parameter '${t}' has value of undefined.`);continue}let r=this[t];if(r===void 0){W(`Material: '${t}' is not a property of THREE.${this.type}.`);continue}r&&r.isColor?r.set(n):r&&r.isVector3&&n&&n.isVector3?r.copy(n):this[t]=n}}toJSON(e){let t=e===void 0||typeof e==`string`;t&&(e={textures:{},images:{}});let n={metadata:{version:4.7,type:`Material`,generator:`Material.toJSON`}};n.uuid=this.uuid,n.type=this.type,this.name!==``&&(n.name=this.name),this.color&&this.color.isColor&&(n.color=this.color.getHex()),this.roughness!==void 0&&(n.roughness=this.roughness),this.metalness!==void 0&&(n.metalness=this.metalness),this.sheen!==void 0&&(n.sheen=this.sheen),this.sheenColor&&this.sheenColor.isColor&&(n.sheenColor=this.sheenColor.getHex()),this.sheenRoughness!==void 0&&(n.sheenRoughness=this.sheenRoughness),this.emissive&&this.emissive.isColor&&(n.emissive=this.emissive.getHex()),this.emissiveIntensity!==void 0&&this.emissiveIntensity!==1&&(n.emissiveIntensity=this.emissiveIntensity),this.specular&&this.specular.isColor&&(n.specular=this.specular.getHex()),this.specularIntensity!==void 0&&(n.specularIntensity=this.specularIntensity),this.specularColor&&this.specularColor.isColor&&(n.specularColor=this.specularColor.getHex()),this.shininess!==void 0&&(n.shininess=this.shininess),this.clearcoat!==void 0&&(n.clearcoat=this.clearcoat),this.clearcoatRoughness!==void 0&&(n.clearcoatRoughness=this.clearcoatRoughness),this.clearcoatMap&&this.clearcoatMap.isTexture&&(n.clearcoatMap=this.clearcoatMap.toJSON(e).uuid),this.clearcoatRoughnessMap&&this.clearcoatRoughnessMap.isTexture&&(n.clearcoatRoughnessMap=this.clearcoatRoughnessMap.toJSON(e).uuid),this.clearcoatNormalMap&&this.clearcoatNormalMap.isTexture&&(n.clearcoatNormalMap=this.clearcoatNormalMap.toJSON(e).uuid,n.clearcoatNormalScale=this.clearcoatNormalScale.toArray()),this.sheenColorMap&&this.sheenColorMap.isTexture&&(n.sheenColorMap=this.sheenColorMap.toJSON(e).uuid),this.sheenRoughnessMap&&this.sheenRoughnessMap.isTexture&&(n.sheenRoughnessMap=this.sheenRoughnessMap.toJSON(e).uuid),this.dispersion!==void 0&&(n.dispersion=this.dispersion),this.iridescence!==void 0&&(n.iridescence=this.iridescence),this.iridescenceIOR!==void 0&&(n.iridescenceIOR=this.iridescenceIOR),this.iridescenceThicknessRange!==void 0&&(n.iridescenceThicknessRange=this.iridescenceThicknessRange),this.iridescenceMap&&this.iridescenceMap.isTexture&&(n.iridescenceMap=this.iridescenceMap.toJSON(e).uuid),this.iridescenceThicknessMap&&this.iridescenceThicknessMap.isTexture&&(n.iridescenceThicknessMap=this.iridescenceThicknessMap.toJSON(e).uuid),this.anisotropy!==void 0&&(n.anisotropy=this.anisotropy),this.anisotropyRotation!==void 0&&(n.anisotropyRotation=this.anisotropyRotation),this.anisotropyMap&&this.anisotropyMap.isTexture&&(n.anisotropyMap=this.anisotropyMap.toJSON(e).uuid),this.map&&this.map.isTexture&&(n.map=this.map.toJSON(e).uuid),this.matcap&&this.matcap.isTexture&&(n.matcap=this.matcap.toJSON(e).uuid),this.alphaMap&&this.alphaMap.isTexture&&(n.alphaMap=this.alphaMap.toJSON(e).uuid),this.lightMap&&this.lightMap.isTexture&&(n.lightMap=this.lightMap.toJSON(e).uuid,n.lightMapIntensity=this.lightMapIntensity),this.aoMap&&this.aoMap.isTexture&&(n.aoMap=this.aoMap.toJSON(e).uuid,n.aoMapIntensity=this.aoMapIntensity),this.bumpMap&&this.bumpMap.isTexture&&(n.bumpMap=this.bumpMap.toJSON(e).uuid,n.bumpScale=this.bumpScale),this.normalMap&&this.normalMap.isTexture&&(n.normalMap=this.normalMap.toJSON(e).uuid,n.normalMapType=this.normalMapType,n.normalScale=this.normalScale.toArray()),this.displacementMap&&this.displacementMap.isTexture&&(n.displacementMap=this.displacementMap.toJSON(e).uuid,n.displacementScale=this.displacementScale,n.displacementBias=this.displacementBias),this.roughnessMap&&this.roughnessMap.isTexture&&(n.roughnessMap=this.roughnessMap.toJSON(e).uuid),this.metalnessMap&&this.metalnessMap.isTexture&&(n.metalnessMap=this.metalnessMap.toJSON(e).uuid),this.emissiveMap&&this.emissiveMap.isTexture&&(n.emissiveMap=this.emissiveMap.toJSON(e).uuid),this.specularMap&&this.specularMap.isTexture&&(n.specularMap=this.specularMap.toJSON(e).uuid),this.specularIntensityMap&&this.specularIntensityMap.isTexture&&(n.specularIntensityMap=this.specularIntensityMap.toJSON(e).uuid),this.specularColorMap&&this.specularColorMap.isTexture&&(n.specularColorMap=this.specularColorMap.toJSON(e).uuid),this.envMap&&this.envMap.isTexture&&(n.envMap=this.envMap.toJSON(e).uuid,this.combine!==void 0&&(n.combine=this.combine)),this.envMapRotation!==void 0&&(n.envMapRotation=this.envMapRotation.toArray()),this.envMapIntensity!==void 0&&(n.envMapIntensity=this.envMapIntensity),this.reflectivity!==void 0&&(n.reflectivity=this.reflectivity),this.refractionRatio!==void 0&&(n.refractionRatio=this.refractionRatio),this.gradientMap&&this.gradientMap.isTexture&&(n.gradientMap=this.gradientMap.toJSON(e).uuid),this.transmission!==void 0&&(n.transmission=this.transmission),this.transmissionMap&&this.transmissionMap.isTexture&&(n.transmissionMap=this.transmissionMap.toJSON(e).uuid),this.thickness!==void 0&&(n.thickness=this.thickness),this.thicknessMap&&this.thicknessMap.isTexture&&(n.thicknessMap=this.thicknessMap.toJSON(e).uuid),this.attenuationDistance!==void 0&&this.attenuationDistance!==1/0&&(n.attenuationDistance=this.attenuationDistance),this.attenuationColor!==void 0&&(n.attenuationColor=this.attenuationColor.getHex()),this.size!==void 0&&(n.size=this.size),this.shadowSide!==null&&(n.shadowSide=this.shadowSide),this.sizeAttenuation!==void 0&&(n.sizeAttenuation=this.sizeAttenuation),this.blending!==1&&(n.blending=this.blending),this.side!==0&&(n.side=this.side),this.vertexColors===!0&&(n.vertexColors=!0),this.opacity<1&&(n.opacity=this.opacity),this.transparent===!0&&(n.transparent=!0),this.blendSrc!==204&&(n.blendSrc=this.blendSrc),this.blendDst!==205&&(n.blendDst=this.blendDst),this.blendEquation!==100&&(n.blendEquation=this.blendEquation),this.blendSrcAlpha!==null&&(n.blendSrcAlpha=this.blendSrcAlpha),this.blendDstAlpha!==null&&(n.blendDstAlpha=this.blendDstAlpha),this.blendEquationAlpha!==null&&(n.blendEquationAlpha=this.blendEquationAlpha),this.blendColor&&this.blendColor.isColor&&(n.blendColor=this.blendColor.getHex()),this.blendAlpha!==0&&(n.blendAlpha=this.blendAlpha),this.depthFunc!==3&&(n.depthFunc=this.depthFunc),this.depthTest===!1&&(n.depthTest=this.depthTest),this.depthWrite===!1&&(n.depthWrite=this.depthWrite),this.colorWrite===!1&&(n.colorWrite=this.colorWrite),this.stencilWriteMask!==255&&(n.stencilWriteMask=this.stencilWriteMask),this.stencilFunc!==519&&(n.stencilFunc=this.stencilFunc),this.stencilRef!==0&&(n.stencilRef=this.stencilRef),this.stencilFuncMask!==255&&(n.stencilFuncMask=this.stencilFuncMask),this.stencilFail!==7680&&(n.stencilFail=this.stencilFail),this.stencilZFail!==7680&&(n.stencilZFail=this.stencilZFail),this.stencilZPass!==7680&&(n.stencilZPass=this.stencilZPass),this.stencilWrite===!0&&(n.stencilWrite=this.stencilWrite),this.rotation!==void 0&&this.rotation!==0&&(n.rotation=this.rotation),this.polygonOffset===!0&&(n.polygonOffset=!0),this.polygonOffsetFactor!==0&&(n.polygonOffsetFactor=this.polygonOffsetFactor),this.polygonOffsetUnits!==0&&(n.polygonOffsetUnits=this.polygonOffsetUnits),this.linewidth!==void 0&&this.linewidth!==1&&(n.linewidth=this.linewidth),this.dashSize!==void 0&&(n.dashSize=this.dashSize),this.gapSize!==void 0&&(n.gapSize=this.gapSize),this.scale!==void 0&&(n.scale=this.scale),this.dithering===!0&&(n.dithering=!0),this.alphaTest>0&&(n.alphaTest=this.alphaTest),this.alphaHash===!0&&(n.alphaHash=!0),this.alphaToCoverage===!0&&(n.alphaToCoverage=!0),this.premultipliedAlpha===!0&&(n.premultipliedAlpha=!0),this.forceSinglePass===!0&&(n.forceSinglePass=!0),this.allowOverride===!1&&(n.allowOverride=!1),this.wireframe===!0&&(n.wireframe=!0),this.wireframeLinewidth>1&&(n.wireframeLinewidth=this.wireframeLinewidth),this.wireframeLinecap!==`round`&&(n.wireframeLinecap=this.wireframeLinecap),this.wireframeLinejoin!==`round`&&(n.wireframeLinejoin=this.wireframeLinejoin),this.flatShading===!0&&(n.flatShading=!0),this.visible===!1&&(n.visible=!1),this.toneMapped===!1&&(n.toneMapped=!1),this.fog===!1&&(n.fog=!1),Object.keys(this.userData).length>0&&(n.userData=this.userData);function r(e){let t=[];for(let n in e){let r=e[n];delete r.metadata,t.push(r)}return t}if(t){let t=r(e.textures),i=r(e.images);t.length>0&&(n.textures=t),i.length>0&&(n.images=i)}return n}clone(){return new this.constructor().copy(this)}copy(e){this.name=e.name,this.blending=e.blending,this.side=e.side,this.vertexColors=e.vertexColors,this.opacity=e.opacity,this.transparent=e.transparent,this.blendSrc=e.blendSrc,this.blendDst=e.blendDst,this.blendEquation=e.blendEquation,this.blendSrcAlpha=e.blendSrcAlpha,this.blendDstAlpha=e.blendDstAlpha,this.blendEquationAlpha=e.blendEquationAlpha,this.blendColor.copy(e.blendColor),this.blendAlpha=e.blendAlpha,this.depthFunc=e.depthFunc,this.depthTest=e.depthTest,this.depthWrite=e.depthWrite,this.stencilWriteMask=e.stencilWriteMask,this.stencilFunc=e.stencilFunc,this.stencilRef=e.stencilRef,this.stencilFuncMask=e.stencilFuncMask,this.stencilFail=e.stencilFail,this.stencilZFail=e.stencilZFail,this.stencilZPass=e.stencilZPass,this.stencilWrite=e.stencilWrite;let t=e.clippingPlanes,n=null;if(t!==null){let e=t.length;n=Array(e);for(let r=0;r!==e;++r)n[r]=t[r].clone()}return this.clippingPlanes=n,this.clipIntersection=e.clipIntersection,this.clipShadows=e.clipShadows,this.shadowSide=e.shadowSide,this.colorWrite=e.colorWrite,this.precision=e.precision,this.polygonOffset=e.polygonOffset,this.polygonOffsetFactor=e.polygonOffsetFactor,this.polygonOffsetUnits=e.polygonOffsetUnits,this.dithering=e.dithering,this.alphaTest=e.alphaTest,this.alphaHash=e.alphaHash,this.alphaToCoverage=e.alphaToCoverage,this.premultipliedAlpha=e.premultipliedAlpha,this.forceSinglePass=e.forceSinglePass,this.allowOverride=e.allowOverride,this.visible=e.visible,this.toneMapped=e.toneMapped,this.userData=JSON.parse(JSON.stringify(e.userData)),this}dispose(){this.dispatchEvent({type:`dispose`})}set needsUpdate(e){e===!0&&this.version++}},so=new K,co=new K,lo=new K,uo=new K,fo=new K,po=new K,mo=new K,ho=class{constructor(e=new K,t=new K(0,0,-1)){this.origin=e,this.direction=t}set(e,t){return this.origin.copy(e),this.direction.copy(t),this}copy(e){return this.origin.copy(e.origin),this.direction.copy(e.direction),this}at(e,t){return t.copy(this.origin).addScaledVector(this.direction,e)}lookAt(e){return this.direction.copy(e).sub(this.origin).normalize(),this}recast(e){return this.origin.copy(this.at(e,so)),this}closestPointToPoint(e,t){t.subVectors(e,this.origin);let n=t.dot(this.direction);return n<0?t.copy(this.origin):t.copy(this.origin).addScaledVector(this.direction,n)}distanceToPoint(e){return Math.sqrt(this.distanceSqToPoint(e))}distanceSqToPoint(e){let t=so.subVectors(e,this.origin).dot(this.direction);return t<0?this.origin.distanceToSquared(e):(so.copy(this.origin).addScaledVector(this.direction,t),so.distanceToSquared(e))}distanceSqToSegment(e,t,n,r){co.copy(e).add(t).multiplyScalar(.5),lo.copy(t).sub(e).normalize(),uo.copy(this.origin).sub(co);let i=e.distanceTo(t)*.5,a=-this.direction.dot(lo),o=uo.dot(this.direction),s=-uo.dot(lo),c=uo.lengthSq(),l=Math.abs(1-a*a),u,d,f,p;if(l>0)if(u=a*s-o,d=a*o-s,p=i*l,u>=0)if(d>=-p)if(d<=p){let e=1/l;u*=e,d*=e,f=u*(u+a*d+2*o)+d*(a*u+d+2*s)+c}else d=i,u=Math.max(0,-(a*d+o)),f=-u*u+d*(d+2*s)+c;else d=-i,u=Math.max(0,-(a*d+o)),f=-u*u+d*(d+2*s)+c;else d<=-p?(u=Math.max(0,-(-a*i+o)),d=u>0?-i:Math.min(Math.max(-i,-s),i),f=-u*u+d*(d+2*s)+c):d<=p?(u=0,d=Math.min(Math.max(-i,-s),i),f=d*(d+2*s)+c):(u=Math.max(0,-(a*i+o)),d=u>0?i:Math.min(Math.max(-i,-s),i),f=-u*u+d*(d+2*s)+c);else d=a>0?-i:i,u=Math.max(0,-(a*d+o)),f=-u*u+d*(d+2*s)+c;return n&&n.copy(this.origin).addScaledVector(this.direction,u),r&&r.copy(co).addScaledVector(lo,d),f}intersectSphere(e,t){so.subVectors(e.center,this.origin);let n=so.dot(this.direction),r=so.dot(so)-n*n,i=e.radius*e.radius;if(r>i)return null;let a=Math.sqrt(i-r),o=n-a,s=n+a;return s<0?null:o<0?this.at(s,t):this.at(o,t)}intersectsSphere(e){return e.radius<0?!1:this.distanceSqToPoint(e.center)<=e.radius*e.radius}distanceToPlane(e){let t=e.normal.dot(this.direction);if(t===0)return e.distanceToPoint(this.origin)===0?0:null;let n=-(this.origin.dot(e.normal)+e.constant)/t;return n>=0?n:null}intersectPlane(e,t){let n=this.distanceToPlane(e);return n===null?null:this.at(n,t)}intersectsPlane(e){let t=e.distanceToPoint(this.origin);return t===0||e.normal.dot(this.direction)*t<0}intersectBox(e,t){let n,r,i,a,o,s,c=1/this.direction.x,l=1/this.direction.y,u=1/this.direction.z,d=this.origin;return c>=0?(n=(e.min.x-d.x)*c,r=(e.max.x-d.x)*c):(n=(e.max.x-d.x)*c,r=(e.min.x-d.x)*c),l>=0?(i=(e.min.y-d.y)*l,a=(e.max.y-d.y)*l):(i=(e.max.y-d.y)*l,a=(e.min.y-d.y)*l),n>a||i>r||((i>n||isNaN(n))&&(n=i),(a<r||isNaN(r))&&(r=a),u>=0?(o=(e.min.z-d.z)*u,s=(e.max.z-d.z)*u):(o=(e.max.z-d.z)*u,s=(e.min.z-d.z)*u),n>s||o>r)||((o>n||n!==n)&&(n=o),(s<r||r!==r)&&(r=s),r<0)?null:this.at(n>=0?n:r,t)}intersectsBox(e){return this.intersectBox(e,so)!==null}intersectTriangle(e,t,n,r,i){fo.subVectors(t,e),po.subVectors(n,e),mo.crossVectors(fo,po);let a=this.direction.dot(mo),o;if(a>0){if(r)return null;o=1}else if(a<0)o=-1,a=-a;else return null;uo.subVectors(this.origin,e);let s=o*this.direction.dot(po.crossVectors(uo,po));if(s<0)return null;let c=o*this.direction.dot(fo.cross(uo));if(c<0||s+c>a)return null;let l=-o*uo.dot(mo);return l<0?null:this.at(l/a,i)}applyMatrix4(e){return this.origin.applyMatrix4(e),this.direction.transformDirection(e),this}equals(e){return e.origin.equals(this.origin)&&e.direction.equals(this.direction)}clone(){return new this.constructor().copy(this)}},go=class extends oo{constructor(e){super(),this.isMeshBasicMaterial=!0,this.type=`MeshBasicMaterial`,this.color=new X(16777215),this.map=null,this.lightMap=null,this.lightMapIntensity=1,this.aoMap=null,this.aoMapIntensity=1,this.specularMap=null,this.alphaMap=null,this.envMap=null,this.envMapRotation=new Bi,this.combine=0,this.reflectivity=1,this.refractionRatio=.98,this.wireframe=!1,this.wireframeLinewidth=1,this.wireframeLinecap=`round`,this.wireframeLinejoin=`round`,this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.color.copy(e.color),this.map=e.map,this.lightMap=e.lightMap,this.lightMapIntensity=e.lightMapIntensity,this.aoMap=e.aoMap,this.aoMapIntensity=e.aoMapIntensity,this.specularMap=e.specularMap,this.alphaMap=e.alphaMap,this.envMap=e.envMap,this.envMapRotation.copy(e.envMapRotation),this.combine=e.combine,this.reflectivity=e.reflectivity,this.refractionRatio=e.refractionRatio,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.wireframeLinecap=e.wireframeLinecap,this.wireframeLinejoin=e.wireframeLinejoin,this.fog=e.fog,this}},_o=new Y,vo=new ho,yo=new Xa,bo=new K,xo=new K,So=new K,Co=new K,wo=new K,To=new K,Eo=new K,Do=new K,Oo=class extends ra{constructor(e=new io,t=new go){super(),this.isMesh=!0,this.type=`Mesh`,this.geometry=e,this.material=t,this.morphTargetDictionary=void 0,this.morphTargetInfluences=void 0,this.count=1,this.updateMorphTargets()}copy(e,t){return super.copy(e,t),e.morphTargetInfluences!==void 0&&(this.morphTargetInfluences=e.morphTargetInfluences.slice()),e.morphTargetDictionary!==void 0&&(this.morphTargetDictionary=Object.assign({},e.morphTargetDictionary)),this.material=Array.isArray(e.material)?e.material.slice():e.material,this.geometry=e.geometry,this}updateMorphTargets(){let e=this.geometry.morphAttributes,t=Object.keys(e);if(t.length>0){let n=e[t[0]];if(n!==void 0){this.morphTargetInfluences=[],this.morphTargetDictionary={};for(let e=0,t=n.length;e<t;e++){let t=n[e].name||String(e);this.morphTargetInfluences.push(0),this.morphTargetDictionary[t]=e}}}}getVertexPosition(e,t){let n=this.geometry,r=n.attributes.position,i=n.morphAttributes.position,a=n.morphTargetsRelative;t.fromBufferAttribute(r,e);let o=this.morphTargetInfluences;if(i&&o){To.set(0,0,0);for(let n=0,r=i.length;n<r;n++){let r=o[n],s=i[n];r!==0&&(wo.fromBufferAttribute(s,e),a?To.addScaledVector(wo,r):To.addScaledVector(wo.sub(t),r))}t.add(To)}return t}raycast(e,t){let n=this.geometry,r=this.material,i=this.matrixWorld;r!==void 0&&(n.boundingSphere===null&&n.computeBoundingSphere(),yo.copy(n.boundingSphere),yo.applyMatrix4(i),vo.copy(e.ray).recast(e.near),!(yo.containsPoint(vo.origin)===!1&&(vo.intersectSphere(yo,bo)===null||vo.origin.distanceToSquared(bo)>(e.far-e.near)**2))&&(_o.copy(i).invert(),vo.copy(e.ray).applyMatrix4(_o),!(n.boundingBox!==null&&vo.intersectsBox(n.boundingBox)===!1)&&this._computeIntersections(e,t,vo)))}_computeIntersections(e,t,n){let r,i=this.geometry,a=this.material,o=i.index,s=i.attributes.position,c=i.attributes.uv,l=i.attributes.uv1,u=i.attributes.normal,d=i.groups,f=i.drawRange;if(o!==null)if(Array.isArray(a))for(let i=0,s=d.length;i<s;i++){let s=d[i],p=a[s.materialIndex],m=Math.max(s.start,f.start),h=Math.min(o.count,Math.min(s.start+s.count,f.start+f.count));for(let i=m,a=h;i<a;i+=3){let a=o.getX(i),d=o.getX(i+1),f=o.getX(i+2);r=an(this,p,e,n,c,l,u,a,d,f),r&&(r.faceIndex=Math.floor(i/3),r.face.materialIndex=s.materialIndex,t.push(r))}}else{let i=Math.max(0,f.start),s=Math.min(o.count,f.start+f.count);for(let d=i,f=s;d<f;d+=3){let i=o.getX(d),s=o.getX(d+1),f=o.getX(d+2);r=an(this,a,e,n,c,l,u,i,s,f),r&&(r.faceIndex=Math.floor(d/3),t.push(r))}}else if(s!==void 0)if(Array.isArray(a))for(let i=0,o=d.length;i<o;i++){let o=d[i],p=a[o.materialIndex],m=Math.max(o.start,f.start),h=Math.min(s.count,Math.min(o.start+o.count,f.start+f.count));for(let i=m,a=h;i<a;i+=3){let a=i,s=i+1,d=i+2;r=an(this,p,e,n,c,l,u,a,s,d),r&&(r.faceIndex=Math.floor(i/3),r.face.materialIndex=o.materialIndex,t.push(r))}}else{let i=Math.max(0,f.start),o=Math.min(s.count,f.start+f.count);for(let s=i,d=o;s<d;s+=3){let i=s,o=s+1,d=s+2;r=an(this,a,e,n,c,l,u,i,o,d),r&&(r.faceIndex=Math.floor(s/3),t.push(r))}}}},ko=new Ei,Ao=new Ei,jo=new Ei,Mo=new Ei,No=new Y,Po=new K,Fo=new Xa,Io=new Y,Lo=new ho,Ro=class extends Oo{constructor(e,t){super(e,t),this.isSkinnedMesh=!0,this.type=`SkinnedMesh`,this.bindMode=Cn,this.bindMatrix=new Y,this.bindMatrixInverse=new Y,this.boundingBox=null,this.boundingSphere=null}computeBoundingBox(){let e=this.geometry;this.boundingBox===null&&(this.boundingBox=new Ea),this.boundingBox.makeEmpty();let t=e.getAttribute(`position`);for(let e=0;e<t.count;e++)this.getVertexPosition(e,Po),this.boundingBox.expandByPoint(Po)}computeBoundingSphere(){let e=this.geometry;this.boundingSphere===null&&(this.boundingSphere=new Xa),this.boundingSphere.makeEmpty();let t=e.getAttribute(`position`);for(let e=0;e<t.count;e++)this.getVertexPosition(e,Po),this.boundingSphere.expandByPoint(Po)}copy(e,t){return super.copy(e,t),this.bindMode=e.bindMode,this.bindMatrix.copy(e.bindMatrix),this.bindMatrixInverse.copy(e.bindMatrixInverse),this.skeleton=e.skeleton,e.boundingBox!==null&&(this.boundingBox=e.boundingBox.clone()),e.boundingSphere!==null&&(this.boundingSphere=e.boundingSphere.clone()),this}raycast(e,t){let n=this.material,r=this.matrixWorld;n!==void 0&&(this.boundingSphere===null&&this.computeBoundingSphere(),Fo.copy(this.boundingSphere),Fo.applyMatrix4(r),e.ray.intersectsSphere(Fo)!==!1&&(Io.copy(r).invert(),Lo.copy(e.ray).applyMatrix4(Io),!(this.boundingBox!==null&&Lo.intersectsBox(this.boundingBox)===!1)&&this._computeIntersections(e,t,Lo)))}getVertexPosition(e,t){return super.getVertexPosition(e,t),this.applyBoneTransform(e,t),t}bind(e,t){this.skeleton=e,t===void 0&&(this.updateMatrixWorld(!0),this.skeleton.calculateInverses(),t=this.matrixWorld),this.bindMatrix.copy(t),this.bindMatrixInverse.copy(t).invert()}pose(){this.skeleton.pose()}normalizeSkinWeights(){let e=new Ei,t=this.geometry.attributes.skinWeight;for(let n=0,r=t.count;n<r;n++){e.fromBufferAttribute(t,n);let r=1/e.manhattanLength();r===1/0?e.set(1,0,0,0):e.multiplyScalar(r),t.setXYZW(n,e.x,e.y,e.z,e.w)}}updateMatrixWorld(e){super.updateMatrixWorld(e),this.bindMode===`attached`?this.bindMatrixInverse.copy(this.matrixWorld).invert():this.bindMode===`detached`?this.bindMatrixInverse.copy(this.bindMatrix).invert():W(`SkinnedMesh: Unrecognized bindMode: `+this.bindMode)}applyBoneTransform(e,t){let n=this.skeleton,r=this.geometry;Ao.fromBufferAttribute(r.attributes.skinIndex,e),jo.fromBufferAttribute(r.attributes.skinWeight,e),t.isVector4?(ko.copy(t),t.set(0,0,0,0)):(ko.set(...t,1),t.set(0,0,0)),ko.applyMatrix4(this.bindMatrix);for(let e=0;e<4;e++){let r=jo.getComponent(e);if(r!==0){let i=Ao.getComponent(e);No.multiplyMatrices(n.bones[i].matrixWorld,n.boneInverses[i]),t.addScaledVector(Mo.copy(ko).applyMatrix4(No),r)}}return t.isVector4&&(t.w=ko.w),t.applyMatrix4(this.bindMatrixInverse)}},zo=class extends ra{constructor(){super(),this.isBone=!0,this.type=`Bone`}},Bo=class extends Ti{constructor(e=null,t=1,n=1,r,i,a,o,s,c=Dn,l=Dn,u,d){super(null,a,o,s,c,l,r,i,u,d),this.isDataTexture=!0,this.image={data:e,width:t,height:n},this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}},Vo=class extends Ua{constructor(e,t,n,r=1){super(e,t,n),this.isInstancedBufferAttribute=!0,this.meshPerAttribute=r}copy(e){return super.copy(e),this.meshPerAttribute=e.meshPerAttribute,this}toJSON(){let e=super.toJSON();return e.meshPerAttribute=this.meshPerAttribute,e.isInstancedBufferAttribute=!0,e}},Ho=new Y,Uo=new Y,Wo=[],Go=new Ea,Ko=new Y,qo=new Oo,Jo=new Xa,Yo=class extends Oo{constructor(e,t,n){super(e,t),this.isInstancedMesh=!0,this.instanceMatrix=new Vo(new Float32Array(n*16),16),this.previousInstanceMatrix=null,this.instanceColor=null,this.morphTexture=null,this.count=n,this.boundingBox=null,this.boundingSphere=null;for(let e=0;e<n;e++)this.setMatrixAt(e,Ko)}computeBoundingBox(){let e=this.geometry,t=this.count;this.boundingBox===null&&(this.boundingBox=new Ea),e.boundingBox===null&&e.computeBoundingBox(),this.boundingBox.makeEmpty();for(let n=0;n<t;n++)this.getMatrixAt(n,Ho),Go.copy(e.boundingBox).applyMatrix4(Ho),this.boundingBox.union(Go)}computeBoundingSphere(){let e=this.geometry,t=this.count;this.boundingSphere===null&&(this.boundingSphere=new Xa),e.boundingSphere===null&&e.computeBoundingSphere(),this.boundingSphere.makeEmpty();for(let n=0;n<t;n++)this.getMatrixAt(n,Ho),Jo.copy(e.boundingSphere).applyMatrix4(Ho),this.boundingSphere.union(Jo)}copy(e,t){return super.copy(e,t),this.instanceMatrix.copy(e.instanceMatrix),e.previousInstanceMatrix!==null&&(this.previousInstanceMatrix=e.previousInstanceMatrix.clone()),e.morphTexture!==null&&(this.morphTexture=e.morphTexture.clone()),e.instanceColor!==null&&(this.instanceColor=e.instanceColor.clone()),this.count=e.count,e.boundingBox!==null&&(this.boundingBox=e.boundingBox.clone()),e.boundingSphere!==null&&(this.boundingSphere=e.boundingSphere.clone()),this}getColorAt(e,t){return this.instanceColor===null?t.setRGB(1,1,1):t.fromArray(this.instanceColor.array,e*3)}getMatrixAt(e,t){return t.fromArray(this.instanceMatrix.array,e*16)}getMorphAt(e,t){let n=t.morphTargetInfluences,r=this.morphTexture.source.data.data,i=e*(n.length+1)+1;for(let e=0;e<n.length;e++)n[e]=r[i+e]}raycast(e,t){let n=this.matrixWorld,r=this.count;if(qo.geometry=this.geometry,qo.material=this.material,qo.material!==void 0&&(this.boundingSphere===null&&this.computeBoundingSphere(),Jo.copy(this.boundingSphere),Jo.applyMatrix4(n),e.ray.intersectsSphere(Jo)!==!1))for(let i=0;i<r;i++){this.getMatrixAt(i,Ho),Uo.multiplyMatrices(n,Ho),qo.matrixWorld=Uo,qo.raycast(e,Wo);for(let e=0,n=Wo.length;e<n;e++){let n=Wo[e];n.instanceId=i,n.object=this,t.push(n)}Wo.length=0}}setColorAt(e,t){return this.instanceColor===null&&(this.instanceColor=new Vo(new Float32Array(this.instanceMatrix.count*3).fill(1),3)),t.toArray(this.instanceColor.array,e*3),this}setMatrixAt(e,t){return t.toArray(this.instanceMatrix.array,e*16),this}setMorphAt(e,t){let n=t.morphTargetInfluences,r=n.length+1;this.morphTexture===null&&(this.morphTexture=new Bo(new Float32Array(r*this.count),r,this.count,Zn,zn));let i=this.morphTexture.source.data.data,a=0;for(let e=0;e<n.length;e++)a+=n[e];let o=this.geometry.morphTargetsRelative?1:1-a,s=r*e;return i[s]=o,i.set(n,s+1),this}updateMorphTargets(){}dispose(){this.dispatchEvent({type:`dispose`}),this.morphTexture!==null&&(this.morphTexture.dispose(),this.morphTexture=null)}},Xo=new K,Zo=new K,Qo=new q,$o=class{constructor(e=new K(1,0,0),t=0){this.isPlane=!0,this.normal=e,this.constant=t}set(e,t){return this.normal.copy(e),this.constant=t,this}setComponents(e,t,n,r){return this.normal.set(e,t,n),this.constant=r,this}setFromNormalAndCoplanarPoint(e,t){return this.normal.copy(e),this.constant=-t.dot(this.normal),this}setFromCoplanarPoints(e,t,n){let r=Xo.subVectors(n,t).cross(Zo.subVectors(e,t)).normalize();return this.setFromNormalAndCoplanarPoint(r,e),this}copy(e){return this.normal.copy(e.normal),this.constant=e.constant,this}normalize(){let e=1/this.normal.length();return this.normal.multiplyScalar(e),this.constant*=e,this}negate(){return this.constant*=-1,this.normal.negate(),this}distanceToPoint(e){return this.normal.dot(e)+this.constant}distanceToSphere(e){return this.distanceToPoint(e.center)-e.radius}projectPoint(e,t){return t.copy(e).addScaledVector(this.normal,-this.distanceToPoint(e))}intersectLine(e,t,n=!0){let r=e.delta(Xo),i=this.normal.dot(r);if(i===0)return this.distanceToPoint(e.start)===0?t.copy(e.start):null;let a=-(e.start.dot(this.normal)+this.constant)/i;return n===!0&&(a<0||a>1)?null:t.copy(e.start).addScaledVector(r,a)}intersectsLine(e){let t=this.distanceToPoint(e.start),n=this.distanceToPoint(e.end);return t<0&&n>0||n<0&&t>0}intersectsBox(e){return e.intersectsPlane(this)}intersectsSphere(e){return e.intersectsPlane(this)}coplanarPoint(e){return e.copy(this.normal).multiplyScalar(-this.constant)}applyMatrix4(e,t){let n=t||Qo.getNormalMatrix(e),r=this.coplanarPoint(Xo).applyMatrix4(e),i=this.normal.applyMatrix3(n).normalize();return this.constant=-r.dot(i),this}translate(e){return this.constant-=e.dot(this.normal),this}equals(e){return e.normal.equals(this.normal)&&e.constant===this.constant}clone(){return new this.constructor().copy(this)}},es=new Xa,ts=new fi(.5,.5),ns=new K,rs=class{constructor(e=new $o,t=new $o,n=new $o,r=new $o,i=new $o,a=new $o){this.planes=[e,t,n,r,i,a]}set(e,t,n,r,i,a){let o=this.planes;return o[0].copy(e),o[1].copy(t),o[2].copy(n),o[3].copy(r),o[4].copy(i),o[5].copy(a),this}copy(e){let t=this.planes;for(let n=0;n<6;n++)t[n].copy(e.planes[n]);return this}setFromProjectionMatrix(e,t=ii,n=!1){let r=this.planes,i=e.elements,a=i[0],o=i[1],s=i[2],c=i[3],l=i[4],u=i[5],d=i[6],f=i[7],p=i[8],m=i[9],h=i[10],g=i[11],_=i[12],v=i[13],y=i[14],b=i[15];if(r[0].setComponents(c-a,f-l,g-p,b-_).normalize(),r[1].setComponents(c+a,f+l,g+p,b+_).normalize(),r[2].setComponents(c+o,f+u,g+m,b+v).normalize(),r[3].setComponents(c-o,f-u,g-m,b-v).normalize(),n)r[4].setComponents(s,d,h,y).normalize(),r[5].setComponents(c-s,f-d,g-h,b-y).normalize();else if(r[4].setComponents(c-s,f-d,g-h,b-y).normalize(),t===2e3)r[5].setComponents(c+s,f+d,g+h,b+y).normalize();else if(t===2001)r[5].setComponents(s,d,h,y).normalize();else throw Error(`THREE.Frustum.setFromProjectionMatrix(): Invalid coordinate system: `+t);return this}intersectsObject(e){if(e.boundingSphere!==void 0)e.boundingSphere===null&&e.computeBoundingSphere(),es.copy(e.boundingSphere).applyMatrix4(e.matrixWorld);else{let t=e.geometry;t.boundingSphere===null&&t.computeBoundingSphere(),es.copy(t.boundingSphere).applyMatrix4(e.matrixWorld)}return this.intersectsSphere(es)}intersectsSprite(e){es.center.set(0,0,0);let t=ts.distanceTo(e.center);return es.radius=.7071067811865476+t,es.applyMatrix4(e.matrixWorld),this.intersectsSphere(es)}intersectsSphere(e){let t=this.planes,n=e.center,r=-e.radius;for(let e=0;e<6;e++)if(t[e].distanceToPoint(n)<r)return!1;return!0}intersectsBox(e){let t=this.planes;for(let n=0;n<6;n++){let r=t[n];if(ns.x=r.normal.x>0?e.max.x:e.min.x,ns.y=r.normal.y>0?e.max.y:e.min.y,ns.z=r.normal.z>0?e.max.z:e.min.z,r.distanceToPoint(ns)<0)return!1}return!0}containsPoint(e){let t=this.planes;for(let n=0;n<6;n++)if(t[n].distanceToPoint(e)<0)return!1;return!0}clone(){return new this.constructor().copy(this)}},is=class extends oo{constructor(e){super(),this.isLineBasicMaterial=!0,this.type=`LineBasicMaterial`,this.color=new X(16777215),this.map=null,this.linewidth=1,this.linecap=`round`,this.linejoin=`round`,this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.color.copy(e.color),this.map=e.map,this.linewidth=e.linewidth,this.linecap=e.linecap,this.linejoin=e.linejoin,this.fog=e.fog,this}},as=new K,os=new K,ss=new Y,cs=new ho,ls=new Xa,us=new K,ds=new K,fs=class extends ra{constructor(e=new io,t=new is){super(),this.isLine=!0,this.type=`Line`,this.geometry=e,this.material=t,this.morphTargetDictionary=void 0,this.morphTargetInfluences=void 0,this.updateMorphTargets()}copy(e,t){return super.copy(e,t),this.material=Array.isArray(e.material)?e.material.slice():e.material,this.geometry=e.geometry,this}computeLineDistances(){let e=this.geometry;if(e.index===null){let t=e.attributes.position,n=[0];for(let e=1,r=t.count;e<r;e++)as.fromBufferAttribute(t,e-1),os.fromBufferAttribute(t,e),n[e]=n[e-1],n[e]+=as.distanceTo(os);e.setAttribute(`lineDistance`,new Ka(n,1))}else W(`Line.computeLineDistances(): Computation only possible with non-indexed BufferGeometry.`);return this}raycast(e,t){let n=this.geometry,r=this.matrixWorld,i=e.params.Line.threshold,a=n.drawRange;if(n.boundingSphere===null&&n.computeBoundingSphere(),ls.copy(n.boundingSphere),ls.applyMatrix4(r),ls.radius+=i,e.ray.intersectsSphere(ls)===!1)return;ss.copy(r).invert(),cs.copy(e.ray).applyMatrix4(ss);let o=i/((this.scale.x+this.scale.y+this.scale.z)/3),s=o*o,c=this.isLineSegments?2:1,l=n.index,u=n.attributes.position;if(l!==null){let n=Math.max(0,a.start),r=Math.min(l.count,a.start+a.count);for(let i=n,a=r-1;i<a;i+=c){let n=l.getX(i),r=l.getX(i+1),a=on(this,e,cs,s,n,r,i);a&&t.push(a)}if(this.isLineLoop){let i=l.getX(r-1),a=l.getX(n),o=on(this,e,cs,s,i,a,r-1);o&&t.push(o)}}else{let n=Math.max(0,a.start),r=Math.min(u.count,a.start+a.count);for(let i=n,a=r-1;i<a;i+=c){let n=on(this,e,cs,s,i,i+1,i);n&&t.push(n)}if(this.isLineLoop){let i=on(this,e,cs,s,r-1,n,r-1);i&&t.push(i)}}}updateMorphTargets(){let e=this.geometry.morphAttributes,t=Object.keys(e);if(t.length>0){let n=e[t[0]];if(n!==void 0){this.morphTargetInfluences=[],this.morphTargetDictionary={};for(let e=0,t=n.length;e<t;e++){let t=n[e].name||String(e);this.morphTargetInfluences.push(0),this.morphTargetDictionary[t]=e}}}}},ps=new K,ms=new K,hs=class extends fs{constructor(e,t){super(e,t),this.isLineSegments=!0,this.type=`LineSegments`}computeLineDistances(){let e=this.geometry;if(e.index===null){let t=e.attributes.position,n=[];for(let e=0,r=t.count;e<r;e+=2)ps.fromBufferAttribute(t,e),ms.fromBufferAttribute(t,e+1),n[e]=e===0?0:n[e-1],n[e+1]=n[e]+ps.distanceTo(ms);e.setAttribute(`lineDistance`,new Ka(n,1))}else W(`LineSegments.computeLineDistances(): Computation only possible with non-indexed BufferGeometry.`);return this}},gs=class extends oo{constructor(e){super(),this.isPointsMaterial=!0,this.type=`PointsMaterial`,this.color=new X(16777215),this.map=null,this.alphaMap=null,this.size=1,this.sizeAttenuation=!0,this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.color.copy(e.color),this.map=e.map,this.alphaMap=e.alphaMap,this.size=e.size,this.sizeAttenuation=e.sizeAttenuation,this.fog=e.fog,this}},_s=new Y,vs=new ho,ys=new Xa,bs=new K,xs=class extends ra{constructor(e=new io,t=new gs){super(),this.isPoints=!0,this.type=`Points`,this.geometry=e,this.material=t,this.morphTargetDictionary=void 0,this.morphTargetInfluences=void 0,this.updateMorphTargets()}copy(e,t){return super.copy(e,t),this.material=Array.isArray(e.material)?e.material.slice():e.material,this.geometry=e.geometry,this}raycast(e,t){let n=this.geometry,r=this.matrixWorld,i=e.params.Points.threshold,a=n.drawRange;if(n.boundingSphere===null&&n.computeBoundingSphere(),ys.copy(n.boundingSphere),ys.applyMatrix4(r),ys.radius+=i,e.ray.intersectsSphere(ys)===!1)return;_s.copy(r).invert(),vs.copy(e.ray).applyMatrix4(_s);let o=i/((this.scale.x+this.scale.y+this.scale.z)/3),s=o*o,c=n.index,l=n.attributes.position;if(c!==null){let n=Math.max(0,a.start),i=Math.min(c.count,a.start+a.count);for(let a=n,o=i;a<o;a++){let n=c.getX(a);bs.fromBufferAttribute(l,n),sn(bs,n,s,r,e,t,this)}}else{let n=Math.max(0,a.start),i=Math.min(l.count,a.start+a.count);for(let a=n,o=i;a<o;a++)bs.fromBufferAttribute(l,a),sn(bs,a,s,r,e,t,this)}}updateMorphTargets(){let e=this.geometry.morphAttributes,t=Object.keys(e);if(t.length>0){let n=e[t[0]];if(n!==void 0){this.morphTargetInfluences=[],this.morphTargetDictionary={};for(let e=0,t=n.length;e<t;e++){let t=n[e].name||String(e);this.morphTargetInfluences.push(0),this.morphTargetDictionary[t]=e}}}}},Ss=class extends Ti{constructor(e=[],t=301,n,r,i,a,o,s,c,l){super(e,t,n,r,i,a,o,s,c,l),this.isCubeTexture=!0,this.flipY=!1}get images(){return this.image}set images(e){this.image=e}},Cs=class extends Ti{constructor(e,t,n=Rn,r,i,a,o=Dn,s=Dn,c,l=Yn,u=1){if(l!==1026&&l!==1027)throw Error(`DepthTexture format must be either THREE.DepthFormat or THREE.DepthStencilFormat`);super({width:e,height:t,depth:u},r,i,a,o,s,l,n,c),this.isDepthTexture=!0,this.flipY=!1,this.generateMipmaps=!1,this.compareFunction=null}copy(e){return super.copy(e),this.source=new Si(Object.assign({},e.image)),this.compareFunction=e.compareFunction,this}toJSON(e){let t=super.toJSON(e);return this.compareFunction!==null&&(t.compareFunction=this.compareFunction),t}},ws=class extends Cs{constructor(e,t=Rn,n=301,r,i,a=Dn,o=Dn,s,c=Yn){let l={width:e,height:e,depth:1},u=[l,l,l,l,l,l];super(e,e,t,n,r,i,a,o,s,c),this.image=u,this.isCubeDepthTexture=!0,this.isCubeTexture=!0}get images(){return this.image}set images(e){this.image=e}},Ts=class extends Ti{constructor(e=null){super(),this.sourceTexture=e,this.isExternalTexture=!0}copy(e){return super.copy(e),this.sourceTexture=e.sourceTexture,this}},Es=class e extends io{constructor(e=1,t=1,n=1,r=1,i=1,a=1){super(),this.type=`BoxGeometry`,this.parameters={width:e,height:t,depth:n,widthSegments:r,heightSegments:i,depthSegments:a};let o=this;r=Math.floor(r),i=Math.floor(i),a=Math.floor(a);let s=[],c=[],l=[],u=[],d=0,f=0;p(`z`,`y`,`x`,-1,-1,n,t,e,a,i,0),p(`z`,`y`,`x`,1,-1,n,t,-e,a,i,1),p(`x`,`z`,`y`,1,1,e,n,t,r,a,2),p(`x`,`z`,`y`,1,-1,e,n,-t,r,a,3),p(`x`,`y`,`z`,1,-1,e,t,n,r,i,4),p(`x`,`y`,`z`,-1,-1,e,t,-n,r,i,5),this.setIndex(s),this.setAttribute(`position`,new Ka(c,3)),this.setAttribute(`normal`,new Ka(l,3)),this.setAttribute(`uv`,new Ka(u,2));function p(e,t,n,r,i,a,p,m,h,g,_){let v=a/h,y=p/g,b=a/2,x=p/2,S=m/2,C=h+1,w=g+1,T=0,E=0,D=new K;for(let a=0;a<w;a++){let o=a*y-x;for(let s=0;s<C;s++)D[e]=(s*v-b)*r,D[t]=o*i,D[n]=S,c.push(D.x,D.y,D.z),D[e]=0,D[t]=0,D[n]=m>0?1:-1,l.push(D.x,D.y,D.z),u.push(s/h),u.push(1-a/g),T+=1}for(let e=0;e<g;e++)for(let t=0;t<h;t++){let n=d+t+C*e,r=d+t+C*(e+1),i=d+(t+1)+C*(e+1),a=d+(t+1)+C*e;s.push(n,r,a),s.push(r,i,a),E+=6}o.addGroup(f,E,_),f+=E,d+=T}}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(t){return new e(t.width,t.height,t.depth,t.widthSegments,t.heightSegments,t.depthSegments)}},Ds=class e extends io{constructor(e=1,t=1,n=1,r=1){super(),this.type=`PlaneGeometry`,this.parameters={width:e,height:t,widthSegments:n,heightSegments:r};let i=e/2,a=t/2,o=Math.floor(n),s=Math.floor(r),c=o+1,l=s+1,u=e/o,d=t/s,f=[],p=[],m=[],h=[];for(let e=0;e<l;e++){let t=e*d-a;for(let n=0;n<c;n++){let r=n*u-i;p.push(r,-t,0),m.push(0,0,1),h.push(n/o),h.push(1-e/s)}}for(let e=0;e<s;e++)for(let t=0;t<o;t++){let n=t+c*e,r=t+c*(e+1),i=t+1+c*(e+1),a=t+1+c*e;f.push(n,r,a),f.push(r,i,a)}this.setIndex(f),this.setAttribute(`position`,new Ka(p,3)),this.setAttribute(`normal`,new Ka(m,3)),this.setAttribute(`uv`,new Ka(h,2))}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(t){return new e(t.width,t.height,t.widthSegments,t.heightSegments)}},Os=class e extends io{constructor(e=1,t=32,n=16,r=0,i=Math.PI*2,a=0,o=Math.PI){super(),this.type=`SphereGeometry`,this.parameters={radius:e,widthSegments:t,heightSegments:n,phiStart:r,phiLength:i,thetaStart:a,thetaLength:o},t=Math.max(3,Math.floor(t)),n=Math.max(2,Math.floor(n));let s=Math.min(a+o,Math.PI),c=0,l=[],u=new K,d=new K,f=[],p=[],m=[],h=[];for(let f=0;f<=n;f++){let g=[],_=f/n,v=0;f===0&&a===0?v=.5/t:f===n&&s===Math.PI&&(v=-.5/t);for(let n=0;n<=t;n++){let s=n/t;u.x=-e*Math.cos(r+s*i)*Math.sin(a+_*o),u.y=e*Math.cos(a+_*o),u.z=e*Math.sin(r+s*i)*Math.sin(a+_*o),p.push(u.x,u.y,u.z),d.copy(u).normalize(),m.push(d.x,d.y,d.z),h.push(s+v,1-_),g.push(c++)}l.push(g)}for(let e=0;e<n;e++)for(let r=0;r<t;r++){let t=l[e][r+1],i=l[e][r],o=l[e+1][r],c=l[e+1][r+1];(e!==0||a>0)&&f.push(t,i,c),(e!==n-1||s<Math.PI)&&f.push(i,o,c)}this.setIndex(f),this.setAttribute(`position`,new Ka(p,3)),this.setAttribute(`normal`,new Ka(m,3)),this.setAttribute(`uv`,new Ka(h,2))}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(t){return new e(t.radius,t.widthSegments,t.heightSegments,t.phiStart,t.phiLength,t.thetaStart,t.thetaLength)}},ks={clone:cn,merge:ln},As=`void main() {
	gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}`,js=`void main() {
	gl_FragColor = vec4( 1.0, 0.0, 0.0, 1.0 );
}`,Ms=class extends oo{constructor(e){super(),this.isShaderMaterial=!0,this.type=`ShaderMaterial`,this.defines={},this.uniforms={},this.uniformsGroups=[],this.vertexShader=As,this.fragmentShader=js,this.linewidth=1,this.wireframe=!1,this.wireframeLinewidth=1,this.fog=!1,this.lights=!1,this.clipping=!1,this.forceSinglePass=!0,this.extensions={clipCullDistance:!1,multiDraw:!1},this.defaultAttributeValues={color:[1,1,1],uv:[0,0],uv1:[0,0]},this.index0AttributeName=void 0,this.uniformsNeedUpdate=!1,this.glslVersion=null,e!==void 0&&this.setValues(e)}copy(e){return super.copy(e),this.fragmentShader=e.fragmentShader,this.vertexShader=e.vertexShader,this.uniforms=cn(e.uniforms),this.uniformsGroups=dn(e.uniformsGroups),this.defines=Object.assign({},e.defines),this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.fog=e.fog,this.lights=e.lights,this.clipping=e.clipping,this.extensions=Object.assign({},e.extensions),this.glslVersion=e.glslVersion,this.defaultAttributeValues=Object.assign({},e.defaultAttributeValues),this.index0AttributeName=e.index0AttributeName,this.uniformsNeedUpdate=e.uniformsNeedUpdate,this}toJSON(e){let t=super.toJSON(e);t.glslVersion=this.glslVersion,t.uniforms={};for(let n in this.uniforms){let r=this.uniforms[n].value;r&&r.isTexture?t.uniforms[n]={type:`t`,value:r.toJSON(e).uuid}:r&&r.isColor?t.uniforms[n]={type:`c`,value:r.getHex()}:r&&r.isVector2?t.uniforms[n]={type:`v2`,value:r.toArray()}:r&&r.isVector3?t.uniforms[n]={type:`v3`,value:r.toArray()}:r&&r.isVector4?t.uniforms[n]={type:`v4`,value:r.toArray()}:r&&r.isMatrix3?t.uniforms[n]={type:`m3`,value:r.toArray()}:r&&r.isMatrix4?t.uniforms[n]={type:`m4`,value:r.toArray()}:t.uniforms[n]={value:r}}Object.keys(this.defines).length>0&&(t.defines=this.defines),t.vertexShader=this.vertexShader,t.fragmentShader=this.fragmentShader,t.lights=this.lights,t.clipping=this.clipping;let n={};for(let e in this.extensions)this.extensions[e]===!0&&(n[e]=!0);return Object.keys(n).length>0&&(t.extensions=n),t}},Ns=class extends Ms{constructor(e){super(e),this.isRawShaderMaterial=!0,this.type=`RawShaderMaterial`}},Ps=class extends oo{constructor(e){super(),this.isMeshStandardMaterial=!0,this.type=`MeshStandardMaterial`,this.defines={STANDARD:``},this.color=new X(16777215),this.roughness=1,this.metalness=0,this.map=null,this.lightMap=null,this.lightMapIntensity=1,this.aoMap=null,this.aoMapIntensity=1,this.emissive=new X(0),this.emissiveIntensity=1,this.emissiveMap=null,this.bumpMap=null,this.bumpScale=1,this.normalMap=null,this.normalMapType=0,this.normalScale=new fi(1,1),this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.roughnessMap=null,this.metalnessMap=null,this.alphaMap=null,this.envMap=null,this.envMapRotation=new Bi,this.envMapIntensity=1,this.wireframe=!1,this.wireframeLinewidth=1,this.wireframeLinecap=`round`,this.wireframeLinejoin=`round`,this.flatShading=!1,this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.defines={STANDARD:``},this.color.copy(e.color),this.roughness=e.roughness,this.metalness=e.metalness,this.map=e.map,this.lightMap=e.lightMap,this.lightMapIntensity=e.lightMapIntensity,this.aoMap=e.aoMap,this.aoMapIntensity=e.aoMapIntensity,this.emissive.copy(e.emissive),this.emissiveMap=e.emissiveMap,this.emissiveIntensity=e.emissiveIntensity,this.bumpMap=e.bumpMap,this.bumpScale=e.bumpScale,this.normalMap=e.normalMap,this.normalMapType=e.normalMapType,this.normalScale.copy(e.normalScale),this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this.roughnessMap=e.roughnessMap,this.metalnessMap=e.metalnessMap,this.alphaMap=e.alphaMap,this.envMap=e.envMap,this.envMapRotation.copy(e.envMapRotation),this.envMapIntensity=e.envMapIntensity,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.wireframeLinecap=e.wireframeLinecap,this.wireframeLinejoin=e.wireframeLinejoin,this.flatShading=e.flatShading,this.fog=e.fog,this}},Fs=class extends oo{constructor(e){super(),this.isMeshDepthMaterial=!0,this.type=`MeshDepthMaterial`,this.depthPacking=Xr,this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.wireframe=!1,this.wireframeLinewidth=1,this.setValues(e)}copy(e){return super.copy(e),this.depthPacking=e.depthPacking,this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this}},Is=class extends oo{constructor(e){super(),this.isMeshDistanceMaterial=!0,this.type=`MeshDistanceMaterial`,this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.setValues(e)}copy(e){return super.copy(e),this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this}},Ls=class{constructor(e,t,n,r){this.parameterPositions=e,this._cachedIndex=0,this.resultBuffer=r===void 0?new t.constructor(n):r,this.sampleValues=t,this.valueSize=n,this.settings=null,this.DefaultSettings_={}}evaluate(e){let t=this.parameterPositions,n=this._cachedIndex,r=t[n],i=t[n-1];validate_interval:{seek:{let a;linear_scan:{forward_scan:if(!(e<r)){for(let a=n+2;;){if(r===void 0){if(e<i)break forward_scan;return n=t.length,this._cachedIndex=n,this.copySampleValue_(n-1)}if(n===a)break;if(i=r,r=t[++n],e<r)break seek}a=t.length;break linear_scan}if(!(e>=i)){let o=t[1];e<o&&(n=2,i=o);for(let a=n-2;;){if(i===void 0)return this._cachedIndex=0,this.copySampleValue_(0);if(n===a)break;if(r=i,i=t[--n-1],e>=i)break seek}a=n,n=0;break linear_scan}break validate_interval}for(;n<a;){let r=n+a>>>1;e<t[r]?a=r:n=r+1}if(r=t[n],i=t[n-1],i===void 0)return this._cachedIndex=0,this.copySampleValue_(0);if(r===void 0)return n=t.length,this._cachedIndex=n,this.copySampleValue_(n-1)}this._cachedIndex=n,this.intervalChanged_(n,i,r)}return this.interpolate_(n,i,e,r)}getSettings_(){return this.settings||this.DefaultSettings_}copySampleValue_(e){let t=this.resultBuffer,n=this.sampleValues,r=this.valueSize,i=e*r;for(let e=0;e!==r;++e)t[e]=n[i+e];return t}interpolate_(){throw Error(`call to abstract method`)}intervalChanged_(){}},Rs=class extends Ls{constructor(e,t,n,r){super(e,t,n,r),this._weightPrev=-0,this._offsetPrev=-0,this._weightNext=-0,this._offsetNext=-0,this.DefaultSettings_={endingStart:Gr,endingEnd:Gr}}intervalChanged_(e,t,n){let r=this.parameterPositions,i=e-2,a=e+1,o=r[i],s=r[a];if(o===void 0)switch(this.getSettings_().endingStart){case Kr:i=e,o=2*t-n;break;case qr:i=r.length-2,o=t+r[i]-r[i+1];break;default:i=e,o=n}if(s===void 0)switch(this.getSettings_().endingEnd){case Kr:a=e,s=2*n-t;break;case qr:a=1,s=n+r[1]-r[0];break;default:a=e-1,s=t}let c=(n-t)*.5,l=this.valueSize;this._weightPrev=c/(t-o),this._weightNext=c/(s-n),this._offsetPrev=i*l,this._offsetNext=a*l}interpolate_(e,t,n,r){let i=this.resultBuffer,a=this.sampleValues,o=this.valueSize,s=e*o,c=s-o,l=this._offsetPrev,u=this._offsetNext,d=this._weightPrev,f=this._weightNext,p=(n-t)/(r-t),m=p*p,h=m*p,g=-d*h+2*d*m-d*p,_=(1+d)*h+(-1.5-2*d)*m+(-.5+d)*p+1,v=(-1-f)*h+(1.5+f)*m+.5*p,y=f*h-f*m;for(let e=0;e!==o;++e)i[e]=g*a[l+e]+_*a[c+e]+v*a[s+e]+y*a[u+e];return i}},zs=class extends Ls{constructor(e,t,n,r){super(e,t,n,r)}interpolate_(e,t,n,r){let i=this.resultBuffer,a=this.sampleValues,o=this.valueSize,s=e*o,c=s-o,l=(n-t)/(r-t),u=1-l;for(let e=0;e!==o;++e)i[e]=a[c+e]*u+a[s+e]*l;return i}},Bs=class extends Ls{constructor(e,t,n,r){super(e,t,n,r)}interpolate_(e){return this.copySampleValue_(e-1)}},Vs=class extends Ls{interpolate_(e,t,n,r){let i=this.resultBuffer,a=this.sampleValues,o=this.valueSize,s=e*o,c=s-o,l=this.settings||this.DefaultSettings_,u=l.inTangents,d=l.outTangents;if(!u||!d){let e=(n-t)/(r-t),l=1-e;for(let t=0;t!==o;++t)i[t]=a[c+t]*l+a[s+t]*e;return i}let f=o*2,p=e-1;for(let l=0;l!==o;++l){let o=a[c+l],m=a[s+l],h=p*f+l*2,g=d[h],_=d[h+1],v=e*f+l*2,y=u[v],b=u[v+1],x=(n-t)/(r-t),S,C,w,T,E;for(let e=0;e<8;e++){S=x*x,C=S*x,w=1-x,T=w*w,E=T*w;let e=E*t+3*T*x*g+3*w*S*y+C*r-n;if(Math.abs(e)<1e-10)break;let i=3*T*(g-t)+6*w*x*(y-g)+3*S*(r-y);if(Math.abs(i)<1e-10)break;x-=e/i,x=Math.max(0,Math.min(1,x))}i[l]=E*o+3*T*x*_+3*w*S*b+C*m}return i}},Hs=class{constructor(e,t,n,r){if(e===void 0)throw Error(`THREE.KeyframeTrack: track name is undefined`);if(t===void 0||t.length===0)throw Error(`THREE.KeyframeTrack: no keyframes in track named `+e);this.name=e,this.times=pn(t,this.TimeBufferType),this.values=pn(n,this.ValueBufferType),this.setInterpolation(r||this.DefaultInterpolation)}static toJSON(e){let t=e.constructor,n;if(t.toJSON!==this.toJSON)n=t.toJSON(e);else{n={name:e.name,times:pn(e.times,Array),values:pn(e.values,Array)};let t=e.getInterpolation();t!==e.DefaultInterpolation&&(n.interpolation=t)}return n.type=e.ValueTypeName,n}InterpolantFactoryMethodDiscrete(e){return new Bs(this.times,this.values,this.getValueSize(),e)}InterpolantFactoryMethodLinear(e){return new zs(this.times,this.values,this.getValueSize(),e)}InterpolantFactoryMethodSmooth(e){return new Rs(this.times,this.values,this.getValueSize(),e)}InterpolantFactoryMethodBezier(e){let t=new Vs(this.times,this.values,this.getValueSize(),e);return this.settings&&(t.settings=this.settings),t}setInterpolation(e){let t;switch(e){case Vr:t=this.InterpolantFactoryMethodDiscrete;break;case Hr:t=this.InterpolantFactoryMethodLinear;break;case Ur:t=this.InterpolantFactoryMethodSmooth;break;case Wr:t=this.InterpolantFactoryMethodBezier;break}if(t===void 0){let t=`unsupported interpolation for `+this.ValueTypeName+` keyframe track named `+this.name;if(this.createInterpolant===void 0)if(e!==this.DefaultInterpolation)this.setInterpolation(this.DefaultInterpolation);else throw Error(t);return W(`KeyframeTrack:`,t),this}return this.createInterpolant=t,this}getInterpolation(){switch(this.createInterpolant){case this.InterpolantFactoryMethodDiscrete:return Vr;case this.InterpolantFactoryMethodLinear:return Hr;case this.InterpolantFactoryMethodSmooth:return Ur;case this.InterpolantFactoryMethodBezier:return Wr}}getValueSize(){return this.values.length/this.times.length}shift(e){if(e!==0){let t=this.times;for(let n=0,r=t.length;n!==r;++n)t[n]+=e}return this}scale(e){if(e!==1){let t=this.times;for(let n=0,r=t.length;n!==r;++n)t[n]*=e}return this}trim(e,t){let n=this.times,r=n.length,i=0,a=r-1;for(;i!==r&&n[i]<e;)++i;for(;a!==-1&&n[a]>t;)--a;if(++a,i!==0||a!==r){i>=a&&(a=Math.max(a,1),i=a-1);let e=this.getValueSize();this.times=n.slice(i,a),this.values=this.values.slice(i*e,a*e)}return this}validate(){let e=!0,t=this.getValueSize();t-Math.floor(t)!==0&&(G(`KeyframeTrack: Invalid value size in track.`,this),e=!1);let n=this.times,r=this.values,i=n.length;i===0&&(G(`KeyframeTrack: Track is empty.`,this),e=!1);let a=null;for(let t=0;t!==i;t++){let r=n[t];if(typeof r==`number`&&isNaN(r)){G(`KeyframeTrack: Time is not a valid number.`,this,t,r),e=!1;break}if(a!==null&&a>r){G(`KeyframeTrack: Out of order keys.`,this,t,r,a),e=!1;break}a=r}if(r!==void 0&&Rt(r))for(let t=0,n=r.length;t!==n;++t){let n=r[t];if(isNaN(n)){G(`KeyframeTrack: Value is not a valid number.`,this,t,n),e=!1;break}}return e}optimize(){let e=this.times.slice(),t=this.values.slice(),n=this.getValueSize(),r=this.getInterpolation()===Ur,i=e.length-1,a=1;for(let o=1;o<i;++o){let i=!1,s=e[o];if(s!==e[o+1]&&(o!==1||s!==e[0]))if(r)i=!0;else{let e=o*n,r=e-n,a=e+n;for(let o=0;o!==n;++o){let n=t[e+o];if(n!==t[r+o]||n!==t[a+o]){i=!0;break}}}if(i){if(o!==a){e[a]=e[o];let r=o*n,i=a*n;for(let e=0;e!==n;++e)t[i+e]=t[r+e]}++a}}if(i>0){e[a]=e[i];for(let e=i*n,r=a*n,o=0;o!==n;++o)t[r+o]=t[e+o];++a}return a===e.length?(this.times=e,this.values=t):(this.times=e.slice(0,a),this.values=t.slice(0,a*n)),this}clone(){let e=this.times.slice(),t=this.values.slice(),n=this.constructor,r=new n(this.name,e,t);return r.createInterpolant=this.createInterpolant,r}},Hs.prototype.ValueTypeName=``,Hs.prototype.TimeBufferType=Float32Array,Hs.prototype.ValueBufferType=Float32Array,Hs.prototype.DefaultInterpolation=Hr,Us=class extends Hs{constructor(e,t,n){super(e,t,n)}},Us.prototype.ValueTypeName=`bool`,Us.prototype.ValueBufferType=Array,Us.prototype.DefaultInterpolation=Vr,Us.prototype.InterpolantFactoryMethodLinear=void 0,Us.prototype.InterpolantFactoryMethodSmooth=void 0,Ws=class extends Hs{constructor(e,t,n,r){super(e,t,n,r)}},Ws.prototype.ValueTypeName=`color`,Gs=class extends Hs{constructor(e,t,n,r){super(e,t,n,r)}},Gs.prototype.ValueTypeName=`number`,Ks=class extends Ls{constructor(e,t,n,r){super(e,t,n,r)}interpolate_(e,t,n,r){let i=this.resultBuffer,a=this.sampleValues,o=this.valueSize,s=(n-t)/(r-t),c=e*o;for(let e=c+o;c!==e;c+=4)pi.slerpFlat(i,0,a,c-o,a,c,s);return i}},qs=class extends Hs{constructor(e,t,n,r){super(e,t,n,r)}InterpolantFactoryMethodLinear(e){return new Ks(this.times,this.values,this.getValueSize(),e)}},qs.prototype.ValueTypeName=`quaternion`,qs.prototype.InterpolantFactoryMethodSmooth=void 0,Js=class extends Hs{constructor(e,t,n){super(e,t,n)}},Js.prototype.ValueTypeName=`string`,Js.prototype.ValueBufferType=Array,Js.prototype.DefaultInterpolation=Vr,Js.prototype.InterpolantFactoryMethodLinear=void 0,Js.prototype.InterpolantFactoryMethodSmooth=void 0,Ys=class extends Hs{constructor(e,t,n,r){super(e,t,n,r)}},Ys.prototype.ValueTypeName=`vector`,Xs=class{constructor(e=``,t=-1,n=[],r=Jr){this.name=e,this.tracks=n,this.duration=t,this.blendMode=r,this.uuid=Gt(),this.userData={},this.duration<0&&this.resetDuration()}static parse(e){let t=[],n=e.tracks,r=1/(e.fps||1);for(let e=0,i=n.length;e!==i;++e)t.push(vn(n[e]).scale(r));let i=new this(e.name,e.duration,t,e.blendMode);return i.uuid=e.uuid,i.userData=JSON.parse(e.userData||`{}`),i}static toJSON(e){let t=[],n=e.tracks,r={name:e.name,duration:e.duration,tracks:t,uuid:e.uuid,blendMode:e.blendMode,userData:JSON.stringify(e.userData)};for(let e=0,r=n.length;e!==r;++e)t.push(Hs.toJSON(n[e]));return r}static CreateFromMorphTargetSequence(e,t,n,r){let i=t.length,a=[];for(let e=0;e<i;e++){let o=[],s=[];o.push((e+i-1)%i,e,(e+1)%i),s.push(0,1,0);let c=mn(o);o=hn(o,1,c),s=hn(s,1,c),!r&&o[0]===0&&(o.push(i),s.push(s[0])),a.push(new Gs(`.morphTargetInfluences[`+t[e].name+`]`,o,s).scale(1/n))}return new this(e,-1,a)}static findByName(e,t){let n=e;if(!Array.isArray(e)){let t=e;n=t.geometry&&t.geometry.animations||t.animations}for(let e=0;e<n.length;e++)if(n[e].name===t)return n[e];return null}static CreateClipsFromMorphTargetSequences(e,t,n){let r={},i=/^([\w-]*?)([\d]+)$/;for(let t=0,n=e.length;t<n;t++){let n=e[t],a=n.name.match(i);if(a&&a.length>1){let e=a[1],t=r[e];t||(r[e]=t=[]),t.push(n)}}let a=[];for(let e in r)a.push(this.CreateFromMorphTargetSequence(e,r[e],t,n));return a}static parseAnimation(e,t){if(W(`AnimationClip: parseAnimation() is deprecated and will be removed with r185`),!e)return G(`AnimationClip: No animation in JSONLoader data.`),null;let n=function(e,t,n,r,i){if(n.length!==0){let a=[],o=[];gn(n,a,o,r),a.length!==0&&i.push(new e(t,a,o))}},r=[],i=e.name||`default`,a=e.fps||30,o=e.blendMode,s=e.length||-1,c=e.hierarchy||[];for(let e=0;e<c.length;e++){let i=c[e].keys;if(!(!i||i.length===0))if(i[0].morphTargets){let e={},t;for(t=0;t<i.length;t++)if(i[t].morphTargets)for(let n=0;n<i[t].morphTargets.length;n++)e[i[t].morphTargets[n]]=-1;for(let n in e){let e=[],a=[];for(let r=0;r!==i[t].morphTargets.length;++r){let r=i[t];e.push(r.time),a.push(+(r.morphTarget===n))}r.push(new Gs(`.morphTargetInfluence[`+n+`]`,e,a))}s=e.length*a}else{let a=`.bones[`+t[e].name+`]`;n(Ys,a+`.position`,i,`pos`,r),n(qs,a+`.quaternion`,i,`rot`,r),n(Ys,a+`.scale`,i,`scl`,r)}}return r.length===0?null:new this(i,s,r,o)}resetDuration(){let e=this.tracks,t=0;for(let n=0,r=e.length;n!==r;++n){let e=this.tracks[n];t=Math.max(t,e.times[e.times.length-1])}return this.duration=t,this}trim(){for(let e=0;e<this.tracks.length;e++)this.tracks[e].trim(0,this.duration);return this}validate(){let e=!0;for(let t=0;t<this.tracks.length;t++)e&&=this.tracks[t].validate();return e}optimize(){for(let e=0;e<this.tracks.length;e++)this.tracks[e].optimize();return this}clone(){let e=[];for(let t=0;t<this.tracks.length;t++)e.push(this.tracks[t].clone());let t=new this.constructor(this.name,this.duration,e,this.blendMode);return t.userData=JSON.parse(JSON.stringify(this.userData)),t}toJSON(){return this.constructor.toJSON(this)}},Zs=class{constructor(e,t,n){let r=this,i=!1,a=0,o=0,s,c=[];this.onStart=void 0,this.onLoad=e,this.onProgress=t,this.onError=n,this._abortController=null,this.itemStart=function(e){o++,i===!1&&r.onStart!==void 0&&r.onStart(e,a,o),i=!0},this.itemEnd=function(e){a++,r.onProgress!==void 0&&r.onProgress(e,a,o),a===o&&(i=!1,r.onLoad!==void 0&&r.onLoad())},this.itemError=function(e){r.onError!==void 0&&r.onError(e)},this.resolveURL=function(e){return s?s(e):e},this.setURLModifier=function(e){return s=e,this},this.addHandler=function(e,t){return c.push(e,t),this},this.removeHandler=function(e){let t=c.indexOf(e);return t!==-1&&c.splice(t,2),this},this.getHandler=function(e){for(let t=0,n=c.length;t<n;t+=2){let n=c[t],r=c[t+1];if(n.global&&(n.lastIndex=0),n.test(e))return r}return null},this.abort=function(){return this.abortController.abort(),this._abortController=null,this}}get abortController(){return this._abortController||=new AbortController,this._abortController}},Qs=new Zs,$s=class{constructor(e){this.manager=e===void 0?Qs:e,this.crossOrigin=`anonymous`,this.withCredentials=!1,this.path=``,this.resourcePath=``,this.requestHeader={},typeof __THREE_DEVTOOLS__<`u`&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent(`observe`,{detail:this}))}load(){}loadAsync(e,t){let n=this;return new Promise(function(r,i){n.load(e,r,t,i)})}parse(){}setCrossOrigin(e){return this.crossOrigin=e,this}setWithCredentials(e){return this.withCredentials=e,this}setPath(e){return this.path=e,this}setResourcePath(e){return this.resourcePath=e,this}setRequestHeader(e){return this.requestHeader=e,this}abort(){return this}},$s.DEFAULT_MATERIAL_NAME=`__DEFAULT`,ec=class extends ra{constructor(e,t=1){super(),this.isLight=!0,this.type=`Light`,this.color=new X(e),this.intensity=t}dispose(){this.dispatchEvent({type:`dispose`})}copy(e,t){return super.copy(e,t),this.color.copy(e.color),this.intensity=e.intensity,this}toJSON(e){let t=super.toJSON(e);return t.object.color=this.color.getHex(),t.object.intensity=this.intensity,t}},tc=class extends ec{constructor(e,t,n){super(e,n),this.isHemisphereLight=!0,this.type=`HemisphereLight`,this.position.copy(ra.DEFAULT_UP),this.updateMatrix(),this.groundColor=new X(t)}copy(e,t){return super.copy(e,t),this.groundColor.copy(e.groundColor),this}toJSON(e){let t=super.toJSON(e);return t.object.groundColor=this.groundColor.getHex(),t}},nc=new Y,rc=new K,ic=new K,ac=class{constructor(e){this.camera=e,this.intensity=1,this.bias=0,this.biasNode=null,this.normalBias=0,this.radius=1,this.blurSamples=8,this.mapSize=new fi(512,512),this.mapType=Nn,this.map=null,this.mapPass=null,this.matrix=new Y,this.autoUpdate=!0,this.needsUpdate=!1,this._frustum=new rs,this._frameExtents=new fi(1,1),this._viewportCount=1,this._viewports=[new Ei(0,0,1,1)]}getViewportCount(){return this._viewportCount}getFrustum(){return this._frustum}updateMatrices(e){let t=this.camera,n=this.matrix;rc.setFromMatrixPosition(e.matrixWorld),t.position.copy(rc),ic.setFromMatrixPosition(e.target.matrixWorld),t.lookAt(ic),t.updateMatrixWorld(),nc.multiplyMatrices(t.projectionMatrix,t.matrixWorldInverse),this._frustum.setFromProjectionMatrix(nc,t.coordinateSystem,t.reversedDepth),t.coordinateSystem===2001||t.reversedDepth?n.set(.5,0,0,.5,0,.5,0,.5,0,0,1,0,0,0,0,1):n.set(.5,0,0,.5,0,.5,0,.5,0,0,.5,.5,0,0,0,1),n.multiply(nc)}getViewport(e){return this._viewports[e]}getFrameExtents(){return this._frameExtents}dispose(){this.map&&this.map.dispose(),this.mapPass&&this.mapPass.dispose()}copy(e){return this.camera=e.camera.clone(),this.intensity=e.intensity,this.bias=e.bias,this.radius=e.radius,this.autoUpdate=e.autoUpdate,this.needsUpdate=e.needsUpdate,this.normalBias=e.normalBias,this.blurSamples=e.blurSamples,this.mapSize.copy(e.mapSize),this.biasNode=e.biasNode,this}clone(){return new this.constructor().copy(this)}toJSON(){let e={};return this.intensity!==1&&(e.intensity=this.intensity),this.bias!==0&&(e.bias=this.bias),this.normalBias!==0&&(e.normalBias=this.normalBias),this.radius!==1&&(e.radius=this.radius),(this.mapSize.x!==512||this.mapSize.y!==512)&&(e.mapSize=this.mapSize.toArray()),e.camera=this.camera.toJSON(!1).object,delete e.camera.matrix,e}},oc=new K,sc=new pi,cc=new K,lc=class extends ra{constructor(){super(),this.isCamera=!0,this.type=`Camera`,this.matrixWorldInverse=new Y,this.projectionMatrix=new Y,this.projectionMatrixInverse=new Y,this.coordinateSystem=ii,this._reversedDepth=!1}get reversedDepth(){return this._reversedDepth}copy(e,t){return super.copy(e,t),this.matrixWorldInverse.copy(e.matrixWorldInverse),this.projectionMatrix.copy(e.projectionMatrix),this.projectionMatrixInverse.copy(e.projectionMatrixInverse),this.coordinateSystem=e.coordinateSystem,this}getWorldDirection(e){return super.getWorldDirection(e).negate()}updateMatrixWorld(e){super.updateMatrixWorld(e),this.matrixWorld.decompose(oc,sc,cc),cc.x===1&&cc.y===1&&cc.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(oc,sc,cc.set(1,1,1)).invert()}updateWorldMatrix(e,t){super.updateWorldMatrix(e,t),this.matrixWorld.decompose(oc,sc,cc),cc.x===1&&cc.y===1&&cc.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(oc,sc,cc.set(1,1,1)).invert()}clone(){return new this.constructor().copy(this)}},uc=new K,dc=new fi,fc=new fi,pc=class extends lc{constructor(e=50,t=1,n=.1,r=2e3){super(),this.isPerspectiveCamera=!0,this.type=`PerspectiveCamera`,this.fov=e,this.zoom=1,this.near=n,this.far=r,this.focus=10,this.aspect=t,this.view=null,this.filmGauge=35,this.filmOffset=0,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.fov=e.fov,this.zoom=e.zoom,this.near=e.near,this.far=e.far,this.focus=e.focus,this.aspect=e.aspect,this.view=e.view===null?null:Object.assign({},e.view),this.filmGauge=e.filmGauge,this.filmOffset=e.filmOffset,this}setFocalLength(e){let t=.5*this.getFilmHeight()/e;this.fov=di*2*Math.atan(t),this.updateProjectionMatrix()}getFocalLength(){let e=Math.tan(ui*.5*this.fov);return .5*this.getFilmHeight()/e}getEffectiveFOV(){return di*2*Math.atan(Math.tan(ui*.5*this.fov)/this.zoom)}getFilmWidth(){return this.filmGauge*Math.min(this.aspect,1)}getFilmHeight(){return this.filmGauge/Math.max(this.aspect,1)}getViewBounds(e,t,n){uc.set(-1,-1,.5).applyMatrix4(this.projectionMatrixInverse),t.set(uc.x,uc.y).multiplyScalar(-e/uc.z),uc.set(1,1,.5).applyMatrix4(this.projectionMatrixInverse),n.set(uc.x,uc.y).multiplyScalar(-e/uc.z)}getViewSize(e,t){return this.getViewBounds(e,dc,fc),t.subVectors(fc,dc)}setViewOffset(e,t,n,r,i,a){this.aspect=e/t,this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=n,this.view.offsetY=r,this.view.width=i,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){let e=this.near,t=e*Math.tan(ui*.5*this.fov)/this.zoom,n=2*t,r=this.aspect*n,i=-.5*r,a=this.view;if(this.view!==null&&this.view.enabled){let e=a.fullWidth,o=a.fullHeight;i+=a.offsetX*r/e,t-=a.offsetY*n/o,r*=a.width/e,n*=a.height/o}let o=this.filmOffset;o!==0&&(i+=e*o/this.getFilmWidth()),this.projectionMatrix.makePerspective(i,i+r,t,t-n,e,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){let t=super.toJSON(e);return t.object.fov=this.fov,t.object.zoom=this.zoom,t.object.near=this.near,t.object.far=this.far,t.object.focus=this.focus,t.object.aspect=this.aspect,this.view!==null&&(t.object.view=Object.assign({},this.view)),t.object.filmGauge=this.filmGauge,t.object.filmOffset=this.filmOffset,t}},mc=class extends ac{constructor(){super(new pc(50,1,.5,500)),this.isSpotLightShadow=!0,this.focus=1,this.aspect=1}updateMatrices(e){let t=this.camera,n=di*2*e.angle*this.focus,r=this.mapSize.width/this.mapSize.height*this.aspect,i=e.distance||t.far;(n!==t.fov||r!==t.aspect||i!==t.far)&&(t.fov=n,t.aspect=r,t.far=i,t.updateProjectionMatrix()),super.updateMatrices(e)}copy(e){return super.copy(e),this.focus=e.focus,this}},hc=class extends ec{constructor(e,t,n=0,r=Math.PI/3,i=0,a=2){super(e,t),this.isSpotLight=!0,this.type=`SpotLight`,this.position.copy(ra.DEFAULT_UP),this.updateMatrix(),this.target=new ra,this.distance=n,this.angle=r,this.penumbra=i,this.decay=a,this.map=null,this.shadow=new mc}get power(){return this.intensity*Math.PI}set power(e){this.intensity=e/Math.PI}dispose(){super.dispose(),this.shadow.dispose()}copy(e,t){return super.copy(e,t),this.distance=e.distance,this.angle=e.angle,this.penumbra=e.penumbra,this.decay=e.decay,this.target=e.target.clone(),this.map=e.map,this.shadow=e.shadow.clone(),this}toJSON(e){let t=super.toJSON(e);return t.object.distance=this.distance,t.object.angle=this.angle,t.object.decay=this.decay,t.object.penumbra=this.penumbra,t.object.target=this.target.uuid,this.map&&this.map.isTexture&&(t.object.map=this.map.toJSON(e).uuid),t.object.shadow=this.shadow.toJSON(),t}},gc=class extends ac{constructor(){super(new pc(90,1,.5,500)),this.isPointLightShadow=!0}},_c=class extends ec{constructor(e,t,n=0,r=2){super(e,t),this.isPointLight=!0,this.type=`PointLight`,this.distance=n,this.decay=r,this.shadow=new gc}get power(){return this.intensity*4*Math.PI}set power(e){this.intensity=e/(4*Math.PI)}dispose(){super.dispose(),this.shadow.dispose()}copy(e,t){return super.copy(e,t),this.distance=e.distance,this.decay=e.decay,this.shadow=e.shadow.clone(),this}toJSON(e){let t=super.toJSON(e);return t.object.distance=this.distance,t.object.decay=this.decay,t.object.shadow=this.shadow.toJSON(),t}},vc=class extends lc{constructor(e=-1,t=1,n=1,r=-1,i=.1,a=2e3){super(),this.isOrthographicCamera=!0,this.type=`OrthographicCamera`,this.zoom=1,this.view=null,this.left=e,this.right=t,this.top=n,this.bottom=r,this.near=i,this.far=a,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.left=e.left,this.right=e.right,this.top=e.top,this.bottom=e.bottom,this.near=e.near,this.far=e.far,this.zoom=e.zoom,this.view=e.view===null?null:Object.assign({},e.view),this}setViewOffset(e,t,n,r,i,a){this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=n,this.view.offsetY=r,this.view.width=i,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){let e=(this.right-this.left)/(2*this.zoom),t=(this.top-this.bottom)/(2*this.zoom),n=(this.right+this.left)/2,r=(this.top+this.bottom)/2,i=n-e,a=n+e,o=r+t,s=r-t;if(this.view!==null&&this.view.enabled){let e=(this.right-this.left)/this.view.fullWidth/this.zoom,t=(this.top-this.bottom)/this.view.fullHeight/this.zoom;i+=e*this.view.offsetX,a=i+e*this.view.width,o-=t*this.view.offsetY,s=o-t*this.view.height}this.projectionMatrix.makeOrthographic(i,a,o,s,this.near,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){let t=super.toJSON(e);return t.object.zoom=this.zoom,t.object.left=this.left,t.object.right=this.right,t.object.top=this.top,t.object.bottom=this.bottom,t.object.near=this.near,t.object.far=this.far,this.view!==null&&(t.object.view=Object.assign({},this.view)),t}},yc=class extends ac{constructor(){super(new vc(-5,5,5,-5,.5,500)),this.isDirectionalLightShadow=!0}},bc=class extends ec{constructor(e,t){super(e,t),this.isDirectionalLight=!0,this.type=`DirectionalLight`,this.position.copy(ra.DEFAULT_UP),this.updateMatrix(),this.target=new ra,this.shadow=new yc}dispose(){super.dispose(),this.shadow.dispose()}copy(e){return super.copy(e),this.target=e.target.clone(),this.shadow=e.shadow.clone(),this}toJSON(e){let t=super.toJSON(e);return t.object.shadow=this.shadow.toJSON(),t.object.target=this.target.uuid,t}},xc=class extends ec{constructor(e,t){super(e,t),this.isAmbientLight=!0,this.type=`AmbientLight`}},Sc=-90,Cc=1,wc=class extends ra{constructor(e,t,n){super(),this.type=`CubeCamera`,this.renderTarget=n,this.coordinateSystem=null,this.activeMipmapLevel=0;let r=new pc(Sc,Cc,e,t);r.layers=this.layers,this.add(r);let i=new pc(Sc,Cc,e,t);i.layers=this.layers,this.add(i);let a=new pc(Sc,Cc,e,t);a.layers=this.layers,this.add(a);let o=new pc(Sc,Cc,e,t);o.layers=this.layers,this.add(o);let s=new pc(Sc,Cc,e,t);s.layers=this.layers,this.add(s);let c=new pc(Sc,Cc,e,t);c.layers=this.layers,this.add(c)}updateCoordinateSystem(){let e=this.coordinateSystem,t=this.children.concat(),[n,r,i,a,o,s]=t;for(let e of t)this.remove(e);if(e===2e3)n.up.set(0,1,0),n.lookAt(1,0,0),r.up.set(0,1,0),r.lookAt(-1,0,0),i.up.set(0,0,-1),i.lookAt(0,1,0),a.up.set(0,0,1),a.lookAt(0,-1,0),o.up.set(0,1,0),o.lookAt(0,0,1),s.up.set(0,1,0),s.lookAt(0,0,-1);else if(e===2001)n.up.set(0,-1,0),n.lookAt(-1,0,0),r.up.set(0,-1,0),r.lookAt(1,0,0),i.up.set(0,0,1),i.lookAt(0,1,0),a.up.set(0,0,-1),a.lookAt(0,-1,0),o.up.set(0,-1,0),o.lookAt(0,0,1),s.up.set(0,-1,0),s.lookAt(0,0,-1);else throw Error(`THREE.CubeCamera.updateCoordinateSystem(): Invalid coordinate system: `+e);for(let e of t)this.add(e),e.updateMatrixWorld()}update(e,t){this.parent===null&&this.updateMatrixWorld();let{renderTarget:n,activeMipmapLevel:r}=this;this.coordinateSystem!==e.coordinateSystem&&(this.coordinateSystem=e.coordinateSystem,this.updateCoordinateSystem());let[i,a,o,s,c,l]=this.children,u=e.getRenderTarget(),d=e.getActiveCubeFace(),f=e.getActiveMipmapLevel(),p=e.xr.enabled;e.xr.enabled=!1;let m=n.texture.generateMipmaps;n.texture.generateMipmaps=!1;let h=!1;h=e.isWebGLRenderer===!0?e.state.buffers.depth.getReversed():e.reversedDepthBuffer,e.setRenderTarget(n,0,r),h&&e.autoClear===!1&&e.clearDepth(),e.render(t,i),e.setRenderTarget(n,1,r),h&&e.autoClear===!1&&e.clearDepth(),e.render(t,a),e.setRenderTarget(n,2,r),h&&e.autoClear===!1&&e.clearDepth(),e.render(t,o),e.setRenderTarget(n,3,r),h&&e.autoClear===!1&&e.clearDepth(),e.render(t,s),e.setRenderTarget(n,4,r),h&&e.autoClear===!1&&e.clearDepth(),e.render(t,c),n.texture.generateMipmaps=m,e.setRenderTarget(n,5,r),h&&e.autoClear===!1&&e.clearDepth(),e.render(t,l),e.setRenderTarget(u,d,f),e.xr.enabled=p,n.texture.needsPMREMUpdate=!0}},Tc=class extends pc{constructor(e=[]){super(),this.isArrayCamera=!0,this.isMultiViewCamera=!1,this.cameras=e}},Ec=class{constructor(e,t,n){this.binding=e,this.valueSize=n;let r,i,a;switch(t){case`quaternion`:r=this._slerp,i=this._slerpAdditive,a=this._setAdditiveIdentityQuaternion,this.buffer=new Float64Array(n*6),this._workIndex=5;break;case`string`:case`bool`:r=this._select,i=this._select,a=this._setAdditiveIdentityOther,this.buffer=Array(n*5);break;default:r=this._lerp,i=this._lerpAdditive,a=this._setAdditiveIdentityNumeric,this.buffer=new Float64Array(n*5)}this._mixBufferRegion=r,this._mixBufferRegionAdditive=i,this._setIdentity=a,this._origIndex=3,this._addIndex=4,this.cumulativeWeight=0,this.cumulativeWeightAdditive=0,this.useCount=0,this.referenceCount=0}accumulate(e,t){let n=this.buffer,r=this.valueSize,i=e*r+r,a=this.cumulativeWeight;if(a===0){for(let e=0;e!==r;++e)n[i+e]=n[e];a=t}else{a+=t;let e=t/a;this._mixBufferRegion(n,i,0,e,r)}this.cumulativeWeight=a}accumulateAdditive(e){let t=this.buffer,n=this.valueSize,r=n*this._addIndex;this.cumulativeWeightAdditive===0&&this._setIdentity(),this._mixBufferRegionAdditive(t,r,0,e,n),this.cumulativeWeightAdditive+=e}apply(e){let t=this.valueSize,n=this.buffer,r=e*t+t,i=this.cumulativeWeight,a=this.cumulativeWeightAdditive,o=this.binding;if(this.cumulativeWeight=0,this.cumulativeWeightAdditive=0,i<1){let e=t*this._origIndex;this._mixBufferRegion(n,r,e,1-i,t)}a>0&&this._mixBufferRegionAdditive(n,r,this._addIndex*t,1,t);for(let e=t,i=t+t;e!==i;++e)if(n[e]!==n[e+t]){o.setValue(n,r);break}}saveOriginalState(){let e=this.binding,t=this.buffer,n=this.valueSize,r=n*this._origIndex;e.getValue(t,r);for(let e=n,i=r;e!==i;++e)t[e]=t[r+e%n];this._setIdentity(),this.cumulativeWeight=0,this.cumulativeWeightAdditive=0}restoreOriginalState(){let e=this.valueSize*3;this.binding.setValue(this.buffer,e)}_setAdditiveIdentityNumeric(){let e=this._addIndex*this.valueSize,t=e+this.valueSize;for(let n=e;n<t;n++)this.buffer[n]=0}_setAdditiveIdentityQuaternion(){this._setAdditiveIdentityNumeric(),this.buffer[this._addIndex*this.valueSize+3]=1}_setAdditiveIdentityOther(){let e=this._origIndex*this.valueSize,t=this._addIndex*this.valueSize;for(let n=0;n<this.valueSize;n++)this.buffer[t+n]=this.buffer[e+n]}_select(e,t,n,r,i){if(r>=.5)for(let r=0;r!==i;++r)e[t+r]=e[n+r]}_slerp(e,t,n,r){pi.slerpFlat(e,t,e,t,e,n,r)}_slerpAdditive(e,t,n,r,i){let a=this._workIndex*i;pi.multiplyQuaternionsFlat(e,a,e,t,e,n),pi.slerpFlat(e,t,e,t,e,a,r)}_lerp(e,t,n,r,i){let a=1-r;for(let o=0;o!==i;++o){let i=t+o;e[i]=e[i]*a+e[n+o]*r}}_lerpAdditive(e,t,n,r,i){for(let a=0;a!==i;++a){let i=t+a;e[i]=e[i]+e[n+a]*r}}},Dc=`\\[\\]\\.:\\/`,Oc=RegExp(`[\\[\\]\\.:\\/]`,`g`),kc=`[^\\[\\]\\.:\\/]`,Ac=`[^`+Dc.replace(`\\.`,``)+`]`,jc=`((?:WC+[\\/:])*)`.replace(`WC`,kc),Mc=`(WCOD+)?`.replace(`WCOD`,Ac),Nc=`(?:\\.(WC+)(?:\\[(.+)\\])?)?`.replace(`WC`,kc),Pc=`\\.(WC+)(?:\\[(.+)\\])?`.replace(`WC`,kc),Fc=RegExp(`^`+jc+Mc+Nc+Pc+`$`),Ic=[`material`,`materials`,`bones`,`map`],Lc=class{constructor(e,t,n){let r=n||Rc.parseTrackName(t);this._targetGroup=e,this._bindings=e.subscribe_(t,r)}getValue(e,t){this.bind();let n=this._targetGroup.nCachedObjects_,r=this._bindings[n];r!==void 0&&r.getValue(e,t)}setValue(e,t){let n=this._bindings;for(let r=this._targetGroup.nCachedObjects_,i=n.length;r!==i;++r)n[r].setValue(e,t)}bind(){let e=this._bindings;for(let t=this._targetGroup.nCachedObjects_,n=e.length;t!==n;++t)e[t].bind()}unbind(){let e=this._bindings;for(let t=this._targetGroup.nCachedObjects_,n=e.length;t!==n;++t)e[t].unbind()}},Rc=class e{constructor(t,n,r){this.path=n,this.parsedPath=r||e.parseTrackName(n),this.node=e.findNode(t,this.parsedPath.nodeName),this.rootNode=t,this.getValue=this._getValue_unbound,this.setValue=this._setValue_unbound}static create(t,n,r){return t&&t.isAnimationObjectGroup?new e.Composite(t,n,r):new e(t,n,r)}static sanitizeNodeName(e){return e.replace(/\s/g,`_`).replace(Oc,``)}static parseTrackName(e){let t=Fc.exec(e);if(t===null)throw Error(`PropertyBinding: Cannot parse trackName: `+e);let n={nodeName:t[2],objectName:t[3],objectIndex:t[4],propertyName:t[5],propertyIndex:t[6]},r=n.nodeName&&n.nodeName.lastIndexOf(`.`);if(r!==void 0&&r!==-1){let e=n.nodeName.substring(r+1);Ic.indexOf(e)!==-1&&(n.nodeName=n.nodeName.substring(0,r),n.objectName=e)}if(n.propertyName===null||n.propertyName.length===0)throw Error(`PropertyBinding: can not parse propertyName from trackName: `+e);return n}static findNode(e,t){if(t===void 0||t===``||t===`.`||t===-1||t===e.name||t===e.uuid)return e;if(e.skeleton){let n=e.skeleton.getBoneByName(t);if(n!==void 0)return n}if(e.children){let n=function(e){for(let r=0;r<e.length;r++){let i=e[r];if(i.name===t||i.uuid===t)return i;let a=n(i.children);if(a)return a}return null},r=n(e.children);if(r)return r}return null}_getValue_unavailable(){}_setValue_unavailable(){}_getValue_direct(e,t){e[t]=this.targetObject[this.propertyName]}_getValue_array(e,t){let n=this.resolvedProperty;for(let r=0,i=n.length;r!==i;++r)e[t++]=n[r]}_getValue_arrayElement(e,t){e[t]=this.resolvedProperty[this.propertyIndex]}_getValue_toArray(e,t){this.resolvedProperty.toArray(e,t)}_setValue_direct(e,t){this.targetObject[this.propertyName]=e[t]}_setValue_direct_setNeedsUpdate(e,t){this.targetObject[this.propertyName]=e[t],this.targetObject.needsUpdate=!0}_setValue_direct_setMatrixWorldNeedsUpdate(e,t){this.targetObject[this.propertyName]=e[t],this.targetObject.matrixWorldNeedsUpdate=!0}_setValue_array(e,t){let n=this.resolvedProperty;for(let r=0,i=n.length;r!==i;++r)n[r]=e[t++]}_setValue_array_setNeedsUpdate(e,t){let n=this.resolvedProperty;for(let r=0,i=n.length;r!==i;++r)n[r]=e[t++];this.targetObject.needsUpdate=!0}_setValue_array_setMatrixWorldNeedsUpdate(e,t){let n=this.resolvedProperty;for(let r=0,i=n.length;r!==i;++r)n[r]=e[t++];this.targetObject.matrixWorldNeedsUpdate=!0}_setValue_arrayElement(e,t){this.resolvedProperty[this.propertyIndex]=e[t]}_setValue_arrayElement_setNeedsUpdate(e,t){this.resolvedProperty[this.propertyIndex]=e[t],this.targetObject.needsUpdate=!0}_setValue_arrayElement_setMatrixWorldNeedsUpdate(e,t){this.resolvedProperty[this.propertyIndex]=e[t],this.targetObject.matrixWorldNeedsUpdate=!0}_setValue_fromArray(e,t){this.resolvedProperty.fromArray(e,t)}_setValue_fromArray_setNeedsUpdate(e,t){this.resolvedProperty.fromArray(e,t),this.targetObject.needsUpdate=!0}_setValue_fromArray_setMatrixWorldNeedsUpdate(e,t){this.resolvedProperty.fromArray(e,t),this.targetObject.matrixWorldNeedsUpdate=!0}_getValue_unbound(e,t){this.bind(),this.getValue(e,t)}_setValue_unbound(e,t){this.bind(),this.setValue(e,t)}bind(){let t=this.node,n=this.parsedPath,r=n.objectName,i=n.propertyName,a=n.propertyIndex;if(t||(t=e.findNode(this.rootNode,n.nodeName),this.node=t),this.getValue=this._getValue_unavailable,this.setValue=this._setValue_unavailable,!t){W(`PropertyBinding: No target node found for track: `+this.path+`.`);return}if(r){let e=n.objectIndex;switch(r){case`materials`:if(!t.material){G(`PropertyBinding: Can not bind to material as node does not have a material.`,this);return}if(!t.material.materials){G(`PropertyBinding: Can not bind to material.materials as node.material does not have a materials array.`,this);return}t=t.material.materials;break;case`bones`:if(!t.skeleton){G(`PropertyBinding: Can not bind to bones as node does not have a skeleton.`,this);return}t=t.skeleton.bones;for(let n=0;n<t.length;n++)if(t[n].name===e){e=n;break}break;case`map`:if(`map`in t){t=t.map;break}if(!t.material){G(`PropertyBinding: Can not bind to material as node does not have a material.`,this);return}if(!t.material.map){G(`PropertyBinding: Can not bind to material.map as node.material does not have a map.`,this);return}t=t.material.map;break;default:if(t[r]===void 0){G(`PropertyBinding: Can not bind to objectName of node undefined.`,this);return}t=t[r]}if(e!==void 0){if(t[e]===void 0){G(`PropertyBinding: Trying to bind to objectIndex of objectName, but is undefined.`,this,t);return}t=t[e]}}let o=t[i];if(o===void 0){let e=n.nodeName;G(`PropertyBinding: Trying to update property for track: `+e+`.`+i+` but it wasn't found.`,t);return}let s=this.Versioning.None;this.targetObject=t,t.isMaterial===!0?s=this.Versioning.NeedsUpdate:t.isObject3D===!0&&(s=this.Versioning.MatrixWorldNeedsUpdate);let c=this.BindingType.Direct;if(a!==void 0){if(i===`morphTargetInfluences`){if(!t.geometry){G(`PropertyBinding: Can not bind to morphTargetInfluences because node does not have a geometry.`,this);return}if(!t.geometry.morphAttributes){G(`PropertyBinding: Can not bind to morphTargetInfluences because node does not have a geometry.morphAttributes.`,this);return}t.morphTargetDictionary[a]!==void 0&&(a=t.morphTargetDictionary[a])}c=this.BindingType.ArrayElement,this.resolvedProperty=o,this.propertyIndex=a}else o.fromArray!==void 0&&o.toArray!==void 0?(c=this.BindingType.HasFromToArray,this.resolvedProperty=o):Array.isArray(o)?(c=this.BindingType.EntireArray,this.resolvedProperty=o):this.propertyName=i;this.getValue=this.GetterByBindingType[c],this.setValue=this.SetterByBindingTypeAndVersioning[c][s]}unbind(){this.node=null,this.getValue=this._getValue_unbound,this.setValue=this._setValue_unbound}},Rc.Composite=Lc,Rc.prototype.BindingType={Direct:0,EntireArray:1,ArrayElement:2,HasFromToArray:3},Rc.prototype.Versioning={None:0,NeedsUpdate:1,MatrixWorldNeedsUpdate:2},Rc.prototype.GetterByBindingType=[Rc.prototype._getValue_direct,Rc.prototype._getValue_array,Rc.prototype._getValue_arrayElement,Rc.prototype._getValue_toArray],Rc.prototype.SetterByBindingTypeAndVersioning=[[Rc.prototype._setValue_direct,Rc.prototype._setValue_direct_setNeedsUpdate,Rc.prototype._setValue_direct_setMatrixWorldNeedsUpdate],[Rc.prototype._setValue_array,Rc.prototype._setValue_array_setNeedsUpdate,Rc.prototype._setValue_array_setMatrixWorldNeedsUpdate],[Rc.prototype._setValue_arrayElement,Rc.prototype._setValue_arrayElement_setNeedsUpdate,Rc.prototype._setValue_arrayElement_setMatrixWorldNeedsUpdate],[Rc.prototype._setValue_fromArray,Rc.prototype._setValue_fromArray_setNeedsUpdate,Rc.prototype._setValue_fromArray_setMatrixWorldNeedsUpdate]],zc=class{constructor(e,t,n=null,r=t.blendMode){this._mixer=e,this._clip=t,this._localRoot=n,this.blendMode=r;let i=t.tracks,a=i.length,o=Array(a),s={endingStart:Gr,endingEnd:Gr};for(let e=0;e!==a;++e){let t=i[e].createInterpolant(null);o[e]=t,t.settings&&Object.assign(s,t.settings),t.settings=s}this._interpolantSettings=s,this._interpolants=o,this._propertyBindings=Array(a),this._cacheIndex=null,this._byClipCacheIndex=null,this._timeScaleInterpolant=null,this._weightInterpolant=null,this.loop=zr,this._loopCount=-1,this._startTime=null,this.time=0,this.timeScale=1,this._effectiveTimeScale=1,this.weight=1,this._effectiveWeight=1,this.repetitions=1/0,this.paused=!1,this.enabled=!0,this.clampWhenFinished=!1,this.zeroSlopeAtStart=!0,this.zeroSlopeAtEnd=!0}play(){return this._mixer._activateAction(this),this}stop(){return this._mixer._deactivateAction(this),this.reset()}reset(){return this.paused=!1,this.enabled=!0,this.time=0,this._loopCount=-1,this._startTime=null,this.stopFading().stopWarping()}isRunning(){return this.enabled&&!this.paused&&this.timeScale!==0&&this._startTime===null&&this._mixer._isActiveAction(this)}isScheduled(){return this._mixer._isActiveAction(this)}startAt(e){return this._startTime=e,this}setLoop(e,t){return this.loop=e,this.repetitions=t,this}setEffectiveWeight(e){return this.weight=e,this._effectiveWeight=this.enabled?e:0,this.stopFading()}getEffectiveWeight(){return this._effectiveWeight}fadeIn(e){return this._scheduleFading(e,0,1)}fadeOut(e){return this._scheduleFading(e,1,0)}crossFadeFrom(e,t,n=!1){if(e.fadeOut(t),this.fadeIn(t),n===!0){let n=this._clip.duration,r=e._clip.duration,i=r/n,a=n/r;e.warp(1,i,t),this.warp(a,1,t)}return this}crossFadeTo(e,t,n=!1){return e.crossFadeFrom(this,t,n)}stopFading(){let e=this._weightInterpolant;return e!==null&&(this._weightInterpolant=null,this._mixer._takeBackControlInterpolant(e)),this}setEffectiveTimeScale(e){return this.timeScale=e,this._effectiveTimeScale=this.paused?0:e,this.stopWarping()}getEffectiveTimeScale(){return this._effectiveTimeScale}setDuration(e){return this.timeScale=this._clip.duration/e,this.stopWarping()}syncWith(e){return this.time=e.time,this.timeScale=e.timeScale,this.stopWarping()}halt(e){return this.warp(this._effectiveTimeScale,0,e)}warp(e,t,n){let r=this._mixer,i=r.time,a=this.timeScale,o=this._timeScaleInterpolant;o===null&&(o=r._lendControlInterpolant(),this._timeScaleInterpolant=o);let s=o.parameterPositions,c=o.sampleValues;return s[0]=i,s[1]=i+n,c[0]=e/a,c[1]=t/a,this}stopWarping(){let e=this._timeScaleInterpolant;return e!==null&&(this._timeScaleInterpolant=null,this._mixer._takeBackControlInterpolant(e)),this}getMixer(){return this._mixer}getClip(){return this._clip}getRoot(){return this._localRoot||this._mixer._root}_update(e,t,n,r){if(!this.enabled){this._updateWeight(e);return}let i=this._startTime;if(i!==null){let r=(e-i)*n;r<0||n===0?t=0:(this._startTime=null,t=n*r)}t*=this._updateTimeScale(e);let a=this._updateTime(t),o=this._updateWeight(e);if(o>0){let e=this._interpolants,t=this._propertyBindings;switch(this.blendMode){case Yr:for(let n=0,r=e.length;n!==r;++n)e[n].evaluate(a),t[n].accumulateAdditive(o);break;case Jr:default:for(let n=0,i=e.length;n!==i;++n)e[n].evaluate(a),t[n].accumulate(r,o)}}}_updateWeight(e){let t=0;if(this.enabled){t=this.weight;let n=this._weightInterpolant;if(n!==null){let r=n.evaluate(e)[0];t*=r,e>n.parameterPositions[1]&&(this.stopFading(),r===0&&(this.enabled=!1))}}return this._effectiveWeight=t,t}_updateTimeScale(e){let t=0;if(!this.paused){t=this.timeScale;let n=this._timeScaleInterpolant;if(n!==null){let r=n.evaluate(e)[0];t*=r,e>n.parameterPositions[1]&&(this.stopWarping(),t===0?this.paused=!0:this.timeScale=t)}}return this._effectiveTimeScale=t,t}_updateTime(e){let t=this._clip.duration,n=this.loop,r=this.time+e,i=this._loopCount,a=n===Br;if(e===0)return i===-1?r:a&&(i&1)==1?t-r:r;if(n===2200){i===-1&&(this._loopCount=0,this._setEndings(!0,!0,!1));handle_stop:{if(r>=t)r=t;else if(r<0)r=0;else{this.time=r;break handle_stop}this.clampWhenFinished?this.paused=!0:this.enabled=!1,this.time=r,this._mixer.dispatchEvent({type:`finished`,action:this,direction:e<0?-1:1})}}else{if(i===-1&&(e>=0?(i=0,this._setEndings(!0,this.repetitions===0,a)):this._setEndings(this.repetitions===0,!0,a)),r>=t||r<0){let n=Math.floor(r/t);r-=t*n,i+=Math.abs(n);let o=this.repetitions-i;if(o<=0)this.clampWhenFinished?this.paused=!0:this.enabled=!1,r=e>0?t:0,this.time=r,this._mixer.dispatchEvent({type:`finished`,action:this,direction:e>0?1:-1});else{if(o===1){let t=e<0;this._setEndings(t,!t,a)}else this._setEndings(!1,!1,a);this._loopCount=i,this.time=r,this._mixer.dispatchEvent({type:`loop`,action:this,loopDelta:n})}}else this._loopCount=i,this.time=r;if(a&&(i&1)==1)return t-r}return r}_setEndings(e,t,n){let r=this._interpolantSettings;n?(r.endingStart=Kr,r.endingEnd=Kr):(e?r.endingStart=this.zeroSlopeAtStart?Kr:Gr:r.endingStart=qr,t?r.endingEnd=this.zeroSlopeAtEnd?Kr:Gr:r.endingEnd=qr)}_scheduleFading(e,t,n){let r=this._mixer,i=r.time,a=this._weightInterpolant;a===null&&(a=r._lendControlInterpolant(),this._weightInterpolant=a);let o=a.parameterPositions,s=a.sampleValues;return o[0]=i,s[0]=t,o[1]=i+e,s[1]=n,this}},Bc=new Float32Array(1),Vc=class extends ci{constructor(e){super(),this._root=e,this._initMemoryManager(),this._accuIndex=0,this.time=0,this.timeScale=1,typeof __THREE_DEVTOOLS__<`u`&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent(`observe`,{detail:this}))}_bindAction(e,t){let n=e._localRoot||this._root,r=e._clip.tracks,i=r.length,a=e._propertyBindings,o=e._interpolants,s=n.uuid,c=this._bindingsByRootAndName,l=c[s];l===void 0&&(l={},c[s]=l);for(let e=0;e!==i;++e){let i=r[e],c=i.name,u=l[c];if(u!==void 0)++u.referenceCount,a[e]=u;else{if(u=a[e],u!==void 0){u._cacheIndex===null&&(++u.referenceCount,this._addInactiveBinding(u,s,c));continue}let r=t&&t._propertyBindings[e].binding.parsedPath;u=new Ec(Rc.create(n,c,r),i.ValueTypeName,i.getValueSize()),++u.referenceCount,this._addInactiveBinding(u,s,c),a[e]=u}o[e].resultBuffer=u.buffer}}_activateAction(e){if(!this._isActiveAction(e)){if(e._cacheIndex===null){let t=(e._localRoot||this._root).uuid,n=e._clip.uuid,r=this._actionsByClip[n];this._bindAction(e,r&&r.knownActions[0]),this._addInactiveAction(e,n,t)}let t=e._propertyBindings;for(let e=0,n=t.length;e!==n;++e){let n=t[e];n.useCount++===0&&(this._lendBinding(n),n.saveOriginalState())}this._lendAction(e)}}_deactivateAction(e){if(this._isActiveAction(e)){let t=e._propertyBindings;for(let e=0,n=t.length;e!==n;++e){let n=t[e];--n.useCount===0&&(n.restoreOriginalState(),this._takeBackBinding(n))}this._takeBackAction(e)}}_initMemoryManager(){this._actions=[],this._nActiveActions=0,this._actionsByClip={},this._bindings=[],this._nActiveBindings=0,this._bindingsByRootAndName={},this._controlInterpolants=[],this._nActiveControlInterpolants=0;let e=this;this.stats={actions:{get total(){return e._actions.length},get inUse(){return e._nActiveActions}},bindings:{get total(){return e._bindings.length},get inUse(){return e._nActiveBindings}},controlInterpolants:{get total(){return e._controlInterpolants.length},get inUse(){return e._nActiveControlInterpolants}}}}_isActiveAction(e){let t=e._cacheIndex;return t!==null&&t<this._nActiveActions}_addInactiveAction(e,t,n){let r=this._actions,i=this._actionsByClip,a=i[t];if(a===void 0)a={knownActions:[e],actionByRoot:{}},e._byClipCacheIndex=0,i[t]=a;else{let t=a.knownActions;e._byClipCacheIndex=t.length,t.push(e)}e._cacheIndex=r.length,r.push(e),a.actionByRoot[n]=e}_removeInactiveAction(e){let t=this._actions,n=t[t.length-1],r=e._cacheIndex;n._cacheIndex=r,t[r]=n,t.pop(),e._cacheIndex=null;let i=e._clip.uuid,a=this._actionsByClip,o=a[i],s=o.knownActions,c=s[s.length-1],l=e._byClipCacheIndex;c._byClipCacheIndex=l,s[l]=c,s.pop(),e._byClipCacheIndex=null;let u=o.actionByRoot,d=(e._localRoot||this._root).uuid;delete u[d],s.length===0&&delete a[i],this._removeInactiveBindingsForAction(e)}_removeInactiveBindingsForAction(e){let t=e._propertyBindings;for(let e=0,n=t.length;e!==n;++e){let n=t[e];--n.referenceCount===0&&this._removeInactiveBinding(n)}}_lendAction(e){let t=this._actions,n=e._cacheIndex,r=this._nActiveActions++,i=t[r];e._cacheIndex=r,t[r]=e,i._cacheIndex=n,t[n]=i}_takeBackAction(e){let t=this._actions,n=e._cacheIndex,r=--this._nActiveActions,i=t[r];e._cacheIndex=r,t[r]=e,i._cacheIndex=n,t[n]=i}_addInactiveBinding(e,t,n){let r=this._bindingsByRootAndName,i=this._bindings,a=r[t];a===void 0&&(a={},r[t]=a),a[n]=e,e._cacheIndex=i.length,i.push(e)}_removeInactiveBinding(e){let t=this._bindings,n=e.binding,r=n.rootNode.uuid,i=n.path,a=this._bindingsByRootAndName,o=a[r],s=t[t.length-1],c=e._cacheIndex;s._cacheIndex=c,t[c]=s,t.pop(),delete o[i],Object.keys(o).length===0&&delete a[r]}_lendBinding(e){let t=this._bindings,n=e._cacheIndex,r=this._nActiveBindings++,i=t[r];e._cacheIndex=r,t[r]=e,i._cacheIndex=n,t[n]=i}_takeBackBinding(e){let t=this._bindings,n=e._cacheIndex,r=--this._nActiveBindings,i=t[r];e._cacheIndex=r,t[r]=e,i._cacheIndex=n,t[n]=i}_lendControlInterpolant(){let e=this._controlInterpolants,t=this._nActiveControlInterpolants++,n=e[t];return n===void 0&&(n=new zs(new Float32Array(2),new Float32Array(2),1,Bc),n.__cacheIndex=t,e[t]=n),n}_takeBackControlInterpolant(e){let t=this._controlInterpolants,n=e.__cacheIndex,r=--this._nActiveControlInterpolants,i=t[r];e.__cacheIndex=r,t[r]=e,i.__cacheIndex=n,t[n]=i}clipAction(e,t,n){let r=t||this._root,i=r.uuid,a=typeof e==`string`?Xs.findByName(r,e):e,o=a===null?e:a.uuid,s=this._actionsByClip[o],c=null;if(n===void 0&&(n=a===null?Jr:a.blendMode),s!==void 0){let e=s.actionByRoot[i];if(e!==void 0&&e.blendMode===n)return e;c=s.knownActions[0],a===null&&(a=c._clip)}if(a===null)return null;let l=new zc(this,a,t,n);return this._bindAction(l,c),this._addInactiveAction(l,o,i),l}existingAction(e,t){let n=t||this._root,r=n.uuid,i=typeof e==`string`?Xs.findByName(n,e):e,a=i?i.uuid:e,o=this._actionsByClip[a];return o===void 0?null:o.actionByRoot[r]||null}stopAllAction(){let e=this._actions,t=this._nActiveActions;for(let n=t-1;n>=0;--n)e[n].stop();return this}update(e){e*=this.timeScale;let t=this._actions,n=this._nActiveActions,r=this.time+=e,i=Math.sign(e),a=this._accuIndex^=1;for(let o=0;o!==n;++o)t[o]._update(r,e,i,a);let o=this._bindings,s=this._nActiveBindings;for(let e=0;e!==s;++e)o[e].apply(a);return this}setTime(e){this.time=0;for(let e=0;e<this._actions.length;e++)this._actions[e].time=0;return this.update(e)}getRoot(){return this._root}uncacheClip(e){let t=this._actions,n=e.uuid,r=this._actionsByClip,i=r[n];if(i!==void 0){let e=i.knownActions;for(let n=0,r=e.length;n!==r;++n){let r=e[n];this._deactivateAction(r);let i=r._cacheIndex,a=t[t.length-1];r._cacheIndex=null,r._byClipCacheIndex=null,a._cacheIndex=i,t[i]=a,t.pop(),this._removeInactiveBindingsForAction(r)}delete r[n]}}uncacheRoot(e){let t=e.uuid,n=this._actionsByClip;for(let e in n){let r=n[e].actionByRoot[t];r!==void 0&&(this._deactivateAction(r),this._removeInactiveAction(r))}let r=this._bindingsByRootAndName[t];if(r!==void 0)for(let e in r){let t=r[e];t.restoreOriginalState(),this._removeInactiveBinding(t)}}uncacheAction(e,t){let n=this.existingAction(e,t);n!==null&&(this._deactivateAction(n),this._removeInactiveAction(n))}},Hc=new Y,Uc=class{constructor(e,t,n=0,r=1/0){this.ray=new ho(e,t),this.near=n,this.far=r,this.camera=null,this.layers=new Vi,this.params={Mesh:{},Line:{threshold:1},LOD:{},Points:{threshold:1},Sprite:{}}}set(e,t){this.ray.set(e,t)}setFromCamera(e,t){t.isPerspectiveCamera?(this.ray.origin.setFromMatrixPosition(t.matrixWorld),this.ray.direction.set(e.x,e.y,.5).unproject(t).sub(this.ray.origin).normalize(),this.camera=t):t.isOrthographicCamera?(this.ray.origin.set(e.x,e.y,(t.near+t.far)/(t.near-t.far)).unproject(t),this.ray.direction.set(0,0,-1).transformDirection(t.matrixWorld),this.camera=t):G(`Raycaster: Unsupported camera type: `+t.type)}setFromXRController(e){return Hc.identity().extractRotation(e.matrixWorld),this.ray.origin.setFromMatrixPosition(e.matrixWorld),this.ray.direction.set(0,0,-1).applyMatrix4(Hc),this}intersectObject(e,t=!0,n=[]){return bn(e,this,n,t),n.sort(yn),n}intersectObjects(e,t=!0,n=[]){for(let r=0,i=e.length;r<i;r++)bn(e[r],this,n,t);return n.sort(yn),n}},class e{static{e.prototype.isMatrix2=!0}constructor(e,t,n,r){this.elements=[1,0,0,1],e!==void 0&&this.set(e,t,n,r)}identity(){return this.set(1,0,0,1),this}fromArray(e,t=0){for(let n=0;n<4;n++)this.elements[n]=e[n+t];return this}set(e,t,n,r){let i=this.elements;return i[0]=e,i[2]=t,i[1]=n,i[3]=r,this}},typeof __THREE_DEVTOOLS__<`u`&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent(`register`,{detail:{revision:`184`}})),typeof window<`u`&&(window.__THREE__?W(`WARNING: Multiple instances of Three.js being imported.`):window.__THREE__=`184`)}));function Gc(){let e=null,t=!1,n=null,r=null;function i(t,a){n(t,a),r=e.requestAnimationFrame(i)}return{start:function(){t!==!0&&n!==null&&e!==null&&(r=e.requestAnimationFrame(i),t=!0)},stop:function(){e!==null&&e.cancelAnimationFrame(r),t=!1},setAnimationLoop:function(e){n=e},setContext:function(t){e=t}}}function Kc(e){let t=new WeakMap;function n(t,n){let r=t.array,i=t.usage,a=r.byteLength,o=e.createBuffer();e.bindBuffer(n,o),e.bufferData(n,r,i),t.onUploadCallback();let s;if(r instanceof Float32Array)s=e.FLOAT;else if(typeof Float16Array<`u`&&r instanceof Float16Array)s=e.HALF_FLOAT;else if(r instanceof Uint16Array)s=t.isFloat16BufferAttribute?e.HALF_FLOAT:e.UNSIGNED_SHORT;else if(r instanceof Int16Array)s=e.SHORT;else if(r instanceof Uint32Array)s=e.UNSIGNED_INT;else if(r instanceof Int32Array)s=e.INT;else if(r instanceof Int8Array)s=e.BYTE;else if(r instanceof Uint8Array)s=e.UNSIGNED_BYTE;else if(r instanceof Uint8ClampedArray)s=e.UNSIGNED_BYTE;else throw Error(`THREE.WebGLAttributes: Unsupported buffer data format: `+r);return{buffer:o,type:s,bytesPerElement:r.BYTES_PER_ELEMENT,version:t.version,size:a}}function r(t,n,r){let i=n.array,a=n.updateRanges;if(e.bindBuffer(r,t),a.length===0)e.bufferSubData(r,0,i);else{a.sort((e,t)=>e.start-t.start);let t=0;for(let e=1;e<a.length;e++){let n=a[t],r=a[e];r.start<=n.start+n.count+1?n.count=Math.max(n.count,r.start+r.count-n.start):(++t,a[t]=r)}a.length=t+1;for(let t=0,n=a.length;t<n;t++){let n=a[t];e.bufferSubData(r,n.start*i.BYTES_PER_ELEMENT,i,n.start,n.count)}n.clearUpdateRanges()}n.onUploadCallback()}function i(e){return e.isInterleavedBufferAttribute&&(e=e.data),t.get(e)}function a(n){n.isInterleavedBufferAttribute&&(n=n.data);let r=t.get(n);r&&(e.deleteBuffer(r.buffer),t.delete(n))}function o(e,i){if(e.isInterleavedBufferAttribute&&(e=e.data),e.isGLBufferAttribute){let n=t.get(e);(!n||n.version<e.version)&&t.set(e,{buffer:e.buffer,type:e.type,bytesPerElement:e.elementSize,version:e.version});return}let a=t.get(e);if(a===void 0)t.set(e,n(e,i));else if(a.version<e.version){if(a.size!==e.array.byteLength)throw Error(`THREE.WebGLAttributes: The size of the buffer attribute's array buffer does not match the original size. Resizing buffer attributes is not supported.`);r(a.buffer,e,i),a.version=e.version}}return{get:i,remove:a,update:o}}function qc(e,t,n,r,i,a){let o=new X(0),s=i===!0?0:1,c,l,u=null,d=0,f=null;function p(e){let n=e.isScene===!0?e.background:null;if(n&&n.isTexture){let r=e.backgroundBlurriness>0;n=t.get(n,r)}return n}function m(t){let r=!1,i=p(t);i===null?g(o,s):i&&i.isColor&&(g(i,1),r=!0);let c=e.xr.getEnvironmentBlendMode();c===`additive`?n.buffers.color.setClear(0,0,0,1,a):c===`alpha-blend`&&n.buffers.color.setClear(0,0,0,0,a),(e.autoClear||r)&&(n.buffers.depth.setTest(!0),n.buffers.depth.setMask(!0),n.buffers.color.setMask(!0),e.clear(e.autoClearColor,e.autoClearDepth,e.autoClearStencil))}function h(t,n){let i=p(n);i&&(i.isCubeTexture||i.mapping===306)?(l===void 0&&(l=new Oo(new Es(1,1,1),new Ms({name:`BackgroundCubeMaterial`,uniforms:cn(Xu.backgroundCube.uniforms),vertexShader:Xu.backgroundCube.vertexShader,fragmentShader:Xu.backgroundCube.fragmentShader,side:1,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),l.geometry.deleteAttribute(`normal`),l.geometry.deleteAttribute(`uv`),l.onBeforeRender=function(e,t,n){this.matrixWorld.copyPosition(n.matrixWorld)},Object.defineProperty(l.material,"envMap",{get:function(){return this.uniforms.envMap.value}}),r.update(l)),l.material.uniforms.envMap.value=i,l.material.uniforms.backgroundBlurriness.value=n.backgroundBlurriness,l.material.uniforms.backgroundIntensity.value=n.backgroundIntensity,l.material.uniforms.backgroundRotation.value.setFromMatrix4(Qu.makeRotationFromEuler(n.backgroundRotation)).transpose(),i.isCubeTexture&&i.isRenderTargetTexture===!1&&l.material.uniforms.backgroundRotation.value.premultiply($u),l.material.toneMapped=J.getTransfer(i.colorSpace)!==ei,(u!==i||d!==i.version||f!==e.toneMapping)&&(l.material.needsUpdate=!0,u=i,d=i.version,f=e.toneMapping),l.layers.enableAll(),t.unshift(l,l.geometry,l.material,0,0,null)):i&&i.isTexture&&(c===void 0&&(c=new Oo(new Ds(2,2),new Ms({name:`BackgroundMaterial`,uniforms:cn(Xu.background.uniforms),vertexShader:Xu.background.vertexShader,fragmentShader:Xu.background.fragmentShader,side:0,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),c.geometry.deleteAttribute(`normal`),Object.defineProperty(c.material,"map",{get:function(){return this.uniforms.t2D.value}}),r.update(c)),c.material.uniforms.t2D.value=i,c.material.uniforms.backgroundIntensity.value=n.backgroundIntensity,c.material.toneMapped=J.getTransfer(i.colorSpace)!==ei,i.matrixAutoUpdate===!0&&i.updateMatrix(),c.material.uniforms.uvTransform.value.copy(i.matrix),(u!==i||d!==i.version||f!==e.toneMapping)&&(c.material.needsUpdate=!0,u=i,d=i.version,f=e.toneMapping),c.layers.enableAll(),t.unshift(c,c.geometry,c.material,0,0,null))}function g(t,r){t.getRGB(Zu,fn(e)),n.buffers.color.setClear(Zu.r,Zu.g,Zu.b,r,a)}function _(){l!==void 0&&(l.geometry.dispose(),l.material.dispose(),l=void 0),c!==void 0&&(c.geometry.dispose(),c.material.dispose(),c=void 0)}return{getClearColor:function(){return o},setClearColor:function(e,t=1){o.set(e),s=t,g(o,s)},getClearAlpha:function(){return s},setClearAlpha:function(e){s=e,g(o,s)},render:m,addToRenderList:h,dispose:_}}function Jc(e,t){let n=e.getParameter(e.MAX_VERTEX_ATTRIBS),r={},i=f(null),a=i,o=!1;function s(n,r,i,s,c){let u=!1,f=d(n,s,i,r);a!==f&&(a=f,l(a.object)),u=p(n,s,i,c),u&&m(n,s,i,c),c!==null&&t.update(c,e.ELEMENT_ARRAY_BUFFER),(u||o)&&(o=!1,b(n,r,i,s),c!==null&&e.bindBuffer(e.ELEMENT_ARRAY_BUFFER,t.get(c).buffer))}function c(){return e.createVertexArray()}function l(t){return e.bindVertexArray(t)}function u(t){return e.deleteVertexArray(t)}function d(e,t,n,i){let a=i.wireframe===!0,o=r[t.id];o===void 0&&(o={},r[t.id]=o);let s=e.isInstancedMesh===!0?e.id:0,l=o[s];l===void 0&&(l={},o[s]=l);let u=l[n.id];u===void 0&&(u={},l[n.id]=u);let d=u[a];return d===void 0&&(d=f(c()),u[a]=d),d}function f(e){let t=[],r=[],i=[];for(let e=0;e<n;e++)t[e]=0,r[e]=0,i[e]=0;return{geometry:null,program:null,wireframe:!1,newAttributes:t,enabledAttributes:r,attributeDivisors:i,object:e,attributes:{},index:null}}function p(e,t,n,r){let i=a.attributes,o=t.attributes,s=0,c=n.getAttributes();for(let t in c)if(c[t].location>=0){let n=i[t],r=o[t];if(r===void 0&&(t===`instanceMatrix`&&e.instanceMatrix&&(r=e.instanceMatrix),t===`instanceColor`&&e.instanceColor&&(r=e.instanceColor)),n===void 0||n.attribute!==r||r&&n.data!==r.data)return!0;s++}return a.attributesNum!==s||a.index!==r}function m(e,t,n,r){let i={},o=t.attributes,s=0,c=n.getAttributes();for(let t in c)if(c[t].location>=0){let n=o[t];n===void 0&&(t===`instanceMatrix`&&e.instanceMatrix&&(n=e.instanceMatrix),t===`instanceColor`&&e.instanceColor&&(n=e.instanceColor));let r={};r.attribute=n,n&&n.data&&(r.data=n.data),i[t]=r,s++}a.attributes=i,a.attributesNum=s,a.index=r}function h(){let e=a.newAttributes;for(let t=0,n=e.length;t<n;t++)e[t]=0}function g(e){_(e,0)}function _(t,n){let r=a.newAttributes,i=a.enabledAttributes,o=a.attributeDivisors;r[t]=1,i[t]===0&&(e.enableVertexAttribArray(t),i[t]=1),o[t]!==n&&(e.vertexAttribDivisor(t,n),o[t]=n)}function v(){let t=a.newAttributes,n=a.enabledAttributes;for(let r=0,i=n.length;r<i;r++)n[r]!==t[r]&&(e.disableVertexAttribArray(r),n[r]=0)}function y(t,n,r,i,a,o,s){s===!0?e.vertexAttribIPointer(t,n,r,a,o):e.vertexAttribPointer(t,n,r,i,a,o)}function b(n,r,i,a){h();let o=a.attributes,s=i.getAttributes(),c=r.defaultAttributeValues;for(let r in s){let i=s[r];if(i.location>=0){let s=o[r];if(s===void 0&&(r===`instanceMatrix`&&n.instanceMatrix&&(s=n.instanceMatrix),r===`instanceColor`&&n.instanceColor&&(s=n.instanceColor)),s!==void 0){let r=s.normalized,o=s.itemSize,c=t.get(s);if(c===void 0)continue;let l=c.buffer,u=c.type,d=c.bytesPerElement,f=u===e.INT||u===e.UNSIGNED_INT||s.gpuType===1013;if(s.isInterleavedBufferAttribute){let t=s.data,c=t.stride,p=s.offset;if(t.isInstancedInterleavedBuffer){for(let e=0;e<i.locationSize;e++)_(i.location+e,t.meshPerAttribute);n.isInstancedMesh!==!0&&a._maxInstanceCount===void 0&&(a._maxInstanceCount=t.meshPerAttribute*t.count)}else for(let e=0;e<i.locationSize;e++)g(i.location+e);e.bindBuffer(e.ARRAY_BUFFER,l);for(let e=0;e<i.locationSize;e++)y(i.location+e,o/i.locationSize,u,r,c*d,(p+o/i.locationSize*e)*d,f)}else{if(s.isInstancedBufferAttribute){for(let e=0;e<i.locationSize;e++)_(i.location+e,s.meshPerAttribute);n.isInstancedMesh!==!0&&a._maxInstanceCount===void 0&&(a._maxInstanceCount=s.meshPerAttribute*s.count)}else for(let e=0;e<i.locationSize;e++)g(i.location+e);e.bindBuffer(e.ARRAY_BUFFER,l);for(let e=0;e<i.locationSize;e++)y(i.location+e,o/i.locationSize,u,r,o*d,o/i.locationSize*e*d,f)}}else if(c!==void 0){let t=c[r];if(t!==void 0)switch(t.length){case 2:e.vertexAttrib2fv(i.location,t);break;case 3:e.vertexAttrib3fv(i.location,t);break;case 4:e.vertexAttrib4fv(i.location,t);break;default:e.vertexAttrib1fv(i.location,t)}}}}v()}function x(){T();for(let e in r){let t=r[e];for(let e in t){let n=t[e];for(let e in n){let t=n[e];for(let e in t)u(t[e].object),delete t[e];delete n[e]}}delete r[e]}}function S(e){if(r[e.id]===void 0)return;let t=r[e.id];for(let e in t){let n=t[e];for(let e in n){let t=n[e];for(let e in t)u(t[e].object),delete t[e];delete n[e]}}delete r[e.id]}function C(e){for(let t in r){let n=r[t];for(let t in n){let r=n[t];if(r[e.id]===void 0)continue;let i=r[e.id];for(let e in i)u(i[e].object),delete i[e];delete r[e.id]}}}function w(e){for(let t in r){let n=r[t],i=e.isInstancedMesh===!0?e.id:0,a=n[i];if(a!==void 0){for(let e in a){let t=a[e];for(let e in t)u(t[e].object),delete t[e];delete a[e]}delete n[i],Object.keys(n).length===0&&delete r[t]}}}function T(){E(),o=!0,a!==i&&(a=i,l(a.object))}function E(){i.geometry=null,i.program=null,i.wireframe=!1}return{setup:s,reset:T,resetDefaultState:E,dispose:x,releaseStatesOfGeometry:S,releaseStatesOfObject:w,releaseStatesOfProgram:C,initAttributes:h,enableAttribute:g,disableUnusedAttributes:v}}function Yc(e,t,n){let r;function i(e){r=e}function a(t,i){e.drawArrays(r,t,i),n.update(i,r,1)}function o(t,i,a){a!==0&&(e.drawArraysInstanced(r,t,i,a),n.update(i,r,a))}function s(e,i,a){if(a===0)return;t.get(`WEBGL_multi_draw`).multiDrawArraysWEBGL(r,e,0,i,0,a);let o=0;for(let e=0;e<a;e++)o+=i[e];n.update(o,r,1)}this.setMode=i,this.render=a,this.renderInstances=o,this.renderMultiDraw=s}function Xc(e,t,n,r){let i;function a(){if(i!==void 0)return i;if(t.has(`EXT_texture_filter_anisotropic`)===!0){let n=t.get(`EXT_texture_filter_anisotropic`);i=e.getParameter(n.MAX_TEXTURE_MAX_ANISOTROPY_EXT)}else i=0;return i}function o(t){return!(t!==1023&&r.convert(t)!==e.getParameter(e.IMPLEMENTATION_COLOR_READ_FORMAT))}function s(n){let i=n===1016&&(t.has(`EXT_color_buffer_half_float`)||t.has(`EXT_color_buffer_float`));return!(n!==1009&&r.convert(n)!==e.getParameter(e.IMPLEMENTATION_COLOR_READ_TYPE)&&n!==1015&&!i)}function c(t){if(t===`highp`){if(e.getShaderPrecisionFormat(e.VERTEX_SHADER,e.HIGH_FLOAT).precision>0&&e.getShaderPrecisionFormat(e.FRAGMENT_SHADER,e.HIGH_FLOAT).precision>0)return`highp`;t=`mediump`}return t===`mediump`&&e.getShaderPrecisionFormat(e.VERTEX_SHADER,e.MEDIUM_FLOAT).precision>0&&e.getShaderPrecisionFormat(e.FRAGMENT_SHADER,e.MEDIUM_FLOAT).precision>0?`mediump`:`lowp`}let l=n.precision===void 0?`highp`:n.precision,u=c(l);u!==l&&(W(`WebGLRenderer:`,l,`not supported, using`,u,`instead.`),l=u);let d=n.logarithmicDepthBuffer===!0,f=n.reversedDepthBuffer===!0&&t.has(`EXT_clip_control`);n.reversedDepthBuffer===!0&&f===!1&&W(`WebGLRenderer: Unable to use reversed depth buffer due to missing EXT_clip_control extension. Fallback to default depth buffer.`);let p=e.getParameter(e.MAX_TEXTURE_IMAGE_UNITS),m=e.getParameter(e.MAX_VERTEX_TEXTURE_IMAGE_UNITS),h=e.getParameter(e.MAX_TEXTURE_SIZE),g=e.getParameter(e.MAX_CUBE_MAP_TEXTURE_SIZE),_=e.getParameter(e.MAX_VERTEX_ATTRIBS),v=e.getParameter(e.MAX_VERTEX_UNIFORM_VECTORS),y=e.getParameter(e.MAX_VARYING_VECTORS),b=e.getParameter(e.MAX_FRAGMENT_UNIFORM_VECTORS),x=e.getParameter(e.MAX_SAMPLES),S=e.getParameter(e.SAMPLES);return{isWebGL2:!0,getMaxAnisotropy:a,getMaxPrecision:c,textureFormatReadable:o,textureTypeReadable:s,precision:l,logarithmicDepthBuffer:d,reversedDepthBuffer:f,maxTextures:p,maxVertexTextures:m,maxTextureSize:h,maxCubemapSize:g,maxAttributes:_,maxVertexUniforms:v,maxVaryings:y,maxFragmentUniforms:b,maxSamples:x,samples:S}}function Zc(e){let t=this,n=null,r=0,i=!1,a=!1,o=new $o,s=new q,c={value:null,needsUpdate:!1};this.uniform=c,this.numPlanes=0,this.numIntersection=0,this.init=function(e,t){let n=e.length!==0||t||r!==0||i;return i=t,r=e.length,n},this.beginShadows=function(){a=!0,u(null)},this.endShadows=function(){a=!1},this.setGlobalState=function(e,t){n=u(e,t,0)},this.setState=function(t,o,s){let d=t.clippingPlanes,f=t.clipIntersection,p=t.clipShadows,m=e.get(t);if(!i||d===null||d.length===0||a&&!p)a?u(null):l();else{let e=a?0:r,t=e*4,i=m.clippingState||null;c.value=i,i=u(d,o,t,s);for(let e=0;e!==t;++e)i[e]=n[e];m.clippingState=i,this.numIntersection=f?this.numPlanes:0,this.numPlanes+=e}};function l(){c.value!==n&&(c.value=n,c.needsUpdate=r>0),t.numPlanes=r,t.numIntersection=0}function u(e,n,r,i){let a=e===null?0:e.length,l=null;if(a!==0){if(l=c.value,i!==!0||l===null){let t=r+a*4,i=n.matrixWorldInverse;s.getNormalMatrix(i),(l===null||l.length<t)&&(l=new Float32Array(t));for(let t=0,n=r;t!==a;++t,n+=4)o.copy(e[t]).applyMatrix4(i,s),o.normal.toArray(l,n),l[n+3]=o.constant}c.value=l,c.needsUpdate=!0}return t.numPlanes=a,t.numIntersection=0,l}}function Qc(e){let t=[],n=[],r=[],i=e,a=e-ed+1+td.length;for(let o=0;o<a;o++){let a=2**i;t.push(a);let s=1/a;o>e-ed?s=td[o-e+ed-1]:o===0&&(s=0),n.push(s);let c=1/(a-2),l=-c,u=1+c,d=[l,l,u,l,u,u,l,l,u,u,l,u],f=new Float32Array(108),p=new Float32Array(72),m=new Float32Array(36);for(let e=0;e<6;e++){let t=e%3*2/3-1,n=e>2?0:-1,r=[t,n,0,t+2/3,n,0,t+2/3,n+1,0,t,n,0,t+2/3,n+1,0,t,n+1,0];f.set(r,18*e),p.set(d,12*e);let i=[e,e,e,e,e,e];m.set(i,6*e)}let h=new io;h.setAttribute(`position`,new Ua(f,3)),h.setAttribute(`uv`,new Ua(p,2)),h.setAttribute(`faceIndex`,new Ua(m,1)),r.push(new Oo(h,null)),i>ed&&i--}return{lodMeshes:r,sizeLods:t,sigmas:n}}function $c(e,t,n){let r=new Oi(e,t,n);return r.texture.mapping=306,r.texture.name=`PMREM.cubeUv`,r.scissorTest=!0,r}function el(e,t,n,r,i){e.viewport.set(t,n,r,i),e.scissor.set(t,n,r,i)}function tl(e,t,n){return new Ms({name:`PMREMGGXConvolution`,defines:{GGX_SAMPLES:rd,CUBEUV_TEXEL_WIDTH:1/t,CUBEUV_TEXEL_HEIGHT:1/n,CUBEUV_MAX_MIP:`${e}.0`},uniforms:{envMap:{value:null},roughness:{value:0},mipInt:{value:0}},vertexShader:al(),fragmentShader:`

			precision highp float;
			precision highp int;

			varying vec3 vOutputDirection;

			uniform sampler2D envMap;
			uniform float roughness;
			uniform float mipInt;

			#define ENVMAP_TYPE_CUBE_UV
			#include <cube_uv_reflection_fragment>

			#define PI 3.14159265359

			// Van der Corput radical inverse
			float radicalInverse_VdC(uint bits) {
				bits = (bits << 16u) | (bits >> 16u);
				bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
				bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
				bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
				bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
				return float(bits) * 2.3283064365386963e-10; // / 0x100000000
			}

			// Hammersley sequence
			vec2 hammersley(uint i, uint N) {
				return vec2(float(i) / float(N), radicalInverse_VdC(i));
			}

			// GGX VNDF importance sampling (Eric Heitz 2018)
			// "Sampling the GGX Distribution of Visible Normals"
			// https://jcgt.org/published/0007/04/01/
			vec3 importanceSampleGGX_VNDF(vec2 Xi, vec3 V, float roughness) {
				float alpha = roughness * roughness;

				// Section 4.1: Orthonormal basis
				vec3 T1 = vec3(1.0, 0.0, 0.0);
				vec3 T2 = cross(V, T1);

				// Section 4.2: Parameterization of projected area
				float r = sqrt(Xi.x);
				float phi = 2.0 * PI * Xi.y;
				float t1 = r * cos(phi);
				float t2 = r * sin(phi);
				float s = 0.5 * (1.0 + V.z);
				t2 = (1.0 - s) * sqrt(1.0 - t1 * t1) + s * t2;

				// Section 4.3: Reprojection onto hemisphere
				vec3 Nh = t1 * T1 + t2 * T2 + sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)) * V;

				// Section 3.4: Transform back to ellipsoid configuration
				return normalize(vec3(alpha * Nh.x, alpha * Nh.y, max(0.0, Nh.z)));
			}

			void main() {
				vec3 N = normalize(vOutputDirection);
				vec3 V = N; // Assume view direction equals normal for pre-filtering

				vec3 prefilteredColor = vec3(0.0);
				float totalWeight = 0.0;

				// For very low roughness, just sample the environment directly
				if (roughness < 0.001) {
					gl_FragColor = vec4(bilinearCubeUV(envMap, N, mipInt), 1.0);
					return;
				}

				// Tangent space basis for VNDF sampling
				vec3 up = abs(N.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
				vec3 tangent = normalize(cross(up, N));
				vec3 bitangent = cross(N, tangent);

				for(uint i = 0u; i < uint(GGX_SAMPLES); i++) {
					vec2 Xi = hammersley(i, uint(GGX_SAMPLES));

					// For PMREM, V = N, so in tangent space V is always (0, 0, 1)
					vec3 H_tangent = importanceSampleGGX_VNDF(Xi, vec3(0.0, 0.0, 1.0), roughness);

					// Transform H back to world space
					vec3 H = normalize(tangent * H_tangent.x + bitangent * H_tangent.y + N * H_tangent.z);
					vec3 L = normalize(2.0 * dot(V, H) * H - V);

					float NdotL = max(dot(N, L), 0.0);

					if(NdotL > 0.0) {
						// Sample environment at fixed mip level
						// VNDF importance sampling handles the distribution filtering
						vec3 sampleColor = bilinearCubeUV(envMap, L, mipInt);

						// Weight by NdotL for the split-sum approximation
						// VNDF PDF naturally accounts for the visible microfacet distribution
						prefilteredColor += sampleColor * NdotL;
						totalWeight += NdotL;
					}
				}

				if (totalWeight > 0.0) {
					prefilteredColor = prefilteredColor / totalWeight;
				}

				gl_FragColor = vec4(prefilteredColor, 1.0);
			}
		`,blending:0,depthTest:!1,depthWrite:!1})}function nl(e,t,n){let r=new Float32Array(nd),i=new K(0,1,0);return new Ms({name:`SphericalGaussianBlur`,defines:{n:nd,CUBEUV_TEXEL_WIDTH:1/t,CUBEUV_TEXEL_HEIGHT:1/n,CUBEUV_MAX_MIP:`${e}.0`},uniforms:{envMap:{value:null},samples:{value:1},weights:{value:r},latitudinal:{value:!1},dTheta:{value:0},mipInt:{value:0},poleAxis:{value:i}},vertexShader:al(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			varying vec3 vOutputDirection;

			uniform sampler2D envMap;
			uniform int samples;
			uniform float weights[ n ];
			uniform bool latitudinal;
			uniform float dTheta;
			uniform float mipInt;
			uniform vec3 poleAxis;

			#define ENVMAP_TYPE_CUBE_UV
			#include <cube_uv_reflection_fragment>

			vec3 getSample( float theta, vec3 axis ) {

				float cosTheta = cos( theta );
				// Rodrigues' axis-angle rotation
				vec3 sampleDirection = vOutputDirection * cosTheta
					+ cross( axis, vOutputDirection ) * sin( theta )
					+ axis * dot( axis, vOutputDirection ) * ( 1.0 - cosTheta );

				return bilinearCubeUV( envMap, sampleDirection, mipInt );

			}

			void main() {

				vec3 axis = latitudinal ? poleAxis : cross( poleAxis, vOutputDirection );

				if ( all( equal( axis, vec3( 0.0 ) ) ) ) {

					axis = vec3( vOutputDirection.z, 0.0, - vOutputDirection.x );

				}

				axis = normalize( axis );

				gl_FragColor = vec4( 0.0, 0.0, 0.0, 1.0 );
				gl_FragColor.rgb += weights[ 0 ] * getSample( 0.0, axis );

				for ( int i = 1; i < n; i++ ) {

					if ( i >= samples ) {

						break;

					}

					float theta = dTheta * float( i );
					gl_FragColor.rgb += weights[ i ] * getSample( -1.0 * theta, axis );
					gl_FragColor.rgb += weights[ i ] * getSample( theta, axis );

				}

			}
		`,blending:0,depthTest:!1,depthWrite:!1})}function rl(){return new Ms({name:`EquirectangularToCubeUV`,uniforms:{envMap:{value:null}},vertexShader:al(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			varying vec3 vOutputDirection;

			uniform sampler2D envMap;

			#include <common>

			void main() {

				vec3 outputDirection = normalize( vOutputDirection );
				vec2 uv = equirectUv( outputDirection );

				gl_FragColor = vec4( texture2D ( envMap, uv ).rgb, 1.0 );

			}
		`,blending:0,depthTest:!1,depthWrite:!1})}function il(){return new Ms({name:`CubemapToCubeUV`,uniforms:{envMap:{value:null},flipEnvMap:{value:-1}},vertexShader:al(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			uniform float flipEnvMap;

			varying vec3 vOutputDirection;

			uniform samplerCube envMap;

			void main() {

				gl_FragColor = textureCube( envMap, vec3( flipEnvMap * vOutputDirection.x, vOutputDirection.yz ) );

			}
		`,blending:0,depthTest:!1,depthWrite:!1})}function al(){return`

		precision mediump float;
		precision mediump int;

		attribute float faceIndex;

		varying vec3 vOutputDirection;

		// RH coordinate system; PMREM face-indexing convention
		vec3 getDirection( vec2 uv, float face ) {

			uv = 2.0 * uv - 1.0;

			vec3 direction = vec3( uv, 1.0 );

			if ( face == 0.0 ) {

				direction = direction.zyx; // ( 1, v, u ) pos x

			} else if ( face == 1.0 ) {

				direction = direction.xzy;
				direction.xz *= -1.0; // ( -u, 1, -v ) pos y

			} else if ( face == 2.0 ) {

				direction.x *= -1.0; // ( -u, v, 1 ) pos z

			} else if ( face == 3.0 ) {

				direction = direction.zyx;
				direction.xz *= -1.0; // ( -1, v, -u ) neg x

			} else if ( face == 4.0 ) {

				direction = direction.xzy;
				direction.xy *= -1.0; // ( -u, -1, v ) neg y

			} else if ( face == 5.0 ) {

				direction.z *= -1.0; // ( u, v, -1 ) neg z

			}

			return direction;

		}

		void main() {

			vOutputDirection = getDirection( uv, faceIndex );
			gl_Position = vec4( position, 1.0 );

		}
	`}function ol(e){let t=new WeakMap,n=new WeakMap,r=null;function i(e,t=!1){return e==null?null:t?o(e):a(e)}function a(n){if(n&&n.isTexture){let r=n.mapping;if(r===303||r===304)if(t.has(n)){let e=t.get(n).texture;return s(e,n.mapping)}else{let r=n.image;if(r&&r.height>0){let i=new fd(r.height);return i.fromEquirectangularTexture(e,n),t.set(n,i),n.addEventListener(`dispose`,l),s(i.texture,n.mapping)}else return null}}return n}function o(t){if(t&&t.isTexture){let i=t.mapping,a=i===303||i===304,o=i===301||i===302;if(a||o){let i=n.get(t),s=i===void 0?0:i.texture.pmremVersion;if(t.isRenderTargetTexture&&t.pmremVersion!==s)return r===null&&(r=new dd(e)),i=a?r.fromEquirectangular(t,i):r.fromCubemap(t,i),i.texture.pmremVersion=t.pmremVersion,n.set(t,i),i.texture;if(i!==void 0)return i.texture;{let s=t.image;return a&&s&&s.height>0||o&&s&&c(s)?(r===null&&(r=new dd(e)),i=a?r.fromEquirectangular(t):r.fromCubemap(t),i.texture.pmremVersion=t.pmremVersion,n.set(t,i),t.addEventListener(`dispose`,u),i.texture):null}}}return t}function s(e,t){return t===303?e.mapping=301:t===304&&(e.mapping=302),e}function c(e){let t=0;for(let n=0;n<6;n++)e[n]!==void 0&&t++;return t===6}function l(e){let n=e.target;n.removeEventListener(`dispose`,l);let r=t.get(n);r!==void 0&&(t.delete(n),r.dispose())}function u(e){let t=e.target;t.removeEventListener(`dispose`,u);let r=n.get(t);r!==void 0&&(n.delete(t),r.dispose())}function d(){t=new WeakMap,n=new WeakMap,r!==null&&(r.dispose(),r=null)}return{get:i,dispose:d}}function sl(e){let t={};function n(n){if(t[n]!==void 0)return t[n];let r=e.getExtension(n);return t[n]=r,r}return{has:function(e){return n(e)!==null},init:function(){n(`EXT_color_buffer_float`),n(`WEBGL_clip_cull_distance`),n(`OES_texture_float_linear`),n(`EXT_color_buffer_half_float`),n(`WEBGL_multisampled_render_to_texture`),n(`WEBGL_render_shared_exponent`)},get:function(e){let t=n(e);return t===null&&Ut(`WebGLRenderer: `+e+` extension not supported.`),t}}}function cl(e,t,n,r){let i={},a=new WeakMap;function o(e){let s=e.target;s.index!==null&&t.remove(s.index);for(let e in s.attributes)t.remove(s.attributes[e]);s.removeEventListener(`dispose`,o),delete i[s.id];let c=a.get(s);c&&(t.remove(c),a.delete(s)),r.releaseStatesOfGeometry(s),s.isInstancedBufferGeometry===!0&&delete s._maxInstanceCount,n.memory.geometries--}function s(e,t){return i[t.id]===!0?t:(t.addEventListener(`dispose`,o),i[t.id]=!0,n.memory.geometries++,t)}function c(n){let r=n.attributes;for(let n in r)t.update(r[n],e.ARRAY_BUFFER)}function l(e){let n=[],r=e.index,i=e.attributes.position,o=0;if(i===void 0)return;if(r!==null){let e=r.array;o=r.version;for(let t=0,r=e.length;t<r;t+=3){let r=e[t+0],i=e[t+1],a=e[t+2];n.push(r,i,i,a,a,r)}}else{let e=i.array;o=i.version;for(let t=0,r=e.length/3-1;t<r;t+=3){let e=t+0,r=t+1,i=t+2;n.push(e,r,r,i,i,e)}}let s=new(i.count>=65535?Ga:Wa)(n,1);s.version=o;let c=a.get(e);c&&t.remove(c),a.set(e,s)}function u(e){let t=a.get(e);if(t){let n=e.index;n!==null&&t.version<n.version&&l(e)}else l(e);return a.get(e)}return{get:s,update:c,getWireframeAttribute:u}}function ll(e,t,n){let r;function i(e){r=e}let a,o;function s(e){a=e.type,o=e.bytesPerElement}function c(t,i){e.drawElements(r,i,a,t*o),n.update(i,r,1)}function l(t,i,s){s!==0&&(e.drawElementsInstanced(r,i,a,t*o,s),n.update(i,r,s))}function u(e,i,o){if(o===0)return;t.get(`WEBGL_multi_draw`).multiDrawElementsWEBGL(r,i,0,a,e,0,o);let s=0;for(let e=0;e<o;e++)s+=i[e];n.update(s,r,1)}this.setMode=i,this.setIndex=s,this.render=c,this.renderInstances=l,this.renderMultiDraw=u}function ul(e){let t={geometries:0,textures:0},n={frame:0,calls:0,triangles:0,points:0,lines:0};function r(t,r,i){switch(n.calls++,r){case e.TRIANGLES:n.triangles+=t/3*i;break;case e.LINES:n.lines+=t/2*i;break;case e.LINE_STRIP:n.lines+=i*(t-1);break;case e.LINE_LOOP:n.lines+=i*t;break;case e.POINTS:n.points+=i*t;break;default:G(`WebGLInfo: Unknown draw mode:`,r);break}}function i(){n.calls=0,n.triangles=0,n.points=0,n.lines=0}return{memory:t,render:n,programs:null,autoReset:!0,reset:i,update:r}}function dl(e,t,n){let r=new WeakMap,i=new Ei;function a(a,o,s){let c=a.morphTargetInfluences,l=o.morphAttributes.position||o.morphAttributes.normal||o.morphAttributes.color,u=l===void 0?0:l.length,d=r.get(o);if(d===void 0||d.count!==u){d!==void 0&&d.texture.dispose();let e=o.morphAttributes.position!==void 0,n=o.morphAttributes.normal!==void 0,a=o.morphAttributes.color!==void 0,s=o.morphAttributes.position||[],c=o.morphAttributes.normal||[],l=o.morphAttributes.color||[],p=0;e===!0&&(p=1),n===!0&&(p=2),a===!0&&(p=3);let m=o.attributes.position.count*p,h=1;m>t.maxTextureSize&&(h=Math.ceil(m/t.maxTextureSize),m=t.maxTextureSize);let g=new Float32Array(m*h*4*u),_=new ki(g,m,h,u);_.type=zn,_.needsUpdate=!0;let v=p*4;for(let t=0;t<u;t++){let r=s[t],o=c[t],u=l[t],d=m*h*4*t;for(let t=0;t<r.count;t++){let s=t*v;e===!0&&(i.fromBufferAttribute(r,t),g[d+s+0]=i.x,g[d+s+1]=i.y,g[d+s+2]=i.z,g[d+s+3]=0),n===!0&&(i.fromBufferAttribute(o,t),g[d+s+4]=i.x,g[d+s+5]=i.y,g[d+s+6]=i.z,g[d+s+7]=0),a===!0&&(i.fromBufferAttribute(u,t),g[d+s+8]=i.x,g[d+s+9]=i.y,g[d+s+10]=i.z,g[d+s+11]=u.itemSize===4?i.w:1)}}d={count:u,texture:_,size:new fi(m,h)},r.set(o,d);function f(){_.dispose(),r.delete(o),o.removeEventListener(`dispose`,f)}o.addEventListener(`dispose`,f)}if(a.isInstancedMesh===!0&&a.morphTexture!==null)s.getUniforms().setValue(e,`morphTexture`,a.morphTexture,n);else{let t=0;for(let e=0;e<c.length;e++)t+=c[e];let n=o.morphTargetsRelative?1:1-t;s.getUniforms().setValue(e,`morphTargetBaseInfluence`,n),s.getUniforms().setValue(e,`morphTargetInfluences`,c)}s.getUniforms().setValue(e,`morphTargetsTexture`,d.texture,n),s.getUniforms().setValue(e,`morphTargetsTextureSize`,d.size)}return{update:a}}function fl(e,t,n,r,i){let a=new WeakMap;function o(r){let o=i.render.frame,s=r.geometry,l=t.get(r,s);if(a.get(l)!==o&&(t.update(l),a.set(l,o)),r.isInstancedMesh&&(r.hasEventListener(`dispose`,c)===!1&&r.addEventListener(`dispose`,c),a.get(r)!==o&&(n.update(r.instanceMatrix,e.ARRAY_BUFFER),r.instanceColor!==null&&n.update(r.instanceColor,e.ARRAY_BUFFER),a.set(r,o))),r.isSkinnedMesh){let e=r.skeleton;a.get(e)!==o&&(e.update(),a.set(e,o))}return l}function s(){a=new WeakMap}function c(e){let t=e.target;t.removeEventListener(`dispose`,c),r.releaseStatesOfObject(t),n.remove(t.instanceMatrix),t.instanceColor!==null&&n.remove(t.instanceColor)}return{update:o,dispose:s}}function pl(e,t,n,r,i){let a=new Oi(t,n,{type:e,depthBuffer:r,stencilBuffer:i,depthTexture:r?new Cs(t,n):void 0}),o=new Oi(t,n,{type:Bn,depthBuffer:!1,stencilBuffer:!1}),s=new io;s.setAttribute(`position`,new Ka([-1,3,0,-1,-1,0,3,-1,0],3)),s.setAttribute(`uv`,new Ka([0,2,0,0,2,0],2));let c=new Ns({uniforms:{tDiffuse:{value:null}},vertexShader:`
			precision highp float;

			uniform mat4 modelViewMatrix;
			uniform mat4 projectionMatrix;

			attribute vec3 position;
			attribute vec2 uv;

			varying vec2 vUv;

			void main() {
				vUv = uv;
				gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
			}`,fragmentShader:`
			precision highp float;

			uniform sampler2D tDiffuse;

			varying vec2 vUv;

			#include <tonemapping_pars_fragment>
			#include <colorspace_pars_fragment>

			void main() {
				gl_FragColor = texture2D( tDiffuse, vUv );

				#ifdef LINEAR_TONE_MAPPING
					gl_FragColor.rgb = LinearToneMapping( gl_FragColor.rgb );
				#elif defined( REINHARD_TONE_MAPPING )
					gl_FragColor.rgb = ReinhardToneMapping( gl_FragColor.rgb );
				#elif defined( CINEON_TONE_MAPPING )
					gl_FragColor.rgb = CineonToneMapping( gl_FragColor.rgb );
				#elif defined( ACES_FILMIC_TONE_MAPPING )
					gl_FragColor.rgb = ACESFilmicToneMapping( gl_FragColor.rgb );
				#elif defined( AGX_TONE_MAPPING )
					gl_FragColor.rgb = AgXToneMapping( gl_FragColor.rgb );
				#elif defined( NEUTRAL_TONE_MAPPING )
					gl_FragColor.rgb = NeutralToneMapping( gl_FragColor.rgb );
				#elif defined( CUSTOM_TONE_MAPPING )
					gl_FragColor.rgb = CustomToneMapping( gl_FragColor.rgb );
				#endif

				#ifdef SRGB_TRANSFER
					gl_FragColor = sRGBTransferOETF( gl_FragColor );
				#endif
			}`,depthTest:!1,depthWrite:!1}),l=new Oo(s,c),u=new vc(-1,1,1,-1,0,1),d=null,f=null,p=!1,m,h=null,g=[],_=!1;this.setSize=function(e,t){a.setSize(e,t),o.setSize(e,t);for(let n=0;n<g.length;n++){let r=g[n];r.setSize&&r.setSize(e,t)}},this.setEffects=function(e){g=e,_=g.length>0&&g[0].isRenderPass===!0;let t=a.width,n=a.height;for(let e=0;e<g.length;e++){let r=g[e];r.setSize&&r.setSize(t,n)}},this.begin=function(e,t){if(p||e.toneMapping===0&&g.length===0)return!1;if(h=t,t!==null){let e=t.width,n=t.height;(a.width!==e||a.height!==n)&&this.setSize(e,n)}return _===!1&&e.setRenderTarget(a),m=e.toneMapping,e.toneMapping=0,!0},this.hasRenderPass=function(){return _},this.end=function(e,t){e.toneMapping=m,p=!0;let n=a,r=o;for(let i=0;i<g.length;i++){let a=g[i];if(a.enabled!==!1&&(a.render(e,r,n,t),a.needsSwap!==!1)){let e=n;n=r,r=e}}if(d!==e.outputColorSpace||f!==e.toneMapping){d=e.outputColorSpace,f=e.toneMapping,c.defines={},J.getTransfer(d)===`srgb`&&(c.defines.SRGB_TRANSFER=``);let t=pd[f];t&&(c.defines[t]=``),c.needsUpdate=!0}c.uniforms.tDiffuse.value=n.texture,e.setRenderTarget(h),e.render(l,u),h=null,p=!1},this.isCompositing=function(){return p},this.dispose=function(){a.depthTexture&&a.depthTexture.dispose(),a.dispose(),o.dispose(),s.dispose(),c.dispose()}}function ml(e,t,n){let r=e[0];if(r<=0||r>0)return e;let i=t*n,a=yd[i];if(a===void 0&&(a=new Float32Array(i),yd[i]=a),t!==0){r.toArray(a,0);for(let r=1,i=0;r!==t;++r)i+=n,e[r].toArray(a,i)}return a}function hl(e,t){if(e.length!==t.length)return!1;for(let n=0,r=e.length;n<r;n++)if(e[n]!==t[n])return!1;return!0}function gl(e,t){for(let n=0,r=t.length;n<r;n++)e[n]=t[n]}function _l(e,t){let n=bd[t];n===void 0&&(n=new Int32Array(t),bd[t]=n);for(let r=0;r!==t;++r)n[r]=e.allocateTextureUnit();return n}function vl(e,t){let n=this.cache;n[0]!==t&&(e.uniform1f(this.addr,t),n[0]=t)}function yl(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y)&&(e.uniform2f(this.addr,t.x,t.y),n[0]=t.x,n[1]=t.y);else{if(hl(n,t))return;e.uniform2fv(this.addr,t),gl(n,t)}}function bl(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y||n[2]!==t.z)&&(e.uniform3f(this.addr,t.x,t.y,t.z),n[0]=t.x,n[1]=t.y,n[2]=t.z);else if(t.r!==void 0)(n[0]!==t.r||n[1]!==t.g||n[2]!==t.b)&&(e.uniform3f(this.addr,t.r,t.g,t.b),n[0]=t.r,n[1]=t.g,n[2]=t.b);else{if(hl(n,t))return;e.uniform3fv(this.addr,t),gl(n,t)}}function xl(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y||n[2]!==t.z||n[3]!==t.w)&&(e.uniform4f(this.addr,t.x,t.y,t.z,t.w),n[0]=t.x,n[1]=t.y,n[2]=t.z,n[3]=t.w);else{if(hl(n,t))return;e.uniform4fv(this.addr,t),gl(n,t)}}function Sl(e,t){let n=this.cache,r=t.elements;if(r===void 0){if(hl(n,t))return;e.uniformMatrix2fv(this.addr,!1,t),gl(n,t)}else{if(hl(n,r))return;Cd.set(r),e.uniformMatrix2fv(this.addr,!1,Cd),gl(n,r)}}function Cl(e,t){let n=this.cache,r=t.elements;if(r===void 0){if(hl(n,t))return;e.uniformMatrix3fv(this.addr,!1,t),gl(n,t)}else{if(hl(n,r))return;Sd.set(r),e.uniformMatrix3fv(this.addr,!1,Sd),gl(n,r)}}function wl(e,t){let n=this.cache,r=t.elements;if(r===void 0){if(hl(n,t))return;e.uniformMatrix4fv(this.addr,!1,t),gl(n,t)}else{if(hl(n,r))return;xd.set(r),e.uniformMatrix4fv(this.addr,!1,xd),gl(n,r)}}function Tl(e,t){let n=this.cache;n[0]!==t&&(e.uniform1i(this.addr,t),n[0]=t)}function El(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y)&&(e.uniform2i(this.addr,t.x,t.y),n[0]=t.x,n[1]=t.y);else{if(hl(n,t))return;e.uniform2iv(this.addr,t),gl(n,t)}}function Dl(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y||n[2]!==t.z)&&(e.uniform3i(this.addr,t.x,t.y,t.z),n[0]=t.x,n[1]=t.y,n[2]=t.z);else{if(hl(n,t))return;e.uniform3iv(this.addr,t),gl(n,t)}}function Ol(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y||n[2]!==t.z||n[3]!==t.w)&&(e.uniform4i(this.addr,t.x,t.y,t.z,t.w),n[0]=t.x,n[1]=t.y,n[2]=t.z,n[3]=t.w);else{if(hl(n,t))return;e.uniform4iv(this.addr,t),gl(n,t)}}function kl(e,t){let n=this.cache;n[0]!==t&&(e.uniform1ui(this.addr,t),n[0]=t)}function Al(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y)&&(e.uniform2ui(this.addr,t.x,t.y),n[0]=t.x,n[1]=t.y);else{if(hl(n,t))return;e.uniform2uiv(this.addr,t),gl(n,t)}}function jl(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y||n[2]!==t.z)&&(e.uniform3ui(this.addr,t.x,t.y,t.z),n[0]=t.x,n[1]=t.y,n[2]=t.z);else{if(hl(n,t))return;e.uniform3uiv(this.addr,t),gl(n,t)}}function Ml(e,t){let n=this.cache;if(t.x!==void 0)(n[0]!==t.x||n[1]!==t.y||n[2]!==t.z||n[3]!==t.w)&&(e.uniform4ui(this.addr,t.x,t.y,t.z,t.w),n[0]=t.x,n[1]=t.y,n[2]=t.z,n[3]=t.w);else{if(hl(n,t))return;e.uniform4uiv(this.addr,t),gl(n,t)}}function Nl(e,t,n){let r=this.cache,i=n.allocateTextureUnit();r[0]!==i&&(e.uniform1i(this.addr,i),r[0]=i);let a;this.type===e.SAMPLER_2D_SHADOW?(hd.compareFunction=n.isReversedDepthBuffer()?518:515,a=hd):a=md,n.setTexture2D(t||a,i)}function Pl(e,t,n){let r=this.cache,i=n.allocateTextureUnit();r[0]!==i&&(e.uniform1i(this.addr,i),r[0]=i),n.setTexture3D(t||_d,i)}function Fl(e,t,n){let r=this.cache,i=n.allocateTextureUnit();r[0]!==i&&(e.uniform1i(this.addr,i),r[0]=i),n.setTextureCube(t||vd,i)}function Il(e,t,n){let r=this.cache,i=n.allocateTextureUnit();r[0]!==i&&(e.uniform1i(this.addr,i),r[0]=i),n.setTexture2DArray(t||gd,i)}function Ll(e){switch(e){case 5126:return vl;case 35664:return yl;case 35665:return bl;case 35666:return xl;case 35674:return Sl;case 35675:return Cl;case 35676:return wl;case 5124:case 35670:return Tl;case 35667:case 35671:return El;case 35668:case 35672:return Dl;case 35669:case 35673:return Ol;case 5125:return kl;case 36294:return Al;case 36295:return jl;case 36296:return Ml;case 35678:case 36198:case 36298:case 36306:case 35682:return Nl;case 35679:case 36299:case 36307:return Pl;case 35680:case 36300:case 36308:case 36293:return Fl;case 36289:case 36303:case 36311:case 36292:return Il}}function Rl(e,t){e.uniform1fv(this.addr,t)}function zl(e,t){let n=ml(t,this.size,2);e.uniform2fv(this.addr,n)}function Bl(e,t){let n=ml(t,this.size,3);e.uniform3fv(this.addr,n)}function Vl(e,t){let n=ml(t,this.size,4);e.uniform4fv(this.addr,n)}function Hl(e,t){let n=ml(t,this.size,4);e.uniformMatrix2fv(this.addr,!1,n)}function Ul(e,t){let n=ml(t,this.size,9);e.uniformMatrix3fv(this.addr,!1,n)}function Wl(e,t){let n=ml(t,this.size,16);e.uniformMatrix4fv(this.addr,!1,n)}function Gl(e,t){e.uniform1iv(this.addr,t)}function Kl(e,t){e.uniform2iv(this.addr,t)}function ql(e,t){e.uniform3iv(this.addr,t)}function Jl(e,t){e.uniform4iv(this.addr,t)}function Yl(e,t){e.uniform1uiv(this.addr,t)}function Xl(e,t){e.uniform2uiv(this.addr,t)}function Zl(e,t){e.uniform3uiv(this.addr,t)}function Ql(e,t){e.uniform4uiv(this.addr,t)}function $l(e,t,n){let r=this.cache,i=t.length,a=_l(n,i);hl(r,a)||(e.uniform1iv(this.addr,a),gl(r,a));let o;o=this.type===e.SAMPLER_2D_SHADOW?hd:md;for(let e=0;e!==i;++e)n.setTexture2D(t[e]||o,a[e])}function eu(e,t,n){let r=this.cache,i=t.length,a=_l(n,i);hl(r,a)||(e.uniform1iv(this.addr,a),gl(r,a));for(let e=0;e!==i;++e)n.setTexture3D(t[e]||_d,a[e])}function tu(e,t,n){let r=this.cache,i=t.length,a=_l(n,i);hl(r,a)||(e.uniform1iv(this.addr,a),gl(r,a));for(let e=0;e!==i;++e)n.setTextureCube(t[e]||vd,a[e])}function nu(e,t,n){let r=this.cache,i=t.length,a=_l(n,i);hl(r,a)||(e.uniform1iv(this.addr,a),gl(r,a));for(let e=0;e!==i;++e)n.setTexture2DArray(t[e]||gd,a[e])}function ru(e){switch(e){case 5126:return Rl;case 35664:return zl;case 35665:return Bl;case 35666:return Vl;case 35674:return Hl;case 35675:return Ul;case 35676:return Wl;case 5124:case 35670:return Gl;case 35667:case 35671:return Kl;case 35668:case 35672:return ql;case 35669:case 35673:return Jl;case 5125:return Yl;case 36294:return Xl;case 36295:return Zl;case 36296:return Ql;case 35678:case 36198:case 36298:case 36306:case 35682:return $l;case 35679:case 36299:case 36307:return eu;case 35680:case 36300:case 36308:case 36293:return tu;case 36289:case 36303:case 36311:case 36292:return nu}}function iu(e,t){e.seq.push(t),e.map[t.id]=t}function au(e,t,n){let r=e.name,i=r.length;for(Dd.lastIndex=0;;){let a=Dd.exec(r),o=Dd.lastIndex,s=a[1],c=a[2]===`]`,l=a[3];if(c&&(s|=0),l===void 0||l===`[`&&o+2===i){iu(n,l===void 0?new wd(s,e,t):new Td(s,e,t));break}else{let e=n.map[s];e===void 0&&(e=new Ed(s),iu(n,e)),n=e}}}function ou(e,t,n){let r=e.createShader(t);return e.shaderSource(r,n),e.compileShader(r),r}function su(e,t){let n=e.split(`
`),r=[],i=Math.max(t-6,0),a=Math.min(t+6,n.length);for(let e=i;e<a;e++){let i=e+1;r.push(`${i===t?`>`:` `} ${i}: ${n[e]}`)}return r.join(`
`)}function cu(e){J._getMatrix(jd,J.workingColorSpace,e);let t=`mat3( ${jd.elements.map(e=>e.toFixed(4))} )`;switch(J.getTransfer(e)){case $r:return[t,`LinearTransferOETF`];case ei:return[t,`sRGBTransferOETF`];default:return W(`WebGLProgram: Unsupported color space: `,e),[t,`LinearTransferOETF`]}}function lu(e,t,n){let r=e.getShaderParameter(t,e.COMPILE_STATUS),i=(e.getShaderInfoLog(t)||``).trim();if(r&&i===``)return``;let a=/ERROR: 0:(\d+)/.exec(i);if(a){let r=parseInt(a[1]);return n.toUpperCase()+`

`+i+`

`+su(e.getShaderSource(t),r)}else return i}function uu(e,t){let n=cu(t);return[`vec4 ${e}( vec4 value ) {`,`	return ${n[1]}( vec4( value.rgb * ${n[0]}, value.a ) );`,`}`].join(`
`)}function du(e,t){let n=Md[t];return n===void 0?(W(`WebGLProgram: Unsupported toneMapping:`,t),`vec3 `+e+`( vec3 color ) { return LinearToneMapping( color ); }`):`vec3 `+e+`( vec3 color ) { return `+n+`ToneMapping( color ); }`}function fu(){return J.getLuminanceCoefficients(Nd),[`float luminance( const in vec3 rgb ) {`,`	const vec3 weights = vec3( ${Nd.x.toFixed(4)}, ${Nd.y.toFixed(4)}, ${Nd.z.toFixed(4)} );`,`	return dot( weights, rgb );`,`}`].join(`
`)}function pu(e){return[e.extensionClipCullDistance?`#extension GL_ANGLE_clip_cull_distance : require`:``,e.extensionMultiDraw?`#extension GL_ANGLE_multi_draw : require`:``].filter(gu).join(`
`)}function mu(e){let t=[];for(let n in e){let r=e[n];r!==!1&&t.push(`#define `+n+` `+r)}return t.join(`
`)}function hu(e,t){let n={},r=e.getProgramParameter(t,e.ACTIVE_ATTRIBUTES);for(let i=0;i<r;i++){let r=e.getActiveAttrib(t,i),a=r.name,o=1;r.type===e.FLOAT_MAT2&&(o=2),r.type===e.FLOAT_MAT3&&(o=3),r.type===e.FLOAT_MAT4&&(o=4),n[a]={type:r.type,location:e.getAttribLocation(t,a),locationSize:o}}return n}function gu(e){return e!==``}function _u(e,t){let n=t.numSpotLightShadows+t.numSpotLightMaps-t.numSpotLightShadowsWithMaps;return e.replace(/NUM_DIR_LIGHTS/g,t.numDirLights).replace(/NUM_SPOT_LIGHTS/g,t.numSpotLights).replace(/NUM_SPOT_LIGHT_MAPS/g,t.numSpotLightMaps).replace(/NUM_SPOT_LIGHT_COORDS/g,n).replace(/NUM_RECT_AREA_LIGHTS/g,t.numRectAreaLights).replace(/NUM_POINT_LIGHTS/g,t.numPointLights).replace(/NUM_HEMI_LIGHTS/g,t.numHemiLights).replace(/NUM_DIR_LIGHT_SHADOWS/g,t.numDirLightShadows).replace(/NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS/g,t.numSpotLightShadowsWithMaps).replace(/NUM_SPOT_LIGHT_SHADOWS/g,t.numSpotLightShadows).replace(/NUM_POINT_LIGHT_SHADOWS/g,t.numPointLightShadows)}function vu(e,t){return e.replace(/NUM_CLIPPING_PLANES/g,t.numClippingPlanes).replace(/UNION_CLIPPING_PLANES/g,t.numClippingPlanes-t.numClipIntersection)}function yu(e){return e.replace(Pd,bu)}function bu(e,t){let n=Z[t];if(n===void 0){let e=Fd.get(t);if(e!==void 0)n=Z[e],W(`WebGLRenderer: Shader chunk "%s" has been deprecated. Use "%s" instead.`,t,e);else throw Error(`Can not resolve #include <`+t+`>`)}return yu(n)}function xu(e){return e.replace(Id,Su)}function Su(e,t,n,r){let i=``;for(let e=parseInt(t);e<parseInt(n);e++)i+=r.replace(/\[\s*i\s*\]/g,`[ `+e+` ]`).replace(/UNROLLED_LOOP_INDEX/g,e);return i}function Cu(e){let t=`precision ${e.precision} float;
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
	`;return e.precision===`highp`?t+=`
#define HIGH_PRECISION`:e.precision===`mediump`?t+=`
#define MEDIUM_PRECISION`:e.precision===`lowp`&&(t+=`
#define LOW_PRECISION`),t}function wu(e){return Ld[e.shadowMapType]||`SHADOWMAP_TYPE_BASIC`}function Tu(e){return e.envMap===!1?`ENVMAP_TYPE_CUBE`:Rd[e.envMapMode]||`ENVMAP_TYPE_CUBE`}function Eu(e){return e.envMap===!1?`ENVMAP_MODE_REFLECTION`:zd[e.envMapMode]||`ENVMAP_MODE_REFLECTION`}function Du(e){return e.envMap===!1?`ENVMAP_BLENDING_NONE`:Bd[e.combine]||`ENVMAP_BLENDING_NONE`}function Ou(e){let t=e.envMapCubeUVHeight;if(t===null)return null;let n=Math.log2(t)-2,r=1/t;return{texelWidth:1/(3*Math.max(2**n,112)),texelHeight:r,maxMip:n}}function ku(e,t,n,r){let i=e.getContext(),a=n.defines,o=n.vertexShader,s=n.fragmentShader,c=wu(n),l=Tu(n),u=Eu(n),d=Du(n),f=Ou(n),p=pu(n),m=mu(a),h=i.createProgram(),g,_,v=n.glslVersion?`#version `+n.glslVersion+`
`:``;n.isRawShaderMaterial?(g=[`#define SHADER_TYPE `+n.shaderType,`#define SHADER_NAME `+n.shaderName,m].filter(gu).join(`
`),g.length>0&&(g+=`
`),_=[`#define SHADER_TYPE `+n.shaderType,`#define SHADER_NAME `+n.shaderName,m].filter(gu).join(`
`),_.length>0&&(_+=`
`)):(g=[Cu(n),`#define SHADER_TYPE `+n.shaderType,`#define SHADER_NAME `+n.shaderName,m,n.extensionClipCullDistance?`#define USE_CLIP_DISTANCE`:``,n.batching?`#define USE_BATCHING`:``,n.batchingColor?`#define USE_BATCHING_COLOR`:``,n.instancing?`#define USE_INSTANCING`:``,n.instancingColor?`#define USE_INSTANCING_COLOR`:``,n.instancingMorph?`#define USE_INSTANCING_MORPH`:``,n.useFog&&n.fog?`#define USE_FOG`:``,n.useFog&&n.fogExp2?`#define FOG_EXP2`:``,n.map?`#define USE_MAP`:``,n.envMap?`#define USE_ENVMAP`:``,n.envMap?`#define `+u:``,n.lightMap?`#define USE_LIGHTMAP`:``,n.aoMap?`#define USE_AOMAP`:``,n.bumpMap?`#define USE_BUMPMAP`:``,n.normalMap?`#define USE_NORMALMAP`:``,n.normalMapObjectSpace?`#define USE_NORMALMAP_OBJECTSPACE`:``,n.normalMapTangentSpace?`#define USE_NORMALMAP_TANGENTSPACE`:``,n.displacementMap?`#define USE_DISPLACEMENTMAP`:``,n.emissiveMap?`#define USE_EMISSIVEMAP`:``,n.anisotropy?`#define USE_ANISOTROPY`:``,n.anisotropyMap?`#define USE_ANISOTROPYMAP`:``,n.clearcoatMap?`#define USE_CLEARCOATMAP`:``,n.clearcoatRoughnessMap?`#define USE_CLEARCOAT_ROUGHNESSMAP`:``,n.clearcoatNormalMap?`#define USE_CLEARCOAT_NORMALMAP`:``,n.iridescenceMap?`#define USE_IRIDESCENCEMAP`:``,n.iridescenceThicknessMap?`#define USE_IRIDESCENCE_THICKNESSMAP`:``,n.specularMap?`#define USE_SPECULARMAP`:``,n.specularColorMap?`#define USE_SPECULAR_COLORMAP`:``,n.specularIntensityMap?`#define USE_SPECULAR_INTENSITYMAP`:``,n.roughnessMap?`#define USE_ROUGHNESSMAP`:``,n.metalnessMap?`#define USE_METALNESSMAP`:``,n.alphaMap?`#define USE_ALPHAMAP`:``,n.alphaHash?`#define USE_ALPHAHASH`:``,n.transmission?`#define USE_TRANSMISSION`:``,n.transmissionMap?`#define USE_TRANSMISSIONMAP`:``,n.thicknessMap?`#define USE_THICKNESSMAP`:``,n.sheenColorMap?`#define USE_SHEEN_COLORMAP`:``,n.sheenRoughnessMap?`#define USE_SHEEN_ROUGHNESSMAP`:``,n.mapUv?`#define MAP_UV `+n.mapUv:``,n.alphaMapUv?`#define ALPHAMAP_UV `+n.alphaMapUv:``,n.lightMapUv?`#define LIGHTMAP_UV `+n.lightMapUv:``,n.aoMapUv?`#define AOMAP_UV `+n.aoMapUv:``,n.emissiveMapUv?`#define EMISSIVEMAP_UV `+n.emissiveMapUv:``,n.bumpMapUv?`#define BUMPMAP_UV `+n.bumpMapUv:``,n.normalMapUv?`#define NORMALMAP_UV `+n.normalMapUv:``,n.displacementMapUv?`#define DISPLACEMENTMAP_UV `+n.displacementMapUv:``,n.metalnessMapUv?`#define METALNESSMAP_UV `+n.metalnessMapUv:``,n.roughnessMapUv?`#define ROUGHNESSMAP_UV `+n.roughnessMapUv:``,n.anisotropyMapUv?`#define ANISOTROPYMAP_UV `+n.anisotropyMapUv:``,n.clearcoatMapUv?`#define CLEARCOATMAP_UV `+n.clearcoatMapUv:``,n.clearcoatNormalMapUv?`#define CLEARCOAT_NORMALMAP_UV `+n.clearcoatNormalMapUv:``,n.clearcoatRoughnessMapUv?`#define CLEARCOAT_ROUGHNESSMAP_UV `+n.clearcoatRoughnessMapUv:``,n.iridescenceMapUv?`#define IRIDESCENCEMAP_UV `+n.iridescenceMapUv:``,n.iridescenceThicknessMapUv?`#define IRIDESCENCE_THICKNESSMAP_UV `+n.iridescenceThicknessMapUv:``,n.sheenColorMapUv?`#define SHEEN_COLORMAP_UV `+n.sheenColorMapUv:``,n.sheenRoughnessMapUv?`#define SHEEN_ROUGHNESSMAP_UV `+n.sheenRoughnessMapUv:``,n.specularMapUv?`#define SPECULARMAP_UV `+n.specularMapUv:``,n.specularColorMapUv?`#define SPECULAR_COLORMAP_UV `+n.specularColorMapUv:``,n.specularIntensityMapUv?`#define SPECULAR_INTENSITYMAP_UV `+n.specularIntensityMapUv:``,n.transmissionMapUv?`#define TRANSMISSIONMAP_UV `+n.transmissionMapUv:``,n.thicknessMapUv?`#define THICKNESSMAP_UV `+n.thicknessMapUv:``,n.vertexTangents&&n.flatShading===!1?`#define USE_TANGENT`:``,n.vertexNormals?`#define HAS_NORMAL`:``,n.vertexColors?`#define USE_COLOR`:``,n.vertexAlphas?`#define USE_COLOR_ALPHA`:``,n.vertexUv1s?`#define USE_UV1`:``,n.vertexUv2s?`#define USE_UV2`:``,n.vertexUv3s?`#define USE_UV3`:``,n.pointsUvs?`#define USE_POINTS_UV`:``,n.flatShading?`#define FLAT_SHADED`:``,n.skinning?`#define USE_SKINNING`:``,n.morphTargets?`#define USE_MORPHTARGETS`:``,n.morphNormals&&n.flatShading===!1?`#define USE_MORPHNORMALS`:``,n.morphColors?`#define USE_MORPHCOLORS`:``,n.morphTargetsCount>0?`#define MORPHTARGETS_TEXTURE_STRIDE `+n.morphTextureStride:``,n.morphTargetsCount>0?`#define MORPHTARGETS_COUNT `+n.morphTargetsCount:``,n.doubleSided?`#define DOUBLE_SIDED`:``,n.flipSided?`#define FLIP_SIDED`:``,n.shadowMapEnabled?`#define USE_SHADOWMAP`:``,n.shadowMapEnabled?`#define `+c:``,n.sizeAttenuation?`#define USE_SIZEATTENUATION`:``,n.numLightProbes>0?`#define USE_LIGHT_PROBES`:``,n.logarithmicDepthBuffer?`#define USE_LOGARITHMIC_DEPTH_BUFFER`:``,n.reversedDepthBuffer?`#define USE_REVERSED_DEPTH_BUFFER`:``,`uniform mat4 modelMatrix;`,`uniform mat4 modelViewMatrix;`,`uniform mat4 projectionMatrix;`,`uniform mat4 viewMatrix;`,`uniform mat3 normalMatrix;`,`uniform vec3 cameraPosition;`,`uniform bool isOrthographic;`,`#ifdef USE_INSTANCING`,`	attribute mat4 instanceMatrix;`,`#endif`,`#ifdef USE_INSTANCING_COLOR`,`	attribute vec3 instanceColor;`,`#endif`,`#ifdef USE_INSTANCING_MORPH`,`	uniform sampler2D morphTexture;`,`#endif`,`attribute vec3 position;`,`attribute vec3 normal;`,`attribute vec2 uv;`,`#ifdef USE_UV1`,`	attribute vec2 uv1;`,`#endif`,`#ifdef USE_UV2`,`	attribute vec2 uv2;`,`#endif`,`#ifdef USE_UV3`,`	attribute vec2 uv3;`,`#endif`,`#ifdef USE_TANGENT`,`	attribute vec4 tangent;`,`#endif`,`#if defined( USE_COLOR_ALPHA )`,`	attribute vec4 color;`,`#elif defined( USE_COLOR )`,`	attribute vec3 color;`,`#endif`,`#ifdef USE_SKINNING`,`	attribute vec4 skinIndex;`,`	attribute vec4 skinWeight;`,`#endif`,`
`].filter(gu).join(`
`),_=[Cu(n),`#define SHADER_TYPE `+n.shaderType,`#define SHADER_NAME `+n.shaderName,m,n.useFog&&n.fog?`#define USE_FOG`:``,n.useFog&&n.fogExp2?`#define FOG_EXP2`:``,n.alphaToCoverage?`#define ALPHA_TO_COVERAGE`:``,n.map?`#define USE_MAP`:``,n.matcap?`#define USE_MATCAP`:``,n.envMap?`#define USE_ENVMAP`:``,n.envMap?`#define `+l:``,n.envMap?`#define `+u:``,n.envMap?`#define `+d:``,f?`#define CUBEUV_TEXEL_WIDTH `+f.texelWidth:``,f?`#define CUBEUV_TEXEL_HEIGHT `+f.texelHeight:``,f?`#define CUBEUV_MAX_MIP `+f.maxMip+`.0`:``,n.lightMap?`#define USE_LIGHTMAP`:``,n.aoMap?`#define USE_AOMAP`:``,n.bumpMap?`#define USE_BUMPMAP`:``,n.normalMap?`#define USE_NORMALMAP`:``,n.normalMapObjectSpace?`#define USE_NORMALMAP_OBJECTSPACE`:``,n.normalMapTangentSpace?`#define USE_NORMALMAP_TANGENTSPACE`:``,n.packedNormalMap?`#define USE_PACKED_NORMALMAP`:``,n.emissiveMap?`#define USE_EMISSIVEMAP`:``,n.anisotropy?`#define USE_ANISOTROPY`:``,n.anisotropyMap?`#define USE_ANISOTROPYMAP`:``,n.clearcoat?`#define USE_CLEARCOAT`:``,n.clearcoatMap?`#define USE_CLEARCOATMAP`:``,n.clearcoatRoughnessMap?`#define USE_CLEARCOAT_ROUGHNESSMAP`:``,n.clearcoatNormalMap?`#define USE_CLEARCOAT_NORMALMAP`:``,n.dispersion?`#define USE_DISPERSION`:``,n.iridescence?`#define USE_IRIDESCENCE`:``,n.iridescenceMap?`#define USE_IRIDESCENCEMAP`:``,n.iridescenceThicknessMap?`#define USE_IRIDESCENCE_THICKNESSMAP`:``,n.specularMap?`#define USE_SPECULARMAP`:``,n.specularColorMap?`#define USE_SPECULAR_COLORMAP`:``,n.specularIntensityMap?`#define USE_SPECULAR_INTENSITYMAP`:``,n.roughnessMap?`#define USE_ROUGHNESSMAP`:``,n.metalnessMap?`#define USE_METALNESSMAP`:``,n.alphaMap?`#define USE_ALPHAMAP`:``,n.alphaTest?`#define USE_ALPHATEST`:``,n.alphaHash?`#define USE_ALPHAHASH`:``,n.sheen?`#define USE_SHEEN`:``,n.sheenColorMap?`#define USE_SHEEN_COLORMAP`:``,n.sheenRoughnessMap?`#define USE_SHEEN_ROUGHNESSMAP`:``,n.transmission?`#define USE_TRANSMISSION`:``,n.transmissionMap?`#define USE_TRANSMISSIONMAP`:``,n.thicknessMap?`#define USE_THICKNESSMAP`:``,n.vertexTangents&&n.flatShading===!1?`#define USE_TANGENT`:``,n.vertexColors||n.instancingColor?`#define USE_COLOR`:``,n.vertexAlphas||n.batchingColor?`#define USE_COLOR_ALPHA`:``,n.vertexUv1s?`#define USE_UV1`:``,n.vertexUv2s?`#define USE_UV2`:``,n.vertexUv3s?`#define USE_UV3`:``,n.pointsUvs?`#define USE_POINTS_UV`:``,n.gradientMap?`#define USE_GRADIENTMAP`:``,n.flatShading?`#define FLAT_SHADED`:``,n.doubleSided?`#define DOUBLE_SIDED`:``,n.flipSided?`#define FLIP_SIDED`:``,n.shadowMapEnabled?`#define USE_SHADOWMAP`:``,n.shadowMapEnabled?`#define `+c:``,n.premultipliedAlpha?`#define PREMULTIPLIED_ALPHA`:``,n.numLightProbes>0?`#define USE_LIGHT_PROBES`:``,n.numLightProbeGrids>0?`#define USE_LIGHT_PROBES_GRID`:``,n.decodeVideoTexture?`#define DECODE_VIDEO_TEXTURE`:``,n.decodeVideoTextureEmissive?`#define DECODE_VIDEO_TEXTURE_EMISSIVE`:``,n.logarithmicDepthBuffer?`#define USE_LOGARITHMIC_DEPTH_BUFFER`:``,n.reversedDepthBuffer?`#define USE_REVERSED_DEPTH_BUFFER`:``,`uniform mat4 viewMatrix;`,`uniform vec3 cameraPosition;`,`uniform bool isOrthographic;`,n.toneMapping===0?``:`#define TONE_MAPPING`,n.toneMapping===0?``:Z.tonemapping_pars_fragment,n.toneMapping===0?``:du(`toneMapping`,n.toneMapping),n.dithering?`#define DITHERING`:``,n.opaque?`#define OPAQUE`:``,Z.colorspace_pars_fragment,uu(`linearToOutputTexel`,n.outputColorSpace),fu(),n.useDepthPacking?`#define DEPTH_PACKING `+n.depthPacking:``,`
`].filter(gu).join(`
`)),o=yu(o),o=_u(o,n),o=vu(o,n),s=yu(s),s=_u(s,n),s=vu(s,n),o=xu(o),s=xu(s),n.isRawShaderMaterial!==!0&&(v=`#version 300 es
`,g=[p,`#define attribute in`,`#define varying out`,`#define texture2D texture`].join(`
`)+`
`+g,_=[`#define varying in`,n.glslVersion===`300 es`?``:`layout(location = 0) out highp vec4 pc_fragColor;`,n.glslVersion===`300 es`?``:`#define gl_FragColor pc_fragColor`,`#define gl_FragDepthEXT gl_FragDepth`,`#define texture2D texture`,`#define textureCube texture`,`#define texture2DProj textureProj`,`#define texture2DLodEXT textureLod`,`#define texture2DProjLodEXT textureProjLod`,`#define textureCubeLodEXT textureLod`,`#define texture2DGradEXT textureGrad`,`#define texture2DProjGradEXT textureProjGrad`,`#define textureCubeGradEXT textureGrad`].join(`
`)+`
`+_);let y=v+g+o,b=v+_+s,x=ou(i,i.VERTEX_SHADER,y),S=ou(i,i.FRAGMENT_SHADER,b);i.attachShader(h,x),i.attachShader(h,S),n.index0AttributeName===void 0?n.morphTargets===!0&&i.bindAttribLocation(h,0,`position`):i.bindAttribLocation(h,0,n.index0AttributeName),i.linkProgram(h);function C(t){if(e.debug.checkShaderErrors){let n=i.getProgramInfoLog(h)||``,r=i.getShaderInfoLog(x)||``,a=i.getShaderInfoLog(S)||``,o=n.trim(),s=r.trim(),c=a.trim(),l=!0,u=!0;if(i.getProgramParameter(h,i.LINK_STATUS)===!1)if(l=!1,typeof e.debug.onShaderError==`function`)e.debug.onShaderError(i,h,x,S);else{let e=lu(i,x,`vertex`),n=lu(i,S,`fragment`);G(`THREE.WebGLProgram: Shader Error `+i.getError()+` - VALIDATE_STATUS `+i.getProgramParameter(h,i.VALIDATE_STATUS)+`

Material Name: `+t.name+`
Material Type: `+t.type+`

Program Info Log: `+o+`
`+e+`
`+n)}else o===``?(s===``||c===``)&&(u=!1):W(`WebGLProgram: Program Info Log:`,o);u&&(t.diagnostics={runnable:l,programLog:o,vertexShader:{log:s,prefix:g},fragmentShader:{log:c,prefix:_}})}i.deleteShader(x),i.deleteShader(S),w=new Od(i,h),T=hu(i,h)}let w;this.getUniforms=function(){return w===void 0&&C(this),w};let T;this.getAttributes=function(){return T===void 0&&C(this),T};let E=n.rendererExtensionParallelShaderCompile===!1;return this.isReady=function(){return E===!1&&(E=i.getProgramParameter(h,kd)),E},this.destroy=function(){r.releaseStatesOfProgram(this),i.deleteProgram(h),this.program=void 0},this.type=n.shaderType,this.name=n.shaderName,this.id=Ad++,this.cacheKey=t,this.usedTimes=1,this.program=h,this.vertexShader=x,this.fragmentShader=S,this}function Au(e){return e===1030||e===37490||e===36285}function ju(e,t,n,r,i,a){let o=new Vi,s=new Hd,c=new Set,l=[],u=new Map,d=r.logarithmicDepthBuffer,f=r.precision,p={MeshDepthMaterial:`depth`,MeshDistanceMaterial:`distance`,MeshNormalMaterial:`normal`,MeshBasicMaterial:`basic`,MeshLambertMaterial:`lambert`,MeshPhongMaterial:`phong`,MeshToonMaterial:`toon`,MeshStandardMaterial:`physical`,MeshPhysicalMaterial:`physical`,MeshMatcapMaterial:`matcap`,LineBasicMaterial:`basic`,LineDashedMaterial:`dashed`,PointsMaterial:`points`,ShadowMaterial:`shadow`,SpriteMaterial:`sprite`};function m(e){return c.add(e),e===0?`uv`:`uv${e}`}function h(i,o,l,u,h,g){let _=u.fog,v=h.geometry,y=i.isMeshStandardMaterial||i.isMeshLambertMaterial||i.isMeshPhongMaterial?u.environment:null,b=i.isMeshStandardMaterial||i.isMeshLambertMaterial&&!i.envMap||i.isMeshPhongMaterial&&!i.envMap,x=t.get(i.envMap||y,b),S=x&&x.mapping===306?x.image.height:null,C=p[i.type];i.precision!==null&&(f=r.getMaxPrecision(i.precision),f!==i.precision&&W(`WebGLProgram.getParameters:`,i.precision,`not supported, using`,f,`instead.`));let w=v.morphAttributes.position||v.morphAttributes.normal||v.morphAttributes.color,T=w===void 0?0:w.length,E=0;v.morphAttributes.position!==void 0&&(E=1),v.morphAttributes.normal!==void 0&&(E=2),v.morphAttributes.color!==void 0&&(E=3);let D,O,ee,k;if(C){let e=Xu[C];D=e.vertexShader,O=e.fragmentShader}else D=i.vertexShader,O=i.fragmentShader,s.update(i),ee=s.getVertexShaderID(i),k=s.getFragmentShaderID(i);let te=e.getRenderTarget(),ne=e.state.buffers.depth.getReversed(),re=h.isInstancedMesh===!0,ie=h.isBatchedMesh===!0,ae=!!i.map,oe=!!i.matcap,se=!!x,ce=!!i.aoMap,le=!!i.lightMap,ue=!!i.bumpMap,de=!!i.normalMap,fe=!!i.displacementMap,pe=!!i.emissiveMap,me=!!i.metalnessMap,he=!!i.roughnessMap,ge=i.anisotropy>0,_e=i.clearcoat>0,ve=i.dispersion>0,ye=i.iridescence>0,be=i.sheen>0,xe=i.transmission>0,Se=ge&&!!i.anisotropyMap,Ce=_e&&!!i.clearcoatMap,we=_e&&!!i.clearcoatNormalMap,A=_e&&!!i.clearcoatRoughnessMap,Te=ye&&!!i.iridescenceMap,j=ye&&!!i.iridescenceThicknessMap,Ee=be&&!!i.sheenColorMap,M=be&&!!i.sheenRoughnessMap,De=!!i.specularMap,N=!!i.specularColorMap,P=!!i.specularIntensityMap,Oe=xe&&!!i.transmissionMap,ke=xe&&!!i.thicknessMap,Ae=!!i.gradientMap,je=!!i.alphaMap,F=i.alphaTest>0,Me=!!i.alphaHash,Ne=!!i.extensions,Pe=0;i.toneMapped&&(te===null||te.isXRRenderTarget===!0)&&(Pe=e.toneMapping);let I={shaderID:C,shaderType:i.type,shaderName:i.name,vertexShader:D,fragmentShader:O,defines:i.defines,customVertexShaderID:ee,customFragmentShaderID:k,isRawShaderMaterial:i.isRawShaderMaterial===!0,glslVersion:i.glslVersion,precision:f,batching:ie,batchingColor:ie&&h._colorsTexture!==null,instancing:re,instancingColor:re&&h.instanceColor!==null,instancingMorph:re&&h.morphTexture!==null,outputColorSpace:te===null?e.outputColorSpace:te.isXRRenderTarget===!0?te.texture.colorSpace:J.workingColorSpace,alphaToCoverage:!!i.alphaToCoverage,map:ae,matcap:oe,envMap:se,envMapMode:se&&x.mapping,envMapCubeUVHeight:S,aoMap:ce,lightMap:le,bumpMap:ue,normalMap:de,displacementMap:fe,emissiveMap:pe,normalMapObjectSpace:de&&i.normalMapType===1,normalMapTangentSpace:de&&i.normalMapType===0,packedNormalMap:de&&i.normalMapType===0&&Au(i.normalMap.format),metalnessMap:me,roughnessMap:he,anisotropy:ge,anisotropyMap:Se,clearcoat:_e,clearcoatMap:Ce,clearcoatNormalMap:we,clearcoatRoughnessMap:A,dispersion:ve,iridescence:ye,iridescenceMap:Te,iridescenceThicknessMap:j,sheen:be,sheenColorMap:Ee,sheenRoughnessMap:M,specularMap:De,specularColorMap:N,specularIntensityMap:P,transmission:xe,transmissionMap:Oe,thicknessMap:ke,gradientMap:Ae,opaque:i.transparent===!1&&i.blending===1&&i.alphaToCoverage===!1,alphaMap:je,alphaTest:F,alphaHash:Me,combine:i.combine,mapUv:ae&&m(i.map.channel),aoMapUv:ce&&m(i.aoMap.channel),lightMapUv:le&&m(i.lightMap.channel),bumpMapUv:ue&&m(i.bumpMap.channel),normalMapUv:de&&m(i.normalMap.channel),displacementMapUv:fe&&m(i.displacementMap.channel),emissiveMapUv:pe&&m(i.emissiveMap.channel),metalnessMapUv:me&&m(i.metalnessMap.channel),roughnessMapUv:he&&m(i.roughnessMap.channel),anisotropyMapUv:Se&&m(i.anisotropyMap.channel),clearcoatMapUv:Ce&&m(i.clearcoatMap.channel),clearcoatNormalMapUv:we&&m(i.clearcoatNormalMap.channel),clearcoatRoughnessMapUv:A&&m(i.clearcoatRoughnessMap.channel),iridescenceMapUv:Te&&m(i.iridescenceMap.channel),iridescenceThicknessMapUv:j&&m(i.iridescenceThicknessMap.channel),sheenColorMapUv:Ee&&m(i.sheenColorMap.channel),sheenRoughnessMapUv:M&&m(i.sheenRoughnessMap.channel),specularMapUv:De&&m(i.specularMap.channel),specularColorMapUv:N&&m(i.specularColorMap.channel),specularIntensityMapUv:P&&m(i.specularIntensityMap.channel),transmissionMapUv:Oe&&m(i.transmissionMap.channel),thicknessMapUv:ke&&m(i.thicknessMap.channel),alphaMapUv:je&&m(i.alphaMap.channel),vertexTangents:!!v.attributes.tangent&&(de||ge),vertexNormals:!!v.attributes.normal,vertexColors:i.vertexColors,vertexAlphas:i.vertexColors===!0&&!!v.attributes.color&&v.attributes.color.itemSize===4,pointsUvs:h.isPoints===!0&&!!v.attributes.uv&&(ae||je),fog:!!_,useFog:i.fog===!0,fogExp2:!!_&&_.isFogExp2,flatShading:i.wireframe===!1&&(i.flatShading===!0||v.attributes.normal===void 0&&de===!1&&(i.isMeshLambertMaterial||i.isMeshPhongMaterial||i.isMeshStandardMaterial||i.isMeshPhysicalMaterial)),sizeAttenuation:i.sizeAttenuation===!0,logarithmicDepthBuffer:d,reversedDepthBuffer:ne,skinning:h.isSkinnedMesh===!0,morphTargets:v.morphAttributes.position!==void 0,morphNormals:v.morphAttributes.normal!==void 0,morphColors:v.morphAttributes.color!==void 0,morphTargetsCount:T,morphTextureStride:E,numDirLights:o.directional.length,numPointLights:o.point.length,numSpotLights:o.spot.length,numSpotLightMaps:o.spotLightMap.length,numRectAreaLights:o.rectArea.length,numHemiLights:o.hemi.length,numDirLightShadows:o.directionalShadowMap.length,numPointLightShadows:o.pointShadowMap.length,numSpotLightShadows:o.spotShadowMap.length,numSpotLightShadowsWithMaps:o.numSpotLightShadowsWithMaps,numLightProbes:o.numLightProbes,numLightProbeGrids:g.length,numClippingPlanes:a.numPlanes,numClipIntersection:a.numIntersection,dithering:i.dithering,shadowMapEnabled:e.shadowMap.enabled&&l.length>0,shadowMapType:e.shadowMap.type,toneMapping:Pe,decodeVideoTexture:ae&&i.map.isVideoTexture===!0&&J.getTransfer(i.map.colorSpace)===`srgb`,decodeVideoTextureEmissive:pe&&i.emissiveMap.isVideoTexture===!0&&J.getTransfer(i.emissiveMap.colorSpace)===`srgb`,premultipliedAlpha:i.premultipliedAlpha,doubleSided:i.side===2,flipSided:i.side===1,useDepthPacking:i.depthPacking>=0,depthPacking:i.depthPacking||0,index0AttributeName:i.index0AttributeName,extensionClipCullDistance:Ne&&i.extensions.clipCullDistance===!0&&n.has(`WEBGL_clip_cull_distance`),extensionMultiDraw:(Ne&&i.extensions.multiDraw===!0||ie)&&n.has(`WEBGL_multi_draw`),rendererExtensionParallelShaderCompile:n.has(`KHR_parallel_shader_compile`),customProgramCacheKey:i.customProgramCacheKey()};return I.vertexUv1s=c.has(1),I.vertexUv2s=c.has(2),I.vertexUv3s=c.has(3),c.clear(),I}function g(t){let n=[];if(t.shaderID?n.push(t.shaderID):(n.push(t.customVertexShaderID),n.push(t.customFragmentShaderID)),t.defines!==void 0)for(let e in t.defines)n.push(e),n.push(t.defines[e]);return t.isRawShaderMaterial===!1&&(_(n,t),v(n,t),n.push(e.outputColorSpace)),n.push(t.customProgramCacheKey),n.join()}function _(e,t){e.push(t.precision),e.push(t.outputColorSpace),e.push(t.envMapMode),e.push(t.envMapCubeUVHeight),e.push(t.mapUv),e.push(t.alphaMapUv),e.push(t.lightMapUv),e.push(t.aoMapUv),e.push(t.bumpMapUv),e.push(t.normalMapUv),e.push(t.displacementMapUv),e.push(t.emissiveMapUv),e.push(t.metalnessMapUv),e.push(t.roughnessMapUv),e.push(t.anisotropyMapUv),e.push(t.clearcoatMapUv),e.push(t.clearcoatNormalMapUv),e.push(t.clearcoatRoughnessMapUv),e.push(t.iridescenceMapUv),e.push(t.iridescenceThicknessMapUv),e.push(t.sheenColorMapUv),e.push(t.sheenRoughnessMapUv),e.push(t.specularMapUv),e.push(t.specularColorMapUv),e.push(t.specularIntensityMapUv),e.push(t.transmissionMapUv),e.push(t.thicknessMapUv),e.push(t.combine),e.push(t.fogExp2),e.push(t.sizeAttenuation),e.push(t.morphTargetsCount),e.push(t.morphAttributeCount),e.push(t.numDirLights),e.push(t.numPointLights),e.push(t.numSpotLights),e.push(t.numSpotLightMaps),e.push(t.numHemiLights),e.push(t.numRectAreaLights),e.push(t.numDirLightShadows),e.push(t.numPointLightShadows),e.push(t.numSpotLightShadows),e.push(t.numSpotLightShadowsWithMaps),e.push(t.numLightProbes),e.push(t.shadowMapType),e.push(t.toneMapping),e.push(t.numClippingPlanes),e.push(t.numClipIntersection),e.push(t.depthPacking)}function v(e,t){o.disableAll(),t.instancing&&o.enable(0),t.instancingColor&&o.enable(1),t.instancingMorph&&o.enable(2),t.matcap&&o.enable(3),t.envMap&&o.enable(4),t.normalMapObjectSpace&&o.enable(5),t.normalMapTangentSpace&&o.enable(6),t.clearcoat&&o.enable(7),t.iridescence&&o.enable(8),t.alphaTest&&o.enable(9),t.vertexColors&&o.enable(10),t.vertexAlphas&&o.enable(11),t.vertexUv1s&&o.enable(12),t.vertexUv2s&&o.enable(13),t.vertexUv3s&&o.enable(14),t.vertexTangents&&o.enable(15),t.anisotropy&&o.enable(16),t.alphaHash&&o.enable(17),t.batching&&o.enable(18),t.dispersion&&o.enable(19),t.batchingColor&&o.enable(20),t.gradientMap&&o.enable(21),t.packedNormalMap&&o.enable(22),t.vertexNormals&&o.enable(23),e.push(o.mask),o.disableAll(),t.fog&&o.enable(0),t.useFog&&o.enable(1),t.flatShading&&o.enable(2),t.logarithmicDepthBuffer&&o.enable(3),t.reversedDepthBuffer&&o.enable(4),t.skinning&&o.enable(5),t.morphTargets&&o.enable(6),t.morphNormals&&o.enable(7),t.morphColors&&o.enable(8),t.premultipliedAlpha&&o.enable(9),t.shadowMapEnabled&&o.enable(10),t.doubleSided&&o.enable(11),t.flipSided&&o.enable(12),t.useDepthPacking&&o.enable(13),t.dithering&&o.enable(14),t.transmission&&o.enable(15),t.sheen&&o.enable(16),t.opaque&&o.enable(17),t.pointsUvs&&o.enable(18),t.decodeVideoTexture&&o.enable(19),t.decodeVideoTextureEmissive&&o.enable(20),t.alphaToCoverage&&o.enable(21),t.numLightProbeGrids>0&&o.enable(22),e.push(o.mask)}function y(e){let t=p[e.type],n;if(t){let e=Xu[t];n=ks.clone(e.uniforms)}else n=e.uniforms;return n}function b(t,n){let r=u.get(n);return r===void 0?(r=new ku(e,n,t,i),l.push(r),u.set(n,r)):++r.usedTimes,r}function x(e){if(--e.usedTimes===0){let t=l.indexOf(e);l[t]=l[l.length-1],l.pop(),u.delete(e.cacheKey),e.destroy()}}function S(e){s.remove(e)}function C(){s.dispose()}return{getParameters:h,getProgramCacheKey:g,getUniforms:y,acquireProgram:b,releaseProgram:x,releaseShaderCache:S,programs:l,dispose:C}}function Mu(){let e=new WeakMap;function t(t){return e.has(t)}function n(t){let n=e.get(t);return n===void 0&&(n={},e.set(t,n)),n}function r(t){e.delete(t)}function i(t,n,r){e.get(t)[n]=r}function a(){e=new WeakMap}return{has:t,get:n,remove:r,update:i,dispose:a}}function Nu(e,t){return e.groupOrder===t.groupOrder?e.renderOrder===t.renderOrder?e.material.id===t.material.id?e.materialVariant===t.materialVariant?e.z===t.z?e.id-t.id:e.z-t.z:e.materialVariant-t.materialVariant:e.material.id-t.material.id:e.renderOrder-t.renderOrder:e.groupOrder-t.groupOrder}function Pu(e,t){return e.groupOrder===t.groupOrder?e.renderOrder===t.renderOrder?e.z===t.z?e.id-t.id:t.z-e.z:e.renderOrder-t.renderOrder:e.groupOrder-t.groupOrder}function Fu(){let e=[],t=0,n=[],r=[],i=[];function a(){t=0,n.length=0,r.length=0,i.length=0}function o(e){let t=0;return e.isInstancedMesh&&(t+=2),e.isSkinnedMesh&&(t+=1),t}function s(n,r,i,a,s,c){let l=e[t];return l===void 0?(l={id:n.id,object:n,geometry:r,material:i,materialVariant:o(n),groupOrder:a,renderOrder:n.renderOrder,z:s,group:c},e[t]=l):(l.id=n.id,l.object=n,l.geometry=r,l.material=i,l.materialVariant=o(n),l.groupOrder=a,l.renderOrder=n.renderOrder,l.z=s,l.group=c),t++,l}function c(e,t,a,o,c,l){let u=s(e,t,a,o,c,l);a.transmission>0?r.push(u):a.transparent===!0?i.push(u):n.push(u)}function l(e,t,a,o,c,l){let u=s(e,t,a,o,c,l);a.transmission>0?r.unshift(u):a.transparent===!0?i.unshift(u):n.unshift(u)}function u(e,t){n.length>1&&n.sort(e||Nu),r.length>1&&r.sort(t||Pu),i.length>1&&i.sort(t||Pu)}function d(){for(let n=t,r=e.length;n<r;n++){let t=e[n];if(t.id===null)break;t.id=null,t.object=null,t.geometry=null,t.material=null,t.group=null}}return{opaque:n,transmissive:r,transparent:i,init:a,push:c,unshift:l,finish:d,sort:u}}function Iu(){let e=new WeakMap;function t(t,n){let r=e.get(t),i;return r===void 0?(i=new Fu,e.set(t,[i])):n>=r.length?(i=new Fu,r.push(i)):i=r[n],i}function n(){e=new WeakMap}return{get:t,dispose:n}}function Lu(){let e={};return{get:function(t){if(e[t.id]!==void 0)return e[t.id];let n;switch(t.type){case`DirectionalLight`:n={direction:new K,color:new X};break;case`SpotLight`:n={position:new K,direction:new K,color:new X,distance:0,coneCos:0,penumbraCos:0,decay:0};break;case`PointLight`:n={position:new K,color:new X,distance:0,decay:0};break;case`HemisphereLight`:n={direction:new K,skyColor:new X,groundColor:new X};break;case`RectAreaLight`:n={color:new X,position:new K,halfWidth:new K,halfHeight:new K};break}return e[t.id]=n,n}}}function Ru(){let e={};return{get:function(t){if(e[t.id]!==void 0)return e[t.id];let n;switch(t.type){case`DirectionalLight`:n={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new fi};break;case`SpotLight`:n={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new fi};break;case`PointLight`:n={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new fi,shadowCameraNear:1,shadowCameraFar:1e3};break}return e[t.id]=n,n}}}function zu(e,t){return(t.castShadow?2:0)-(e.castShadow?2:0)+ +!!t.map-!!e.map}function Bu(e){let t=new Lu,n=Ru(),r={version:0,hash:{directionalLength:-1,pointLength:-1,spotLength:-1,rectAreaLength:-1,hemiLength:-1,numDirectionalShadows:-1,numPointShadows:-1,numSpotShadows:-1,numSpotMaps:-1,numLightProbes:-1},ambient:[0,0,0],probe:[],directional:[],directionalShadow:[],directionalShadowMap:[],directionalShadowMatrix:[],spot:[],spotLightMap:[],spotShadow:[],spotShadowMap:[],spotLightMatrix:[],rectArea:[],rectAreaLTC1:null,rectAreaLTC2:null,point:[],pointShadow:[],pointShadowMap:[],pointShadowMatrix:[],hemi:[],numSpotLightShadowsWithMaps:0,numLightProbes:0};for(let e=0;e<9;e++)r.probe.push(new K);let i=new K,a=new Y,o=new Y;function s(i){let a=0,o=0,s=0;for(let e=0;e<9;e++)r.probe[e].set(0,0,0);let c=0,l=0,u=0,d=0,f=0,p=0,m=0,h=0,g=0,_=0,v=0;i.sort(zu);for(let e=0,y=i.length;e<y;e++){let y=i[e],b=y.color,x=y.intensity,S=y.distance,C=null;if(y.shadow&&y.shadow.map&&(C=y.shadow.map.texture.format===1030?y.shadow.map.texture:y.shadow.map.depthTexture||y.shadow.map.texture),y.isAmbientLight)a+=b.r*x,o+=b.g*x,s+=b.b*x;else if(y.isLightProbe){for(let e=0;e<9;e++)r.probe[e].addScaledVector(y.sh.coefficients[e],x);v++}else if(y.isDirectionalLight){let e=t.get(y);if(e.color.copy(y.color).multiplyScalar(y.intensity),y.castShadow){let e=y.shadow,t=n.get(y);t.shadowIntensity=e.intensity,t.shadowBias=e.bias,t.shadowNormalBias=e.normalBias,t.shadowRadius=e.radius,t.shadowMapSize=e.mapSize,r.directionalShadow[c]=t,r.directionalShadowMap[c]=C,r.directionalShadowMatrix[c]=y.shadow.matrix,p++}r.directional[c]=e,c++}else if(y.isSpotLight){let e=t.get(y);e.position.setFromMatrixPosition(y.matrixWorld),e.color.copy(b).multiplyScalar(x),e.distance=S,e.coneCos=Math.cos(y.angle),e.penumbraCos=Math.cos(y.angle*(1-y.penumbra)),e.decay=y.decay,r.spot[u]=e;let i=y.shadow;if(y.map&&(r.spotLightMap[g]=y.map,g++,i.updateMatrices(y),y.castShadow&&_++),r.spotLightMatrix[u]=i.matrix,y.castShadow){let e=n.get(y);e.shadowIntensity=i.intensity,e.shadowBias=i.bias,e.shadowNormalBias=i.normalBias,e.shadowRadius=i.radius,e.shadowMapSize=i.mapSize,r.spotShadow[u]=e,r.spotShadowMap[u]=C,h++}u++}else if(y.isRectAreaLight){let e=t.get(y);e.color.copy(b).multiplyScalar(x),e.halfWidth.set(y.width*.5,0,0),e.halfHeight.set(0,y.height*.5,0),r.rectArea[d]=e,d++}else if(y.isPointLight){let e=t.get(y);if(e.color.copy(y.color).multiplyScalar(y.intensity),e.distance=y.distance,e.decay=y.decay,y.castShadow){let e=y.shadow,t=n.get(y);t.shadowIntensity=e.intensity,t.shadowBias=e.bias,t.shadowNormalBias=e.normalBias,t.shadowRadius=e.radius,t.shadowMapSize=e.mapSize,t.shadowCameraNear=e.camera.near,t.shadowCameraFar=e.camera.far,r.pointShadow[l]=t,r.pointShadowMap[l]=C,r.pointShadowMatrix[l]=y.shadow.matrix,m++}r.point[l]=e,l++}else if(y.isHemisphereLight){let e=t.get(y);e.skyColor.copy(y.color).multiplyScalar(x),e.groundColor.copy(y.groundColor).multiplyScalar(x),r.hemi[f]=e,f++}}d>0&&(e.has(`OES_texture_float_linear`)===!0?(r.rectAreaLTC1=Q.LTC_FLOAT_1,r.rectAreaLTC2=Q.LTC_FLOAT_2):(r.rectAreaLTC1=Q.LTC_HALF_1,r.rectAreaLTC2=Q.LTC_HALF_2)),r.ambient[0]=a,r.ambient[1]=o,r.ambient[2]=s;let y=r.hash;(y.directionalLength!==c||y.pointLength!==l||y.spotLength!==u||y.rectAreaLength!==d||y.hemiLength!==f||y.numDirectionalShadows!==p||y.numPointShadows!==m||y.numSpotShadows!==h||y.numSpotMaps!==g||y.numLightProbes!==v)&&(r.directional.length=c,r.spot.length=u,r.rectArea.length=d,r.point.length=l,r.hemi.length=f,r.directionalShadow.length=p,r.directionalShadowMap.length=p,r.pointShadow.length=m,r.pointShadowMap.length=m,r.spotShadow.length=h,r.spotShadowMap.length=h,r.directionalShadowMatrix.length=p,r.pointShadowMatrix.length=m,r.spotLightMatrix.length=h+g-_,r.spotLightMap.length=g,r.numSpotLightShadowsWithMaps=_,r.numLightProbes=v,y.directionalLength=c,y.pointLength=l,y.spotLength=u,y.rectAreaLength=d,y.hemiLength=f,y.numDirectionalShadows=p,y.numPointShadows=m,y.numSpotShadows=h,y.numSpotMaps=g,y.numLightProbes=v,r.version=Wd++)}function c(e,t){let n=0,s=0,c=0,l=0,u=0,d=t.matrixWorldInverse;for(let t=0,f=e.length;t<f;t++){let f=e[t];if(f.isDirectionalLight){let e=r.directional[n];e.direction.setFromMatrixPosition(f.matrixWorld),i.setFromMatrixPosition(f.target.matrixWorld),e.direction.sub(i),e.direction.transformDirection(d),n++}else if(f.isSpotLight){let e=r.spot[c];e.position.setFromMatrixPosition(f.matrixWorld),e.position.applyMatrix4(d),e.direction.setFromMatrixPosition(f.matrixWorld),i.setFromMatrixPosition(f.target.matrixWorld),e.direction.sub(i),e.direction.transformDirection(d),c++}else if(f.isRectAreaLight){let e=r.rectArea[l];e.position.setFromMatrixPosition(f.matrixWorld),e.position.applyMatrix4(d),o.identity(),a.copy(f.matrixWorld),a.premultiply(d),o.extractRotation(a),e.halfWidth.set(f.width*.5,0,0),e.halfHeight.set(0,f.height*.5,0),e.halfWidth.applyMatrix4(o),e.halfHeight.applyMatrix4(o),l++}else if(f.isPointLight){let e=r.point[s];e.position.setFromMatrixPosition(f.matrixWorld),e.position.applyMatrix4(d),s++}else if(f.isHemisphereLight){let e=r.hemi[u];e.direction.setFromMatrixPosition(f.matrixWorld),e.direction.transformDirection(d),u++}}}return{setup:s,setupView:c,state:r}}function Vu(e){let t=new Bu(e),n=[],r=[],i=[];function a(e){d.camera=e,n.length=0,r.length=0,i.length=0}function o(e){n.push(e)}function s(e){r.push(e)}function c(e){i.push(e)}function l(){t.setup(n)}function u(e){t.setupView(n,e)}let d={lightsArray:n,shadowsArray:r,lightProbeGridArray:i,camera:null,lights:t,transmissionRenderTarget:{},textureUnits:0};return{init:a,state:d,setupLights:l,setupLightsView:u,pushLight:o,pushShadow:s,pushLightProbeGrid:c}}function Hu(e){let t=new WeakMap;function n(n,r=0){let i=t.get(n),a;return i===void 0?(a=new Vu(e),t.set(n,[a])):r>=i.length?(a=new Vu(e),i.push(a)):a=i[r],a}function r(){t=new WeakMap}return{get:n,dispose:r}}function Uu(e,t,n){let r=new rs,i=new fi,a=new fi,o=new Ei,s=new Fs,c=new Is,l={},u=n.maxTextureSize,d={0:1,1:0,2:2},f=new Ms({defines:{VSM_SAMPLES:8},uniforms:{shadow_pass:{value:null},resolution:{value:new fi},radius:{value:4}},vertexShader:Gd,fragmentShader:Kd}),p=f.clone();p.defines.HORIZONTAL_PASS=1;let m=new io;m.setAttribute(`position`,new Ua(new Float32Array([-1,-1,.5,3,-1,.5,-1,3,.5]),3));let h=new Oo(m,f),g=this;this.enabled=!1,this.autoUpdate=!0,this.needsUpdate=!1,this.type=1;let _=this.type;this.render=function(t,n,s){if(g.enabled===!1||g.autoUpdate===!1&&g.needsUpdate===!1||t.length===0)return;this.type===2&&(W(`WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead.`),this.type=1);let c=e.getRenderTarget(),l=e.getActiveCubeFace(),d=e.getActiveMipmapLevel(),f=e.state;f.setBlending(0),f.buffers.depth.getReversed()===!0?f.buffers.color.setClear(0,0,0,0):f.buffers.color.setClear(1,1,1,1),f.buffers.depth.setTest(!0),f.setScissorTest(!1);let p=_!==this.type;p&&n.traverse(function(e){e.material&&(Array.isArray(e.material)?e.material.forEach(e=>e.needsUpdate=!0):e.material.needsUpdate=!0)});for(let c=0,l=t.length;c<l;c++){let l=t[c],d=l.shadow;if(d===void 0){W(`WebGLShadowMap:`,l,`has no shadow.`);continue}if(d.autoUpdate===!1&&d.needsUpdate===!1)continue;i.copy(d.mapSize);let m=d.getFrameExtents();i.multiply(m),a.copy(d.mapSize),(i.x>u||i.y>u)&&(i.x>u&&(a.x=Math.floor(u/m.x),i.x=a.x*m.x,d.mapSize.x=a.x),i.y>u&&(a.y=Math.floor(u/m.y),i.y=a.y*m.y,d.mapSize.y=a.y));let h=e.state.buffers.depth.getReversed();if(d.camera._reversedDepth=h,d.map===null||p===!0){if(d.map!==null&&(d.map.depthTexture!==null&&(d.map.depthTexture.dispose(),d.map.depthTexture=null),d.map.dispose()),this.type===3){if(l.isPointLight){W(`WebGLShadowMap: VSM shadow maps are not supported for PointLights. Use PCF or BasicShadowMap instead.`);continue}d.map=new Oi(i.x,i.y,{format:$n,type:Bn,minFilter:An,magFilter:An,generateMipmaps:!1}),d.map.texture.name=l.name+`.shadowMap`,d.map.depthTexture=new Cs(i.x,i.y,zn),d.map.depthTexture.name=l.name+`.shadowMapDepth`,d.map.depthTexture.format=Yn,d.map.depthTexture.compareFunction=null,d.map.depthTexture.minFilter=Dn,d.map.depthTexture.magFilter=Dn}else l.isPointLight?(d.map=new fd(i.x),d.map.depthTexture=new ws(i.x,Rn)):(d.map=new Oi(i.x,i.y),d.map.depthTexture=new Cs(i.x,i.y,Rn)),d.map.depthTexture.name=l.name+`.shadowMap`,d.map.depthTexture.format=Yn,this.type===1?(d.map.depthTexture.compareFunction=h?518:515,d.map.depthTexture.minFilter=An,d.map.depthTexture.magFilter=An):(d.map.depthTexture.compareFunction=null,d.map.depthTexture.minFilter=Dn,d.map.depthTexture.magFilter=Dn);d.camera.updateProjectionMatrix()}let g=d.map.isWebGLCubeRenderTarget?6:1;for(let t=0;t<g;t++){if(d.map.isWebGLCubeRenderTarget)e.setRenderTarget(d.map,t),e.clear();else{t===0&&(e.setRenderTarget(d.map),e.clear());let n=d.getViewport(t);o.set(a.x*n.x,a.y*n.y,a.x*n.z,a.y*n.w),f.viewport(o)}if(l.isPointLight){let e=d.camera,n=d.matrix,r=l.distance||e.far;r!==e.far&&(e.far=r,e.updateProjectionMatrix()),Xd.setFromMatrixPosition(l.matrixWorld),e.position.copy(Xd),Zd.copy(e.position),Zd.add(qd[t]),e.up.copy(Jd[t]),e.lookAt(Zd),e.updateMatrixWorld(),n.makeTranslation(-Xd.x,-Xd.y,-Xd.z),Yd.multiplyMatrices(e.projectionMatrix,e.matrixWorldInverse),d._frustum.setFromProjectionMatrix(Yd,e.coordinateSystem,e.reversedDepth)}else d.updateMatrices(l);r=d.getFrustum(),b(n,s,d.camera,l,this.type)}d.isPointLightShadow!==!0&&this.type===3&&v(d,s),d.needsUpdate=!1}_=this.type,g.needsUpdate=!1,e.setRenderTarget(c,l,d)};function v(n,r){let a=t.update(h);f.defines.VSM_SAMPLES!==n.blurSamples&&(f.defines.VSM_SAMPLES=n.blurSamples,p.defines.VSM_SAMPLES=n.blurSamples,f.needsUpdate=!0,p.needsUpdate=!0),n.mapPass===null&&(n.mapPass=new Oi(i.x,i.y,{format:$n,type:Bn})),f.uniforms.shadow_pass.value=n.map.depthTexture,f.uniforms.resolution.value=n.mapSize,f.uniforms.radius.value=n.radius,e.setRenderTarget(n.mapPass),e.clear(),e.renderBufferDirect(r,null,a,f,h,null),p.uniforms.shadow_pass.value=n.mapPass.texture,p.uniforms.resolution.value=n.mapSize,p.uniforms.radius.value=n.radius,e.setRenderTarget(n.map),e.clear(),e.renderBufferDirect(r,null,a,p,h,null)}function y(t,n,r,i){let a=null,o=r.isPointLight===!0?t.customDistanceMaterial:t.customDepthMaterial;if(o!==void 0)a=o;else if(a=r.isPointLight===!0?c:s,e.localClippingEnabled&&n.clipShadows===!0&&Array.isArray(n.clippingPlanes)&&n.clippingPlanes.length!==0||n.displacementMap&&n.displacementScale!==0||n.alphaMap&&n.alphaTest>0||n.map&&n.alphaTest>0||n.alphaToCoverage===!0){let e=a.uuid,t=n.uuid,r=l[e];r===void 0&&(r={},l[e]=r);let i=r[t];i===void 0&&(i=a.clone(),r[t]=i,n.addEventListener(`dispose`,x)),a=i}if(a.visible=n.visible,a.wireframe=n.wireframe,i===3?a.side=n.shadowSide===null?n.side:n.shadowSide:a.side=n.shadowSide===null?d[n.side]:n.shadowSide,a.alphaMap=n.alphaMap,a.alphaTest=n.alphaToCoverage===!0?.5:n.alphaTest,a.map=n.map,a.clipShadows=n.clipShadows,a.clippingPlanes=n.clippingPlanes,a.clipIntersection=n.clipIntersection,a.displacementMap=n.displacementMap,a.displacementScale=n.displacementScale,a.displacementBias=n.displacementBias,a.wireframeLinewidth=n.wireframeLinewidth,a.linewidth=n.linewidth,r.isPointLight===!0&&a.isMeshDistanceMaterial===!0){let t=e.properties.get(a);t.light=r}return a}function b(n,i,a,o,s){if(n.visible===!1)return;if(n.layers.test(i.layers)&&(n.isMesh||n.isLine||n.isPoints)&&(n.castShadow||n.receiveShadow&&s===3)&&(!n.frustumCulled||r.intersectsObject(n))){n.modelViewMatrix.multiplyMatrices(a.matrixWorldInverse,n.matrixWorld);let r=t.update(n),c=n.material;if(Array.isArray(c)){let t=r.groups;for(let l=0,u=t.length;l<u;l++){let u=t[l],d=c[u.materialIndex];if(d&&d.visible){let t=y(n,d,o,s);n.onBeforeShadow(e,n,i,a,r,t,u),e.renderBufferDirect(a,null,r,t,n,u),n.onAfterShadow(e,n,i,a,r,t,u)}}}else if(c.visible){let t=y(n,c,o,s);n.onBeforeShadow(e,n,i,a,r,t,null),e.renderBufferDirect(a,null,r,t,n,null),n.onAfterShadow(e,n,i,a,r,t,null)}}let c=n.children;for(let e=0,t=c.length;e<t;e++)b(c[e],i,a,o,s)}function x(e){e.target.removeEventListener(`dispose`,x);for(let t in l){let n=l[t],r=e.target.uuid;r in n&&(n[r].dispose(),delete n[r])}}}function Wu(e,t){function n(){let t=!1,n=new Ei,r=null,i=new Ei(0,0,0,0);return{setMask:function(n){r!==n&&!t&&(e.colorMask(n,n,n,n),r=n)},setLocked:function(e){t=e},setClear:function(t,r,a,o,s){s===!0&&(t*=o,r*=o,a*=o),n.set(t,r,a,o),i.equals(n)===!1&&(e.clearColor(t,r,a,o),i.copy(n))},reset:function(){t=!1,r=null,i.set(-1,0,0,0)}}}function r(){let n=!1,r=!1,i=null,a=null,o=null;return{setReversed:function(e){if(r!==e){let n=t.get(`EXT_clip_control`);e?n.clipControlEXT(n.LOWER_LEFT_EXT,n.ZERO_TO_ONE_EXT):n.clipControlEXT(n.LOWER_LEFT_EXT,n.NEGATIVE_ONE_TO_ONE_EXT),r=e;let i=o;o=null,this.setClear(i)}},getReversed:function(){return r},setTest:function(t){t?me(e.DEPTH_TEST):he(e.DEPTH_TEST)},setMask:function(t){i!==t&&!n&&(e.depthMask(t),i=t)},setFunc:function(t){if(r&&(t=si[t]),a!==t){switch(t){case 0:e.depthFunc(e.NEVER);break;case 1:e.depthFunc(e.ALWAYS);break;case 2:e.depthFunc(e.LESS);break;case 3:e.depthFunc(e.LEQUAL);break;case 4:e.depthFunc(e.EQUAL);break;case 5:e.depthFunc(e.GEQUAL);break;case 6:e.depthFunc(e.GREATER);break;case 7:e.depthFunc(e.NOTEQUAL);break;default:e.depthFunc(e.LEQUAL)}a=t}},setLocked:function(e){n=e},setClear:function(t){o!==t&&(o=t,r&&(t=1-t),e.clearDepth(t))},reset:function(){n=!1,i=null,a=null,o=null,r=!1}}}function i(){let t=!1,n=null,r=null,i=null,a=null,o=null,s=null,c=null,l=null;return{setTest:function(n){t||(n?me(e.STENCIL_TEST):he(e.STENCIL_TEST))},setMask:function(r){n!==r&&!t&&(e.stencilMask(r),n=r)},setFunc:function(t,n,o){(r!==t||i!==n||a!==o)&&(e.stencilFunc(t,n,o),r=t,i=n,a=o)},setOp:function(t,n,r){(o!==t||s!==n||c!==r)&&(e.stencilOp(t,n,r),o=t,s=n,c=r)},setLocked:function(e){t=e},setClear:function(t){l!==t&&(e.clearStencil(t),l=t)},reset:function(){t=!1,n=null,r=null,i=null,a=null,o=null,s=null,c=null,l=null}}}let a=new n,o=new r,s=new i,c=new WeakMap,l=new WeakMap,u={},d={},f={},p=new WeakMap,m=[],h=null,g=!1,_=null,v=null,y=null,b=null,x=null,S=null,C=null,w=new X(0,0,0),T=0,E=!1,D=null,O=null,ee=null,k=null,te=null,ne=e.getParameter(e.MAX_COMBINED_TEXTURE_IMAGE_UNITS),re=!1,ie=0,ae=e.getParameter(e.VERSION);ae.indexOf(`WebGL`)===-1?ae.indexOf(`OpenGL ES`)!==-1&&(ie=parseFloat(/^OpenGL ES (\d)/.exec(ae)[1]),re=ie>=2):(ie=parseFloat(/^WebGL (\d)/.exec(ae)[1]),re=ie>=1);let oe=null,se={},ce=e.getParameter(e.SCISSOR_BOX),le=e.getParameter(e.VIEWPORT),ue=new Ei().fromArray(ce),de=new Ei().fromArray(le);function fe(t,n,r,i){let a=new Uint8Array(4),o=e.createTexture();e.bindTexture(t,o),e.texParameteri(t,e.TEXTURE_MIN_FILTER,e.NEAREST),e.texParameteri(t,e.TEXTURE_MAG_FILTER,e.NEAREST);for(let o=0;o<r;o++)t===e.TEXTURE_3D||t===e.TEXTURE_2D_ARRAY?e.texImage3D(n,0,e.RGBA,1,1,i,0,e.RGBA,e.UNSIGNED_BYTE,a):e.texImage2D(n+o,0,e.RGBA,1,1,0,e.RGBA,e.UNSIGNED_BYTE,a);return o}let pe={};pe[e.TEXTURE_2D]=fe(e.TEXTURE_2D,e.TEXTURE_2D,1),pe[e.TEXTURE_CUBE_MAP]=fe(e.TEXTURE_CUBE_MAP,e.TEXTURE_CUBE_MAP_POSITIVE_X,6),pe[e.TEXTURE_2D_ARRAY]=fe(e.TEXTURE_2D_ARRAY,e.TEXTURE_2D_ARRAY,1,1),pe[e.TEXTURE_3D]=fe(e.TEXTURE_3D,e.TEXTURE_3D,1,1),a.setClear(0,0,0,1),o.setClear(1),s.setClear(0),me(e.DEPTH_TEST),o.setFunc(3),Ce(!1),we(1),me(e.CULL_FACE),xe(0);function me(t){u[t]!==!0&&(e.enable(t),u[t]=!0)}function he(t){u[t]!==!1&&(e.disable(t),u[t]=!1)}function ge(t,n){return f[t]===n?!1:(e.bindFramebuffer(t,n),f[t]=n,t===e.DRAW_FRAMEBUFFER&&(f[e.FRAMEBUFFER]=n),t===e.FRAMEBUFFER&&(f[e.DRAW_FRAMEBUFFER]=n),!0)}function _e(t,n){let r=m,i=!1;if(t){r=p.get(n),r===void 0&&(r=[],p.set(n,r));let a=t.textures;if(r.length!==a.length||r[0]!==e.COLOR_ATTACHMENT0){for(let t=0,n=a.length;t<n;t++)r[t]=e.COLOR_ATTACHMENT0+t;r.length=a.length,i=!0}}else r[0]!==e.BACK&&(r[0]=e.BACK,i=!0);i&&e.drawBuffers(r)}function ve(t){return h===t?!1:(e.useProgram(t),h=t,!0)}let ye={100:e.FUNC_ADD,101:e.FUNC_SUBTRACT,102:e.FUNC_REVERSE_SUBTRACT};ye[103]=e.MIN,ye[104]=e.MAX;let be={200:e.ZERO,201:e.ONE,202:e.SRC_COLOR,204:e.SRC_ALPHA,210:e.SRC_ALPHA_SATURATE,208:e.DST_COLOR,206:e.DST_ALPHA,203:e.ONE_MINUS_SRC_COLOR,205:e.ONE_MINUS_SRC_ALPHA,209:e.ONE_MINUS_DST_COLOR,207:e.ONE_MINUS_DST_ALPHA,211:e.CONSTANT_COLOR,212:e.ONE_MINUS_CONSTANT_COLOR,213:e.CONSTANT_ALPHA,214:e.ONE_MINUS_CONSTANT_ALPHA};function xe(t,n,r,i,a,o,s,c,l,u){if(t===0){g===!0&&(he(e.BLEND),g=!1);return}if(g===!1&&(me(e.BLEND),g=!0),t!==5){if(t!==_||u!==E){if((v!==100||x!==100)&&(e.blendEquation(e.FUNC_ADD),v=100,x=100),u)switch(t){case 1:e.blendFuncSeparate(e.ONE,e.ONE_MINUS_SRC_ALPHA,e.ONE,e.ONE_MINUS_SRC_ALPHA);break;case 2:e.blendFunc(e.ONE,e.ONE);break;case 3:e.blendFuncSeparate(e.ZERO,e.ONE_MINUS_SRC_COLOR,e.ZERO,e.ONE);break;case 4:e.blendFuncSeparate(e.DST_COLOR,e.ONE_MINUS_SRC_ALPHA,e.ZERO,e.ONE);break;default:G(`WebGLState: Invalid blending: `,t);break}else switch(t){case 1:e.blendFuncSeparate(e.SRC_ALPHA,e.ONE_MINUS_SRC_ALPHA,e.ONE,e.ONE_MINUS_SRC_ALPHA);break;case 2:e.blendFuncSeparate(e.SRC_ALPHA,e.ONE,e.ONE,e.ONE);break;case 3:G(`WebGLState: SubtractiveBlending requires material.premultipliedAlpha = true`);break;case 4:G(`WebGLState: MultiplyBlending requires material.premultipliedAlpha = true`);break;default:G(`WebGLState: Invalid blending: `,t);break}y=null,b=null,S=null,C=null,w.set(0,0,0),T=0,_=t,E=u}return}a||=n,o||=r,s||=i,(n!==v||a!==x)&&(e.blendEquationSeparate(ye[n],ye[a]),v=n,x=a),(r!==y||i!==b||o!==S||s!==C)&&(e.blendFuncSeparate(be[r],be[i],be[o],be[s]),y=r,b=i,S=o,C=s),(c.equals(w)===!1||l!==T)&&(e.blendColor(c.r,c.g,c.b,l),w.copy(c),T=l),_=t,E=!1}function Se(t,n){t.side===2?he(e.CULL_FACE):me(e.CULL_FACE);let r=t.side===1;n&&(r=!r),Ce(r),t.blending===1&&t.transparent===!1?xe(0):xe(t.blending,t.blendEquation,t.blendSrc,t.blendDst,t.blendEquationAlpha,t.blendSrcAlpha,t.blendDstAlpha,t.blendColor,t.blendAlpha,t.premultipliedAlpha),o.setFunc(t.depthFunc),o.setTest(t.depthTest),o.setMask(t.depthWrite),a.setMask(t.colorWrite);let i=t.stencilWrite;s.setTest(i),i&&(s.setMask(t.stencilWriteMask),s.setFunc(t.stencilFunc,t.stencilRef,t.stencilFuncMask),s.setOp(t.stencilFail,t.stencilZFail,t.stencilZPass)),Te(t.polygonOffset,t.polygonOffsetFactor,t.polygonOffsetUnits),t.alphaToCoverage===!0?me(e.SAMPLE_ALPHA_TO_COVERAGE):he(e.SAMPLE_ALPHA_TO_COVERAGE)}function Ce(t){D!==t&&(t?e.frontFace(e.CW):e.frontFace(e.CCW),D=t)}function we(t){t===0?he(e.CULL_FACE):(me(e.CULL_FACE),t!==O&&(t===1?e.cullFace(e.BACK):t===2?e.cullFace(e.FRONT):e.cullFace(e.FRONT_AND_BACK))),O=t}function A(t){t!==ee&&(re&&e.lineWidth(t),ee=t)}function Te(t,n,r){t?(me(e.POLYGON_OFFSET_FILL),(k!==n||te!==r)&&(k=n,te=r,o.getReversed()&&(n=-n),e.polygonOffset(n,r))):he(e.POLYGON_OFFSET_FILL)}function j(t){t?me(e.SCISSOR_TEST):he(e.SCISSOR_TEST)}function Ee(t){t===void 0&&(t=e.TEXTURE0+ne-1),oe!==t&&(e.activeTexture(t),oe=t)}function M(t,n,r){r===void 0&&(r=oe===null?e.TEXTURE0+ne-1:oe);let i=se[r];i===void 0&&(i={type:void 0,texture:void 0},se[r]=i),(i.type!==t||i.texture!==n)&&(oe!==r&&(e.activeTexture(r),oe=r),e.bindTexture(t,n||pe[t]),i.type=t,i.texture=n)}function De(){let t=se[oe];t!==void 0&&t.type!==void 0&&(e.bindTexture(t.type,null),t.type=void 0,t.texture=void 0)}function N(){try{e.compressedTexImage2D(...arguments)}catch(e){G(`WebGLState:`,e)}}function P(){try{e.compressedTexImage3D(...arguments)}catch(e){G(`WebGLState:`,e)}}function Oe(){try{e.texSubImage2D(...arguments)}catch(e){G(`WebGLState:`,e)}}function ke(){try{e.texSubImage3D(...arguments)}catch(e){G(`WebGLState:`,e)}}function Ae(){try{e.compressedTexSubImage2D(...arguments)}catch(e){G(`WebGLState:`,e)}}function je(){try{e.compressedTexSubImage3D(...arguments)}catch(e){G(`WebGLState:`,e)}}function F(){try{e.texStorage2D(...arguments)}catch(e){G(`WebGLState:`,e)}}function Me(){try{e.texStorage3D(...arguments)}catch(e){G(`WebGLState:`,e)}}function Ne(){try{e.texImage2D(...arguments)}catch(e){G(`WebGLState:`,e)}}function Pe(){try{e.texImage3D(...arguments)}catch(e){G(`WebGLState:`,e)}}function I(t){return d[t]===void 0?e.getParameter(t):d[t]}function Fe(t,n){d[t]!==n&&(e.pixelStorei(t,n),d[t]=n)}function L(t){ue.equals(t)===!1&&(e.scissor(t.x,t.y,t.z,t.w),ue.copy(t))}function Ie(t){de.equals(t)===!1&&(e.viewport(t.x,t.y,t.z,t.w),de.copy(t))}function Le(t,n){let r=l.get(n);r===void 0&&(r=new WeakMap,l.set(n,r));let i=r.get(t);i===void 0&&(i=e.getUniformBlockIndex(n,t.name),r.set(t,i))}function Re(t,n){let r=l.get(n).get(t);c.get(n)!==r&&(e.uniformBlockBinding(n,r,t.__bindingPointIndex),c.set(n,r))}function R(){e.disable(e.BLEND),e.disable(e.CULL_FACE),e.disable(e.DEPTH_TEST),e.disable(e.POLYGON_OFFSET_FILL),e.disable(e.SCISSOR_TEST),e.disable(e.STENCIL_TEST),e.disable(e.SAMPLE_ALPHA_TO_COVERAGE),e.blendEquation(e.FUNC_ADD),e.blendFunc(e.ONE,e.ZERO),e.blendFuncSeparate(e.ONE,e.ZERO,e.ONE,e.ZERO),e.blendColor(0,0,0,0),e.colorMask(!0,!0,!0,!0),e.clearColor(0,0,0,0),e.depthMask(!0),e.depthFunc(e.LESS),o.setReversed(!1),e.clearDepth(1),e.stencilMask(4294967295),e.stencilFunc(e.ALWAYS,0,4294967295),e.stencilOp(e.KEEP,e.KEEP,e.KEEP),e.clearStencil(0),e.cullFace(e.BACK),e.frontFace(e.CCW),e.polygonOffset(0,0),e.activeTexture(e.TEXTURE0),e.bindFramebuffer(e.FRAMEBUFFER,null),e.bindFramebuffer(e.DRAW_FRAMEBUFFER,null),e.bindFramebuffer(e.READ_FRAMEBUFFER,null),e.useProgram(null),e.lineWidth(1),e.scissor(0,0,e.canvas.width,e.canvas.height),e.viewport(0,0,e.canvas.width,e.canvas.height),e.pixelStorei(e.PACK_ALIGNMENT,4),e.pixelStorei(e.UNPACK_ALIGNMENT,4),e.pixelStorei(e.UNPACK_FLIP_Y_WEBGL,!1),e.pixelStorei(e.UNPACK_PREMULTIPLY_ALPHA_WEBGL,!1),e.pixelStorei(e.UNPACK_COLORSPACE_CONVERSION_WEBGL,e.BROWSER_DEFAULT_WEBGL),e.pixelStorei(e.PACK_ROW_LENGTH,0),e.pixelStorei(e.PACK_SKIP_PIXELS,0),e.pixelStorei(e.PACK_SKIP_ROWS,0),e.pixelStorei(e.UNPACK_ROW_LENGTH,0),e.pixelStorei(e.UNPACK_IMAGE_HEIGHT,0),e.pixelStorei(e.UNPACK_SKIP_PIXELS,0),e.pixelStorei(e.UNPACK_SKIP_ROWS,0),e.pixelStorei(e.UNPACK_SKIP_IMAGES,0),u={},d={},oe=null,se={},f={},p=new WeakMap,m=[],h=null,g=!1,_=null,v=null,y=null,b=null,x=null,S=null,C=null,w=new X(0,0,0),T=0,E=!1,D=null,O=null,ee=null,k=null,te=null,ue.set(0,0,e.canvas.width,e.canvas.height),de.set(0,0,e.canvas.width,e.canvas.height),a.reset(),o.reset(),s.reset()}return{buffers:{color:a,depth:o,stencil:s},enable:me,disable:he,bindFramebuffer:ge,drawBuffers:_e,useProgram:ve,setBlending:xe,setMaterial:Se,setFlipSided:Ce,setCullFace:we,setLineWidth:A,setPolygonOffset:Te,setScissorTest:j,activeTexture:Ee,bindTexture:M,unbindTexture:De,compressedTexImage2D:N,compressedTexImage3D:P,texImage2D:Ne,texImage3D:Pe,pixelStorei:Fe,getParameter:I,updateUBOMapping:Le,uniformBlockBinding:Re,texStorage2D:F,texStorage3D:Me,texSubImage2D:Oe,texSubImage3D:ke,compressedTexSubImage2D:Ae,compressedTexSubImage3D:je,scissor:L,viewport:Ie,reset:R}}function Gu(e,t,n,r,i,a,o){let s=t.has(`WEBGL_multisampled_render_to_texture`)?t.get(`WEBGL_multisampled_render_to_texture`):null,c=typeof navigator>`u`?!1:/OculusBrowser/g.test(navigator.userAgent),l=new fi,u=new WeakMap,d=new Set,f,p=new WeakMap,m=!1;try{m=typeof OffscreenCanvas<`u`&&new OffscreenCanvas(1,1).getContext(`2d`)!==null}catch{}function h(e,t){return m?new OffscreenCanvas(e,t):zt(`canvas`)}function g(e,t,n){let r=1,i=N(e);if((i.width>n||i.height>n)&&(r=n/Math.max(i.width,i.height)),r<1)if(typeof HTMLImageElement<`u`&&e instanceof HTMLImageElement||typeof HTMLCanvasElement<`u`&&e instanceof HTMLCanvasElement||typeof ImageBitmap<`u`&&e instanceof ImageBitmap||typeof VideoFrame<`u`&&e instanceof VideoFrame){let n=Math.floor(r*i.width),a=Math.floor(r*i.height);f===void 0&&(f=h(n,a));let o=t?h(n,a):f;return o.width=n,o.height=a,o.getContext(`2d`).drawImage(e,0,0,n,a),W(`WebGLRenderer: Texture has been resized from (`+i.width+`x`+i.height+`) to (`+n+`x`+a+`).`),o}else return`data`in e&&W(`WebGLRenderer: Image in DataTexture is too big (`+i.width+`x`+i.height+`).`),e;return e}function _(e){return e.generateMipmaps}function v(t){e.generateMipmap(t)}function y(t){return t.isWebGLCubeRenderTarget?e.TEXTURE_CUBE_MAP:t.isWebGL3DRenderTarget?e.TEXTURE_3D:t.isWebGLArrayRenderTarget||t.isCompressedArrayTexture?e.TEXTURE_2D_ARRAY:e.TEXTURE_2D}function b(n,r,i,a,o,s=!1){if(n!==null){if(e[n]!==void 0)return e[n];W(`WebGLRenderer: Attempt to use non-existing WebGL internal format '`+n+`'`)}let c;a&&(c=t.get(`EXT_texture_norm16`),c||W(`WebGLRenderer: Unable to use normalized textures without EXT_texture_norm16 extension`));let l=r;if(r===e.RED&&(i===e.FLOAT&&(l=e.R32F),i===e.HALF_FLOAT&&(l=e.R16F),i===e.UNSIGNED_BYTE&&(l=e.R8),i===e.UNSIGNED_SHORT&&c&&(l=c.R16_EXT),i===e.SHORT&&c&&(l=c.R16_SNORM_EXT)),r===e.RED_INTEGER&&(i===e.UNSIGNED_BYTE&&(l=e.R8UI),i===e.UNSIGNED_SHORT&&(l=e.R16UI),i===e.UNSIGNED_INT&&(l=e.R32UI),i===e.BYTE&&(l=e.R8I),i===e.SHORT&&(l=e.R16I),i===e.INT&&(l=e.R32I)),r===e.RG&&(i===e.FLOAT&&(l=e.RG32F),i===e.HALF_FLOAT&&(l=e.RG16F),i===e.UNSIGNED_BYTE&&(l=e.RG8),i===e.UNSIGNED_SHORT&&c&&(l=c.RG16_EXT),i===e.SHORT&&c&&(l=c.RG16_SNORM_EXT)),r===e.RG_INTEGER&&(i===e.UNSIGNED_BYTE&&(l=e.RG8UI),i===e.UNSIGNED_SHORT&&(l=e.RG16UI),i===e.UNSIGNED_INT&&(l=e.RG32UI),i===e.BYTE&&(l=e.RG8I),i===e.SHORT&&(l=e.RG16I),i===e.INT&&(l=e.RG32I)),r===e.RGB_INTEGER&&(i===e.UNSIGNED_BYTE&&(l=e.RGB8UI),i===e.UNSIGNED_SHORT&&(l=e.RGB16UI),i===e.UNSIGNED_INT&&(l=e.RGB32UI),i===e.BYTE&&(l=e.RGB8I),i===e.SHORT&&(l=e.RGB16I),i===e.INT&&(l=e.RGB32I)),r===e.RGBA_INTEGER&&(i===e.UNSIGNED_BYTE&&(l=e.RGBA8UI),i===e.UNSIGNED_SHORT&&(l=e.RGBA16UI),i===e.UNSIGNED_INT&&(l=e.RGBA32UI),i===e.BYTE&&(l=e.RGBA8I),i===e.SHORT&&(l=e.RGBA16I),i===e.INT&&(l=e.RGBA32I)),r===e.RGB&&(i===e.UNSIGNED_SHORT&&c&&(l=c.RGB16_EXT),i===e.SHORT&&c&&(l=c.RGB16_SNORM_EXT),i===e.UNSIGNED_INT_5_9_9_9_REV&&(l=e.RGB9_E5),i===e.UNSIGNED_INT_10F_11F_11F_REV&&(l=e.R11F_G11F_B10F)),r===e.RGBA){let t=s?$r:J.getTransfer(o);i===e.FLOAT&&(l=e.RGBA32F),i===e.HALF_FLOAT&&(l=e.RGBA16F),i===e.UNSIGNED_BYTE&&(l=t===`srgb`?e.SRGB8_ALPHA8:e.RGBA8),i===e.UNSIGNED_SHORT&&c&&(l=c.RGBA16_EXT),i===e.SHORT&&c&&(l=c.RGBA16_SNORM_EXT),i===e.UNSIGNED_SHORT_4_4_4_4&&(l=e.RGBA4),i===e.UNSIGNED_SHORT_5_5_5_1&&(l=e.RGB5_A1)}return(l===e.R16F||l===e.R32F||l===e.RG16F||l===e.RG32F||l===e.RGBA16F||l===e.RGBA32F)&&t.get(`EXT_color_buffer_float`),l}function x(t,n){let r;return t?n===null||n===1014||n===1020?r=e.DEPTH24_STENCIL8:n===1015?r=e.DEPTH32F_STENCIL8:n===1012&&(r=e.DEPTH24_STENCIL8,W(`DepthTexture: 16 bit depth attachment is not supported with stencil. Using 24-bit attachment.`)):n===null||n===1014||n===1020?r=e.DEPTH_COMPONENT24:n===1015?r=e.DEPTH_COMPONENT32F:n===1012&&(r=e.DEPTH_COMPONENT16),r}function S(e,t){return _(e)===!0||e.isFramebufferTexture&&e.minFilter!==1003&&e.minFilter!==1006?Math.log2(Math.max(t.width,t.height))+1:e.mipmaps!==void 0&&e.mipmaps.length>0?e.mipmaps.length:e.isCompressedTexture&&Array.isArray(e.image)?t.mipmaps.length:1}function C(e){let t=e.target;t.removeEventListener(`dispose`,C),T(t),t.isVideoTexture&&u.delete(t),t.isHTMLTexture&&d.delete(t)}function w(e){let t=e.target;t.removeEventListener(`dispose`,w),D(t)}function T(e){let t=r.get(e);if(t.__webglInit===void 0)return;let n=e.source,i=p.get(n);if(i){let r=i[t.__cacheKey];r.usedTimes--,r.usedTimes===0&&E(e),Object.keys(i).length===0&&p.delete(n)}r.remove(e)}function E(t){let n=r.get(t);e.deleteTexture(n.__webglTexture);let i=t.source,a=p.get(i);delete a[n.__cacheKey],o.memory.textures--}function D(t){let n=r.get(t);if(t.depthTexture&&(t.depthTexture.dispose(),r.remove(t.depthTexture)),t.isWebGLCubeRenderTarget)for(let t=0;t<6;t++){if(Array.isArray(n.__webglFramebuffer[t]))for(let r=0;r<n.__webglFramebuffer[t].length;r++)e.deleteFramebuffer(n.__webglFramebuffer[t][r]);else e.deleteFramebuffer(n.__webglFramebuffer[t]);n.__webglDepthbuffer&&e.deleteRenderbuffer(n.__webglDepthbuffer[t])}else{if(Array.isArray(n.__webglFramebuffer))for(let t=0;t<n.__webglFramebuffer.length;t++)e.deleteFramebuffer(n.__webglFramebuffer[t]);else e.deleteFramebuffer(n.__webglFramebuffer);if(n.__webglDepthbuffer&&e.deleteRenderbuffer(n.__webglDepthbuffer),n.__webglMultisampledFramebuffer&&e.deleteFramebuffer(n.__webglMultisampledFramebuffer),n.__webglColorRenderbuffer)for(let t=0;t<n.__webglColorRenderbuffer.length;t++)n.__webglColorRenderbuffer[t]&&e.deleteRenderbuffer(n.__webglColorRenderbuffer[t]);n.__webglDepthRenderbuffer&&e.deleteRenderbuffer(n.__webglDepthRenderbuffer)}let i=t.textures;for(let t=0,n=i.length;t<n;t++){let n=r.get(i[t]);n.__webglTexture&&(e.deleteTexture(n.__webglTexture),o.memory.textures--),r.remove(i[t])}r.remove(t)}let O=0;function ee(){O=0}function k(){return O}function te(e){O=e}function ne(){let e=O;return e>=i.maxTextures&&W(`WebGLTextures: Trying to use `+e+` texture units while this GPU supports only `+i.maxTextures),O+=1,e}function re(e){let t=[];return t.push(e.wrapS),t.push(e.wrapT),t.push(e.wrapR||0),t.push(e.magFilter),t.push(e.minFilter),t.push(e.anisotropy),t.push(e.internalFormat),t.push(e.format),t.push(e.type),t.push(e.generateMipmaps),t.push(e.premultiplyAlpha),t.push(e.flipY),t.push(e.unpackAlignment),t.push(e.colorSpace),t.join()}function ie(t,i){let a=r.get(t);if(t.isVideoTexture&&M(t),t.isRenderTargetTexture===!1&&t.isExternalTexture!==!0&&t.version>0&&a.__version!==t.version){let e=t.image;if(e===null)W(`WebGLRenderer: Texture marked for update but no image data found.`);else if(e.complete===!1)W(`WebGLRenderer: Texture marked for update but image is incomplete`);else{he(a,t,i);return}}else t.isExternalTexture&&(a.__webglTexture=t.sourceTexture?t.sourceTexture:null);n.bindTexture(e.TEXTURE_2D,a.__webglTexture,e.TEXTURE0+i)}function ae(t,i){let a=r.get(t);if(t.isRenderTargetTexture===!1&&t.version>0&&a.__version!==t.version){he(a,t,i);return}else t.isExternalTexture&&(a.__webglTexture=t.sourceTexture?t.sourceTexture:null);n.bindTexture(e.TEXTURE_2D_ARRAY,a.__webglTexture,e.TEXTURE0+i)}function oe(t,i){let a=r.get(t);if(t.isRenderTargetTexture===!1&&t.version>0&&a.__version!==t.version){he(a,t,i);return}n.bindTexture(e.TEXTURE_3D,a.__webglTexture,e.TEXTURE0+i)}function se(t,i){let a=r.get(t);if(t.isCubeDepthTexture!==!0&&t.version>0&&a.__version!==t.version){ge(a,t,i);return}n.bindTexture(e.TEXTURE_CUBE_MAP,a.__webglTexture,e.TEXTURE0+i)}let ce={[wn]:e.REPEAT,[Tn]:e.CLAMP_TO_EDGE,[En]:e.MIRRORED_REPEAT},le={[Dn]:e.NEAREST,[On]:e.NEAREST_MIPMAP_NEAREST,[kn]:e.NEAREST_MIPMAP_LINEAR,[An]:e.LINEAR,[jn]:e.LINEAR_MIPMAP_NEAREST,[Mn]:e.LINEAR_MIPMAP_LINEAR},ue={512:e.NEVER,519:e.ALWAYS,513:e.LESS,515:e.LEQUAL,514:e.EQUAL,518:e.GEQUAL,516:e.GREATER,517:e.NOTEQUAL};function de(n,a){if(a.type===1015&&t.has(`OES_texture_float_linear`)===!1&&(a.magFilter===1006||a.magFilter===1007||a.magFilter===1005||a.magFilter===1008||a.minFilter===1006||a.minFilter===1007||a.minFilter===1005||a.minFilter===1008)&&W(`WebGLRenderer: Unable to use linear filtering with floating point textures. OES_texture_float_linear not supported on this device.`),e.texParameteri(n,e.TEXTURE_WRAP_S,ce[a.wrapS]),e.texParameteri(n,e.TEXTURE_WRAP_T,ce[a.wrapT]),(n===e.TEXTURE_3D||n===e.TEXTURE_2D_ARRAY)&&e.texParameteri(n,e.TEXTURE_WRAP_R,ce[a.wrapR]),e.texParameteri(n,e.TEXTURE_MAG_FILTER,le[a.magFilter]),e.texParameteri(n,e.TEXTURE_MIN_FILTER,le[a.minFilter]),a.compareFunction&&(e.texParameteri(n,e.TEXTURE_COMPARE_MODE,e.COMPARE_REF_TO_TEXTURE),e.texParameteri(n,e.TEXTURE_COMPARE_FUNC,ue[a.compareFunction])),t.has(`EXT_texture_filter_anisotropic`)===!0){if(a.magFilter===1003||a.minFilter!==1005&&a.minFilter!==1008||a.type===1015&&t.has(`OES_texture_float_linear`)===!1)return;if(a.anisotropy>1||r.get(a).__currentAnisotropy){let o=t.get(`EXT_texture_filter_anisotropic`);e.texParameterf(n,o.TEXTURE_MAX_ANISOTROPY_EXT,Math.min(a.anisotropy,i.getMaxAnisotropy())),r.get(a).__currentAnisotropy=a.anisotropy}}}function fe(t,n){let r=!1;t.__webglInit===void 0&&(t.__webglInit=!0,n.addEventListener(`dispose`,C));let i=n.source,a=p.get(i);a===void 0&&(a={},p.set(i,a));let s=re(n);if(s!==t.__cacheKey){a[s]===void 0&&(a[s]={texture:e.createTexture(),usedTimes:0},o.memory.textures++,r=!0),a[s].usedTimes++;let i=a[t.__cacheKey];i!==void 0&&(a[t.__cacheKey].usedTimes--,i.usedTimes===0&&E(n)),t.__cacheKey=s,t.__webglTexture=a[s].texture}return r}function pe(e,t,n){return Math.floor(Math.floor(e/n)/t)}function me(t,r,i,a){let o=t.updateRanges;if(o.length===0)n.texSubImage2D(e.TEXTURE_2D,0,0,0,r.width,r.height,i,a,r.data);else{o.sort((e,t)=>e.start-t.start);let s=0;for(let e=1;e<o.length;e++){let t=o[s],n=o[e],i=t.start+t.count,a=pe(n.start,r.width,4),c=pe(t.start,r.width,4);n.start<=i+1&&a===c&&pe(n.start+n.count-1,r.width,4)===a?t.count=Math.max(t.count,n.start+n.count-t.start):(++s,o[s]=n)}o.length=s+1;let c=n.getParameter(e.UNPACK_ROW_LENGTH),l=n.getParameter(e.UNPACK_SKIP_PIXELS),u=n.getParameter(e.UNPACK_SKIP_ROWS);n.pixelStorei(e.UNPACK_ROW_LENGTH,r.width);for(let t=0,s=o.length;t<s;t++){let s=o[t],c=Math.floor(s.start/4),l=Math.ceil(s.count/4),u=c%r.width,d=Math.floor(c/r.width),f=l;n.pixelStorei(e.UNPACK_SKIP_PIXELS,u),n.pixelStorei(e.UNPACK_SKIP_ROWS,d),n.texSubImage2D(e.TEXTURE_2D,0,u,d,f,1,i,a,r.data)}t.clearUpdateRanges(),n.pixelStorei(e.UNPACK_ROW_LENGTH,c),n.pixelStorei(e.UNPACK_SKIP_PIXELS,l),n.pixelStorei(e.UNPACK_SKIP_ROWS,u)}}function he(t,o,s){let c=e.TEXTURE_2D;(o.isDataArrayTexture||o.isCompressedArrayTexture)&&(c=e.TEXTURE_2D_ARRAY),o.isData3DTexture&&(c=e.TEXTURE_3D);let l=fe(t,o),u=o.source;n.bindTexture(c,t.__webglTexture,e.TEXTURE0+s);let f=r.get(u);if(u.version!==f.__version||l===!0){if(n.activeTexture(e.TEXTURE0+s),!(typeof ImageBitmap<`u`&&o.image instanceof ImageBitmap)){let t=J.getPrimaries(J.workingColorSpace),r=o.colorSpace===``?null:J.getPrimaries(o.colorSpace),i=o.colorSpace===``||t===r?e.NONE:e.BROWSER_DEFAULT_WEBGL;n.pixelStorei(e.UNPACK_FLIP_Y_WEBGL,o.flipY),n.pixelStorei(e.UNPACK_PREMULTIPLY_ALPHA_WEBGL,o.premultiplyAlpha),n.pixelStorei(e.UNPACK_COLORSPACE_CONVERSION_WEBGL,i)}n.pixelStorei(e.UNPACK_ALIGNMENT,o.unpackAlignment);let t=g(o.image,!1,i.maxTextureSize);t=De(o,t);let r=a.convert(o.format,o.colorSpace),p=a.convert(o.type),m=b(o.internalFormat,r,p,o.normalized,o.colorSpace,o.isVideoTexture);de(c,o);let h,y=o.mipmaps,C=o.isVideoTexture!==!0,w=f.__version===void 0||l===!0,T=u.dataReady,E=S(o,t);if(o.isDepthTexture)m=x(o.format===Xn,o.type),w&&(C?n.texStorage2D(e.TEXTURE_2D,1,m,t.width,t.height):n.texImage2D(e.TEXTURE_2D,0,m,t.width,t.height,0,r,p,null));else if(o.isDataTexture)if(y.length>0){C&&w&&n.texStorage2D(e.TEXTURE_2D,E,m,y[0].width,y[0].height);for(let t=0,i=y.length;t<i;t++)h=y[t],C?T&&n.texSubImage2D(e.TEXTURE_2D,t,0,0,h.width,h.height,r,p,h.data):n.texImage2D(e.TEXTURE_2D,t,m,h.width,h.height,0,r,p,h.data);o.generateMipmaps=!1}else C?(w&&n.texStorage2D(e.TEXTURE_2D,E,m,t.width,t.height),T&&me(o,t,r,p)):n.texImage2D(e.TEXTURE_2D,0,m,t.width,t.height,0,r,p,t.data);else if(o.isCompressedTexture)if(o.isCompressedArrayTexture){C&&w&&n.texStorage3D(e.TEXTURE_2D_ARRAY,E,m,y[0].width,y[0].height,t.depth);for(let i=0,a=y.length;i<a;i++)if(h=y[i],o.format!==1023)if(r!==null)if(C){if(T)if(o.layerUpdates.size>0){let t=xn(h.width,h.height,o.format,o.type);for(let a of o.layerUpdates){let o=h.data.subarray(a*t/h.data.BYTES_PER_ELEMENT,(a+1)*t/h.data.BYTES_PER_ELEMENT);n.compressedTexSubImage3D(e.TEXTURE_2D_ARRAY,i,0,0,a,h.width,h.height,1,r,o)}o.clearLayerUpdates()}else n.compressedTexSubImage3D(e.TEXTURE_2D_ARRAY,i,0,0,0,h.width,h.height,t.depth,r,h.data)}else n.compressedTexImage3D(e.TEXTURE_2D_ARRAY,i,m,h.width,h.height,t.depth,0,h.data,0,0);else W(`WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()`);else C?T&&n.texSubImage3D(e.TEXTURE_2D_ARRAY,i,0,0,0,h.width,h.height,t.depth,r,p,h.data):n.texImage3D(e.TEXTURE_2D_ARRAY,i,m,h.width,h.height,t.depth,0,r,p,h.data)}else{C&&w&&n.texStorage2D(e.TEXTURE_2D,E,m,y[0].width,y[0].height);for(let t=0,i=y.length;t<i;t++)h=y[t],o.format===1023?C?T&&n.texSubImage2D(e.TEXTURE_2D,t,0,0,h.width,h.height,r,p,h.data):n.texImage2D(e.TEXTURE_2D,t,m,h.width,h.height,0,r,p,h.data):r===null?W(`WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()`):C?T&&n.compressedTexSubImage2D(e.TEXTURE_2D,t,0,0,h.width,h.height,r,h.data):n.compressedTexImage2D(e.TEXTURE_2D,t,m,h.width,h.height,0,h.data)}else if(o.isDataArrayTexture)if(C){if(w&&n.texStorage3D(e.TEXTURE_2D_ARRAY,E,m,t.width,t.height,t.depth),T)if(o.layerUpdates.size>0){let i=xn(t.width,t.height,o.format,o.type);for(let a of o.layerUpdates){let o=t.data.subarray(a*i/t.data.BYTES_PER_ELEMENT,(a+1)*i/t.data.BYTES_PER_ELEMENT);n.texSubImage3D(e.TEXTURE_2D_ARRAY,0,0,0,a,t.width,t.height,1,r,p,o)}o.clearLayerUpdates()}else n.texSubImage3D(e.TEXTURE_2D_ARRAY,0,0,0,0,t.width,t.height,t.depth,r,p,t.data)}else n.texImage3D(e.TEXTURE_2D_ARRAY,0,m,t.width,t.height,t.depth,0,r,p,t.data);else if(o.isData3DTexture)C?(w&&n.texStorage3D(e.TEXTURE_3D,E,m,t.width,t.height,t.depth),T&&n.texSubImage3D(e.TEXTURE_3D,0,0,0,0,t.width,t.height,t.depth,r,p,t.data)):n.texImage3D(e.TEXTURE_3D,0,m,t.width,t.height,t.depth,0,r,p,t.data);else if(o.isFramebufferTexture){if(w)if(C)n.texStorage2D(e.TEXTURE_2D,E,m,t.width,t.height);else{let i=t.width,a=t.height;for(let t=0;t<E;t++)n.texImage2D(e.TEXTURE_2D,t,m,i,a,0,r,p,null),i>>=1,a>>=1}}else if(o.isHTMLTexture){if(`texElementImage2D`in e){let n=e.canvas;if(n.hasAttribute(`layoutsubtree`)||n.setAttribute(`layoutsubtree`,`true`),t.parentNode!==n){n.appendChild(t),d.add(o),n.onpaint=e=>{let t=e.changedElements;for(let e of d)t.includes(e.image)&&(e.needsUpdate=!0)},n.requestPaint();return}let r=e.RGBA,i=e.RGBA,a=e.UNSIGNED_BYTE;e.texElementImage2D(e.TEXTURE_2D,0,r,i,a,t),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_MIN_FILTER,e.LINEAR),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_WRAP_S,e.CLAMP_TO_EDGE),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_WRAP_T,e.CLAMP_TO_EDGE)}}else if(y.length>0){if(C&&w){let t=N(y[0]);n.texStorage2D(e.TEXTURE_2D,E,m,t.width,t.height)}for(let t=0,i=y.length;t<i;t++)h=y[t],C?T&&n.texSubImage2D(e.TEXTURE_2D,t,0,0,r,p,h):n.texImage2D(e.TEXTURE_2D,t,m,r,p,h);o.generateMipmaps=!1}else if(C){if(w){let r=N(t);n.texStorage2D(e.TEXTURE_2D,E,m,r.width,r.height)}T&&n.texSubImage2D(e.TEXTURE_2D,0,0,0,r,p,t)}else n.texImage2D(e.TEXTURE_2D,0,m,r,p,t);_(o)&&v(c),f.__version=u.version,o.onUpdate&&o.onUpdate(o)}t.__version=o.version}function ge(t,o,s){if(o.image.length!==6)return;let c=fe(t,o),l=o.source;n.bindTexture(e.TEXTURE_CUBE_MAP,t.__webglTexture,e.TEXTURE0+s);let u=r.get(l);if(l.version!==u.__version||c===!0){n.activeTexture(e.TEXTURE0+s);let t=J.getPrimaries(J.workingColorSpace),r=o.colorSpace===``?null:J.getPrimaries(o.colorSpace),d=o.colorSpace===``||t===r?e.NONE:e.BROWSER_DEFAULT_WEBGL;n.pixelStorei(e.UNPACK_FLIP_Y_WEBGL,o.flipY),n.pixelStorei(e.UNPACK_PREMULTIPLY_ALPHA_WEBGL,o.premultiplyAlpha),n.pixelStorei(e.UNPACK_ALIGNMENT,o.unpackAlignment),n.pixelStorei(e.UNPACK_COLORSPACE_CONVERSION_WEBGL,d);let f=o.isCompressedTexture||o.image[0].isCompressedTexture,p=o.image[0]&&o.image[0].isDataTexture,m=[];for(let e=0;e<6;e++)!f&&!p?m[e]=g(o.image[e],!0,i.maxCubemapSize):m[e]=p?o.image[e].image:o.image[e],m[e]=De(o,m[e]);let h=m[0],y=a.convert(o.format,o.colorSpace),x=a.convert(o.type),C=b(o.internalFormat,y,x,o.normalized,o.colorSpace),w=o.isVideoTexture!==!0,T=u.__version===void 0||c===!0,E=l.dataReady,D=S(o,h);de(e.TEXTURE_CUBE_MAP,o);let O;if(f){w&&T&&n.texStorage2D(e.TEXTURE_CUBE_MAP,D,C,h.width,h.height);for(let t=0;t<6;t++){O=m[t].mipmaps;for(let r=0;r<O.length;r++){let i=O[r];o.format===1023?w?E&&n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r,0,0,i.width,i.height,y,x,i.data):n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r,C,i.width,i.height,0,y,x,i.data):y===null?W(`WebGLRenderer: Attempt to load unsupported compressed texture format in .setTextureCube()`):w?E&&n.compressedTexSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r,0,0,i.width,i.height,y,i.data):n.compressedTexImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r,C,i.width,i.height,0,i.data)}}}else{if(O=o.mipmaps,w&&T){O.length>0&&D++;let t=N(m[0]);n.texStorage2D(e.TEXTURE_CUBE_MAP,D,C,t.width,t.height)}for(let t=0;t<6;t++)if(p){w?E&&n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,0,0,0,m[t].width,m[t].height,y,x,m[t].data):n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,0,C,m[t].width,m[t].height,0,y,x,m[t].data);for(let r=0;r<O.length;r++){let i=O[r].image[t].image;w?E&&n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r+1,0,0,i.width,i.height,y,x,i.data):n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r+1,C,i.width,i.height,0,y,x,i.data)}}else{w?E&&n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,0,0,0,y,x,m[t]):n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,0,C,y,x,m[t]);for(let r=0;r<O.length;r++){let i=O[r];w?E&&n.texSubImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r+1,0,0,y,x,i.image[t]):n.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+t,r+1,C,y,x,i.image[t])}}}_(o)&&v(e.TEXTURE_CUBE_MAP),u.__version=l.version,o.onUpdate&&o.onUpdate(o)}t.__version=o.version}function _e(t,i,o,c,l,u){let d=a.convert(o.format,o.colorSpace),f=a.convert(o.type),p=b(o.internalFormat,d,f,o.normalized,o.colorSpace),m=r.get(i),h=r.get(o);if(h.__renderTarget=i,!m.__hasExternalTextures){let t=Math.max(1,i.width>>u),r=Math.max(1,i.height>>u);l===e.TEXTURE_3D||l===e.TEXTURE_2D_ARRAY?n.texImage3D(l,u,p,t,r,i.depth,0,d,f,null):n.texImage2D(l,u,p,t,r,0,d,f,null)}n.bindFramebuffer(e.FRAMEBUFFER,t),Ee(i)?s.framebufferTexture2DMultisampleEXT(e.FRAMEBUFFER,c,l,h.__webglTexture,0,j(i)):(l===e.TEXTURE_2D||l>=e.TEXTURE_CUBE_MAP_POSITIVE_X&&l<=e.TEXTURE_CUBE_MAP_NEGATIVE_Z)&&e.framebufferTexture2D(e.FRAMEBUFFER,c,l,h.__webglTexture,u),n.bindFramebuffer(e.FRAMEBUFFER,null)}function ve(t,n,r){if(e.bindRenderbuffer(e.RENDERBUFFER,t),n.depthBuffer){let i=n.depthTexture,a=i&&i.isDepthTexture?i.type:null,o=x(n.stencilBuffer,a),c=n.stencilBuffer?e.DEPTH_STENCIL_ATTACHMENT:e.DEPTH_ATTACHMENT;Ee(n)?s.renderbufferStorageMultisampleEXT(e.RENDERBUFFER,j(n),o,n.width,n.height):r?e.renderbufferStorageMultisample(e.RENDERBUFFER,j(n),o,n.width,n.height):e.renderbufferStorage(e.RENDERBUFFER,o,n.width,n.height),e.framebufferRenderbuffer(e.FRAMEBUFFER,c,e.RENDERBUFFER,t)}else{let t=n.textures;for(let i=0;i<t.length;i++){let o=t[i],c=a.convert(o.format,o.colorSpace),l=a.convert(o.type),u=b(o.internalFormat,c,l,o.normalized,o.colorSpace);Ee(n)?s.renderbufferStorageMultisampleEXT(e.RENDERBUFFER,j(n),u,n.width,n.height):r?e.renderbufferStorageMultisample(e.RENDERBUFFER,j(n),u,n.width,n.height):e.renderbufferStorage(e.RENDERBUFFER,u,n.width,n.height)}}e.bindRenderbuffer(e.RENDERBUFFER,null)}function ye(t,i,o){let c=i.isWebGLCubeRenderTarget===!0;if(n.bindFramebuffer(e.FRAMEBUFFER,t),!(i.depthTexture&&i.depthTexture.isDepthTexture))throw Error(`renderTarget.depthTexture must be an instance of THREE.DepthTexture`);let l=r.get(i.depthTexture);if(l.__renderTarget=i,(!l.__webglTexture||i.depthTexture.image.width!==i.width||i.depthTexture.image.height!==i.height)&&(i.depthTexture.image.width=i.width,i.depthTexture.image.height=i.height,i.depthTexture.needsUpdate=!0),c){if(l.__webglInit===void 0&&(l.__webglInit=!0,i.depthTexture.addEventListener(`dispose`,C)),l.__webglTexture===void 0){l.__webglTexture=e.createTexture(),n.bindTexture(e.TEXTURE_CUBE_MAP,l.__webglTexture),de(e.TEXTURE_CUBE_MAP,i.depthTexture);let t=a.convert(i.depthTexture.format),r=a.convert(i.depthTexture.type),o;i.depthTexture.format===1026?o=e.DEPTH_COMPONENT24:i.depthTexture.format===1027&&(o=e.DEPTH24_STENCIL8);for(let n=0;n<6;n++)e.texImage2D(e.TEXTURE_CUBE_MAP_POSITIVE_X+n,0,o,i.width,i.height,0,t,r,null)}}else ie(i.depthTexture,0);let u=l.__webglTexture,d=j(i),f=c?e.TEXTURE_CUBE_MAP_POSITIVE_X+o:e.TEXTURE_2D,p=i.depthTexture.format===1027?e.DEPTH_STENCIL_ATTACHMENT:e.DEPTH_ATTACHMENT;if(i.depthTexture.format===1026)Ee(i)?s.framebufferTexture2DMultisampleEXT(e.FRAMEBUFFER,p,f,u,0,d):e.framebufferTexture2D(e.FRAMEBUFFER,p,f,u,0);else if(i.depthTexture.format===1027)Ee(i)?s.framebufferTexture2DMultisampleEXT(e.FRAMEBUFFER,p,f,u,0,d):e.framebufferTexture2D(e.FRAMEBUFFER,p,f,u,0);else throw Error(`Unknown depthTexture format`)}function be(t){let i=r.get(t),a=t.isWebGLCubeRenderTarget===!0;if(i.__boundDepthTexture!==t.depthTexture){let e=t.depthTexture;if(i.__depthDisposeCallback&&i.__depthDisposeCallback(),e){let t=()=>{delete i.__boundDepthTexture,delete i.__depthDisposeCallback,e.removeEventListener(`dispose`,t)};e.addEventListener(`dispose`,t),i.__depthDisposeCallback=t}i.__boundDepthTexture=e}if(t.depthTexture&&!i.__autoAllocateDepthBuffer)if(a)for(let e=0;e<6;e++)ye(i.__webglFramebuffer[e],t,e);else{let e=t.texture.mipmaps;e&&e.length>0?ye(i.__webglFramebuffer[0],t,0):ye(i.__webglFramebuffer,t,0)}else if(a){i.__webglDepthbuffer=[];for(let r=0;r<6;r++)if(n.bindFramebuffer(e.FRAMEBUFFER,i.__webglFramebuffer[r]),i.__webglDepthbuffer[r]===void 0)i.__webglDepthbuffer[r]=e.createRenderbuffer(),ve(i.__webglDepthbuffer[r],t,!1);else{let n=t.stencilBuffer?e.DEPTH_STENCIL_ATTACHMENT:e.DEPTH_ATTACHMENT,a=i.__webglDepthbuffer[r];e.bindRenderbuffer(e.RENDERBUFFER,a),e.framebufferRenderbuffer(e.FRAMEBUFFER,n,e.RENDERBUFFER,a)}}else{let r=t.texture.mipmaps;if(r&&r.length>0?n.bindFramebuffer(e.FRAMEBUFFER,i.__webglFramebuffer[0]):n.bindFramebuffer(e.FRAMEBUFFER,i.__webglFramebuffer),i.__webglDepthbuffer===void 0)i.__webglDepthbuffer=e.createRenderbuffer(),ve(i.__webglDepthbuffer,t,!1);else{let n=t.stencilBuffer?e.DEPTH_STENCIL_ATTACHMENT:e.DEPTH_ATTACHMENT,r=i.__webglDepthbuffer;e.bindRenderbuffer(e.RENDERBUFFER,r),e.framebufferRenderbuffer(e.FRAMEBUFFER,n,e.RENDERBUFFER,r)}}n.bindFramebuffer(e.FRAMEBUFFER,null)}function xe(t,n,i){let a=r.get(t);n!==void 0&&_e(a.__webglFramebuffer,t,t.texture,e.COLOR_ATTACHMENT0,e.TEXTURE_2D,0),i!==void 0&&be(t)}function Se(t){let i=t.texture,s=r.get(t),c=r.get(i);t.addEventListener(`dispose`,w);let l=t.textures,u=t.isWebGLCubeRenderTarget===!0,d=l.length>1;if(d||(c.__webglTexture===void 0&&(c.__webglTexture=e.createTexture()),c.__version=i.version,o.memory.textures++),u){s.__webglFramebuffer=[];for(let t=0;t<6;t++)if(i.mipmaps&&i.mipmaps.length>0){s.__webglFramebuffer[t]=[];for(let n=0;n<i.mipmaps.length;n++)s.__webglFramebuffer[t][n]=e.createFramebuffer()}else s.__webglFramebuffer[t]=e.createFramebuffer()}else{if(i.mipmaps&&i.mipmaps.length>0){s.__webglFramebuffer=[];for(let t=0;t<i.mipmaps.length;t++)s.__webglFramebuffer[t]=e.createFramebuffer()}else s.__webglFramebuffer=e.createFramebuffer();if(d)for(let t=0,n=l.length;t<n;t++){let n=r.get(l[t]);n.__webglTexture===void 0&&(n.__webglTexture=e.createTexture(),o.memory.textures++)}if(t.samples>0&&Ee(t)===!1){s.__webglMultisampledFramebuffer=e.createFramebuffer(),s.__webglColorRenderbuffer=[],n.bindFramebuffer(e.FRAMEBUFFER,s.__webglMultisampledFramebuffer);for(let n=0;n<l.length;n++){let r=l[n];s.__webglColorRenderbuffer[n]=e.createRenderbuffer(),e.bindRenderbuffer(e.RENDERBUFFER,s.__webglColorRenderbuffer[n]);let i=a.convert(r.format,r.colorSpace),o=a.convert(r.type),c=b(r.internalFormat,i,o,r.normalized,r.colorSpace,t.isXRRenderTarget===!0),u=j(t);e.renderbufferStorageMultisample(e.RENDERBUFFER,u,c,t.width,t.height),e.framebufferRenderbuffer(e.FRAMEBUFFER,e.COLOR_ATTACHMENT0+n,e.RENDERBUFFER,s.__webglColorRenderbuffer[n])}e.bindRenderbuffer(e.RENDERBUFFER,null),t.depthBuffer&&(s.__webglDepthRenderbuffer=e.createRenderbuffer(),ve(s.__webglDepthRenderbuffer,t,!0)),n.bindFramebuffer(e.FRAMEBUFFER,null)}}if(u){n.bindTexture(e.TEXTURE_CUBE_MAP,c.__webglTexture),de(e.TEXTURE_CUBE_MAP,i);for(let n=0;n<6;n++)if(i.mipmaps&&i.mipmaps.length>0)for(let r=0;r<i.mipmaps.length;r++)_e(s.__webglFramebuffer[n][r],t,i,e.COLOR_ATTACHMENT0,e.TEXTURE_CUBE_MAP_POSITIVE_X+n,r);else _e(s.__webglFramebuffer[n],t,i,e.COLOR_ATTACHMENT0,e.TEXTURE_CUBE_MAP_POSITIVE_X+n,0);_(i)&&v(e.TEXTURE_CUBE_MAP),n.unbindTexture()}else if(d){for(let i=0,a=l.length;i<a;i++){let a=l[i],o=r.get(a),c=e.TEXTURE_2D;(t.isWebGL3DRenderTarget||t.isWebGLArrayRenderTarget)&&(c=t.isWebGL3DRenderTarget?e.TEXTURE_3D:e.TEXTURE_2D_ARRAY),n.bindTexture(c,o.__webglTexture),de(c,a),_e(s.__webglFramebuffer,t,a,e.COLOR_ATTACHMENT0+i,c,0),_(a)&&v(c)}n.unbindTexture()}else{let r=e.TEXTURE_2D;if((t.isWebGL3DRenderTarget||t.isWebGLArrayRenderTarget)&&(r=t.isWebGL3DRenderTarget?e.TEXTURE_3D:e.TEXTURE_2D_ARRAY),n.bindTexture(r,c.__webglTexture),de(r,i),i.mipmaps&&i.mipmaps.length>0)for(let n=0;n<i.mipmaps.length;n++)_e(s.__webglFramebuffer[n],t,i,e.COLOR_ATTACHMENT0,r,n);else _e(s.__webglFramebuffer,t,i,e.COLOR_ATTACHMENT0,r,0);_(i)&&v(r),n.unbindTexture()}t.depthBuffer&&be(t)}function Ce(e){let t=e.textures;for(let i=0,a=t.length;i<a;i++){let a=t[i];if(_(a)){let t=y(e),i=r.get(a).__webglTexture;n.bindTexture(t,i),v(t),n.unbindTexture()}}}let we=[],A=[];function Te(t){if(t.samples>0){if(Ee(t)===!1){let i=t.textures,a=t.width,o=t.height,s=e.COLOR_BUFFER_BIT,l=t.stencilBuffer?e.DEPTH_STENCIL_ATTACHMENT:e.DEPTH_ATTACHMENT,u=r.get(t),d=i.length>1;if(d)for(let t=0;t<i.length;t++)n.bindFramebuffer(e.FRAMEBUFFER,u.__webglMultisampledFramebuffer),e.framebufferRenderbuffer(e.FRAMEBUFFER,e.COLOR_ATTACHMENT0+t,e.RENDERBUFFER,null),n.bindFramebuffer(e.FRAMEBUFFER,u.__webglFramebuffer),e.framebufferTexture2D(e.DRAW_FRAMEBUFFER,e.COLOR_ATTACHMENT0+t,e.TEXTURE_2D,null,0);n.bindFramebuffer(e.READ_FRAMEBUFFER,u.__webglMultisampledFramebuffer);let f=t.texture.mipmaps;f&&f.length>0?n.bindFramebuffer(e.DRAW_FRAMEBUFFER,u.__webglFramebuffer[0]):n.bindFramebuffer(e.DRAW_FRAMEBUFFER,u.__webglFramebuffer);for(let n=0;n<i.length;n++){if(t.resolveDepthBuffer&&(t.depthBuffer&&(s|=e.DEPTH_BUFFER_BIT),t.stencilBuffer&&t.resolveStencilBuffer&&(s|=e.STENCIL_BUFFER_BIT)),d){e.framebufferRenderbuffer(e.READ_FRAMEBUFFER,e.COLOR_ATTACHMENT0,e.RENDERBUFFER,u.__webglColorRenderbuffer[n]);let t=r.get(i[n]).__webglTexture;e.framebufferTexture2D(e.DRAW_FRAMEBUFFER,e.COLOR_ATTACHMENT0,e.TEXTURE_2D,t,0)}e.blitFramebuffer(0,0,a,o,0,0,a,o,s,e.NEAREST),c===!0&&(we.length=0,A.length=0,we.push(e.COLOR_ATTACHMENT0+n),t.depthBuffer&&t.resolveDepthBuffer===!1&&(we.push(l),A.push(l),e.invalidateFramebuffer(e.DRAW_FRAMEBUFFER,A)),e.invalidateFramebuffer(e.READ_FRAMEBUFFER,we))}if(n.bindFramebuffer(e.READ_FRAMEBUFFER,null),n.bindFramebuffer(e.DRAW_FRAMEBUFFER,null),d)for(let t=0;t<i.length;t++){n.bindFramebuffer(e.FRAMEBUFFER,u.__webglMultisampledFramebuffer),e.framebufferRenderbuffer(e.FRAMEBUFFER,e.COLOR_ATTACHMENT0+t,e.RENDERBUFFER,u.__webglColorRenderbuffer[t]);let a=r.get(i[t]).__webglTexture;n.bindFramebuffer(e.FRAMEBUFFER,u.__webglFramebuffer),e.framebufferTexture2D(e.DRAW_FRAMEBUFFER,e.COLOR_ATTACHMENT0+t,e.TEXTURE_2D,a,0)}n.bindFramebuffer(e.DRAW_FRAMEBUFFER,u.__webglMultisampledFramebuffer)}else if(t.depthBuffer&&t.resolveDepthBuffer===!1&&c){let n=t.stencilBuffer?e.DEPTH_STENCIL_ATTACHMENT:e.DEPTH_ATTACHMENT;e.invalidateFramebuffer(e.DRAW_FRAMEBUFFER,[n])}}}function j(e){return Math.min(i.maxSamples,e.samples)}function Ee(e){let n=r.get(e);return e.samples>0&&t.has(`WEBGL_multisampled_render_to_texture`)===!0&&n.__useRenderToTexture!==!1}function M(e){let t=o.render.frame;u.get(e)!==t&&(u.set(e,t),e.update())}function De(e,t){let n=e.colorSpace,r=e.format,i=e.type;return e.isCompressedTexture===!0||e.isVideoTexture===!0||n!==`srgb-linear`&&n!==``&&(J.getTransfer(n)===`srgb`?(r!==1023||i!==1009)&&W(`WebGLTextures: sRGB encoded textures have to use RGBAFormat and UnsignedByteType.`):G(`WebGLTextures: Unsupported texture color space:`,n)),t}function N(e){return typeof HTMLImageElement<`u`&&e instanceof HTMLImageElement?(l.width=e.naturalWidth||e.width,l.height=e.naturalHeight||e.height):typeof VideoFrame<`u`&&e instanceof VideoFrame?(l.width=e.displayWidth,l.height=e.displayHeight):(l.width=e.width,l.height=e.height),l}this.allocateTextureUnit=ne,this.resetTextureUnits=ee,this.getTextureUnits=k,this.setTextureUnits=te,this.setTexture2D=ie,this.setTexture2DArray=ae,this.setTexture3D=oe,this.setTextureCube=se,this.rebindTextures=xe,this.setupRenderTarget=Se,this.updateRenderTargetMipmap=Ce,this.updateMultisampleRenderTarget=Te,this.setupDepthRenderbuffer=be,this.setupFrameBufferTexture=_e,this.useMultisampledRTT=Ee,this.isReversedDepthBuffer=function(){return n.buffers.depth.getReversed()}}function Ku(e,t){function n(n,r=``){let i,a=J.getTransfer(r);if(n===1009)return e.UNSIGNED_BYTE;if(n===1017)return e.UNSIGNED_SHORT_4_4_4_4;if(n===1018)return e.UNSIGNED_SHORT_5_5_5_1;if(n===35902)return e.UNSIGNED_INT_5_9_9_9_REV;if(n===35899)return e.UNSIGNED_INT_10F_11F_11F_REV;if(n===1010)return e.BYTE;if(n===1011)return e.SHORT;if(n===1012)return e.UNSIGNED_SHORT;if(n===1013)return e.INT;if(n===1014)return e.UNSIGNED_INT;if(n===1015)return e.FLOAT;if(n===1016)return e.HALF_FLOAT;if(n===1021)return e.ALPHA;if(n===1022)return e.RGB;if(n===1023)return e.RGBA;if(n===1026)return e.DEPTH_COMPONENT;if(n===1027)return e.DEPTH_STENCIL;if(n===1028)return e.RED;if(n===1029)return e.RED_INTEGER;if(n===1030)return e.RG;if(n===1031)return e.RG_INTEGER;if(n===1033)return e.RGBA_INTEGER;if(n===33776||n===33777||n===33778||n===33779)if(a===`srgb`)if(i=t.get(`WEBGL_compressed_texture_s3tc_srgb`),i!==null){if(n===33776)return i.COMPRESSED_SRGB_S3TC_DXT1_EXT;if(n===33777)return i.COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT;if(n===33778)return i.COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT;if(n===33779)return i.COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT}else return null;else if(i=t.get(`WEBGL_compressed_texture_s3tc`),i!==null){if(n===33776)return i.COMPRESSED_RGB_S3TC_DXT1_EXT;if(n===33777)return i.COMPRESSED_RGBA_S3TC_DXT1_EXT;if(n===33778)return i.COMPRESSED_RGBA_S3TC_DXT3_EXT;if(n===33779)return i.COMPRESSED_RGBA_S3TC_DXT5_EXT}else return null;if(n===35840||n===35841||n===35842||n===35843)if(i=t.get(`WEBGL_compressed_texture_pvrtc`),i!==null){if(n===35840)return i.COMPRESSED_RGB_PVRTC_4BPPV1_IMG;if(n===35841)return i.COMPRESSED_RGB_PVRTC_2BPPV1_IMG;if(n===35842)return i.COMPRESSED_RGBA_PVRTC_4BPPV1_IMG;if(n===35843)return i.COMPRESSED_RGBA_PVRTC_2BPPV1_IMG}else return null;if(n===36196||n===37492||n===37496||n===37488||n===37489||n===37490||n===37491)if(i=t.get(`WEBGL_compressed_texture_etc`),i!==null){if(n===36196||n===37492)return a===`srgb`?i.COMPRESSED_SRGB8_ETC2:i.COMPRESSED_RGB8_ETC2;if(n===37496)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ETC2_EAC:i.COMPRESSED_RGBA8_ETC2_EAC;if(n===37488)return i.COMPRESSED_R11_EAC;if(n===37489)return i.COMPRESSED_SIGNED_R11_EAC;if(n===37490)return i.COMPRESSED_RG11_EAC;if(n===37491)return i.COMPRESSED_SIGNED_RG11_EAC}else return null;if(n===37808||n===37809||n===37810||n===37811||n===37812||n===37813||n===37814||n===37815||n===37816||n===37817||n===37818||n===37819||n===37820||n===37821)if(i=t.get(`WEBGL_compressed_texture_astc`),i!==null){if(n===37808)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_4x4_KHR:i.COMPRESSED_RGBA_ASTC_4x4_KHR;if(n===37809)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_5x4_KHR:i.COMPRESSED_RGBA_ASTC_5x4_KHR;if(n===37810)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_5x5_KHR:i.COMPRESSED_RGBA_ASTC_5x5_KHR;if(n===37811)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_6x5_KHR:i.COMPRESSED_RGBA_ASTC_6x5_KHR;if(n===37812)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_6x6_KHR:i.COMPRESSED_RGBA_ASTC_6x6_KHR;if(n===37813)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_8x5_KHR:i.COMPRESSED_RGBA_ASTC_8x5_KHR;if(n===37814)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_8x6_KHR:i.COMPRESSED_RGBA_ASTC_8x6_KHR;if(n===37815)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_8x8_KHR:i.COMPRESSED_RGBA_ASTC_8x8_KHR;if(n===37816)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x5_KHR:i.COMPRESSED_RGBA_ASTC_10x5_KHR;if(n===37817)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x6_KHR:i.COMPRESSED_RGBA_ASTC_10x6_KHR;if(n===37818)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x8_KHR:i.COMPRESSED_RGBA_ASTC_10x8_KHR;if(n===37819)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_10x10_KHR:i.COMPRESSED_RGBA_ASTC_10x10_KHR;if(n===37820)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_12x10_KHR:i.COMPRESSED_RGBA_ASTC_12x10_KHR;if(n===37821)return a===`srgb`?i.COMPRESSED_SRGB8_ALPHA8_ASTC_12x12_KHR:i.COMPRESSED_RGBA_ASTC_12x12_KHR}else return null;if(n===36492||n===36494||n===36495)if(i=t.get(`EXT_texture_compression_bptc`),i!==null){if(n===36492)return a===`srgb`?i.COMPRESSED_SRGB_ALPHA_BPTC_UNORM_EXT:i.COMPRESSED_RGBA_BPTC_UNORM_EXT;if(n===36494)return i.COMPRESSED_RGB_BPTC_SIGNED_FLOAT_EXT;if(n===36495)return i.COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT_EXT}else return null;if(n===36283||n===36284||n===36285||n===36286)if(i=t.get(`EXT_texture_compression_rgtc`),i!==null){if(n===36283)return i.COMPRESSED_RED_RGTC1_EXT;if(n===36284)return i.COMPRESSED_SIGNED_RED_RGTC1_EXT;if(n===36285)return i.COMPRESSED_RED_GREEN_RGTC2_EXT;if(n===36286)return i.COMPRESSED_SIGNED_RED_GREEN_RGTC2_EXT}else return null;return n===1020?e.UNSIGNED_INT_24_8:e[n]===void 0?null:e[n]}return{convert:n}}function qu(e,t){function n(e,t){e.matrixAutoUpdate===!0&&e.updateMatrix(),t.value.copy(e.matrix)}function r(t,n){n.color.getRGB(t.fogColor.value,fn(e)),n.isFog?(t.fogNear.value=n.near,t.fogFar.value=n.far):n.isFogExp2&&(t.fogDensity.value=n.density)}function i(e,t,n,r,i){t.isNodeMaterial?t.uniformsNeedUpdate=!1:t.isMeshBasicMaterial?a(e,t):t.isMeshLambertMaterial?(a(e,t),t.envMap&&(e.envMapIntensity.value=t.envMapIntensity)):t.isMeshToonMaterial?(a(e,t),d(e,t)):t.isMeshPhongMaterial?(a(e,t),u(e,t),t.envMap&&(e.envMapIntensity.value=t.envMapIntensity)):t.isMeshStandardMaterial?(a(e,t),f(e,t),t.isMeshPhysicalMaterial&&p(e,t,i)):t.isMeshMatcapMaterial?(a(e,t),m(e,t)):t.isMeshDepthMaterial?a(e,t):t.isMeshDistanceMaterial?(a(e,t),h(e,t)):t.isMeshNormalMaterial?a(e,t):t.isLineBasicMaterial?(o(e,t),t.isLineDashedMaterial&&s(e,t)):t.isPointsMaterial?c(e,t,n,r):t.isSpriteMaterial?l(e,t):t.isShadowMaterial?(e.color.value.copy(t.color),e.opacity.value=t.opacity):t.isShaderMaterial&&(t.uniformsNeedUpdate=!1)}function a(e,r){e.opacity.value=r.opacity,r.color&&e.diffuse.value.copy(r.color),r.emissive&&e.emissive.value.copy(r.emissive).multiplyScalar(r.emissiveIntensity),r.map&&(e.map.value=r.map,n(r.map,e.mapTransform)),r.alphaMap&&(e.alphaMap.value=r.alphaMap,n(r.alphaMap,e.alphaMapTransform)),r.bumpMap&&(e.bumpMap.value=r.bumpMap,n(r.bumpMap,e.bumpMapTransform),e.bumpScale.value=r.bumpScale,r.side===1&&(e.bumpScale.value*=-1)),r.normalMap&&(e.normalMap.value=r.normalMap,n(r.normalMap,e.normalMapTransform),e.normalScale.value.copy(r.normalScale),r.side===1&&e.normalScale.value.negate()),r.displacementMap&&(e.displacementMap.value=r.displacementMap,n(r.displacementMap,e.displacementMapTransform),e.displacementScale.value=r.displacementScale,e.displacementBias.value=r.displacementBias),r.emissiveMap&&(e.emissiveMap.value=r.emissiveMap,n(r.emissiveMap,e.emissiveMapTransform)),r.specularMap&&(e.specularMap.value=r.specularMap,n(r.specularMap,e.specularMapTransform)),r.alphaTest>0&&(e.alphaTest.value=r.alphaTest);let i=t.get(r),a=i.envMap,o=i.envMapRotation;a&&(e.envMap.value=a,e.envMapRotation.value.setFromMatrix4(nf.makeRotationFromEuler(o)).transpose(),a.isCubeTexture&&a.isRenderTargetTexture===!1&&e.envMapRotation.value.premultiply(rf),e.reflectivity.value=r.reflectivity,e.ior.value=r.ior,e.refractionRatio.value=r.refractionRatio),r.lightMap&&(e.lightMap.value=r.lightMap,e.lightMapIntensity.value=r.lightMapIntensity,n(r.lightMap,e.lightMapTransform)),r.aoMap&&(e.aoMap.value=r.aoMap,e.aoMapIntensity.value=r.aoMapIntensity,n(r.aoMap,e.aoMapTransform))}function o(e,t){e.diffuse.value.copy(t.color),e.opacity.value=t.opacity,t.map&&(e.map.value=t.map,n(t.map,e.mapTransform))}function s(e,t){e.dashSize.value=t.dashSize,e.totalSize.value=t.dashSize+t.gapSize,e.scale.value=t.scale}function c(e,t,r,i){e.diffuse.value.copy(t.color),e.opacity.value=t.opacity,e.size.value=t.size*r,e.scale.value=i*.5,t.map&&(e.map.value=t.map,n(t.map,e.uvTransform)),t.alphaMap&&(e.alphaMap.value=t.alphaMap,n(t.alphaMap,e.alphaMapTransform)),t.alphaTest>0&&(e.alphaTest.value=t.alphaTest)}function l(e,t){e.diffuse.value.copy(t.color),e.opacity.value=t.opacity,e.rotation.value=t.rotation,t.map&&(e.map.value=t.map,n(t.map,e.mapTransform)),t.alphaMap&&(e.alphaMap.value=t.alphaMap,n(t.alphaMap,e.alphaMapTransform)),t.alphaTest>0&&(e.alphaTest.value=t.alphaTest)}function u(e,t){e.specular.value.copy(t.specular),e.shininess.value=Math.max(t.shininess,1e-4)}function d(e,t){t.gradientMap&&(e.gradientMap.value=t.gradientMap)}function f(e,t){e.metalness.value=t.metalness,t.metalnessMap&&(e.metalnessMap.value=t.metalnessMap,n(t.metalnessMap,e.metalnessMapTransform)),e.roughness.value=t.roughness,t.roughnessMap&&(e.roughnessMap.value=t.roughnessMap,n(t.roughnessMap,e.roughnessMapTransform)),t.envMap&&(e.envMapIntensity.value=t.envMapIntensity)}function p(e,t,r){e.ior.value=t.ior,t.sheen>0&&(e.sheenColor.value.copy(t.sheenColor).multiplyScalar(t.sheen),e.sheenRoughness.value=t.sheenRoughness,t.sheenColorMap&&(e.sheenColorMap.value=t.sheenColorMap,n(t.sheenColorMap,e.sheenColorMapTransform)),t.sheenRoughnessMap&&(e.sheenRoughnessMap.value=t.sheenRoughnessMap,n(t.sheenRoughnessMap,e.sheenRoughnessMapTransform))),t.clearcoat>0&&(e.clearcoat.value=t.clearcoat,e.clearcoatRoughness.value=t.clearcoatRoughness,t.clearcoatMap&&(e.clearcoatMap.value=t.clearcoatMap,n(t.clearcoatMap,e.clearcoatMapTransform)),t.clearcoatRoughnessMap&&(e.clearcoatRoughnessMap.value=t.clearcoatRoughnessMap,n(t.clearcoatRoughnessMap,e.clearcoatRoughnessMapTransform)),t.clearcoatNormalMap&&(e.clearcoatNormalMap.value=t.clearcoatNormalMap,n(t.clearcoatNormalMap,e.clearcoatNormalMapTransform),e.clearcoatNormalScale.value.copy(t.clearcoatNormalScale),t.side===1&&e.clearcoatNormalScale.value.negate())),t.dispersion>0&&(e.dispersion.value=t.dispersion),t.iridescence>0&&(e.iridescence.value=t.iridescence,e.iridescenceIOR.value=t.iridescenceIOR,e.iridescenceThicknessMinimum.value=t.iridescenceThicknessRange[0],e.iridescenceThicknessMaximum.value=t.iridescenceThicknessRange[1],t.iridescenceMap&&(e.iridescenceMap.value=t.iridescenceMap,n(t.iridescenceMap,e.iridescenceMapTransform)),t.iridescenceThicknessMap&&(e.iridescenceThicknessMap.value=t.iridescenceThicknessMap,n(t.iridescenceThicknessMap,e.iridescenceThicknessMapTransform))),t.transmission>0&&(e.transmission.value=t.transmission,e.transmissionSamplerMap.value=r.texture,e.transmissionSamplerSize.value.set(r.width,r.height),t.transmissionMap&&(e.transmissionMap.value=t.transmissionMap,n(t.transmissionMap,e.transmissionMapTransform)),e.thickness.value=t.thickness,t.thicknessMap&&(e.thicknessMap.value=t.thicknessMap,n(t.thicknessMap,e.thicknessMapTransform)),e.attenuationDistance.value=t.attenuationDistance,e.attenuationColor.value.copy(t.attenuationColor)),t.anisotropy>0&&(e.anisotropyVector.value.set(t.anisotropy*Math.cos(t.anisotropyRotation),t.anisotropy*Math.sin(t.anisotropyRotation)),t.anisotropyMap&&(e.anisotropyMap.value=t.anisotropyMap,n(t.anisotropyMap,e.anisotropyMapTransform))),e.specularIntensity.value=t.specularIntensity,e.specularColor.value.copy(t.specularColor),t.specularColorMap&&(e.specularColorMap.value=t.specularColorMap,n(t.specularColorMap,e.specularColorMapTransform)),t.specularIntensityMap&&(e.specularIntensityMap.value=t.specularIntensityMap,n(t.specularIntensityMap,e.specularIntensityMapTransform))}function m(e,t){t.matcap&&(e.matcap.value=t.matcap)}function h(e,n){let r=t.get(n).light;e.referencePosition.value.setFromMatrixPosition(r.matrixWorld),e.nearDistance.value=r.shadow.camera.near,e.farDistance.value=r.shadow.camera.far}return{refreshFogUniforms:r,refreshMaterialUniforms:i}}function Ju(e,t,n,r){let i={},a={},o=[],s=e.getParameter(e.MAX_UNIFORM_BUFFER_BINDINGS);function c(e,t){let n=t.program;r.uniformBlockBinding(e,n)}function l(e,n){let o=i[e.id];o===void 0&&(m(e),o=u(e),i[e.id]=o,e.addEventListener(`dispose`,g));let s=n.program;r.updateUBOMapping(e,s);let c=t.render.frame;a[e.id]!==c&&(f(e),a[e.id]=c)}function u(t){let n=d();t.__bindingPointIndex=n;let r=e.createBuffer(),i=t.__size,a=t.usage;return e.bindBuffer(e.UNIFORM_BUFFER,r),e.bufferData(e.UNIFORM_BUFFER,i,a),e.bindBuffer(e.UNIFORM_BUFFER,null),e.bindBufferBase(e.UNIFORM_BUFFER,n,r),r}function d(){for(let e=0;e<s;e++)if(o.indexOf(e)===-1)return o.push(e),e;return G(`WebGLRenderer: Maximum number of simultaneously usable uniforms groups reached.`),0}function f(t){let n=i[t.id],r=t.uniforms,a=t.__cache;e.bindBuffer(e.UNIFORM_BUFFER,n);for(let t=0,n=r.length;t<n;t++){let n=Array.isArray(r[t])?r[t]:[r[t]];for(let r=0,i=n.length;r<i;r++){let i=n[r];if(p(i,t,r,a)===!0){let t=i.__offset,n=Array.isArray(i.value)?i.value:[i.value],r=0;for(let a=0;a<n.length;a++){let o=n[a],s=h(o);typeof o==`number`||typeof o==`boolean`?(i.__data[0]=o,e.bufferSubData(e.UNIFORM_BUFFER,t+r,i.__data)):o.isMatrix3?(i.__data[0]=o.elements[0],i.__data[1]=o.elements[1],i.__data[2]=o.elements[2],i.__data[3]=0,i.__data[4]=o.elements[3],i.__data[5]=o.elements[4],i.__data[6]=o.elements[5],i.__data[7]=0,i.__data[8]=o.elements[6],i.__data[9]=o.elements[7],i.__data[10]=o.elements[8],i.__data[11]=0):ArrayBuffer.isView(o)?i.__data.set(new o.constructor(o.buffer,o.byteOffset,i.__data.length)):(o.toArray(i.__data,r),r+=s.storage/Float32Array.BYTES_PER_ELEMENT)}e.bufferSubData(e.UNIFORM_BUFFER,t,i.__data)}}}e.bindBuffer(e.UNIFORM_BUFFER,null)}function p(e,t,n,r){let i=e.value,a=t+`_`+n;if(r[a]===void 0)return typeof i==`number`||typeof i==`boolean`?r[a]=i:ArrayBuffer.isView(i)?r[a]=i.slice():r[a]=i.clone(),!0;{let e=r[a];if(typeof i==`number`||typeof i==`boolean`){if(e!==i)return r[a]=i,!0}else if(ArrayBuffer.isView(i))return!0;else if(e.equals(i)===!1)return e.copy(i),!0}return!1}function m(e){let t=e.uniforms,n=0;for(let e=0,r=t.length;e<r;e++){let r=Array.isArray(t[e])?t[e]:[t[e]];for(let e=0,t=r.length;e<t;e++){let t=r[e],i=Array.isArray(t.value)?t.value:[t.value];for(let e=0,r=i.length;e<r;e++){let r=i[e],a=h(r),o=n%16,s=o%a.boundary,c=o+s;n+=s,c!==0&&16-c<a.storage&&(n+=16-c),t.__data=new Float32Array(a.storage/Float32Array.BYTES_PER_ELEMENT),t.__offset=n,n+=a.storage}}}let r=n%16;return r>0&&(n+=16-r),e.__size=n,e.__cache={},this}function h(e){let t={boundary:0,storage:0};return typeof e==`number`||typeof e==`boolean`?(t.boundary=4,t.storage=4):e.isVector2?(t.boundary=8,t.storage=8):e.isVector3||e.isColor?(t.boundary=16,t.storage=12):e.isVector4?(t.boundary=16,t.storage=16):e.isMatrix3?(t.boundary=48,t.storage=48):e.isMatrix4?(t.boundary=64,t.storage=64):e.isTexture?W(`WebGLRenderer: Texture samplers can not be part of an uniforms group.`):ArrayBuffer.isView(e)?(t.boundary=16,t.storage=e.byteLength):W(`WebGLRenderer: Unsupported uniform value type.`,e),t}function g(t){let n=t.target;n.removeEventListener(`dispose`,g);let r=o.indexOf(n.__bindingPointIndex);o.splice(r,1),e.deleteBuffer(i[n.id]),delete i[n.id],delete a[n.id]}function _(){for(let t in i)e.deleteBuffer(i[t]);o=[],i={},a={}}return{bind:c,update:l,dispose:_}}function Yu(){return of===null&&(of=new Bo(af,16,16,$n,Bn),of.name=`DFG_LUT`,of.minFilter=An,of.magFilter=An,of.wrapS=Tn,of.wrapT=Tn,of.generateMipmaps=!1,of.needsUpdate=!0),of}var Z,Q,Xu,Zu,Qu,$u,ed,td,nd,rd,id,ad,od,sd,cd,ld,ud,dd,fd,pd,md,hd,gd,_d,vd,yd,bd,xd,Sd,Cd,wd,Td,Ed,Dd,Od,kd,Ad,jd,Md,Nd,Pd,Fd,Id,Ld,Rd,zd,Bd,Vd,Hd,Ud,Wd,Gd,Kd,qd,Jd,Yd,Xd,Zd,Qd,$d,ef,tf,nf,rf,af,of,sf,cf=e((()=>{Wc(),Z={alphahash_fragment:`#ifdef USE_ALPHAHASH
	if ( diffuseColor.a < getAlphaHashThreshold( vPosition ) ) discard;
#endif`,alphahash_pars_fragment:`#ifdef USE_ALPHAHASH
	const float ALPHA_HASH_SCALE = 0.05;
	float hash2D( vec2 value ) {
		return fract( 1.0e4 * sin( 17.0 * value.x + 0.1 * value.y ) * ( 0.1 + abs( sin( 13.0 * value.y + value.x ) ) ) );
	}
	float hash3D( vec3 value ) {
		return hash2D( vec2( hash2D( value.xy ), value.z ) );
	}
	float getAlphaHashThreshold( vec3 position ) {
		float maxDeriv = max(
			length( dFdx( position.xyz ) ),
			length( dFdy( position.xyz ) )
		);
		float pixScale = 1.0 / ( ALPHA_HASH_SCALE * maxDeriv );
		vec2 pixScales = vec2(
			exp2( floor( log2( pixScale ) ) ),
			exp2( ceil( log2( pixScale ) ) )
		);
		vec2 alpha = vec2(
			hash3D( floor( pixScales.x * position.xyz ) ),
			hash3D( floor( pixScales.y * position.xyz ) )
		);
		float lerpFactor = fract( log2( pixScale ) );
		float x = ( 1.0 - lerpFactor ) * alpha.x + lerpFactor * alpha.y;
		float a = min( lerpFactor, 1.0 - lerpFactor );
		vec3 cases = vec3(
			x * x / ( 2.0 * a * ( 1.0 - a ) ),
			( x - 0.5 * a ) / ( 1.0 - a ),
			1.0 - ( ( 1.0 - x ) * ( 1.0 - x ) / ( 2.0 * a * ( 1.0 - a ) ) )
		);
		float threshold = ( x < ( 1.0 - a ) )
			? ( ( x < a ) ? cases.x : cases.y )
			: cases.z;
		return clamp( threshold , 1.0e-6, 1.0 );
	}
#endif`,alphamap_fragment:`#ifdef USE_ALPHAMAP
	diffuseColor.a *= texture2D( alphaMap, vAlphaMapUv ).g;
#endif`,alphamap_pars_fragment:`#ifdef USE_ALPHAMAP
	uniform sampler2D alphaMap;
#endif`,alphatest_fragment:`#ifdef USE_ALPHATEST
	#ifdef ALPHA_TO_COVERAGE
	diffuseColor.a = smoothstep( alphaTest, alphaTest + fwidth( diffuseColor.a ), diffuseColor.a );
	if ( diffuseColor.a == 0.0 ) discard;
	#else
	if ( diffuseColor.a < alphaTest ) discard;
	#endif
#endif`,alphatest_pars_fragment:`#ifdef USE_ALPHATEST
	uniform float alphaTest;
#endif`,aomap_fragment:`#ifdef USE_AOMAP
	float ambientOcclusion = ( texture2D( aoMap, vAoMapUv ).r - 1.0 ) * aoMapIntensity + 1.0;
	reflectedLight.indirectDiffuse *= ambientOcclusion;
	#if defined( USE_CLEARCOAT ) 
		clearcoatSpecularIndirect *= ambientOcclusion;
	#endif
	#if defined( USE_SHEEN ) 
		sheenSpecularIndirect *= ambientOcclusion;
	#endif
	#if defined( USE_ENVMAP ) && defined( STANDARD )
		float dotNV = saturate( dot( geometryNormal, geometryViewDir ) );
		reflectedLight.indirectSpecular *= computeSpecularOcclusion( dotNV, ambientOcclusion, material.roughness );
	#endif
#endif`,aomap_pars_fragment:`#ifdef USE_AOMAP
	uniform sampler2D aoMap;
	uniform float aoMapIntensity;
#endif`,batching_pars_vertex:`#ifdef USE_BATCHING
	#if ! defined( GL_ANGLE_multi_draw )
	#define gl_DrawID _gl_DrawID
	uniform int _gl_DrawID;
	#endif
	uniform highp sampler2D batchingTexture;
	uniform highp usampler2D batchingIdTexture;
	mat4 getBatchingMatrix( const in float i ) {
		int size = textureSize( batchingTexture, 0 ).x;
		int j = int( i ) * 4;
		int x = j % size;
		int y = j / size;
		vec4 v1 = texelFetch( batchingTexture, ivec2( x, y ), 0 );
		vec4 v2 = texelFetch( batchingTexture, ivec2( x + 1, y ), 0 );
		vec4 v3 = texelFetch( batchingTexture, ivec2( x + 2, y ), 0 );
		vec4 v4 = texelFetch( batchingTexture, ivec2( x + 3, y ), 0 );
		return mat4( v1, v2, v3, v4 );
	}
	float getIndirectIndex( const in int i ) {
		int size = textureSize( batchingIdTexture, 0 ).x;
		int x = i % size;
		int y = i / size;
		return float( texelFetch( batchingIdTexture, ivec2( x, y ), 0 ).r );
	}
#endif
#ifdef USE_BATCHING_COLOR
	uniform sampler2D batchingColorTexture;
	vec4 getBatchingColor( const in float i ) {
		int size = textureSize( batchingColorTexture, 0 ).x;
		int j = int( i );
		int x = j % size;
		int y = j / size;
		return texelFetch( batchingColorTexture, ivec2( x, y ), 0 );
	}
#endif`,batching_vertex:`#ifdef USE_BATCHING
	mat4 batchingMatrix = getBatchingMatrix( getIndirectIndex( gl_DrawID ) );
#endif`,begin_vertex:`vec3 transformed = vec3( position );
#ifdef USE_ALPHAHASH
	vPosition = vec3( position );
#endif`,beginnormal_vertex:`vec3 objectNormal = vec3( normal );
#ifdef USE_TANGENT
	vec3 objectTangent = vec3( tangent.xyz );
#endif`,bsdfs:`float G_BlinnPhong_Implicit( ) {
	return 0.25;
}
float D_BlinnPhong( const in float shininess, const in float dotNH ) {
	return RECIPROCAL_PI * ( shininess * 0.5 + 1.0 ) * pow( dotNH, shininess );
}
vec3 BRDF_BlinnPhong( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in vec3 specularColor, const in float shininess ) {
	vec3 halfDir = normalize( lightDir + viewDir );
	float dotNH = saturate( dot( normal, halfDir ) );
	float dotVH = saturate( dot( viewDir, halfDir ) );
	vec3 F = F_Schlick( specularColor, 1.0, dotVH );
	float G = G_BlinnPhong_Implicit( );
	float D = D_BlinnPhong( shininess, dotNH );
	return F * ( G * D );
} // validated`,iridescence_fragment:`#ifdef USE_IRIDESCENCE
	const mat3 XYZ_TO_REC709 = mat3(
		 3.2404542, -0.9692660,  0.0556434,
		-1.5371385,  1.8760108, -0.2040259,
		-0.4985314,  0.0415560,  1.0572252
	);
	vec3 Fresnel0ToIor( vec3 fresnel0 ) {
		vec3 sqrtF0 = sqrt( fresnel0 );
		return ( vec3( 1.0 ) + sqrtF0 ) / ( vec3( 1.0 ) - sqrtF0 );
	}
	vec3 IorToFresnel0( vec3 transmittedIor, float incidentIor ) {
		return pow2( ( transmittedIor - vec3( incidentIor ) ) / ( transmittedIor + vec3( incidentIor ) ) );
	}
	float IorToFresnel0( float transmittedIor, float incidentIor ) {
		return pow2( ( transmittedIor - incidentIor ) / ( transmittedIor + incidentIor ));
	}
	vec3 evalSensitivity( float OPD, vec3 shift ) {
		float phase = 2.0 * PI * OPD * 1.0e-9;
		vec3 val = vec3( 5.4856e-13, 4.4201e-13, 5.2481e-13 );
		vec3 pos = vec3( 1.6810e+06, 1.7953e+06, 2.2084e+06 );
		vec3 var = vec3( 4.3278e+09, 9.3046e+09, 6.6121e+09 );
		vec3 xyz = val * sqrt( 2.0 * PI * var ) * cos( pos * phase + shift ) * exp( - pow2( phase ) * var );
		xyz.x += 9.7470e-14 * sqrt( 2.0 * PI * 4.5282e+09 ) * cos( 2.2399e+06 * phase + shift[ 0 ] ) * exp( - 4.5282e+09 * pow2( phase ) );
		xyz /= 1.0685e-7;
		vec3 rgb = XYZ_TO_REC709 * xyz;
		return rgb;
	}
	vec3 evalIridescence( float outsideIOR, float eta2, float cosTheta1, float thinFilmThickness, vec3 baseF0 ) {
		vec3 I;
		float iridescenceIOR = mix( outsideIOR, eta2, smoothstep( 0.0, 0.03, thinFilmThickness ) );
		float sinTheta2Sq = pow2( outsideIOR / iridescenceIOR ) * ( 1.0 - pow2( cosTheta1 ) );
		float cosTheta2Sq = 1.0 - sinTheta2Sq;
		if ( cosTheta2Sq < 0.0 ) {
			return vec3( 1.0 );
		}
		float cosTheta2 = sqrt( cosTheta2Sq );
		float R0 = IorToFresnel0( iridescenceIOR, outsideIOR );
		float R12 = F_Schlick( R0, 1.0, cosTheta1 );
		float T121 = 1.0 - R12;
		float phi12 = 0.0;
		if ( iridescenceIOR < outsideIOR ) phi12 = PI;
		float phi21 = PI - phi12;
		vec3 baseIOR = Fresnel0ToIor( clamp( baseF0, 0.0, 0.9999 ) );		vec3 R1 = IorToFresnel0( baseIOR, iridescenceIOR );
		vec3 R23 = F_Schlick( R1, 1.0, cosTheta2 );
		vec3 phi23 = vec3( 0.0 );
		if ( baseIOR[ 0 ] < iridescenceIOR ) phi23[ 0 ] = PI;
		if ( baseIOR[ 1 ] < iridescenceIOR ) phi23[ 1 ] = PI;
		if ( baseIOR[ 2 ] < iridescenceIOR ) phi23[ 2 ] = PI;
		float OPD = 2.0 * iridescenceIOR * thinFilmThickness * cosTheta2;
		vec3 phi = vec3( phi21 ) + phi23;
		vec3 R123 = clamp( R12 * R23, 1e-5, 0.9999 );
		vec3 r123 = sqrt( R123 );
		vec3 Rs = pow2( T121 ) * R23 / ( vec3( 1.0 ) - R123 );
		vec3 C0 = R12 + Rs;
		I = C0;
		vec3 Cm = Rs - T121;
		for ( int m = 1; m <= 2; ++ m ) {
			Cm *= r123;
			vec3 Sm = 2.0 * evalSensitivity( float( m ) * OPD, float( m ) * phi );
			I += Cm * Sm;
		}
		return max( I, vec3( 0.0 ) );
	}
#endif`,bumpmap_pars_fragment:`#ifdef USE_BUMPMAP
	uniform sampler2D bumpMap;
	uniform float bumpScale;
	vec2 dHdxy_fwd() {
		vec2 dSTdx = dFdx( vBumpMapUv );
		vec2 dSTdy = dFdy( vBumpMapUv );
		float Hll = bumpScale * texture2D( bumpMap, vBumpMapUv ).x;
		float dBx = bumpScale * texture2D( bumpMap, vBumpMapUv + dSTdx ).x - Hll;
		float dBy = bumpScale * texture2D( bumpMap, vBumpMapUv + dSTdy ).x - Hll;
		return vec2( dBx, dBy );
	}
	vec3 perturbNormalArb( vec3 surf_pos, vec3 surf_norm, vec2 dHdxy, float faceDirection ) {
		vec3 vSigmaX = normalize( dFdx( surf_pos.xyz ) );
		vec3 vSigmaY = normalize( dFdy( surf_pos.xyz ) );
		vec3 vN = surf_norm;
		vec3 R1 = cross( vSigmaY, vN );
		vec3 R2 = cross( vN, vSigmaX );
		float fDet = dot( vSigmaX, R1 ) * faceDirection;
		vec3 vGrad = sign( fDet ) * ( dHdxy.x * R1 + dHdxy.y * R2 );
		return normalize( abs( fDet ) * surf_norm - vGrad );
	}
#endif`,clipping_planes_fragment:`#if NUM_CLIPPING_PLANES > 0
	vec4 plane;
	#ifdef ALPHA_TO_COVERAGE
		float distanceToPlane, distanceGradient;
		float clipOpacity = 1.0;
		#pragma unroll_loop_start
		for ( int i = 0; i < UNION_CLIPPING_PLANES; i ++ ) {
			plane = clippingPlanes[ i ];
			distanceToPlane = - dot( vClipPosition, plane.xyz ) + plane.w;
			distanceGradient = fwidth( distanceToPlane ) / 2.0;
			clipOpacity *= smoothstep( - distanceGradient, distanceGradient, distanceToPlane );
			if ( clipOpacity == 0.0 ) discard;
		}
		#pragma unroll_loop_end
		#if UNION_CLIPPING_PLANES < NUM_CLIPPING_PLANES
			float unionClipOpacity = 1.0;
			#pragma unroll_loop_start
			for ( int i = UNION_CLIPPING_PLANES; i < NUM_CLIPPING_PLANES; i ++ ) {
				plane = clippingPlanes[ i ];
				distanceToPlane = - dot( vClipPosition, plane.xyz ) + plane.w;
				distanceGradient = fwidth( distanceToPlane ) / 2.0;
				unionClipOpacity *= 1.0 - smoothstep( - distanceGradient, distanceGradient, distanceToPlane );
			}
			#pragma unroll_loop_end
			clipOpacity *= 1.0 - unionClipOpacity;
		#endif
		diffuseColor.a *= clipOpacity;
		if ( diffuseColor.a == 0.0 ) discard;
	#else
		#pragma unroll_loop_start
		for ( int i = 0; i < UNION_CLIPPING_PLANES; i ++ ) {
			plane = clippingPlanes[ i ];
			if ( dot( vClipPosition, plane.xyz ) > plane.w ) discard;
		}
		#pragma unroll_loop_end
		#if UNION_CLIPPING_PLANES < NUM_CLIPPING_PLANES
			bool clipped = true;
			#pragma unroll_loop_start
			for ( int i = UNION_CLIPPING_PLANES; i < NUM_CLIPPING_PLANES; i ++ ) {
				plane = clippingPlanes[ i ];
				clipped = ( dot( vClipPosition, plane.xyz ) > plane.w ) && clipped;
			}
			#pragma unroll_loop_end
			if ( clipped ) discard;
		#endif
	#endif
#endif`,clipping_planes_pars_fragment:`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
	uniform vec4 clippingPlanes[ NUM_CLIPPING_PLANES ];
#endif`,clipping_planes_pars_vertex:`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
#endif`,clipping_planes_vertex:`#if NUM_CLIPPING_PLANES > 0
	vClipPosition = - mvPosition.xyz;
#endif`,color_fragment:`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	diffuseColor *= vColor;
#endif`,color_pars_fragment:`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	varying vec4 vColor;
#endif`,color_pars_vertex:`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
	varying vec4 vColor;
#endif`,color_vertex:`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
	vColor = vec4( 1.0 );
#endif
#ifdef USE_COLOR_ALPHA
	vColor *= color;
#elif defined( USE_COLOR )
	vColor.rgb *= color;
#endif
#ifdef USE_INSTANCING_COLOR
	vColor.rgb *= instanceColor.rgb;
#endif
#ifdef USE_BATCHING_COLOR
	vColor *= getBatchingColor( getIndirectIndex( gl_DrawID ) );
#endif`,common:`#define PI 3.141592653589793
#define PI2 6.283185307179586
#define PI_HALF 1.5707963267948966
#define RECIPROCAL_PI 0.3183098861837907
#define RECIPROCAL_PI2 0.15915494309189535
#define EPSILON 1e-6
#ifndef saturate
#define saturate( a ) clamp( a, 0.0, 1.0 )
#endif
#define whiteComplement( a ) ( 1.0 - saturate( a ) )
float pow2( const in float x ) { return x*x; }
vec3 pow2( const in vec3 x ) { return x*x; }
float pow3( const in float x ) { return x*x*x; }
float pow4( const in float x ) { float x2 = x*x; return x2*x2; }
float max3( const in vec3 v ) { return max( max( v.x, v.y ), v.z ); }
float average( const in vec3 v ) { return dot( v, vec3( 0.3333333 ) ); }
highp float rand( const in vec2 uv ) {
	const highp float a = 12.9898, b = 78.233, c = 43758.5453;
	highp float dt = dot( uv.xy, vec2( a,b ) ), sn = mod( dt, PI );
	return fract( sin( sn ) * c );
}
#ifdef HIGH_PRECISION
	float precisionSafeLength( vec3 v ) { return length( v ); }
#else
	float precisionSafeLength( vec3 v ) {
		float maxComponent = max3( abs( v ) );
		return length( v / maxComponent ) * maxComponent;
	}
#endif
struct IncidentLight {
	vec3 color;
	vec3 direction;
	bool visible;
};
struct ReflectedLight {
	vec3 directDiffuse;
	vec3 directSpecular;
	vec3 indirectDiffuse;
	vec3 indirectSpecular;
};
#ifdef USE_ALPHAHASH
	varying vec3 vPosition;
#endif
vec3 transformDirection( in vec3 dir, in mat4 matrix ) {
	return normalize( ( matrix * vec4( dir, 0.0 ) ).xyz );
}
vec3 inverseTransformDirection( in vec3 dir, in mat4 matrix ) {
	return normalize( ( vec4( dir, 0.0 ) * matrix ).xyz );
}
bool isPerspectiveMatrix( mat4 m ) {
	return m[ 2 ][ 3 ] == - 1.0;
}
vec2 equirectUv( in vec3 dir ) {
	float u = atan( dir.z, dir.x ) * RECIPROCAL_PI2 + 0.5;
	float v = asin( clamp( dir.y, - 1.0, 1.0 ) ) * RECIPROCAL_PI + 0.5;
	return vec2( u, v );
}
vec3 BRDF_Lambert( const in vec3 diffuseColor ) {
	return RECIPROCAL_PI * diffuseColor;
}
vec3 F_Schlick( const in vec3 f0, const in float f90, const in float dotVH ) {
	float fresnel = exp2( ( - 5.55473 * dotVH - 6.98316 ) * dotVH );
	return f0 * ( 1.0 - fresnel ) + ( f90 * fresnel );
}
float F_Schlick( const in float f0, const in float f90, const in float dotVH ) {
	float fresnel = exp2( ( - 5.55473 * dotVH - 6.98316 ) * dotVH );
	return f0 * ( 1.0 - fresnel ) + ( f90 * fresnel );
} // validated`,cube_uv_reflection_fragment:`#ifdef ENVMAP_TYPE_CUBE_UV
	#define cubeUV_minMipLevel 4.0
	#define cubeUV_minTileSize 16.0
	float getFace( vec3 direction ) {
		vec3 absDirection = abs( direction );
		float face = - 1.0;
		if ( absDirection.x > absDirection.z ) {
			if ( absDirection.x > absDirection.y )
				face = direction.x > 0.0 ? 0.0 : 3.0;
			else
				face = direction.y > 0.0 ? 1.0 : 4.0;
		} else {
			if ( absDirection.z > absDirection.y )
				face = direction.z > 0.0 ? 2.0 : 5.0;
			else
				face = direction.y > 0.0 ? 1.0 : 4.0;
		}
		return face;
	}
	vec2 getUV( vec3 direction, float face ) {
		vec2 uv;
		if ( face == 0.0 ) {
			uv = vec2( direction.z, direction.y ) / abs( direction.x );
		} else if ( face == 1.0 ) {
			uv = vec2( - direction.x, - direction.z ) / abs( direction.y );
		} else if ( face == 2.0 ) {
			uv = vec2( - direction.x, direction.y ) / abs( direction.z );
		} else if ( face == 3.0 ) {
			uv = vec2( - direction.z, direction.y ) / abs( direction.x );
		} else if ( face == 4.0 ) {
			uv = vec2( - direction.x, direction.z ) / abs( direction.y );
		} else {
			uv = vec2( direction.x, direction.y ) / abs( direction.z );
		}
		return 0.5 * ( uv + 1.0 );
	}
	vec3 bilinearCubeUV( sampler2D envMap, vec3 direction, float mipInt ) {
		float face = getFace( direction );
		float filterInt = max( cubeUV_minMipLevel - mipInt, 0.0 );
		mipInt = max( mipInt, cubeUV_minMipLevel );
		float faceSize = exp2( mipInt );
		highp vec2 uv = getUV( direction, face ) * ( faceSize - 2.0 ) + 1.0;
		if ( face > 2.0 ) {
			uv.y += faceSize;
			face -= 3.0;
		}
		uv.x += face * faceSize;
		uv.x += filterInt * 3.0 * cubeUV_minTileSize;
		uv.y += 4.0 * ( exp2( CUBEUV_MAX_MIP ) - faceSize );
		uv.x *= CUBEUV_TEXEL_WIDTH;
		uv.y *= CUBEUV_TEXEL_HEIGHT;
		#ifdef texture2DGradEXT
			return texture2DGradEXT( envMap, uv, vec2( 0.0 ), vec2( 0.0 ) ).rgb;
		#else
			return texture2D( envMap, uv ).rgb;
		#endif
	}
	#define cubeUV_r0 1.0
	#define cubeUV_m0 - 2.0
	#define cubeUV_r1 0.8
	#define cubeUV_m1 - 1.0
	#define cubeUV_r4 0.4
	#define cubeUV_m4 2.0
	#define cubeUV_r5 0.305
	#define cubeUV_m5 3.0
	#define cubeUV_r6 0.21
	#define cubeUV_m6 4.0
	float roughnessToMip( float roughness ) {
		float mip = 0.0;
		if ( roughness >= cubeUV_r1 ) {
			mip = ( cubeUV_r0 - roughness ) * ( cubeUV_m1 - cubeUV_m0 ) / ( cubeUV_r0 - cubeUV_r1 ) + cubeUV_m0;
		} else if ( roughness >= cubeUV_r4 ) {
			mip = ( cubeUV_r1 - roughness ) * ( cubeUV_m4 - cubeUV_m1 ) / ( cubeUV_r1 - cubeUV_r4 ) + cubeUV_m1;
		} else if ( roughness >= cubeUV_r5 ) {
			mip = ( cubeUV_r4 - roughness ) * ( cubeUV_m5 - cubeUV_m4 ) / ( cubeUV_r4 - cubeUV_r5 ) + cubeUV_m4;
		} else if ( roughness >= cubeUV_r6 ) {
			mip = ( cubeUV_r5 - roughness ) * ( cubeUV_m6 - cubeUV_m5 ) / ( cubeUV_r5 - cubeUV_r6 ) + cubeUV_m5;
		} else {
			mip = - 2.0 * log2( 1.16 * roughness );		}
		return mip;
	}
	vec4 textureCubeUV( sampler2D envMap, vec3 sampleDir, float roughness ) {
		float mip = clamp( roughnessToMip( roughness ), cubeUV_m0, CUBEUV_MAX_MIP );
		float mipF = fract( mip );
		float mipInt = floor( mip );
		vec3 color0 = bilinearCubeUV( envMap, sampleDir, mipInt );
		if ( mipF == 0.0 ) {
			return vec4( color0, 1.0 );
		} else {
			vec3 color1 = bilinearCubeUV( envMap, sampleDir, mipInt + 1.0 );
			return vec4( mix( color0, color1, mipF ), 1.0 );
		}
	}
#endif`,defaultnormal_vertex:`vec3 transformedNormal = objectNormal;
#ifdef USE_TANGENT
	vec3 transformedTangent = objectTangent;
#endif
#ifdef USE_BATCHING
	mat3 bm = mat3( batchingMatrix );
	transformedNormal /= vec3( dot( bm[ 0 ], bm[ 0 ] ), dot( bm[ 1 ], bm[ 1 ] ), dot( bm[ 2 ], bm[ 2 ] ) );
	transformedNormal = bm * transformedNormal;
	#ifdef USE_TANGENT
		transformedTangent = bm * transformedTangent;
	#endif
#endif
#ifdef USE_INSTANCING
	mat3 im = mat3( instanceMatrix );
	transformedNormal /= vec3( dot( im[ 0 ], im[ 0 ] ), dot( im[ 1 ], im[ 1 ] ), dot( im[ 2 ], im[ 2 ] ) );
	transformedNormal = im * transformedNormal;
	#ifdef USE_TANGENT
		transformedTangent = im * transformedTangent;
	#endif
#endif
transformedNormal = normalMatrix * transformedNormal;
#ifdef FLIP_SIDED
	transformedNormal = - transformedNormal;
#endif
#ifdef USE_TANGENT
	transformedTangent = ( modelViewMatrix * vec4( transformedTangent, 0.0 ) ).xyz;
	#ifdef FLIP_SIDED
		transformedTangent = - transformedTangent;
	#endif
#endif`,displacementmap_pars_vertex:`#ifdef USE_DISPLACEMENTMAP
	uniform sampler2D displacementMap;
	uniform float displacementScale;
	uniform float displacementBias;
#endif`,displacementmap_vertex:`#ifdef USE_DISPLACEMENTMAP
	transformed += normalize( objectNormal ) * ( texture2D( displacementMap, vDisplacementMapUv ).x * displacementScale + displacementBias );
#endif`,emissivemap_fragment:`#ifdef USE_EMISSIVEMAP
	vec4 emissiveColor = texture2D( emissiveMap, vEmissiveMapUv );
	#ifdef DECODE_VIDEO_TEXTURE_EMISSIVE
		emissiveColor = sRGBTransferEOTF( emissiveColor );
	#endif
	totalEmissiveRadiance *= emissiveColor.rgb;
#endif`,emissivemap_pars_fragment:`#ifdef USE_EMISSIVEMAP
	uniform sampler2D emissiveMap;
#endif`,colorspace_fragment:`gl_FragColor = linearToOutputTexel( gl_FragColor );`,colorspace_pars_fragment:`vec4 LinearTransferOETF( in vec4 value ) {
	return value;
}
vec4 sRGBTransferEOTF( in vec4 value ) {
	return vec4( mix( pow( value.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), value.rgb * 0.0773993808, vec3( lessThanEqual( value.rgb, vec3( 0.04045 ) ) ) ), value.a );
}
vec4 sRGBTransferOETF( in vec4 value ) {
	return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );
}`,envmap_fragment:`#ifdef USE_ENVMAP
	#ifdef ENV_WORLDPOS
		vec3 cameraToFrag;
		if ( isOrthographic ) {
			cameraToFrag = normalize( vec3( - viewMatrix[ 0 ][ 2 ], - viewMatrix[ 1 ][ 2 ], - viewMatrix[ 2 ][ 2 ] ) );
		} else {
			cameraToFrag = normalize( vWorldPosition - cameraPosition );
		}
		vec3 worldNormal = inverseTransformDirection( normal, viewMatrix );
		#ifdef ENVMAP_MODE_REFLECTION
			vec3 reflectVec = reflect( cameraToFrag, worldNormal );
		#else
			vec3 reflectVec = refract( cameraToFrag, worldNormal, refractionRatio );
		#endif
	#else
		vec3 reflectVec = vReflect;
	#endif
	#ifdef ENVMAP_TYPE_CUBE
		vec4 envColor = textureCube( envMap, envMapRotation * reflectVec );
		#ifdef ENVMAP_BLENDING_MULTIPLY
			outgoingLight = mix( outgoingLight, outgoingLight * envColor.xyz, specularStrength * reflectivity );
		#elif defined( ENVMAP_BLENDING_MIX )
			outgoingLight = mix( outgoingLight, envColor.xyz, specularStrength * reflectivity );
		#elif defined( ENVMAP_BLENDING_ADD )
			outgoingLight += envColor.xyz * specularStrength * reflectivity;
		#endif
	#endif
#endif`,envmap_common_pars_fragment:`#ifdef USE_ENVMAP
	uniform float envMapIntensity;
	uniform mat3 envMapRotation;
	#ifdef ENVMAP_TYPE_CUBE
		uniform samplerCube envMap;
	#else
		uniform sampler2D envMap;
	#endif
#endif`,envmap_pars_fragment:`#ifdef USE_ENVMAP
	uniform float reflectivity;
	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )
		#define ENV_WORLDPOS
	#endif
	#ifdef ENV_WORLDPOS
		varying vec3 vWorldPosition;
		uniform float refractionRatio;
	#else
		varying vec3 vReflect;
	#endif
#endif`,envmap_pars_vertex:`#ifdef USE_ENVMAP
	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )
		#define ENV_WORLDPOS
	#endif
	#ifdef ENV_WORLDPOS
		
		varying vec3 vWorldPosition;
	#else
		varying vec3 vReflect;
		uniform float refractionRatio;
	#endif
#endif`,envmap_physical_pars_fragment:`#ifdef USE_ENVMAP
	vec3 getIBLIrradiance( const in vec3 normal ) {
		#ifdef ENVMAP_TYPE_CUBE_UV
			vec3 worldNormal = inverseTransformDirection( normal, viewMatrix );
			vec4 envMapColor = textureCubeUV( envMap, envMapRotation * worldNormal, 1.0 );
			return PI * envMapColor.rgb * envMapIntensity;
		#else
			return vec3( 0.0 );
		#endif
	}
	vec3 getIBLRadiance( const in vec3 viewDir, const in vec3 normal, const in float roughness ) {
		#ifdef ENVMAP_TYPE_CUBE_UV
			vec3 reflectVec = reflect( - viewDir, normal );
			reflectVec = normalize( mix( reflectVec, normal, pow4( roughness ) ) );
			reflectVec = inverseTransformDirection( reflectVec, viewMatrix );
			vec4 envMapColor = textureCubeUV( envMap, envMapRotation * reflectVec, roughness );
			return envMapColor.rgb * envMapIntensity;
		#else
			return vec3( 0.0 );
		#endif
	}
	#ifdef USE_ANISOTROPY
		vec3 getIBLAnisotropyRadiance( const in vec3 viewDir, const in vec3 normal, const in float roughness, const in vec3 bitangent, const in float anisotropy ) {
			#ifdef ENVMAP_TYPE_CUBE_UV
				vec3 bentNormal = cross( bitangent, viewDir );
				bentNormal = normalize( cross( bentNormal, bitangent ) );
				bentNormal = normalize( mix( bentNormal, normal, pow2( pow2( 1.0 - anisotropy * ( 1.0 - roughness ) ) ) ) );
				return getIBLRadiance( viewDir, bentNormal, roughness );
			#else
				return vec3( 0.0 );
			#endif
		}
	#endif
#endif`,envmap_vertex:`#ifdef USE_ENVMAP
	#ifdef ENV_WORLDPOS
		vWorldPosition = worldPosition.xyz;
	#else
		vec3 cameraToVertex;
		if ( isOrthographic ) {
			cameraToVertex = normalize( vec3( - viewMatrix[ 0 ][ 2 ], - viewMatrix[ 1 ][ 2 ], - viewMatrix[ 2 ][ 2 ] ) );
		} else {
			cameraToVertex = normalize( worldPosition.xyz - cameraPosition );
		}
		vec3 worldNormal = inverseTransformDirection( transformedNormal, viewMatrix );
		#ifdef ENVMAP_MODE_REFLECTION
			vReflect = reflect( cameraToVertex, worldNormal );
		#else
			vReflect = refract( cameraToVertex, worldNormal, refractionRatio );
		#endif
	#endif
#endif`,fog_vertex:`#ifdef USE_FOG
	vFogDepth = - mvPosition.z;
#endif`,fog_pars_vertex:`#ifdef USE_FOG
	varying float vFogDepth;
#endif`,fog_fragment:`#ifdef USE_FOG
	#ifdef FOG_EXP2
		float fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
	#else
		float fogFactor = smoothstep( fogNear, fogFar, vFogDepth );
	#endif
	gl_FragColor.rgb = mix( gl_FragColor.rgb, fogColor, fogFactor );
#endif`,fog_pars_fragment:`#ifdef USE_FOG
	uniform vec3 fogColor;
	varying float vFogDepth;
	#ifdef FOG_EXP2
		uniform float fogDensity;
	#else
		uniform float fogNear;
		uniform float fogFar;
	#endif
#endif`,gradientmap_pars_fragment:`#ifdef USE_GRADIENTMAP
	uniform sampler2D gradientMap;
#endif
vec3 getGradientIrradiance( vec3 normal, vec3 lightDirection ) {
	float dotNL = dot( normal, lightDirection );
	vec2 coord = vec2( dotNL * 0.5 + 0.5, 0.0 );
	#ifdef USE_GRADIENTMAP
		return vec3( texture2D( gradientMap, coord ).r );
	#else
		vec2 fw = fwidth( coord ) * 0.5;
		return mix( vec3( 0.7 ), vec3( 1.0 ), smoothstep( 0.7 - fw.x, 0.7 + fw.x, coord.x ) );
	#endif
}`,lightmap_pars_fragment:`#ifdef USE_LIGHTMAP
	uniform sampler2D lightMap;
	uniform float lightMapIntensity;
#endif`,lights_lambert_fragment:`LambertMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularStrength = specularStrength;`,lights_lambert_pars_fragment:`varying vec3 vViewPosition;
struct LambertMaterial {
	vec3 diffuseColor;
	float specularStrength;
};
void RE_Direct_Lambert( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in LambertMaterial material, inout ReflectedLight reflectedLight ) {
	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );
	vec3 irradiance = dotNL * directLight.color;
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
void RE_IndirectDiffuse_Lambert( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in LambertMaterial material, inout ReflectedLight reflectedLight ) {
	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
#define RE_Direct				RE_Direct_Lambert
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Lambert`,lights_pars_begin:`uniform bool receiveShadow;
uniform vec3 ambientLightColor;
#if defined( USE_LIGHT_PROBES )
	uniform vec3 lightProbe[ 9 ];
#endif
vec3 shGetIrradianceAt( in vec3 normal, in vec3 shCoefficients[ 9 ] ) {
	float x = normal.x, y = normal.y, z = normal.z;
	vec3 result = shCoefficients[ 0 ] * 0.886227;
	result += shCoefficients[ 1 ] * 2.0 * 0.511664 * y;
	result += shCoefficients[ 2 ] * 2.0 * 0.511664 * z;
	result += shCoefficients[ 3 ] * 2.0 * 0.511664 * x;
	result += shCoefficients[ 4 ] * 2.0 * 0.429043 * x * y;
	result += shCoefficients[ 5 ] * 2.0 * 0.429043 * y * z;
	result += shCoefficients[ 6 ] * ( 0.743125 * z * z - 0.247708 );
	result += shCoefficients[ 7 ] * 2.0 * 0.429043 * x * z;
	result += shCoefficients[ 8 ] * 0.429043 * ( x * x - y * y );
	return result;
}
vec3 getLightProbeIrradiance( const in vec3 lightProbe[ 9 ], const in vec3 normal ) {
	vec3 worldNormal = inverseTransformDirection( normal, viewMatrix );
	vec3 irradiance = shGetIrradianceAt( worldNormal, lightProbe );
	return irradiance;
}
vec3 getAmbientLightIrradiance( const in vec3 ambientLightColor ) {
	vec3 irradiance = ambientLightColor;
	return irradiance;
}
float getDistanceAttenuation( const in float lightDistance, const in float cutoffDistance, const in float decayExponent ) {
	float distanceFalloff = 1.0 / max( pow( lightDistance, decayExponent ), 0.01 );
	if ( cutoffDistance > 0.0 ) {
		distanceFalloff *= pow2( saturate( 1.0 - pow4( lightDistance / cutoffDistance ) ) );
	}
	return distanceFalloff;
}
float getSpotAttenuation( const in float coneCosine, const in float penumbraCosine, const in float angleCosine ) {
	return smoothstep( coneCosine, penumbraCosine, angleCosine );
}
#if NUM_DIR_LIGHTS > 0
	struct DirectionalLight {
		vec3 direction;
		vec3 color;
	};
	uniform DirectionalLight directionalLights[ NUM_DIR_LIGHTS ];
	void getDirectionalLightInfo( const in DirectionalLight directionalLight, out IncidentLight light ) {
		light.color = directionalLight.color;
		light.direction = directionalLight.direction;
		light.visible = true;
	}
#endif
#if NUM_POINT_LIGHTS > 0
	struct PointLight {
		vec3 position;
		vec3 color;
		float distance;
		float decay;
	};
	uniform PointLight pointLights[ NUM_POINT_LIGHTS ];
	void getPointLightInfo( const in PointLight pointLight, const in vec3 geometryPosition, out IncidentLight light ) {
		vec3 lVector = pointLight.position - geometryPosition;
		light.direction = normalize( lVector );
		float lightDistance = length( lVector );
		light.color = pointLight.color;
		light.color *= getDistanceAttenuation( lightDistance, pointLight.distance, pointLight.decay );
		light.visible = ( light.color != vec3( 0.0 ) );
	}
#endif
#if NUM_SPOT_LIGHTS > 0
	struct SpotLight {
		vec3 position;
		vec3 direction;
		vec3 color;
		float distance;
		float decay;
		float coneCos;
		float penumbraCos;
	};
	uniform SpotLight spotLights[ NUM_SPOT_LIGHTS ];
	void getSpotLightInfo( const in SpotLight spotLight, const in vec3 geometryPosition, out IncidentLight light ) {
		vec3 lVector = spotLight.position - geometryPosition;
		light.direction = normalize( lVector );
		float angleCos = dot( light.direction, spotLight.direction );
		float spotAttenuation = getSpotAttenuation( spotLight.coneCos, spotLight.penumbraCos, angleCos );
		if ( spotAttenuation > 0.0 ) {
			float lightDistance = length( lVector );
			light.color = spotLight.color * spotAttenuation;
			light.color *= getDistanceAttenuation( lightDistance, spotLight.distance, spotLight.decay );
			light.visible = ( light.color != vec3( 0.0 ) );
		} else {
			light.color = vec3( 0.0 );
			light.visible = false;
		}
	}
#endif
#if NUM_RECT_AREA_LIGHTS > 0
	struct RectAreaLight {
		vec3 color;
		vec3 position;
		vec3 halfWidth;
		vec3 halfHeight;
	};
	uniform sampler2D ltc_1;	uniform sampler2D ltc_2;
	uniform RectAreaLight rectAreaLights[ NUM_RECT_AREA_LIGHTS ];
#endif
#if NUM_HEMI_LIGHTS > 0
	struct HemisphereLight {
		vec3 direction;
		vec3 skyColor;
		vec3 groundColor;
	};
	uniform HemisphereLight hemisphereLights[ NUM_HEMI_LIGHTS ];
	vec3 getHemisphereLightIrradiance( const in HemisphereLight hemiLight, const in vec3 normal ) {
		float dotNL = dot( normal, hemiLight.direction );
		float hemiDiffuseWeight = 0.5 * dotNL + 0.5;
		vec3 irradiance = mix( hemiLight.groundColor, hemiLight.skyColor, hemiDiffuseWeight );
		return irradiance;
	}
#endif
#include <lightprobes_pars_fragment>`,lights_toon_fragment:`ToonMaterial material;
material.diffuseColor = diffuseColor.rgb;`,lights_toon_pars_fragment:`varying vec3 vViewPosition;
struct ToonMaterial {
	vec3 diffuseColor;
};
void RE_Direct_Toon( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in ToonMaterial material, inout ReflectedLight reflectedLight ) {
	vec3 irradiance = getGradientIrradiance( geometryNormal, directLight.direction ) * directLight.color;
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
void RE_IndirectDiffuse_Toon( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in ToonMaterial material, inout ReflectedLight reflectedLight ) {
	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
#define RE_Direct				RE_Direct_Toon
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Toon`,lights_phong_fragment:`BlinnPhongMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularColor = specular;
material.specularShininess = shininess;
material.specularStrength = specularStrength;`,lights_phong_pars_fragment:`varying vec3 vViewPosition;
struct BlinnPhongMaterial {
	vec3 diffuseColor;
	vec3 specularColor;
	float specularShininess;
	float specularStrength;
};
void RE_Direct_BlinnPhong( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in BlinnPhongMaterial material, inout ReflectedLight reflectedLight ) {
	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );
	vec3 irradiance = dotNL * directLight.color;
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
	reflectedLight.directSpecular += irradiance * BRDF_BlinnPhong( directLight.direction, geometryViewDir, geometryNormal, material.specularColor, material.specularShininess ) * material.specularStrength;
}
void RE_IndirectDiffuse_BlinnPhong( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in BlinnPhongMaterial material, inout ReflectedLight reflectedLight ) {
	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
#define RE_Direct				RE_Direct_BlinnPhong
#define RE_IndirectDiffuse		RE_IndirectDiffuse_BlinnPhong`,lights_physical_fragment:`PhysicalMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.diffuseContribution = diffuseColor.rgb * ( 1.0 - metalnessFactor );
material.metalness = metalnessFactor;
vec3 dxy = max( abs( dFdx( nonPerturbedNormal ) ), abs( dFdy( nonPerturbedNormal ) ) );
float geometryRoughness = max( max( dxy.x, dxy.y ), dxy.z );
material.roughness = max( roughnessFactor, 0.0525 );material.roughness += geometryRoughness;
material.roughness = min( material.roughness, 1.0 );
#ifdef IOR
	material.ior = ior;
	#ifdef USE_SPECULAR
		float specularIntensityFactor = specularIntensity;
		vec3 specularColorFactor = specularColor;
		#ifdef USE_SPECULAR_COLORMAP
			specularColorFactor *= texture2D( specularColorMap, vSpecularColorMapUv ).rgb;
		#endif
		#ifdef USE_SPECULAR_INTENSITYMAP
			specularIntensityFactor *= texture2D( specularIntensityMap, vSpecularIntensityMapUv ).a;
		#endif
		material.specularF90 = mix( specularIntensityFactor, 1.0, metalnessFactor );
	#else
		float specularIntensityFactor = 1.0;
		vec3 specularColorFactor = vec3( 1.0 );
		material.specularF90 = 1.0;
	#endif
	material.specularColor = min( pow2( ( material.ior - 1.0 ) / ( material.ior + 1.0 ) ) * specularColorFactor, vec3( 1.0 ) ) * specularIntensityFactor;
	material.specularColorBlended = mix( material.specularColor, diffuseColor.rgb, metalnessFactor );
#else
	material.specularColor = vec3( 0.04 );
	material.specularColorBlended = mix( material.specularColor, diffuseColor.rgb, metalnessFactor );
	material.specularF90 = 1.0;
#endif
#ifdef USE_CLEARCOAT
	material.clearcoat = clearcoat;
	material.clearcoatRoughness = clearcoatRoughness;
	material.clearcoatF0 = vec3( 0.04 );
	material.clearcoatF90 = 1.0;
	#ifdef USE_CLEARCOATMAP
		material.clearcoat *= texture2D( clearcoatMap, vClearcoatMapUv ).x;
	#endif
	#ifdef USE_CLEARCOAT_ROUGHNESSMAP
		material.clearcoatRoughness *= texture2D( clearcoatRoughnessMap, vClearcoatRoughnessMapUv ).y;
	#endif
	material.clearcoat = saturate( material.clearcoat );	material.clearcoatRoughness = max( material.clearcoatRoughness, 0.0525 );
	material.clearcoatRoughness += geometryRoughness;
	material.clearcoatRoughness = min( material.clearcoatRoughness, 1.0 );
#endif
#ifdef USE_DISPERSION
	material.dispersion = dispersion;
#endif
#ifdef USE_IRIDESCENCE
	material.iridescence = iridescence;
	material.iridescenceIOR = iridescenceIOR;
	#ifdef USE_IRIDESCENCEMAP
		material.iridescence *= texture2D( iridescenceMap, vIridescenceMapUv ).r;
	#endif
	#ifdef USE_IRIDESCENCE_THICKNESSMAP
		material.iridescenceThickness = (iridescenceThicknessMaximum - iridescenceThicknessMinimum) * texture2D( iridescenceThicknessMap, vIridescenceThicknessMapUv ).g + iridescenceThicknessMinimum;
	#else
		material.iridescenceThickness = iridescenceThicknessMaximum;
	#endif
#endif
#ifdef USE_SHEEN
	material.sheenColor = sheenColor;
	#ifdef USE_SHEEN_COLORMAP
		material.sheenColor *= texture2D( sheenColorMap, vSheenColorMapUv ).rgb;
	#endif
	material.sheenRoughness = clamp( sheenRoughness, 0.0001, 1.0 );
	#ifdef USE_SHEEN_ROUGHNESSMAP
		material.sheenRoughness *= texture2D( sheenRoughnessMap, vSheenRoughnessMapUv ).a;
	#endif
#endif
#ifdef USE_ANISOTROPY
	#ifdef USE_ANISOTROPYMAP
		mat2 anisotropyMat = mat2( anisotropyVector.x, anisotropyVector.y, - anisotropyVector.y, anisotropyVector.x );
		vec3 anisotropyPolar = texture2D( anisotropyMap, vAnisotropyMapUv ).rgb;
		vec2 anisotropyV = anisotropyMat * normalize( 2.0 * anisotropyPolar.rg - vec2( 1.0 ) ) * anisotropyPolar.b;
	#else
		vec2 anisotropyV = anisotropyVector;
	#endif
	material.anisotropy = length( anisotropyV );
	if( material.anisotropy == 0.0 ) {
		anisotropyV = vec2( 1.0, 0.0 );
	} else {
		anisotropyV /= material.anisotropy;
		material.anisotropy = saturate( material.anisotropy );
	}
	material.alphaT = mix( pow2( material.roughness ), 1.0, pow2( material.anisotropy ) );
	material.anisotropyT = tbn[ 0 ] * anisotropyV.x + tbn[ 1 ] * anisotropyV.y;
	material.anisotropyB = tbn[ 1 ] * anisotropyV.x - tbn[ 0 ] * anisotropyV.y;
#endif`,lights_physical_pars_fragment:`uniform sampler2D dfgLUT;
struct PhysicalMaterial {
	vec3 diffuseColor;
	vec3 diffuseContribution;
	vec3 specularColor;
	vec3 specularColorBlended;
	float roughness;
	float metalness;
	float specularF90;
	float dispersion;
	#ifdef USE_CLEARCOAT
		float clearcoat;
		float clearcoatRoughness;
		vec3 clearcoatF0;
		float clearcoatF90;
	#endif
	#ifdef USE_IRIDESCENCE
		float iridescence;
		float iridescenceIOR;
		float iridescenceThickness;
		vec3 iridescenceFresnel;
		vec3 iridescenceF0;
		vec3 iridescenceFresnelDielectric;
		vec3 iridescenceFresnelMetallic;
	#endif
	#ifdef USE_SHEEN
		vec3 sheenColor;
		float sheenRoughness;
	#endif
	#ifdef IOR
		float ior;
	#endif
	#ifdef USE_TRANSMISSION
		float transmission;
		float transmissionAlpha;
		float thickness;
		float attenuationDistance;
		vec3 attenuationColor;
	#endif
	#ifdef USE_ANISOTROPY
		float anisotropy;
		float alphaT;
		vec3 anisotropyT;
		vec3 anisotropyB;
	#endif
};
vec3 clearcoatSpecularDirect = vec3( 0.0 );
vec3 clearcoatSpecularIndirect = vec3( 0.0 );
vec3 sheenSpecularDirect = vec3( 0.0 );
vec3 sheenSpecularIndirect = vec3(0.0 );
vec3 Schlick_to_F0( const in vec3 f, const in float f90, const in float dotVH ) {
    float x = clamp( 1.0 - dotVH, 0.0, 1.0 );
    float x2 = x * x;
    float x5 = clamp( x * x2 * x2, 0.0, 0.9999 );
    return ( f - vec3( f90 ) * x5 ) / ( 1.0 - x5 );
}
float V_GGX_SmithCorrelated( const in float alpha, const in float dotNL, const in float dotNV ) {
	float a2 = pow2( alpha );
	float gv = dotNL * sqrt( a2 + ( 1.0 - a2 ) * pow2( dotNV ) );
	float gl = dotNV * sqrt( a2 + ( 1.0 - a2 ) * pow2( dotNL ) );
	return 0.5 / max( gv + gl, EPSILON );
}
float D_GGX( const in float alpha, const in float dotNH ) {
	float a2 = pow2( alpha );
	float denom = pow2( dotNH ) * ( a2 - 1.0 ) + 1.0;
	return RECIPROCAL_PI * a2 / pow2( denom );
}
#ifdef USE_ANISOTROPY
	float V_GGX_SmithCorrelated_Anisotropic( const in float alphaT, const in float alphaB, const in float dotTV, const in float dotBV, const in float dotTL, const in float dotBL, const in float dotNV, const in float dotNL ) {
		float gv = dotNL * length( vec3( alphaT * dotTV, alphaB * dotBV, dotNV ) );
		float gl = dotNV * length( vec3( alphaT * dotTL, alphaB * dotBL, dotNL ) );
		return 0.5 / max( gv + gl, EPSILON );
	}
	float D_GGX_Anisotropic( const in float alphaT, const in float alphaB, const in float dotNH, const in float dotTH, const in float dotBH ) {
		float a2 = alphaT * alphaB;
		highp vec3 v = vec3( alphaB * dotTH, alphaT * dotBH, a2 * dotNH );
		highp float v2 = dot( v, v );
		float w2 = a2 / v2;
		return RECIPROCAL_PI * a2 * pow2 ( w2 );
	}
#endif
#ifdef USE_CLEARCOAT
	vec3 BRDF_GGX_Clearcoat( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material) {
		vec3 f0 = material.clearcoatF0;
		float f90 = material.clearcoatF90;
		float roughness = material.clearcoatRoughness;
		float alpha = pow2( roughness );
		vec3 halfDir = normalize( lightDir + viewDir );
		float dotNL = saturate( dot( normal, lightDir ) );
		float dotNV = saturate( dot( normal, viewDir ) );
		float dotNH = saturate( dot( normal, halfDir ) );
		float dotVH = saturate( dot( viewDir, halfDir ) );
		vec3 F = F_Schlick( f0, f90, dotVH );
		float V = V_GGX_SmithCorrelated( alpha, dotNL, dotNV );
		float D = D_GGX( alpha, dotNH );
		return F * ( V * D );
	}
#endif
vec3 BRDF_GGX( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material ) {
	vec3 f0 = material.specularColorBlended;
	float f90 = material.specularF90;
	float roughness = material.roughness;
	float alpha = pow2( roughness );
	vec3 halfDir = normalize( lightDir + viewDir );
	float dotNL = saturate( dot( normal, lightDir ) );
	float dotNV = saturate( dot( normal, viewDir ) );
	float dotNH = saturate( dot( normal, halfDir ) );
	float dotVH = saturate( dot( viewDir, halfDir ) );
	vec3 F = F_Schlick( f0, f90, dotVH );
	#ifdef USE_IRIDESCENCE
		F = mix( F, material.iridescenceFresnel, material.iridescence );
	#endif
	#ifdef USE_ANISOTROPY
		float dotTL = dot( material.anisotropyT, lightDir );
		float dotTV = dot( material.anisotropyT, viewDir );
		float dotTH = dot( material.anisotropyT, halfDir );
		float dotBL = dot( material.anisotropyB, lightDir );
		float dotBV = dot( material.anisotropyB, viewDir );
		float dotBH = dot( material.anisotropyB, halfDir );
		float V = V_GGX_SmithCorrelated_Anisotropic( material.alphaT, alpha, dotTV, dotBV, dotTL, dotBL, dotNV, dotNL );
		float D = D_GGX_Anisotropic( material.alphaT, alpha, dotNH, dotTH, dotBH );
	#else
		float V = V_GGX_SmithCorrelated( alpha, dotNL, dotNV );
		float D = D_GGX( alpha, dotNH );
	#endif
	return F * ( V * D );
}
vec2 LTC_Uv( const in vec3 N, const in vec3 V, const in float roughness ) {
	const float LUT_SIZE = 64.0;
	const float LUT_SCALE = ( LUT_SIZE - 1.0 ) / LUT_SIZE;
	const float LUT_BIAS = 0.5 / LUT_SIZE;
	float dotNV = saturate( dot( N, V ) );
	vec2 uv = vec2( roughness, sqrt( 1.0 - dotNV ) );
	uv = uv * LUT_SCALE + LUT_BIAS;
	return uv;
}
float LTC_ClippedSphereFormFactor( const in vec3 f ) {
	float l = length( f );
	return max( ( l * l + f.z ) / ( l + 1.0 ), 0.0 );
}
vec3 LTC_EdgeVectorFormFactor( const in vec3 v1, const in vec3 v2 ) {
	float x = dot( v1, v2 );
	float y = abs( x );
	float a = 0.8543985 + ( 0.4965155 + 0.0145206 * y ) * y;
	float b = 3.4175940 + ( 4.1616724 + y ) * y;
	float v = a / b;
	float theta_sintheta = ( x > 0.0 ) ? v : 0.5 * inversesqrt( max( 1.0 - x * x, 1e-7 ) ) - v;
	return cross( v1, v2 ) * theta_sintheta;
}
vec3 LTC_Evaluate( const in vec3 N, const in vec3 V, const in vec3 P, const in mat3 mInv, const in vec3 rectCoords[ 4 ] ) {
	vec3 v1 = rectCoords[ 1 ] - rectCoords[ 0 ];
	vec3 v2 = rectCoords[ 3 ] - rectCoords[ 0 ];
	vec3 lightNormal = cross( v1, v2 );
	if( dot( lightNormal, P - rectCoords[ 0 ] ) < 0.0 ) return vec3( 0.0 );
	vec3 T1, T2;
	T1 = normalize( V - N * dot( V, N ) );
	T2 = - cross( N, T1 );
	mat3 mat = mInv * transpose( mat3( T1, T2, N ) );
	vec3 coords[ 4 ];
	coords[ 0 ] = mat * ( rectCoords[ 0 ] - P );
	coords[ 1 ] = mat * ( rectCoords[ 1 ] - P );
	coords[ 2 ] = mat * ( rectCoords[ 2 ] - P );
	coords[ 3 ] = mat * ( rectCoords[ 3 ] - P );
	coords[ 0 ] = normalize( coords[ 0 ] );
	coords[ 1 ] = normalize( coords[ 1 ] );
	coords[ 2 ] = normalize( coords[ 2 ] );
	coords[ 3 ] = normalize( coords[ 3 ] );
	vec3 vectorFormFactor = vec3( 0.0 );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 0 ], coords[ 1 ] );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 1 ], coords[ 2 ] );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 2 ], coords[ 3 ] );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 3 ], coords[ 0 ] );
	float result = LTC_ClippedSphereFormFactor( vectorFormFactor );
	return vec3( result );
}
#if defined( USE_SHEEN )
float D_Charlie( float roughness, float dotNH ) {
	float alpha = pow2( roughness );
	float invAlpha = 1.0 / alpha;
	float cos2h = dotNH * dotNH;
	float sin2h = max( 1.0 - cos2h, 0.0078125 );
	return ( 2.0 + invAlpha ) * pow( sin2h, invAlpha * 0.5 ) / ( 2.0 * PI );
}
float V_Neubelt( float dotNV, float dotNL ) {
	return saturate( 1.0 / ( 4.0 * ( dotNL + dotNV - dotNL * dotNV ) ) );
}
vec3 BRDF_Sheen( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, vec3 sheenColor, const in float sheenRoughness ) {
	vec3 halfDir = normalize( lightDir + viewDir );
	float dotNL = saturate( dot( normal, lightDir ) );
	float dotNV = saturate( dot( normal, viewDir ) );
	float dotNH = saturate( dot( normal, halfDir ) );
	float D = D_Charlie( sheenRoughness, dotNH );
	float V = V_Neubelt( dotNV, dotNL );
	return sheenColor * ( D * V );
}
#endif
float IBLSheenBRDF( const in vec3 normal, const in vec3 viewDir, const in float roughness ) {
	float dotNV = saturate( dot( normal, viewDir ) );
	float r2 = roughness * roughness;
	float rInv = 1.0 / ( roughness + 0.1 );
	float a = -1.9362 + 1.0678 * roughness + 0.4573 * r2 - 0.8469 * rInv;
	float b = -0.6014 + 0.5538 * roughness - 0.4670 * r2 - 0.1255 * rInv;
	float DG = exp( a * dotNV + b );
	return saturate( DG );
}
vec3 EnvironmentBRDF( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float roughness ) {
	float dotNV = saturate( dot( normal, viewDir ) );
	vec2 fab = texture2D( dfgLUT, vec2( roughness, dotNV ) ).rg;
	return specularColor * fab.x + specularF90 * fab.y;
}
#ifdef USE_IRIDESCENCE
void computeMultiscatteringIridescence( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float iridescence, const in vec3 iridescenceF0, const in float roughness, inout vec3 singleScatter, inout vec3 multiScatter ) {
#else
void computeMultiscattering( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float roughness, inout vec3 singleScatter, inout vec3 multiScatter ) {
#endif
	float dotNV = saturate( dot( normal, viewDir ) );
	vec2 fab = texture2D( dfgLUT, vec2( roughness, dotNV ) ).rg;
	#ifdef USE_IRIDESCENCE
		vec3 Fr = mix( specularColor, iridescenceF0, iridescence );
	#else
		vec3 Fr = specularColor;
	#endif
	vec3 FssEss = Fr * fab.x + specularF90 * fab.y;
	float Ess = fab.x + fab.y;
	float Ems = 1.0 - Ess;
	vec3 Favg = Fr + ( 1.0 - Fr ) * 0.047619;	vec3 Fms = FssEss * Favg / ( 1.0 - Ems * Favg );
	singleScatter += FssEss;
	multiScatter += Fms * Ems;
}
vec3 BRDF_GGX_Multiscatter( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material ) {
	vec3 singleScatter = BRDF_GGX( lightDir, viewDir, normal, material );
	float dotNL = saturate( dot( normal, lightDir ) );
	float dotNV = saturate( dot( normal, viewDir ) );
	vec2 dfgV = texture2D( dfgLUT, vec2( material.roughness, dotNV ) ).rg;
	vec2 dfgL = texture2D( dfgLUT, vec2( material.roughness, dotNL ) ).rg;
	vec3 FssEss_V = material.specularColorBlended * dfgV.x + material.specularF90 * dfgV.y;
	vec3 FssEss_L = material.specularColorBlended * dfgL.x + material.specularF90 * dfgL.y;
	float Ess_V = dfgV.x + dfgV.y;
	float Ess_L = dfgL.x + dfgL.y;
	float Ems_V = 1.0 - Ess_V;
	float Ems_L = 1.0 - Ess_L;
	vec3 Favg = material.specularColorBlended + ( 1.0 - material.specularColorBlended ) * 0.047619;
	vec3 Fms = FssEss_V * FssEss_L * Favg / ( 1.0 - Ems_V * Ems_L * Favg + EPSILON );
	float compensationFactor = Ems_V * Ems_L;
	vec3 multiScatter = Fms * compensationFactor;
	return singleScatter + multiScatter;
}
#if NUM_RECT_AREA_LIGHTS > 0
	void RE_Direct_RectArea_Physical( const in RectAreaLight rectAreaLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {
		vec3 normal = geometryNormal;
		vec3 viewDir = geometryViewDir;
		vec3 position = geometryPosition;
		vec3 lightPos = rectAreaLight.position;
		vec3 halfWidth = rectAreaLight.halfWidth;
		vec3 halfHeight = rectAreaLight.halfHeight;
		vec3 lightColor = rectAreaLight.color;
		float roughness = material.roughness;
		vec3 rectCoords[ 4 ];
		rectCoords[ 0 ] = lightPos + halfWidth - halfHeight;		rectCoords[ 1 ] = lightPos - halfWidth - halfHeight;
		rectCoords[ 2 ] = lightPos - halfWidth + halfHeight;
		rectCoords[ 3 ] = lightPos + halfWidth + halfHeight;
		vec2 uv = LTC_Uv( normal, viewDir, roughness );
		vec4 t1 = texture2D( ltc_1, uv );
		vec4 t2 = texture2D( ltc_2, uv );
		mat3 mInv = mat3(
			vec3( t1.x, 0, t1.y ),
			vec3(    0, 1,    0 ),
			vec3( t1.z, 0, t1.w )
		);
		vec3 fresnel = ( material.specularColorBlended * t2.x + ( material.specularF90 - material.specularColorBlended ) * t2.y );
		reflectedLight.directSpecular += lightColor * fresnel * LTC_Evaluate( normal, viewDir, position, mInv, rectCoords );
		reflectedLight.directDiffuse += lightColor * material.diffuseContribution * LTC_Evaluate( normal, viewDir, position, mat3( 1.0 ), rectCoords );
		#ifdef USE_CLEARCOAT
			vec3 Ncc = geometryClearcoatNormal;
			vec2 uvClearcoat = LTC_Uv( Ncc, viewDir, material.clearcoatRoughness );
			vec4 t1Clearcoat = texture2D( ltc_1, uvClearcoat );
			vec4 t2Clearcoat = texture2D( ltc_2, uvClearcoat );
			mat3 mInvClearcoat = mat3(
				vec3( t1Clearcoat.x, 0, t1Clearcoat.y ),
				vec3(             0, 1,             0 ),
				vec3( t1Clearcoat.z, 0, t1Clearcoat.w )
			);
			vec3 fresnelClearcoat = material.clearcoatF0 * t2Clearcoat.x + ( material.clearcoatF90 - material.clearcoatF0 ) * t2Clearcoat.y;
			clearcoatSpecularDirect += lightColor * fresnelClearcoat * LTC_Evaluate( Ncc, viewDir, position, mInvClearcoat, rectCoords );
		#endif
	}
#endif
void RE_Direct_Physical( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {
	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );
	vec3 irradiance = dotNL * directLight.color;
	#ifdef USE_CLEARCOAT
		float dotNLcc = saturate( dot( geometryClearcoatNormal, directLight.direction ) );
		vec3 ccIrradiance = dotNLcc * directLight.color;
		clearcoatSpecularDirect += ccIrradiance * BRDF_GGX_Clearcoat( directLight.direction, geometryViewDir, geometryClearcoatNormal, material );
	#endif
	#ifdef USE_SHEEN
 
 		sheenSpecularDirect += irradiance * BRDF_Sheen( directLight.direction, geometryViewDir, geometryNormal, material.sheenColor, material.sheenRoughness );
 
 		float sheenAlbedoV = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );
 		float sheenAlbedoL = IBLSheenBRDF( geometryNormal, directLight.direction, material.sheenRoughness );
 
 		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * max( sheenAlbedoV, sheenAlbedoL );
 
 		irradiance *= sheenEnergyComp;
 
 	#endif
	reflectedLight.directSpecular += irradiance * BRDF_GGX_Multiscatter( directLight.direction, geometryViewDir, geometryNormal, material );
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseContribution );
}
void RE_IndirectDiffuse_Physical( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {
	vec3 diffuse = irradiance * BRDF_Lambert( material.diffuseContribution );
	#ifdef USE_SHEEN
		float sheenAlbedo = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );
		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * sheenAlbedo;
		diffuse *= sheenEnergyComp;
	#endif
	reflectedLight.indirectDiffuse += diffuse;
}
void RE_IndirectSpecular_Physical( const in vec3 radiance, const in vec3 irradiance, const in vec3 clearcoatRadiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight) {
	#ifdef USE_CLEARCOAT
		clearcoatSpecularIndirect += clearcoatRadiance * EnvironmentBRDF( geometryClearcoatNormal, geometryViewDir, material.clearcoatF0, material.clearcoatF90, material.clearcoatRoughness );
	#endif
	#ifdef USE_SHEEN
		sheenSpecularIndirect += irradiance * material.sheenColor * IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness ) * RECIPROCAL_PI;
 	#endif
	vec3 singleScatteringDielectric = vec3( 0.0 );
	vec3 multiScatteringDielectric = vec3( 0.0 );
	vec3 singleScatteringMetallic = vec3( 0.0 );
	vec3 multiScatteringMetallic = vec3( 0.0 );
	#ifdef USE_IRIDESCENCE
		computeMultiscatteringIridescence( geometryNormal, geometryViewDir, material.specularColor, material.specularF90, material.iridescence, material.iridescenceFresnelDielectric, material.roughness, singleScatteringDielectric, multiScatteringDielectric );
		computeMultiscatteringIridescence( geometryNormal, geometryViewDir, material.diffuseColor, material.specularF90, material.iridescence, material.iridescenceFresnelMetallic, material.roughness, singleScatteringMetallic, multiScatteringMetallic );
	#else
		computeMultiscattering( geometryNormal, geometryViewDir, material.specularColor, material.specularF90, material.roughness, singleScatteringDielectric, multiScatteringDielectric );
		computeMultiscattering( geometryNormal, geometryViewDir, material.diffuseColor, material.specularF90, material.roughness, singleScatteringMetallic, multiScatteringMetallic );
	#endif
	vec3 singleScattering = mix( singleScatteringDielectric, singleScatteringMetallic, material.metalness );
	vec3 multiScattering = mix( multiScatteringDielectric, multiScatteringMetallic, material.metalness );
	vec3 totalScatteringDielectric = singleScatteringDielectric + multiScatteringDielectric;
	vec3 diffuse = material.diffuseContribution * ( 1.0 - totalScatteringDielectric );
	vec3 cosineWeightedIrradiance = irradiance * RECIPROCAL_PI;
	vec3 indirectSpecular = radiance * singleScattering;
	indirectSpecular += multiScattering * cosineWeightedIrradiance;
	vec3 indirectDiffuse = diffuse * cosineWeightedIrradiance;
	#ifdef USE_SHEEN
		float sheenAlbedo = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );
		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * sheenAlbedo;
		indirectSpecular *= sheenEnergyComp;
		indirectDiffuse *= sheenEnergyComp;
	#endif
	reflectedLight.indirectSpecular += indirectSpecular;
	reflectedLight.indirectDiffuse += indirectDiffuse;
}
#define RE_Direct				RE_Direct_Physical
#define RE_Direct_RectArea		RE_Direct_RectArea_Physical
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Physical
#define RE_IndirectSpecular		RE_IndirectSpecular_Physical
float computeSpecularOcclusion( const in float dotNV, const in float ambientOcclusion, const in float roughness ) {
	return saturate( pow( dotNV + ambientOcclusion, exp2( - 16.0 * roughness - 1.0 ) ) - 1.0 + ambientOcclusion );
}`,lights_fragment_begin:`
vec3 geometryPosition = - vViewPosition;
vec3 geometryNormal = normal;
vec3 geometryViewDir = ( isOrthographic ) ? vec3( 0, 0, 1 ) : normalize( vViewPosition );
vec3 geometryClearcoatNormal = vec3( 0.0 );
#ifdef USE_CLEARCOAT
	geometryClearcoatNormal = clearcoatNormal;
#endif
#ifdef USE_IRIDESCENCE
	float dotNVi = saturate( dot( normal, geometryViewDir ) );
	if ( material.iridescenceThickness == 0.0 ) {
		material.iridescence = 0.0;
	} else {
		material.iridescence = saturate( material.iridescence );
	}
	if ( material.iridescence > 0.0 ) {
		material.iridescenceFresnelDielectric = evalIridescence( 1.0, material.iridescenceIOR, dotNVi, material.iridescenceThickness, material.specularColor );
		material.iridescenceFresnelMetallic = evalIridescence( 1.0, material.iridescenceIOR, dotNVi, material.iridescenceThickness, material.diffuseColor );
		material.iridescenceFresnel = mix( material.iridescenceFresnelDielectric, material.iridescenceFresnelMetallic, material.metalness );
		material.iridescenceF0 = Schlick_to_F0( material.iridescenceFresnel, 1.0, dotNVi );
	}
#endif
IncidentLight directLight;
#if ( NUM_POINT_LIGHTS > 0 ) && defined( RE_Direct )
	PointLight pointLight;
	#if defined( USE_SHADOWMAP ) && NUM_POINT_LIGHT_SHADOWS > 0
	PointLightShadow pointLightShadow;
	#endif
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_POINT_LIGHTS; i ++ ) {
		pointLight = pointLights[ i ];
		getPointLightInfo( pointLight, geometryPosition, directLight );
		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_POINT_LIGHT_SHADOWS ) && ( defined( SHADOWMAP_TYPE_PCF ) || defined( SHADOWMAP_TYPE_BASIC ) )
		pointLightShadow = pointLightShadows[ i ];
		directLight.color *= ( directLight.visible && receiveShadow ) ? getPointShadow( pointShadowMap[ i ], pointLightShadow.shadowMapSize, pointLightShadow.shadowIntensity, pointLightShadow.shadowBias, pointLightShadow.shadowRadius, vPointShadowCoord[ i ], pointLightShadow.shadowCameraNear, pointLightShadow.shadowCameraFar ) : 1.0;
		#endif
		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if ( NUM_SPOT_LIGHTS > 0 ) && defined( RE_Direct )
	SpotLight spotLight;
	vec4 spotColor;
	vec3 spotLightCoord;
	bool inSpotLightMap;
	#if defined( USE_SHADOWMAP ) && NUM_SPOT_LIGHT_SHADOWS > 0
	SpotLightShadow spotLightShadow;
	#endif
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_SPOT_LIGHTS; i ++ ) {
		spotLight = spotLights[ i ];
		getSpotLightInfo( spotLight, geometryPosition, directLight );
		#if ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS )
		#define SPOT_LIGHT_MAP_INDEX UNROLLED_LOOP_INDEX
		#elif ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )
		#define SPOT_LIGHT_MAP_INDEX NUM_SPOT_LIGHT_MAPS
		#else
		#define SPOT_LIGHT_MAP_INDEX ( UNROLLED_LOOP_INDEX - NUM_SPOT_LIGHT_SHADOWS + NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS )
		#endif
		#if ( SPOT_LIGHT_MAP_INDEX < NUM_SPOT_LIGHT_MAPS )
			spotLightCoord = vSpotLightCoord[ i ].xyz / vSpotLightCoord[ i ].w;
			inSpotLightMap = all( lessThan( abs( spotLightCoord * 2. - 1. ), vec3( 1.0 ) ) );
			spotColor = texture2D( spotLightMap[ SPOT_LIGHT_MAP_INDEX ], spotLightCoord.xy );
			directLight.color = inSpotLightMap ? directLight.color * spotColor.rgb : directLight.color;
		#endif
		#undef SPOT_LIGHT_MAP_INDEX
		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )
		spotLightShadow = spotLightShadows[ i ];
		directLight.color *= ( directLight.visible && receiveShadow ) ? getShadow( spotShadowMap[ i ], spotLightShadow.shadowMapSize, spotLightShadow.shadowIntensity, spotLightShadow.shadowBias, spotLightShadow.shadowRadius, vSpotLightCoord[ i ] ) : 1.0;
		#endif
		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if ( NUM_DIR_LIGHTS > 0 ) && defined( RE_Direct )
	DirectionalLight directionalLight;
	#if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
	DirectionalLightShadow directionalLightShadow;
	#endif
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_DIR_LIGHTS; i ++ ) {
		directionalLight = directionalLights[ i ];
		getDirectionalLightInfo( directionalLight, directLight );
		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_DIR_LIGHT_SHADOWS )
		directionalLightShadow = directionalLightShadows[ i ];
		directLight.color *= ( directLight.visible && receiveShadow ) ? getShadow( directionalShadowMap[ i ], directionalLightShadow.shadowMapSize, directionalLightShadow.shadowIntensity, directionalLightShadow.shadowBias, directionalLightShadow.shadowRadius, vDirectionalShadowCoord[ i ] ) : 1.0;
		#endif
		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if ( NUM_RECT_AREA_LIGHTS > 0 ) && defined( RE_Direct_RectArea )
	RectAreaLight rectAreaLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_RECT_AREA_LIGHTS; i ++ ) {
		rectAreaLight = rectAreaLights[ i ];
		RE_Direct_RectArea( rectAreaLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if defined( RE_IndirectDiffuse )
	vec3 iblIrradiance = vec3( 0.0 );
	vec3 irradiance = getAmbientLightIrradiance( ambientLightColor );
	#if defined( USE_LIGHT_PROBES )
		irradiance += getLightProbeIrradiance( lightProbe, geometryNormal );
	#endif
	#if ( NUM_HEMI_LIGHTS > 0 )
		#pragma unroll_loop_start
		for ( int i = 0; i < NUM_HEMI_LIGHTS; i ++ ) {
			irradiance += getHemisphereLightIrradiance( hemisphereLights[ i ], geometryNormal );
		}
		#pragma unroll_loop_end
	#endif
	#ifdef USE_LIGHT_PROBES_GRID
		vec3 probeWorldPos = ( ( vec4( geometryPosition, 1.0 ) - viewMatrix[ 3 ] ) * viewMatrix ).xyz;
		vec3 probeWorldNormal = inverseTransformDirection( geometryNormal, viewMatrix );
		irradiance += getLightProbeGridIrradiance( probeWorldPos, probeWorldNormal );
	#endif
#endif
#if defined( RE_IndirectSpecular )
	vec3 radiance = vec3( 0.0 );
	vec3 clearcoatRadiance = vec3( 0.0 );
#endif`,lights_fragment_maps:`#if defined( RE_IndirectDiffuse )
	#ifdef USE_LIGHTMAP
		vec4 lightMapTexel = texture2D( lightMap, vLightMapUv );
		vec3 lightMapIrradiance = lightMapTexel.rgb * lightMapIntensity;
		irradiance += lightMapIrradiance;
	#endif
	#if defined( USE_ENVMAP ) && defined( ENVMAP_TYPE_CUBE_UV )
		#if defined( STANDARD ) || defined( LAMBERT ) || defined( PHONG )
			iblIrradiance += getIBLIrradiance( geometryNormal );
		#endif
	#endif
#endif
#if defined( USE_ENVMAP ) && defined( RE_IndirectSpecular )
	#ifdef USE_ANISOTROPY
		radiance += getIBLAnisotropyRadiance( geometryViewDir, geometryNormal, material.roughness, material.anisotropyB, material.anisotropy );
	#else
		radiance += getIBLRadiance( geometryViewDir, geometryNormal, material.roughness );
	#endif
	#ifdef USE_CLEARCOAT
		clearcoatRadiance += getIBLRadiance( geometryViewDir, geometryClearcoatNormal, material.clearcoatRoughness );
	#endif
#endif`,lights_fragment_end:`#if defined( RE_IndirectDiffuse )
	#if defined( LAMBERT ) || defined( PHONG )
		irradiance += iblIrradiance;
	#endif
	RE_IndirectDiffuse( irradiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif
#if defined( RE_IndirectSpecular )
	RE_IndirectSpecular( radiance, iblIrradiance, clearcoatRadiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif`,lightprobes_pars_fragment:`#ifdef USE_LIGHT_PROBES_GRID
uniform highp sampler3D probesSH;
uniform vec3 probesMin;
uniform vec3 probesMax;
uniform vec3 probesResolution;
vec3 getLightProbeGridIrradiance( vec3 worldPos, vec3 worldNormal ) {
	vec3 res = probesResolution;
	vec3 gridRange = probesMax - probesMin;
	vec3 resMinusOne = res - 1.0;
	vec3 probeSpacing = gridRange / resMinusOne;
	vec3 samplePos = worldPos + worldNormal * probeSpacing * 0.5;
	vec3 uvw = clamp( ( samplePos - probesMin ) / gridRange, 0.0, 1.0 );
	uvw = uvw * resMinusOne / res + 0.5 / res;
	float nz          = res.z;
	float paddedSlices = nz + 2.0;
	float atlasDepth  = 7.0 * paddedSlices;
	float uvZBase     = uvw.z * nz + 1.0;
	vec4 s0 = texture( probesSH, vec3( uvw.xy, ( uvZBase                       ) / atlasDepth ) );
	vec4 s1 = texture( probesSH, vec3( uvw.xy, ( uvZBase +       paddedSlices   ) / atlasDepth ) );
	vec4 s2 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 2.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s3 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 3.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s4 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 4.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s5 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 5.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s6 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 6.0 * paddedSlices   ) / atlasDepth ) );
	vec3 c0 = s0.xyz;
	vec3 c1 = vec3( s0.w, s1.xy );
	vec3 c2 = vec3( s1.zw, s2.x );
	vec3 c3 = s2.yzw;
	vec3 c4 = s3.xyz;
	vec3 c5 = vec3( s3.w, s4.xy );
	vec3 c6 = vec3( s4.zw, s5.x );
	vec3 c7 = s5.yzw;
	vec3 c8 = s6.xyz;
	float x = worldNormal.x, y = worldNormal.y, z = worldNormal.z;
	vec3 result = c0 * 0.886227;
	result += c1 * 2.0 * 0.511664 * y;
	result += c2 * 2.0 * 0.511664 * z;
	result += c3 * 2.0 * 0.511664 * x;
	result += c4 * 2.0 * 0.429043 * x * y;
	result += c5 * 2.0 * 0.429043 * y * z;
	result += c6 * ( 0.743125 * z * z - 0.247708 );
	result += c7 * 2.0 * 0.429043 * x * z;
	result += c8 * 0.429043 * ( x * x - y * y );
	return max( result, vec3( 0.0 ) );
}
#endif`,logdepthbuf_fragment:`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	gl_FragDepth = vIsPerspective == 0.0 ? gl_FragCoord.z : log2( vFragDepth ) * logDepthBufFC * 0.5;
#endif`,logdepthbuf_pars_fragment:`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	uniform float logDepthBufFC;
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,logdepthbuf_pars_vertex:`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,logdepthbuf_vertex:`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	vFragDepth = 1.0 + gl_Position.w;
	vIsPerspective = float( isPerspectiveMatrix( projectionMatrix ) );
#endif`,map_fragment:`#ifdef USE_MAP
	vec4 sampledDiffuseColor = texture2D( map, vMapUv );
	#ifdef DECODE_VIDEO_TEXTURE
		sampledDiffuseColor = sRGBTransferEOTF( sampledDiffuseColor );
	#endif
	diffuseColor *= sampledDiffuseColor;
#endif`,map_pars_fragment:`#ifdef USE_MAP
	uniform sampler2D map;
#endif`,map_particle_fragment:`#if defined( USE_MAP ) || defined( USE_ALPHAMAP )
	#if defined( USE_POINTS_UV )
		vec2 uv = vUv;
	#else
		vec2 uv = ( uvTransform * vec3( gl_PointCoord.x, 1.0 - gl_PointCoord.y, 1 ) ).xy;
	#endif
#endif
#ifdef USE_MAP
	diffuseColor *= texture2D( map, uv );
#endif
#ifdef USE_ALPHAMAP
	diffuseColor.a *= texture2D( alphaMap, uv ).g;
#endif`,map_particle_pars_fragment:`#if defined( USE_POINTS_UV )
	varying vec2 vUv;
#else
	#if defined( USE_MAP ) || defined( USE_ALPHAMAP )
		uniform mat3 uvTransform;
	#endif
#endif
#ifdef USE_MAP
	uniform sampler2D map;
#endif
#ifdef USE_ALPHAMAP
	uniform sampler2D alphaMap;
#endif`,metalnessmap_fragment:`float metalnessFactor = metalness;
#ifdef USE_METALNESSMAP
	vec4 texelMetalness = texture2D( metalnessMap, vMetalnessMapUv );
	metalnessFactor *= texelMetalness.b;
#endif`,metalnessmap_pars_fragment:`#ifdef USE_METALNESSMAP
	uniform sampler2D metalnessMap;
#endif`,morphinstance_vertex:`#ifdef USE_INSTANCING_MORPH
	float morphTargetInfluences[ MORPHTARGETS_COUNT ];
	float morphTargetBaseInfluence = texelFetch( morphTexture, ivec2( 0, gl_InstanceID ), 0 ).r;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		morphTargetInfluences[i] =  texelFetch( morphTexture, ivec2( i + 1, gl_InstanceID ), 0 ).r;
	}
#endif`,morphcolor_vertex:`#if defined( USE_MORPHCOLORS )
	vColor *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		#if defined( USE_COLOR_ALPHA )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ) * morphTargetInfluences[ i ];
		#elif defined( USE_COLOR )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ).rgb * morphTargetInfluences[ i ];
		#endif
	}
#endif`,morphnormal_vertex:`#ifdef USE_MORPHNORMALS
	objectNormal *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) objectNormal += getMorph( gl_VertexID, i, 1 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,morphtarget_pars_vertex:`#ifdef USE_MORPHTARGETS
	#ifndef USE_INSTANCING_MORPH
		uniform float morphTargetBaseInfluence;
		uniform float morphTargetInfluences[ MORPHTARGETS_COUNT ];
	#endif
	uniform sampler2DArray morphTargetsTexture;
	uniform ivec2 morphTargetsTextureSize;
	vec4 getMorph( const in int vertexIndex, const in int morphTargetIndex, const in int offset ) {
		int texelIndex = vertexIndex * MORPHTARGETS_TEXTURE_STRIDE + offset;
		int y = texelIndex / morphTargetsTextureSize.x;
		int x = texelIndex - y * morphTargetsTextureSize.x;
		ivec3 morphUV = ivec3( x, y, morphTargetIndex );
		return texelFetch( morphTargetsTexture, morphUV, 0 );
	}
#endif`,morphtarget_vertex:`#ifdef USE_MORPHTARGETS
	transformed *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) transformed += getMorph( gl_VertexID, i, 0 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,normal_fragment_begin:`float faceDirection = gl_FrontFacing ? 1.0 : - 1.0;
#ifdef FLAT_SHADED
	vec3 fdx = dFdx( vViewPosition );
	vec3 fdy = dFdy( vViewPosition );
	vec3 normal = normalize( cross( fdx, fdy ) );
#else
	vec3 normal = normalize( vNormal );
	#ifdef DOUBLE_SIDED
		normal *= faceDirection;
	#endif
#endif
#if defined( USE_NORMALMAP_TANGENTSPACE ) || defined( USE_CLEARCOAT_NORMALMAP ) || defined( USE_ANISOTROPY )
	#ifdef USE_TANGENT
		mat3 tbn = mat3( normalize( vTangent ), normalize( vBitangent ), normal );
	#else
		mat3 tbn = getTangentFrame( - vViewPosition, normal,
		#if defined( USE_NORMALMAP )
			vNormalMapUv
		#elif defined( USE_CLEARCOAT_NORMALMAP )
			vClearcoatNormalMapUv
		#else
			vUv
		#endif
		);
	#endif
	#if defined( DOUBLE_SIDED ) && ! defined( FLAT_SHADED )
		tbn[0] *= faceDirection;
		tbn[1] *= faceDirection;
	#endif
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	#ifdef USE_TANGENT
		mat3 tbn2 = mat3( normalize( vTangent ), normalize( vBitangent ), normal );
	#else
		mat3 tbn2 = getTangentFrame( - vViewPosition, normal, vClearcoatNormalMapUv );
	#endif
	#if defined( DOUBLE_SIDED ) && ! defined( FLAT_SHADED )
		tbn2[0] *= faceDirection;
		tbn2[1] *= faceDirection;
	#endif
#endif
vec3 nonPerturbedNormal = normal;`,normal_fragment_maps:`#ifdef USE_NORMALMAP_OBJECTSPACE
	normal = texture2D( normalMap, vNormalMapUv ).xyz * 2.0 - 1.0;
	#ifdef FLIP_SIDED
		normal = - normal;
	#endif
	#ifdef DOUBLE_SIDED
		normal = normal * faceDirection;
	#endif
	normal = normalize( normalMatrix * normal );
#elif defined( USE_NORMALMAP_TANGENTSPACE )
	vec3 mapN = texture2D( normalMap, vNormalMapUv ).xyz * 2.0 - 1.0;
	#if defined( USE_PACKED_NORMALMAP )
		mapN = vec3( mapN.xy, sqrt( saturate( 1.0 - dot( mapN.xy, mapN.xy ) ) ) );
	#endif
	mapN.xy *= normalScale;
	normal = normalize( tbn * mapN );
#elif defined( USE_BUMPMAP )
	normal = perturbNormalArb( - vViewPosition, normal, dHdxy_fwd(), faceDirection );
#endif`,normal_pars_fragment:`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,normal_pars_vertex:`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,normal_vertex:`#ifndef FLAT_SHADED
	vNormal = normalize( transformedNormal );
	#ifdef USE_TANGENT
		vTangent = normalize( transformedTangent );
		vBitangent = normalize( cross( vNormal, vTangent ) * tangent.w );
	#endif
#endif`,normalmap_pars_fragment:`#ifdef USE_NORMALMAP
	uniform sampler2D normalMap;
	uniform vec2 normalScale;
#endif
#ifdef USE_NORMALMAP_OBJECTSPACE
	uniform mat3 normalMatrix;
#endif
#if ! defined ( USE_TANGENT ) && ( defined ( USE_NORMALMAP_TANGENTSPACE ) || defined ( USE_CLEARCOAT_NORMALMAP ) || defined( USE_ANISOTROPY ) )
	mat3 getTangentFrame( vec3 eye_pos, vec3 surf_norm, vec2 uv ) {
		vec3 q0 = dFdx( eye_pos.xyz );
		vec3 q1 = dFdy( eye_pos.xyz );
		vec2 st0 = dFdx( uv.st );
		vec2 st1 = dFdy( uv.st );
		vec3 N = surf_norm;
		vec3 q1perp = cross( q1, N );
		vec3 q0perp = cross( N, q0 );
		vec3 T = q1perp * st0.x + q0perp * st1.x;
		vec3 B = q1perp * st0.y + q0perp * st1.y;
		float det = max( dot( T, T ), dot( B, B ) );
		float scale = ( det == 0.0 ) ? 0.0 : inversesqrt( det );
		return mat3( T * scale, B * scale, N );
	}
#endif`,clearcoat_normal_fragment_begin:`#ifdef USE_CLEARCOAT
	vec3 clearcoatNormal = nonPerturbedNormal;
#endif`,clearcoat_normal_fragment_maps:`#ifdef USE_CLEARCOAT_NORMALMAP
	vec3 clearcoatMapN = texture2D( clearcoatNormalMap, vClearcoatNormalMapUv ).xyz * 2.0 - 1.0;
	clearcoatMapN.xy *= clearcoatNormalScale;
	clearcoatNormal = normalize( tbn2 * clearcoatMapN );
#endif`,clearcoat_pars_fragment:`#ifdef USE_CLEARCOATMAP
	uniform sampler2D clearcoatMap;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	uniform sampler2D clearcoatNormalMap;
	uniform vec2 clearcoatNormalScale;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	uniform sampler2D clearcoatRoughnessMap;
#endif`,iridescence_pars_fragment:`#ifdef USE_IRIDESCENCEMAP
	uniform sampler2D iridescenceMap;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	uniform sampler2D iridescenceThicknessMap;
#endif`,opaque_fragment:`#ifdef OPAQUE
diffuseColor.a = 1.0;
#endif
#ifdef USE_TRANSMISSION
diffuseColor.a *= material.transmissionAlpha;
#endif
gl_FragColor = vec4( outgoingLight, diffuseColor.a );`,packing:`vec3 packNormalToRGB( const in vec3 normal ) {
	return normalize( normal ) * 0.5 + 0.5;
}
vec3 unpackRGBToNormal( const in vec3 rgb ) {
	return 2.0 * rgb.xyz - 1.0;
}
const float PackUpscale = 256. / 255.;const float UnpackDownscale = 255. / 256.;const float ShiftRight8 = 1. / 256.;
const float Inv255 = 1. / 255.;
const vec4 PackFactors = vec4( 1.0, 256.0, 256.0 * 256.0, 256.0 * 256.0 * 256.0 );
const vec2 UnpackFactors2 = vec2( UnpackDownscale, 1.0 / PackFactors.g );
const vec3 UnpackFactors3 = vec3( UnpackDownscale / PackFactors.rg, 1.0 / PackFactors.b );
const vec4 UnpackFactors4 = vec4( UnpackDownscale / PackFactors.rgb, 1.0 / PackFactors.a );
vec4 packDepthToRGBA( const in float v ) {
	if( v <= 0.0 )
		return vec4( 0., 0., 0., 0. );
	if( v >= 1.0 )
		return vec4( 1., 1., 1., 1. );
	float vuf;
	float af = modf( v * PackFactors.a, vuf );
	float bf = modf( vuf * ShiftRight8, vuf );
	float gf = modf( vuf * ShiftRight8, vuf );
	return vec4( vuf * Inv255, gf * PackUpscale, bf * PackUpscale, af );
}
vec3 packDepthToRGB( const in float v ) {
	if( v <= 0.0 )
		return vec3( 0., 0., 0. );
	if( v >= 1.0 )
		return vec3( 1., 1., 1. );
	float vuf;
	float bf = modf( v * PackFactors.b, vuf );
	float gf = modf( vuf * ShiftRight8, vuf );
	return vec3( vuf * Inv255, gf * PackUpscale, bf );
}
vec2 packDepthToRG( const in float v ) {
	if( v <= 0.0 )
		return vec2( 0., 0. );
	if( v >= 1.0 )
		return vec2( 1., 1. );
	float vuf;
	float gf = modf( v * 256., vuf );
	return vec2( vuf * Inv255, gf );
}
float unpackRGBAToDepth( const in vec4 v ) {
	return dot( v, UnpackFactors4 );
}
float unpackRGBToDepth( const in vec3 v ) {
	return dot( v, UnpackFactors3 );
}
float unpackRGToDepth( const in vec2 v ) {
	return v.r * UnpackFactors2.r + v.g * UnpackFactors2.g;
}
vec4 pack2HalfToRGBA( const in vec2 v ) {
	vec4 r = vec4( v.x, fract( v.x * 255.0 ), v.y, fract( v.y * 255.0 ) );
	return vec4( r.x - r.y / 255.0, r.y, r.z - r.w / 255.0, r.w );
}
vec2 unpackRGBATo2Half( const in vec4 v ) {
	return vec2( v.x + ( v.y / 255.0 ), v.z + ( v.w / 255.0 ) );
}
float viewZToOrthographicDepth( const in float viewZ, const in float near, const in float far ) {
	return ( viewZ + near ) / ( near - far );
}
float orthographicDepthToViewZ( const in float depth, const in float near, const in float far ) {
	#ifdef USE_REVERSED_DEPTH_BUFFER
	
		return depth * ( far - near ) - far;
	#else
		return depth * ( near - far ) - near;
	#endif
}
float viewZToPerspectiveDepth( const in float viewZ, const in float near, const in float far ) {
	return ( ( near + viewZ ) * far ) / ( ( far - near ) * viewZ );
}
float perspectiveDepthToViewZ( const in float depth, const in float near, const in float far ) {
	
	#ifdef USE_REVERSED_DEPTH_BUFFER
		return ( near * far ) / ( ( near - far ) * depth - near );
	#else
		return ( near * far ) / ( ( far - near ) * depth - far );
	#endif
}`,premultiplied_alpha_fragment:`#ifdef PREMULTIPLIED_ALPHA
	gl_FragColor.rgb *= gl_FragColor.a;
#endif`,project_vertex:`vec4 mvPosition = vec4( transformed, 1.0 );
#ifdef USE_BATCHING
	mvPosition = batchingMatrix * mvPosition;
#endif
#ifdef USE_INSTANCING
	mvPosition = instanceMatrix * mvPosition;
#endif
mvPosition = modelViewMatrix * mvPosition;
gl_Position = projectionMatrix * mvPosition;`,dithering_fragment:`#ifdef DITHERING
	gl_FragColor.rgb = dithering( gl_FragColor.rgb );
#endif`,dithering_pars_fragment:`#ifdef DITHERING
	vec3 dithering( vec3 color ) {
		float grid_position = rand( gl_FragCoord.xy );
		vec3 dither_shift_RGB = vec3( 0.25 / 255.0, -0.25 / 255.0, 0.25 / 255.0 );
		dither_shift_RGB = mix( 2.0 * dither_shift_RGB, -2.0 * dither_shift_RGB, grid_position );
		return color + dither_shift_RGB;
	}
#endif`,roughnessmap_fragment:`float roughnessFactor = roughness;
#ifdef USE_ROUGHNESSMAP
	vec4 texelRoughness = texture2D( roughnessMap, vRoughnessMapUv );
	roughnessFactor *= texelRoughness.g;
#endif`,roughnessmap_pars_fragment:`#ifdef USE_ROUGHNESSMAP
	uniform sampler2D roughnessMap;
#endif`,shadowmap_pars_fragment:`#if NUM_SPOT_LIGHT_COORDS > 0
	varying vec4 vSpotLightCoord[ NUM_SPOT_LIGHT_COORDS ];
#endif
#if NUM_SPOT_LIGHT_MAPS > 0
	uniform sampler2D spotLightMap[ NUM_SPOT_LIGHT_MAPS ];
#endif
#ifdef USE_SHADOWMAP
	#if NUM_DIR_LIGHT_SHADOWS > 0
		#if defined( SHADOWMAP_TYPE_PCF )
			uniform sampler2DShadow directionalShadowMap[ NUM_DIR_LIGHT_SHADOWS ];
		#else
			uniform sampler2D directionalShadowMap[ NUM_DIR_LIGHT_SHADOWS ];
		#endif
		varying vec4 vDirectionalShadowCoord[ NUM_DIR_LIGHT_SHADOWS ];
		struct DirectionalLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform DirectionalLightShadow directionalLightShadows[ NUM_DIR_LIGHT_SHADOWS ];
	#endif
	#if NUM_SPOT_LIGHT_SHADOWS > 0
		#if defined( SHADOWMAP_TYPE_PCF )
			uniform sampler2DShadow spotShadowMap[ NUM_SPOT_LIGHT_SHADOWS ];
		#else
			uniform sampler2D spotShadowMap[ NUM_SPOT_LIGHT_SHADOWS ];
		#endif
		struct SpotLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform SpotLightShadow spotLightShadows[ NUM_SPOT_LIGHT_SHADOWS ];
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
		#if defined( SHADOWMAP_TYPE_PCF )
			uniform samplerCubeShadow pointShadowMap[ NUM_POINT_LIGHT_SHADOWS ];
		#elif defined( SHADOWMAP_TYPE_BASIC )
			uniform samplerCube pointShadowMap[ NUM_POINT_LIGHT_SHADOWS ];
		#endif
		varying vec4 vPointShadowCoord[ NUM_POINT_LIGHT_SHADOWS ];
		struct PointLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
			float shadowCameraNear;
			float shadowCameraFar;
		};
		uniform PointLightShadow pointLightShadows[ NUM_POINT_LIGHT_SHADOWS ];
	#endif
	#if defined( SHADOWMAP_TYPE_PCF )
		float interleavedGradientNoise( vec2 position ) {
			return fract( 52.9829189 * fract( dot( position, vec2( 0.06711056, 0.00583715 ) ) ) );
		}
		vec2 vogelDiskSample( int sampleIndex, int samplesCount, float phi ) {
			const float goldenAngle = 2.399963229728653;
			float r = sqrt( ( float( sampleIndex ) + 0.5 ) / float( samplesCount ) );
			float theta = float( sampleIndex ) * goldenAngle + phi;
			return vec2( cos( theta ), sin( theta ) ) * r;
		}
	#endif
	#if defined( SHADOWMAP_TYPE_PCF )
		float getShadow( sampler2DShadow shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {
			float shadow = 1.0;
			shadowCoord.xyz /= shadowCoord.w;
			shadowCoord.z += shadowBias;
			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;
			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;
			if ( frustumTest ) {
				vec2 texelSize = vec2( 1.0 ) / shadowMapSize;
				float radius = shadowRadius * texelSize.x;
				float phi = interleavedGradientNoise( gl_FragCoord.xy ) * PI2;
				shadow = (
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 0, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 1, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 2, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 3, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 4, 5, phi ) * radius, shadowCoord.z ) )
				) * 0.2;
			}
			return mix( 1.0, shadow, shadowIntensity );
		}
	#elif defined( SHADOWMAP_TYPE_VSM )
		float getShadow( sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {
			float shadow = 1.0;
			shadowCoord.xyz /= shadowCoord.w;
			#ifdef USE_REVERSED_DEPTH_BUFFER
				shadowCoord.z -= shadowBias;
			#else
				shadowCoord.z += shadowBias;
			#endif
			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;
			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;
			if ( frustumTest ) {
				vec2 distribution = texture2D( shadowMap, shadowCoord.xy ).rg;
				float mean = distribution.x;
				float variance = distribution.y * distribution.y;
				#ifdef USE_REVERSED_DEPTH_BUFFER
					float hard_shadow = step( mean, shadowCoord.z );
				#else
					float hard_shadow = step( shadowCoord.z, mean );
				#endif
				
				if ( hard_shadow == 1.0 ) {
					shadow = 1.0;
				} else {
					variance = max( variance, 0.0000001 );
					float d = shadowCoord.z - mean;
					float p_max = variance / ( variance + d * d );
					p_max = clamp( ( p_max - 0.3 ) / 0.65, 0.0, 1.0 );
					shadow = max( hard_shadow, p_max );
				}
			}
			return mix( 1.0, shadow, shadowIntensity );
		}
	#else
		float getShadow( sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {
			float shadow = 1.0;
			shadowCoord.xyz /= shadowCoord.w;
			#ifdef USE_REVERSED_DEPTH_BUFFER
				shadowCoord.z -= shadowBias;
			#else
				shadowCoord.z += shadowBias;
			#endif
			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;
			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;
			if ( frustumTest ) {
				float depth = texture2D( shadowMap, shadowCoord.xy ).r;
				#ifdef USE_REVERSED_DEPTH_BUFFER
					shadow = step( depth, shadowCoord.z );
				#else
					shadow = step( shadowCoord.z, depth );
				#endif
			}
			return mix( 1.0, shadow, shadowIntensity );
		}
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
	#if defined( SHADOWMAP_TYPE_PCF )
	float getPointShadow( samplerCubeShadow shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord, float shadowCameraNear, float shadowCameraFar ) {
		float shadow = 1.0;
		vec3 lightToPosition = shadowCoord.xyz;
		vec3 bd3D = normalize( lightToPosition );
		vec3 absVec = abs( lightToPosition );
		float viewSpaceZ = max( max( absVec.x, absVec.y ), absVec.z );
		if ( viewSpaceZ - shadowCameraFar <= 0.0 && viewSpaceZ - shadowCameraNear >= 0.0 ) {
			#ifdef USE_REVERSED_DEPTH_BUFFER
				float dp = ( shadowCameraNear * ( shadowCameraFar - viewSpaceZ ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );
				dp -= shadowBias;
			#else
				float dp = ( shadowCameraFar * ( viewSpaceZ - shadowCameraNear ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );
				dp += shadowBias;
			#endif
			float texelSize = shadowRadius / shadowMapSize.x;
			vec3 absDir = abs( bd3D );
			vec3 tangent = absDir.x > absDir.z ? vec3( 0.0, 1.0, 0.0 ) : vec3( 1.0, 0.0, 0.0 );
			tangent = normalize( cross( bd3D, tangent ) );
			vec3 bitangent = cross( bd3D, tangent );
			float phi = interleavedGradientNoise( gl_FragCoord.xy ) * PI2;
			vec2 sample0 = vogelDiskSample( 0, 5, phi );
			vec2 sample1 = vogelDiskSample( 1, 5, phi );
			vec2 sample2 = vogelDiskSample( 2, 5, phi );
			vec2 sample3 = vogelDiskSample( 3, 5, phi );
			vec2 sample4 = vogelDiskSample( 4, 5, phi );
			shadow = (
				texture( shadowMap, vec4( bd3D + ( tangent * sample0.x + bitangent * sample0.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample1.x + bitangent * sample1.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample2.x + bitangent * sample2.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample3.x + bitangent * sample3.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample4.x + bitangent * sample4.y ) * texelSize, dp ) )
			) * 0.2;
		}
		return mix( 1.0, shadow, shadowIntensity );
	}
	#elif defined( SHADOWMAP_TYPE_BASIC )
	float getPointShadow( samplerCube shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord, float shadowCameraNear, float shadowCameraFar ) {
		float shadow = 1.0;
		vec3 lightToPosition = shadowCoord.xyz;
		vec3 absVec = abs( lightToPosition );
		float viewSpaceZ = max( max( absVec.x, absVec.y ), absVec.z );
		if ( viewSpaceZ - shadowCameraFar <= 0.0 && viewSpaceZ - shadowCameraNear >= 0.0 ) {
			float dp = ( shadowCameraFar * ( viewSpaceZ - shadowCameraNear ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );
			dp += shadowBias;
			vec3 bd3D = normalize( lightToPosition );
			float depth = textureCube( shadowMap, bd3D ).r;
			#ifdef USE_REVERSED_DEPTH_BUFFER
				depth = 1.0 - depth;
			#endif
			shadow = step( dp, depth );
		}
		return mix( 1.0, shadow, shadowIntensity );
	}
	#endif
	#endif
#endif`,shadowmap_pars_vertex:`#if NUM_SPOT_LIGHT_COORDS > 0
	uniform mat4 spotLightMatrix[ NUM_SPOT_LIGHT_COORDS ];
	varying vec4 vSpotLightCoord[ NUM_SPOT_LIGHT_COORDS ];
#endif
#ifdef USE_SHADOWMAP
	#if NUM_DIR_LIGHT_SHADOWS > 0
		uniform mat4 directionalShadowMatrix[ NUM_DIR_LIGHT_SHADOWS ];
		varying vec4 vDirectionalShadowCoord[ NUM_DIR_LIGHT_SHADOWS ];
		struct DirectionalLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform DirectionalLightShadow directionalLightShadows[ NUM_DIR_LIGHT_SHADOWS ];
	#endif
	#if NUM_SPOT_LIGHT_SHADOWS > 0
		struct SpotLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform SpotLightShadow spotLightShadows[ NUM_SPOT_LIGHT_SHADOWS ];
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
		uniform mat4 pointShadowMatrix[ NUM_POINT_LIGHT_SHADOWS ];
		varying vec4 vPointShadowCoord[ NUM_POINT_LIGHT_SHADOWS ];
		struct PointLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
			float shadowCameraNear;
			float shadowCameraFar;
		};
		uniform PointLightShadow pointLightShadows[ NUM_POINT_LIGHT_SHADOWS ];
	#endif
#endif`,shadowmap_vertex:`#if ( defined( USE_SHADOWMAP ) && ( NUM_DIR_LIGHT_SHADOWS > 0 || NUM_POINT_LIGHT_SHADOWS > 0 ) ) || ( NUM_SPOT_LIGHT_COORDS > 0 )
	#ifdef HAS_NORMAL
		vec3 shadowWorldNormal = inverseTransformDirection( transformedNormal, viewMatrix );
	#else
		vec3 shadowWorldNormal = vec3( 0.0 );
	#endif
	vec4 shadowWorldPosition;
#endif
#if defined( USE_SHADOWMAP )
	#if NUM_DIR_LIGHT_SHADOWS > 0
		#pragma unroll_loop_start
		for ( int i = 0; i < NUM_DIR_LIGHT_SHADOWS; i ++ ) {
			shadowWorldPosition = worldPosition + vec4( shadowWorldNormal * directionalLightShadows[ i ].shadowNormalBias, 0 );
			vDirectionalShadowCoord[ i ] = directionalShadowMatrix[ i ] * shadowWorldPosition;
		}
		#pragma unroll_loop_end
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
		#pragma unroll_loop_start
		for ( int i = 0; i < NUM_POINT_LIGHT_SHADOWS; i ++ ) {
			shadowWorldPosition = worldPosition + vec4( shadowWorldNormal * pointLightShadows[ i ].shadowNormalBias, 0 );
			vPointShadowCoord[ i ] = pointShadowMatrix[ i ] * shadowWorldPosition;
		}
		#pragma unroll_loop_end
	#endif
#endif
#if NUM_SPOT_LIGHT_COORDS > 0
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_SPOT_LIGHT_COORDS; i ++ ) {
		shadowWorldPosition = worldPosition;
		#if ( defined( USE_SHADOWMAP ) && UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )
			shadowWorldPosition.xyz += shadowWorldNormal * spotLightShadows[ i ].shadowNormalBias;
		#endif
		vSpotLightCoord[ i ] = spotLightMatrix[ i ] * shadowWorldPosition;
	}
	#pragma unroll_loop_end
#endif`,shadowmask_pars_fragment:`float getShadowMask() {
	float shadow = 1.0;
	#ifdef USE_SHADOWMAP
	#if NUM_DIR_LIGHT_SHADOWS > 0
	DirectionalLightShadow directionalLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_DIR_LIGHT_SHADOWS; i ++ ) {
		directionalLight = directionalLightShadows[ i ];
		shadow *= receiveShadow ? getShadow( directionalShadowMap[ i ], directionalLight.shadowMapSize, directionalLight.shadowIntensity, directionalLight.shadowBias, directionalLight.shadowRadius, vDirectionalShadowCoord[ i ] ) : 1.0;
	}
	#pragma unroll_loop_end
	#endif
	#if NUM_SPOT_LIGHT_SHADOWS > 0
	SpotLightShadow spotLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_SPOT_LIGHT_SHADOWS; i ++ ) {
		spotLight = spotLightShadows[ i ];
		shadow *= receiveShadow ? getShadow( spotShadowMap[ i ], spotLight.shadowMapSize, spotLight.shadowIntensity, spotLight.shadowBias, spotLight.shadowRadius, vSpotLightCoord[ i ] ) : 1.0;
	}
	#pragma unroll_loop_end
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0 && ( defined( SHADOWMAP_TYPE_PCF ) || defined( SHADOWMAP_TYPE_BASIC ) )
	PointLightShadow pointLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_POINT_LIGHT_SHADOWS; i ++ ) {
		pointLight = pointLightShadows[ i ];
		shadow *= receiveShadow ? getPointShadow( pointShadowMap[ i ], pointLight.shadowMapSize, pointLight.shadowIntensity, pointLight.shadowBias, pointLight.shadowRadius, vPointShadowCoord[ i ], pointLight.shadowCameraNear, pointLight.shadowCameraFar ) : 1.0;
	}
	#pragma unroll_loop_end
	#endif
	#endif
	return shadow;
}`,skinbase_vertex:`#ifdef USE_SKINNING
	mat4 boneMatX = getBoneMatrix( skinIndex.x );
	mat4 boneMatY = getBoneMatrix( skinIndex.y );
	mat4 boneMatZ = getBoneMatrix( skinIndex.z );
	mat4 boneMatW = getBoneMatrix( skinIndex.w );
#endif`,skinning_pars_vertex:`#ifdef USE_SKINNING
	uniform mat4 bindMatrix;
	uniform mat4 bindMatrixInverse;
	uniform highp sampler2D boneTexture;
	mat4 getBoneMatrix( const in float i ) {
		int size = textureSize( boneTexture, 0 ).x;
		int j = int( i ) * 4;
		int x = j % size;
		int y = j / size;
		vec4 v1 = texelFetch( boneTexture, ivec2( x, y ), 0 );
		vec4 v2 = texelFetch( boneTexture, ivec2( x + 1, y ), 0 );
		vec4 v3 = texelFetch( boneTexture, ivec2( x + 2, y ), 0 );
		vec4 v4 = texelFetch( boneTexture, ivec2( x + 3, y ), 0 );
		return mat4( v1, v2, v3, v4 );
	}
#endif`,skinning_vertex:`#ifdef USE_SKINNING
	vec4 skinVertex = bindMatrix * vec4( transformed, 1.0 );
	vec4 skinned = vec4( 0.0 );
	skinned += boneMatX * skinVertex * skinWeight.x;
	skinned += boneMatY * skinVertex * skinWeight.y;
	skinned += boneMatZ * skinVertex * skinWeight.z;
	skinned += boneMatW * skinVertex * skinWeight.w;
	transformed = ( bindMatrixInverse * skinned ).xyz;
#endif`,skinnormal_vertex:`#ifdef USE_SKINNING
	mat4 skinMatrix = mat4( 0.0 );
	skinMatrix += skinWeight.x * boneMatX;
	skinMatrix += skinWeight.y * boneMatY;
	skinMatrix += skinWeight.z * boneMatZ;
	skinMatrix += skinWeight.w * boneMatW;
	skinMatrix = bindMatrixInverse * skinMatrix * bindMatrix;
	objectNormal = vec4( skinMatrix * vec4( objectNormal, 0.0 ) ).xyz;
	#ifdef USE_TANGENT
		objectTangent = vec4( skinMatrix * vec4( objectTangent, 0.0 ) ).xyz;
	#endif
#endif`,specularmap_fragment:`float specularStrength;
#ifdef USE_SPECULARMAP
	vec4 texelSpecular = texture2D( specularMap, vSpecularMapUv );
	specularStrength = texelSpecular.r;
#else
	specularStrength = 1.0;
#endif`,specularmap_pars_fragment:`#ifdef USE_SPECULARMAP
	uniform sampler2D specularMap;
#endif`,tonemapping_fragment:`#if defined( TONE_MAPPING )
	gl_FragColor.rgb = toneMapping( gl_FragColor.rgb );
#endif`,tonemapping_pars_fragment:`#ifndef saturate
#define saturate( a ) clamp( a, 0.0, 1.0 )
#endif
uniform float toneMappingExposure;
vec3 LinearToneMapping( vec3 color ) {
	return saturate( toneMappingExposure * color );
}
vec3 ReinhardToneMapping( vec3 color ) {
	color *= toneMappingExposure;
	return saturate( color / ( vec3( 1.0 ) + color ) );
}
vec3 CineonToneMapping( vec3 color ) {
	color *= toneMappingExposure;
	color = max( vec3( 0.0 ), color - 0.004 );
	return pow( ( color * ( 6.2 * color + 0.5 ) ) / ( color * ( 6.2 * color + 1.7 ) + 0.06 ), vec3( 2.2 ) );
}
vec3 RRTAndODTFit( vec3 v ) {
	vec3 a = v * ( v + 0.0245786 ) - 0.000090537;
	vec3 b = v * ( 0.983729 * v + 0.4329510 ) + 0.238081;
	return a / b;
}
vec3 ACESFilmicToneMapping( vec3 color ) {
	const mat3 ACESInputMat = mat3(
		vec3( 0.59719, 0.07600, 0.02840 ),		vec3( 0.35458, 0.90834, 0.13383 ),
		vec3( 0.04823, 0.01566, 0.83777 )
	);
	const mat3 ACESOutputMat = mat3(
		vec3(  1.60475, -0.10208, -0.00327 ),		vec3( -0.53108,  1.10813, -0.07276 ),
		vec3( -0.07367, -0.00605,  1.07602 )
	);
	color *= toneMappingExposure / 0.6;
	color = ACESInputMat * color;
	color = RRTAndODTFit( color );
	color = ACESOutputMat * color;
	return saturate( color );
}
const mat3 LINEAR_REC2020_TO_LINEAR_SRGB = mat3(
	vec3( 1.6605, - 0.1246, - 0.0182 ),
	vec3( - 0.5876, 1.1329, - 0.1006 ),
	vec3( - 0.0728, - 0.0083, 1.1187 )
);
const mat3 LINEAR_SRGB_TO_LINEAR_REC2020 = mat3(
	vec3( 0.6274, 0.0691, 0.0164 ),
	vec3( 0.3293, 0.9195, 0.0880 ),
	vec3( 0.0433, 0.0113, 0.8956 )
);
vec3 agxDefaultContrastApprox( vec3 x ) {
	vec3 x2 = x * x;
	vec3 x4 = x2 * x2;
	return + 15.5 * x4 * x2
		- 40.14 * x4 * x
		+ 31.96 * x4
		- 6.868 * x2 * x
		+ 0.4298 * x2
		+ 0.1191 * x
		- 0.00232;
}
vec3 AgXToneMapping( vec3 color ) {
	const mat3 AgXInsetMatrix = mat3(
		vec3( 0.856627153315983, 0.137318972929847, 0.11189821299995 ),
		vec3( 0.0951212405381588, 0.761241990602591, 0.0767994186031903 ),
		vec3( 0.0482516061458583, 0.101439036467562, 0.811302368396859 )
	);
	const mat3 AgXOutsetMatrix = mat3(
		vec3( 1.1271005818144368, - 0.1413297634984383, - 0.14132976349843826 ),
		vec3( - 0.11060664309660323, 1.157823702216272, - 0.11060664309660294 ),
		vec3( - 0.016493938717834573, - 0.016493938717834257, 1.2519364065950405 )
	);
	const float AgxMinEv = - 12.47393;	const float AgxMaxEv = 4.026069;
	color *= toneMappingExposure;
	color = LINEAR_SRGB_TO_LINEAR_REC2020 * color;
	color = AgXInsetMatrix * color;
	color = max( color, 1e-10 );	color = log2( color );
	color = ( color - AgxMinEv ) / ( AgxMaxEv - AgxMinEv );
	color = clamp( color, 0.0, 1.0 );
	color = agxDefaultContrastApprox( color );
	color = AgXOutsetMatrix * color;
	color = pow( max( vec3( 0.0 ), color ), vec3( 2.2 ) );
	color = LINEAR_REC2020_TO_LINEAR_SRGB * color;
	color = clamp( color, 0.0, 1.0 );
	return color;
}
vec3 NeutralToneMapping( vec3 color ) {
	const float StartCompression = 0.8 - 0.04;
	const float Desaturation = 0.15;
	color *= toneMappingExposure;
	float x = min( color.r, min( color.g, color.b ) );
	float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
	color -= offset;
	float peak = max( color.r, max( color.g, color.b ) );
	if ( peak < StartCompression ) return color;
	float d = 1. - StartCompression;
	float newPeak = 1. - d * d / ( peak + d - StartCompression );
	color *= newPeak / peak;
	float g = 1. - 1. / ( Desaturation * ( peak - newPeak ) + 1. );
	return mix( color, vec3( newPeak ), g );
}
vec3 CustomToneMapping( vec3 color ) { return color; }`,transmission_fragment:`#ifdef USE_TRANSMISSION
	material.transmission = transmission;
	material.transmissionAlpha = 1.0;
	material.thickness = thickness;
	material.attenuationDistance = attenuationDistance;
	material.attenuationColor = attenuationColor;
	#ifdef USE_TRANSMISSIONMAP
		material.transmission *= texture2D( transmissionMap, vTransmissionMapUv ).r;
	#endif
	#ifdef USE_THICKNESSMAP
		material.thickness *= texture2D( thicknessMap, vThicknessMapUv ).g;
	#endif
	vec3 pos = vWorldPosition;
	vec3 v = normalize( cameraPosition - pos );
	vec3 n = inverseTransformDirection( normal, viewMatrix );
	vec4 transmitted = getIBLVolumeRefraction(
		n, v, material.roughness, material.diffuseContribution, material.specularColorBlended, material.specularF90,
		pos, modelMatrix, viewMatrix, projectionMatrix, material.dispersion, material.ior, material.thickness,
		material.attenuationColor, material.attenuationDistance );
	material.transmissionAlpha = mix( material.transmissionAlpha, transmitted.a, material.transmission );
	totalDiffuse = mix( totalDiffuse, transmitted.rgb, material.transmission );
#endif`,transmission_pars_fragment:`#ifdef USE_TRANSMISSION
	uniform float transmission;
	uniform float thickness;
	uniform float attenuationDistance;
	uniform vec3 attenuationColor;
	#ifdef USE_TRANSMISSIONMAP
		uniform sampler2D transmissionMap;
	#endif
	#ifdef USE_THICKNESSMAP
		uniform sampler2D thicknessMap;
	#endif
	uniform vec2 transmissionSamplerSize;
	uniform sampler2D transmissionSamplerMap;
	uniform mat4 modelMatrix;
	uniform mat4 projectionMatrix;
	varying vec3 vWorldPosition;
	float w0( float a ) {
		return ( 1.0 / 6.0 ) * ( a * ( a * ( - a + 3.0 ) - 3.0 ) + 1.0 );
	}
	float w1( float a ) {
		return ( 1.0 / 6.0 ) * ( a *  a * ( 3.0 * a - 6.0 ) + 4.0 );
	}
	float w2( float a ){
		return ( 1.0 / 6.0 ) * ( a * ( a * ( - 3.0 * a + 3.0 ) + 3.0 ) + 1.0 );
	}
	float w3( float a ) {
		return ( 1.0 / 6.0 ) * ( a * a * a );
	}
	float g0( float a ) {
		return w0( a ) + w1( a );
	}
	float g1( float a ) {
		return w2( a ) + w3( a );
	}
	float h0( float a ) {
		return - 1.0 + w1( a ) / ( w0( a ) + w1( a ) );
	}
	float h1( float a ) {
		return 1.0 + w3( a ) / ( w2( a ) + w3( a ) );
	}
	vec4 bicubic( sampler2D tex, vec2 uv, vec4 texelSize, float lod ) {
		uv = uv * texelSize.zw + 0.5;
		vec2 iuv = floor( uv );
		vec2 fuv = fract( uv );
		float g0x = g0( fuv.x );
		float g1x = g1( fuv.x );
		float h0x = h0( fuv.x );
		float h1x = h1( fuv.x );
		float h0y = h0( fuv.y );
		float h1y = h1( fuv.y );
		vec2 p0 = ( vec2( iuv.x + h0x, iuv.y + h0y ) - 0.5 ) * texelSize.xy;
		vec2 p1 = ( vec2( iuv.x + h1x, iuv.y + h0y ) - 0.5 ) * texelSize.xy;
		vec2 p2 = ( vec2( iuv.x + h0x, iuv.y + h1y ) - 0.5 ) * texelSize.xy;
		vec2 p3 = ( vec2( iuv.x + h1x, iuv.y + h1y ) - 0.5 ) * texelSize.xy;
		return g0( fuv.y ) * ( g0x * textureLod( tex, p0, lod ) + g1x * textureLod( tex, p1, lod ) ) +
			g1( fuv.y ) * ( g0x * textureLod( tex, p2, lod ) + g1x * textureLod( tex, p3, lod ) );
	}
	vec4 textureBicubic( sampler2D sampler, vec2 uv, float lod ) {
		vec2 fLodSize = vec2( textureSize( sampler, int( lod ) ) );
		vec2 cLodSize = vec2( textureSize( sampler, int( lod + 1.0 ) ) );
		vec2 fLodSizeInv = 1.0 / fLodSize;
		vec2 cLodSizeInv = 1.0 / cLodSize;
		vec4 fSample = bicubic( sampler, uv, vec4( fLodSizeInv, fLodSize ), floor( lod ) );
		vec4 cSample = bicubic( sampler, uv, vec4( cLodSizeInv, cLodSize ), ceil( lod ) );
		return mix( fSample, cSample, fract( lod ) );
	}
	vec3 getVolumeTransmissionRay( const in vec3 n, const in vec3 v, const in float thickness, const in float ior, const in mat4 modelMatrix ) {
		vec3 refractionVector = refract( - v, normalize( n ), 1.0 / ior );
		vec3 modelScale;
		modelScale.x = length( vec3( modelMatrix[ 0 ].xyz ) );
		modelScale.y = length( vec3( modelMatrix[ 1 ].xyz ) );
		modelScale.z = length( vec3( modelMatrix[ 2 ].xyz ) );
		return normalize( refractionVector ) * thickness * modelScale;
	}
	float applyIorToRoughness( const in float roughness, const in float ior ) {
		return roughness * clamp( ior * 2.0 - 2.0, 0.0, 1.0 );
	}
	vec4 getTransmissionSample( const in vec2 fragCoord, const in float roughness, const in float ior ) {
		float lod = log2( transmissionSamplerSize.x ) * applyIorToRoughness( roughness, ior );
		return textureBicubic( transmissionSamplerMap, fragCoord.xy, lod );
	}
	vec3 volumeAttenuation( const in float transmissionDistance, const in vec3 attenuationColor, const in float attenuationDistance ) {
		if ( isinf( attenuationDistance ) ) {
			return vec3( 1.0 );
		} else {
			vec3 attenuationCoefficient = -log( attenuationColor ) / attenuationDistance;
			vec3 transmittance = exp( - attenuationCoefficient * transmissionDistance );			return transmittance;
		}
	}
	vec4 getIBLVolumeRefraction( const in vec3 n, const in vec3 v, const in float roughness, const in vec3 diffuseColor,
		const in vec3 specularColor, const in float specularF90, const in vec3 position, const in mat4 modelMatrix,
		const in mat4 viewMatrix, const in mat4 projMatrix, const in float dispersion, const in float ior, const in float thickness,
		const in vec3 attenuationColor, const in float attenuationDistance ) {
		vec4 transmittedLight;
		vec3 transmittance;
		#ifdef USE_DISPERSION
			float halfSpread = ( ior - 1.0 ) * 0.025 * dispersion;
			vec3 iors = vec3( ior - halfSpread, ior, ior + halfSpread );
			for ( int i = 0; i < 3; i ++ ) {
				vec3 transmissionRay = getVolumeTransmissionRay( n, v, thickness, iors[ i ], modelMatrix );
				vec3 refractedRayExit = position + transmissionRay;
				vec4 ndcPos = projMatrix * viewMatrix * vec4( refractedRayExit, 1.0 );
				vec2 refractionCoords = ndcPos.xy / ndcPos.w;
				refractionCoords += 1.0;
				refractionCoords /= 2.0;
				vec4 transmissionSample = getTransmissionSample( refractionCoords, roughness, iors[ i ] );
				transmittedLight[ i ] = transmissionSample[ i ];
				transmittedLight.a += transmissionSample.a;
				transmittance[ i ] = diffuseColor[ i ] * volumeAttenuation( length( transmissionRay ), attenuationColor, attenuationDistance )[ i ];
			}
			transmittedLight.a /= 3.0;
		#else
			vec3 transmissionRay = getVolumeTransmissionRay( n, v, thickness, ior, modelMatrix );
			vec3 refractedRayExit = position + transmissionRay;
			vec4 ndcPos = projMatrix * viewMatrix * vec4( refractedRayExit, 1.0 );
			vec2 refractionCoords = ndcPos.xy / ndcPos.w;
			refractionCoords += 1.0;
			refractionCoords /= 2.0;
			transmittedLight = getTransmissionSample( refractionCoords, roughness, ior );
			transmittance = diffuseColor * volumeAttenuation( length( transmissionRay ), attenuationColor, attenuationDistance );
		#endif
		vec3 attenuatedColor = transmittance * transmittedLight.rgb;
		vec3 F = EnvironmentBRDF( n, v, specularColor, specularF90, roughness );
		float transmittanceFactor = ( transmittance.r + transmittance.g + transmittance.b ) / 3.0;
		return vec4( ( 1.0 - F ) * attenuatedColor, 1.0 - ( 1.0 - transmittedLight.a ) * transmittanceFactor );
	}
#endif`,uv_pars_fragment:`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
	varying vec2 vUv;
#endif
#ifdef USE_MAP
	varying vec2 vMapUv;
#endif
#ifdef USE_ALPHAMAP
	varying vec2 vAlphaMapUv;
#endif
#ifdef USE_LIGHTMAP
	varying vec2 vLightMapUv;
#endif
#ifdef USE_AOMAP
	varying vec2 vAoMapUv;
#endif
#ifdef USE_BUMPMAP
	varying vec2 vBumpMapUv;
#endif
#ifdef USE_NORMALMAP
	varying vec2 vNormalMapUv;
#endif
#ifdef USE_EMISSIVEMAP
	varying vec2 vEmissiveMapUv;
#endif
#ifdef USE_METALNESSMAP
	varying vec2 vMetalnessMapUv;
#endif
#ifdef USE_ROUGHNESSMAP
	varying vec2 vRoughnessMapUv;
#endif
#ifdef USE_ANISOTROPYMAP
	varying vec2 vAnisotropyMapUv;
#endif
#ifdef USE_CLEARCOATMAP
	varying vec2 vClearcoatMapUv;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	varying vec2 vClearcoatNormalMapUv;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	varying vec2 vClearcoatRoughnessMapUv;
#endif
#ifdef USE_IRIDESCENCEMAP
	varying vec2 vIridescenceMapUv;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	varying vec2 vIridescenceThicknessMapUv;
#endif
#ifdef USE_SHEEN_COLORMAP
	varying vec2 vSheenColorMapUv;
#endif
#ifdef USE_SHEEN_ROUGHNESSMAP
	varying vec2 vSheenRoughnessMapUv;
#endif
#ifdef USE_SPECULARMAP
	varying vec2 vSpecularMapUv;
#endif
#ifdef USE_SPECULAR_COLORMAP
	varying vec2 vSpecularColorMapUv;
#endif
#ifdef USE_SPECULAR_INTENSITYMAP
	varying vec2 vSpecularIntensityMapUv;
#endif
#ifdef USE_TRANSMISSIONMAP
	uniform mat3 transmissionMapTransform;
	varying vec2 vTransmissionMapUv;
#endif
#ifdef USE_THICKNESSMAP
	uniform mat3 thicknessMapTransform;
	varying vec2 vThicknessMapUv;
#endif`,uv_pars_vertex:`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
	varying vec2 vUv;
#endif
#ifdef USE_MAP
	uniform mat3 mapTransform;
	varying vec2 vMapUv;
#endif
#ifdef USE_ALPHAMAP
	uniform mat3 alphaMapTransform;
	varying vec2 vAlphaMapUv;
#endif
#ifdef USE_LIGHTMAP
	uniform mat3 lightMapTransform;
	varying vec2 vLightMapUv;
#endif
#ifdef USE_AOMAP
	uniform mat3 aoMapTransform;
	varying vec2 vAoMapUv;
#endif
#ifdef USE_BUMPMAP
	uniform mat3 bumpMapTransform;
	varying vec2 vBumpMapUv;
#endif
#ifdef USE_NORMALMAP
	uniform mat3 normalMapTransform;
	varying vec2 vNormalMapUv;
#endif
#ifdef USE_DISPLACEMENTMAP
	uniform mat3 displacementMapTransform;
	varying vec2 vDisplacementMapUv;
#endif
#ifdef USE_EMISSIVEMAP
	uniform mat3 emissiveMapTransform;
	varying vec2 vEmissiveMapUv;
#endif
#ifdef USE_METALNESSMAP
	uniform mat3 metalnessMapTransform;
	varying vec2 vMetalnessMapUv;
#endif
#ifdef USE_ROUGHNESSMAP
	uniform mat3 roughnessMapTransform;
	varying vec2 vRoughnessMapUv;
#endif
#ifdef USE_ANISOTROPYMAP
	uniform mat3 anisotropyMapTransform;
	varying vec2 vAnisotropyMapUv;
#endif
#ifdef USE_CLEARCOATMAP
	uniform mat3 clearcoatMapTransform;
	varying vec2 vClearcoatMapUv;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	uniform mat3 clearcoatNormalMapTransform;
	varying vec2 vClearcoatNormalMapUv;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	uniform mat3 clearcoatRoughnessMapTransform;
	varying vec2 vClearcoatRoughnessMapUv;
#endif
#ifdef USE_SHEEN_COLORMAP
	uniform mat3 sheenColorMapTransform;
	varying vec2 vSheenColorMapUv;
#endif
#ifdef USE_SHEEN_ROUGHNESSMAP
	uniform mat3 sheenRoughnessMapTransform;
	varying vec2 vSheenRoughnessMapUv;
#endif
#ifdef USE_IRIDESCENCEMAP
	uniform mat3 iridescenceMapTransform;
	varying vec2 vIridescenceMapUv;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	uniform mat3 iridescenceThicknessMapTransform;
	varying vec2 vIridescenceThicknessMapUv;
#endif
#ifdef USE_SPECULARMAP
	uniform mat3 specularMapTransform;
	varying vec2 vSpecularMapUv;
#endif
#ifdef USE_SPECULAR_COLORMAP
	uniform mat3 specularColorMapTransform;
	varying vec2 vSpecularColorMapUv;
#endif
#ifdef USE_SPECULAR_INTENSITYMAP
	uniform mat3 specularIntensityMapTransform;
	varying vec2 vSpecularIntensityMapUv;
#endif
#ifdef USE_TRANSMISSIONMAP
	uniform mat3 transmissionMapTransform;
	varying vec2 vTransmissionMapUv;
#endif
#ifdef USE_THICKNESSMAP
	uniform mat3 thicknessMapTransform;
	varying vec2 vThicknessMapUv;
#endif`,uv_vertex:`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
	vUv = vec3( uv, 1 ).xy;
#endif
#ifdef USE_MAP
	vMapUv = ( mapTransform * vec3( MAP_UV, 1 ) ).xy;
#endif
#ifdef USE_ALPHAMAP
	vAlphaMapUv = ( alphaMapTransform * vec3( ALPHAMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_LIGHTMAP
	vLightMapUv = ( lightMapTransform * vec3( LIGHTMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_AOMAP
	vAoMapUv = ( aoMapTransform * vec3( AOMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_BUMPMAP
	vBumpMapUv = ( bumpMapTransform * vec3( BUMPMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_NORMALMAP
	vNormalMapUv = ( normalMapTransform * vec3( NORMALMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_DISPLACEMENTMAP
	vDisplacementMapUv = ( displacementMapTransform * vec3( DISPLACEMENTMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_EMISSIVEMAP
	vEmissiveMapUv = ( emissiveMapTransform * vec3( EMISSIVEMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_METALNESSMAP
	vMetalnessMapUv = ( metalnessMapTransform * vec3( METALNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_ROUGHNESSMAP
	vRoughnessMapUv = ( roughnessMapTransform * vec3( ROUGHNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_ANISOTROPYMAP
	vAnisotropyMapUv = ( anisotropyMapTransform * vec3( ANISOTROPYMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_CLEARCOATMAP
	vClearcoatMapUv = ( clearcoatMapTransform * vec3( CLEARCOATMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	vClearcoatNormalMapUv = ( clearcoatNormalMapTransform * vec3( CLEARCOAT_NORMALMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	vClearcoatRoughnessMapUv = ( clearcoatRoughnessMapTransform * vec3( CLEARCOAT_ROUGHNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_IRIDESCENCEMAP
	vIridescenceMapUv = ( iridescenceMapTransform * vec3( IRIDESCENCEMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	vIridescenceThicknessMapUv = ( iridescenceThicknessMapTransform * vec3( IRIDESCENCE_THICKNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SHEEN_COLORMAP
	vSheenColorMapUv = ( sheenColorMapTransform * vec3( SHEEN_COLORMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SHEEN_ROUGHNESSMAP
	vSheenRoughnessMapUv = ( sheenRoughnessMapTransform * vec3( SHEEN_ROUGHNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SPECULARMAP
	vSpecularMapUv = ( specularMapTransform * vec3( SPECULARMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SPECULAR_COLORMAP
	vSpecularColorMapUv = ( specularColorMapTransform * vec3( SPECULAR_COLORMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SPECULAR_INTENSITYMAP
	vSpecularIntensityMapUv = ( specularIntensityMapTransform * vec3( SPECULAR_INTENSITYMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_TRANSMISSIONMAP
	vTransmissionMapUv = ( transmissionMapTransform * vec3( TRANSMISSIONMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_THICKNESSMAP
	vThicknessMapUv = ( thicknessMapTransform * vec3( THICKNESSMAP_UV, 1 ) ).xy;
#endif`,worldpos_vertex:`#if defined( USE_ENVMAP ) || defined( DISTANCE ) || defined ( USE_SHADOWMAP ) || defined ( USE_TRANSMISSION ) || NUM_SPOT_LIGHT_COORDS > 0
	vec4 worldPosition = vec4( transformed, 1.0 );
	#ifdef USE_BATCHING
		worldPosition = batchingMatrix * worldPosition;
	#endif
	#ifdef USE_INSTANCING
		worldPosition = instanceMatrix * worldPosition;
	#endif
	worldPosition = modelMatrix * worldPosition;
#endif`,background_vert:`varying vec2 vUv;
uniform mat3 uvTransform;
void main() {
	vUv = ( uvTransform * vec3( uv, 1 ) ).xy;
	gl_Position = vec4( position.xy, 1.0, 1.0 );
}`,background_frag:`uniform sampler2D t2D;
uniform float backgroundIntensity;
varying vec2 vUv;
void main() {
	vec4 texColor = texture2D( t2D, vUv );
	#ifdef DECODE_VIDEO_TEXTURE
		texColor = vec4( mix( pow( texColor.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), texColor.rgb * 0.0773993808, vec3( lessThanEqual( texColor.rgb, vec3( 0.04045 ) ) ) ), texColor.w );
	#endif
	texColor.rgb *= backgroundIntensity;
	gl_FragColor = texColor;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,backgroundCube_vert:`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,backgroundCube_frag:`#ifdef ENVMAP_TYPE_CUBE
	uniform samplerCube envMap;
#elif defined( ENVMAP_TYPE_CUBE_UV )
	uniform sampler2D envMap;
#endif
uniform float backgroundBlurriness;
uniform float backgroundIntensity;
uniform mat3 backgroundRotation;
varying vec3 vWorldDirection;
#include <cube_uv_reflection_fragment>
void main() {
	#ifdef ENVMAP_TYPE_CUBE
		vec4 texColor = textureCube( envMap, backgroundRotation * vWorldDirection );
	#elif defined( ENVMAP_TYPE_CUBE_UV )
		vec4 texColor = textureCubeUV( envMap, backgroundRotation * vWorldDirection, backgroundBlurriness );
	#else
		vec4 texColor = vec4( 0.0, 0.0, 0.0, 1.0 );
	#endif
	texColor.rgb *= backgroundIntensity;
	gl_FragColor = texColor;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,cube_vert:`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,cube_frag:`uniform samplerCube tCube;
uniform float tFlip;
uniform float opacity;
varying vec3 vWorldDirection;
void main() {
	vec4 texColor = textureCube( tCube, vec3( tFlip * vWorldDirection.x, vWorldDirection.yz ) );
	gl_FragColor = texColor;
	gl_FragColor.a *= opacity;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,depth_vert:`#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
varying vec2 vHighPrecisionZW;
void main() {
	#include <uv_vertex>
	#include <batching_vertex>
	#include <skinbase_vertex>
	#include <morphinstance_vertex>
	#ifdef USE_DISPLACEMENTMAP
		#include <beginnormal_vertex>
		#include <morphnormal_vertex>
		#include <skinnormal_vertex>
	#endif
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vHighPrecisionZW = gl_Position.zw;
}`,depth_frag:`#if DEPTH_PACKING == 3200
	uniform float opacity;
#endif
#include <common>
#include <packing>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
varying vec2 vHighPrecisionZW;
void main() {
	vec4 diffuseColor = vec4( 1.0 );
	#include <clipping_planes_fragment>
	#if DEPTH_PACKING == 3200
		diffuseColor.a = opacity;
	#endif
	#include <map_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <logdepthbuf_fragment>
	#ifdef USE_REVERSED_DEPTH_BUFFER
		float fragCoordZ = vHighPrecisionZW[ 0 ] / vHighPrecisionZW[ 1 ];
	#else
		float fragCoordZ = 0.5 * vHighPrecisionZW[ 0 ] / vHighPrecisionZW[ 1 ] + 0.5;
	#endif
	#if DEPTH_PACKING == 3200
		gl_FragColor = vec4( vec3( 1.0 - fragCoordZ ), opacity );
	#elif DEPTH_PACKING == 3201
		gl_FragColor = packDepthToRGBA( fragCoordZ );
	#elif DEPTH_PACKING == 3202
		gl_FragColor = vec4( packDepthToRGB( fragCoordZ ), 1.0 );
	#elif DEPTH_PACKING == 3203
		gl_FragColor = vec4( packDepthToRG( fragCoordZ ), 0.0, 1.0 );
	#endif
}`,distance_vert:`#define DISTANCE
varying vec3 vWorldPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <batching_vertex>
	#include <skinbase_vertex>
	#include <morphinstance_vertex>
	#ifdef USE_DISPLACEMENTMAP
		#include <beginnormal_vertex>
		#include <morphnormal_vertex>
		#include <skinnormal_vertex>
	#endif
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <worldpos_vertex>
	#include <clipping_planes_vertex>
	vWorldPosition = worldPosition.xyz;
}`,distance_frag:`#define DISTANCE
uniform vec3 referencePosition;
uniform float nearDistance;
uniform float farDistance;
varying vec3 vWorldPosition;
#include <common>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <clipping_planes_pars_fragment>
void main () {
	vec4 diffuseColor = vec4( 1.0 );
	#include <clipping_planes_fragment>
	#include <map_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	float dist = length( vWorldPosition - referencePosition );
	dist = ( dist - nearDistance ) / ( farDistance - nearDistance );
	dist = saturate( dist );
	gl_FragColor = vec4( dist, 0.0, 0.0, 1.0 );
}`,equirect_vert:`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
}`,equirect_frag:`uniform sampler2D tEquirect;
varying vec3 vWorldDirection;
#include <common>
void main() {
	vec3 direction = normalize( vWorldDirection );
	vec2 sampleUV = equirectUv( direction );
	gl_FragColor = texture2D( tEquirect, sampleUV );
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,linedashed_vert:`uniform float scale;
attribute float lineDistance;
varying float vLineDistance;
#include <common>
#include <uv_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	vLineDistance = scale * lineDistance;
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <fog_vertex>
}`,linedashed_frag:`uniform vec3 diffuse;
uniform float opacity;
uniform float dashSize;
uniform float totalSize;
varying float vLineDistance;
#include <common>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <fog_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	if ( mod( vLineDistance, totalSize ) > dashSize ) {
		discard;
	}
	vec3 outgoingLight = vec3( 0.0 );
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	outgoingLight = diffuseColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
}`,meshbasic_vert:`#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <envmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#if defined ( USE_ENVMAP ) || defined ( USE_SKINNING )
		#include <beginnormal_vertex>
		#include <morphnormal_vertex>
		#include <skinbase_vertex>
		#include <skinnormal_vertex>
		#include <defaultnormal_vertex>
	#endif
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <worldpos_vertex>
	#include <envmap_vertex>
	#include <fog_vertex>
}`,meshbasic_frag:`uniform vec3 diffuse;
uniform float opacity;
#ifndef FLAT_SHADED
	varying vec3 vNormal;
#endif
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_pars_fragment>
#include <fog_pars_fragment>
#include <specularmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <specularmap_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	#ifdef USE_LIGHTMAP
		vec4 lightMapTexel = texture2D( lightMap, vLightMapUv );
		reflectedLight.indirectDiffuse += lightMapTexel.rgb * lightMapIntensity * RECIPROCAL_PI;
	#else
		reflectedLight.indirectDiffuse += vec3( 1.0 );
	#endif
	#include <aomap_fragment>
	reflectedLight.indirectDiffuse *= diffuseColor.rgb;
	vec3 outgoingLight = reflectedLight.indirectDiffuse;
	#include <envmap_fragment>
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,meshlambert_vert:`#define LAMBERT
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <envmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <envmap_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,meshlambert_frag:`#define LAMBERT
uniform vec3 diffuse;
uniform vec3 emissive;
uniform float opacity;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <cube_uv_reflection_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_pars_fragment>
#include <envmap_physical_pars_fragment>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_lambert_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <specularmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <specularmap_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_lambert_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + totalEmissiveRadiance;
	#include <envmap_fragment>
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,meshmatcap_vert:`#define MATCAP
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <color_pars_vertex>
#include <displacementmap_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <fog_vertex>
	vViewPosition = - mvPosition.xyz;
}`,meshmatcap_frag:`#define MATCAP
uniform vec3 diffuse;
uniform float opacity;
uniform sampler2D matcap;
varying vec3 vViewPosition;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <fog_pars_fragment>
#include <normal_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	vec3 viewDir = normalize( vViewPosition );
	vec3 x = normalize( vec3( viewDir.z, 0.0, - viewDir.x ) );
	vec3 y = cross( viewDir, x );
	vec2 uv = vec2( dot( x, normal ), dot( y, normal ) ) * 0.495 + 0.5;
	#ifdef USE_MATCAP
		vec4 matcapColor = texture2D( matcap, uv );
	#else
		vec4 matcapColor = vec4( vec3( mix( 0.2, 0.8, uv.y ) ), 1.0 );
	#endif
	vec3 outgoingLight = diffuseColor.rgb * matcapColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,meshnormal_vert:`#define NORMAL
#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )
	varying vec3 vViewPosition;
#endif
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphinstance_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )
	vViewPosition = - mvPosition.xyz;
#endif
}`,meshnormal_frag:`#define NORMAL
uniform float opacity;
#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )
	varying vec3 vViewPosition;
#endif
#include <uv_pars_fragment>
#include <normal_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( 0.0, 0.0, 0.0, opacity );
	#include <clipping_planes_fragment>
	#include <logdepthbuf_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	gl_FragColor = vec4( normalize( normal ) * 0.5 + 0.5, diffuseColor.a );
	#ifdef OPAQUE
		gl_FragColor.a = 1.0;
	#endif
}`,meshphong_vert:`#define PHONG
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <envmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphinstance_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <envmap_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,meshphong_frag:`#define PHONG
uniform vec3 diffuse;
uniform vec3 emissive;
uniform vec3 specular;
uniform float shininess;
uniform float opacity;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <cube_uv_reflection_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_pars_fragment>
#include <envmap_physical_pars_fragment>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_phong_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <specularmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <specularmap_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_phong_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + reflectedLight.directSpecular + reflectedLight.indirectSpecular + totalEmissiveRadiance;
	#include <envmap_fragment>
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,meshphysical_vert:`#define STANDARD
varying vec3 vViewPosition;
#ifdef USE_TRANSMISSION
	varying vec3 vWorldPosition;
#endif
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
#ifdef USE_TRANSMISSION
	vWorldPosition = worldPosition.xyz;
#endif
}`,meshphysical_frag:`#define STANDARD
#ifdef PHYSICAL
	#define IOR
	#define USE_SPECULAR
#endif
uniform vec3 diffuse;
uniform vec3 emissive;
uniform float roughness;
uniform float metalness;
uniform float opacity;
#ifdef IOR
	uniform float ior;
#endif
#ifdef USE_SPECULAR
	uniform float specularIntensity;
	uniform vec3 specularColor;
	#ifdef USE_SPECULAR_COLORMAP
		uniform sampler2D specularColorMap;
	#endif
	#ifdef USE_SPECULAR_INTENSITYMAP
		uniform sampler2D specularIntensityMap;
	#endif
#endif
#ifdef USE_CLEARCOAT
	uniform float clearcoat;
	uniform float clearcoatRoughness;
#endif
#ifdef USE_DISPERSION
	uniform float dispersion;
#endif
#ifdef USE_IRIDESCENCE
	uniform float iridescence;
	uniform float iridescenceIOR;
	uniform float iridescenceThicknessMinimum;
	uniform float iridescenceThicknessMaximum;
#endif
#ifdef USE_SHEEN
	uniform vec3 sheenColor;
	uniform float sheenRoughness;
	#ifdef USE_SHEEN_COLORMAP
		uniform sampler2D sheenColorMap;
	#endif
	#ifdef USE_SHEEN_ROUGHNESSMAP
		uniform sampler2D sheenRoughnessMap;
	#endif
#endif
#ifdef USE_ANISOTROPY
	uniform vec2 anisotropyVector;
	#ifdef USE_ANISOTROPYMAP
		uniform sampler2D anisotropyMap;
	#endif
#endif
varying vec3 vViewPosition;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <iridescence_fragment>
#include <cube_uv_reflection_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_physical_pars_fragment>
#include <fog_pars_fragment>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_physical_pars_fragment>
#include <transmission_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <clearcoat_pars_fragment>
#include <iridescence_pars_fragment>
#include <roughnessmap_pars_fragment>
#include <metalnessmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <roughnessmap_fragment>
	#include <metalnessmap_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <clearcoat_normal_fragment_begin>
	#include <clearcoat_normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_physical_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 totalDiffuse = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse;
	vec3 totalSpecular = reflectedLight.directSpecular + reflectedLight.indirectSpecular;
	#include <transmission_fragment>
	vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;
	#ifdef USE_SHEEN
 
		outgoingLight = outgoingLight + sheenSpecularDirect + sheenSpecularIndirect;
 
 	#endif
	#ifdef USE_CLEARCOAT
		float dotNVcc = saturate( dot( geometryClearcoatNormal, geometryViewDir ) );
		vec3 Fcc = F_Schlick( material.clearcoatF0, material.clearcoatF90, dotNVcc );
		outgoingLight = outgoingLight * ( 1.0 - material.clearcoat * Fcc ) + ( clearcoatSpecularDirect + clearcoatSpecularIndirect ) * material.clearcoat;
	#endif
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,meshtoon_vert:`#define TOON
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,meshtoon_frag:`#define TOON
uniform vec3 diffuse;
uniform vec3 emissive;
uniform float opacity;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <gradientmap_pars_fragment>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_toon_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_toon_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + totalEmissiveRadiance;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,points_vert:`uniform float size;
uniform float scale;
#include <common>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
#ifdef USE_POINTS_UV
	varying vec2 vUv;
	uniform mat3 uvTransform;
#endif
void main() {
	#ifdef USE_POINTS_UV
		vUv = ( uvTransform * vec3( uv, 1 ) ).xy;
	#endif
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <project_vertex>
	gl_PointSize = size;
	#ifdef USE_SIZEATTENUATION
		bool isPerspective = isPerspectiveMatrix( projectionMatrix );
		if ( isPerspective ) gl_PointSize *= ( scale / - mvPosition.z );
	#endif
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <worldpos_vertex>
	#include <fog_vertex>
}`,points_frag:`uniform vec3 diffuse;
uniform float opacity;
#include <common>
#include <color_pars_fragment>
#include <map_particle_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <fog_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	vec3 outgoingLight = vec3( 0.0 );
	#include <logdepthbuf_fragment>
	#include <map_particle_fragment>
	#include <color_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	outgoingLight = diffuseColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
}`,shadow_vert:`#include <common>
#include <batching_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <shadowmap_pars_vertex>
void main() {
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphinstance_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <worldpos_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,shadow_frag:`uniform vec3 color;
uniform float opacity;
#include <common>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <logdepthbuf_pars_fragment>
#include <shadowmap_pars_fragment>
#include <shadowmask_pars_fragment>
void main() {
	#include <logdepthbuf_fragment>
	gl_FragColor = vec4( color, opacity * ( 1.0 - getShadowMask() ) );
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
}`,sprite_vert:`uniform float rotation;
uniform vec2 center;
#include <common>
#include <uv_pars_vertex>
#include <fog_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	vec4 mvPosition = modelViewMatrix[ 3 ];
	vec2 scale = vec2( length( modelMatrix[ 0 ].xyz ), length( modelMatrix[ 1 ].xyz ) );
	#ifndef USE_SIZEATTENUATION
		bool isPerspective = isPerspectiveMatrix( projectionMatrix );
		if ( isPerspective ) scale *= - mvPosition.z;
	#endif
	vec2 alignedPosition = ( position.xy - ( center - vec2( 0.5 ) ) ) * scale;
	vec2 rotatedPosition;
	rotatedPosition.x = cos( rotation ) * alignedPosition.x - sin( rotation ) * alignedPosition.y;
	rotatedPosition.y = sin( rotation ) * alignedPosition.x + cos( rotation ) * alignedPosition.y;
	mvPosition.xy += rotatedPosition;
	gl_Position = projectionMatrix * mvPosition;
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <fog_vertex>
}`,sprite_frag:`uniform vec3 diffuse;
uniform float opacity;
#include <common>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <fog_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	vec3 outgoingLight = vec3( 0.0 );
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	outgoingLight = diffuseColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
}`},Q={common:{diffuse:{value:new X(16777215)},opacity:{value:1},map:{value:null},mapTransform:{value:new q},alphaMap:{value:null},alphaMapTransform:{value:new q},alphaTest:{value:0}},specularmap:{specularMap:{value:null},specularMapTransform:{value:new q}},envmap:{envMap:{value:null},envMapRotation:{value:new q},reflectivity:{value:1},ior:{value:1.5},refractionRatio:{value:.98},dfgLUT:{value:null}},aomap:{aoMap:{value:null},aoMapIntensity:{value:1},aoMapTransform:{value:new q}},lightmap:{lightMap:{value:null},lightMapIntensity:{value:1},lightMapTransform:{value:new q}},bumpmap:{bumpMap:{value:null},bumpMapTransform:{value:new q},bumpScale:{value:1}},normalmap:{normalMap:{value:null},normalMapTransform:{value:new q},normalScale:{value:new fi(1,1)}},displacementmap:{displacementMap:{value:null},displacementMapTransform:{value:new q},displacementScale:{value:1},displacementBias:{value:0}},emissivemap:{emissiveMap:{value:null},emissiveMapTransform:{value:new q}},metalnessmap:{metalnessMap:{value:null},metalnessMapTransform:{value:new q}},roughnessmap:{roughnessMap:{value:null},roughnessMapTransform:{value:new q}},gradientmap:{gradientMap:{value:null}},fog:{fogDensity:{value:25e-5},fogNear:{value:1},fogFar:{value:2e3},fogColor:{value:new X(16777215)}},lights:{ambientLightColor:{value:[]},lightProbe:{value:[]},directionalLights:{value:[],properties:{direction:{},color:{}}},directionalLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},directionalShadowMatrix:{value:[]},spotLights:{value:[],properties:{color:{},position:{},direction:{},distance:{},coneCos:{},penumbraCos:{},decay:{}}},spotLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},spotLightMap:{value:[]},spotLightMatrix:{value:[]},pointLights:{value:[],properties:{color:{},position:{},decay:{},distance:{}}},pointLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{},shadowCameraNear:{},shadowCameraFar:{}}},pointShadowMatrix:{value:[]},hemisphereLights:{value:[],properties:{direction:{},skyColor:{},groundColor:{}}},rectAreaLights:{value:[],properties:{color:{},position:{},width:{},height:{}}},ltc_1:{value:null},ltc_2:{value:null},probesSH:{value:null},probesMin:{value:new K},probesMax:{value:new K},probesResolution:{value:new K}},points:{diffuse:{value:new X(16777215)},opacity:{value:1},size:{value:1},scale:{value:1},map:{value:null},alphaMap:{value:null},alphaMapTransform:{value:new q},alphaTest:{value:0},uvTransform:{value:new q}},sprite:{diffuse:{value:new X(16777215)},opacity:{value:1},center:{value:new fi(.5,.5)},rotation:{value:0},map:{value:null},mapTransform:{value:new q},alphaMap:{value:null},alphaMapTransform:{value:new q},alphaTest:{value:0}}},Xu={basic:{uniforms:ln([Q.common,Q.specularmap,Q.envmap,Q.aomap,Q.lightmap,Q.fog]),vertexShader:Z.meshbasic_vert,fragmentShader:Z.meshbasic_frag},lambert:{uniforms:ln([Q.common,Q.specularmap,Q.envmap,Q.aomap,Q.lightmap,Q.emissivemap,Q.bumpmap,Q.normalmap,Q.displacementmap,Q.fog,Q.lights,{emissive:{value:new X(0)},envMapIntensity:{value:1}}]),vertexShader:Z.meshlambert_vert,fragmentShader:Z.meshlambert_frag},phong:{uniforms:ln([Q.common,Q.specularmap,Q.envmap,Q.aomap,Q.lightmap,Q.emissivemap,Q.bumpmap,Q.normalmap,Q.displacementmap,Q.fog,Q.lights,{emissive:{value:new X(0)},specular:{value:new X(1118481)},shininess:{value:30},envMapIntensity:{value:1}}]),vertexShader:Z.meshphong_vert,fragmentShader:Z.meshphong_frag},standard:{uniforms:ln([Q.common,Q.envmap,Q.aomap,Q.lightmap,Q.emissivemap,Q.bumpmap,Q.normalmap,Q.displacementmap,Q.roughnessmap,Q.metalnessmap,Q.fog,Q.lights,{emissive:{value:new X(0)},roughness:{value:1},metalness:{value:0},envMapIntensity:{value:1}}]),vertexShader:Z.meshphysical_vert,fragmentShader:Z.meshphysical_frag},toon:{uniforms:ln([Q.common,Q.aomap,Q.lightmap,Q.emissivemap,Q.bumpmap,Q.normalmap,Q.displacementmap,Q.gradientmap,Q.fog,Q.lights,{emissive:{value:new X(0)}}]),vertexShader:Z.meshtoon_vert,fragmentShader:Z.meshtoon_frag},matcap:{uniforms:ln([Q.common,Q.bumpmap,Q.normalmap,Q.displacementmap,Q.fog,{matcap:{value:null}}]),vertexShader:Z.meshmatcap_vert,fragmentShader:Z.meshmatcap_frag},points:{uniforms:ln([Q.points,Q.fog]),vertexShader:Z.points_vert,fragmentShader:Z.points_frag},dashed:{uniforms:ln([Q.common,Q.fog,{scale:{value:1},dashSize:{value:1},totalSize:{value:2}}]),vertexShader:Z.linedashed_vert,fragmentShader:Z.linedashed_frag},depth:{uniforms:ln([Q.common,Q.displacementmap]),vertexShader:Z.depth_vert,fragmentShader:Z.depth_frag},normal:{uniforms:ln([Q.common,Q.bumpmap,Q.normalmap,Q.displacementmap,{opacity:{value:1}}]),vertexShader:Z.meshnormal_vert,fragmentShader:Z.meshnormal_frag},sprite:{uniforms:ln([Q.sprite,Q.fog]),vertexShader:Z.sprite_vert,fragmentShader:Z.sprite_frag},background:{uniforms:{uvTransform:{value:new q},t2D:{value:null},backgroundIntensity:{value:1}},vertexShader:Z.background_vert,fragmentShader:Z.background_frag},backgroundCube:{uniforms:{envMap:{value:null},backgroundBlurriness:{value:0},backgroundIntensity:{value:1},backgroundRotation:{value:new q}},vertexShader:Z.backgroundCube_vert,fragmentShader:Z.backgroundCube_frag},cube:{uniforms:{tCube:{value:null},tFlip:{value:-1},opacity:{value:1}},vertexShader:Z.cube_vert,fragmentShader:Z.cube_frag},equirect:{uniforms:{tEquirect:{value:null}},vertexShader:Z.equirect_vert,fragmentShader:Z.equirect_frag},distance:{uniforms:ln([Q.common,Q.displacementmap,{referencePosition:{value:new K},nearDistance:{value:1},farDistance:{value:1e3}}]),vertexShader:Z.distance_vert,fragmentShader:Z.distance_frag},shadow:{uniforms:ln([Q.lights,Q.fog,{color:{value:new X(0)},opacity:{value:1}}]),vertexShader:Z.shadow_vert,fragmentShader:Z.shadow_frag}},Xu.physical={uniforms:ln([Xu.standard.uniforms,{clearcoat:{value:0},clearcoatMap:{value:null},clearcoatMapTransform:{value:new q},clearcoatNormalMap:{value:null},clearcoatNormalMapTransform:{value:new q},clearcoatNormalScale:{value:new fi(1,1)},clearcoatRoughness:{value:0},clearcoatRoughnessMap:{value:null},clearcoatRoughnessMapTransform:{value:new q},dispersion:{value:0},iridescence:{value:0},iridescenceMap:{value:null},iridescenceMapTransform:{value:new q},iridescenceIOR:{value:1.3},iridescenceThicknessMinimum:{value:100},iridescenceThicknessMaximum:{value:400},iridescenceThicknessMap:{value:null},iridescenceThicknessMapTransform:{value:new q},sheen:{value:0},sheenColor:{value:new X(0)},sheenColorMap:{value:null},sheenColorMapTransform:{value:new q},sheenRoughness:{value:1},sheenRoughnessMap:{value:null},sheenRoughnessMapTransform:{value:new q},transmission:{value:0},transmissionMap:{value:null},transmissionMapTransform:{value:new q},transmissionSamplerSize:{value:new fi},transmissionSamplerMap:{value:null},thickness:{value:0},thicknessMap:{value:null},thicknessMapTransform:{value:new q},attenuationDistance:{value:0},attenuationColor:{value:new X(0)},specularColor:{value:new X(1,1,1)},specularColorMap:{value:null},specularColorMapTransform:{value:new q},specularIntensity:{value:1},specularIntensityMap:{value:null},specularIntensityMapTransform:{value:new q},anisotropyVector:{value:new fi},anisotropyMap:{value:null},anisotropyMapTransform:{value:new q}}]),vertexShader:Z.meshphysical_vert,fragmentShader:Z.meshphysical_frag},Zu={r:0,b:0,g:0},Qu=new Y,$u=new q,$u.set(-1,0,0,0,1,0,0,0,1),ed=4,td=[.125,.215,.35,.446,.526,.582],nd=20,rd=256,id=new vc,ad=new X,od=null,sd=0,cd=0,ld=!1,ud=new K,dd=class{constructor(e){this._renderer=e,this._pingPongRenderTarget=null,this._lodMax=0,this._cubeSize=0,this._sizeLods=[],this._sigmas=[],this._lodMeshes=[],this._backgroundBox=null,this._cubemapMaterial=null,this._equirectMaterial=null,this._blurMaterial=null,this._ggxMaterial=null}fromScene(e,t=0,n=.1,r=100,i={}){let{size:a=256,position:o=ud}=i;od=this._renderer.getRenderTarget(),sd=this._renderer.getActiveCubeFace(),cd=this._renderer.getActiveMipmapLevel(),ld=this._renderer.xr.enabled,this._renderer.xr.enabled=!1,this._setSize(a);let s=this._allocateTargets();return s.depthBuffer=!0,this._sceneToCubeUV(e,n,r,s,o),t>0&&this._blur(s,0,0,t),this._applyPMREM(s),this._cleanup(s),s}fromEquirectangular(e,t=null){return this._fromTexture(e,t)}fromCubemap(e,t=null){return this._fromTexture(e,t)}compileCubemapShader(){this._cubemapMaterial===null&&(this._cubemapMaterial=il(),this._compileMaterial(this._cubemapMaterial))}compileEquirectangularShader(){this._equirectMaterial===null&&(this._equirectMaterial=rl(),this._compileMaterial(this._equirectMaterial))}dispose(){this._dispose(),this._cubemapMaterial!==null&&this._cubemapMaterial.dispose(),this._equirectMaterial!==null&&this._equirectMaterial.dispose(),this._backgroundBox!==null&&(this._backgroundBox.geometry.dispose(),this._backgroundBox.material.dispose())}_setSize(e){this._lodMax=Math.floor(Math.log2(e)),this._cubeSize=2**this._lodMax}_dispose(){this._blurMaterial!==null&&this._blurMaterial.dispose(),this._ggxMaterial!==null&&this._ggxMaterial.dispose(),this._pingPongRenderTarget!==null&&this._pingPongRenderTarget.dispose();for(let e=0;e<this._lodMeshes.length;e++)this._lodMeshes[e].geometry.dispose()}_cleanup(e){this._renderer.setRenderTarget(od,sd,cd),this._renderer.xr.enabled=ld,e.scissorTest=!1,el(e,0,0,e.width,e.height)}_fromTexture(e,t){e.mapping===301||e.mapping===302?this._setSize(e.image.length===0?16:e.image[0].width||e.image[0].image.width):this._setSize(e.image.width/4),od=this._renderer.getRenderTarget(),sd=this._renderer.getActiveCubeFace(),cd=this._renderer.getActiveMipmapLevel(),ld=this._renderer.xr.enabled,this._renderer.xr.enabled=!1;let n=t||this._allocateTargets();return this._textureToCubeUV(e,n),this._applyPMREM(n),this._cleanup(n),n}_allocateTargets(){let e=3*Math.max(this._cubeSize,112),t=4*this._cubeSize,n={magFilter:An,minFilter:An,generateMipmaps:!1,type:Bn,format:Jn,colorSpace:Qr,depthBuffer:!1},r=$c(e,t,n);if(this._pingPongRenderTarget===null||this._pingPongRenderTarget.width!==e||this._pingPongRenderTarget.height!==t){this._pingPongRenderTarget!==null&&this._dispose(),this._pingPongRenderTarget=$c(e,t,n);let{_lodMax:r}=this;({lodMeshes:this._lodMeshes,sizeLods:this._sizeLods,sigmas:this._sigmas}=Qc(r)),this._blurMaterial=nl(r,e,t),this._ggxMaterial=tl(r,e,t)}return r}_compileMaterial(e){let t=new Oo(new io,e);this._renderer.compile(t,id)}_sceneToCubeUV(e,t,n,r,i){let a=new pc(90,1,t,n),o=[1,-1,1,1,1,1],s=[1,1,1,-1,-1,-1],c=this._renderer,l=c.autoClear,u=c.toneMapping;c.getClearColor(ad),c.toneMapping=0,c.autoClear=!1,c.state.buffers.depth.getReversed()&&(c.setRenderTarget(r),c.clearDepth(),c.setRenderTarget(null)),this._backgroundBox===null&&(this._backgroundBox=new Oo(new Es,new go({name:`PMREM.Background`,side:1,depthWrite:!1,depthTest:!1})));let d=this._backgroundBox,f=d.material,p=!1,m=e.background;m?m.isColor&&(f.color.copy(m),e.background=null,p=!0):(f.color.copy(ad),p=!0);for(let t=0;t<6;t++){let n=t%3;n===0?(a.up.set(0,o[t],0),a.position.set(i.x,i.y,i.z),a.lookAt(i.x+s[t],i.y,i.z)):n===1?(a.up.set(0,0,o[t]),a.position.set(i.x,i.y,i.z),a.lookAt(i.x,i.y+s[t],i.z)):(a.up.set(0,o[t],0),a.position.set(i.x,i.y,i.z),a.lookAt(i.x,i.y,i.z+s[t]));let l=this._cubeSize;el(r,n*l,t>2?l:0,l,l),c.setRenderTarget(r),p&&c.render(d,a),c.render(e,a)}c.toneMapping=u,c.autoClear=l,e.background=m}_textureToCubeUV(e,t){let n=this._renderer,r=e.mapping===301||e.mapping===302;r?(this._cubemapMaterial===null&&(this._cubemapMaterial=il()),this._cubemapMaterial.uniforms.flipEnvMap.value=e.isRenderTargetTexture===!1?-1:1):this._equirectMaterial===null&&(this._equirectMaterial=rl());let i=r?this._cubemapMaterial:this._equirectMaterial,a=this._lodMeshes[0];a.material=i;let o=i.uniforms;o.envMap.value=e;let s=this._cubeSize;el(t,0,0,3*s,2*s),n.setRenderTarget(t),n.render(a,id)}_applyPMREM(e){let t=this._renderer,n=t.autoClear;t.autoClear=!1;let r=this._lodMeshes.length;for(let t=1;t<r;t++)this._applyGGXFilter(e,t-1,t);t.autoClear=n}_applyGGXFilter(e,t,n){let r=this._renderer,i=this._pingPongRenderTarget,a=this._ggxMaterial,o=this._lodMeshes[n];o.material=a;let s=a.uniforms,c=n/(this._lodMeshes.length-1),l=t/(this._lodMeshes.length-1),u=Math.sqrt(c*c-l*l)*(0+c*1.25),{_lodMax:d}=this,f=this._sizeLods[n],p=3*f*(n>d-ed?n-d+ed:0),m=4*(this._cubeSize-f);s.envMap.value=e.texture,s.roughness.value=u,s.mipInt.value=d-t,el(i,p,m,3*f,2*f),r.setRenderTarget(i),r.render(o,id),s.envMap.value=i.texture,s.roughness.value=0,s.mipInt.value=d-n,el(e,p,m,3*f,2*f),r.setRenderTarget(e),r.render(o,id)}_blur(e,t,n,r,i){let a=this._pingPongRenderTarget;this._halfBlur(e,a,t,n,r,`latitudinal`,i),this._halfBlur(a,e,n,n,r,`longitudinal`,i)}_halfBlur(e,t,n,r,i,a,o){let s=this._renderer,c=this._blurMaterial;a!==`latitudinal`&&a!==`longitudinal`&&G(`blur direction must be either latitudinal or longitudinal!`);let l=this._lodMeshes[r];l.material=c;let u=c.uniforms,d=this._sizeLods[n]-1,f=isFinite(i)?Math.PI/(2*d):2*Math.PI/(2*nd-1),p=i/f,m=isFinite(i)?1+Math.floor(3*p):nd;m>nd&&W(`sigmaRadians, ${i}, is too large and will clip, as it requested ${m} samples when the maximum is set to ${nd}`);let h=[],g=0;for(let e=0;e<nd;++e){let t=e/p,n=Math.exp(-t*t/2);h.push(n),e===0?g+=n:e<m&&(g+=2*n)}for(let e=0;e<h.length;e++)h[e]=h[e]/g;u.envMap.value=e.texture,u.samples.value=m,u.weights.value=h,u.latitudinal.value=a===`latitudinal`,o&&(u.poleAxis.value=o);let{_lodMax:_}=this;u.dTheta.value=f,u.mipInt.value=_-n;let v=this._sizeLods[r];el(t,3*v*(r>_-ed?r-_+ed:0),4*(this._cubeSize-v),3*v,2*v),s.setRenderTarget(t),s.render(l,id)}},fd=class extends Oi{constructor(e=1,t={}){super(e,e,t),this.isWebGLCubeRenderTarget=!0;let n={width:e,height:e,depth:1},r=[n,n,n,n,n,n];this.texture=new Ss(r),this._setTextureOptions(t),this.texture.isRenderTargetTexture=!0}fromEquirectangularTexture(e,t){this.texture.type=t.type,this.texture.colorSpace=t.colorSpace,this.texture.generateMipmaps=t.generateMipmaps,this.texture.minFilter=t.minFilter,this.texture.magFilter=t.magFilter;let n={uniforms:{tEquirect:{value:null}},vertexShader:`

				varying vec3 vWorldDirection;

				vec3 transformDirection( in vec3 dir, in mat4 matrix ) {

					return normalize( ( matrix * vec4( dir, 0.0 ) ).xyz );

				}

				void main() {

					vWorldDirection = transformDirection( position, modelMatrix );

					#include <begin_vertex>
					#include <project_vertex>

				}
			`,fragmentShader:`

				uniform sampler2D tEquirect;

				varying vec3 vWorldDirection;

				#include <common>

				void main() {

					vec3 direction = normalize( vWorldDirection );

					vec2 sampleUV = equirectUv( direction );

					gl_FragColor = texture2D( tEquirect, sampleUV );

				}
			`},r=new Es(5,5,5),i=new Ms({name:`CubemapFromEquirect`,uniforms:cn(n.uniforms),vertexShader:n.vertexShader,fragmentShader:n.fragmentShader,side:1,blending:0});i.uniforms.tEquirect.value=t;let a=new Oo(r,i),o=t.minFilter;return t.minFilter===1008&&(t.minFilter=An),new wc(1,10,this).update(e,a),t.minFilter=o,a.geometry.dispose(),a.material.dispose(),this}clear(e,t=!0,n=!0,r=!0){let i=e.getRenderTarget();for(let i=0;i<6;i++)e.setRenderTarget(this,i),e.clear(t,n,r);e.setRenderTarget(i)}},pd={1:`LINEAR_TONE_MAPPING`,2:`REINHARD_TONE_MAPPING`,3:`CINEON_TONE_MAPPING`,4:`ACES_FILMIC_TONE_MAPPING`,6:`AGX_TONE_MAPPING`,7:`NEUTRAL_TONE_MAPPING`,5:`CUSTOM_TONE_MAPPING`},md=new Ti,hd=new Cs(1,1),gd=new ki,_d=new Ai,vd=new Ss,yd=[],bd=[],xd=new Float32Array(16),Sd=new Float32Array(9),Cd=new Float32Array(4),wd=class{constructor(e,t,n){this.id=e,this.addr=n,this.cache=[],this.type=t.type,this.setValue=Ll(t.type)}},Td=class{constructor(e,t,n){this.id=e,this.addr=n,this.cache=[],this.type=t.type,this.size=t.size,this.setValue=ru(t.type)}},Ed=class{constructor(e){this.id=e,this.seq=[],this.map={}}setValue(e,t,n){let r=this.seq;for(let i=0,a=r.length;i!==a;++i){let a=r[i];a.setValue(e,t[a.id],n)}}},Dd=/(\w+)(\])?(\[|\.)?/g,Od=class{constructor(e,t){this.seq=[],this.map={};let n=e.getProgramParameter(t,e.ACTIVE_UNIFORMS);for(let r=0;r<n;++r){let n=e.getActiveUniform(t,r);au(n,e.getUniformLocation(t,n.name),this)}let r=[],i=[];for(let t of this.seq)t.type===e.SAMPLER_2D_SHADOW||t.type===e.SAMPLER_CUBE_SHADOW||t.type===e.SAMPLER_2D_ARRAY_SHADOW?r.push(t):i.push(t);r.length>0&&(this.seq=r.concat(i))}setValue(e,t,n,r){let i=this.map[t];i!==void 0&&i.setValue(e,n,r)}setOptional(e,t,n){let r=t[n];r!==void 0&&this.setValue(e,n,r)}static upload(e,t,n,r){for(let i=0,a=t.length;i!==a;++i){let a=t[i],o=n[a.id];o.needsUpdate!==!1&&a.setValue(e,o.value,r)}}static seqWithValue(e,t){let n=[];for(let r=0,i=e.length;r!==i;++r){let i=e[r];i.id in t&&n.push(i)}return n}},kd=37297,Ad=0,jd=new q,Md={1:`Linear`,2:`Reinhard`,3:`Cineon`,4:`ACESFilmic`,6:`AgX`,7:`Neutral`,5:`Custom`},Nd=new K,Pd=/^[ \t]*#include +<([\w\d./]+)>/gm,Fd=new Map,Id=/#pragma unroll_loop_start\s+for\s*\(\s*int\s+i\s*=\s*(\d+)\s*;\s*i\s*<\s*(\d+)\s*;\s*i\s*\+\+\s*\)\s*{([\s\S]+?)}\s+#pragma unroll_loop_end/g,Ld={1:`SHADOWMAP_TYPE_PCF`,3:`SHADOWMAP_TYPE_VSM`},Rd={301:`ENVMAP_TYPE_CUBE`,302:`ENVMAP_TYPE_CUBE`,306:`ENVMAP_TYPE_CUBE_UV`},zd={302:`ENVMAP_MODE_REFRACTION`},Bd={0:`ENVMAP_BLENDING_MULTIPLY`,1:`ENVMAP_BLENDING_MIX`,2:`ENVMAP_BLENDING_ADD`},Vd=0,Hd=class{constructor(){this.shaderCache=new Map,this.materialCache=new Map}update(e){let t=e.vertexShader,n=e.fragmentShader,r=this._getShaderStage(t),i=this._getShaderStage(n),a=this._getShaderCacheForMaterial(e);return a.has(r)===!1&&(a.add(r),r.usedTimes++),a.has(i)===!1&&(a.add(i),i.usedTimes++),this}remove(e){let t=this.materialCache.get(e);for(let e of t)e.usedTimes--,e.usedTimes===0&&this.shaderCache.delete(e.code);return this.materialCache.delete(e),this}getVertexShaderID(e){return this._getShaderStage(e.vertexShader).id}getFragmentShaderID(e){return this._getShaderStage(e.fragmentShader).id}dispose(){this.shaderCache.clear(),this.materialCache.clear()}_getShaderCacheForMaterial(e){let t=this.materialCache,n=t.get(e);return n===void 0&&(n=new Set,t.set(e,n)),n}_getShaderStage(e){let t=this.shaderCache,n=t.get(e);return n===void 0&&(n=new Ud(e),t.set(e,n)),n}},Ud=class{constructor(e){this.id=Vd++,this.code=e,this.usedTimes=0}},Wd=0,Gd=`void main() {
	gl_Position = vec4( position, 1.0 );
}`,Kd=`uniform sampler2D shadow_pass;
uniform vec2 resolution;
uniform float radius;
void main() {
	const float samples = float( VSM_SAMPLES );
	float mean = 0.0;
	float squared_mean = 0.0;
	float uvStride = samples <= 1.0 ? 0.0 : 2.0 / ( samples - 1.0 );
	float uvStart = samples <= 1.0 ? 0.0 : - 1.0;
	for ( float i = 0.0; i < samples; i ++ ) {
		float uvOffset = uvStart + i * uvStride;
		#ifdef HORIZONTAL_PASS
			vec2 distribution = texture2D( shadow_pass, ( gl_FragCoord.xy + vec2( uvOffset, 0.0 ) * radius ) / resolution ).rg;
			mean += distribution.x;
			squared_mean += distribution.y * distribution.y + distribution.x * distribution.x;
		#else
			float depth = texture2D( shadow_pass, ( gl_FragCoord.xy + vec2( 0.0, uvOffset ) * radius ) / resolution ).r;
			mean += depth;
			squared_mean += depth * depth;
		#endif
	}
	mean = mean / samples;
	squared_mean = squared_mean / samples;
	float std_dev = sqrt( max( 0.0, squared_mean - mean * mean ) );
	gl_FragColor = vec4( mean, std_dev, 0.0, 1.0 );
}`,qd=[new K(1,0,0),new K(-1,0,0),new K(0,1,0),new K(0,-1,0),new K(0,0,1),new K(0,0,-1)],Jd=[new K(0,-1,0),new K(0,-1,0),new K(0,0,1),new K(0,0,-1),new K(0,-1,0),new K(0,-1,0)],Yd=new Y,Xd=new K,Zd=new K,Qd=`
void main() {

	gl_Position = vec4( position, 1.0 );

}`,$d=`
uniform sampler2DArray depthColor;
uniform float depthWidth;
uniform float depthHeight;

void main() {

	vec2 coord = vec2( gl_FragCoord.x / depthWidth, gl_FragCoord.y / depthHeight );

	if ( coord.x >= 1.0 ) {

		gl_FragDepth = texture( depthColor, vec3( coord.x - 1.0, coord.y, 1 ) ).r;

	} else {

		gl_FragDepth = texture( depthColor, vec3( coord.x, coord.y, 0 ) ).r;

	}

}`,ef=class{constructor(){this.texture=null,this.mesh=null,this.depthNear=0,this.depthFar=0}init(e,t){if(this.texture===null){let n=new Ts(e.texture);(e.depthNear!==t.depthNear||e.depthFar!==t.depthFar)&&(this.depthNear=e.depthNear,this.depthFar=e.depthFar),this.texture=n}}getMesh(e){if(this.texture!==null&&this.mesh===null){let t=e.cameras[0].viewport,n=new Ms({vertexShader:Qd,fragmentShader:$d,uniforms:{depthColor:{value:this.texture},depthWidth:{value:t.z},depthHeight:{value:t.w}}});this.mesh=new Oo(new Ds(20,20),n)}return this.mesh}reset(){this.texture=null,this.mesh=null}getDepthTexture(){return this.texture}},tf=class extends ci{constructor(e,t){super();let n=this,r=null,i=1,a=null,o=`local-floor`,s=1,c=null,l=null,u=null,d=null,f=null,p=null,m=typeof XRWebGLBinding<`u`,h=new ef,g={},_=t.getContextAttributes(),v=null,y=null,b=[],x=[],S=new fi,C=null,w=new pc;w.viewport=new Ei;let T=new pc;T.viewport=new Ei;let E=[w,T],D=new Tc,O=null,ee=null;this.cameraAutoUpdate=!0,this.enabled=!1,this.isPresenting=!1,this.getController=function(e){let t=b[e];return t===void 0&&(t=new oa,b[e]=t),t.getTargetRaySpace()},this.getControllerGrip=function(e){let t=b[e];return t===void 0&&(t=new oa,b[e]=t),t.getGripSpace()},this.getHand=function(e){let t=b[e];return t===void 0&&(t=new oa,b[e]=t),t.getHandSpace()};function k(e){let t=x.indexOf(e.inputSource);if(t===-1)return;let n=b[t];n!==void 0&&(n.update(e.inputSource,e.frame,c||a),n.dispatchEvent({type:e.type,data:e.inputSource}))}function te(){r.removeEventListener(`select`,k),r.removeEventListener(`selectstart`,k),r.removeEventListener(`selectend`,k),r.removeEventListener(`squeeze`,k),r.removeEventListener(`squeezestart`,k),r.removeEventListener(`squeezeend`,k),r.removeEventListener(`end`,te),r.removeEventListener(`inputsourceschange`,ne);for(let e=0;e<b.length;e++){let t=x[e];t!==null&&(x[e]=null,b[e].disconnect(t))}O=null,ee=null,h.reset();for(let e in g)delete g[e];e.setRenderTarget(v),f=null,d=null,u=null,r=null,y=null,ue.stop(),n.isPresenting=!1,e.setPixelRatio(C),e.setSize(S.width,S.height,!1),n.dispatchEvent({type:`sessionend`})}this.setFramebufferScaleFactor=function(e){i=e,n.isPresenting===!0&&W(`WebXRManager: Cannot change framebuffer scale while presenting.`)},this.setReferenceSpaceType=function(e){o=e,n.isPresenting===!0&&W(`WebXRManager: Cannot change reference space type while presenting.`)},this.getReferenceSpace=function(){return c||a},this.setReferenceSpace=function(e){c=e},this.getBaseLayer=function(){return d===null?f:d},this.getBinding=function(){return u===null&&m&&(u=new XRWebGLBinding(r,t)),u},this.getFrame=function(){return p},this.getSession=function(){return r},this.setSession=async function(l){if(r=l,r!==null){if(v=e.getRenderTarget(),r.addEventListener(`select`,k),r.addEventListener(`selectstart`,k),r.addEventListener(`selectend`,k),r.addEventListener(`squeeze`,k),r.addEventListener(`squeezestart`,k),r.addEventListener(`squeezeend`,k),r.addEventListener(`end`,te),r.addEventListener(`inputsourceschange`,ne),_.xrCompatible!==!0&&await t.makeXRCompatible(),C=e.getPixelRatio(),e.getSize(S),m&&`createProjectionLayer`in XRWebGLBinding.prototype){let n=null,a=null,o=null;_.depth&&(o=_.stencil?t.DEPTH24_STENCIL8:t.DEPTH_COMPONENT24,n=_.stencil?Xn:Yn,a=_.stencil?Un:Rn);let s={colorFormat:t.RGBA8,depthFormat:o,scaleFactor:i};u=this.getBinding(),d=u.createProjectionLayer(s),r.updateRenderState({layers:[d]}),e.setPixelRatio(1),e.setSize(d.textureWidth,d.textureHeight,!1),y=new Oi(d.textureWidth,d.textureHeight,{format:Jn,type:Nn,depthTexture:new Cs(d.textureWidth,d.textureHeight,a,void 0,void 0,void 0,void 0,void 0,void 0,n),stencilBuffer:_.stencil,colorSpace:e.outputColorSpace,samples:_.antialias?4:0,resolveDepthBuffer:d.ignoreDepthValues===!1,resolveStencilBuffer:d.ignoreDepthValues===!1})}else{let n={antialias:_.antialias,alpha:!0,depth:_.depth,stencil:_.stencil,framebufferScaleFactor:i};f=new XRWebGLLayer(r,t,n),r.updateRenderState({baseLayer:f}),e.setPixelRatio(1),e.setSize(f.framebufferWidth,f.framebufferHeight,!1),y=new Oi(f.framebufferWidth,f.framebufferHeight,{format:Jn,type:Nn,colorSpace:e.outputColorSpace,stencilBuffer:_.stencil,resolveDepthBuffer:f.ignoreDepthValues===!1,resolveStencilBuffer:f.ignoreDepthValues===!1})}y.isXRRenderTarget=!0,this.setFoveation(s),c=null,a=await r.requestReferenceSpace(o),ue.setContext(r),ue.start(),n.isPresenting=!0,n.dispatchEvent({type:`sessionstart`})}},this.getEnvironmentBlendMode=function(){if(r!==null)return r.environmentBlendMode},this.getDepthTexture=function(){return h.getDepthTexture()};function ne(e){for(let t=0;t<e.removed.length;t++){let n=e.removed[t],r=x.indexOf(n);r>=0&&(x[r]=null,b[r].disconnect(n))}for(let t=0;t<e.added.length;t++){let n=e.added[t],r=x.indexOf(n);if(r===-1){for(let e=0;e<b.length;e++)if(e>=x.length){x.push(n),r=e;break}else if(x[e]===null){x[e]=n,r=e;break}if(r===-1)break}let i=b[r];i&&i.connect(n)}}let re=new K,ie=new K;function ae(e,t,n){re.setFromMatrixPosition(t.matrixWorld),ie.setFromMatrixPosition(n.matrixWorld);let r=re.distanceTo(ie),i=t.projectionMatrix.elements,a=n.projectionMatrix.elements,o=i[14]/(i[10]-1),s=i[14]/(i[10]+1),c=(i[9]+1)/i[5],l=(i[9]-1)/i[5],u=(i[8]-1)/i[0],d=(a[8]+1)/a[0],f=o*u,p=o*d,m=r/(-u+d),h=m*-u;if(t.matrixWorld.decompose(e.position,e.quaternion,e.scale),e.translateX(h),e.translateZ(m),e.matrixWorld.compose(e.position,e.quaternion,e.scale),e.matrixWorldInverse.copy(e.matrixWorld).invert(),i[10]===-1)e.projectionMatrix.copy(t.projectionMatrix),e.projectionMatrixInverse.copy(t.projectionMatrixInverse);else{let t=o+m,n=s+m,i=f-h,a=p+(r-h),u=c*s/n*t,d=l*s/n*t;e.projectionMatrix.makePerspective(i,a,u,d,t,n),e.projectionMatrixInverse.copy(e.projectionMatrix).invert()}}function oe(e,t){t===null?e.matrixWorld.copy(e.matrix):e.matrixWorld.multiplyMatrices(t.matrixWorld,e.matrix),e.matrixWorldInverse.copy(e.matrixWorld).invert()}this.updateCamera=function(e){if(r===null)return;let t=e.near,n=e.far;h.texture!==null&&(h.depthNear>0&&(t=h.depthNear),h.depthFar>0&&(n=h.depthFar)),D.near=T.near=w.near=t,D.far=T.far=w.far=n,(O!==D.near||ee!==D.far)&&(r.updateRenderState({depthNear:D.near,depthFar:D.far}),O=D.near,ee=D.far),D.layers.mask=e.layers.mask|6,w.layers.mask=D.layers.mask&-5,T.layers.mask=D.layers.mask&-3;let i=e.parent,a=D.cameras;oe(D,i);for(let e=0;e<a.length;e++)oe(a[e],i);a.length===2?ae(D,w,T):D.projectionMatrix.copy(w.projectionMatrix),se(e,D,i)};function se(e,t,n){n===null?e.matrix.copy(t.matrixWorld):(e.matrix.copy(n.matrixWorld),e.matrix.invert(),e.matrix.multiply(t.matrixWorld)),e.matrix.decompose(e.position,e.quaternion,e.scale),e.updateMatrixWorld(!0),e.projectionMatrix.copy(t.projectionMatrix),e.projectionMatrixInverse.copy(t.projectionMatrixInverse),e.isPerspectiveCamera&&(e.fov=di*2*Math.atan(1/e.projectionMatrix.elements[5]),e.zoom=1)}this.getCamera=function(){return D},this.getFoveation=function(){if(!(d===null&&f===null))return s},this.setFoveation=function(e){s=e,d!==null&&(d.fixedFoveation=e),f!==null&&f.fixedFoveation!==void 0&&(f.fixedFoveation=e)},this.hasDepthSensing=function(){return h.texture!==null},this.getDepthSensingMesh=function(){return h.getMesh(D)},this.getCameraTexture=function(e){return g[e]};let ce=null;function le(t,i){if(l=i.getViewerPose(c||a),p=i,l!==null){let t=l.views;f!==null&&(e.setRenderTargetFramebuffer(y,f.framebuffer),e.setRenderTarget(y));let i=!1;t.length!==D.cameras.length&&(D.cameras.length=0,i=!0);for(let n=0;n<t.length;n++){let r=t[n],a=null;if(f!==null)a=f.getViewport(r);else{let t=u.getViewSubImage(d,r);a=t.viewport,n===0&&(e.setRenderTargetTextures(y,t.colorTexture,t.depthStencilTexture),e.setRenderTarget(y))}let o=E[n];o===void 0&&(o=new pc,o.layers.enable(n),o.viewport=new Ei,E[n]=o),o.matrix.fromArray(r.transform.matrix),o.matrix.decompose(o.position,o.quaternion,o.scale),o.projectionMatrix.fromArray(r.projectionMatrix),o.projectionMatrixInverse.copy(o.projectionMatrix).invert(),o.viewport.set(a.x,a.y,a.width,a.height),n===0&&(D.matrix.copy(o.matrix),D.matrix.decompose(D.position,D.quaternion,D.scale)),i===!0&&D.cameras.push(o)}let a=r.enabledFeatures;if(a&&a.includes(`depth-sensing`)&&r.depthUsage==`gpu-optimized`&&m){u=n.getBinding();let e=u.getDepthInformation(t[0]);e&&e.isValid&&e.texture&&h.init(e,r.renderState)}if(a&&a.includes(`camera-access`)&&m){e.state.unbindTexture(),u=n.getBinding();for(let e=0;e<t.length;e++){let n=t[e].camera;if(n){let e=g[n];e||(e=new Ts,g[n]=e);let t=u.getCameraImage(n);e.sourceTexture=t}}}}for(let e=0;e<b.length;e++){let t=x[e],n=b[e];t!==null&&n!==void 0&&n.update(t,i,c||a)}ce&&ce(t,i),i.detectedPlanes&&n.dispatchEvent({type:`planesdetected`,data:i}),p=null}let ue=new Gc;ue.setAnimationLoop(le),this.setAnimationLoop=function(e){ce=e},this.dispose=function(){}}},nf=new Y,rf=new q,rf.set(-1,0,0,0,1,0,0,0,1),af=new Uint16Array([12469,15057,12620,14925,13266,14620,13807,14376,14323,13990,14545,13625,14713,13328,14840,12882,14931,12528,14996,12233,15039,11829,15066,11525,15080,11295,15085,10976,15082,10705,15073,10495,13880,14564,13898,14542,13977,14430,14158,14124,14393,13732,14556,13410,14702,12996,14814,12596,14891,12291,14937,11834,14957,11489,14958,11194,14943,10803,14921,10506,14893,10278,14858,9960,14484,14039,14487,14025,14499,13941,14524,13740,14574,13468,14654,13106,14743,12678,14818,12344,14867,11893,14889,11509,14893,11180,14881,10751,14852,10428,14812,10128,14765,9754,14712,9466,14764,13480,14764,13475,14766,13440,14766,13347,14769,13070,14786,12713,14816,12387,14844,11957,14860,11549,14868,11215,14855,10751,14825,10403,14782,10044,14729,9651,14666,9352,14599,9029,14967,12835,14966,12831,14963,12804,14954,12723,14936,12564,14917,12347,14900,11958,14886,11569,14878,11247,14859,10765,14828,10401,14784,10011,14727,9600,14660,9289,14586,8893,14508,8533,15111,12234,15110,12234,15104,12216,15092,12156,15067,12010,15028,11776,14981,11500,14942,11205,14902,10752,14861,10393,14812,9991,14752,9570,14682,9252,14603,8808,14519,8445,14431,8145,15209,11449,15208,11451,15202,11451,15190,11438,15163,11384,15117,11274,15055,10979,14994,10648,14932,10343,14871,9936,14803,9532,14729,9218,14645,8742,14556,8381,14461,8020,14365,7603,15273,10603,15272,10607,15267,10619,15256,10631,15231,10614,15182,10535,15118,10389,15042,10167,14963,9787,14883,9447,14800,9115,14710,8665,14615,8318,14514,7911,14411,7507,14279,7198,15314,9675,15313,9683,15309,9712,15298,9759,15277,9797,15229,9773,15166,9668,15084,9487,14995,9274,14898,8910,14800,8539,14697,8234,14590,7790,14479,7409,14367,7067,14178,6621,15337,8619,15337,8631,15333,8677,15325,8769,15305,8871,15264,8940,15202,8909,15119,8775,15022,8565,14916,8328,14804,8009,14688,7614,14569,7287,14448,6888,14321,6483,14088,6171,15350,7402,15350,7419,15347,7480,15340,7613,15322,7804,15287,7973,15229,8057,15148,8012,15046,7846,14933,7611,14810,7357,14682,7069,14552,6656,14421,6316,14251,5948,14007,5528,15356,5942,15356,5977,15353,6119,15348,6294,15332,6551,15302,6824,15249,7044,15171,7122,15070,7050,14949,6861,14818,6611,14679,6349,14538,6067,14398,5651,14189,5311,13935,4958,15359,4123,15359,4153,15356,4296,15353,4646,15338,5160,15311,5508,15263,5829,15188,6042,15088,6094,14966,6001,14826,5796,14678,5543,14527,5287,14377,4985,14133,4586,13869,4257,15360,1563,15360,1642,15358,2076,15354,2636,15341,3350,15317,4019,15273,4429,15203,4732,15105,4911,14981,4932,14836,4818,14679,4621,14517,4386,14359,4156,14083,3795,13808,3437,15360,122,15360,137,15358,285,15355,636,15344,1274,15322,2177,15281,2765,15215,3223,15120,3451,14995,3569,14846,3567,14681,3466,14511,3305,14344,3121,14037,2800,13753,2467,15360,0,15360,1,15359,21,15355,89,15346,253,15325,479,15287,796,15225,1148,15133,1492,15008,1749,14856,1882,14685,1886,14506,1783,14324,1608,13996,1398,13702,1183]),of=null,sf=class{constructor(e={}){let{canvas:t=Bt(),context:n=null,depth:r=!0,stencil:i=!1,alpha:a=!1,antialias:o=!1,premultipliedAlpha:s=!0,preserveDrawingBuffer:c=!1,powerPreference:l=`default`,failIfMajorPerformanceCaveat:u=!1,reversedDepthBuffer:d=!1,outputBufferType:f=Nn}=e;this.isWebGLRenderer=!0;let p;if(n!==null){if(typeof WebGLRenderingContext<`u`&&n instanceof WebGLRenderingContext)throw Error(`THREE.WebGLRenderer: WebGL 1 is not supported since r163.`);p=n.getContextAttributes().alpha}else p=a;let m=f,h=new Set([tr,er,Qn]),g=new Set([Nn,Rn,In,Un,Vn,Hn]),_=new Uint32Array(4),v=new Int32Array(4),y=new K,b=null,x=null,S=[],C=[],w=null;this.domElement=t,this.debug={checkShaderErrors:!0,onShaderError:null},this.autoClear=!0,this.autoClearColor=!0,this.autoClearDepth=!0,this.autoClearStencil=!0,this.sortObjects=!0,this.clippingPlanes=[],this.localClippingEnabled=!1,this.toneMapping=0,this.toneMappingExposure=1,this.transmissionResolutionScale=1;let T=this,E=!1,D=null;this._outputColorSpace=Zr;let O=0,ee=0,k=null,te=-1,ne=null,re=new Ei,ie=new Ei,ae=null,oe=new X(0),se=0,ce=t.width,le=t.height,ue=1,de=null,fe=null,pe=new Ei(0,0,ce,le),me=new Ei(0,0,ce,le),he=!1,ge=new rs,_e=!1,ve=!1,ye=new Y,be=new K,xe=new Ei,Se={background:null,fog:null,environment:null,overrideMaterial:null,isScene:!0},Ce=!1;function we(){return k===null?ue:1}let A=n;function Te(e,n){return t.getContext(e,n)}try{let e={alpha:!0,depth:r,stencil:i,antialias:o,premultipliedAlpha:s,preserveDrawingBuffer:c,powerPreference:l,failIfMajorPerformanceCaveat:u};if(`setAttribute`in t&&t.setAttribute(`data-engine`,`three.js r184`),t.addEventListener(`webglcontextlost`,V,!1),t.addEventListener(`webglcontextrestored`,Ve,!1),t.addEventListener(`webglcontextcreationerror`,He,!1),A===null){let t=`webgl2`;if(A=Te(t,e),A===null)throw Te(t)?Error(`Error creating WebGL context with your selected attributes.`):Error(`Error creating WebGL context.`)}}catch(e){throw G(`WebGLRenderer: `+e.message),e}let j,Ee,M,De,N,P,Oe,ke,Ae,je,F,Me,Ne,Pe,I,Fe,L,Ie,Le,Re,R,ze,z;function Be(){j=new sl(A),j.init(),R=new Ku(A,j),Ee=new Xc(A,j,e,R),M=new Wu(A,j),Ee.reversedDepthBuffer&&d&&M.buffers.depth.setReversed(!0),De=new ul(A),N=new Mu,P=new Gu(A,j,M,N,Ee,R,De),Oe=new ol(T),ke=new Kc(A),ze=new Jc(A,ke),Ae=new cl(A,ke,De,ze),je=new fl(A,Ae,ke,ze,De),Ie=new dl(A,Ee,P),I=new Zc(N),F=new ju(T,Oe,j,Ee,ze,I),Me=new qu(T,N),Ne=new Iu,Pe=new Hu(j),L=new qc(T,Oe,M,je,p,s),Fe=new Uu(T,je,Ee),z=new Ju(A,De,Ee,M),Le=new Yc(A,j,De),Re=new ll(A,j,De),De.programs=F.programs,T.capabilities=Ee,T.extensions=j,T.properties=N,T.renderLists=Ne,T.shadowMap=Fe,T.state=M,T.info=De}Be(),m!==1009&&(w=new pl(m,t.width,t.height,r,i));let B=new tf(T,A);this.xr=B,this.getContext=function(){return A},this.getContextAttributes=function(){return A.getContextAttributes()},this.forceContextLoss=function(){let e=j.get(`WEBGL_lose_context`);e&&e.loseContext()},this.forceContextRestore=function(){let e=j.get(`WEBGL_lose_context`);e&&e.restoreContext()},this.getPixelRatio=function(){return ue},this.setPixelRatio=function(e){e!==void 0&&(ue=e,this.setSize(ce,le,!1))},this.getSize=function(e){return e.set(ce,le)},this.setSize=function(e,n,r=!0){if(B.isPresenting){W(`WebGLRenderer: Can't change size while VR device is presenting.`);return}ce=e,le=n,t.width=Math.floor(e*ue),t.height=Math.floor(n*ue),r===!0&&(t.style.width=e+`px`,t.style.height=n+`px`),w!==null&&w.setSize(t.width,t.height),this.setViewport(0,0,e,n)},this.getDrawingBufferSize=function(e){return e.set(ce*ue,le*ue).floor()},this.setDrawingBufferSize=function(e,n,r){ce=e,le=n,ue=r,t.width=Math.floor(e*r),t.height=Math.floor(n*r),this.setViewport(0,0,e,n)},this.setEffects=function(e){if(m===1009){G(`THREE.WebGLRenderer: setEffects() requires outputBufferType set to HalfFloatType or FloatType.`);return}if(e){for(let t=0;t<e.length;t++)if(e[t].isOutputPass===!0){W(`THREE.WebGLRenderer: OutputPass is not needed in setEffects(). Tone mapping and color space conversion are applied automatically.`);break}}w.setEffects(e||[])},this.getCurrentViewport=function(e){return e.copy(re)},this.getViewport=function(e){return e.copy(pe)},this.setViewport=function(e,t,n,r){e.isVector4?pe.set(e.x,e.y,e.z,e.w):pe.set(e,t,n,r),M.viewport(re.copy(pe).multiplyScalar(ue).round())},this.getScissor=function(e){return e.copy(me)},this.setScissor=function(e,t,n,r){e.isVector4?me.set(e.x,e.y,e.z,e.w):me.set(e,t,n,r),M.scissor(ie.copy(me).multiplyScalar(ue).round())},this.getScissorTest=function(){return he},this.setScissorTest=function(e){M.setScissorTest(he=e)},this.setOpaqueSort=function(e){de=e},this.setTransparentSort=function(e){fe=e},this.getClearColor=function(e){return e.copy(L.getClearColor())},this.setClearColor=function(){L.setClearColor(...arguments)},this.getClearAlpha=function(){return L.getClearAlpha()},this.setClearAlpha=function(){L.setClearAlpha(...arguments)},this.clear=function(e=!0,t=!0,n=!0){let r=0;if(e){let e=!1;if(k!==null){let t=k.texture.format;e=h.has(t)}if(e){let e=k.texture.type,t=g.has(e),n=L.getClearColor(),r=L.getClearAlpha(),i=n.r,a=n.g,o=n.b;t?(_[0]=i,_[1]=a,_[2]=o,_[3]=r,A.clearBufferuiv(A.COLOR,0,_)):(v[0]=i,v[1]=a,v[2]=o,v[3]=r,A.clearBufferiv(A.COLOR,0,v))}else r|=A.COLOR_BUFFER_BIT}t&&(r|=A.DEPTH_BUFFER_BIT,this.state.buffers.depth.setMask(!0)),n&&(r|=A.STENCIL_BUFFER_BIT,this.state.buffers.stencil.setMask(4294967295)),r!==0&&A.clear(r)},this.clearColor=function(){this.clear(!0,!1,!1)},this.clearDepth=function(){this.clear(!1,!0,!1)},this.clearStencil=function(){this.clear(!1,!1,!0)},this.setNodesHandler=function(e){e.setRenderer(this),D=e},this.dispose=function(){t.removeEventListener(`webglcontextlost`,V,!1),t.removeEventListener(`webglcontextrestored`,Ve,!1),t.removeEventListener(`webglcontextcreationerror`,He,!1),L.dispose(),Ne.dispose(),Pe.dispose(),N.dispose(),Oe.dispose(),je.dispose(),ze.dispose(),z.dispose(),F.dispose(),B.dispose(),B.removeEventListener(`sessionstart`,Ye),B.removeEventListener(`sessionend`,Xe),Ze.stop()};function V(e){e.preventDefault(),Vt(`WebGLRenderer: Context Lost.`),E=!0}function Ve(){Vt(`WebGLRenderer: Context Restored.`),E=!1;let e=De.autoReset,t=Fe.enabled,n=Fe.autoUpdate,r=Fe.needsUpdate,i=Fe.type;Be(),De.autoReset=e,Fe.enabled=t,Fe.autoUpdate=n,Fe.needsUpdate=r,Fe.type=i}function He(e){G(`WebGLRenderer: A WebGL context could not be created. Reason: `,e.statusMessage)}function Ue(e){let t=e.target;t.removeEventListener(`dispose`,Ue),We(t)}function We(e){Ge(e),N.remove(e)}function Ge(e){let t=N.get(e).programs;t!==void 0&&(t.forEach(function(e){F.releaseProgram(e)}),e.isShaderMaterial&&F.releaseShaderCache(e))}this.renderBufferDirect=function(e,t,n,r,i,a){t===null&&(t=Se);let o=i.isMesh&&i.matrixWorld.determinant()<0,s=st(e,t,n,r,i);M.setMaterial(r,o);let c=n.index,l=1;if(r.wireframe===!0){if(c=Ae.getWireframeAttribute(n),c===void 0)return;l=2}let u=n.drawRange,d=n.attributes.position,f=u.start*l,p=(u.start+u.count)*l;a!==null&&(f=Math.max(f,a.start*l),p=Math.min(p,(a.start+a.count)*l)),c===null?d!=null&&(f=Math.max(f,0),p=Math.min(p,d.count)):(f=Math.max(f,0),p=Math.min(p,c.count));let m=p-f;if(m<0||m===1/0)return;ze.setup(i,r,s,n,c);let h,g=Le;if(c!==null&&(h=ke.get(c),g=Re,g.setIndex(h)),i.isMesh)r.wireframe===!0?(M.setLineWidth(r.wireframeLinewidth*we()),g.setMode(A.LINES)):g.setMode(A.TRIANGLES);else if(i.isLine){let e=r.linewidth;e===void 0&&(e=1),M.setLineWidth(e*we()),i.isLineSegments?g.setMode(A.LINES):i.isLineLoop?g.setMode(A.LINE_LOOP):g.setMode(A.LINE_STRIP)}else i.isPoints?g.setMode(A.POINTS):i.isSprite&&g.setMode(A.TRIANGLES);if(i.isBatchedMesh)if(j.get(`WEBGL_multi_draw`))g.renderMultiDraw(i._multiDrawStarts,i._multiDrawCounts,i._multiDrawCount);else{let e=i._multiDrawStarts,t=i._multiDrawCounts,n=i._multiDrawCount,a=c?ke.get(c).bytesPerElement:1,o=N.get(r).currentProgram.getUniforms();for(let r=0;r<n;r++)o.setValue(A,`_gl_DrawID`,r),g.render(e[r]/a,t[r])}else if(i.isInstancedMesh)g.renderInstances(f,m,i.count);else if(n.isInstancedBufferGeometry){let e=n._maxInstanceCount===void 0?1/0:n._maxInstanceCount,t=Math.min(n.instanceCount,e);g.renderInstances(f,m,t)}else g.render(f,m)};function Ke(e,t,n){e.transparent===!0&&e.side===2&&e.forceSinglePass===!1?(e.side=1,e.needsUpdate=!0,rt(e,t,n),e.side=0,e.needsUpdate=!0,rt(e,t,n),e.side=2):rt(e,t,n)}this.compile=function(e,t,n=null){n===null&&(n=e),x=Pe.get(n),x.init(t),C.push(x),n.traverseVisible(function(e){e.isLight&&e.layers.test(t.layers)&&(x.pushLight(e),e.castShadow&&x.pushShadow(e))}),e!==n&&e.traverseVisible(function(e){e.isLight&&e.layers.test(t.layers)&&(x.pushLight(e),e.castShadow&&x.pushShadow(e))}),x.setupLights();let r=new Set;return e.traverse(function(e){if(!(e.isMesh||e.isPoints||e.isLine||e.isSprite))return;let t=e.material;if(t)if(Array.isArray(t))for(let i=0;i<t.length;i++){let a=t[i];Ke(a,n,e),r.add(a)}else Ke(t,n,e),r.add(t)}),x=C.pop(),r},this.compileAsync=function(e,t,n=null){let r=this.compile(e,t,n);return new Promise(t=>{function n(){if(r.forEach(function(e){N.get(e).currentProgram.isReady()&&r.delete(e)}),r.size===0){t(e);return}setTimeout(n,10)}j.get(`KHR_parallel_shader_compile`)===null?setTimeout(n,10):n()})};let qe=null;function Je(e){qe&&qe(e)}function Ye(){Ze.stop()}function Xe(){Ze.start()}let Ze=new Gc;Ze.setAnimationLoop(Je),typeof self<`u`&&Ze.setContext(self),this.setAnimationLoop=function(e){qe=e,B.setAnimationLoop(e),e===null?Ze.stop():Ze.start()},B.addEventListener(`sessionstart`,Ye),B.addEventListener(`sessionend`,Xe),this.render=function(e,t){if(t!==void 0&&t.isCamera!==!0){G(`WebGLRenderer.render: camera is not an instance of THREE.Camera.`);return}if(E===!0)return;D!==null&&D.renderStart(e,t);let n=B.enabled===!0&&B.isPresenting===!0,r=w!==null&&(k===null||n)&&w.begin(T,k);if(e.matrixWorldAutoUpdate===!0&&e.updateMatrixWorld(),t.parent===null&&t.matrixWorldAutoUpdate===!0&&t.updateMatrixWorld(),B.enabled===!0&&B.isPresenting===!0&&(w===null||w.isCompositing()===!1)&&(B.cameraAutoUpdate===!0&&B.updateCamera(t),t=B.getCamera()),e.isScene===!0&&e.onBeforeRender(T,e,t,k),x=Pe.get(e,C.length),x.init(t),x.state.textureUnits=P.getTextureUnits(),C.push(x),ye.multiplyMatrices(t.projectionMatrix,t.matrixWorldInverse),ge.setFromProjectionMatrix(ye,ii,t.reversedDepth),ve=this.localClippingEnabled,_e=I.init(this.clippingPlanes,ve),b=Ne.get(e,S.length),b.init(),S.push(b),B.enabled===!0&&B.isPresenting===!0){let e=T.xr.getDepthSensingMesh();e!==null&&Qe(e,t,-1/0,T.sortObjects)}Qe(e,t,0,T.sortObjects),b.finish(),T.sortObjects===!0&&b.sort(de,fe),Ce=B.enabled===!1||B.isPresenting===!1||B.hasDepthSensing()===!1,Ce&&L.addToRenderList(b,e),this.info.render.frame++,_e===!0&&I.beginShadows();let i=x.state.shadowsArray;if(Fe.render(i,e,t),_e===!0&&I.endShadows(),this.info.autoReset===!0&&this.info.reset(),(r&&w.hasRenderPass())===!1){let n=b.opaque,r=b.transmissive;if(x.setupLights(),t.isArrayCamera){let i=t.cameras;if(r.length>0)for(let t=0,a=i.length;t<a;t++){let a=i[t];et(n,r,e,a)}Ce&&L.render(e);for(let t=0,n=i.length;t<n;t++){let n=i[t];$e(b,e,n,n.viewport)}}else r.length>0&&et(n,r,e,t),Ce&&L.render(e),$e(b,e,t)}k!==null&&ee===0&&(P.updateMultisampleRenderTarget(k),P.updateRenderTargetMipmap(k)),r&&w.end(T),e.isScene===!0&&e.onAfterRender(T,e,t),ze.resetDefaultState(),te=-1,ne=null,C.pop(),C.length>0?(x=C[C.length-1],P.setTextureUnits(x.state.textureUnits),_e===!0&&I.setGlobalState(T.clippingPlanes,x.state.camera)):x=null,S.pop(),b=S.length>0?S[S.length-1]:null,D!==null&&D.renderEnd()};function Qe(e,t,n,r){if(e.visible===!1)return;if(e.layers.test(t.layers)){if(e.isGroup)n=e.renderOrder;else if(e.isLOD)e.autoUpdate===!0&&e.update(t);else if(e.isLightProbeGrid)x.pushLightProbeGrid(e);else if(e.isLight)x.pushLight(e),e.castShadow&&x.pushShadow(e);else if(e.isSprite){if(!e.frustumCulled||ge.intersectsSprite(e)){r&&xe.setFromMatrixPosition(e.matrixWorld).applyMatrix4(ye);let t=je.update(e),i=e.material;i.visible&&b.push(e,t,i,n,xe.z,null)}}else if((e.isMesh||e.isLine||e.isPoints)&&(!e.frustumCulled||ge.intersectsObject(e))){let t=je.update(e),i=e.material;if(r&&(e.boundingSphere===void 0?(t.boundingSphere===null&&t.computeBoundingSphere(),xe.copy(t.boundingSphere.center)):(e.boundingSphere===null&&e.computeBoundingSphere(),xe.copy(e.boundingSphere.center)),xe.applyMatrix4(e.matrixWorld).applyMatrix4(ye)),Array.isArray(i)){let r=t.groups;for(let a=0,o=r.length;a<o;a++){let o=r[a],s=i[o.materialIndex];s&&s.visible&&b.push(e,t,s,n,xe.z,o)}}else i.visible&&b.push(e,t,i,n,xe.z,null)}}let i=e.children;for(let e=0,a=i.length;e<a;e++)Qe(i[e],t,n,r)}function $e(e,t,n,r){let{opaque:i,transmissive:a,transparent:o}=e;x.setupLightsView(n),_e===!0&&I.setGlobalState(T.clippingPlanes,n),r&&M.viewport(re.copy(r)),i.length>0&&tt(i,t,n),a.length>0&&tt(a,t,n),o.length>0&&tt(o,t,n),M.buffers.depth.setTest(!0),M.buffers.depth.setMask(!0),M.buffers.color.setMask(!0),M.setPolygonOffset(!1)}function et(e,t,n,r){if((n.isScene===!0?n.overrideMaterial:null)!==null)return;if(x.state.transmissionRenderTarget[r.id]===void 0){let e=j.has(`EXT_color_buffer_half_float`)||j.has(`EXT_color_buffer_float`);x.state.transmissionRenderTarget[r.id]=new Oi(1,1,{generateMipmaps:!0,type:e?Bn:Nn,minFilter:Mn,samples:Math.max(4,Ee.samples),stencilBuffer:i,resolveDepthBuffer:!1,resolveStencilBuffer:!1,colorSpace:J.workingColorSpace})}let a=x.state.transmissionRenderTarget[r.id],o=r.viewport||re;a.setSize(o.z*T.transmissionResolutionScale,o.w*T.transmissionResolutionScale);let s=T.getRenderTarget(),c=T.getActiveCubeFace(),l=T.getActiveMipmapLevel();T.setRenderTarget(a),T.getClearColor(oe),se=T.getClearAlpha(),se<1&&T.setClearColor(16777215,.5),T.clear(),Ce&&L.render(n);let u=T.toneMapping;T.toneMapping=0;let d=r.viewport;if(r.viewport!==void 0&&(r.viewport=void 0),x.setupLightsView(r),_e===!0&&I.setGlobalState(T.clippingPlanes,r),tt(e,n,r),P.updateMultisampleRenderTarget(a),P.updateRenderTargetMipmap(a),j.has(`WEBGL_multisampled_render_to_texture`)===!1){let e=!1;for(let i=0,a=t.length;i<a;i++){let{object:a,geometry:o,material:s,group:c}=t[i];if(s.side===2&&a.layers.test(r.layers)){let t=s.side;s.side=1,s.needsUpdate=!0,nt(a,n,r,o,s,c),s.side=t,s.needsUpdate=!0,e=!0}}e===!0&&(P.updateMultisampleRenderTarget(a),P.updateRenderTargetMipmap(a))}T.setRenderTarget(s,c,l),T.setClearColor(oe,se),d!==void 0&&(r.viewport=d),T.toneMapping=u}function tt(e,t,n){let r=t.isScene===!0?t.overrideMaterial:null;for(let i=0,a=e.length;i<a;i++){let a=e[i],{object:o,geometry:s,group:c}=a,l=a.material;l.allowOverride===!0&&r!==null&&(l=r),o.layers.test(n.layers)&&nt(o,t,n,s,l,c)}}function nt(e,t,n,r,i,a){e.onBeforeRender(T,t,n,r,i,a),e.modelViewMatrix.multiplyMatrices(n.matrixWorldInverse,e.matrixWorld),e.normalMatrix.getNormalMatrix(e.modelViewMatrix),i.onBeforeRender(T,t,n,r,e,a),i.transparent===!0&&i.side===2&&i.forceSinglePass===!1?(i.side=1,i.needsUpdate=!0,T.renderBufferDirect(n,t,r,i,e,a),i.side=0,i.needsUpdate=!0,T.renderBufferDirect(n,t,r,i,e,a),i.side=2):T.renderBufferDirect(n,t,r,i,e,a),e.onAfterRender(T,t,n,r,i,a)}function rt(e,t,n){t.isScene!==!0&&(t=Se);let r=N.get(e),i=x.state.lights,a=x.state.shadowsArray,o=i.state.version,s=F.getParameters(e,i.state,a,t,n,x.state.lightProbeGridArray),c=F.getProgramCacheKey(s),l=r.programs;r.environment=e.isMeshStandardMaterial||e.isMeshLambertMaterial||e.isMeshPhongMaterial?t.environment:null,r.fog=t.fog;let u=e.isMeshStandardMaterial||e.isMeshLambertMaterial&&!e.envMap||e.isMeshPhongMaterial&&!e.envMap;r.envMap=Oe.get(e.envMap||r.environment,u),r.envMapRotation=r.environment!==null&&e.envMap===null?t.environmentRotation:e.envMapRotation,l===void 0&&(e.addEventListener(`dispose`,Ue),l=new Map,r.programs=l);let d=l.get(c);if(d!==void 0){if(r.currentProgram===d&&r.lightsStateVersion===o)return at(e,s),d}else s.uniforms=F.getUniforms(e),D!==null&&e.isNodeMaterial&&D.build(e,n,s),e.onBeforeCompile(s,T),d=F.acquireProgram(s,c),l.set(c,d),r.uniforms=s.uniforms;let f=r.uniforms;return(!e.isShaderMaterial&&!e.isRawShaderMaterial||e.clipping===!0)&&(f.clippingPlanes=I.uniform),at(e,s),r.needsLights=lt(e),r.lightsStateVersion=o,r.needsLights&&(f.ambientLightColor.value=i.state.ambient,f.lightProbe.value=i.state.probe,f.directionalLights.value=i.state.directional,f.directionalLightShadows.value=i.state.directionalShadow,f.spotLights.value=i.state.spot,f.spotLightShadows.value=i.state.spotShadow,f.rectAreaLights.value=i.state.rectArea,f.ltc_1.value=i.state.rectAreaLTC1,f.ltc_2.value=i.state.rectAreaLTC2,f.pointLights.value=i.state.point,f.pointLightShadows.value=i.state.pointShadow,f.hemisphereLights.value=i.state.hemi,f.directionalShadowMatrix.value=i.state.directionalShadowMatrix,f.spotLightMatrix.value=i.state.spotLightMatrix,f.spotLightMap.value=i.state.spotLightMap,f.pointShadowMatrix.value=i.state.pointShadowMatrix),r.lightProbeGrid=x.state.lightProbeGridArray.length>0,r.currentProgram=d,r.uniformsList=null,d}function it(e){if(e.uniformsList===null){let t=e.currentProgram.getUniforms();e.uniformsList=Od.seqWithValue(t.seq,e.uniforms)}return e.uniformsList}function at(e,t){let n=N.get(e);n.outputColorSpace=t.outputColorSpace,n.batching=t.batching,n.batchingColor=t.batchingColor,n.instancing=t.instancing,n.instancingColor=t.instancingColor,n.instancingMorph=t.instancingMorph,n.skinning=t.skinning,n.morphTargets=t.morphTargets,n.morphNormals=t.morphNormals,n.morphColors=t.morphColors,n.morphTargetsCount=t.morphTargetsCount,n.numClippingPlanes=t.numClippingPlanes,n.numIntersection=t.numClipIntersection,n.vertexAlphas=t.vertexAlphas,n.vertexTangents=t.vertexTangents,n.toneMapping=t.toneMapping}function ot(e,t){if(e.length===0)return null;if(e.length===1)return e[0].texture===null?null:e[0];y.setFromMatrixPosition(t.matrixWorld);for(let t=0,n=e.length;t<n;t++){let n=e[t];if(n.texture!==null&&n.boundingBox.containsPoint(y))return n}return null}function st(e,t,n,r,i){t.isScene!==!0&&(t=Se),P.resetTextureUnits();let a=t.fog,o=r.isMeshStandardMaterial||r.isMeshLambertMaterial||r.isMeshPhongMaterial?t.environment:null,s=k===null?T.outputColorSpace:k.isXRRenderTarget===!0?k.texture.colorSpace:J.workingColorSpace,c=r.isMeshStandardMaterial||r.isMeshLambertMaterial&&!r.envMap||r.isMeshPhongMaterial&&!r.envMap,l=Oe.get(r.envMap||o,c),u=r.vertexColors===!0&&!!n.attributes.color&&n.attributes.color.itemSize===4,d=!!n.attributes.tangent&&(!!r.normalMap||r.anisotropy>0),f=!!n.morphAttributes.position,p=!!n.morphAttributes.normal,m=!!n.morphAttributes.color,h=0;r.toneMapped&&(k===null||k.isXRRenderTarget===!0)&&(h=T.toneMapping);let g=n.morphAttributes.position||n.morphAttributes.normal||n.morphAttributes.color,_=g===void 0?0:g.length,v=N.get(r),y=x.state.lights;if(_e===!0&&(ve===!0||e!==ne)){let t=e===ne&&r.id===te;I.setState(r,e,t)}let b=!1;r.version===v.__version?v.needsLights&&v.lightsStateVersion!==y.state.version?b=!0:v.outputColorSpace===s?i.isBatchedMesh&&v.batching===!1||!i.isBatchedMesh&&v.batching===!0||i.isBatchedMesh&&v.batchingColor===!0&&i.colorTexture===null||i.isBatchedMesh&&v.batchingColor===!1&&i.colorTexture!==null||i.isInstancedMesh&&v.instancing===!1||!i.isInstancedMesh&&v.instancing===!0||i.isSkinnedMesh&&v.skinning===!1||!i.isSkinnedMesh&&v.skinning===!0||i.isInstancedMesh&&v.instancingColor===!0&&i.instanceColor===null||i.isInstancedMesh&&v.instancingColor===!1&&i.instanceColor!==null||i.isInstancedMesh&&v.instancingMorph===!0&&i.morphTexture===null||i.isInstancedMesh&&v.instancingMorph===!1&&i.morphTexture!==null?b=!0:v.envMap===l?r.fog===!0&&v.fog!==a||v.numClippingPlanes!==void 0&&(v.numClippingPlanes!==I.numPlanes||v.numIntersection!==I.numIntersection)?b=!0:v.vertexAlphas===u&&v.vertexTangents===d&&v.morphTargets===f&&v.morphNormals===p&&v.morphColors===m&&v.toneMapping===h&&v.morphTargetsCount===_?!!v.lightProbeGrid!=x.state.lightProbeGridArray.length>0&&(b=!0):b=!0:b=!0:b=!0:(b=!0,v.__version=r.version);let S=v.currentProgram;b===!0&&(S=rt(r,t,i),D&&r.isNodeMaterial&&D.onUpdateProgram(r,S,v));let C=!1,w=!1,E=!1,O=S.getUniforms(),ee=v.uniforms;if(M.useProgram(S.program)&&(C=!0,w=!0,E=!0),r.id!==te&&(te=r.id,w=!0),v.needsLights){let e=ot(x.state.lightProbeGridArray,i);v.lightProbeGrid!==e&&(v.lightProbeGrid=e,w=!0)}if(C||ne!==e){M.buffers.depth.getReversed()&&e.reversedDepth!==!0&&(e._reversedDepth=!0,e.updateProjectionMatrix()),O.setValue(A,`projectionMatrix`,e.projectionMatrix),O.setValue(A,`viewMatrix`,e.matrixWorldInverse);let t=O.map.cameraPosition;t!==void 0&&t.setValue(A,be.setFromMatrixPosition(e.matrixWorld)),Ee.logarithmicDepthBuffer&&O.setValue(A,`logDepthBufFC`,2/(Math.log(e.far+1)/Math.LN2)),(r.isMeshPhongMaterial||r.isMeshToonMaterial||r.isMeshLambertMaterial||r.isMeshBasicMaterial||r.isMeshStandardMaterial||r.isShaderMaterial)&&O.setValue(A,`isOrthographic`,e.isOrthographicCamera===!0),ne!==e&&(ne=e,w=!0,E=!0)}if(v.needsLights&&(y.state.directionalShadowMap.length>0&&O.setValue(A,`directionalShadowMap`,y.state.directionalShadowMap,P),y.state.spotShadowMap.length>0&&O.setValue(A,`spotShadowMap`,y.state.spotShadowMap,P),y.state.pointShadowMap.length>0&&O.setValue(A,`pointShadowMap`,y.state.pointShadowMap,P)),i.isSkinnedMesh){O.setOptional(A,i,`bindMatrix`),O.setOptional(A,i,`bindMatrixInverse`);let e=i.skeleton;e&&(e.boneTexture===null&&e.computeBoneTexture(),O.setValue(A,`boneTexture`,e.boneTexture,P))}i.isBatchedMesh&&(O.setOptional(A,i,`batchingTexture`),O.setValue(A,`batchingTexture`,i._matricesTexture,P),O.setOptional(A,i,`batchingIdTexture`),O.setValue(A,`batchingIdTexture`,i._indirectTexture,P),O.setOptional(A,i,`batchingColorTexture`),i._colorsTexture!==null&&O.setValue(A,`batchingColorTexture`,i._colorsTexture,P));let re=n.morphAttributes;if((re.position!==void 0||re.normal!==void 0||re.color!==void 0)&&Ie.update(i,n,S),(w||v.receiveShadow!==i.receiveShadow)&&(v.receiveShadow=i.receiveShadow,O.setValue(A,`receiveShadow`,i.receiveShadow)),(r.isMeshStandardMaterial||r.isMeshLambertMaterial||r.isMeshPhongMaterial)&&r.envMap===null&&t.environment!==null&&(ee.envMapIntensity.value=t.environmentIntensity),ee.dfgLUT!==void 0&&(ee.dfgLUT.value=Yu()),w){if(O.setValue(A,`toneMappingExposure`,T.toneMappingExposure),v.needsLights&&ct(ee,E),a&&r.fog===!0&&Me.refreshFogUniforms(ee,a),Me.refreshMaterialUniforms(ee,r,ue,le,x.state.transmissionRenderTarget[e.id]),v.needsLights&&v.lightProbeGrid){let e=v.lightProbeGrid;ee.probesSH.value=e.texture,ee.probesMin.value.copy(e.boundingBox.min),ee.probesMax.value.copy(e.boundingBox.max),ee.probesResolution.value.copy(e.resolution)}Od.upload(A,it(v),ee,P)}if(r.isShaderMaterial&&r.uniformsNeedUpdate===!0&&(Od.upload(A,it(v),ee,P),r.uniformsNeedUpdate=!1),r.isSpriteMaterial&&O.setValue(A,`center`,i.center),O.setValue(A,`modelViewMatrix`,i.modelViewMatrix),O.setValue(A,`normalMatrix`,i.normalMatrix),O.setValue(A,`modelMatrix`,i.matrixWorld),r.uniformsGroups!==void 0){let e=r.uniformsGroups;for(let t=0,n=e.length;t<n;t++){let n=e[t];z.update(n,S),z.bind(n,S)}}return S}function ct(e,t){e.ambientLightColor.needsUpdate=t,e.lightProbe.needsUpdate=t,e.directionalLights.needsUpdate=t,e.directionalLightShadows.needsUpdate=t,e.pointLights.needsUpdate=t,e.pointLightShadows.needsUpdate=t,e.spotLights.needsUpdate=t,e.spotLightShadows.needsUpdate=t,e.rectAreaLights.needsUpdate=t,e.hemisphereLights.needsUpdate=t}function lt(e){return e.isMeshLambertMaterial||e.isMeshToonMaterial||e.isMeshPhongMaterial||e.isMeshStandardMaterial||e.isShadowMaterial||e.isShaderMaterial&&e.lights===!0}this.getActiveCubeFace=function(){return O},this.getActiveMipmapLevel=function(){return ee},this.getRenderTarget=function(){return k},this.setRenderTargetTextures=function(e,t,n){let r=N.get(e);r.__autoAllocateDepthBuffer=e.resolveDepthBuffer===!1,r.__autoAllocateDepthBuffer===!1&&(r.__useRenderToTexture=!1),N.get(e.texture).__webglTexture=t,N.get(e.depthTexture).__webglTexture=r.__autoAllocateDepthBuffer?void 0:n,r.__hasExternalTextures=!0},this.setRenderTargetFramebuffer=function(e,t){let n=N.get(e);n.__webglFramebuffer=t,n.__useDefaultFramebuffer=t===void 0};let ut=A.createFramebuffer();this.setRenderTarget=function(e,t=0,n=0){k=e,O=t,ee=n;let r=null,i=!1,a=!1;if(e){let o=N.get(e);if(o.__useDefaultFramebuffer!==void 0){M.bindFramebuffer(A.FRAMEBUFFER,o.__webglFramebuffer),re.copy(e.viewport),ie.copy(e.scissor),ae=e.scissorTest,M.viewport(re),M.scissor(ie),M.setScissorTest(ae),te=-1;return}else if(o.__webglFramebuffer===void 0)P.setupRenderTarget(e);else if(o.__hasExternalTextures)P.rebindTextures(e,N.get(e.texture).__webglTexture,N.get(e.depthTexture).__webglTexture);else if(e.depthBuffer){let t=e.depthTexture;if(o.__boundDepthTexture!==t){if(t!==null&&N.has(t)&&(e.width!==t.image.width||e.height!==t.image.height))throw Error(`WebGLRenderTarget: Attached DepthTexture is initialized to the incorrect size.`);P.setupDepthRenderbuffer(e)}}let s=e.texture;(s.isData3DTexture||s.isDataArrayTexture||s.isCompressedArrayTexture)&&(a=!0);let c=N.get(e).__webglFramebuffer;e.isWebGLCubeRenderTarget?(r=Array.isArray(c[t])?c[t][n]:c[t],i=!0):r=e.samples>0&&P.useMultisampledRTT(e)===!1?N.get(e).__webglMultisampledFramebuffer:Array.isArray(c)?c[n]:c,re.copy(e.viewport),ie.copy(e.scissor),ae=e.scissorTest}else re.copy(pe).multiplyScalar(ue).floor(),ie.copy(me).multiplyScalar(ue).floor(),ae=he;if(n!==0&&(r=ut),M.bindFramebuffer(A.FRAMEBUFFER,r)&&M.drawBuffers(e,r),M.viewport(re),M.scissor(ie),M.setScissorTest(ae),i){let r=N.get(e.texture);A.framebufferTexture2D(A.FRAMEBUFFER,A.COLOR_ATTACHMENT0,A.TEXTURE_CUBE_MAP_POSITIVE_X+t,r.__webglTexture,n)}else if(a){let r=t;for(let t=0;t<e.textures.length;t++){let i=N.get(e.textures[t]);A.framebufferTextureLayer(A.FRAMEBUFFER,A.COLOR_ATTACHMENT0+t,i.__webglTexture,n,r)}}else if(e!==null&&n!==0){let t=N.get(e.texture);A.framebufferTexture2D(A.FRAMEBUFFER,A.COLOR_ATTACHMENT0,A.TEXTURE_2D,t.__webglTexture,n)}te=-1},this.readRenderTargetPixels=function(e,t,n,r,i,a,o,s=0){if(!(e&&e.isWebGLRenderTarget)){G(`WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.`);return}let c=N.get(e).__webglFramebuffer;if(e.isWebGLCubeRenderTarget&&o!==void 0&&(c=c[o]),c){M.bindFramebuffer(A.FRAMEBUFFER,c);try{let o=e.textures[s],c=o.format,l=o.type;if(e.textures.length>1&&A.readBuffer(A.COLOR_ATTACHMENT0+s),!Ee.textureFormatReadable(c)){G(`WebGLRenderer.readRenderTargetPixels: renderTarget is not in RGBA or implementation defined format.`);return}if(!Ee.textureTypeReadable(l)){G(`WebGLRenderer.readRenderTargetPixels: renderTarget is not in UnsignedByteType or implementation defined type.`);return}t>=0&&t<=e.width-r&&n>=0&&n<=e.height-i&&A.readPixels(t,n,r,i,R.convert(c),R.convert(l),a)}finally{let e=k===null?null:N.get(k).__webglFramebuffer;M.bindFramebuffer(A.FRAMEBUFFER,e)}}},this.readRenderTargetPixelsAsync=async function(e,t,n,r,i,a,o,s=0){if(!(e&&e.isWebGLRenderTarget))throw Error(`THREE.WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.`);let c=N.get(e).__webglFramebuffer;if(e.isWebGLCubeRenderTarget&&o!==void 0&&(c=c[o]),c)if(t>=0&&t<=e.width-r&&n>=0&&n<=e.height-i){M.bindFramebuffer(A.FRAMEBUFFER,c);let o=e.textures[s],l=o.format,u=o.type;if(e.textures.length>1&&A.readBuffer(A.COLOR_ATTACHMENT0+s),!Ee.textureFormatReadable(l))throw Error(`THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in RGBA or implementation defined format.`);if(!Ee.textureTypeReadable(u))throw Error(`THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in UnsignedByteType or implementation defined type.`);let d=A.createBuffer();A.bindBuffer(A.PIXEL_PACK_BUFFER,d),A.bufferData(A.PIXEL_PACK_BUFFER,a.byteLength,A.STREAM_READ),A.readPixels(t,n,r,i,R.convert(l),R.convert(u),0);let f=k===null?null:N.get(k).__webglFramebuffer;M.bindFramebuffer(A.FRAMEBUFFER,f);let p=A.fenceSync(A.SYNC_GPU_COMMANDS_COMPLETE,0);return A.flush(),await Wt(A,p,4),A.bindBuffer(A.PIXEL_PACK_BUFFER,d),A.getBufferSubData(A.PIXEL_PACK_BUFFER,0,a),A.deleteBuffer(d),A.deleteSync(p),a}else throw Error(`THREE.WebGLRenderer.readRenderTargetPixelsAsync: requested read bounds are out of range.`)},this.copyFramebufferToTexture=function(e,t=null,n=0){let r=2**-n,i=Math.floor(e.image.width*r),a=Math.floor(e.image.height*r),o=t===null?0:t.x,s=t===null?0:t.y;P.setTexture2D(e,0),A.copyTexSubImage2D(A.TEXTURE_2D,n,0,0,o,s,i,a),M.unbindTexture()};let dt=A.createFramebuffer(),ft=A.createFramebuffer();this.copyTextureToTexture=function(e,t,n=null,r=null,i=0,a=0){let o,s,c,l,u,d,f,p,m,h=e.isCompressedTexture?e.mipmaps[a]:e.image;if(n!==null)o=n.max.x-n.min.x,s=n.max.y-n.min.y,c=n.isBox3?n.max.z-n.min.z:1,l=n.min.x,u=n.min.y,d=n.isBox3?n.min.z:0;else{let t=2**-i;o=Math.floor(h.width*t),s=Math.floor(h.height*t),c=e.isDataArrayTexture?h.depth:e.isData3DTexture?Math.floor(h.depth*t):1,l=0,u=0,d=0}r===null?(f=0,p=0,m=0):(f=r.x,p=r.y,m=r.z);let g=R.convert(t.format),_=R.convert(t.type),v;t.isData3DTexture?(P.setTexture3D(t,0),v=A.TEXTURE_3D):t.isDataArrayTexture||t.isCompressedArrayTexture?(P.setTexture2DArray(t,0),v=A.TEXTURE_2D_ARRAY):(P.setTexture2D(t,0),v=A.TEXTURE_2D),M.activeTexture(A.TEXTURE0),M.pixelStorei(A.UNPACK_FLIP_Y_WEBGL,t.flipY),M.pixelStorei(A.UNPACK_PREMULTIPLY_ALPHA_WEBGL,t.premultiplyAlpha),M.pixelStorei(A.UNPACK_ALIGNMENT,t.unpackAlignment);let y=M.getParameter(A.UNPACK_ROW_LENGTH),b=M.getParameter(A.UNPACK_IMAGE_HEIGHT),x=M.getParameter(A.UNPACK_SKIP_PIXELS),S=M.getParameter(A.UNPACK_SKIP_ROWS),C=M.getParameter(A.UNPACK_SKIP_IMAGES);M.pixelStorei(A.UNPACK_ROW_LENGTH,h.width),M.pixelStorei(A.UNPACK_IMAGE_HEIGHT,h.height),M.pixelStorei(A.UNPACK_SKIP_PIXELS,l),M.pixelStorei(A.UNPACK_SKIP_ROWS,u),M.pixelStorei(A.UNPACK_SKIP_IMAGES,d);let w=e.isDataArrayTexture||e.isData3DTexture,T=t.isDataArrayTexture||t.isData3DTexture;if(e.isDepthTexture){let n=N.get(e),r=N.get(t),h=N.get(n.__renderTarget),g=N.get(r.__renderTarget);M.bindFramebuffer(A.READ_FRAMEBUFFER,h.__webglFramebuffer),M.bindFramebuffer(A.DRAW_FRAMEBUFFER,g.__webglFramebuffer);for(let n=0;n<c;n++)w&&(A.framebufferTextureLayer(A.READ_FRAMEBUFFER,A.COLOR_ATTACHMENT0,N.get(e).__webglTexture,i,d+n),A.framebufferTextureLayer(A.DRAW_FRAMEBUFFER,A.COLOR_ATTACHMENT0,N.get(t).__webglTexture,a,m+n)),A.blitFramebuffer(l,u,o,s,f,p,o,s,A.DEPTH_BUFFER_BIT,A.NEAREST);M.bindFramebuffer(A.READ_FRAMEBUFFER,null),M.bindFramebuffer(A.DRAW_FRAMEBUFFER,null)}else if(i!==0||e.isRenderTargetTexture||N.has(e)){let n=N.get(e),r=N.get(t);M.bindFramebuffer(A.READ_FRAMEBUFFER,dt),M.bindFramebuffer(A.DRAW_FRAMEBUFFER,ft);for(let e=0;e<c;e++)w?A.framebufferTextureLayer(A.READ_FRAMEBUFFER,A.COLOR_ATTACHMENT0,n.__webglTexture,i,d+e):A.framebufferTexture2D(A.READ_FRAMEBUFFER,A.COLOR_ATTACHMENT0,A.TEXTURE_2D,n.__webglTexture,i),T?A.framebufferTextureLayer(A.DRAW_FRAMEBUFFER,A.COLOR_ATTACHMENT0,r.__webglTexture,a,m+e):A.framebufferTexture2D(A.DRAW_FRAMEBUFFER,A.COLOR_ATTACHMENT0,A.TEXTURE_2D,r.__webglTexture,a),i===0?T?A.copyTexSubImage3D(v,a,f,p,m+e,l,u,o,s):A.copyTexSubImage2D(v,a,f,p,l,u,o,s):A.blitFramebuffer(l,u,o,s,f,p,o,s,A.COLOR_BUFFER_BIT,A.NEAREST);M.bindFramebuffer(A.READ_FRAMEBUFFER,null),M.bindFramebuffer(A.DRAW_FRAMEBUFFER,null)}else T?e.isDataTexture||e.isData3DTexture?A.texSubImage3D(v,a,f,p,m,o,s,c,g,_,h.data):t.isCompressedArrayTexture?A.compressedTexSubImage3D(v,a,f,p,m,o,s,c,g,h.data):A.texSubImage3D(v,a,f,p,m,o,s,c,g,_,h):e.isDataTexture?A.texSubImage2D(A.TEXTURE_2D,a,f,p,o,s,g,_,h.data):e.isCompressedTexture?A.compressedTexSubImage2D(A.TEXTURE_2D,a,f,p,h.width,h.height,g,h.data):A.texSubImage2D(A.TEXTURE_2D,a,f,p,o,s,g,_,h);M.pixelStorei(A.UNPACK_ROW_LENGTH,y),M.pixelStorei(A.UNPACK_IMAGE_HEIGHT,b),M.pixelStorei(A.UNPACK_SKIP_PIXELS,x),M.pixelStorei(A.UNPACK_SKIP_ROWS,S),M.pixelStorei(A.UNPACK_SKIP_IMAGES,C),a===0&&t.generateMipmaps&&A.generateMipmap(v),M.unbindTexture()},this.initRenderTarget=function(e){N.get(e).__webglFramebuffer===void 0&&P.setupRenderTarget(e)},this.initTexture=function(e){e.isCubeTexture?P.setTextureCube(e,0):e.isData3DTexture?P.setTexture3D(e,0):e.isDataArrayTexture||e.isCompressedArrayTexture?P.setTexture2DArray(e,0):P.setTexture2D(e,0),M.unbindTexture()},this.resetState=function(){O=0,ee=0,k=null,M.reset(),ze.reset()},typeof __THREE_DEVTOOLS__<`u`&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent(`observe`,{detail:this}))}get coordinateSystem(){return ii}get outputColorSpace(){return this._outputColorSpace}set outputColorSpace(e){this._outputColorSpace=e;let t=this.getContext();t.drawingBufferColorSpace=J._getDrawingBufferColorSpace(e),t.unpackColorSpace=J._getUnpackColorSpace()}}}));function lf(e){let t=new Map,n=new Map,r=e.clone();return uf(e,r,function(e,r){t.set(r,e),n.set(e,r)}),r.traverse(function(e){if(!e.isSkinnedMesh)return;let r=e,i=t.get(e),a=i.skeleton.bones;r.skeleton=i.skeleton.clone(),r.bindMatrix.copy(i.bindMatrix),r.skeleton.bones=a.map(function(e){return n.get(e)}),r.bind(r.skeleton,r.bindMatrix)}),r}function uf(e,t,n){n(e,t);for(let r=0;r<e.children.length;r++)uf(e.children[r],t.children[r],n)}var df=e((()=>{}));function ff(e){let t=lf(e),n=new Map,r=new Map;return t.traverse(e=>{let t=e;if(t.geometry instanceof io){let e=t.geometry,r=n.get(e);r===void 0&&(r=e.clone(),n.set(e,r)),t.geometry=r}Array.isArray(t.material)?t.material=t.material.map(e=>pf(e,r)):t.material instanceof oo&&(t.material=pf(t.material,r))}),t}function pf(e,t){let n=t.get(e);return n===void 0&&(n=e.clone(),t.set(e,n)),n}function mf(e){let t=new Set,n=new Set;e.traverse(e=>{let r=e;r.geometry instanceof io&&t.add(r.geometry),Array.isArray(r.material)?r.material.forEach(e=>n.add(e)):r.material instanceof oo&&n.add(r.material)}),t.forEach(e=>e.dispose()),n.forEach(e=>e.dispose())}function hf(e,t){for(let n of e.clips)gf(t,n.id,n.name)}function gf(e,t,n){let r=e.clips.find(e=>e.name===t||n!==null&&e.name===n);if(!r)throw new jf(`animated mesh ${e.asset} does not contain clip ${t}`);return r}function _f(e,t){switch(t.kind){case`play`:yf(e,t);return;case`stop`:bf(e,t.fadeSeconds),e.currentClip=null,e.commandSelected=!0,e.status=`stopped`,e.loop=null,e.speed=null,e.weight=null;return;case`pause`:xf(e,`pause`).paused=!0,e.commandSelected=!0,e.status=`paused`;return;case`resume`:{let t=xf(e,`resume`);t.paused=!1,t.play(),e.commandSelected=!0,e.status=`playing`;return}}}function vf(e,t){if(t.length===0||t.length>4)throw new jf(`setAnimationControllerWeights: expected one to four clips`);let n=new Map,r=0;for(let i of t){if(n.has(i.clip)||!Number.isFinite(i.weight)||i.weight<0||i.weight>1||!Number.isFinite(i.speed)||i.speed<=0)throw new jf(`setAnimationControllerWeights: invalid clip sample`);if(!e.actions.has(i.clip))throw new jf(`setAnimationControllerWeights: missing clip ${i.clip} on ${e.asset}`);n.set(i.clip,i),r+=i.weight}if(Math.abs(r-1)>.001)throw new jf(`setAnimationControllerWeights: weights must sum to 1, received ${r}`);for(let[t,r]of e.actions){let e=n.get(t);if(e===void 0){r.stop();continue}r.enabled=!0,r.paused=!1,r.setLoop(zr,1/0),r.setEffectiveTimeScale(e.speed),r.setEffectiveWeight(e.weight),r.play()}e.currentClip=t.reduce((e,t)=>e===null||t.weight>e.weight?t:e,null)?.clip??null,e.commandSelected=!1,e.status=`playing`,e.loop=`repeat`,e.speed=null,e.weight=null,e.controllerClips=t.map(e=>({...e}))}function yf(e,t){let n=e.actions.get(t.clip);if(!n)throw new jf(`setAnimatedMeshPlayback: missing clip ${t.clip} on ${e.asset}`);let r=e.currentClip===null?null:e.actions.get(e.currentClip)??null;t.restart&&n.reset(),n.enabled=!0,n.paused=!1,n.clampWhenFinished=t.loop===`once`,n.setLoop(Sf(t.loop),t.loop===`once`?1:1/0),n.setEffectiveTimeScale(t.speed),n.setEffectiveWeight(t.weight),r&&r!==n&&(t.fadeSeconds!==null&&t.fadeSeconds>0?n.crossFadeFrom(r,t.fadeSeconds,!1):r.stop()),n.play(),e.currentClip=t.clip,e.controllerClips=[],e.commandSelected=!0,e.status=`playing`,e.loop=t.loop,e.speed=t.speed,e.weight=t.weight}function bf(e,t){let n=e.currentClip===null?null:e.actions.get(e.currentClip)??null;n&&(t!==null&&t>0?n.fadeOut(t):n.stop())}function xf(e,t){let n=e.currentClip===null?null:e.actions.get(e.currentClip)??null;if(!n)throw new jf(`setAnimatedMeshPlayback.${t}: no current clip on ${e.asset}`);return n}function Sf(e){switch(e){case`once`:return Rr;case`repeat`:return zr;case`pingPong`:return Br}}function Cf(e){let t=[0,0,0],n=[0,0,0,0],r=[0,0,0],i=0;return e.traverse(e=>{i+=1,t[0]+=e.position.x,t[1]+=e.position.y,t[2]+=e.position.z,n[0]+=e.quaternion.x,n[1]+=e.quaternion.y,n[2]+=e.quaternion.z,n[3]+=e.quaternion.w,r[0]+=e.scale.x,r[1]+=e.scale.y,r[2]+=e.scale.z}),{rootTranslation:[e.position.x,e.position.y,e.position.z],rootRotation:[e.quaternion.x,e.quaternion.y,e.quaternion.z,e.quaternion.w],rootScale:[e.scale.x,e.scale.y,e.scale.z],hierarchyNodeCount:i,hierarchyTranslationSum:t,hierarchyRotationSum:n,hierarchyScaleSum:r}}function wf(e,t){return e.commandSelected?e.status===`stopped`?[`animation_stopped`]:t?.paused||e.status===`paused`?[`animation_paused`]:[]:[`animation_not_started`]}function Tf(e,t,n,r,i,a,o){let s=[],c=(e,t,n)=>{s.length<Pf&&s.push({code:e,message:t,node:Of(n)})},l=0,u=0,d=!1,f=new Ea,p=new K,m=new Y;e.object.updateMatrixWorld(!0),e.object.traverse(e=>{Df(e)||c(`node_transform_non_finite`,`node transform contains a non-finite value`,e);let t=e.quaternion.lengthSq();if((!Number.isFinite(t)||t<1e-12)&&c(`node_quaternion_invalid`,`node quaternion is non-finite or has zero length`,e),(!Number.isFinite(e.scale.x)||!Number.isFinite(e.scale.y)||!Number.isFinite(e.scale.z)||Math.abs(e.scale.x)<1e-12||Math.abs(e.scale.y)<1e-12||Math.abs(e.scale.z)<1e-12)&&c(`node_scale_invalid`,`node scale is non-finite or singular`,e),e instanceof zo&&(l+=1),!(e instanceof Oo))return;let n=e.geometry.getAttribute(`position`);if(n!==void 0){if(u+n.count>Nf){d=!0;return}if(e instanceof Ro){e.skeleton.update();for(let t=0;t<e.skeleton.bones.length;t+=1){let n=e.skeleton.bones[t],r=e.skeleton.boneInverses[t];n===void 0||r===void 0||(m.multiplyMatrices(n.matrixWorld,r),m.elements.every(Number.isFinite)?Math.abs(m.determinant())<1e-12&&c(`bone_matrix_singular`,`bone skin matrix is singular`,n):c(`bone_matrix_non_finite`,`bone skin matrix contains a non-finite value`,n))}}for(let t=0;t<n.count;t+=1)p.fromBufferAttribute(n,t),e instanceof Ro&&e.applyBoneTransform(t,p),e.localToWorld(p),f.expandByPoint(p);u+=n.count}}),d&&c(`vertex_budget_exceeded`,`sample contains more than ${Nf} vertices`,null);let h=!f.isEmpty()&&!d?kf(f):null;return h!==null&&Af(a,h,e.object)&&c(`sampled_bounds_implausible`,`sampled world bounds expand beyond eight times the admitted asset extent`,null),{handle:e.handle,asset:e.asset,contentHash:o,clip:n,normalizedTime:r,durationSeconds:i,assetBounds:{min:[...a.min],max:[...a.max]},sampledWorldBounds:h,sampledVertexCount:u,boneCount:l,skinningFacts:t,diagnostics:s}}function Ef(e,t,n){let r=new Map,i=new Map,a=new Set,o=new Map;if(t.updateMatrixWorld(!0),t.traverse(e=>{e instanceof zo&&r.set(e.name,e),e instanceof Oo&&o.set(e.name,e),e instanceof Ro&&(a.add(e.skeleton),e.skeleton.bones.forEach((t,n)=>{let r=e.skeleton.boneInverses[n];r!==void 0&&i.set(t.name,r)}))}),r.size>Ff)throw new jf(`sampleAnimatedMesh: joint count exceeds ${Ff}`);let s=0,c=0,l=!0,u=0,d=0,f=0,p=!0,m=0,h=0;e.updateMatrixWorld(!0),e.traverse(e=>{if(!(e instanceof Oo))return;let t=o.get(e.name);if(t?.geometry===e.geometry&&(m+=1),t?.material===e.material&&(h+=1),!(e instanceof Ro))return;s+=1,a.has(e.skeleton)&&(p=!1),e.skeleton.bones.forEach((t,n)=>{r.get(t.name)===t&&(p=!1);let i=e.skeleton.boneInverses[n];i!==void 0&&(c+=1,i.elements.every(Number.isFinite)||(l=!1))});let n=e.geometry.getAttribute(`skinWeight`);if(n!==void 0)for(let e=0;e<n.count;e+=1){let t=n.getX(e)+(n.itemSize>1?n.getY(e):0)+(n.itemSize>2?n.getZ(e):0)+(n.itemSize>3?n.getW(e):0);if(u+=1,!Number.isFinite(t)||t<=0){d+=1;continue}f=Math.max(f,Math.abs(t-1))}});let g=[...new Set(n.tracks.map(e=>{switch(e.getInterpolation()){case Vr:return`discrete`;case Ur:return`smooth`;default:return`linear`}}))].sort();return{joints:[...r.values()].map(e=>({name:e.name,parent:e.parent instanceof zo?e.parent.name:null,restLocalMatrix:[...e.matrix.elements],inverseBindMatrix:i.has(e.name)?[...i.get(e.name).elements]:null})),skinnedMeshCount:s,inverseBindMatrixCount:c,inverseBindMatricesFinite:l,weightedVertexCount:u,invalidWeightVertexCount:d,maximumWeightSumError:f,weightsNormalized:u>0&&d===0&&f<=If,interpolationModes:g,instanceRootDistinctFromTemplate:e!==t,skeletonsIndependentFromTemplate:p,sharedGeometryCount:m,sharedMaterialCount:h}}function Df(e){return[...e.position.toArray(),...e.quaternion.toArray(),...e.scale.toArray(),...e.matrix.elements,...e.matrixWorld.elements].every(Number.isFinite)}function Of(e){return e===null?null:e.name.length>0?e.name:`${e.type}:${e.id}`}function kf(e){return{min:e.min.toArray(),max:e.max.toArray()}}function Af(e,t,n){let r=Math.max(e.max[0]-e.min[0],e.max[1]-e.min[1],e.max[2]-e.min[2],1e-6),i=Math.max(t.max[0]-t.min[0],t.max[1]-t.min[1],t.max[2]-t.min[2]),a=n.getWorldScale(new K);return i>r*Math.max(Math.abs(a.x),Math.abs(a.y),Math.abs(a.z),1e-6)*8}var jf,Mf,Nf,Pf,Ff,If,Lf=e((()=>{cf(),df(),jf=class extends Error{constructor(e){super(e),this.name=`AnimatedMeshApplyError`}},Mf=class{#e;#t=new Map;#n=new Map;constructor(e){this.#e=e}get instanceCount(){return this.#n.size}define(e){let t=this.#t.get(e.asset);if(t&&t.refCount>0)throw new jf(`defineAnimatedMesh: asset ${e.asset} is in use by ${t.refCount} instance(s)`);let n=this.#r(e),r=ff(n.scene);t&&mf(t.scene),this.#t.set(e.asset,{asset:e,resource:n,scene:r,refCount:0})}validateDefinition(e){this.#r(e)}#r(e){if(e.runtimeFormat!==`glb`)throw new jf(`defineAnimatedMesh: unsupported runtime format ${e.runtimeFormat}`);let t=this.#e?.getAnimatedMeshResource(e);if(!t)throw new jf(`defineAnimatedMesh: missing animated mesh resource ${e.asset}`);if(t.contentHash!==void 0&&t.contentHash!==e.contentHash)throw new jf(`defineAnimatedMesh: content hash mismatch for ${e.asset}; expected ${t.contentHash}, received ${e.contentHash}`);return hf(e,t),t}create(e,t){let n=this.#t.get(t.asset);if(!n)throw new jf(`createAnimatedMeshInstance: undefined animated mesh asset ${t.asset}`);if(t.materialOverrides.length>0)throw new jf(`createAnimatedMeshInstance: material overrides are not implemented for animated mesh ${t.asset}`);let r=lf(n.scene),i=new Vc(r),a=new Map;for(let e of n.asset.clips)a.set(e.id,i.clipAction(gf(n.resource,e.id,e.name)));let o={handle:e,asset:t.asset,object:r,mixer:i,actions:a,currentClip:null,commandSelected:!1,status:`not_started`,loop:null,speed:null,weight:null,controllerClips:[]};return this.#n.set(e,o),n.refCount+=1,t.playback&&this.setPlayback(e,t.playback),o}setPlayback(e,t){_f(this.#i(e,`setAnimatedMeshPlayback`),t)}setControllerWeights(e,t){vf(this.#i(e,`setAnimationControllerWeights`),t)}hasClips(e,t){let n=this.#n.get(e);return n!==void 0&&t.every(e=>n.actions.has(e))}clearControllerWeights(e){let t=this.#i(e,`clearAnimationControllerWeights`);t.mixer.stopAllAction(),t.currentClip=null,t.controllerClips=[],t.commandSelected=!1,t.status=`stopped`,t.loop=null,t.speed=null,t.weight=null}advance(e){if(!Number.isFinite(e)||e<0)throw new jf(`advanceAnimation: deltaSeconds must be finite and non-negative`);for(let t of this.#n.values())t.mixer.update(e)}playback(e){let t=this.#n.get(e);if(!t)return;let n=t.currentClip===null?null:t.actions.get(t.currentClip)??null;return{handle:e,asset:t.asset,status:t.status,currentClip:t.currentClip,mixerTimeSeconds:t.mixer.time,actionTimeSeconds:n?.time??null,running:n?.isRunning()??!1,paused:n?.paused??!1,loop:t.loop,speed:t.speed,weight:t.weight,commandSelected:t.commandSelected,poseSample:Cf(t.object),diagnostics:wf(t,n),controllerClips:t.controllerClips}}sample(e,t,n){if(!Number.isFinite(n)||n<0||n>1)throw new jf(`sampleAnimatedMesh: normalizedTime must be finite and between 0 and 1`);let r=this.#i(e,`sampleAnimatedMesh`),i=r.actions.get(t);if(i===void 0)throw new jf(`sampleAnimatedMesh: missing clip ${t} on ${r.asset}`);let a=i.getClip().duration;if(!Number.isFinite(a)||a<=0)throw new jf(`sampleAnimatedMesh: clip ${t} has an invalid duration`);let o=this.#t.get(r.asset);if(o===void 0)throw new jf(`sampleAnimatedMesh: missing defined asset ${r.asset}`);let s=Ef(r.object,o.scene,i.getClip());return r.mixer.stopAllAction(),i.reset(),i.enabled=!0,i.paused=!1,i.clampWhenFinished=!0,i.setLoop(Rr,1),i.setEffectiveTimeScale(1),i.setEffectiveWeight(1),i.play(),r.mixer.setTime(a*n),i.paused=!0,r.currentClip=t,r.commandSelected=!0,r.status=`paused`,r.loop=`once`,r.speed=1,r.weight=1,r.controllerClips=[],Tf(r,s,t,n,a,o.asset.bounds,o.asset.contentHash)}release(e){let t=this.#n.get(e);if(!t)return;t.mixer.stopAllAction(),t.mixer.uncacheRoot(t.object),this.#n.delete(e);let n=this.#t.get(t.asset);n&&--n.refCount}dispose(){for(let e of[...this.#n.keys()])this.release(e);for(let e of this.#t.values())mf(e.scene);this.#t.clear()}#i(e,t){let n=this.#n.get(e);if(!n)throw new jf(`${t}: handle ${e} is not an animated mesh`);return n}},Nf=1e6,Pf=64,Ff=256,If=1e-4}));function Rf(e,t){let n=new X(...e.color),r;switch(e.kind){case`ambient`:r=new xc(n,e.intensity);break;case`directional`:{let t=new bc(n,e.intensity);t.add(t.target),t.target.position.set(...Gf(e.direction)),r=t;break}case`point`:r=new _c(n,e.intensity,e.range??0,e.decay),r.position.set(...e.position);break;case`spot`:{let t=new hc(n,e.intensity,e.range??0,e.outerAngleRadians,e.penumbra,e.decay);t.position.set(...e.position),t.add(t.target),t.target.position.set(...Gf(e.direction)),r=t;break}}return r.visible=e.enabled,Wf(r,e,t),r}function zf(e,t,n){let r=e;if(r.color.setRGB(t.color[0],t.color[1],t.color[2]),r.intensity=t.intensity,r.visible=t.enabled,t.kind===`directional`)r.target.position.set(...Gf(t.direction));else if(t.kind===`point`){let e=r;e.position.set(...t.position),e.distance=t.range??0,e.decay=t.decay}else if(t.kind===`spot`){let e=r;e.position.set(...t.position),e.target.position.set(...Gf(t.direction)),e.distance=t.range??0,e.decay=t.decay,e.angle=t.outerAngleRadians,e.penumbra=t.penumbra}Wf(r,t,n)}function Bf(e,t){return!e.enabled||e.shadowIntent===`disabled`?`disabled`:t&&e.kind!==`ambient`?`active`:`requested_unsupported`}function Vf(e,t){if(e===null)return null;for(let[n,r]of t)if(r.object===e)return n;return null}function Hf(e){e.clear(),e.removeFromParent()}function Uf(e,t,n){if(e.color.forEach((e,r)=>{if(!Number.isFinite(e)||e<0||e>1)throw n(`${t}.color[${r}] must be finite and in 0..=1`)}),!Number.isFinite(e.intensity)||e.intensity<0||e.intensity>1e4)throw n(`${t}.intensity must be finite and in 0..=${String(r)}`);if((e.kind===`directional`||e.kind===`spot`)&&(e.direction.forEach((e,r)=>{if(!Number.isFinite(e))throw n(`${t}.direction[${r}] must be finite`)}),e.direction.reduce((e,t)=>e+t*t,0)<=2**-52))throw n(`${t}.direction must be non-zero`);if(e.kind===`point`||e.kind===`spot`){if(e.position.forEach((e,r)=>{if(!Number.isFinite(e))throw n(`${t}.position[${r}] must be finite`)}),e.range!==null&&(!Number.isFinite(e.range)||e.range<=0))throw n(`${t}.range must be null or finite and positive`);if(!Number.isFinite(e.decay)||e.decay<0)throw n(`${t}.decay must be finite and non-negative`)}if(e.kind===`spot`){if(!Number.isFinite(e.outerAngleRadians)||e.outerAngleRadians<=0||e.outerAngleRadians>Math.PI/2)throw n(`${t}.outerAngleRadians must be in (0, pi/2]`);if(!Number.isFinite(e.penumbra)||e.penumbra<0||e.penumbra>1)throw n(`${t}.penumbra must be in 0..=1`)}}function Wf(e,t,n){t.kind!==`ambient`&&`castShadow`in e&&(e.castShadow=n&&t.enabled&&t.shadowIntent===`requested`)}function Gf(e){let t=new K(...e).normalize();return[t.x,t.y,t.z]}var Kf,qf=e((()=>{cf(),at(),Kf=class extends Error{code;constructor(e,t){super(t),this.code=e,this.name=`RendererLightingPolicyError`}}}));function Jf(e){let t=e.material;return Array.isArray(t)?t:[t]}var Yf,Xf=e((()=>{Yf={color:[1,1,1,1],wireframe:!1}}));function Zf(e){return e instanceof Uint8Array||ArrayBuffer.isView(e)&&e.constructor.name===`Uint8Array`&&`BYTES_PER_ELEMENT`in e&&e.BYTES_PER_ELEMENT===1}function Qf(e,t,n=``){let r=Zf(e),i=e?.length,a=t!==void 0;if(!r||a&&i!==t){let o=n&&`"${n}" `,s=a?` of length ${t}`:``,c=r?`length=${i}`:`type=${typeof e}`,l=o+`expected Uint8Array`+s+`, got `+c;throw r?RangeError(l):TypeError(l)}return e}function $f(e,t=!0){if(e.destroyed)throw Error(`Hash instance has been destroyed`);if(t&&e.finished)throw Error(`Hash#digest() has already been called`)}function ep(e,t){Qf(e,void 0,`digestInto() output`);let n=t.outputLen;if(e.length<n)throw RangeError(`"digestInto() output" expected to be of length >=`+n)}function tp(...e){for(let t=0;t<e.length;t++)e[t].fill(0)}function np(e){return new DataView(e.buffer,e.byteOffset,e.byteLength)}function rp(e,t){return e<<32-t|e>>>t}function ip(e){if(Qf(e),op)return e.toHex();let t=``;for(let n=0;n<e.length;n++)t+=sp[e[n]];return t}function ap(e,t={}){let n=(t,n)=>e(n).update(t).digest(),r=e(void 0);return n.outputLen=r.outputLen,n.blockLen=r.blockLen,n.canXOF=r.canXOF,n.create=t=>e(t),Object.assign(n,t),Object.freeze(n)}var op,sp,cp,lp=e((()=>{op=typeof Uint8Array.from([]).toHex==`function`&&typeof Uint8Array.fromHex==`function`,sp=Array.from({length:256},(e,t)=>t.toString(16).padStart(2,`0`)),cp=e=>({oid:Uint8Array.from([6,9,96,134,72,1,101,3,4,2,e])})}));function up(e,t,n){return e&t^~e&n}function dp(e,t,n){return e&t^e&n^t&n}var fp,pp,mp=e((()=>{lp(),fp=class{blockLen;outputLen;canXOF=!1;padOffset;isLE;buffer;view;finished=!1;length=0;pos=0;destroyed=!1;constructor(e,t,n,r){this.blockLen=e,this.outputLen=t,this.padOffset=n,this.isLE=r,this.buffer=new Uint8Array(e),this.view=np(this.buffer)}update(e){$f(this),Qf(e);let{view:t,buffer:n,blockLen:r}=this,i=e.length;for(let a=0;a<i;){let o=Math.min(r-this.pos,i-a);if(o===r){let t=np(e);for(;r<=i-a;a+=r)this.process(t,a);continue}n.set(e.subarray(a,a+o),this.pos),this.pos+=o,a+=o,this.pos===r&&(this.process(t,0),this.pos=0)}return this.length+=e.length,this.roundClean(),this}digestInto(e){$f(this),ep(e,this),this.finished=!0;let{buffer:t,view:n,blockLen:r,isLE:i}=this,{pos:a}=this;t[a++]=128,tp(this.buffer.subarray(a)),this.padOffset>r-a&&(this.process(n,0),a=0);for(let e=a;e<r;e++)t[e]=0;n.setBigUint64(r-8,BigInt(this.length*8),i),this.process(n,0);let o=np(e),s=this.outputLen;if(s%4)throw Error(`_sha2: outputLen must be aligned to 32bit`);let c=s/4,l=this.get();if(c>l.length)throw Error(`_sha2: outputLen bigger than state`);for(let e=0;e<c;e++)o.setUint32(4*e,l[e],i)}digest(){let{buffer:e,outputLen:t}=this;this.digestInto(e);let n=e.slice(0,t);return this.destroy(),n}_cloneInto(e){e||=new this.constructor,e.set(...this.get());let{blockLen:t,buffer:n,length:r,finished:i,destroyed:a,pos:o}=this;return e.destroyed=a,e.finished=i,e.length=r,e.pos=o,r%t&&e.buffer.set(n),e}clone(){return this._cloneInto()}},pp=Uint32Array.from([1779033703,3144134277,1013904242,2773480762,1359893119,2600822924,528734635,1541459225])})),hp,gp,_p,vp,yp,bp=e((()=>{mp(),lp(),hp=Uint32Array.from([1116352408,1899447441,3049323471,3921009573,961987163,1508970993,2453635748,2870763221,3624381080,310598401,607225278,1426881987,1925078388,2162078206,2614888103,3248222580,3835390401,4022224774,264347078,604807628,770255983,1249150122,1555081692,1996064986,2554220882,2821834349,2952996808,3210313671,3336571891,3584528711,113926993,338241895,666307205,773529912,1294757372,1396182291,1695183700,1986661051,2177026350,2456956037,2730485921,2820302411,3259730800,3345764771,3516065817,3600352804,4094571909,275423344,430227734,506948616,659060556,883997877,958139571,1322822218,1537002063,1747873779,1955562222,2024104815,2227730452,2361852424,2428436474,2756734187,3204031479,3329325298]),gp=new Uint32Array(64),_p=class extends fp{constructor(e){super(64,e,8,!1)}get(){let{A:e,B:t,C:n,D:r,E:i,F:a,G:o,H:s}=this;return[e,t,n,r,i,a,o,s]}set(e,t,n,r,i,a,o,s){this.A=e|0,this.B=t|0,this.C=n|0,this.D=r|0,this.E=i|0,this.F=a|0,this.G=o|0,this.H=s|0}process(e,t){for(let n=0;n<16;n++,t+=4)gp[n]=e.getUint32(t,!1);for(let e=16;e<64;e++){let t=gp[e-15],n=gp[e-2],r=rp(t,7)^rp(t,18)^t>>>3,i=rp(n,17)^rp(n,19)^n>>>10;gp[e]=i+gp[e-7]+r+gp[e-16]|0}let{A:n,B:r,C:i,D:a,E:o,F:s,G:c,H:l}=this;for(let e=0;e<64;e++){let t=rp(o,6)^rp(o,11)^rp(o,25),u=l+t+up(o,s,c)+hp[e]+gp[e]|0,d=(rp(n,2)^rp(n,13)^rp(n,22))+dp(n,r,i)|0;l=c,c=s,s=o,o=a+u|0,a=i,i=r,r=n,n=u+d|0}n=n+this.A|0,r=r+this.B|0,i=i+this.C|0,a=a+this.D|0,o=o+this.E|0,s=s+this.F|0,c=c+this.G|0,l=l+this.H|0,this.set(n,r,i,a,o,s,c,l)}roundClean(){tp(gp)}destroy(){this.destroyed=!0,this.set(0,0,0,0,0,0,0,0),tp(this.buffer)}},vp=class extends _p{A=pp[0]|0;B=pp[1]|0;C=pp[2]|0;D=pp[3]|0;E=pp[4]|0;F=pp[5]|0;G=pp[6]|0;H=pp[7]|0;constructor(){super(32)}},yp=ap(()=>new vp,cp(1))}));function xp(e,t){return Yp(e.subarray(Zp(e,t&&t.dictionary),-4),{i:2},t&&t.out,t&&t.dictionary)}var Sp,Cp,wp,Tp,Ep,Dp,Op,kp,Ap,jp,Mp,Np,Pp,Fp,Ip,Lp,Rp,zp,Bp,Vp,Hp,Up,Wp,Gp,Kp,qp,Jp,Yp,Xp,Zp,Qp,$p=e((()=>{for(Sp=Uint8Array,Cp=Uint16Array,wp=Int32Array,Tp=new Sp([0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0,0,0,0]),Ep=new Sp([0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13,0,0]),Dp=new Sp([16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15]),Op=function(e,t){for(var n=new Cp(31),r=0;r<31;++r)n[r]=t+=1<<e[r-1];for(var i=new wp(n[30]),r=1;r<30;++r)for(var a=n[r];a<n[r+1];++a)i[a]=a-n[r]<<5|r;return{b:n,r:i}},kp=Op(Tp,2),Ap=kp.b,jp=kp.r,Ap[28]=258,jp[258]=28,Mp=Op(Ep,0),Np=Mp.b,Mp.r,Pp=new Cp(32768),Fp=0;Fp<32768;++Fp)Ip=(Fp&43690)>>1|(Fp&21845)<<1,Ip=(Ip&52428)>>2|(Ip&13107)<<2,Ip=(Ip&61680)>>4|(Ip&3855)<<4,Pp[Fp]=((Ip&65280)>>8|(Ip&255)<<8)>>1;for(Lp=(function(e,t,n){for(var r=e.length,i=0,a=new Cp(t);i<r;++i)e[i]&&++a[e[i]-1];var o=new Cp(t);for(i=1;i<t;++i)o[i]=o[i-1]+a[i-1]<<1;var s;if(n){s=new Cp(1<<t);var c=15-t;for(i=0;i<r;++i)if(e[i])for(var l=i<<4|e[i],u=t-e[i],d=o[e[i]-1]++<<u,f=d|(1<<u)-1;d<=f;++d)s[Pp[d]>>c]=l}else for(s=new Cp(r),i=0;i<r;++i)e[i]&&(s[i]=Pp[o[e[i]-1]++]>>15-e[i]);return s}),Rp=new Sp(288),Fp=0;Fp<144;++Fp)Rp[Fp]=8;for(Fp=144;Fp<256;++Fp)Rp[Fp]=9;for(Fp=256;Fp<280;++Fp)Rp[Fp]=7;for(Fp=280;Fp<288;++Fp)Rp[Fp]=8;for(zp=new Sp(32),Fp=0;Fp<32;++Fp)zp[Fp]=5;Bp=Lp(Rp,9,1),Vp=Lp(zp,5,1),Hp=function(e){for(var t=e[0],n=1;n<e.length;++n)e[n]>t&&(t=e[n]);return t},Up=function(e,t,n){var r=t/8|0;return(e[r]|e[r+1]<<8)>>(t&7)&n},Wp=function(e,t){var n=t/8|0;return(e[n]|e[n+1]<<8|e[n+2]<<16)>>(t&7)},Gp=function(e){return(e+7)/8|0},Kp=function(e,t,n){return(t==null||t<0)&&(t=0),(n==null||n>e.length)&&(n=e.length),new Sp(e.subarray(t,n))},qp=[`unexpected EOF`,`invalid block type`,`invalid length/literal`,`invalid distance`,`stream finished`,`no stream handler`,,`no callback`,`invalid UTF-8 data`,`extra field too long`,`date not in range 1980-2099`,`filename too long`,`stream finishing`,`invalid zip data`],Jp=function(e,t,n){var r=Error(t||qp[e]);if(r.code=e,Error.captureStackTrace&&Error.captureStackTrace(r,Jp),!n)throw r;return r},Yp=function(e,t,n,r){var i=e.length,a=r?r.length:0;if(!i||t.f&&!t.l)return n||new Sp(0);var o=!n,s=o||t.i!=2,c=t.i;o&&(n=new Sp(i*3));var l=function(e){var t=n.length;if(e>t){var r=new Sp(Math.max(t*2,e));r.set(n),n=r}},u=t.f||0,d=t.p||0,f=t.b||0,p=t.l,m=t.d,h=t.m,g=t.n,_=i*8;do{if(!p){u=Up(e,d,1);var v=Up(e,d+1,3);if(d+=3,!v){var y=Gp(d)+4,b=e[y-4]|e[y-3]<<8,x=y+b;if(x>i){c&&Jp(0);break}s&&l(f+b),n.set(e.subarray(y,x),f),t.b=f+=b,t.p=d=x*8,t.f=u;continue}else if(v==1)p=Bp,m=Vp,h=9,g=5;else if(v==2){var S=Up(e,d,31)+257,C=Up(e,d+10,15)+4,w=S+Up(e,d+5,31)+1;d+=14;for(var T=new Sp(w),E=new Sp(19),D=0;D<C;++D)E[Dp[D]]=Up(e,d+D*3,7);d+=C*3;for(var O=Hp(E),ee=(1<<O)-1,k=Lp(E,O,1),D=0;D<w;){var te=k[Up(e,d,ee)];d+=te&15;var y=te>>4;if(y<16)T[D++]=y;else{var ne=0,re=0;for(y==16?(re=3+Up(e,d,3),d+=2,ne=T[D-1]):y==17?(re=3+Up(e,d,7),d+=3):y==18&&(re=11+Up(e,d,127),d+=7);re--;)T[D++]=ne}}var ie=T.subarray(0,S),ae=T.subarray(S);h=Hp(ie),g=Hp(ae),p=Lp(ie,h,1),m=Lp(ae,g,1)}else Jp(1);if(d>_){c&&Jp(0);break}}s&&l(f+131072);for(var oe=(1<<h)-1,se=(1<<g)-1,ce=d;;ce=d){var ne=p[Wp(e,d)&oe],le=ne>>4;if(d+=ne&15,d>_){c&&Jp(0);break}if(ne||Jp(2),le<256)n[f++]=le;else if(le==256){ce=d,p=null;break}else{var ue=le-254;if(le>264){var D=le-257,de=Tp[D];ue=Up(e,d,(1<<de)-1)+Ap[D],d+=de}var fe=m[Wp(e,d)&se],pe=fe>>4;fe||Jp(3),d+=fe&15;var ae=Np[pe];if(pe>3){var de=Ep[pe];ae+=Wp(e,d)&(1<<de)-1,d+=de}if(d>_){c&&Jp(0);break}s&&l(f+131072);var me=f+ue;if(f<ae){var he=a-ae,ge=Math.min(ae,me);for(he+f<0&&Jp(3);f<ge;++f)n[f]=r[he+f]}for(;f<me;++f)n[f]=n[f-ae]}}t.l=p,t.p=ce,t.b=f,t.f=u,p&&(u=1,t.m=h,t.d=m,t.n=g)}while(!u);return f!=n.length&&o?Kp(n,0,f):n.subarray(0,f)},Xp=new Sp(0),Zp=function(e,t){return((e[0]&15)!=8||e[0]>>4>7||(e[0]<<8|e[1])%31)&&Jp(6,`invalid zlib data`),(e[1]>>5&1)==+!t&&Jp(6,`invalid zlib data: `+(e[1]&32?`need`:`unexpected`)+` dictionary`),(e[1]>>3&4)+2},Qp=typeof TextDecoder<`u`&&new TextDecoder;try{Qp.decode(Xp,{stream:!0})}catch{}}));function em(e,t){let n=e.payload;if(n===void 0)throw new im(`texture has no retained payload`);if(t.byteLength!==n.byteLength)throw new im(`encoded byte length ${String(t.byteLength)} does not match ${String(n.byteLength)}`);let r=`sha256:${ip(yp(t))}`;if(r!==n.contentHash||e.contentHash!==r)throw new im(`content hash mismatch: expected ${n.contentHash}, received ${r}`);return tm(t,e.width,e.height)}function tm(e,t,n){if(e.byteLength<45||[137,80,78,71,13,10,26,10].some((t,n)=>e[n]!==t))throw new im(`invalid PNG signature or truncated stream`);let r=new DataView(e.buffer,e.byteOffset,e.byteLength),i=[],a=8,o=!1,s=!1;for(;a<e.byteLength;){if(a+12>e.byteLength)throw new im(`truncated PNG chunk`);let c=r.getUint32(a,!1),l=a+4,u=l+4,d=u+c,f=d+4;if(!Number.isSafeInteger(f)||f>e.byteLength)throw new im(`PNG chunk exceeds encoded bytes`);let p=String.fromCharCode(...e.subarray(l,u)),m=r.getUint32(d,!1);if(rm(e.subarray(l,d))!==m)throw new im(`PNG ${p} CRC mismatch`);if(p===`IHDR`){if(o||a!==8||c!==13)throw new im(`invalid PNG IHDR`);let i=r.getUint32(u,!1),s=r.getUint32(u+4,!1);if(i!==t||s!==n)throw new im(`PNG dimensions do not match the descriptor`);if(e[u+8]!==8||e[u+9]!==6||e[u+10]!==0||e[u+11]!==0||e[u+12]!==0)throw new im(`only non-interlaced RGBA8 PNG is supported`);o=!0}else if(p===`IDAT`){if(!o||s)throw new im(`PNG IDAT ordering is invalid`);i.push(e.slice(u,d))}else if(p===`IEND`){if(!o||i.length===0||s||c!==0||f!==e.byteLength)throw new im(`invalid PNG IEND`);s=!0}else if(e[l]>=65&&e[l]<=90)throw new im(`unsupported critical PNG chunk ${p}`);a=f}if(!o||!s||i.length===0)throw new im(`incomplete PNG stream`);let c=i.reduce((e,t)=>e+t.byteLength,0),l=new Uint8Array(c),u=0;for(let e of i)l.set(e,u),u+=e.byteLength;let d;try{d=xp(l)}catch(e){throw new im(`PNG deflate stream is invalid: ${e instanceof Error?e.message:String(e)}`)}let f=t*4,p=n*(f+1);if(d.byteLength!==p)throw new im(`decoded PNG length ${String(d.byteLength)} does not match ${String(p)}`);let m=new Uint8Array(t*n*4);for(let e=0;e<n;e++){let t=e*(f+1),n=d[t];if(n>4)throw new im(`unsupported PNG row filter ${String(n)}`);let r=t+1,i=e*f;for(let t=0;t<f;t++){let a=d[r+t],o=t>=4?m[i+t-4]:0,s=e>0?m[i+t-f]:0,c=e>0&&t>=4?m[i+t-f-4]:0,l=n===0?0:n===1?o:n===2?s:n===3?Math.floor((o+s)/2):nm(o,s,c);m[i+t]=a+l&255}}return{pixels:m,width:t,height:n}}function nm(e,t,n){let r=e+t-n,i=Math.abs(r-e),a=Math.abs(r-t),o=Math.abs(r-n);return i<=a&&i<=o?e:a<=o?t:n}function rm(e){let t=4294967295;for(let n of e){t^=n;for(let e=0;e<8;e++)t=t&1?t>>>1^3988292384:t>>>1}return(t^4294967295)>>>0}var im,am=e((()=>{lp(),bp(),$p(),im=class extends Error{constructor(e){super(e),this.name=`PngTextureError`}}}));function om(e,t){let n=e.voxelSurface;if(n===void 0)throw new lm(`material ${e.id} has no voxel surface`);let r=n.mapping;if(e.texture!==r.texture||t.id!==r.texture)throw new lm(`material ${e.id} resolved texture ${r.texture} does not match ${t.id}`);if(t.version!==r.textureVersion)throw new lm(`material ${e.id} needs texture ${t.id} version ${String(r.textureVersion)}`);if(t.contentHash!==r.textureContentHash)throw new lm(`material ${e.id} needs texture ${t.id} hash ${r.textureContentHash}`);if(t.payload===void 0)throw new lm(`material ${e.id} needs retained texture payload ${t.id}`);if(t.filter!==n.filter||t.wrap!==n.wrap)throw new lm(`material ${e.id} texture sampling policy does not match ${t.id}`);let i=[0,0],a=[1,1];if(r.kind===`atlas`){let[n,o]=r.region.contentMin,[s,c]=r.region.contentExtent;if(n+s>t.width||o+c>t.height)throw new lm(`material ${e.id} atlas region ${r.region.id} exceeds ${t.id}`);i=[(n+.5)/t.width,(o+.5)/t.height],a=[(n+s-.5)/t.width,(o+c-.5)/t.height]}return Object.freeze({material:e.id,texture:t.id,mapping:r.kind,tileScaleCells:Object.freeze([...r.tileScaleCells]),tileOriginCells:Object.freeze([...r.tileOriginCells]),sampleUvMin:Object.freeze([...i]),sampleUvMax:Object.freeze([...a]),alphaMode:n.alphaMode.kind,alphaCutoff:n.alphaMode.kind===`mask`?n.alphaMode.cutoff:null})}function sm(e,t,n){let r=om(t,n);return e.userData.rustyVoxelSurface=r,e.customProgramCacheKey=()=>[`rusty-engine.voxel-surface.v1`,r.mapping,t.voxelSurface.filter,r.alphaMode].join(`:`),e.onBeforeCompile=e=>{e.uniforms.rustyVoxelTileScale={value:new fi(...r.tileScaleCells)},e.uniforms.rustyVoxelTileOrigin={value:new fi(...r.tileOriginCells)},e.uniforms.rustyVoxelUvMin={value:new fi(...r.sampleUvMin)},e.uniforms.rustyVoxelUvMax={value:new fi(...r.sampleUvMax)},e.fragmentShader=e.fragmentShader.replace(`#include <map_pars_fragment>`,[`#include <map_pars_fragment>`,`#ifdef USE_MAP`,`uniform vec2 rustyVoxelTileScale;`,`uniform vec2 rustyVoxelTileOrigin;`,`uniform vec2 rustyVoxelUvMin;`,`uniform vec2 rustyVoxelUvMax;`,`#endif`].join(`
`)).replace(`#include <map_fragment>`,[`#ifdef USE_MAP`,`vec2 rustyVoxelRepeat = fract((vMapUv - rustyVoxelTileOrigin) / rustyVoxelTileScale);`,`vec2 rustyVoxelUv = mix(rustyVoxelUvMin, rustyVoxelUvMax, rustyVoxelRepeat);`,`vec4 sampledDiffuseColor = texture2D(map, rustyVoxelUv);`,`#ifdef DECODE_VIDEO_TEXTURE`,`sampledDiffuseColor = sRGBTransferEOTF(sampledDiffuseColor);`,`#endif`,`diffuseColor *= sampledDiffuseColor;`,`#endif`].join(`
`))},cm(e,t.voxelSurface),e.needsUpdate=!0,r}function cm(e,t){switch(t.alphaMode.kind){case`opaque`:e.alphaTest=0,e.transparent=!1,e.depthWrite=!0;break;case`mask`:e.alphaTest=t.alphaMode.cutoff,e.transparent=!1,e.depthWrite=!0;break;case`blend`:e.alphaTest=0,e.transparent=!0,e.depthWrite=!1;break}}var lm,um=e((()=>{cf(),lm=class extends Error{constructor(e){super(e),this.name=`VoxelSurfaceMaterialError`}}}));function dm(e,t,n){let r=e.count-(t===void 0?0:1)+(n===void 0?0:1),i=e.encodedBytes-(t?.encodedBytes??0)+(n?.encodedBytes??0),a=e.decodedBytes-(t?.decodedBytes??0)+(n?.decodedBytes??0);if(![r,i,a].every(Number.isSafeInteger)||r<0||i<0||a<0)throw new $(`defineTexture: texture resource budget arithmetic is invalid`);if(r>256)throw new $(`defineTexture: retained texture quota exceeded`);if(i>134217728)throw new $(`defineTexture: aggregate encoded texture byte quota exceeded`);if(a>268435456)throw new $(`defineTexture: aggregate decoded texture byte quota exceeded`);return{count:r,encodedBytes:i,decodedBytes:a}}function fm(e,t){let n=e.parent;for(;n!==null;){if(n===t)return!0;n=n.parent}return!1}function pm(e,t,n){let r=t.object,i=`handle ${e}  layer ${n}`;if(t.kind===`light`&&t.light!==void 0)return[i,`kind light/${t.light.kind}`,`enabled ${t.light.enabled}`,`intensity ${zm(t.light.intensity)}`,`color ${t.light.color.map(zm).join(`,`)}`,`shadow ${t.light.shadowIntent}`].join(`  `);if(t.kind===`staticMesh`)return[i,`kind staticMesh`,`asset ${t.asset}`,`pos ${Bm(r.position)}`,`scale ${Bm(r.scale)}`,`visible ${r.visible}`,`materials ${mm(r)}`,`label ${JSON.stringify(r.name)}`].join(`  `);if(t.kind===`sprite`&&t.sprite){let e=t.sprite,n=e.attachment;return[i,`kind sprite`,`asset ${e.asset}`,`frame ${e.frame}`,`uv ${(r.userData.uv??[0,0,1,1]).map(zm).join(`,`)}`,`pos ${Bm(r.position)}`,`size ${zm(e.size[0])},${zm(e.size[1])}`,`pivot ${zm(e.pivot[0])},${zm(e.pivot[1])}`,`billboard ${e.billboard}`,`tint ${e.tint.map(zm).join(`,`)}`,`order ${r.renderOrder}`,`depth ${e.depth}`,`shading ${e.shading}`,`visible ${r.visible}`,`attach ${n.sourceEntity??`-`}/${n.sourceSceneNode??`-`}/${n.attachmentPoint??`-`}`,`label ${JSON.stringify(r.name)}`].join(`  `)}if(t.kind===`animatedMesh`){let e=r.userData.animatedMeshPlayback??null;return[i,`kind animatedMesh`,`asset ${t.asset}`,`clip ${e?.currentClip??`-`}`,`time ${zm(e?.actionTimeSeconds??0)}`,`pos ${Bm(r.position)}`,`scale ${Bm(r.scale)}`,`visible ${r.visible}`,`label ${JSON.stringify(r.name)}`].join(`  `)}return t.kind===`voxelObject`?[i,`kind voxelObject`,`asset ${t.asset}`,`frame ${t.voxelFrame??0}`,`pos ${Bm(r.position)}`,`scale ${Bm(r.scale)}`,`visible ${r.visible}`,`materials ${mm(r)}`,`label ${JSON.stringify(r.name)}`].join(`  `):[i,`shape ${t.shape}`,`pos ${Bm(r.position)}`,`scale ${Bm(r.scale)}`,`visible ${r.visible}`,`color ${Vm(r)}`,`label ${JSON.stringify(r.name)}`].join(`  `)}function mm(e){let t=e.material;return`[`+(Array.isArray(t)?t:[t]).map(e=>{let t=e.color;if(!t)return`none`;let n=`${zm(t.r)},${zm(t.g)},${zm(t.b)}`;return!(e instanceof Ps)||e.emissiveIntensity===0||e.emissive.r===0&&e.emissive.g===0&&e.emissive.b===0?n:`${n}~emit(${`${zm(e.emissive.r)},${zm(e.emissive.g)},${zm(e.emissive.b)}`}*${zm(e.emissiveIntensity)})`}).join(` `)+`]`}function hm(e){let t=e.object,n=Array.isArray(t.material)?t.material:[t.material];e.ownedMaterialIndices?.forEach(e=>n[e]?.dispose())}function gm(e,t,n,r){let i=t?.textureTint??e.textureTint,a=t?.emissionColor??e.emissionColor,o=t?.emissionIntensity??e.emissionIntensity,s=new X(e.color[0]*i[0],e.color[1]*i[1],e.color[2]*i[2]),c=e.color[3]*i[3],l=new Ps({color:s,emissive:new X(a[0],a[1],a[2]),emissiveIntensity:o,metalness:0,map:n??null,opacity:c,roughness:e.roughness,transparent:c<1});if(e.voxelSurface!==void 0){if(n===void 0||r===void 0)throw new $(`material ${e.id} has no realized voxel texture ${e.voxelSurface.mapping.texture}`);sm(l,e,r)}return l}function _m(e,t,n){let r=e.payload;if(r===void 0)throw new $(`${n}: texture ${e.id} has no retained payload`);let i,a;if(r.source.kind===`inline`)i=Uint8Array.from(r.source.encodedBytes);else{if(t===void 0)throw new $(`${n}: resource texture needs a texture resource provider (${r.source.resource})`);try{i=t.acquireResource(r.source.resource,r.contentHash,r.byteLength).bytes.slice(),a=r.source.resource}catch(e){throw Dm(e,r.source.resource,n,`unavailable`)}}let o;try{o=em(e,i)}catch(r){if(a!==void 0&&t!==void 0)try{t.releaseResource(a)}catch{}throw r instanceof im?new $(`${n}: texture ${e.id} rejected: ${r.message}`):r}if(a!==void 0&&t!==void 0)try{t.releaseResource(a)}catch(e){throw Dm(e,a,n,`release failed`)}let s=new Bo(o.pixels,o.width,o.height,Jn,Nn);return s.colorSpace=Zr,s.flipY=!1,s.generateMipmaps=!1,s.magFilter=e.filter===`nearest`?Dn:An,s.minFilter=e.filter===`nearest`?Dn:An,s.wrapS=e.wrap===`repeat`?wn:Tn,s.wrapT=e.wrap===`repeat`?wn:Tn,s.unpackAlignment=1,s.needsUpdate=!0,{texture:s,readout:Object.freeze({id:e.id,resource:r.source.kind===`resource`?r.source.resource:null,contentHash:r.contentHash,encodedBytes:r.byteLength,decodedBytes:o.pixels.byteLength})}}function vm(e){let t;switch(e.geometry.kind){case`group`:t=new ia;break;case`cube`:t=new Oo(new Es(1,1,1),ym(`cube`,e.material));break;case`sphere`:t=new Oo(new Os(.5,8,8),ym(`sphere`,e.material));break;case`quad`:t=new Oo(new Ds(1,1),ym(`quad`,e.material));break;case`point`:t=new xs(Lm(),ym(`point`,e.material));break;case`line`:t=new hs(Rm(e.geometry.a,e.geometry.b),ym(`line`,e.material));break;default:{let t=e.geometry;throw new $(`unhandled geometry ${JSON.stringify(t)}`)}}return Jm(t,e.transform),t.visible=e.visible,Ym(t,e.metadata),t}function ym(e,t){let n=new X(t.color[0],t.color[1],t.color[2]),r=t.color[3],i=r<1;switch(e){case`point`:return new gs({color:n,opacity:r,transparent:i,size:.1});case`line`:return new is({color:n,opacity:r,transparent:i});default:return new go({color:n,opacity:r,transparent:i,wireframe:t.wireframe})}}function bm(e,t,n){let r=[],i=new Map(e.materialSlots.map((e,t)=>[e.slot,t]));try{return e.meshes.forEach((e,a)=>{let o=xm(e.payload,void 0,t,n,`defineVoxelObject.meshes[${String(a)}]`);o.clearGroups(),e.payload.groups.forEach(e=>{let t=i.get(e.materialSlot);if(t===void 0)throw o.dispose(),new $(`defineVoxelObject.meshes[${String(a)}]: unbound material slot ${e.materialSlot}`);o.addGroup(e.start,e.count,t)}),r.push(o)}),r}catch(e){throw r.forEach(e=>e.dispose()),e}}function xm(e,t,n,r,i){let a=e.source.kind===`inline`?Sm(e.source):e.source.kind===`sharedBuffer`?Cm(e,e.source,n,i):wm(e,e.source,r,i),o=Nm(e,`position`),s=Nm(e,`normal`),c=new io;c.setAttribute(`position`,new Ua(a.positions,o)),c.setAttribute(`normal`,new Ua(a.normals,s)),a.uvs!==void 0&&c.setAttribute(`uv`,new Ua(a.uvs,2)),c.setIndex(new Ua(a.indices,1));let l=t===void 0?void 0:new Map(t.map((e,t)=>[e.slot,t]));for(let t=0;t<e.groups.length;t+=1){let n=e.groups[t],r=l?.get(n.materialSlot)??(l===void 0?t:void 0);if(r===void 0)throw c.dispose(),new $(`${i}: unbound material slot ${n.materialSlot}`);c.addGroup(n.start,n.count,r)}return c.boundingBox=new Ea(new K(e.bounds.min[0],e.bounds.min[1],e.bounds.min[2]),new K(e.bounds.max[0],e.bounds.max[1],e.bounds.max[2])),c}function Sm(e){return{positions:new Float32Array(e.positions),normals:new Float32Array(e.normals),uvs:e.uvs===void 0?void 0:new Float32Array(e.uvs),indices:new Uint32Array(e.indices)}}function Cm(e,t,n,r){if(n===void 0)throw new $(`${r}: shared-buffer payload needs a mesh buffer provider (buffer ${t.buffer})`);let i=t.buffer,a;try{a=n.acquireBuffer(i)}catch(e){throw Am(e,t.buffer,r,`unavailable`)}let o;try{o=Om(a,e,t,r)}catch(e){throw Mm(n,i),e}return jm(n,i,r),o}function wm(e,t,n,r){if(n===void 0)throw new $(`${r}: resource payload needs a mesh resource provider (${t.resource})`);let i;try{i=n.acquireResource(t.resource,t.contentHash,t.byteLength)}catch(e){throw Dm(e,t.resource,r,`unavailable`)}let a;try{Tm(i.bytes,t,r),a=Em(i,e,t,r)}catch(e){try{n.releaseResource(t.resource)}catch{}throw e}try{n.releaseResource(t.resource)}catch(e){throw Dm(e,t.resource,r,`release failed`)}return a}function Tm(e,t,n){let r=t.encoding===`packedStreamsLeV1`?49:50,i=t.encoding===`packedStreamsLeV1`?`v1`:`v2`,a=[82,77,83,72,76,69,48,r];if(e.byteLength!==t.byteLength||a.some((t,n)=>e[n]!==t)||e.byteLength<16)throw new $(`${n}: mesh resource ${t.resource} has an invalid ${i} header`);let o=new DataView(e.buffer,e.byteOffset,16);if(o.getUint32(8,!0)!==e.byteLength||o.getUint32(12,!0)===0)throw new $(`${n}: mesh resource ${t.resource} has an invalid ${i} header`)}function Em(e,t,n,r){let{vertexCount:i,indexCount:a}=t.layout,o=Pm(e,n.positionsByteOffset,i*Nm(t,`position`),`positions`,n.resource,r),s=Pm(e,n.normalsByteOffset,i*Nm(t,`normal`),`normals`,n.resource,r),c=n.uvsByteOffset===void 0?void 0:Pm(e,n.uvsByteOffset,i*Nm(t,`uv`),`uvs`,n.resource,r);km(t,c,n.resource,r);let l=Fm(e,n.indicesByteOffset,a,n.resource,r);for(let e of l)if(e>=i)throw new $(`${r}: index ${e} out of range for ${i} vertices (resource ${n.resource})`);return{positions:o,normals:s,uvs:c,indices:l}}function Dm(e,t,n,r){if(e instanceof ah)return new $(`${n}: resource ${t} ${r} [${e.code}]: ${e.message}`);let i=e instanceof Error?e.message:String(e);return new $(`${n}: resource ${t} ${r} [providerFailure]: ${i}`)}function Om(e,t,n,r){let{vertexCount:i,indexCount:a}=t.layout,o=Nm(t,`position`),s=Nm(t,`normal`),c=Pm(e,n.positionsByteOffset,i*o,`positions`,n.buffer,r),l=Pm(e,n.normalsByteOffset,i*s,`normals`,n.buffer,r),u=n.uvsByteOffset===void 0?void 0:Pm(e,n.uvsByteOffset,i*Nm(t,`uv`),`uvs`,n.buffer,r);km(t,u,`buffer ${n.buffer}`,r);let d=Fm(e,n.indicesByteOffset,a,n.buffer,r);for(let e=0;e<d.length;e++)if(d[e]>=i)throw new $(`${r}: index ${d[e]} out of range for ${i} vertices (buffer ${n.buffer})`);return{positions:c,normals:l,uvs:u,indices:d}}function km(e,t,n,r){if(t===void 0)return;let i=e.provenance===`voxelChunk`||e.provenance===`voxelObject`;for(let e=0;e<t.length;e++){let a=t[e];if(!Number.isFinite(a)||i&&Math.abs(a)>16777216)throw new $(`${r}: invalid voxel tile coordinate ${a} at uvs[${e}] (${n})`)}}function Am(e,t,n,r){if(e instanceof ah)return new $(`${n}: buffer ${t} ${r} [${e.code}]: ${e.message}`);let i=e instanceof Error?e.message:String(e);return new $(`${n}: buffer ${t} ${r} [providerFailure]: ${i}`)}function jm(e,t,n){try{e.releaseBuffer(t)}catch(e){throw Am(e,t,n,`release failed`)}}function Mm(e,t){try{e.releaseBuffer(t)}catch{}}function Nm(e,t){return e.layout.attributes.find(e=>e.name===t)?.components??(t===`uv`?2:3)}function Pm(e,t,n,r,i,a){let o=Im(e,t,n*Float32Array.BYTES_PER_ELEMENT,r,i,a);return new Float32Array(o.buffer,o.byteOffset,n)}function Fm(e,t,n,r,i){let a=Im(e,t,n*Uint32Array.BYTES_PER_ELEMENT,`indices`,r,i);return new Uint32Array(a.buffer,a.byteOffset,n)}function Im(e,t,n,r,i,a){if(t<0||t+n>e.bytes.length)throw new $(`${a}: ${r} window [${t}, ${t+n}) exceeds buffer ${i} length ${e.bytes.length}`);return e.bytes.slice(t,t+n)}function Lm(){let e=new io;return e.setAttribute(`position`,new Ka([0,0,0],3)),e}function Rm(e,t){let n=new io;return n.setAttribute(`position`,new Ka([e[0],e[1],e[2],t[0],t[1],t[2]],3)),n}function zm(e){return String(Number(e.toFixed(4)))}function Bm(e){return`${zm(e.x)},${zm(e.y)},${zm(e.z)}`}function Vm(e){let t=e.material,n=(Array.isArray(t)?t[0]:t)?.color;return n?`${zm(n.r)},${zm(n.g)},${zm(n.b)}`:`none`}function Hm(e,t){return[e.geometry.uuid,t.map(e=>e.uuid).join(`,`),String(e.renderOrder),e.castShadow?`cast`:`no-cast`,e.receiveShadow?`receive`:`no-receive`].join(`|`)}function Um(e,t){let n=e;for(;n!==null;){if(!n.visible)return!1;if(n===t)return!0;n=n.parent}return!1}function Wm(e){return e.kind!==`light`&&!(e.kind===`primitive`&&e.shape===`group`)}function Gm(e,t){let n=!1,r=!1;return t.traverse(t=>{n&&r||Km(t)&&(n=!0,r||=e.intersectsObject(t))}),n&&r}function Km(e){return e instanceof Oo||e instanceof fs||e instanceof xs}function qm(e){return e.elements.every(Number.isFinite)}function Jm(e,t){e.position.set(t.translation[0],t.translation[1],t.translation[2]),e.quaternion.set(t.rotation[0],t.rotation[1],t.rotation[2],t.rotation[3]),e.scale.set(t.scale[0],t.scale[1],t.scale[2])}function Ym(e,t){e.name=t.label??``,e.userData.renderMetadata=structuredClone(t)}function Xm(e){let t=e.userData.renderMetadata;return t===void 0?{sourceEntity:null,sourceSceneNode:null,tags:[],label:null}:structuredClone(t)}function Zm(e,t){if(e.shape===`group`)return;let n=e.object,r=n.material;n.material=ym(e.shape,t),Array.isArray(r)?r.forEach(e=>e.dispose()):r.dispose()}function Qm(e){let t=e;t.geometry?.dispose(),Array.isArray(t.material)?t.material.forEach(e=>e.dispose()):t.material?.dispose()}function $m(e){let t=new Set;for(let n of Object.values(e))if(n instanceof Ti)t.add(n);else if(Array.isArray(n))for(let e of n)e instanceof Ti&&t.add(e);return t}function eh(e){let t=0,n=e.parent;for(;n!==null;)t+=1,n=n.parent;return t}function th(e){if(e instanceof jf)return new $(e.message);throw e}function nh(e){for(let t of e.values())t.forEach(e=>e.dispose())}function rh(e){nh(e.geometries);for(let t of e.textures.values())t?.texture.dispose()}function ih(e){return e.enabled&&e.kind!==`ambient`&&e.shadowIntent===`requested`}var $,ah,oh,sh,ch,lh,uh=e((()=>{cf(),at(),It(),Lf(),qf(),Xf(),am(),um(),$=class extends Error{constructor(e){super(e),this.name=`RenderApplyError`}},ah=class extends Error{code;resource;constructor(e,t,n){super(n),this.code=e,this.resource=t,this.name=`RenderResourceError`}},oh=31,sh=4096,ch=2,lh=class{scene=new da;viewmodelScene=new da;#e=new ia;#t=new ia;#n=new ia;#r=new ia;#i=new Map;#a=new Set;#o=new Map;#s=new Map;#c=new Map;#l=new Map;#u=0;#d=new Set;#f=new Map;#p=new Map;#m=0;#h;#g;#_;#v;#y;#b;#x;#S=new Pt;#C=new Set;#w=new Set;#T=new Map;#E=new Set;#D=new Map;#O=new Map;#k=new Map;#A=new WeakSet;#j=!1;constructor(e={}){if(this.#h=e.meshBufferSource,this.#g=e.meshResourceSource,this.#_=e.textureResourceSource,this.#v=e.animatedMeshSource,this.#y=new Mf(this.#v),this.#b=e.shadowsEnabled??!1,this.#x=e.maximumActiveShadowLights??8,!Number.isSafeInteger(this.#x)||this.#x<0||this.#x>8)throw new Kf(`invalid_shadow_limit`,`maximumActiveShadowLights must be an integer in 0..=8`);this.#e.name=`scene`,this.#t.name=`debug`,this.#n.name=`ui`,this.#r.name=`viewmodel`,this.viewmodelScene.name=`viewmodel`,this.scene.add(this.#e,this.#t,this.#n),this.viewmodelScene.add(this.#r)}#M(e){switch(e){case`scene`:return this.#e;case`debug`:return this.#t;case`ui`:return this.#n;case`viewmodel`:return this.#r}}applyFrame(e){if(this.#j)throw new $(`renderer is disposed`);try{let t=this.#S.validateFrame(e);this.#N(t)}catch(e){throw e instanceof U?new $(e.message):e}let t=this.#F(e),n=this.#oe(e),r=new Set,i=new Set,a=new Set,o=new Set;try{for(let n=0;n<e.ops.length;n+=1){let s=e.ops[n];if(s.op===`destroy`){if(!this.#i.has(s.handle)&&r.has(s.handle))continue;this.#W(s,r)}else this.#P(s,t.geometries.get(n),t.textures.get(n),i,a,o),t.geometries.delete(n),t.textures.delete(n)}for(let e of this.#l.values())e.texture!==null&&a.has(e.texture)&&i.add(e.id);for(let e of[...i].sort())this.#fe(e);this.#xe(a,o)}catch(e){throw rh(t),e}rh(t),this.#S.applyFrame(e),n&&this.#ie(),this.#b&&this.#e.traverse(e=>{e instanceof Oo&&(e.castShadow=!0,e.receiveShadow=!0)})}#N(e){if(!this.#b)return;let t=new Set(this.#S.snapshot().lights.filter(({light:e})=>ih(e)).map(({handle:e})=>e));for(let n of e)if(n.op===`removeLight`?t.delete(n.handle):n.op===`upsertLight`&&(ih(n.light.light)?t.add(n.light.handle):t.delete(n.light.handle)),t.size>this.#x)throw new Kf(`shadow_budget_exceeded`,`active shadow light quota ${String(this.#x)} exceeded`)}applyEncodedFrame(e){this.applyFrame(o(e))}applyDiff(e){this.applyFrame({schemaVersion:1,ops:[e]})}#P(e,t,n,r,i,a){switch(e.op){case`create`:this.#V(e);break;case`update`:this.#U(e);break;case`destroy`:this.#W(e);break;case`replaceMeshPayload`:this.#Se(e,t?.[0]);break;case`createLight`:this.#Te(e);break;case`updateLight`:this.#Ee(e);break;case`defineMaterial`:this.#ue(e.material,r);break;case`setMaterialInstanceParameters`:this.#pe(e);break;case`defineTexture`:this.#de(e.texture,n,i);break;case`defineSpriteAtlas`:this.#p.set(e.atlas.id,e.atlas),a?.add(e.atlas.id);break;case`defineStaticMesh`:this.#G(e.asset,t?.[0]);break;case`defineAnimatedMesh`:this.#J(e);break;case`createAnimatedMeshInstance`:this.#Y(e);break;case`setAnimatedMeshPlayback`:this.#X(e);break;case`defineVoxelObject`:this.#Q(e.asset,t);break;case`releaseVoxelObject`:this.#$(e.asset);break;case`createVoxelObjectInstance`:this.#ee(e);break;case`setVoxelObjectFrame`:this.#te(e);break;case`createStaticMeshInstance`:this.#K(e);break;case`createSprite`:this.#ve(e);break;case`updateSprite`:this.#ye(e);break}}#F(e){let t={geometries:new Map,textures:new Map},n=new Map,r=new Map([...this.#f].map(([e,t])=>[e,t.version])),i=new Map([...this.#f].map(([e,t])=>[e,structuredClone(t)])),a=new Map([...this.#l].map(([e,t])=>[e,structuredClone(t)])),o=new Map([...this.#D].map(([e,t])=>[e,t.readout])),s={count:o.size,encodedBytes:[...o.values()].reduce((e,t)=>e+t.encodedBytes,0),decodedBytes:[...o.values()].reduce((e,t)=>e+t.decodedBytes,0)};try{for(let c=0;c<e.ops.length;c+=1){let l=e.ops[c];if(l.op===`defineStaticMesh`)t.geometries.set(c,[xm(l.asset.payload,l.asset.materialSlots,this.#h,this.#g,`defineStaticMesh`)]);else if(l.op===`replaceMeshPayload`)t.geometries.set(c,[xm(l.payload,void 0,this.#h,this.#g,`replaceMeshPayload`)]);else if(l.op===`defineVoxelObject`)t.geometries.set(c,bm(l.asset,this.#h,this.#g));else if(l.op===`defineTexture`){let e=r.get(l.texture.id);if(e!==void 0&&l.texture.version<=e)throw new $(`defineTexture: stale or duplicate version ${String(l.texture.version)} for ${l.texture.id}`);let n=o.get(l.texture.id),a=l.texture.payload;if(a===void 0)s=dm(s,n,void 0),o.delete(l.texture.id),t.textures.set(c,null);else{let e=l.texture.width*l.texture.height*4,r={encodedBytes:a.byteLength,decodedBytes:e};s=dm(s,n,r);let i=_m(l.texture,this.#_,`defineTexture`);o.set(l.texture.id,i.readout),t.textures.set(c,i)}r.set(l.texture.id,l.texture.version),i.set(l.texture.id,structuredClone(l.texture))}else if(l.op===`defineMaterial`)a.set(l.material.id,structuredClone(l.material));else if(l.op===`defineAnimatedMesh`)this.#y.validateDefinition(l.asset);else if(l.op===`createAnimatedMeshInstance`&&l.instance.materialOverrides.length>0)throw new $(`createAnimatedMeshInstance: material overrides are not implemented for animated mesh ${l.instance.asset}`);else if(l.op===`createAnimatedMeshInstance`){let e=l.instance.playback;if(e?.kind===`pause`||e?.kind===`resume`)throw new $(`createAnimatedMeshInstance.${e.kind}: no current clip on ${l.instance.asset}`);n.set(l.handle,e?.kind===`play`?e.clip:null)}else if(l.op===`setAnimatedMeshPlayback`){let e=n.has(l.handle)?n.get(l.handle)??null:this.#y.playback(l.handle)?.currentClip??null;if((l.playback.kind===`pause`||l.playback.kind===`resume`)&&e===null)throw new $(`setAnimatedMeshPlayback.${l.playback.kind}: no current clip`);l.playback.kind===`play`?n.set(l.handle,l.playback.clip):l.playback.kind===`stop`&&n.set(l.handle,null)}}for(let e of a.values()){if(e.schemaVersion>=3&&e.texture!==null&&!o.has(e.texture))throw new $(`defineMaterial: texture ${e.texture} has no admitted retained payload`);if(e.voxelSurface!==void 0){let t=i.get(e.voxelSurface.mapping.texture);if(t===void 0)throw new $(`defineMaterial: missing voxel surface texture ${e.voxelSurface.mapping.texture}`);try{om(e,t)}catch(e){throw e instanceof lm?new $(`defineMaterial: ${e.message}`):e}}}return t}catch(e){throw rh(t),th(e)}}registerSlotColor(e,t,n,r){this.#c.set(e,new X(t,n,r))}#I(e){let t=this.#c.get(e);if(t)return t.clone();let n=e*.61803398875%1;return new X().setHSL(n,.7,.5)}has(e){return this.#i.has(e)}get handleCount(){return this.#i.size}resourceStatistics(){return Object.freeze({renderHandleCount:this.#i.size,geometryResourceCount:this.#C.size,materialResourceCount:this.#w.size,textureResourceCount:this.#E.size,animatedInstanceCount:this.#y.instanceCount})}#L(e){e.traverse(e=>{let t=e;t.geometry instanceof io&&this.#R(t.geometry),Array.isArray(t.material)?t.material.forEach(e=>this.#z(e)):t.material instanceof oo&&this.#z(t.material)})}#R(e){this.#C.has(e)||(this.#C.add(e),e.addEventListener(`dispose`,()=>this.#C.delete(e)))}#z(e){if(this.#w.has(e))return;this.#w.add(e);let t=$m(e);for(let e of t){let t=this.#T.get(e)??0;this.#B(e),this.#T.set(e,t+1)}e.addEventListener(`dispose`,()=>{if(this.#w.delete(e))for(let e of t){let t=this.#T.get(e);t===void 0||t<=1?this.#T.delete(e):this.#T.set(e,t-1)}})}#B(e){this.#E.has(e)||(this.#E.add(e),e.addEventListener(`dispose`,()=>{this.#E.delete(e),this.#T.delete(e)}))}lightReadout(){return[...this.#i.entries()].filter(e=>e[1].kind===`light`&&e[1].light!==void 0).sort(([e],[t])=>e-t).map(([e,t])=>({descriptor:structuredClone(t.light),handle:e,parent:Vf(t.object.parent,this.#i),shadowStatus:Bf(t.light,this.#b)}))}meshPresentationReadout(){return[...this.#i.entries()].filter(([,e])=>e.meshProvenance!==void 0).sort(([e],[t])=>e-t).map(([e,t])=>({handle:e,lit:Jf(t.object).every(e=>e instanceof Ps),materialSlots:[...t.meshMaterialSlots??[]],opacity:t.viewMaterial?.color[3]??1,wireframe:t.viewMaterial?.wireframe??!1}))}dispose(){if(this.#j)return;this.#le();let e=[...this.#i.entries()].sort((e,t)=>eh(t[1].object)-eh(e[1].object)).map(([e])=>e);for(let t of e)this.#i.has(t)&&this.#W({op:`destroy`,handle:t});this.#a.clear();for(let e of this.#o.values())e.geometry.dispose(),e.materials.forEach(e=>e.dispose());this.#o.clear();for(let e of this.#s.values())e.geometries.forEach(e=>e.dispose()),e.materials.forEach(e=>e.dispose());this.#s.clear(),this.#y.dispose(),this.#c.clear(),this.#l.clear(),this.#d.clear();for(let e of this.#D.values())e.texture.dispose();this.#D.clear(),this.#f.clear(),this.#p.clear(),this.scene.clear(),this.viewmodelScene.clear(),this.#C.clear(),this.#w.clear(),this.#T.clear(),this.#E.clear(),this.#j=!0}objectFor(e){return this.#i.get(e)?.object}projectionIdentityForObject(e,t){if(e instanceof Yo&&t!==void 0){let n=this.#k.get(e)?.handles[t],r=n===void 0?void 0:this.#i.get(n);if(n!==void 0&&r!==void 0)return{handle:n,layer:this.#H(r.object),metadata:Xm(r.object)}}let n=e;for(;n!==null;){for(let[e,t]of this.#i.entries())if(t.object===n)return{handle:e,layer:this.#H(t.object),metadata:Xm(t.object)};n=n.parent}}projectionWorldNormalForObject(e,t,n){if(e instanceof Yo&&t!==void 0&&this.#k.has(e)){let r=new Y;return e.getMatrixAt(t,r),r.premultiply(e.matrixWorld),n.clone().applyNormalMatrix(new q().getNormalMatrix(r))}return n.clone().transformDirection(e.matrixWorld)}prepareStaticInstanceBatches(e){if(this.#j)throw new $(`renderer is disposed`);this.scene.updateMatrixWorld(!0),e.updateMatrixWorld(!0);let t=new Y().multiplyMatrices(e.projectionMatrix,e.matrixWorldInverse),n=new rs().setFromProjectionMatrix(t);for(let e of this.#O.values()){let t=e.candidateHandles.filter(e=>{let t=this.#i.get(e);return t!==void 0&&t.object instanceof Oo&&n.intersectsObject(t.object)});this.#ae(e,t)}}visibilityReadout(e,t=this.scene){if(this.#j)throw new $(`renderer is disposed`);e.updateMatrixWorld(!0),t.updateMatrixWorld(!0),this.prepareSpritesForCamera(e,t);let n=new Y().multiplyMatrices(e.projectionMatrix,e.matrixWorldInverse),r=new rs().setFromProjectionMatrix(n),i=[...this.#i.entries()].filter(([,e])=>fm(e.object,t)).sort(([e],[t])=>e-t).map(([e,n])=>{let i=Um(n.object,t),a=Wm(n),o=a&&Gm(r,n.object);return Object.freeze({handle:e,state:a?i?o?`frustumVisible`:`outsideFrustum`:`hidden`:`notDrawable`,inFrustum:o,effectivelyVisible:i,occlusion:`notMeasured`})});return Object.freeze({schemaVersion:1,basis:`cpuFrustum`,occlusion:`notMeasured`,handles:Object.freeze(i)})}prepareSpritesForCamera(e,t=this.scene){if(this.#j)throw new $(`renderer is disposed`);e.updateMatrixWorld(!0),t.updateMatrixWorld(!0);let n=new K().setFromMatrixPosition(e.matrixWorld),r=e.getWorldQuaternion(new pi),i=new K,a=new K,o=new pi,s=new pi,c=new pi,l=new pi,u=new K,d=new K,f=new K(0,1,0),p=new Y,m=[...this.#a].map(e=>this.#i.get(e)).filter(e=>e!==void 0&&e.kind===`sprite`&&e.sprite!==void 0&&fm(e.object,t)).sort((e,t)=>eh(e.object)-eh(t.object));for(let e of m){let t=e.sprite;t!==void 0&&e.object.quaternion.set(...t.transform.rotation)}t.updateMatrixWorld(!0);for(let t of m){let m=t.sprite;if(m===void 0||m.billboard===`none`)continue;let h=t.object;h.updateMatrixWorld(!0),h.getWorldPosition(a),m.billboard===`spherical`?o.copy(r):(e instanceof vc?(e.getWorldDirection(i),u.copy(i).negate()):u.subVectors(n,a),u.y=0,u.lengthSq()<=2**-52&&(h.getWorldQuaternion(s),u.set(0,0,1).applyQuaternion(s),u.y=0,u.lengthSq()<=2**-52&&u.set(0,0,1)),u.normalize(),d.crossVectors(f,u).normalize(),p.makeBasis(d,f,u),o.setFromRotationMatrix(p).normalize()),h.parent===null?h.quaternion.copy(o):(h.parent.getWorldQuaternion(c),l.copy(c).invert().multiply(o).normalize(),h.quaternion.copy(l)),h.updateMatrixWorld(!0)}t.updateMatrixWorld(!0)}prepareStaticInstanceBatchesForPicking(){if(this.#j)throw new $(`renderer is disposed`);this.scene.updateMatrixWorld(!0);for(let e of this.#O.values())this.#ae(e,e.candidateHandles)}advanceAnimation(e){try{this.#y.advance(e)}catch(e){throw th(e)}for(let[e,t]of this.#i.entries())t.kind===`animatedMesh`&&this.#Z(e,t)}animatedMeshPlayback(e){return this.#y.playback(e)}sampleAnimatedMesh(e,t,n){try{let r=this.#y.sample(e,t,n);return this.#Z(e,this.#De(e,`sampleAnimatedMesh`)),r}catch(e){throw th(e)}}setAnimationControllerWeights(e,t){try{this.#y.setControllerWeights(e,t),this.#Z(e,this.#De(e,`setAnimationControllerWeights`))}catch(e){throw th(e)}}hasAnimationControllerClips(e,t){return this.#y.hasClips(e,t)}clearAnimationControllerWeights(e){try{this.#y.clearControllerWeights(e),this.#Z(e,this.#De(e,`clearAnimationControllerWeights`))}catch(e){throw th(e)}}snapshot(){let e=[...this.#i.entries()].sort((e,t)=>e[0]-t[0]);return e.length===0?`(empty scene)
`:e.map(([e,t])=>pm(e,t,this.#H(t.object))).join(`
`)+`
`}#V(e){if(this.#i.has(e.handle))throw new $(`create: handle ${e.handle} already exists`);let t=vm(e.node);this.#L(t),(e.parent===null?this.#M(e.node.layer):this.#De(e.parent,`create.parent`).object).add(t),this.#i.set(e.handle,{object:t,kind:`primitive`,shape:e.node.geometry.kind,ownsGeometry:e.node.geometry.kind!==`group`,viewMaterial:e.node.material})}#H(e){return fm(e,this.#r)?`viewmodel`:fm(e,this.#t)?`debug`:fm(e,this.#n)?`ui`:`scene`}#U(e){let t=this.#De(e.handle,`update`);e.transform&&Jm(t.object,e.transform),e.material&&(t.meshProvenance===void 0?Zm(t,e.material):this.#we(t,e.material),t.viewMaterial=e.material,this.#L(t.object)),e.visible!==null&&(t.object.visible=e.visible),e.metadata&&Ym(t.object,e.metadata)}#W(e,t){let n=this.#De(e.handle,`destroy`),r=[...this.#i.entries()].filter(([,e])=>e.object.parent===n.object).map(([e])=>e).sort((e,t)=>e-t);for(let e of r)this.#W({op:`destroy`,handle:e},t);if(n.object.parent?.remove(n.object),n.kind===`staticMesh`&&n.asset!==void 0)hm(n),this.#q(n.asset);else if(n.kind===`animatedMesh`)this.#y.release(e.handle);else if(n.kind===`voxelObject`&&n.asset!==void 0){hm(n);let e=this.#s.get(n.asset);e!==void 0&&--e.refCount}else n.kind===`light`?Hf(n.object):Qm(n.object);this.#i.delete(e.handle),this.#a.delete(e.handle),t?.add(e.handle)}#G(e,t){let n=this.#o.get(e.asset);if(n){if(n.refCount>0)throw new $(`defineStaticMesh: asset ${e.asset} is in use by ${n.refCount} instance(s)`);n.geometry.dispose(),n.materials.forEach(e=>e.dispose())}let r=t??xm(e.payload,e.materialSlots,this.#h,this.#g,`defineStaticMesh`);this.#R(r);let i=new Map,a=e.materialSlots.map((e,t)=>(i.set(e.slot,t),this.#me(e)));this.#o.set(e.asset,{geometry:r,materials:a,slotIndex:i,materialSlots:e.materialSlots,collision:e.collision,refCount:0})}#K(e){if(this.#i.has(e.handle))throw new $(`createStaticMeshInstance: handle ${e.handle} already exists`);let t=this.#o.get(e.instance.asset);if(!t)throw new $(`createStaticMeshInstance: undefined static mesh asset ${e.instance.asset}`);let n=t.materials.slice(),r=t.materialSlots.map(e=>e.material),i=new Set;for(let a of e.instance.materialOverrides){let o=t.slotIndex.get(a.slot);if(o===void 0)throw new $(`createStaticMeshInstance: override for unbound slot ${a.slot} on ${e.instance.asset}`);n[o]=this.#me(a),r[o]=a.material,i.add(o)}let a=new Oo(t.geometry,n.length===1?n[0]:n);this.#A.add(a),Jm(a,e.instance.transform),Ym(a,e.instance.metadata),a.visible=e.instance.visible,(e.parent===null?this.#e:this.#De(e.parent,`createStaticMeshInstance.parent`).object).add(a),t.refCount+=1,this.#i.set(e.handle,{object:a,kind:`staticMesh`,shape:`quad`,asset:e.instance.asset,ownsGeometry:!1,materialIds:r,ownedMaterialIndices:i,materialParameterOverrides:new Map})}#q(e){let t=this.#o.get(e);t&&--t.refCount}#J(e){try{this.#y.define(e.asset)}catch(e){throw th(e)}}#Y(e){if(this.#i.has(e.handle))throw new $(`createAnimatedMeshInstance: handle ${e.handle} already exists`);let t;try{t=this.#y.create(e.handle,e.instance)}catch(e){throw th(e)}Jm(t.object,e.instance.transform),Ym(t.object,e.instance.metadata),t.object.visible=e.instance.visible,this.#L(t.object),(e.parent===null?this.#e:this.#De(e.parent,`createAnimatedMeshInstance.parent`).object).add(t.object),this.#i.set(e.handle,{object:t.object,kind:`animatedMesh`,shape:`quad`,asset:e.instance.asset,ownsGeometry:!1}),this.#Z(e.handle,this.#De(e.handle,`createAnimatedMeshInstance`))}#X(e){let t=this.#De(e.handle,`setAnimatedMeshPlayback`);try{this.#y.setPlayback(e.handle,e.playback)}catch(e){throw th(e)}this.#Z(e.handle,t)}#Z(e,t){t.object.userData.animatedMeshPlayback=this.#y.playback(e)}#Q(e,t){let n=t===void 0?bm(e,this.#h,this.#g):[...t];if(n.length!==e.meshes.length)throw new $(`defineVoxelObject: prepared ${n.length} meshes for ${e.meshes.length} descriptors`);let r=new Map,i=e.materialSlots.map((e,t)=>(r.set(e.slot,t),this.#me(e)));n.forEach(e=>this.#R(e));let a=this.#s.get(e.asset),o={geometries:n,frames:e.frames,meshMaterialSlots:e.meshes.map(e=>e.payload.groups.map(e=>e.materialSlot)),materials:i,slotIndex:r,materialSlots:e.materialSlots,refCount:a?.refCount??0};if(a!==void 0){for(let t of this.#i.values()){if(t.kind!==`voxelObject`||t.asset!==e.asset)continue;let r=t.voxelFrame??0,a=o.frames[r],s=a===void 0?void 0:o.geometries[a.mesh];if(a===void 0||s===void 0)throw n.forEach(e=>e.dispose()),i.forEach(e=>e.dispose()),new $(`defineVoxelObject: live frame ${r} is unavailable on ${e.asset}`);let c=this.#re(o,t.voxelMaterialOverrides??[]);hm(t);let l=t.object;l.geometry=s,l.material=c.materials.length===1?c.materials[0]:c.materials,t.materialIds=c.materialIds,t.ownedMaterialIndices=c.ownedMaterialIndices,t.meshMaterialSlots=e.meshes[a.mesh].payload.groups.map(e=>e.materialSlot)}a.geometries.forEach(e=>e.dispose()),a.materials.forEach(e=>e.dispose())}this.#s.set(e.asset,o)}#$(e){let t=this.#s.get(e);if(t===void 0)throw new $(`releaseVoxelObject: undefined voxel object ${e}`);if(t.refCount!==0)throw new $(`releaseVoxelObject: ${e} is in use by ${t.refCount} instance(s)`);t.geometries.forEach(e=>e.dispose()),t.materials.forEach(e=>e.dispose()),this.#s.delete(e)}#ee(e){if(this.#i.has(e.handle))throw new $(`createVoxelObjectInstance: handle ${e.handle} already exists`);let t=this.#s.get(e.instance.asset);if(t===void 0)throw new $(`createVoxelObjectInstance: undefined voxel object ${e.instance.asset}`);let n=t.frames[e.instance.frame],r=n===void 0?void 0:t.geometries[n.mesh];if(r===void 0)throw new $(`createVoxelObjectInstance: frame ${e.instance.frame} unavailable on ${e.instance.asset}`);let i=this.#re(t,e.instance.materialOverrides),a=new Oo(r,i.materials.length===1?i.materials[0]:i.materials);this.#A.add(a),Jm(a,e.instance.transform),Ym(a,e.instance.metadata),a.visible=e.instance.visible,(e.parent===null?this.#e:this.#De(e.parent,`createVoxelObjectInstance.parent`).object).add(a),t.refCount+=1,this.#i.set(e.handle,{object:a,kind:`voxelObject`,shape:`quad`,asset:e.instance.asset,ownsGeometry:!1,materialIds:i.materialIds,ownedMaterialIndices:i.ownedMaterialIndices,meshProvenance:`voxelObject`,meshMaterialSlots:this.#ne(e.instance.asset,e.instance.frame),voxelFrame:e.instance.frame,voxelMaterialOverrides:structuredClone(e.instance.materialOverrides)})}#te(e){let t=this.#De(e.handle,`setVoxelObjectFrame`);if(t.kind!==`voxelObject`||t.asset===void 0)throw new $(`setVoxelObjectFrame: handle ${e.handle} is not a voxel object`);let n=this.#s.get(t.asset),r=n?.frames[e.frame],i=r===void 0?void 0:n?.geometries[r.mesh];if(n===void 0||r===void 0||i===void 0)throw new $(`setVoxelObjectFrame: frame ${e.frame} unavailable on ${t.asset}`);t.object.geometry=i,t.voxelFrame=e.frame,t.meshMaterialSlots=this.#ne(t.asset,e.frame),t.object.userData.voxelObjectFrame=e.frame}#ne(e,t){let n=this.#s.get(e),r=n?.frames[t];return n===void 0||r===void 0?[]:[...n.meshMaterialSlots[r.mesh]??[]]}#re(e,t){let n=e.materials.slice(),r=e.materialSlots.map(e=>e.material),i=new Set;for(let a of t){let t=e.slotIndex.get(a.slot);if(t===void 0)throw new $(`voxel object material override uses unbound slot ${a.slot}`);n[t]=this.#me(a),r[t]=a.material,i.add(t)}return{materials:n,materialIds:r,ownedMaterialIndices:i}}voxelObjectFrame(e){let t=this.#i.get(e);if(t?.kind!==`voxelObject`||t.asset===void 0||t.voxelFrame===void 0)return;let n=this.#s.get(t.asset)?.frames[t.voxelFrame];if(n!==void 0)return{handle:e,asset:t.asset,frame:t.voxelFrame,frameId:n.id,mesh:n.mesh}}instanceCountFor(e){return this.#o.get(e)?.refCount??0}#ie(){let e=new Map;for(let e of this.#i.values())(e.kind===`staticMesh`||e.kind===`voxelObject`)&&e.object instanceof Oo&&e.object.layers.set(0);this.scene.updateMatrixWorld(!0);let t=[...this.#i.entries()].sort(([e],[t])=>e-t);for(let[n,r]of t){if(r.kind!==`staticMesh`&&r.kind!==`voxelObject`||!(r.object instanceof Oo)||r.object instanceof Yo||this.#H(r.object)!==`scene`||!Um(r.object,this.#e)||r.object.matrixWorld.determinant()<=0||!qm(r.object.matrixWorld)||r.object.customDepthMaterial!==void 0||r.object.customDistanceMaterial!==void 0||this.#b&&r.object.castShadow)continue;let t=Array.isArray(r.object.material)?r.object.material:[r.object.material];if(t.length===0||t.some(e=>e.transparent||e.opacity<1))continue;let i=Hm(r.object,t),a=e.get(i)??[];a.push({handle:n,mesh:r.object}),e.set(i,a)}let n=new Set;for(let[t,r]of e.entries())if(!(r.length<ch))for(let e=0;e<r.length;e+=sh){let i=r.slice(e,e+sh);if(i.length<ch)continue;let a=`${t}|chunk:${String(Math.floor(e/sh))}`;n.add(a);let o=i[0].mesh,s=Array.isArray(o.material)?o.material:[o.material],c=this.#O.get(a);if(c===void 0||c.mesh.instanceMatrix.count<i.length){c!==void 0&&this.#ce(a,c);let e=new Yo(o.geometry,s.length===1?s[0]:s,i.length);e.name=`static-instance-batch:${t}`,e.castShadow=o.castShadow,e.receiveShadow=o.receiveShadow,e.renderOrder=o.renderOrder,e.frustumCulled=!0,e.instanceMatrix.setUsage(ri),e.layers.set(0),this.#e.add(e),c={mesh:e,candidateHandles:[],handles:[]},this.#O.set(a,c),this.#k.set(e,c)}c.candidateHandles=i.map(({handle:e})=>e),this.#ae(c,c.candidateHandles)}for(let[e,t]of[...this.#O.entries()])n.has(e)||this.#ce(e,t)}#ae(e,t){for(let t of e.candidateHandles){let e=this.#i.get(t);e?.object instanceof Oo&&e.object.layers.set(oh)}if(t.length<ch){if(e.handles=[],e.mesh.count=0,e.mesh.visible=!1,t.length===1){let e=this.#i.get(t[0]);e?.object instanceof Oo&&e.object.layers.set(0)}return}e.handles=[...t],e.mesh.visible=!0,e.mesh.count=t.length;for(let n=0;n<t.length;n+=1){let r=this.#i.get(t[n]);if(r===void 0)throw new $(`static instance batch references missing handle ${t[n]}`);e.mesh.setMatrixAt(n,r.object.matrixWorld)}e.mesh.instanceMatrix.needsUpdate=!0,e.mesh.boundingBox=null,e.mesh.boundingSphere=null,e.mesh.computeBoundingBox(),e.mesh.computeBoundingSphere()}#oe(e){return e.ops.some(e=>{switch(e.op){case`defineMaterial`:case`defineStaticMesh`:case`defineVoxelObject`:case`releaseVoxelObject`:case`createStaticMeshInstance`:case`createVoxelObjectInstance`:case`setVoxelObjectFrame`:case`setMaterialInstanceParameters`:return!0;case`destroy`:case`replaceMeshPayload`:{let t=this.#i.get(e.handle);return t!==void 0&&this.#se(t.object)}case`update`:{if(e.transform===null&&e.material===null&&e.visible===null)return!1;let t=this.#i.get(e.handle);return t!==void 0&&this.#se(t.object)}default:return!1}})}#se(e){let t=!1;return e.traverse(e=>{t||=this.#A.has(e)}),t}#ce(e,t){t.mesh.parent?.remove(t.mesh),t.mesh.dispose(),this.#k.delete(t.mesh),this.#O.delete(e)}#le(){for(let[e,t]of[...this.#O.entries()])this.#ce(e,t)}#ue(e,t){this.#l.set(e.id,e),t===void 0?this.#fe(e.id):t.add(e.id)}#de(e,t,n){if(e.payload!==void 0&&t===void 0)throw new $(`defineTexture: missing prepared payload for ${e.id}`);let r=this.#D.get(e.id);if(this.#f.set(e.id,structuredClone(e)),t===null||e.payload===void 0?this.#D.delete(e.id):t!==void 0&&(this.#D.set(e.id,t),this.#B(t.texture)),n===void 0)for(let t of this.#l.values())t.texture===e.id&&this.#fe(t.id);else n.add(e.id);r?.texture.dispose()}#fe(e){let t=new Set;for(let n of this.#o.values())for(let r=0;r<n.materialSlots.length;r+=1){let i=n.materialSlots[r];i.material===e&&(t.add(n.materials[r]),n.materials[r]=this.#me(i))}for(let n of this.#s.values())for(let r=0;r<n.materialSlots.length;r+=1){let i=n.materialSlots[r];i.material===e&&(t.add(n.materials[r]),n.materials[r]=this.#me(i))}for(let t of this.#i.values()){if(t.meshMaterialSlots?.some(t=>`voxel-material/${String(t)}`===e)){this.#we(t,t.viewMaterial??Yf);continue}if(t.kind!==`staticMesh`&&t.kind!==`voxelObject`||!t.materialIds||t.asset===void 0)continue;let n=t.kind===`staticMesh`?this.#o.get(t.asset):this.#s.get(t.asset);if(n===void 0)continue;let r=t.object,i=Array.isArray(r.material)?r.material:[r.material],a=!1;for(let r=0;r<t.materialIds.length;r+=1){if(t.materialIds[r]!==e)continue;t.ownedMaterialIndices?.has(r)&&i[r]?.dispose();let o=t.kind===`staticMesh`?t.materialParameterOverrides?.get(r):void 0,s=n.materialSlots[r],c=o===void 0&&s?.material===e;i[r]=c?n.materials[r]:this.#me({slot:s?.slot??r,material:e},o),c?t.ownedMaterialIndices?.delete(r):t.ownedMaterialIndices?.add(r),a=!0}a&&(r.material=i.length===1?i[0]:i)}t.forEach(e=>e.dispose())}#pe(e){let t=this.#De(e.handle,`setMaterialInstanceParameters`);if(t.kind!==`staticMesh`||t.asset===void 0||t.materialIds===void 0)throw new $(`setMaterialInstanceParameters: handle ${e.handle} is not a static-mesh instance`);let n=this.#o.get(t.asset),r=n?.slotIndex.get(e.slot);if(n===void 0||r===void 0)throw new $(`setMaterialInstanceParameters: unbound slot ${e.slot} on ${t.asset}`);let i=t.materialIds[r];if(i==null)throw new $(`setMaterialInstanceParameters: slot ${e.slot} on ${t.asset} has no material`);let a=t.object,o=Array.isArray(a.material)?a.material:[a.material];t.ownedMaterialIndices?.has(r)&&o[r]?.dispose();let s=n.materialSlots[r];e.parameters===null?(t.materialParameterOverrides?.delete(r),s.material===i?(o[r]=n.materials[r],t.ownedMaterialIndices?.delete(r)):(o[r]=this.#me({slot:e.slot,material:i}),t.ownedMaterialIndices?.add(r))):(t.materialParameterOverrides?.set(r,e.parameters),o[r]=this.#me({slot:e.slot,material:i},e.parameters),t.ownedMaterialIndices?.add(r)),a.material=o.length===1?o[0]:o}materialDescriptor(e){return this.#l.get(e)}get fallbackMaterialCount(){return this.#u}fallbackMaterials(){return[...this.#d].sort()}#me(e,t){let n=this.#l.get(e.material);if(n){let e=gm(n,t,n.texture===null?void 0:this.#D.get(n.texture)?.texture,n.texture===null?void 0:this.#f.get(n.texture));return this.#z(e),e}this.#u+=1,this.#d.add(e.material);let r=new Ps({color:this.#I(e.slot),roughness:1,metalness:0});return this.#z(r),r}textureDescriptor(e){let t=this.#f.get(e);return t===void 0?void 0:structuredClone(t)}textureResourceReadout(){return Object.freeze([...this.#D.values()].map(e=>Object.freeze({...e.readout})).sort((e,t)=>e.id.localeCompare(t.id)))}voxelSurfaceMaterialReadout(){return Object.freeze([...this.#w].map(e=>e.userData.rustyVoxelSurface).filter(e=>e!==void 0).map(e=>Object.freeze(structuredClone(e))).sort((e,t)=>e.material.localeCompare(t.material)))}spriteAtlas(e){return this.#p.get(e)}get spriteFallbackCount(){return this.#m}#he(e,t,n){let r=this.#p.get(t),i=r?.frames.find(e=>e.frame===n);if(!i)return(r!==void 0||this.#f.size>0||n!==0)&&(this.#m+=1),[0,0,1,1];let[a,o]=i.uvMin,[s,c]=i.uvMax,l=e.getAttribute(`uv`);return l.setXY(0,a,c),l.setXY(1,s,c),l.setXY(2,a,o),l.setXY(3,s,o),l.needsUpdate=!0,[a,o,s,c]}#ge(e,t,n){return this.#p.get(e)?.frames.find(e=>e.frame===t)?.size??n}#_e(e,t){let n=this.#ge(e.asset,t,e.size),r=new Ds(n[0],n[1]);return r.translate((.5-e.pivot[0])*n[0],(.5-e.pivot[1])*n[1],0),r}#ve(e){if(this.#i.has(e.handle))throw new $(`createSprite: handle ${e.handle} already exists`);let t=e.sprite,n=this.#_e(t,t.frame),r=this.#be(t),i=new Oo(n,r);this.#L(i),i.renderOrder=t.renderOrder,Jm(i,t.transform),Ym(i,t.metadata),i.visible=t.visible,i.userData.frame=t.frame,i.userData.billboard=t.billboard,i.userData.uv=this.#he(n,t.asset,t.frame),(e.parent===null?this.#e:this.#De(e.parent,`createSprite.parent`).object).add(i),this.#i.set(e.handle,{object:i,kind:`sprite`,shape:`quad`,asset:t.asset,ownsGeometry:!0,sprite:t}),t.billboard!==`none`&&this.#a.add(e.handle)}#ye(e){let t=this.#De(e.handle,`updateSprite`);if(t.kind!==`sprite`||!t.sprite)throw new $(`updateSprite: handle ${e.handle} is not a sprite`);let n=t.object,r=n.material;if(e.frame!==null){t.sprite={...t.sprite,frame:e.frame},n.userData.frame=e.frame;let r=n.geometry,i=this.#_e(t.sprite,e.frame);n.geometry=i,this.#R(i),n.userData.uv=this.#he(i,t.sprite.asset,e.frame),r.dispose()}e.tint!==null&&(t.sprite={...t.sprite,tint:e.tint},r.color.setRGB(e.tint[0],e.tint[1],e.tint[2]),r.opacity=e.tint[3],r.transparent=e.tint[3]<1||r.map!==null),e.renderOrder!==null&&(t.sprite={...t.sprite,renderOrder:e.renderOrder},n.renderOrder=e.renderOrder),e.visible!==null&&(n.visible=e.visible,t.sprite={...t.sprite,visible:e.visible})}#be(e){let t=this.#p.get(e.asset),n=t===void 0?void 0:this.#D.get(t.texture)?.texture,r=new go({color:new X(e.tint[0],e.tint[1],e.tint[2]),map:n??null,opacity:e.tint[3],transparent:e.tint[3]<1||n!==void 0,depthTest:e.depth!==`depthTestOff`,depthWrite:e.depth==="default"});return this.#z(r),r}#xe(e,t){if(!(e.size===0&&t.size===0))for(let n of this.#i.values()){if(n.kind!==`sprite`||n.sprite===void 0)continue;let r=this.#p.get(n.sprite.asset);if(r===void 0||!t.has(n.sprite.asset)&&!e.has(r.texture))continue;let i=n.object,a=i.material;i.material=this.#be(n.sprite),t.has(n.sprite.asset)&&(i.userData.uv=this.#he(i.geometry,n.sprite.asset,n.sprite.frame)),a.dispose()}}pickSprite(e){let t=this.#i.get(e);if(!t||t.kind!==`sprite`||!t.sprite)return;let n=t.sprite.attachment;return{handle:e,sourceEntity:n.sourceEntity,sourceSceneNode:n.sourceSceneNode,asset:t.sprite.asset,attachmentPoint:n.attachmentPoint}}#Se(e,t){let n=this.#De(e.handle,`replaceMeshPayload`),r=n.object;if(!(r instanceof Oo))throw new $(`replaceMeshPayload: handle ${e.handle} is not a mesh`);let i=t??xm(e.payload,void 0,this.#h,this.#g,`replaceMeshPayload`);this.#R(i);let a=n.viewMaterial??Yf,o=e.payload.groups.map(e=>this.#Ce(e.materialSlot,a)),s=r.geometry,c=r.material;r.geometry=i,r.material=o.length===1?o[0]:o,s.dispose(),Array.isArray(c)?c.forEach(e=>e.dispose()):c.dispose(),n.meshProvenance=e.payload.provenance,n.meshMaterialSlots=e.payload.groups.map(e=>e.materialSlot),n.viewMaterial=a}#Ce(e,t){let n=this.#l.get(`voxel-material/${String(e)}`);if(n!==void 0){let e=gm(n,void 0,n.texture===null?void 0:this.#D.get(n.texture)?.texture);return e.color.multiply(new X(t.color[0],t.color[1],t.color[2])),e.opacity*=t.color[3],e.transparent=e.opacity<1,e.wireframe=t.wireframe,this.#z(e),e}let r=this.#I(e),i=new Ps({color:new X(r.r*t.color[0],r.g*t.color[1],r.b*t.color[2]),opacity:t.color[3],transparent:t.color[3]<1,wireframe:t.wireframe,roughness:1,metalness:0});return this.#z(i),i}#we(e,t){let n=e.object,r=Jf(n),i=(e.meshMaterialSlots??[]).map(e=>this.#Ce(e,t));n.material=i.length===1?i[0]:i,r.forEach(e=>e.dispose())}#Te(e){if(this.#i.has(e.handle))throw new $(`createLight: handle ${e.handle} already exists`);Uf(e.light,`createLight.light`,e=>new $(e));let t=Rf(e.light,this.#b);(e.parent===null?this.#e:this.#De(e.parent,`createLight.parent`).object).add(t),this.#i.set(e.handle,{object:t,kind:`light`,shape:`point`,ownsGeometry:!1,light:structuredClone(e.light)})}#Ee(e){let t=this.#De(e.handle,`updateLight`);if(t.kind!==`light`||t.light===void 0)throw new $(`updateLight: handle ${e.handle} is not a light`);if(Uf(e.light,`updateLight.light`,e=>new $(e)),t.light.kind!==e.light.kind)throw new $(`updateLight: handle ${e.handle} cannot change kind from ${t.light.kind} to ${e.light.kind}`);zf(t.object,e.light,this.#b),t.light=structuredClone(e.light)}pickMesh(e){let t=this.#i.get(e);if(!t||t.meshProvenance===void 0)return;let n=Xm(t.object);return{handle:e,provenance:t.meshProvenance,sourceEntity:n.sourceEntity,sourceSceneNode:n.sourceSceneNode}}#De(e,t){let n=this.#i.get(e);if(n===void 0)throw new $(`${t}: unknown handle ${e}`);return n}}}));function dh(e,t,n,r,i){e.clear(!0,!0,!0),r.advanceAnimation(i),r.prepareSpritesForCamera(t,r.scene),r.prepareStaticInstanceBatches(t),e.render(r.scene,t),e.clearDepth(),r.prepareSpritesForCamera(n,r.viewmodelScene),e.render(r.viewmodelScene,n)}var fh=e((()=>{}));function ph(e,t){if(!Number.isSafeInteger(e)||e<1)throw RangeError(`${t} must be a positive safe integer`);return e}var mh,hh=e((()=>{mh=class{#e;#t;#n=!1;#r=[];constructor(e,t={}){this.#e=e,this.#t=ph(t.maximumPendingSubmissions??1,`maximum pending GPU submissions`)}ready(e=this.#t){let t=Math.min(this.#t,ph(e,`automatic pending GPU submission limit`));if(this.#e===null||this.#n)return!0;for(let e=this.#r.length-1;e>=0;--e){let t=this.#r[e];if(t===void 0)continue;let n;try{n=this.#e.poll(t)}catch{return this.#i(),!0}if(n===`failed`)return this.#i(),!0;n===`signaled`&&(this.#a(t),this.#r.splice(e,1))}return this.#r.length<t}submitted(){if(!(this.#e===null||this.#n))try{for(;this.#r.length>=this.#t;){let e=this.#r.shift();e!==void 0&&this.#a(e)}let e=this.#e.create();if(e===null){this.#n=!0;return}this.#r.push(e),this.#e.flush()}catch{this.#i()}}sample(){return Object.freeze({schemaVersion:1,mode:this.#e===null?`unsupported`:this.#n?`disabled`:`active`,maximumPendingSubmissions:this.#t,pendingSubmissionCount:this.#r.length})}dispose(){this.#i()}#i(){for(let e of this.#r)this.#a(e);this.#r.length=0,this.#n=!0}#a(e){if(this.#e!==null)try{this.#e.delete(e)}catch{}}}}));function gh(){return{now:()=>globalThis.performance?.now()??0}}function _h(e,t,n){return e===`accelerated`&&t!=null&&Number.isFinite(t)&&t>=0?t:n??0}function vh(e,t,n,r){return Object.freeze({schemaVersion:1,mode:e,state:t,rendererClass:n,timerDurationMs:null,completionAgeMs:null,completionAllowanceMs:r,effectiveDurationMs:null,targetDutyFraction:null,admittedAtMs:null,admissionObservedAtMs:null,observedAtMs:null})}function yh(e,t){return Object.freeze({...e,...t})}function bh(e,t){if(!Number.isSafeInteger(e)||e<1)throw RangeError(`${t} must be a positive safe integer`);return e}var xh,Sh,Ch,wh,Th,Eh,Dh=e((()=>{xh=8,Sh=17,Ch=.5,wh=100,Th=.2,Eh=class{#e;#t;#n;#r;#i;#a=null;#o=!1;#s=null;#c=0;#l=0;#u=[];#d;#f=!1;constructor(e,t={}){this.#n=e,this.#e=t.clock??e??gh(),this.#i=t.rendererClass??`unknown`,this.#r=bh(t.maximumPendingMeasurements??1,`maximum pending GPU measurements`),this.#t=this.#i===`software`?0:Sh,this.#d=vh(e===null?`completionOnly`:`timerQuery`,`idle`,this.#i,this.#t)}begin(e){if(!this.#o){for(this.#p();this.#u.length>=this.#r;)this.#h();if(this.#s=null,this.#d=yh(this.#d,{mode:this.#b(),state:this.#u.length===0?`idle`:`measuring`}),!(this.#n===null||this.#f))try{let t=this.#x(),n=_h(this.#i,e,t),r=this.#n.begin();r===null?this.#_():this.#a={query:r,deadlineOriginMs:n}}catch{this.#_()}}}submitted(){if(this.#o)return;let e=this.#x();if(e===null){this.#v();return}let t=this.#a,n=_h(this.#i,t?.deadlineOriginMs,e);if(this.#l=Math.max(this.#l,n+this.#c),this.#d=yh(this.#d,{mode:this.#b(),state:`measuring`}),this.#n===null||this.#f||t===null){this.#s=e;return}let{query:r}=t;this.#a=null;try{this.#n.end(r),this.#u.push({deadlineOriginMs:_h(this.#i,t.deadlineOriginMs,e),query:r,submittedAtMs:e})}catch{this.#g(r),this.#_(),this.#s=e}}aborted(){if(this.#n===null||this.#a===null)return;let{query:e}=this.#a;this.#a=null;try{this.#n.end(e)}catch{}this.#g(e)}ready(e){if(this.#o)return!0;let t=this.#x();if(t===null)return this.#v(),!0;for(let e=0;e<this.#u.length;){let n=this.#u[e];if(n===void 0){e+=1;continue}let r;if(this.#n===null)r={status:`failed`};else try{r=this.#n.poll(n.query)}catch{r={status:`failed`}}if(r.status===`pending`){e+=1;continue}if(r.status===`failed`||!Number.isFinite(r.durationMs)||r.durationMs<0){let e=Math.max(n.submittedAtMs,...this.#u.map(e=>e.submittedAtMs));this.#_(),this.#s=e;break}this.#u.splice(e,1),this.#g(n.query),this.#y(t,r.durationMs,n.deadlineOriginMs,n.submittedAtMs)}if(this.#s!==null){let e=this.#s;this.#s=null,this.#y(t,null,e,e)}let n=this.#b()===`timerQuery`?this.#r:1,r=this.#u.length<n,i=_h(this.#i,e,t),a=r&&i>=this.#l;return this.#d=yh(this.#d,{mode:this.#b(),state:a?`ready`:r?`waiting`:`measuring`,...a?{admissionObservedAtMs:t}:{}}),a}sample(){return Object.freeze({...this.#d,maximumPendingMeasurements:this.#b()===`timerQuery`?this.#r:1,pendingMeasurementCount:this.#u.length})}dispose(){this.#o||(this.#p(),this.#m(),this.#s=null,this.#c=0,this.#l=0,this.#o=!0,this.#d=yh(this.#d,{state:`disposed`}))}#p(){if(this.#n===null||this.#a===null)return;let{query:e}=this.#a;this.#a=null;try{this.#n.end(e)}catch{}this.#g(e)}#m(){for(let e of this.#u)this.#g(e.query);this.#u.length=0}#h(){let e=this.#u.shift();e!==void 0&&this.#g(e.query)}#g(e){if(this.#n!==null)try{this.#n.delete(e)}catch{this.#f=!0}}#_(){this.#p(),this.#m(),this.#f=!0,this.#d=yh(this.#d,{mode:`timerFailed`})}#v(){this.#p(),this.#m(),this.#s=null,this.#c=0,this.#l=0,this.#f=!0,this.#d=yh(this.#d,{mode:`timerFailed`,state:`ready`})}#y(e,t,n,r){let i=Math.max(0,e-r),a=Math.max(0,i-this.#t),o=this.#i===`accelerated`&&t!==null,s=o?t:Math.max(t??0,a),c=s*(1/Math.min(Ch,Math.max(Th,Ch*xh/Math.max(s,2**-52)))-1),l=s+Math.min(wh,Math.max(0,c-s)),u=s<=2**-52?Ch:s/(s+l);this.#c=s+l;let d=o?n:r;this.#l=Math.max(this.#l,d+this.#c),this.#d=Object.freeze({schemaVersion:1,mode:this.#b(),state:e>=this.#l?`ready`:`waiting`,rendererClass:this.#i,timerDurationMs:t,completionAgeMs:i,completionAllowanceMs:this.#t,effectiveDurationMs:s,targetDutyFraction:u,admittedAtMs:this.#l,admissionObservedAtMs:null,observedAtMs:e})}#b(){return this.#f?`timerFailed`:this.#n===null?`completionOnly`:`timerQuery`}#x(){try{let e=this.#e.now();return Number.isFinite(e)&&e>=0?e:null}catch{return null}}}}));function Oh(e){return typeof e!=`string`||e.length===0?`unknown`:/swiftshader|llvmpipe|software rasterizer|software renderer|microsoft basic render/iu.test(e)?`software`:`accelerated`}var kh=e((()=>{}));function Ah(e,t){return e===`accelerated`&&t?8:1}var jh=e((()=>{}));function Mh(e,t){if(!Number.isFinite(e)||e<=0)throw RangeError(`renderer pixel ratio must be finite and greater than zero`);return t===`software`?Math.min(e,Nh):e}var Nh,Ph=e((()=>{Nh=.25}));function Fh(e,t){e.position.set(...t.position),e.up.set(0,1,0),e.rotation.order=`YXZ`,e.rotation.x=t.pitchDegrees*Ih,e.rotation.y=-t.yawDegrees*Ih,e.rotation.z=0}var Ih,Lh=e((()=>{Ih=Math.PI/180}));function Rh(e){return zh(structuredClone(e))}function zh(e){if(typeof e!=`object`||!e||Object.isFrozen(e))return e;for(let t of Object.values(e))zh(t);return Object.freeze(e)}function Bh(e){let t=e.projection.kind===`perspective`?new pc(e.projection.fovYDegrees,1,e.projection.near,e.projection.far):new vc(-e.projection.verticalSize/2,e.projection.verticalSize/2,e.projection.verticalSize/2,-e.projection.verticalSize/2,e.projection.near,e.projection.far);return t.name=e.id,Fh(t,e.pose),t.updateMatrixWorld(!0),t}function Vh(e,t){if(e instanceof pc){e.aspect=t,e.updateProjectionMatrix();return}if(e instanceof vc){let n=e.top-e.bottom;e.left=-(n*t)/2,e.right=n*t/2,e.updateProjectionMatrix()}}function Hh(e){let t=e.sampling===`nearest`?Dn:An,n=new Oi(e.width,e.height,{depthBuffer:e.depth===`depth24`,generateMipmaps:!1,magFilter:t,minFilter:t,stencilBuffer:!1});return n.texture.colorSpace=Zr,n.texture.name=e.id,n}function Uh(e){let t=new Ms({depthTest:!1,depthWrite:!1,fragmentShader:`
      uniform sampler2D sourceTarget;
      varying vec2 sourceUv;
      void main() {
        gl_FragColor = texture2D(sourceTarget, sourceUv);
      }
    `,toneMapped:!1,uniforms:{sourceTarget:{value:e}},vertexShader:`
      varying vec2 sourceUv;
      void main() {
        sourceUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `}),n=new Ds(2,2),r=new Oo(n,t),i=new da;return i.add(r),{material:t,scene:i}}function Wh(e){for(let t of e.values()){for(let e of t.scene.children)e instanceof Oo&&e.geometry.dispose();t.material.dispose()}}function Gh(e,t){Wh(e.presentations);for(let n of e.createdTargets)t.get(n.descriptor.id)!==n&&n.target.dispose()}function Kh(e,t){return e.id===t.id&&e.revision===t.revision&&e.width===t.width&&e.height===t.height&&e.color===t.color&&e.depth===t.depth&&e.sampling===t.sampling}function qh(e,t){return e.order-t.order||e.id.localeCompare(t.id)}function Jh(e,t,n){let r=Math.round(e.x*t),i=Math.round(e.y*n);return{x:r,y:i,width:Math.max(1,Math.min(t-r,Math.round(e.width*t))),height:Math.max(1,Math.min(n-i,Math.round(e.height*n)))}}function Yh(e,t){let n=1/e.getPixelRatio();e.setViewport(t.x*n,t.y*n,t.width*n,t.height*n),e.setScissor(t.x*n,t.y*n,t.width*n,t.height*n)}function Xh(e){let t=e instanceof Error?e.message:String(e);return e instanceof eg?{code:`stale_target_revision`,message:t}:e instanceof tg?{code:`target_allocation_failed`,message:t}:{code:`invalid_view_composition`,message:t}}var Zh,Qh,$h,eg,tg,ng,rg=e((()=>{cf(),at(),Lh(),Zh=class extends Error{code=`invalid_view_composition`;constructor(e){super(e),this.name=`RendererViewCompositionPolicyError`}},Qh=Object.freeze({schemaVersion:1,cameras:Object.freeze([]),targets:Object.freeze([]),views:Object.freeze([]),presentations:Object.freeze([])}),$h=class{#e=new Map;#t;#n;#r=new Map;#i=Qh;#a=!1;#o=new Map;#s=0;#c=new Map;constructor(e,t){this.#n=e,this.#t=t}configure(e){if(this.#a)return this.#d(`surface_disposed`,`renderer view composition is disposed`);let t=null;try{let n=Rh(e);return We(n),this.#m(n),t=this.#l(n),this.#u(t),Object.freeze({applied:!0,diagnostics:Object.freeze([]),revision:this.#s})}catch(e){t!==null&&Gh(t,this.#c);let n=Xh(e);return this.#d(n.code,n.message)}}readout(){let e=this.#i.targets.map(e=>{let t=this.#c.get(e.id),n=t?.lastRefreshedSubmission??null;return Object.freeze({...e,lastRefreshedSubmission:n,status:n===null?`never_rendered`:t?.stale===!0?`stale`:`current`})});return Object.freeze({schemaVersion:1,revision:this.#s,cameras:this.#i.cameras,targets:Object.freeze(e),views:this.#i.views,presentations:this.#i.presentations,resources:Object.freeze({presentationCount:this.#o.size,targetCount:this.#c.size})})}visibilityReadout(){let e=this.#i.views.map(e=>{let t=this.#r.get(e.cameraId);return t===void 0?null:Object.freeze({viewId:e.id,cameraId:e.cameraId,target:e.target.kind,visibility:this.#t.visibilityReadout(t,this.#t.scene)})}).filter(e=>e!==null).sort((e,t)=>e.viewId.localeCompare(t.viewId));return Object.freeze({schemaVersion:1,views:Object.freeze(e)})}render(e,t,n){if(this.#a||this.#i.views.length===0)return;let r=this.#i.views.filter(e=>e.target.kind===`offscreen`).sort(qh);for(let t of r)this.#f(t,e);let i=[...this.#i.views.filter(e=>e.target.kind===`primary`).map(e=>({id:e.id,kind:`view`,order:e.order})),...this.#i.presentations.map(e=>({id:e.id,kind:`presentation`,order:e.order}))].sort(qh);this.#n.setRenderTarget(null),this.#n.setScissorTest(!0);try{for(let e of i)if(e.kind===`view`){let r=this.#i.views.find(t=>t.id===e.id);r!==void 0&&this.#p(r,t,n)}else{let r=this.#i.presentations.find(t=>t.id===e.id),i=this.#o.get(e.id);if(r!==void 0&&i!==void 0){let e=Jh(r.destination.viewport,t,n);Yh(this.#n,e),this.#n.clear(!1,!0,!1),this.#n.render(i.scene,ng)}}}finally{this.#n.setRenderTarget(null),this.#n.setScissorTest(!1),Yh(this.#n,{x:0,y:0,width:t,height:n})}}invalidate(){if(!this.#a)for(let e of this.#c.values())e.stale=!0}dispose(){if(!this.#a){Wh(this.#o);for(let e of this.#c.values())e.target.dispose();this.#r=new Map,this.#i=Qh,this.#o=new Map,this.#c=new Map,this.#a=!0}}#l(e){let t=new Map(e.cameras.map(e=>[e.id,Bh(e)])),n=new Map,r=[],i=new Map;try{for(let t of e.targets){let e=this.#c.get(t.id);if(e!==void 0&&e.descriptor.revision===t.revision&&Kh(e.descriptor,t)){n.set(t.id,e);continue}let i=Hh(t),a={descriptor:t,target:i,lastRefreshedSubmission:null,stale:!0};r.push(a),this.#n.initRenderTarget(i),n.set(t.id,a)}for(let t of e.presentations){let e=n.get(t.sourceTargetId);if(e===void 0)throw Error(`validated presentation source is missing`);i.set(t.id,Uh(e.target.texture))}return{cameras:t,composition:e,createdTargets:r,presentations:i,targets:n}}catch(e){Wh(i);for(let e of r)e.target.dispose();throw new tg(e instanceof Error?e.message:String(e))}}#u(e){let t=this.#c,n=this.#o;this.#r=e.cameras,this.#i=e.composition,this.#o=e.presentations,this.#c=e.targets,this.invalidate(),this.#s+=1;for(let t of e.composition.targets)this.#e.set(t.id,t.revision);Wh(n);for(let[n,r]of t)e.targets.get(n)!==r&&r.target.dispose()}#d(e,t){return Object.freeze({applied:!1,diagnostics:Object.freeze([Object.freeze({code:e,message:t})]),revision:this.#s})}#f(e,t){if(e.target.kind!==`offscreen`)return;let n=this.#c.get(e.target.targetId),r=this.#r.get(e.cameraId);if(n===void 0||r===void 0)return;let i=Jh(e.viewport,n.descriptor.width,n.descriptor.height);Vh(r,i.width/i.height),r.updateMatrixWorld(!0),this.#t.scene.updateMatrixWorld(!0),this.#n.setRenderTarget(n.target),this.#n.setScissorTest(!1),Yh(this.#n,i),this.#n.setScissorTest(!0),this.#n.clear(!0,!0,!0),this.#t.prepareSpritesForCamera(r,this.#t.scene),this.#t.prepareStaticInstanceBatches(r),this.#n.render(this.#t.scene,r),n.lastRefreshedSubmission=t,n.stale=!1}#p(e,t,n){let r=this.#r.get(e.cameraId);if(r===void 0)return;let i=Jh(e.viewport,t,n);Vh(r,i.width/i.height),Yh(this.#n,i),this.#n.clear(!0,!0,!0),this.#t.prepareSpritesForCamera(r,this.#t.scene),this.#t.prepareStaticInstanceBatches(r),this.#n.render(this.#t.scene,r)}#m(e){for(let t of e.targets){let e=this.#c.get(t.id)?.descriptor,n=this.#e.get(t.id);if(e!==void 0&&t.revision===e.revision){if(!Kh(e,t))throw new eg(`${t.id} revision ${String(t.revision)} cannot change target facts`);continue}if(n!==void 0&&t.revision<=n)throw new eg(`${t.id} revision must be greater than ${String(n)}`)}}},eg=class extends Error{},tg=class extends Error{},ng=new vc(-1,1,1,-1,.1,10),ng.position.z=1,ng.updateMatrixWorld(!0)}));function ig(e,t,n,r,i,a=0,o=0){let s=e>0?e:a>0?a:Number.isFinite(n)?n/i:0,c=t>0?t:o>0?o:Number.isFinite(r)?r/i:0;return{width:Math.max(1,Math.round(s)||800),height:Math.max(1,Math.round(c)||450)}}function ag(e,t={}){let n=og(t.lighting),r=new lh({...t.animatedMeshSource===void 0?{}:{animatedMeshSource:t.animatedMeshSource},...t.meshBufferSource===void 0?{}:{meshBufferSource:t.meshBufferSource},...t.meshResourceSource===void 0?{}:{meshResourceSource:t.meshResourceSource},...t.textureResourceSource===void 0?{}:{textureResourceSource:t.textureResourceSource},shadowsEnabled:n.shadows.enabled,maximumActiveShadowLights:n.shadows.maximumActiveLights}),i=n.defaultLights.world===`neutral`?sg([5,8,6]):[];i.length>0&&r.scene.add(...i);let a=n.defaultLights.viewmodel===`neutral`?sg([2,3,2]):[];a.length>0&&r.viewmodelScene.add(...a);let o=t.frame??pg();try{r.applyFrame(o)}catch(e){throw r.dispose(),e}let s=new sf({canvas:e,antialias:!0});s.shadowMap.enabled=n.shadows.enabled;let c=s.getContext(),l=ug(c),u=cg(c),d=lg(c),f=Ah(l,d!==null),p=new mh(u,{maximumPendingSubmissions:f}),m=new Eh(d,{maximumPendingMeasurements:f,rendererClass:l});s.autoClear=!1,s.info.autoReset=!1,s.setClearColor(t.clearColor??1054752,1);let h=t.pixelRatio??globalThis.devicePixelRatio??1,g=Mh(h,l);s.setPixelRatio(g);let _=fg(t.camera?.projection??{fovYDegrees:55,near:.1,far:100}),v=new pc(_.fovYDegrees,1,_.near,_.far);v.name=`world-camera`;let y=new pc(_.fovYDegrees,1,_.near,_.far);y.name=`viewmodel-camera`;let b=new Uc,x=new fi(0,0),S=new K,C=t.camera?.initialPose??{position:[0,1.62,8],pitchDegrees:0,yawDegrees:0},w=t.camera?.initialBasis??null,T=null,E=null,D=null,O={width:0,height:0},ee=0,k=!1,te=new $h(s,r);if(t.viewComposition!==void 0){let e=te.configure(t.viewComposition);if(!e.applied)throw te.dispose(),s.dispose(),r.dispose(),new Zh(e.diagnostics[0]?.message??`view composition was rejected`)}let ne=(e,t)=>{if(C=e,w=t??null,w===null){Fh(v,e);return}v.position.set(e.position[0],e.position[1],e.position[2]),v.up.set(w.up[0],w.up[1],w.up[2]),S.set(v.position.x+w.forward[0],v.position.y+w.forward[1],v.position.z+w.forward[2]),v.lookAt(S)},re=()=>{let{width:t,height:n}=ig(e.clientWidth,e.clientHeight,e.width,e.height,h,O.width,O.height);(O.width!==t||O.height!==n)&&(s.setSize(t,n,!1),O={width:t,height:n}),v.aspect=t/n,v.updateProjectionMatrix(),y.aspect=t/n,y.updateProjectionMatrix()},ie=(t=globalThis.performance?.now()??0)=>{if(k)throw Error(`renderer browser surface is disposed`);let n=E;E=null,re();let i=D===null?0:Math.min(.05,Math.max(0,(t-D)/1e3));D=t,s.info.reset(),m.begin(n??void 0);try{dh(s,v,y,r,i),ee+=1,te.render(ee,e.width,e.height)}catch(e){throw m.aborted(),e}return m.submitted(),p.submitted(),Object.freeze({schemaVersion:1,drawCallCount:s.info.render.calls,triangleCount:s.info.render.triangles,...r.resourceStatistics()})},ae=e=>{E=null;let t=m.ready(e),n=p.ready(m.sample().mode===`timerQuery`?f:1)&&t;return n&&e!==void 0&&Number.isFinite(e)&&e>=0&&(E=e),n},oe=e=>(re(),v.updateMatrixWorld(!0),dg(v,O,e)),se=e=>{T=globalThis.requestAnimationFrame(se),ae(e)&&ie(e)},ce=()=>{if(k)throw Error(`renderer browser surface is disposed`);T===null&&(T=globalThis.requestAnimationFrame(se))},le=()=>{E=null,T!==null&&(globalThis.cancelAnimationFrame(T),T=null)};return ne(C,w??void 0),ie(0),t.autoStart!==!1&&ce(),{kind:`rusty_renderer_browser_surface.v1`,canvas:e,renderer:r,frame:o,automaticSubmissionPacing:()=>{let e=m.sample(),t=p.sample();return Object.freeze({...e,automaticSubmissionCapacity:f,automaticSubmissionLimit:e.maximumPendingMeasurements,completionFenceMode:t.mode,maximumPendingSubmissions:t.maximumPendingSubmissions,pendingSubmissionCount:t.pendingSubmissionCount})},automaticSubmissionReady:ae,animatedMeshPlayback:e=>r.animatedMeshPlayback(e),sampleAnimatedMesh:(e,t,n)=>r.sampleAnimatedMesh(e,t,n),applyFrame:e=>{r.applyFrame(e),te.invalidate()},configureViews:e=>te.configure(e),cameraPose:()=>C,cameraProjection:()=>_,lightingReadout:()=>{let e=r.lightReadout();return Object.freeze({schemaVersion:1,defaultLights:Object.freeze({...n.defaultLights}),neutralLightCounts:Object.freeze({world:i.length,viewmodel:a.length}),shadows:Object.freeze({enabled:n.shadows.enabled,maximumActiveLights:n.shadows.maximumActiveLights,activeLights:e.filter(e=>e.shadowStatus===`active`).length,requestedUnsupportedLights:e.filter(e=>e.shadowStatus===`requested_unsupported`).length}),retainedLights:e})},visibilityReadout:()=>Object.freeze({schemaVersion:1,world:r.visibilityReadout(v,r.scene),viewmodel:r.visibilityReadout(y,r.viewmodelScene),views:te.visibilityReadout().views}),viewCompositionReadout:()=>te.readout(),projectWorldPoint:oe,pick:e=>mg(r,v,b,x,e),snapshot:()=>r.snapshot(),renderOnce:ie,setCameraPose:ne,start:ce,stop:le,dispose:()=>{k||=(le(),p.dispose(),m.dispose(),te.dispose(),s.dispose(),r.dispose(),!0)}}}function og(e){let t=e??{schemaVersion:1,defaultLights:{world:`neutral`,viewmodel:`neutral`},shadows:{enabled:!1,maximumActiveLights:8}};if(t.schemaVersion!==1)throw new Kf(`invalid_shadow_limit`,`lighting.schemaVersion must equal 1`);for(let[e,n]of Object.entries(t.defaultLights))if(n!==`neutral`&&n!==`disabled`)throw new Kf(`invalid_shadow_limit`,`lighting.defaultLights.${e} must be neutral or disabled`);let n=t.shadows.maximumActiveLights;if(!Number.isSafeInteger(n)||n<0||n>8)throw new Kf(`invalid_shadow_limit`,`lighting.shadows.maximumActiveLights must be in 0..=8`);return t}function sg(e){let t=new tc(16777215,2503224,2.4),n=new bc(16777215,2.2);return n.position.set(...e),[t,n]}function cg(e){if(!(`fenceSync`in e))return null;let t=e;return{create:()=>t.fenceSync(t.SYNC_GPU_COMMANDS_COMPLETE,0),delete:e=>t.deleteSync(e),flush:()=>t.flush(),poll:e=>{let n=t.clientWaitSync(e,0,0);return n===t.TIMEOUT_EXPIRED?`pending`:n===t.ALREADY_SIGNALED||n===t.CONDITION_SATISFIED?`signaled`:`failed`}}}function lg(e){if(!(`createQuery`in e))return null;let t=e,n=t.getExtension(`EXT_disjoint_timer_query_webgl2`);return n===null?null:{begin:()=>{let e=t.createQuery();return e===null?null:(t.beginQuery(n.TIME_ELAPSED_EXT,e),e)},delete:e=>t.deleteQuery(e),end:()=>t.endQuery(n.TIME_ELAPSED_EXT),now:()=>globalThis.performance?.now()??0,poll:e=>{if(t.getParameter(n.GPU_DISJOINT_EXT)===!0)return{status:`failed`};if(t.getQueryParameter(e,t.QUERY_RESULT_AVAILABLE)!==!0)return{status:`pending`};let r=t.getQueryParameter(e,t.QUERY_RESULT);return typeof r==`number`?{durationMs:r/1e6,status:`complete`}:{status:`failed`}}}}function ug(e){let t;try{let n=e.getExtension(`WEBGL_debug_renderer_info`);if(n===null)return`unknown`;t=e.getParameter(n.UNMASKED_RENDERER_WEBGL)}catch{return`unknown`}return Oh(t)}function dg(e,t,n){let r=new K(...n).project(e),i=e.position.distanceTo(new K(...n)),a=r.x>=-1&&r.x<=1&&r.y>=-1&&r.y<=1&&r.z>=-1&&r.z<=1;return{xPixels:(r.x+1)/2*t.width,yPixels:(1-r.y)/2*t.height,depth:Math.max(0,Math.min(1,(r.z+1)/2)),distance:i,insideViewport:a,occluded:!1}}function fg(e){if(![e.fovYDegrees,e.near,e.far].every(Number.isFinite)||e.fovYDegrees<=0||e.fovYDegrees>=180||e.near<=0||e.far<=e.near)throw RangeError(`camera projection must have a finite FOV in (0, 180) and 0 < near < far`);return{fovYDegrees:e.fovYDegrees,near:e.near,far:e.far}}function pg(){let e=vg();return{schemaVersion:1,ops:[{op:`create`,handle:n(4103001),parent:null,node:yg(`rusty-renderer-flat-plane`,`cube`,[0,-.08,0],[18,.16,18],[.16,.22,.2,1])},{op:`create`,handle:n(4103002),parent:null,node:yg(`rusty-renderer-collision-wall-north`,`cube`,[0,.5,-2.5],[6,3,1],[.32,.38,.42,1])},{op:`create`,handle:n(4103003),parent:null,node:yg(`rusty-renderer-collision-wall-south`,`cube`,[0,.5,2.5],[6,3,1],[.32,.38,.42,1])},{op:`create`,handle:n(4103004),parent:null,node:yg(`rusty-renderer-collision-wall-west`,`cube`,[-2.5,.5,0],[1,3,6],[.27,.34,.37,1])},{op:`create`,handle:n(4103005),parent:null,node:yg(`rusty-renderer-collision-wall-east`,`cube`,[2.5,.5,0],[1,3,6],[.27,.34,.37,1])},...e.map((e,t)=>({op:`create`,handle:n(4103100+t),parent:null,node:yg(`rusty-renderer-random-cube-${String(t+1).padStart(2,`0`)}`,`cube`,[e.position[0],e.size[1]/2,e.position[1]],e.size,e.color)}))]}}function mg(e,t,n,r,i){let a=gg(i);if(a.length>0)return{diagnostics:a,hit:null,kind:`rusty_renderer_browser_surface_pick.v1`};e.prepareSpritesForCamera(t,e.scene),e.prepareStaticInstanceBatchesForPicking(),e.scene.updateMatrixWorld(!0),hg(n,t,r,i.ray),n.far=i.maxDistance??1/0;let o=n.intersectObjects(e.scene.children,!0);for(let t of o){let n=e.projectionIdentityForObject(t.object,t.instanceId);if(n===void 0||!_g(n,i.filter))continue;let r=t.face?.normal.clone()??new K(0,0,0);return t.face!==null&&t.face!==void 0&&r.copy(e.projectionWorldNormalForObject(t.object,t.instanceId,t.face.normal)),{diagnostics:[],hit:{channel:`render_projection`,distance:Number(t.distance.toFixed(4)),handle:n.handle,label:n.metadata.label,layer:n.layer,normal:[r.x,r.y,r.z],position:[t.point.x,t.point.y,t.point.z],sourceTrace:n.metadata.sourceEntity===null?null:{entity:n.metadata.sourceEntity,kind:`render_metadata_entity`},tags:[...n.metadata.tags]},kind:`rusty_renderer_browser_surface_pick.v1`}}return{diagnostics:[],hit:null,kind:`rusty_renderer_browser_surface_pick.v1`}}function hg(e,t,n,r){if(r.kind===`viewport`){n.set(r.point[0],r.point[1]),e.setFromCamera(n,t);return}e.set(new K(...r.origin),new K(...r.direction).normalize())}function gg(e){if(e.maxDistance!==void 0&&(!Number.isFinite(e.maxDistance)||e.maxDistance<=0))return[{code:`invalid_max_distance`,message:`maxDistance must be finite and greater than zero`}];if([e.filter?.handles?.length??0,e.filter?.labels?.length??0,e.filter?.layers?.length??0,e.filter?.tags?.length??0].some(e=>e>Cg))return[{code:`filter_limit_exceeded`,message:`pick filters may contain at most ${Cg} values`}];if(e.ray.kind===`viewport`){let[t,n]=e.ray.point;return![t,n].every(Number.isFinite)||t<-1||t>1||n<-1||n>1?[{code:`invalid_viewport_point`,message:`viewport coordinates must be finite and within [-1, 1]`}]:[]}let t=[...e.ray.origin,...e.ray.direction],n=Math.hypot(...e.ray.direction);return!t.every(Number.isFinite)||n===0?[{code:`invalid_world_ray`,message:`world ray values must be finite and direction must be non-zero`}]:[]}function _g(e,t){return t===void 0||!(t.handles!==void 0&&!t.handles.includes(e.handle)||t.labels!==void 0&&(e.metadata.label===null||!t.labels.includes(e.metadata.label))||t.layers!==void 0&&!t.layers.includes(e.layer)||t.tags!==void 0&&!t.tags.every(t=>e.metadata.tags.some(e=>e===t)))}function vg(){let e=xg(1090765022),t=[[.28,.66,.92,1],[.92,.54,.32,1],[.46,.78,.42,1],[.82,.58,.92,1],[.92,.76,.28,1]],n=[{color:t[0],position:[0,-1.35],size:[.62,2.2,.62]},{color:t[1],position:[1.25,-.65],size:[.48,.85,.48]},{color:t[2],position:[-1.15,-.9],size:[.52,1.05,.52]},{color:t[3],position:[.85,1.1],size:[.44,.75,.44]}];for(let r=n.length;r<28;r+=1){let i=Sg(.55+e()*1.55),a=Sg(.65+e()*2.8),o=Sg(.55+e()*1.55),s=Sg(-7+e()*14),c=Sg(-7+e()*14);s>-3.5&&s<3.5&&c>-3.5&&c<3.5&&(c=Sg(c<0?c-3.75:c+3.75)),n.push({color:t[r%t.length],position:[s,c],size:[i,a,o]})}return n}function yg(e,t,n,r,i){return{geometry:{kind:t},material:{color:i,wireframe:!1},transform:bg(n,r),visible:!0,layer:`scene`,metadata:{sourceEntity:null,sourceSceneNode:null,tags:[],label:e}}}function bg(e,t){return{translation:e,rotation:[0,0,0,1],scale:t}}function xg(e){let t=e>>>0;return()=>(t=Math.imul(t,1664525)+1013904223>>>0,t/4294967296)}function Sg(e){return Number(e.toFixed(2))}var Cg,wg=e((()=>{cf(),It(),at(),uh(),qf(),fh(),hh(),Dh(),kh(),jh(),Ph(),Lh(),rg(),Cg=128})),Tg=e((()=>{wg(),uh()})),Eg=e((()=>{uh(),qf(),Xf(),Lf(),wg(),fh(),Tg(),um()}));async function Dg(e,t){if(/^[0-9a-f]{16}$/u.test(t))return Ag(e);let n=t.startsWith(`sha256:`);if(!/^(?:sha256:)?[0-9a-f]{64}$/u.test(t))throw Error(`unsupported renderer resource content hash ${t}`);let r=Og(e);return n?`sha256:${r}`:r}function Og(e){let t=new Uint8Array(e),n=Math.ceil((t.byteLength+9)/64)*64,r=new Uint8Array(n);r.set(t),r[t.byteLength]=128;let i=BigInt(t.byteLength)*8n;for(let e=0;e<8;e+=1)r[n-1-e]=Number(i>>BigInt(e*8)&255n);let a=jg[0],o=jg[1],s=jg[2],c=jg[3],l=jg[4],u=jg[5],d=jg[6],f=jg[7],p=new Uint32Array(64);for(let e=0;e<r.byteLength;e+=64){for(let t=0;t<16;t+=1){let n=e+t*4;p[t]=(r[n]<<24|r[n+1]<<16|r[n+2]<<8|r[n+3])>>>0}for(let e=16;e<p.length;e+=1){let t=p[e-15],n=p[e-2],r=kg(t,7)^kg(t,18)^t>>>3,i=kg(n,17)^kg(n,19)^n>>>10;p[e]=p[e-16]+r+p[e-7]+i>>>0}let t=a,n=o,i=s,m=c,h=l,g=u,_=d,v=f;for(let e=0;e<p.length;e+=1){let r=kg(h,6)^kg(h,11)^kg(h,25),a=h&g^~h&_,o=v+r+a+Mg[e]+p[e]>>>0,s=(kg(t,2)^kg(t,13)^kg(t,22))+(t&n^t&i^n&i)>>>0;v=_,_=g,g=h,h=m+o>>>0,m=i,i=n,n=t,t=o+s>>>0}a=a+t>>>0,o=o+n>>>0,s=s+i>>>0,c=c+m>>>0,l=l+h>>>0,u=u+g>>>0,d=d+_>>>0,f=f+v>>>0}return[a,o,s,c,l,u,d,f].map(e=>e.toString(16).padStart(8,`0`)).join(``)}function kg(e,t){return e>>>t|e<<32-t}function Ag(e){let t=14695981039346656037n;for(let n of new Uint8Array(e))t^=BigInt(n),t=BigInt.asUintN(64,t*1099511628211n);return t.toString(16).padStart(16,`0`)}var jg,Mg,Ng=e((()=>{jg=[1779033703,3144134277,1013904242,2773480762,1359893119,2600822924,528734635,1541459225],Mg=[1116352408,1899447441,3049323471,3921009573,961987163,1508970993,2453635748,2870763221,3624381080,310598401,607225278,1426881987,1925078388,2162078206,2614888103,3248222580,3835390401,4022224774,264347078,604807628,770255983,1249150122,1555081692,1996064986,2554220882,2821834349,2952996808,3210313671,3336571891,3584528711,113926993,338241895,666307205,773529912,1294757372,1396182291,1695183700,1986661051,2177026350,2456956037,2730485921,2820302411,3259730800,3345764771,3516065817,3600352804,4094571909,275423344,430227734,506948616,659060556,883997877,958139571,1322822218,1537002063,1747873779,1955562222,2024104815,2227730452,2361852424,2428436474,2756734187,3204031479,3329325298]}));function Pg(e,t,n=null){return t===void 0?{handle:e,asset:null,contentHash:null,status:`unavailable`,selectedClip:null,mixerTimeSeconds:0,actionTimeSeconds:null,commandSelected:!1,running:!1,paused:!1,loop:null,speed:null,weight:null,poseSample:null,diagnostics:[Ig(`animated_mesh_handle_unavailable`,null,e,`animated mesh handle ${e} is unavailable`)],projectionOnly:!0,controllerClips:[]}:{handle:e,asset:t.asset,contentHash:n,status:t.status,selectedClip:t.currentClip,mixerTimeSeconds:t.mixerTimeSeconds,actionTimeSeconds:t.actionTimeSeconds,commandSelected:t.commandSelected,running:t.running,paused:t.paused,loop:t.loop,speed:t.speed,weight:t.weight,poseSample:t.poseSample,diagnostics:t.diagnostics.map(n=>Ig(Fg(n),t.asset,e,n)),projectionOnly:!0,controllerClips:t.controllerClips}}function Fg(e){switch(e){case`animation_not_started`:case`animation_paused`:case`animation_stopped`:return e;default:return`animated_mesh_frame_rejected`}}function Ig(e,t,n,r){return{code:e,message:r,asset:t,handle:n}}var Lg=e((()=>{Eg()}));async function Rg(e,t){zg(e);let n=await Promise.all(e.resources.map(async e=>{let n;try{n=await t(e)}catch(t){throw Bg(`mesh_resource_unavailable`,e.resource,t)}let r=n.slice(0);if(r.byteLength!==e.byteLength)throw Bg(`mesh_resource_byte_length_mismatch`,e.resource,`expected ${String(e.byteLength)} bytes, received ${String(r.byteLength)}`);let i=await Dg(r,e.contentHash);if(i!==e.contentHash)throw Bg(`mesh_resource_content_hash_mismatch`,e.resource,`expected ${e.contentHash}, received ${i}`);return[e.resource,{descriptor:e,bytes:new Uint8Array(r)}]})),r=new Map(n);return{acquireResource:(e,t,n)=>{let i=r.get(e);if(i===void 0)throw Bg(`mesh_resource_unavailable`,e,`resource was not preloaded`);if(i.descriptor.contentHash!==t||i.descriptor.byteLength!==n)throw Bg(`mesh_resource_manifest_invalid`,e,`retained descriptor does not match the admitted resource manifest`);return{bytes:i.bytes}},releaseResource:()=>{}}}function zg(e){if(e.kind!==`rusty_renderer_mesh_resources.v1`||e.resources.length===0||e.resources.length>1024)throw Bg(`mesh_resource_manifest_invalid`,null,`mesh resource manifest is empty, oversized, or unsupported`);let t=new Set,n=0;for(let r of e.resources){let e=/^sha256:([0-9a-f]{64})$/u.exec(r.contentHash)?.[1];if(e===void 0||r.resource!==`mesh-resource/${e}`||!Number.isSafeInteger(r.byteLength)||r.byteLength<16||r.byteLength>67108864||t.has(r.resource))throw Bg(`mesh_resource_manifest_invalid`,r.resource||null,`mesh resource descriptor is invalid or duplicated`);if(t.add(r.resource),n+=r.byteLength,n>268435456)throw Bg(`mesh_resource_manifest_invalid`,r.resource,`mesh resource manifest exceeds the aggregate byte bound`)}}function Bg(e,t,n){return new Vg(e,t,n instanceof Error?n.message:String(n))}var Vg,Hg=e((()=>{Ng(),Vg=class extends Error{code;resource;constructor(e,t,n){super(n),this.code=e,this.resource=t,this.name=`RendererMeshResourceError`}}}));async function Ug(e,t){Wg(e);let n=await Promise.all(e.resources.map(async e=>{let n;try{n=await t(e)}catch(t){throw Gg(`texture_resource_unavailable`,e.resource,t)}let r=n.slice(0);if(r.byteLength!==e.byteLength)throw Gg(`texture_resource_byte_length_mismatch`,e.resource,`expected ${String(e.byteLength)} bytes, received ${String(r.byteLength)}`);let i=await Dg(r,e.contentHash);if(i!==e.contentHash)throw Gg(`texture_resource_content_hash_mismatch`,e.resource,`expected ${e.contentHash}, received ${i}`);return[e.resource,{descriptor:e,bytes:new Uint8Array(r)}]})),r=new Map(n);return{acquireResource:(e,t,n)=>{let i=r.get(e);if(i===void 0)throw Gg(`texture_resource_unavailable`,e,`resource was not preloaded`);if(i.descriptor.contentHash!==t||i.descriptor.byteLength!==n)throw Gg(`texture_resource_manifest_invalid`,e,`retained descriptor does not match the admitted resource manifest`);return{bytes:i.bytes}},releaseResource:()=>{}}}function Wg(e){if(e.kind!==`rusty_renderer_texture_resources.v1`||e.resources.length===0||e.resources.length>256)throw Gg(`texture_resource_manifest_invalid`,null,`texture resource manifest is empty, oversized, or unsupported`);let t=new Set,n=0;for(let r of e.resources){let e=/^sha256:([0-9a-f]{64})$/u.exec(r.contentHash)?.[1];if(e===void 0||r.resource!==`texture-resource/${e}`||!Number.isSafeInteger(r.byteLength)||r.byteLength<=0||r.byteLength>16777216||t.has(r.resource))throw Gg(`texture_resource_manifest_invalid`,r.resource||null,`texture resource descriptor is invalid or duplicated`);if(t.add(r.resource),n+=r.byteLength,n>134217728)throw Gg(`texture_resource_manifest_invalid`,r.resource,`texture resource manifest exceeds the aggregate byte bound`)}}function Gg(e,t,n){return new Kg(e,t,n instanceof Error?n.message:String(n))}var Kg,qg=e((()=>{Ng(),Kg=class extends Error{code;resource;constructor(e,t,n){super(n),this.code=e,this.resource=t,this.name=`RendererTextureResourceError`}}}));Lg(),Hg(),qg(),at();var Jg=class{#e;constructor(e){this.#e={...e}}async apply(e){s(e);let t=[];for(let n of Yg){let r=e.ops.filter(e=>e.domain===n),i=this.#e[n];if(i===void 0){let e=r.map(e=>Zg(e));t.push({domain:n,configured:!1,requested:r.length,applied:0,diagnostics:e});continue}if(r.length===0){t.push({domain:n,configured:!0,requested:0,applied:0,diagnostics:[]});continue}let a=await i.applyPresentation({schemaVersion:1,ops:r});t.push({domain:n,configured:!0,requested:r.length,applied:a.applied,diagnostics:a.diagnostics.map(e=>({domain:n,...e}))})}return $g(t)}advance(e){if(!Number.isFinite(e)||e<0)throw RangeError(`presentation deltaSeconds must be finite and non-negative`);let t=[],n=[],r=0;for(let i of Xg){let a=this.#e[i];if(a===void 0)continue;let o=a.advance(e);t.push(i),r+=o.applied,n.push(...o.diagnostics.map(e=>({domain:i,...e})))}return{schemaVersion:1,advancedDomains:t,applied:r,diagnostics:n}}requiresAnimationFrame(){return Xg.some(e=>{let t=this.#e[e];return t!==void 0&&(t.requiresAnimationFrame?.()??!0)})}},Yg=[`animation`,`audio`,`billboard`,`particle`,`telemetryOverlay`],Xg=[`animation`,`particle`];function Zg(e){return{domain:e.domain,code:`unavailableHost`,sequence:e.meta.sequence,handle:Qg(e),message:`${e.domain} presentation was requested without a configured host`}}function Qg(e){let t=e.op;return`handle`in t?t.handle:null}function $g(e){return{schemaVersion:1,applied:e.reduce((e,t)=>e+t.applied,0),domains:e,diagnostics:e.flatMap(e=>e.diagnostics)}}var e_=class{#e=null;#t=0;#n=null;record(e){if(t_(e.sourceTimeMs),this.#t===2**53-1)throw Error(`renderer surface timing sequence is exhausted`);let t=n_(this.#e,e.sourceTimeMs),n=r_(e.backendSubmissionStartedMs,e.backendSubmissionEndedMs),r=Object.freeze({schemaVersion:1,renderSequence:this.#t+1,source:e.source,sourceTimeMs:e.sourceTimeMs,frameIntervalMs:t.value,frameIntervalStatus:t.status,backendSubmissionDurationMs:n.value,backendSubmissionDurationStatus:n.status});return this.#e=e.sourceTimeMs,this.#t=r.renderSequence,this.#n=r,r}read(){if(this.#n===null)throw Error(`renderer surface has not submitted a frame`);return this.#n}};function t_(e){if(!Number.isFinite(e)||e<0||e>2**53-1)throw Error(`renderer surface source time must be finite and in 0..=Number.MAX_SAFE_INTEGER`)}function n_(e,t){if(e===null)return{value:null,status:`firstFrame`};let n=t-e;return n<0?{value:null,status:`sourceTimeRegressed`}:n>6e4?{value:null,status:`sourceTimeGapExceeded`}:{value:n,status:`available`}}function r_(e,t){if(!Number.isFinite(e)||!Number.isFinite(t)||e<0||t<0)return{value:null,status:`clockUnavailable`};let n=t-e;return n<0?{value:null,status:`clockRegressed`}:n>6e4?{value:null,status:`durationExceeded`}:{value:n,status:`available`}}function i_(e,t){return Object.freeze({...e,statistics:a_(t)})}function a_(e){return Object.freeze({schemaVersion:1,drawCallCount:l_(`perSubmission`,e.drawCallCount),renderHandleCount:l_(`liveResident`,e.renderHandleCount),geometryResourceCount:l_(`liveResident`,e.geometryResourceCount),materialResourceCount:l_(`liveResident`,e.materialResourceCount),textureResourceCount:l_(`liveResident`,e.textureResourceCount),animatedInstanceCount:l_(`liveResident`,e.animatedInstanceCount),triangleCount:l_(`perSubmission`,e.triangleCount)})}function o_(e){let t=c_(e,`renderer surface statistics`),n=Object.keys(t);if(n.length!==d_.size||n.some(e=>!d_.has(e)))throw Error(`renderer surface statistics must have the complete supported shape`);if(t.schemaVersion!==1)throw Error(`renderer surface statistics schemaVersion must be 1`);for(let[e,n]of Object.entries(u_))s_(t[e],n,e)}function s_(e,t,n){let r=c_(e,`renderer surface statistic ${n}`),i=Object.keys(r);if(i.length!==3||!i.includes(`scope`)||!i.includes(`status`)||!i.includes(`value`))throw Error(`renderer surface statistic ${n} must have scope, status, and value`);if(r.scope!==t)throw Error(`renderer surface statistic ${n} must use ${t} scope`);if(r.status===`available`){if(!Number.isSafeInteger(r.value)||r.value<0)throw Error(`renderer surface statistic ${n} available value must be a non-negative safe integer`);return}if(r.status!==`unavailable`&&r.status!==`unsupported`)throw Error(`renderer surface statistic ${n} status is unsupported`);if(r.value!==null)throw Error(`renderer surface statistic ${n} ${String(r.status)} value must be null`)}function c_(e,t){if(typeof e!=`object`||!e||Array.isArray(e))throw Error(`${t} must be an object`);return e}function l_(e,t){return Object.freeze(t===void 0?{scope:e,status:`unsupported`,value:null}:t===null||!Number.isSafeInteger(t)||t<0?{scope:e,status:`unavailable`,value:null}:{scope:e,status:`available`,value:t})}var u_,d_,f_=e((()=>{u_={drawCallCount:`perSubmission`,renderHandleCount:`liveResident`,geometryResourceCount:`liveResident`,materialResourceCount:`liveResident`,textureResourceCount:`liveResident`,animatedInstanceCount:`liveResident`,triangleCount:`perSubmission`},d_=new Set([`schemaVersion`,...Object.keys(u_)])}));f_();var p_=class{#e=0;#t=0;#n=0;#r=0;#i=[];record(e,t,n,r,i){switch(this.#t+=1,t){case`admitted`:this.#e+=1;break;case`backendBlocked`:this.#n+=1;break;case`noDemand`:this.#r+=1;break}let a=Object.freeze({schemaVersion:1,sequence:this.#t,sourceTimeMs:e,outcome:t,demand:n,callback:Object.freeze({...i}),backend:Object.freeze({mode:r.mode,state:r.state,rendererClass:r.rendererClass,timerDurationMs:r.timerDurationMs,effectiveDurationMs:r.effectiveDurationMs,admittedAtMs:r.admittedAtMs,admissionObservedAtMs:r.admissionObservedAtMs,observedAtMs:r.observedAtMs,automaticSubmissionLimit:r.automaticSubmissionLimit,pendingMeasurementCount:r.pendingMeasurementCount,completionFenceMode:r.completionFenceMode,maximumPendingSubmissions:r.maximumPendingSubmissions,pendingSubmissionCount:r.pendingSubmissionCount})});this.#i.push(a),this.#i.length>64&&this.#i.shift()}sample(){return Object.freeze({schemaVersion:1,attemptCount:this.#t,admittedCount:this.#e,backendBlockedCount:this.#n,noDemandCount:this.#r,recentAttempts:Object.freeze([...this.#i])})}},m_=class{#e=!1;#t;constructor(e){this.#t=e}request(){this.#e=!0}consume(e,t){return this.consumeDecision(e,t).shouldSubmit}consumeDecision(e,t){let n=!h_(this.#t,e);this.#t=e;let r=this.#e,i=r||n||t.controls||t.presentation||t.retainedAnimation;return this.#e=!1,Object.freeze({schemaVersion:1,requested:r,viewportChanged:n,controls:t.controls,presentation:t.presentation,retainedAnimation:t.retainedAnimation,shouldSubmit:i})}submitted(e){this.#t=e,this.#e=!1}};function h_(e,t){return e.bufferHeight===t.bufferHeight&&e.bufferWidth===t.bufferWidth&&e.clientHeight===t.clientHeight&&e.clientWidth===t.clientWidth}It(),Eg();var g_=class extends Error{code=`invalid_lighting_policy`;constructor(e){super(e),this.name=`RendererSurfaceLightingError`}},__={family:`threejs`,implementation:`rusty-engine-renderer-backend`,publicContract:`rusty-renderer-surface.v1`};function v_(){return pg()}function y_(e){let t=e;return t.meshResourceManifest!==void 0||t.resolveMeshResource!==void 0||t.textureResourceManifest!==void 0||t.resolveTextureResource!==void 0}async function b_(e){if(e.meshResourceManifest===void 0!=(e.resolveMeshResource===void 0))throw Error(`meshResourceManifest requires an explicit resource resolver`);if(e.textureResourceManifest===void 0!=(e.resolveTextureResource===void 0))throw Error(`textureResourceManifest requires an explicit resource resolver`);let t=e.meshResourceManifest===void 0?void 0:await Rg(e.meshResourceManifest,e.resolveMeshResource),n=e.textureResourceManifest===void 0?void 0:await Ug(e.textureResourceManifest,e.resolveTextureResource);return{...t===void 0?{}:{meshResourceSource:t},...n===void 0?{}:{textureResourceSource:n}}}function x_(e,t={}){return y_(t)?S_(e,t):C_(e,t)}async function S_(e,t){return C_(e,t,await b_(t))}function C_(e,t,n={}){let r=R_(t.lighting),i=t.frame??v_(),a=new Pt;a.applyFrame(i);let o=O_(e,t.controls),s;try{s=ag(e,{autoStart:!1,...n.animatedMeshSource===void 0?{}:{animatedMeshSource:n.animatedMeshSource},...t.meshBufferSource===void 0?{}:{meshBufferSource:t.meshBufferSource},...n.meshResourceSource===void 0?{}:{meshResourceSource:n.meshResourceSource},...n.textureResourceSource===void 0?{}:{textureResourceSource:n.textureResourceSource},camera:{initialPose:o.cameraPose(),...t.projection===void 0?{}:{projection:t.projection}},...t.clearColor===void 0?{}:{clearColor:t.clearColor},...t.pixelRatio===void 0?{}:{pixelRatio:t.pixelRatio},lighting:r,frame:i,...t.viewComposition===void 0?{}:{viewComposition:t.viewComposition}})}catch(e){throw o.dispose(),e}let c=D_(s,n.contentHashes??new Map),l=t.presentationHosts??null,u=null,d=null,f=new e_,p=null,m=new m_(j_(e)),h=new p_,g=!1,_=()=>({controls:o.requiresAnimationFrame(),presentation:l?.requiresAnimationFrame()??!1,retainedAnimation:A_(p)}),v=()=>{m.request()},y=(t,n)=>{if(g)throw Error(`renderer surface is disposed`);t_(t);let r=d===null?0:Math.min(.05,Math.max(0,(t-d)/1e3));d=t,o.update(r);let i=T_(),a=o.cameraSnapshot();s.setCameraPose(a.pose,a.basis);let c=T_();l?.advance(r);let u=T_(),h=T_(),_=s.renderOnce(t),v=T_();return p=w_(f.record({source:n,sourceTimeMs:t,backendSubmissionStartedMs:h,backendSubmissionEndedMs:v}),_),m.submitted(j_(e)),{submission:p,controlsUpdatedAtMs:i,cameraUpdatedAtMs:c,presentationAdvancedAtMs:u,backendSubmittedAtMs:v}},b=(e=globalThis.performance?.now()??0)=>y(e,`explicit`).submission,x=t=>{let n=T_();u=globalThis.requestAnimationFrame(x);let r=T_(),i=m.consumeDecision(j_(e),_()),a=T_();if(i.shouldSubmit){let e=s.automaticSubmissionReady(t),o=T_(),c=s.automaticSubmissionPacing();if(e){let e=y(t,`animationFrame`),s=T_();h.record(t,`admitted`,i,c,E_({callbackStartedAtMs:n,successorQueuedAtMs:r,demandObservedAtMs:a,backendReadinessObservedAtMs:o,controlsUpdatedAtMs:e.controlsUpdatedAtMs,cameraUpdatedAtMs:e.cameraUpdatedAtMs,presentationAdvancedAtMs:e.presentationAdvancedAtMs,backendSubmittedAtMs:e.backendSubmittedAtMs,callbackEndedAtMs:s}))}else{let e=T_();h.record(t,`backendBlocked`,i,c,E_({callbackStartedAtMs:n,successorQueuedAtMs:r,demandObservedAtMs:a,backendReadinessObservedAtMs:o,callbackEndedAtMs:e})),v()}}else{let e=T_();h.record(t,`noDemand`,i,s.automaticSubmissionPacing(),E_({callbackStartedAtMs:n,successorQueuedAtMs:r,demandObservedAtMs:a,callbackEndedAtMs:e}))}},S=()=>{if(g)throw Error(`renderer surface is disposed`);u===null&&(u=globalThis.requestAnimationFrame(x),v())},C=()=>{u!==null&&(globalThis.cancelAnimationFrame(u),u=null)};return y(0,`mount`),t.autoStart!==!1&&S(),{kind:`rusty_renderer_surface.v1`,backend:__,canvas:e,animationProjection:c,animatedMeshPlayback:e=>c.playback(e),sampleAnimatedMesh:(e,t,n)=>s.sampleAnimatedMesh(e,t,n),applyFrame:e=>{try{return a.validateFrame(e),s.applyFrame(e),a.applyFrame(e),v(),{applied:!0,diagnostics:[]}}catch(e){return{applied:!1,diagnostics:[{code:e instanceof Kf?`renderer_lighting_policy_rejected`:`animated_mesh_frame_rejected`,message:e instanceof Error?e.message:String(e),asset:null,handle:null}]}}},applyPresentation:async e=>{let t=await(l??new Jg({})).apply(e);return t.applied>0&&v(),t},automaticSubmissionPacing:()=>Object.freeze({...s.automaticSubmissionPacing(),hostAdmission:h.sample()}),cameraPose:o.cameraPose,cameraProjection:s.cameraProjection,inputReadout:o.inputReadout,lightingReadout:s.lightingReadout,visibilityReadout:s.visibilityReadout,configureViews:e=>{let t=s.configureViews(e);return t.applied&&v(),t},viewCompositionReadout:s.viewCompositionReadout,lockPointer:o.lockPointer,movementState:o.movementState,pick:e=>{let t=s.pick(e);return{diagnostics:t.diagnostics,hint:t.hit,kind:`rusty_renderer_surface_pick.v1`}},pointerLocked:o.pointerLocked,projectWorldPoint:s.projectWorldPoint,projectionSnapshot:()=>a.snapshot(),releaseInput:o.releaseInput,renderOnce:b,resetCamera:()=>{o.resetCamera(),d=null,y(0,`cameraReset`)},setCameraPose:(e,t)=>{let n=o.cameraSnapshot();o.setCameraPose(e,t),s.setCameraPose(e,t),M_(n,o.cameraSnapshot())||v()},setPresentationHosts:e=>{l=e,v()},snapshot:s.snapshot,start:S,stop:C,submission:()=>{if(p===null)throw Error(`renderer surface has not submitted a frame`);return p},timing:f.read.bind(f),dispose:()=>{g||=(C(),o.dispose(),s.dispose(),!0)}}}function w_(e,t){return i_(e,{drawCallCount:t.drawCallCount,renderHandleCount:t.renderHandleCount,geometryResourceCount:t.geometryResourceCount,materialResourceCount:t.materialResourceCount,textureResourceCount:t.textureResourceCount,animatedInstanceCount:t.animatedInstanceCount,triangleCount:t.triangleCount})}function T_(){return globalThis.performance?.now()??0}function E_(e){return Object.freeze({schemaVersion:1,callbackStartedAtMs:e.callbackStartedAtMs,successorQueuedAtMs:e.successorQueuedAtMs,demandObservedAtMs:e.demandObservedAtMs,backendReadinessObservedAtMs:e.backendReadinessObservedAtMs??null,controlsUpdatedAtMs:e.controlsUpdatedAtMs??null,cameraUpdatedAtMs:e.cameraUpdatedAtMs??null,presentationAdvancedAtMs:e.presentationAdvancedAtMs??null,backendSubmittedAtMs:e.backendSubmittedAtMs??null,callbackEndedAtMs:e.callbackEndedAtMs})}function D_(e,t){return{kind:`rusty_renderer_animated_mesh_projection.v1`,applyFrame:t=>{try{return e.applyFrame(t),{applied:!0,diagnostics:[]}}catch(e){return{applied:!1,diagnostics:[{code:`animated_mesh_frame_rejected`,message:e instanceof Error?e.message:String(e),asset:null,handle:null}]}}},advance:()=>({applied:!0,diagnostics:[]}),playback:n=>{let r=e.animatedMeshPlayback(n);return Pg(n,r,r===void 0?null:t.get(r.asset)??null)},snapshot:e.snapshot,hasAnimationTarget:t=>e.renderer.has(t),setAnimationControllerWeights:(t,n)=>{e.renderer.setAnimationControllerWeights(t,n)},hasAnimationClips:(t,n)=>e.renderer.hasAnimationControllerClips(t,n),clearAnimationControllerWeights:t=>{e.renderer.clearAnimationControllerWeights(t)}}}function O_(e,t){let n=t?.enabled===!0,r=e.ownerDocument,i=U_(t?.moveSpeed??5.8,`moveSpeed`),a=U_(t?.mouseSensitivity??.0021,`mouseSensitivity`),o=H_(t?.eyeHeight??1.62,`eyeHeight`),s=V_(t?.initialPosition??[0,o,8],`initialPosition`),c=t?.resolveMovement,l=new Set,u=[0,0],d,f=0,p=G_(H_(t?.initialPitchDegrees??0,`initialPitchDegrees`)),m=G_(H_(t?.initialYawDegrees??0,`initialYawDegrees`)),h=[...s],g=L_(c),_=e.tabIndex,v=e.style.touchAction;e.tabIndex<0&&(e.tabIndex=0),e.style.touchAction=`none`;let y=()=>r.pointerLockElement===e,b=()=>y()||r.activeElement===e,x=()=>{l.clear(),u=[0,0]},S=()=>{x(),y()&&r.exitPointerLock()},C=t=>{!n||t.button!==0||(t.preventDefault(),e.focus({preventScroll:!0}),y()||e.requestPointerLock())},w=()=>{y()||x()},T=e=>{!n||!y()||(u=[u[0]+e.movementX,u[1]+e.movementY])},E=e=>{!n||!b()||!k_.has(e.code)||(e.preventDefault(),l.add(e.code))},D=e=>{k_.has(e.code)&&l.delete(e.code)};e.addEventListener(`pointerdown`,C),r.addEventListener(`pointerlockchange`,w),r.addEventListener(`mousemove`,T),r.addEventListener(`keydown`,E),r.addEventListener(`keyup`,D),r.defaultView?.addEventListener(`blur`,x);let O=()=>({position:[J_(h[0]),J_(h[1]),J_(h[2])],pitchDegrees:q_(K_(p)),yawDegrees:q_(K_(m))});return{cameraPose:O,cameraSnapshot:()=>({...d===void 0?{}:{basis:d},pose:O()}),inputReadout:()=>({enabled:n,pointerLocked:y(),pressedCodes:[...l].sort()}),lockPointer:()=>{n&&!y()&&e.requestPointerLock()},movementState:()=>g,pointerLocked:y,releaseInput:S,requiresAnimationFrame:()=>n&&(l.size>0||u[0]!==0||u[1]!==0),resetCamera:()=>{x(),d=void 0,f=0,p=G_(t?.initialPitchDegrees??0),m=G_(t?.initialYawDegrees??0),h=[...s],g=L_(c)},setCameraPose:(e,t)=>{z_(e),t!==void 0&&B_(t),h=[...e.position],p=G_(e.pitchDegrees),m=G_(e.yawDegrees),d=t},update:e=>{if(!n)return;let t=Math.max(0,H_(e,`deltaSeconds`)),r=F_(l,`KeyW`,`KeyS`),s=F_(l,`KeyD`,`KeyA`),_=u[0]*K_(a),v=-u[1]*K_(a);if(u=[0,0],r===0&&s===0&&_===0&&v===0)return;if(c!==void 0){f+=1;let e=c({deltaSeconds:t,moveForward:r,moveRight:s,moveSpeedUnitsPerSecond:i,pitchDeltaDegrees:v,poseBefore:O(),sequence:f,yawDeltaDegrees:_});z_(e.pose),h=[...e.pose.position],p=G_(e.pose.pitchDegrees),m=G_(e.pose.yawDegrees),d=e.basis,g={mode:`caller_resolved`,blockedAxes:[...e.blockedAxes??[]],collided:e.collided??!1,resolutionId:e.resolutionId??null};return}m+=G_(_),p=W_(p+G_(v),G_(-85),G_(85)),d=void 0;let y=I_(m,r,s);if(y!==null&&t>0){let e=i*t;h=[h[0]+y[0]*e,o,h[2]+y[2]*e]}g=L_(void 0)},dispose:()=>{S(),e.removeEventListener(`pointerdown`,C),r.removeEventListener(`pointerlockchange`,w),r.removeEventListener(`mousemove`,T),r.removeEventListener(`keydown`,E),r.removeEventListener(`keyup`,D),r.defaultView?.removeEventListener(`blur`,x),e.tabIndex=_,e.style.touchAction=v}}}var k_=new Set([`KeyA`,`KeyD`,`KeyS`,`KeyW`]);function A_(e){let t=e?.statistics.animatedInstanceCount;return t?.status===`available`&&t.value>0}function j_(e){return{bufferHeight:e.height,bufferWidth:e.width,clientHeight:e.clientHeight,clientWidth:e.clientWidth}}function M_(e,t){return P_(e.pose.position,t.pose.position)&&e.pose.pitchDegrees===t.pose.pitchDegrees&&e.pose.yawDegrees===t.pose.yawDegrees&&N_(e.basis,t.basis)}function N_(e,t){return e===void 0||t===void 0?e===t:P_(e.forward,t.forward)&&P_(e.right,t.right)&&P_(e.up,t.up)}function P_(e,t){return e[0]===t[0]&&e[1]===t[1]&&e[2]===t[2]}function F_(e,t,n){return Number(e.has(t))-Number(e.has(n))}function I_(e,t,n){let r=[-Math.sin(e),0,-Math.cos(e)],i=[Math.cos(e),0,-Math.sin(e)],a=[r[0]*t+i[0]*n,0,r[2]*t+i[2]*n],o=Math.hypot(a[0],a[2]);return o===0?null:[a[0]/o,0,a[2]/o]}function L_(e){return{mode:e===void 0?`free_camera`:`caller_resolved`,blockedAxes:[],collided:!1,resolutionId:null}}function R_(e){if(e!==void 0&&e.schemaVersion!==1)throw new g_(`lighting.schemaVersion must equal 1`);let t=e?.defaultLights?.world??`neutral`,n=e?.defaultLights?.viewmodel??`neutral`;if(t!==`neutral`&&t!==`disabled`||n!==`neutral`&&n!==`disabled`)throw new g_(`default lighting mode must be neutral or disabled`);let r=e?.shadows?.enabled??!1;if(typeof r!=`boolean`)throw new g_(`lighting.shadows.enabled must be boolean`);let i=e?.shadows?.maximumActiveLights??8;if(!Number.isSafeInteger(i)||i<0||i>8)throw new g_(`lighting.shadows.maximumActiveLights must be in 0..=8`);return{schemaVersion:1,defaultLights:{world:t,viewmodel:n},shadows:{enabled:r,maximumActiveLights:i}}}function z_(e){V_(e.position,`resolved camera position`),H_(e.pitchDegrees,`resolved camera pitch`),H_(e.yawDegrees,`resolved camera yaw`)}function B_(e){V_(e.forward,`camera basis forward`),V_(e.right,`camera basis right`),V_(e.up,`camera basis up`)}function V_(e,t){return e.forEach((e,n)=>H_(e,`${t}[${n}]`)),e}function H_(e,t){if(!Number.isFinite(e))throw RangeError(`${t} must be finite`);return e}function U_(e,t){if(!Number.isFinite(e)||e<=0)throw RangeError(`${t} must be finite and greater than zero`);return e}function W_(e,t,n){return Math.min(n,Math.max(t,e))}function G_(e){return e*Math.PI/180}function K_(e){return e*180/Math.PI}function q_(e){return Number(e.toFixed(2))}function J_(e){return Number(e.toFixed(4))}var Y_=class{#e;#t;#n;#r;#i=new Map;constructor(e){if(!Number.isFinite(e.pixelsPerWorldUnit??24)||(e.pixelsPerWorldUnit??24)<=0)throw RangeError(`pixelsPerWorldUnit must be finite and greater than zero`);this.#e=e.container,this.#t=e.createElement??(()=>document.createElement(`div`)),this.#n=e.pixelsPerWorldUnit??24,this.#r=e.projectWorld}create(e){if(this.#i.has(e.id))throw Error(`particle billboard ${String(e.id)} already exists`);let t=this.#t();t.dataset.rustyParticleId=String(e.id),t.style.position=`absolute`,t.style.pointerEvents=`none`,t.style.backgroundRepeat=`no-repeat`,t.style.transform=`translate(-50%, -50%)`,t.style.willChange=`left, top, width, height, opacity`,this.#e.appendChild(t),this.#i.set(e.id,t),this.#a(t,e)}update(e){let t=this.#i.get(e.id);if(t===void 0)throw Error(`particle billboard ${String(e.id)} does not exist`);this.#a(t,e)}destroy(e){let t=this.#i.get(e);t!==void 0&&(t.remove(),this.#i.delete(e))}dispose(){for(let e of this.#i.values())e.remove();this.#i.clear()}get activeCount(){return this.#i.size}#a(e,t){let n=this.#r(t.position),r=Math.max(1,t.size*this.#n),i=Math.max(0,Math.min(t.frameCount-1,t.frameIndex)),a=t.frameCount<=1?0:i/(t.frameCount-1)*100;e.style.display=n.insideViewport?`block`:`none`,e.style.left=`${n.xPixels}px`,e.style.top=`${n.yPixels}px`,e.style.width=`${r}px`,e.style.height=`${r}px`,e.style.opacity=String(Math.max(0,Math.min(1,t.color[3]))),e.style.backgroundColor=ev(t.color),e.style.backgroundImage=`url("${tv(t.spriteUrl)}")`,e.style.backgroundSize=`${String(t.frameCount*100)}% 100%`,e.style.backgroundPosition=`${String(a)}% 0`}},X_=class{#e;#t;#n=new Map;constructor(e){this.#e=e.container,this.#t=e.createElement??(()=>document.createElement(`pre`))}render(e,t,n){let r=e,i=this.#n.get(r);i===void 0&&(i=this.#t(),i.dataset.rustyTelemetryHandle=String(r),i.style.position=`absolute`,i.style.zIndex=`31000`,i.style.pointerEvents=`none`,i.style.margin=`12px`,i.style.padding=`8px 10px`,i.style.borderRadius=`4px`,i.style.background=`rgba(8, 12, 16, 0.82)`,i.style.color=`#d9f2ff`,i.style.font=`12px/1.35 ui-monospace, SFMono-Regular, Menlo, monospace`,this.#e.appendChild(i),this.#n.set(r,i)),Z_(i,t.corner),i.style.display=t.visible?`block`:`none`,i.textContent=Q_(t,n)}destroy(e){let t=e,n=this.#n.get(t);n!==void 0&&(n.remove(),this.#n.delete(t))}dispose(){for(let e of this.#n.values())e.remove();this.#n.clear()}get activeCount(){return this.#n.size}};function Z_(e,t){e.style.top=t.startsWith(`top`)?`0`:``,e.style.bottom=t.startsWith(`bottom`)?`0`:``,e.style.left=t.endsWith(`Left`)?`0`:``,e.style.right=t.endsWith(`Right`)?`0`:``}function Q_(e,t){if(t===null)return`${e.title}\nwaiting for telemetry`;let n=t.metrics.map(e=>`${e.counter}: ${$_(e.value)} ${e.unit}`),r=t.diagnostics.map(e=>`! ${e.message}`);return[e.title,...n,...r].join(`
`)}function $_(e){return Number.isInteger(e)?String(e):e.toFixed(2)}function ev(e){return`rgba(${Math.round(e[0]*255)}, ${Math.round(e[1]*255)}, ${Math.round(e[2]*255)}, ${e[3]})`}function tv(e){return e.replaceAll(`\\`,`\\\\`).replaceAll(`"`,`\\"`)}e((()=>{It(),Eg(),Lg(),f_()}))(),180/Math.PI;var nv=class{#e;#t;#n=new Map;#r=[];#i=0;#a=0;constructor(e,t={}){this.#e=e,this.#t=rv(t.cues??[])}applyPresentation(e){let t=[],n=0;for(let r of e.ops){if(r.domain!==`animation`)continue;let e=this.#o(r);e===null?n+=1:(t.push(e),this.#r.push(e))}return{applied:n,diagnostics:t,cues:[],readout:this.readout()}}advance(e){if(!Number.isFinite(e)||e<0)throw Error(`animation host deltaSeconds must be finite and non-negative`);let t=[],n=[];for(let r of this.#n.values()){let i=r.interpolation;if(i!==null){i.elapsedSeconds=Math.min(i.durationSeconds,i.elapsedSeconds+e);let n=i.durationSeconds===0?1:i.elapsedSeconds/i.durationSeconds;r.presented=dv(i.from,i.to,n);try{this.#e.setAnimationControllerWeights(r.target,r.presented)}catch(e){let n=pv(`hostFailure`,0,r.handle,r.target,mv(e));t.push(n),this.#r.push(n)}n===1&&(r.interpolation=null)}n.push(...iv(r,this.#t,e))}return this.#e.advance(e),this.#i+=1,{applied:this.#n.size,diagnostics:t,cues:n,readout:this.readout()}}requiresAnimationFrame(){return this.#n.size>0}readout(){return{activeControllers:this.#n.size,sampledFrames:this.#i,compatibilityFallbacks:this.#a,diagnostics:[...this.#r]}}cleanup(){let e=[],t=0;for(let n of this.#n.values())try{this.#e.clearAnimationControllerWeights(n.target),t+=1}catch(t){let r=pv(`hostFailure`,0,n.handle,n.target,mv(t));e.push(r),this.#r.push(r)}return this.#n.clear(),{applied:t,diagnostics:e,cues:[],readout:this.readout()}}#o(e){let{op:t,meta:n}=e;if(t.op===`create`){if(this.#n.has(t.handle))return pv(`duplicateHandle`,n.sequence,t.handle,t.descriptor.target,`animation handle already exists`);let e=sv(t.descriptor.controller);if(e!==null||t.descriptor.tickDurationMillis===0)return pv(`invalidDescriptor`,n.sequence,t.handle,t.descriptor.target,e??`tick duration must be non-zero`);if(!this.#e.hasAnimationTarget(t.descriptor.target))return pv(`unknownTarget`,n.sequence,t.handle,t.descriptor.target,`animation target is unavailable`);let r=this.#e.playback(t.descriptor.target);if(r.asset===null)return pv(`assetMissing`,n.sequence,t.handle,t.descriptor.target,`animation target has no loaded asset`);if(r.asset!==t.descriptor.asset)return pv(`incompatibleRig`,n.sequence,t.handle,t.descriptor.target,`animation descriptor asset does not match the target rig`);if(r.contentHash!==t.descriptor.contentHash)return pv(`contentHashMismatch`,n.sequence,t.handle,t.descriptor.target,`animation descriptor content hash does not match the loaded target rig`);let i=cv(t.descriptor.controller);if(!this.#e.hasAnimationClips(t.descriptor.target,i.map(e=>e.clip)))return pv(`clipMissing`,n.sequence,t.handle,t.descriptor.target,`controller references an unavailable clip`);try{this.#e.setAnimationControllerWeights(t.descriptor.target,i)}catch(e){return fv(e,n.sequence,t.handle,t.descriptor.target)}return this.#n.set(t.handle,{handle:t.handle,target:t.descriptor.target,asset:t.descriptor.asset,tickDurationSeconds:t.descriptor.tickDurationMillis/1e3,controller:t.descriptor.controller,presented:i,interpolation:null,clipSampleSeconds:new Map,emittedCueKeys:new Set}),null}let r=this.#n.get(t.handle);if(r===void 0)return pv(`unknownHandle`,n.sequence,t.handle,null,`animation handle is unavailable`);if(t.op===`destroy`){try{this.#e.clearAnimationControllerWeights(r.target)}catch(e){return fv(e,n.sequence,t.handle,r.target)}return this.#n.delete(t.handle),null}let i=sv(t.controller);if(i!==null)return pv(`invalidDescriptor`,n.sequence,t.handle,r.target,i);if(t.controller.revision<r.controller.revision)return pv(`staleRevision`,n.sequence,t.handle,r.target,`controller revision moved backward`);if(t.controller.revision===r.controller.revision&&!ov(r.controller,t.controller))return pv(`staleRevision`,n.sequence,t.handle,r.target,`controller state or transition progress moved backward without a new revision`);let a=cv(t.controller);return this.#e.hasAnimationClips(r.target,a.map(e=>e.clip))?(r.controller=t.controller,r.interpolation={from:r.presented,to:a,durationSeconds:r.tickDurationSeconds,elapsedSeconds:0},null):pv(`clipMissing`,n.sequence,t.handle,r.target,`controller references an unavailable clip`)}};function rv(e){let t=new Set;return e.map(e=>{if(e.cueId.trim().length===0||e.asset.trim().length===0||e.clip.trim().length===0||e.signal.id.trim().length===0||!Number.isFinite(e.atSeconds)||e.atSeconds<0)throw Error(`animation cue definitions require non-empty identifiers and a finite non-negative marker`);let n=av(e);if(t.has(n))throw Error(`duplicate animation cue definition ${n}`);return t.add(n),e})}function iv(e,t,n){let r=new Set(e.presented.filter(e=>e.weight>0).map(e=>e.clip));for(let n of e.clipSampleSeconds.keys())if(!r.has(n)){e.clipSampleSeconds.delete(n);for(let r of t)r.asset===e.asset&&r.clip===n&&e.emittedCueKeys.delete(av(r))}let i=[];for(let r of e.presented){if(r.weight<=0)continue;let a=e.clipSampleSeconds.get(r.clip),o=(a??0)+n*r.speed;e.clipSampleSeconds.set(r.clip,o);for(let n of t){if(n.asset!==e.asset||n.clip!==r.clip)continue;let t=av(n);!((a===void 0||a<n.atSeconds)&&n.atSeconds<=o)||e.emittedCueKeys.has(t)||(e.emittedCueKeys.add(t),i.push({kind:`rusty.animation.sampled_cue.v1`,cueId:n.cueId,handle:e.handle,target:e.target,asset:e.asset,clip:n.clip,markerSeconds:n.atSeconds,sampledAtSeconds:o,signal:n.signal}))}}return i}function av(e){return`${e.asset}:${e.clip}:${e.cueId}`}function ov(e,t){return e.graphId!==t.graphId||e.graphVersion!==t.graphVersion||e.stateId!==t.stateId||t.controllerTick<e.controllerTick?!1:e.transition===null||t.transition!==null&&e.transition.transitionId===t.transition.transitionId&&e.transition.fromStateId===t.transition.fromStateId&&e.transition.toStateId===t.transition.toStateId&&e.transition.durationTicks===t.transition.durationTicks&&t.transition.elapsedTicks>=e.transition.elapsedTicks}function sv(e){let t=[e.motion,e.transition?.targetMotion].filter(e=>e!==void 0);for(let e of t)if(e.clipA.length===0||e.blendWeightMilli<0||e.blendWeightMilli>1e3||e.speedMilli<=0||e.clipB===null&&e.blendWeightMilli!==0)return`controller motion is invalid`;let n=e.transition;return n!==null&&(n.durationTicks===0||n.elapsedTicks>n.durationTicks)?`controller transition progress is invalid`:null}function cv(e){let t=e.transition;if(t===null)return lv(e.motion);let n=t.elapsedTicks/t.durationTicks;return uv([...lv(e.motion).map(e=>({...e,weight:e.weight*(1-n)})),...lv(t.targetMotion).map(e=>({...e,weight:e.weight*n}))])}function lv(e){let t=e.clipB===null?0:e.blendWeightMilli/1e3,n=[{clip:e.clipA,weight:1-t,speed:e.speedMilli/1e3}];return e.clipB!==null&&t>0&&n.push({clip:e.clipB,weight:t,speed:e.speedMilli/1e3}),n}function uv(e){let t=new Map;for(let n of e){if(n.weight<=0)continue;let e=t.get(n.clip);t.set(n.clip,{clip:n.clip,weight:(e?.weight??0)+n.weight,speed:n.speed})}return[...t.values()].sort((e,t)=>e.clip.localeCompare(t.clip))}function dv(e,t,n){return uv([...new Set([...e.map(e=>e.clip),...t.map(e=>e.clip)])].map(r=>{let i=e.find(e=>e.clip===r),a=t.find(e=>e.clip===r);return{clip:r,weight:(i?.weight??0)+((a?.weight??0)-(i?.weight??0))*n,speed:a?.speed??i?.speed??1}}))}function fv(e,t,n,r){let i=mv(e);return pv(i.includes(`missing clip`)?`clipMissing`:i.includes(`handle`)?`unknownTarget`:`hostFailure`,t,n,r,i)}function pv(e,t,n,r,i){return{code:e,sequence:t,handle:n,target:r,message:i}}function mv(e){return e instanceof Error?e.message:String(e)}Ng();var hv=class{#e;#t;#n;#r;#i=new Map;#a=new Map;#o=new Set;#s=new Set;#c=[];#l=0;#u=!1;constructor(e){this.#e=e.createContext?.()??_v(),this.#n=e.resolveResource,this.#t=e.resolveEntityPosition??(()=>null);let t=this.#e.createGain(),n=this.#e.createGain(),r=this.#e.createGain();t.connect(this.#e.destination),n.connect(this.#e.destination),r.connect(this.#e.destination),this.#r={sfx:t,ambient:n,ui:r}}async resume(){try{return await this.#e.resume(),this.#e.state===`running`?[]:this.#h(`audioContextBlocked`,`audio context remained `+this.#e.state)}catch(e){return this.#h(`audioContextBlocked`,Ov(e,`audio context resume failed`))}}updateListener(e){if(![...e.position,...e.forward,...e.up].every(Number.isFinite))return this.#h(`invalidDescriptor`,`audio listener pose must be finite`);let t=this.#e.currentTime;return bv(this.#e.listener,`position`,e.position,t),bv(this.#e.listener,`forward`,e.forward,t),bv(this.#e.listener,`up`,e.up,t),[]}async applyPresentation(e){if(this.#u)return this.#g(0,this.#h(`hostFailure`,`audio host is disposed`));let t=[],n=0;for(let r of e.ops){if(r.domain!==`audio`)continue;let e=await this.#d(r);e===null?n+=1:(t.push(e),this.#c.push(e))}return this.#g(n,t)}readout(){return{activeSources:this.#a.size,cachedClips:this.#i.size,emittedSignals:this.#l,diagnostics:[...this.#c]}}refreshLayout(){if(this.#u)return this.#h(`hostFailure`,`audio host is disposed`);let e=[];for(let[t,n]of this.#a){if(n.descriptor.emitter.kind!==`entityAttached`||n.panner===null)continue;let r=xv(n.descriptor.emitter,this.#t);if(r===null||!r.every(Number.isFinite)){e.push({code:`hostFailure`,sequence:n.sequence,handle:t,message:`entity-attached audio source has no finite projected position`});continue}yv(n.panner,r,this.#e.currentTime)}return this.#c.push(...e),e}async dispose(){if(!this.#u){this.#u=!0;for(let e of[...this.#a.values(),...this.#o])Cv(e);this.#a.clear(),this.#o.clear(),this.#s.clear();for(let e of Object.values(this.#r))e.disconnect();await this.#e.close()}}async#d(e){let{meta:t,op:n}=e;try{if(n.op===`emit`){if(this.#s.has(n.signalId))return null;let e=await this.#p(n.descriptor,t.sequence);return this.#s.add(n.signalId),this.#o.add(e),e.source.onended=()=>{this.#o.delete(e),Cv(e)},e.source.start(),this.#l+=1,null}if(n.op===`create`){if(this.#a.has(n.handle))return Tv(`duplicateHandle`,t,n.handle,`audio handle is active`);let e=await this.#p(n.descriptor,t.sequence);return this.#a.set(n.handle,e),e.source.start(),null}if(n.op===`destroy`){let e=this.#a.get(n.handle);return e===void 0?Tv(`unknownHandle`,t,n.handle,`audio handle is unknown`):(this.#a.delete(n.handle),Cv(e),null)}return await this.#f(t,n.handle,n.patch)}catch(e){return Tv(Dv(e),t,wv(n),Ov(e,`audio host operation failed`))}}async#f(e,t,n){let r=this.#a.get(t);if(r===void 0)return Tv(`unknownHandle`,e,t,`audio handle is unknown`);let i=Sv(r.descriptor,n);if(n.emitter!==null){let n=await this.#p(i,e.sequence);return Cv(r),this.#a.set(t,n),n.source.start(),null}return r.descriptor=i,r.sequence=e.sequence,vv(this.#e,r,i,this.#t),null}async#p(e,t){let n=this.#e.createBufferSource();n.buffer=await this.#m(e.clip);let r={descriptor:e,sequence:t,source:n,dryGain:this.#e.createGain(),wetGain:this.#e.createGain(),stereoPanner:this.#e.createStereoPanner(),panner:e.emitter.kind===`global2d`?null:this.#e.createPanner(),disposed:!1};return n.connect(r.stereoPanner),r.stereoPanner.connect(r.dryGain),r.dryGain.connect(this.#r[e.bus]),r.panner!==null&&(n.connect(r.panner),r.panner.connect(r.wetGain),r.wetGain.connect(this.#r[e.bus])),vv(this.#e,r,e,this.#t),r}async#m(e){let t=this.#i.get(e.contentHash);if(t!==void 0)return t;let n=this.#n(e).then(async t=>{if(t.contentHash!==e.contentHash)throw new gv(`contentHashMismatch`,`resolved audio content hash does not match the requested clip`);let n=await Dg(t.bytes,e.contentHash).catch(e=>{throw new gv(`contentHashMismatch`,e instanceof Error?e.message:String(e))});if(n!==e.contentHash)throw new gv(`contentHashMismatch`,`audio bytes hash ${n} does not match ${e.contentHash}`);try{return await this.#e.decodeAudioData(t.bytes.slice(0))}catch(e){throw new gv(`decodeFailed`,Ov(e,`audio clip decoding failed`))}});this.#i.set(e.contentHash,n);try{return await n}catch(t){throw this.#i.delete(e.contentHash),t}}#h(e,t){let n=Ev(e,t);return this.#c.push(n),[n]}#g(e,t){return{applied:e,diagnostics:t,readout:this.readout()}}},gv=class extends Error{code;constructor(e,t){super(t),this.code=e}};function _v(){let e=globalThis.AudioContext;if(e===void 0)throw Error(`Web Audio AudioContext is unavailable`);return new e}function vv(e,t,n,r){let i=e.currentTime;t.source.loop=n.looping,t.source.playbackRate.setValueAtTime(n.pitch,i),t.stereoPanner.pan.setValueAtTime(n.pan,i);let a=n.emitter.kind===`global2d`?0:n.spatialBlend;if(t.dryGain.gain.setValueAtTime(n.volume*(1-a),i),t.wetGain.gain.setValueAtTime(n.volume*a,i),t.panner===null)return;let o=xv(n.emitter,r);if(o===null)throw Error(`entity-attached audio source has no projected position`);t.panner.panningModel=`equalpower`,t.panner.distanceModel=`inverse`,t.panner.refDistance=1,t.panner.maxDistance=n.attenuation,t.panner.rolloffFactor=1,yv(t.panner,o,i)}function yv(e,t,n){e.positionX.setValueAtTime(t[0],n),e.positionY.setValueAtTime(t[1],n),e.positionZ.setValueAtTime(t[2],n)}function bv(e,t,n,r){e[`${t}X`].setValueAtTime(n[0],r),e[`${t}Y`].setValueAtTime(n[1],r),e[`${t}Z`].setValueAtTime(n[2],r)}function xv(e,t){if(e.kind===`global2d`)return[0,0,0];if(e.kind===`world3d`)return e.position;let n=t(e.entity);return n===null?null:[n[0]+e.offset[0],n[1]+e.offset[1],n[2]+e.offset[2]]}function Sv(e,t){return{...e,volume:t.volume??e.volume,pitch:t.pitch??e.pitch,looping:t.looping??e.looping,spatialBlend:t.spatialBlend??e.spatialBlend,attenuation:t.attenuation??e.attenuation,pan:t.pan??e.pan,emitter:t.emitter??e.emitter}}function Cv(e){if(!e.disposed){e.disposed=!0,e.source.onended=null;try{e.source.stop()}catch{}e.source.disconnect(),e.stereoPanner.disconnect(),e.dryGain.disconnect(),e.panner?.disconnect(),e.wetGain.disconnect()}}function wv(e){return e.op===`emit`?null:e.handle}function Tv(e,t,n,r){return{code:e,sequence:t.sequence,handle:n,message:r}}function Ev(e,t){return{code:e,sequence:0,handle:null,message:t}}function Dv(e){return e instanceof gv?e.code:`hostFailure`}function Ov(e,t){return e instanceof Error?e.message:t}Ng();var kv=class{#e;#t;#n;#r;#i;#a;#o;#s=new Map;#c=new Set;#l=new Set;#u=new Map;#d=[];#f=0;constructor(e){this.#e=e.container,this.#t=e.createElement??Pv,this.#n=e.loadFont??Iv,this.#r=e.localize??Nv,this.#i=e.projectWorld,this.#a=e.resolveEntityPosition,this.#o=e.resolveResource??(async()=>null)}async applyPresentation(e){let t=[],n=0;for(let r of e.ops){if(r.domain!==`billboard`)continue;let e=await this.#p(r);e===null?n+=1:(t.push(e),this.#d.push(e))}return t.push(...this.refreshLayout()),{applied:n,diagnostics:t,readout:this.readout()}}refreshLayout(){let e=[],t=0;for(let[n,r]of this.#s){let i=this.#S(r.descriptor.anchor);if(i===null){r.element.style.display=`none`,t+=1,e.push(this.#C(`anchorMissing`,0,n,`billboard entity anchor is unavailable`));continue}let a=this.#i(i),o=!r.descriptor.visible||!a.insideViewport||a.distance>r.descriptor.maxDistance||r.descriptor.layer===`occluded`&&a.occluded;if(r.element.style.display=o?`none`:`block`,o){t+=1;continue}r.element.style.left=`${a.xPixels}px`,r.element.style.top=`${a.yPixels}px`,r.element.style.zIndex=Mv(r.descriptor.layer,a.depth)}return this.#f=t,this.#d.push(...e),e}readout(){return{activeBillboards:this.#s.size,loadedFonts:this.#c.size,loadedIcons:this.#l.size,culledBillboards:this.#f,diagnostics:[...this.#d]}}cleanup(){for(let e of this.#s.values())e.element.remove();this.#s.clear(),this.#f=0}dispose(){this.cleanup(),this.#c.clear(),this.#l.clear(),this.#u.clear(),this.#d.length=0}async#p(e){try{switch(e.op.op){case`create`:return await this.#m(e.meta,e.op);case`update`:return await this.#h(e.meta,e.op);case`destroy`:return this.#g(e.meta,e.op)}}catch(t){return this.#C(Rv(t),e.meta.sequence,e.op.handle,t instanceof Error?t.message:String(t))}}async#m(e,t){if(this.#s.has(t.handle))return this.#C(`duplicateHandle`,e.sequence,t.handle,`billboard handle is already active`);await this.#_(t.descriptor);let n=this.#t();return n.setAttribute(`data-rusty-billboard-handle`,String(t.handle)),this.#b(n,t.descriptor),Fv(this.#e,n),this.#s.set(t.handle,{descriptor:t.descriptor,element:n}),null}async#h(e,t){let n=this.#s.get(t.handle);if(n===void 0)return this.#C(`unknownHandle`,e.sequence,t.handle,`billboard handle is not active`);let r=Av(n.descriptor,t.patch);return await this.#_(r),this.#b(n.element,r),n.descriptor=r,null}#g(e,t){let n=this.#s.get(t.handle);return n===void 0?this.#C(`unknownHandle`,e.sequence,t.handle,`billboard handle is not active`):(n.element.remove(),this.#s.delete(t.handle),null)}async#_(e){await this.#v(e.font),e.content.kind===`icon`&&await this.#y(e.content)}async#v(e){if(e.kind===`system`)return;let t=`${e.asset}:${e.contentHash}`;if(this.#c.has(t))return;let n=await this.#o(e.asset);if(n===null)throw new Lv(`fontLoadFailed`,`font resource ${e.asset} is unavailable`);await zv(n.bytes,e.contentHash),await this.#n(e.family,n.bytes),this.#c.add(t)}async#y(e){let t=`${e.texture.asset}:${e.texture.contentHash}`;if(this.#l.has(t))return;let n=await this.#o(e.texture.asset);if(n===null||n.url===void 0)throw new Lv(`iconLoadFailed`,`icon resource ${e.texture.asset} is unavailable or has no host URL`);await zv(n.bytes,e.texture.contentHash),this.#l.add(t),this.#u.set(t,n.url)}#b(e,t){if(e.style.position=`absolute`,e.style.pointerEvents=`none`,e.style.transform=`translate(-50%, -100%)`,e.style.whiteSpace=`nowrap`,e.style.borderRadius=`4px`,e.style.lineHeight=`1.2`,e.style.fontFamily=t.font.family,e.style.fontSize=`${t.heightPixels}px`,e.style.color=jv(t.color),e.style.backgroundColor=jv(t.background),e.style.backgroundImage=``,e.style.backgroundPosition=`center`,e.style.backgroundRepeat=`no-repeat`,e.style.backgroundSize=`contain`,e.setAttribute(`data-rusty-billboard-layer`,t.layer),e.textContent=this.#x(t.content),t.content.kind===`icon`){e.setAttribute(`role`,`img`),e.setAttribute(`aria-label`,e.textContent);let n=`${t.content.texture.asset}:${t.content.texture.contentHash}`,r=this.#u.get(n);r!==void 0&&(e.style.backgroundImage=`url("${r}")`)}else e.setAttribute(`role`,`status`)}#x(e){if(e.kind===`text`)return this.#r(e.localizationKey,e.fallbackText,Object.fromEntries(e.arguments.map(e=>[e.name,e.value])));if(e.kind===`value`){let t=this.#r(e.labelKey,e.fallbackLabel,{}),n=e.unitKey===null?e.fallbackUnit??``:this.#r(e.unitKey,e.fallbackUnit??``,{});return`${t}: ${e.value}${n===``?``:` ${n}`}`}return this.#r(e.altKey,e.fallbackAlt,{})}#S(e){if(e.kind===`world`)return e.position;let t=this.#a(e.entity);return t===null?null:[t[0]+e.offset[0],t[1]+e.offset[1],t[2]+e.offset[2]]}#C(e,t,n,r){return{code:e,sequence:t,handle:n,message:r}}};function Av(e,t){return{anchor:t.anchor??e.anchor,content:t.content??e.content,font:t.font??e.font,heightPixels:t.heightPixels??e.heightPixels,color:t.color??e.color,background:t.background??e.background,maxDistance:t.maxDistance??e.maxDistance,layer:t.layer??e.layer,visible:t.visible??e.visible}}function jv(e){return`rgba(${Math.round(e[0]*255)}, ${Math.round(e[1]*255)}, ${Math.round(e[2]*255)}, ${e[3]})`}function Mv(e,t){return e===`alwaysOnTop`?`30000`:String(2e4-Math.round(Math.max(0,Math.min(1,t))*1e4))}function Nv(e,t,n){return Object.entries(n).reduce((e,[t,n])=>e.replaceAll(`{${t}}`,n),t)}function Pv(){if(globalThis.document===void 0)throw Error(`billboard DOM host is unavailable`);return globalThis.document.createElement(`div`)}function Fv(e,t){if(globalThis.HTMLElement!==void 0&&e instanceof globalThis.HTMLElement){e.appendChild(t);return}e.appendChild(t)}async function Iv(e,t){if(globalThis.FontFace===void 0||globalThis.document?.fonts===void 0)throw new Lv(`fontLoadFailed`,`browser FontFace host is unavailable`);let n=await new globalThis.FontFace(e,t).load();globalThis.document.fonts.add(n)}var Lv=class extends Error{code;constructor(e,t){super(t),this.code=e}};function Rv(e){return e instanceof Lv?e.code:`hostFailure`}async function zv(e,t){let n=await Dg(e,t).catch(e=>{throw new Lv(`contentHashMismatch`,e instanceof Error?e.message:String(e))});if(n!==t)throw new Lv(`contentHashMismatch`,`billboard resource hash mismatch: expected ${t}, got ${n}`)}Ng();var Bv=class{#e;#t;#n;#r;#i;#a=new Map;#o=new Map;#s=new Map;#c=new Set;#l=new Map;#u=[];#d=1;#f=0;#p=0;constructor(e){this.#e=e.maxActiveEmitters??64,this.#t=e.maxParticles??4096,this.#n=e.resolveEntityPosition,this.#r=e.resolveResource,this.#i=e.sink}async applyPresentation(e){let t=[],n=0;for(let r of e.ops){if(r.domain!==`particle`)continue;let e=await this.#m(r);e===null?n+=1:(t.push(e),this.#u.push(e))}return{applied:n,diagnostics:t,readout:this.readout()}}advance(e){if(!Number.isFinite(e)||e<0||e>1){let e=$v(`invalidDescriptor`,`particle frame delta must be finite and between zero and one second`);return this.#u.push(e),{applied:0,diagnostics:[e],readout:this.readout()}}let t=[];for(let n of this.#a.values()){if(!n.descriptor.visible)continue;n.emissionCarry+=n.descriptor.ratePerSecond*e;let r=Math.floor(n.emissionCarry);n.emissionCarry-=r;let i=this.#y(n,r,0);i!==null&&t.push(i)}for(let t of[...this.#s.values()]){if(t.ageSeconds+=e,t.ageSeconds>=t.lifetimeSeconds){this.#x(t);continue}let n=t.descriptor.acceleration;t.velocity[0]+=n[0]*e,t.velocity[1]+=n[1]*e,t.velocity[2]+=n[2]*e,t.position[0]+=t.velocity[0]*e,t.position[1]+=t.velocity[1]*e,t.position[2]+=t.velocity[2]*e,this.#i.update(Gv(t))}return this.#S(),this.#u.push(...t),{applied:this.#s.size,diagnostics:t,readout:this.readout()}}requiresAnimationFrame(){return this.#s.size>0||this.#o.size>0||[...this.#a.values()].some(e=>e.descriptor.visible&&e.descriptor.ratePerSecond>0)}readout(){return{activeEmitters:this.#a.size,activeParticles:this.#s.size,loadedSprites:this.#l.size,emittedBursts:this.#f,droppedParticles:this.#p,diagnostics:[...this.#u]}}cleanup(){for(let e of[...this.#s.values()])this.#x(e);this.#a.clear(),this.#o.clear(),this.#c.clear()}dispose(){this.cleanup(),this.#l.clear(),this.#u.length=0}async#m(e){try{switch(e.op.op){case`emit`:return await this.#h(e.meta,e.op);case`create`:return await this.#g(e.meta,e.op);case`update`:return await this.#_(e.meta,e.op);case`destroy`:return this.#v(e.meta,e.op)}}catch(t){return Qv(t instanceof ny?t.code:`hostFailure`,e.meta,Zv(e.op),t instanceof Error?t.message:String(t))}}async#h(e,t){if(this.#c.has(t.signalId))return null;let n=await this.#C(t.descriptor.sprite),r=Vv(`signal:${t.signalId}`,null,t.descriptor,n),i=this.#y(r,t.descriptor.burstCount,e.sequence,n);return i?.code===`anchorMissing`?i:(this.#c.add(t.signalId),this.#o.set(r.key,r),this.#f+=1,i)}async#g(e,t){let n=t.handle;if(this.#a.has(n))return Qv(`duplicateHandle`,e,t.handle,`particle emitter handle is already active`);if(this.#a.size>=this.#e)return Qv(`budgetExceeded`,e,t.handle,`particle emitter budget is exhausted`);let r=await this.#C(t.descriptor.sprite),i=Vv(`handle:${n}`,t.handle,t.descriptor,r);return this.#a.set(n,i),this.#y(i,t.descriptor.burstCount,e.sequence,r)}async#_(e,t){let n=this.#a.get(t.handle);if(n===void 0)return Qv(`unknownHandle`,e,t.handle,`particle emitter handle is not active`);let r=Xv(n.descriptor,t.patch);return n.spriteUrl=await this.#C(r.sprite),n.descriptor=r,null}#v(e,t){let n=this.#a.get(t.handle);if(n===void 0)return Qv(`unknownHandle`,e,t.handle,`particle emitter handle is not active`);this.#a.delete(t.handle);for(let e of[...n.particleIds]){let t=this.#s.get(e);t!==void 0&&this.#x(t)}return null}#y(e,t,n,r){if(t<=0||!e.descriptor.visible)return null;let i=Wv(e.descriptor.anchor,this.#n);if(i===null)return Qv(`anchorMissing`,{sequence:n},e.handle,`particle entity anchor is unavailable`);let a=Math.max(0,e.descriptor.maxParticles-e.particleIds.size),o=Math.max(0,this.#t-this.#s.size),s=Math.min(t,a,o);this.#p+=t-s;let c=r??e.spriteUrl;for(let t=0;t<s;t+=1){let t=this.#b(e,i,c);e.particleIds.add(t.id),this.#s.set(t.id,t),this.#i.create(Gv(t))}return s<t?Qv(`budgetExceeded`,{sequence:n},e.handle,`particle budget dropped ${t-s} particles`):null}#b(e,t,n){let r=e.descriptor,i=Uv(e,r.lifetimeSeconds[0],r.lifetimeSeconds[1]),a=[Uv(e,r.velocityMin[0],r.velocityMax[0]),Uv(e,r.velocityMin[1],r.velocityMax[1]),Uv(e,r.velocityMin[2],r.velocityMax[2])];return{id:this.#d++,emitterKey:e.key,descriptor:r,spriteUrl:n,ageSeconds:0,lifetimeSeconds:i,position:[...t],velocity:a}}#x(e){this.#s.delete(e.id),this.#i.destroy(e.id),this.#a.get(Number(e.emitterKey.slice(7)))?.particleIds.delete(e.id),this.#o.get(e.emitterKey)?.particleIds.delete(e.id)}#S(){for(let[e,t]of this.#o)t.particleIds.size===0&&this.#o.delete(e)}async#C(e){let t=ey(e),n=this.#l.get(t);if(n!==void 0)return n;let r=this.#r(e).then(async t=>{if(t===null)throw new ny(`spriteLoadFailed`,`particle sprite ${e.asset} is unavailable`);return await ty(t.bytes,e.contentHash),t.url});this.#l.set(t,r);try{return await r}catch(e){throw this.#l.delete(t),e}}};function Vv(e,t,n,r){return{descriptor:n,spriteUrl:r,key:e,handle:t,randomState:Hv(n.seed),emissionCarry:0,particleIds:new Set}}function Hv(e){let t=Math.trunc(e)>>>0;return t===0?2654435769:t}function Uv(e,t,n){let r=e.randomState;return r^=r<<13,r^=r>>>17,r^=r<<5,e.randomState=r>>>0,t+(n-t)*(e.randomState/4294967296)}function Wv(e,t){if(e.kind===`world`)return e.position;let n=t(e.entity);return n===null?null:[n[0]+e.offset[0],n[1]+e.offset[1],n[2]+e.offset[2]]}function Gv(e){let t=Math.min(1,e.ageSeconds/e.lifetimeSeconds);return{id:e.id,position:[...e.position],size:Kv(e.descriptor.sizeCurve,t),color:qv(e.descriptor.colorCurve,t),frameIndex:e.descriptor.sprite.frameCount===1?0:Math.floor(e.ageSeconds*e.descriptor.flipbookFramesPerSecond)%e.descriptor.sprite.frameCount,frameCount:e.descriptor.sprite.frameCount,spriteUrl:e.spriteUrl}}function Kv(e,t){let[n,r]=Jv(e,t),i=Yv(n.age,r.age,t);return n.value+(r.value-n.value)*i}function qv(e,t){let[n,r]=Jv(e,t),i=Yv(n.age,r.age,t);return[0,1,2,3].map(e=>n.color[e]+(r.color[e]-n.color[e])*i)}function Jv(e,t){for(let n=1;n<e.length;n+=1){let r=e[n];if(t<=r.age)return[e[n-1],r]}return[e[e.length-1],e[e.length-1]]}function Yv(e,t,n){return t===e?0:(n-e)/(t-e)}function Xv(e,t){return{anchor:t.anchor??e.anchor,sprite:t.sprite??e.sprite,ratePerSecond:t.ratePerSecond??e.ratePerSecond,burstCount:t.burstCount??e.burstCount,lifetimeSeconds:t.lifetimeSeconds??e.lifetimeSeconds,velocityMin:t.velocityMin??e.velocityMin,velocityMax:t.velocityMax??e.velocityMax,acceleration:t.acceleration??e.acceleration,sizeCurve:t.sizeCurve??e.sizeCurve,colorCurve:t.colorCurve??e.colorCurve,flipbookFramesPerSecond:t.flipbookFramesPerSecond??e.flipbookFramesPerSecond,seed:e.seed,maxParticles:t.maxParticles??e.maxParticles,visible:t.visible??e.visible}}function Zv(e){return e.op===`emit`?null:e.handle}function Qv(e,t,n,r){return{code:e,sequence:t.sequence,handle:n,message:r}}function $v(e,t){return{code:e,sequence:0,handle:null,message:t}}function ey(e){return`${e.asset}:${e.contentHash}`}async function ty(e,t){let n=await Dg(e,t).catch(e=>{throw new ny(`contentHashMismatch`,e instanceof Error?e.message:String(e))});if(n!==t)throw new ny(`contentHashMismatch`,`particle sprite hash ${n} does not match ${t}`)}var ny=class extends Error{code;constructor(e,t){super(t),this.code=e}};f_();var ry=new Set([`renderHandleCount`,`drawCallCount`,`geometryResourceCount`,`materialResourceCount`,`textureResourceCount`,`animatedInstanceCount`,`triangleCount`]),iy=[`entityCount`,`activeCapabilityCount`,`residentChunkCount`,`dirtyChunkCount`,`renderDiffCount`,`renderHandleCount`,`drawCallCount`,`geometryResourceCount`,`materialResourceCount`,`textureResourceCount`,`animatedInstanceCount`,`triangleCount`,`activeAudioSourceCount`,`activeBillboardCount`,`activeParticleCount`,`droppedFeedbackCount`],ay=new Set(iy.filter(e=>!ry.has(e))),oy=[`mount`,`animationFrame`,`explicit`,`cameraReset`],sy=[`available`,`firstFrame`,`sourceTimeRegressed`,`sourceTimeGapExceeded`],cy=[`available`,`clockUnavailable`,`clockRegressed`,`durationExceeded`],ly=class{#e;#t;#n=[];#r=0;#i=null;constructor(e){this.#e=new Set(e.expectedCounters),this.#t=yy(e.maxFrameTimeSamples??60,1,240,`maxFrameTimeSamples`)}sample(e){return this.#a({sourceTick:e.sourceTick,durations:[{counter:`frameTimeMs`,value:e.frameTimeMs,unavailableMessage:null}],counters:e.counters})}sampleSurface(e){return hy(e.timing),o_(e.timing.statistics),dy(e.counters),this.#a({sourceTick:e.sourceTick,durations:[my(`frameTimeMs`,e.timing.frameIntervalMs,e.timing.frameIntervalStatus),my(`backendSubmissionDurationMs`,e.timing.backendSubmissionDurationMs,e.timing.backendSubmissionDurationStatus)],counters:{...e.counters,...uy(e.timing)}})}#a(e){if(!Number.isSafeInteger(e.sourceTick)||e.sourceTick<0)throw Error(`sourceTick must be a non-negative safe integer`);let t=[],n=[];for(let r of e.durations){if(r.value===null){t.push({code:`counterUnavailable`,counter:r.counter,message:r.unavailableMessage??`${r.counter} is unavailable`});continue}if(!vy(r.value)){t.push({code:`invalidSample`,counter:r.counter,message:`${r.counter} must be finite and non-negative`});continue}r.counter===`frameTimeMs`&&(this.#n.push(r.value),this.#n.length>this.#t&&this.#n.splice(0,this.#n.length-this.#t)),n.push(_y(r.counter,r.value,`durationMs`,`ms`))}for(let r of iy){let i=e.counters[r];if(i==null){this.#e.has(r)&&t.push({code:`counterUnavailable`,counter:r,message:`${r} is unavailable from the current owner adapter`});continue}if(!vy(i)){t.push({code:`invalidSample`,counter:r,message:`${r} must be finite and non-negative`});continue}n.push(_y(r,i,`gauge`,`count`))}return this.#r+=1,this.#i={schemaVersion:1,sourceTick:e.sourceTick,sampleSequence:this.#r,metrics:n,frameTimeHistoryMs:[...this.#n],diagnostics:t},this.readSnapshot()}readSnapshot(){if(this.#i===null)throw Error(`live telemetry has not sampled any owner counters`);return{...this.#i,metrics:[...this.#i.metrics],frameTimeHistoryMs:[...this.#i.frameTimeHistoryMs],diagnostics:[...this.#i.diagnostics]}}tryReadSnapshot(){return this.#i===null?null:this.readSnapshot()}};function uy(e){let t=e.statistics;return{drawCallCount:fy(t.drawCallCount),renderHandleCount:fy(t.renderHandleCount),geometryResourceCount:fy(t.geometryResourceCount),materialResourceCount:fy(t.materialResourceCount),textureResourceCount:fy(t.textureResourceCount),animatedInstanceCount:fy(t.animatedInstanceCount),triangleCount:fy(t.triangleCount)}}function dy(e){if(typeof e!=`object`||!e||Array.isArray(e))throw Error(`renderer surface product counters must be an object`);for(let t of Object.keys(e))if(!ay.has(t))throw Error(`renderer surface telemetry counter ${t} is not product-owned`)}function fy(e){return e.status===`available`?e.value:null}var py=class{#e;#t;#n=new Map;#r=[];#i=0;constructor(e){this.#e=e.collector,this.#t=e.sink}applyPresentation(e){let t=[],n=0;for(let r of e.ops){if(r.domain!==`telemetryOverlay`)continue;let e=this.#o(r);e===null?n+=1:(t.push(e),this.#r.push(e))}return{applied:n,diagnostics:t,readout:this.readout()}}sample(e,t){if(!Number.isFinite(t)||t<0)throw Error(`elapsedMs must be finite and non-negative`);return this.#a(this.#e.sample(e),t)}sampleSurface(e,t){if(!Number.isFinite(t)||t<0)throw Error(`elapsedMs must be finite and non-negative`);return this.#a(this.#e.sampleSurface(e),t)}#a(e,t){for(let[n,r]of this.#n)r.descriptor.visible&&(r.lastRenderedMs===null||t-r.lastRenderedMs>=r.descriptor.refreshIntervalMs)&&(this.#t.render(n,r.descriptor,e),r.lastRenderedMs=t,this.#i+=1);return e}setVisible(e,t){let n=this.#n.get(e);return n===void 0?!1:(n.descriptor={...n.descriptor,visible:t},n.lastRenderedMs=null,this.#t.render(e,n.descriptor,this.#e.tryReadSnapshot()),!0)}toggleVisible(e){let t=this.#n.get(e);if(t===void 0)return null;let n=!t.descriptor.visible;return this.setVisible(e,n),n}readout(){return{activeOverlays:this.#n.size,renderedSnapshots:this.#i,diagnostics:[...this.#r]}}cleanup(){for(let e of this.#n.keys())this.#t.destroy(e);this.#n.clear()}#o(e){let t=e.op.handle;try{if(e.op.op===`create`)return this.#n.has(t)?xy(e,`duplicateHandle`,`overlay handle is already active`):(this.#n.set(t,{descriptor:e.op.descriptor,lastRenderedMs:null}),this.#t.render(e.op.handle,e.op.descriptor,this.#e.tryReadSnapshot()),null);let n=this.#n.get(t);return n===void 0?xy(e,`unknownHandle`,`overlay handle is not active`):(e.op.op===`update`?(n.descriptor=by(n.descriptor,e.op.patch),n.lastRenderedMs=null,this.#t.render(e.op.handle,n.descriptor,this.#e.tryReadSnapshot())):(this.#n.delete(t),this.#t.destroy(e.op.handle)),null)}catch(t){return xy(e,`hostFailure`,t instanceof Error?t.message:String(t))}}};function my(e,t,n){return{counter:e,value:t,unavailableMessage:t===null?`${e} is unavailable because renderer surface timing status is ${n}`:null}}function hy(e){if(e.schemaVersion!==1)throw Error(`renderer surface timing schemaVersion must be 1`);if(!Number.isSafeInteger(e.renderSequence)||e.renderSequence<1)throw Error(`renderer surface timing renderSequence must be a positive safe integer`);if(!oy.includes(e.source))throw Error(`renderer surface timing source is unsupported`);if(!Number.isFinite(e.sourceTimeMs)||e.sourceTimeMs<0||e.sourceTimeMs>2**53-1)throw Error(`renderer surface timing sourceTimeMs is outside the supported range`);if(!sy.includes(e.frameIntervalStatus))throw Error(`renderer surface frameIntervalStatus is unsupported`);if(!cy.includes(e.backendSubmissionDurationStatus))throw Error(`renderer surface backendSubmissionDurationStatus is unsupported`);gy(e.frameIntervalMs,e.frameIntervalStatus===`available`,`frameIntervalMs`),gy(e.backendSubmissionDurationMs,e.backendSubmissionDurationStatus===`available`,`backendSubmissionDurationMs`)}function gy(e,t,n){if(t!==(e!==null)||e!==null&&(!vy(e)||e>6e4))throw Error(`renderer surface timing ${n} does not match its availability status`)}function _y(e,t,n,r){return{counter:e,kind:n,value:t,unit:r}}function vy(e){return Number.isFinite(e)&&e>=0}function yy(e,t,n,r){if(!Number.isInteger(e)||e<t||e>n)throw Error(`${r} must be an integer between ${t} and ${n}`);return e}function by(e,t){return{title:t.title??e.title,corner:t.corner??e.corner,refreshIntervalMs:t.refreshIntervalMs??e.refreshIntervalMs,maxFrameTimeSamples:t.maxFrameTimeSamples??e.maxFrameTimeSamples,visible:t.visible??e.visible}}function xy(e,t,n){return{code:t,sequence:e.meta.sequence,handle:e.op.handle,message:n}}f_(),Lg();var Sy=`rusty_renderer_webview_bridge.v1`;function Cy(){if(globalThis.__rustyEnginePrivateRenderer!==void 0)throw Error(`renderer webview bridge is already installed`);let e=Py(),t=null,n=null,r=null,i=null,a=null,o=null,s=new Set,c=`mounting`,l=null,u=()=>void 0,d=!1,f=!1,p=e=>{let t=JSON.stringify({bridgeVersion:Sy,...e});wy().postMessage(t)},m=()=>{if(c===`failed`)throw Error(`renderer webview bridge mount failed: ${l??`unknown failure`}`);if(c===`disposed`)throw Error(`renderer webview bridge is disposed`);if(c===`mounting`)throw Error(`renderer webview bridge is still mounting`);if(t===null)throw Error(`renderer webview surface is not ready`);return t},h=(e,t,n=null)=>{p({kind:`operationSucceeded`,operation:t,requestId:e,value:n})},g=(e,t,n)=>{p({kind:`operationFailed`,operation:t,requestId:e,message:n instanceof Error?n.message:String(n)})},_=(e,t,n)=>{try{h(e,t,n())}catch(n){g(e,t,n)}},v=(e,t,n)=>{n().then(n=>h(e,t,n),n=>g(e,t,n))},y=async()=>{if(f)return[];f=!0;let e=[];if(d){try{u()}catch(t){e.push(t)}d=!1}try{await n?.dispose()}catch(t){e.push(t)}let c=t=>{try{t()}catch(t){e.push(t)}};c(()=>r?.dispose()),c(()=>i?.dispose()),c(()=>a?.dispose()),c(()=>o?.dispose()),c(()=>t?.dispose()),n=null,r=null,i=null,a=null,o=null,t=null;for(let e of s)URL.revokeObjectURL(e);return s.clear(),e},b=Object.freeze({submitFrame:(e,t)=>_(e,`submitFrame`,()=>m().applyFrame(t)),submitPresentation:(e,t)=>v(e,`submitPresentation`,()=>m().applyPresentation(t)),configureViews:(e,t)=>_(e,`configureViews`,()=>m().configureViews(t)),setCameraPose:(e,t,n)=>_(e,`setCameraPose`,()=>(m().setCameraPose(t,n),m().cameraPose())),pick:(e,t)=>_(e,`pick`,()=>m().pick(t)),readState:e=>_(e,`readState`,()=>Ny(m())),readInput:t=>_(t,`readInput`,()=>Iy(e)),renderOnce:(e,t)=>_(e,`renderOnce`,()=>m().renderOnce(t)),resumeAudio:e=>v(e,`resumeAudio`,async()=>{if(m(),n===null)throw Error(`audio host is unavailable`);return n.resume()}),start:e=>_(e,`start`,()=>(m().start(),Ny(m()))),stop:e=>_(e,`stop`,()=>(m().stop(),Ny(m()))),resize:(e,t,n,r)=>_(e,`resize`,()=>{Ly(t,n,r);let e=m();return e.canvas.style.width=`${String(t)}px`,e.canvas.style.height=`${String(n)}px`,e.canvas.width=Math.max(1,Math.round(t*r)),e.canvas.height=Math.max(1,Math.round(n*r)),e.renderOnce()}),dispose:e=>v(e,`dispose`,async()=>{m(),c=`disposed`;let e=await y();if(e[0]!==void 0)throw e[0];return{disposed:!0}})});Object.defineProperty(globalThis,"__rustyEnginePrivateRenderer",{configurable:!1,enumerable:!1,value:b,writable:!1}),u=Fy(e),d=!0;let x=async()=>{let e=Ty(globalThis.__rustyEngineRendererConfiguration),l=My(`rusty-renderer-canvas`,HTMLCanvasElement),u=My(`rusty-renderer-overlays`,HTMLDivElement),d=new Map(e.resources.map(e=>[e.identity,e])),f=new Map(e.resources.map(e=>[e.contentHash,jy(e.bytesBase64)])),h=e.resources.filter(e=>e.identity.startsWith(`mesh-resource/`)),g=e.resources.filter(e=>e.identity.startsWith(`texture-resource/`)),_={autoStart:!1,...e.clearColor===null?{}:{clearColor:e.clearColor},pixelRatio:e.pixelRatio};t=h.length>0&&g.length>0?await S_(l,{..._,meshResourceManifest:Ey(h),resolveMeshResource:async e=>ky(d,e.resource),textureResourceManifest:Dy(g),resolveTextureResource:async e=>ky(d,e.resource)}):h.length>0?await S_(l,{..._,meshResourceManifest:Ey(h),resolveMeshResource:async e=>ky(d,e.resource)}):g.length>0?await S_(l,{..._,textureResourceManifest:Dy(g),resolveTextureResource:async e=>ky(d,e.resource)}):x_(l,_),n=new hv({resolveResource:async e=>({bytes:Ay(f,e.contentHash),contentHash:e.contentHash})}),r=new kv({container:u,projectWorld:e=>({...m().projectWorldPoint(e),occluded:!1}),resolveEntityPosition:()=>null,resolveResource:async e=>{let t=d.get(e);return t===void 0?null:{bytes:jy(t.bytesBase64)}}}),a=new Y_({container:u,projectWorld:e=>m().projectWorldPoint(e)}),i=new Bv({resolveEntityPosition:()=>null,resolveResource:async e=>{let t=f.get(e.contentHash);if(t===void 0)return null;let n=t.slice(0),r=URL.createObjectURL(new Blob([n],{type:`image/png`}));return s.add(r),{bytes:n,url:r}},sink:a}),o=new X_({container:u});let v=new py({collector:new ly({expectedCounters:[]}),sink:o});t.setPresentationHosts(new Jg({animation:new nv(t.animationProjection),audio:n,billboard:r,particle:i,telemetryOverlay:v})),e.autoStart&&t.start(),c=`ready`,p({kind:`ready`,value:Ny(t)})},S=()=>{x().catch(async e=>{l=e instanceof Error?e.message:String(e),c=`failed`;let t=await y(),n=t[0]===void 0?``:`; cleanup also failed: ${t[0]instanceof Error?t[0].message:String(t[0])}`;p({kind:`mountFailed`,message:`${l}${n}`})})};document.readyState===`loading`?document.addEventListener(`DOMContentLoaded`,S,{once:!0}):S()}function wy(){let e=globalThis.ipc;if(e===void 0||typeof e.postMessage!=`function`)throw Error(`renderer webview IPC is unavailable`);return e}function Ty(e){if(typeof e!=`object`||!e)throw Error(`configuration must be an object`);let t=e;if(typeof t.autoStart!=`boolean`)throw Error(`configuration.autoStart must be boolean`);let n=t.clearColor,r=t.pixelRatio;if(n===void 0||n!==null&&(!Number.isSafeInteger(n)||n<0||n>16777215))throw Error(`configuration.clearColor must be null or an RGB integer`);if(r===void 0||!Number.isFinite(r)||r<=0||r>4)throw Error(`configuration.pixelRatio must be finite and in (0, 4]`);if(!Array.isArray(t.resources)||t.resources.length>1536)throw Error(`configuration.resources must be a bounded array`);let i=new Set,a=t.resources.map((e,t)=>{if(typeof e!=`object`||!e)throw Error(`configuration.resources[${String(t)}] must be an object`);let n=e;if(typeof n.identity!=`string`||n.identity.length===0||i.has(n.identity))throw Error(`configuration.resources[${String(t)}].identity is invalid or duplicated`);if(typeof n.contentHash!=`string`||!/^sha256:[0-9a-f]{64}$/u.test(n.contentHash))throw Error(`configuration.resources[${String(t)}].contentHash is invalid`);if(typeof n.bytesBase64!=`string`||n.bytesBase64.length===0)throw Error(`configuration.resources[${String(t)}].bytesBase64 is invalid`);if(typeof n.mediaType!=`string`||n.mediaType.length===0)throw Error(`configuration.resources[${String(t)}].mediaType is invalid`);return i.add(n.identity),n});return Object.freeze({autoStart:t.autoStart,clearColor:n,pixelRatio:r,resources:Object.freeze(a)})}function Ey(e){return{kind:`rusty_renderer_mesh_resources.v1`,resources:e.map(Oy)}}function Dy(e){return{kind:`rusty_renderer_texture_resources.v1`,resources:e.map(Oy)}}function Oy(e){return{resource:e.identity,contentHash:e.contentHash,byteLength:jy(e.bytesBase64).byteLength}}function ky(e,t){let n=e.get(t);if(n===void 0)throw Error(`resource ${t} is unavailable`);return jy(n.bytesBase64)}function Ay(e,t){let n=e.get(t);if(n===void 0)throw Error(`resource ${t} is unavailable`);return n.slice(0)}function jy(e){let t=atob(e),n=new Uint8Array(t.length);for(let e=0;e<t.length;e+=1)n[e]=t.charCodeAt(e);return n.buffer}function My(e,t){let n=document.getElementById(e);if(!(n instanceof t))throw Error(`required element #${e} is unavailable`);return n}function Ny(e){return Object.freeze({kind:e.kind,backend:e.backend,camera:e.cameraPose(),input:e.inputReadout(),lighting:e.lightingReadout(),movement:e.movementState(),pointerLocked:e.pointerLocked(),submission:e.submission(),timing:e.timing(),views:e.viewCompositionReadout(),visibility:e.visibilityReadout()})}function Py(){return{pressedCodes:new Set,pointerX:0,pointerY:0,pointerButtons:0,wheelDeltaX:0,wheelDeltaY:0}}function Fy(e){let t=t=>{e.pressedCodes.add(t.code)},n=t=>{e.pressedCodes.delete(t.code)},r=()=>{e.pressedCodes.clear(),e.pointerButtons=0},i=t=>{e.pointerX=t.clientX,e.pointerY=t.clientY,e.pointerButtons=t.buttons},a=t=>{e.wheelDeltaX+=t.deltaX,e.wheelDeltaY+=t.deltaY};return globalThis.addEventListener(`keydown`,t),globalThis.addEventListener(`keyup`,n),globalThis.addEventListener(`blur`,r),globalThis.addEventListener(`pointerdown`,i),globalThis.addEventListener(`pointermove`,i),globalThis.addEventListener(`pointerup`,i),globalThis.addEventListener(`wheel`,a,{passive:!0}),()=>{globalThis.removeEventListener(`keydown`,t),globalThis.removeEventListener(`keyup`,n),globalThis.removeEventListener(`blur`,r),globalThis.removeEventListener(`pointerdown`,i),globalThis.removeEventListener(`pointermove`,i),globalThis.removeEventListener(`pointerup`,i),globalThis.removeEventListener(`wheel`,a)}}function Iy(e){let t=Object.freeze({pressedCodes:Object.freeze([...e.pressedCodes].sort()),pointer:Object.freeze({xPixels:e.pointerX,yPixels:e.pointerY,buttons:e.pointerButtons}),wheel:Object.freeze({deltaX:e.wheelDeltaX,deltaY:e.wheelDeltaY})});return e.wheelDeltaX=0,e.wheelDeltaY=0,t}function Ly(e,t,n){if(!Number.isSafeInteger(e)||e<=0||e>16384||!Number.isSafeInteger(t)||t<=0||t>16384||!Number.isFinite(n)||n<=0||n>4)throw Error(`surface size or pixel ratio is invalid`)}Cy()})();