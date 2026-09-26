import * as THREE from 'three';

/** One retained GPU sky primitive. Changing the product clock updates uniforms only. */
export class SkyBlend {
  readonly material = new THREE.ShaderMaterial({
    uniforms: { first: { value: null }, second: { value: null }, amount: { value: 0 } },
    vertexShader: `varying vec3 direction;
      void main() {
        direction = position;
        vec4 clip = projectionMatrix * mat4(mat3(viewMatrix)) * vec4(position, 1.0);
        gl_Position = clip.xyww;
      }`,
    fragmentShader: `uniform sampler2D first; uniform sampler2D second; uniform float amount;
      varying vec3 direction;
      void main() {
        vec3 d = normalize(direction);
        vec2 uv = vec2(atan(d.z, d.x) * 0.15915494309189535 + 0.5,
                       0.5 - asin(clamp(d.y, -1.0, 1.0)) * 0.3183098861837907);
        gl_FragColor = vec4(mix(texture2D(first, uv).rgb, texture2D(second, uv).rgb, amount), 1.0);
        #include <colorspace_fragment>
      }`,
    side: THREE.BackSide,
    depthWrite: false,
    depthFunc: THREE.LessEqualDepth,
    toneMapped: false,
  });
  readonly mesh = new THREE.Mesh(new THREE.BoxGeometry(2, 2, 2), this.material);
  constructor(scene: THREE.Scene) {
    this.mesh.name = 'engine-sky-blend';
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = -Number.MAX_VALUE;
    this.mesh.raycast = () => {};
    scene.add(this.mesh);
  }
  update(first: THREE.Texture, second: THREE.Texture, amount: number): void {
    this.material.uniforms['first']!.value = first;
    this.material.uniforms['second']!.value = second;
    this.material.uniforms['amount']!.value = amount;
    this.mesh.visible = true;
  }
  dispose(): void {
    this.mesh.removeFromParent();
    this.mesh.geometry.dispose();
    this.material.dispose();
  }
}
