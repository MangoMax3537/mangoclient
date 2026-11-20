// Dependency-free WebGL1 renderer for the Minecraft player model, ported from the legacy launcher (src/renderer/js/skin3d.js).
// Cuboid geometry with the 64x64 UV unwrap incl. the second-layer parts (hat, jacket, sleeves, pants), classic (4 px) and slim (3 px)
// arms, nearest-neighbour sampling, one directional light, an idle breathing sway or a walking limb swing, and a cape on the back
// (64x32 cape texture on a 10x16x1 cuboid, standard unwrap). The geometry is pure and unit-tested; the GL class needs a canvas.

export type Vec3 = readonly [number, number, number];
export type PartName =
  | 'head' | 'hat' | 'body' | 'jacket' | 'rightArm' | 'rightSleeve' | 'leftArm' | 'leftSleeve'
  | 'rightLeg' | 'rightPants' | 'leftLeg' | 'leftPants' | 'cape';

export interface PartDef {
  name: PartName;
  /** width, height, depth in model units (1 unit = 1 skin pixel) */
  size: Vec3;
  /** UV origin in texture pixels */
  uv: readonly [number, number];
  pivot: Vec3;
  inflate: number;
  overlay: boolean;
  /** texture size the UV origin refers to */
  tex: readonly [number, number];
}

export const SKIN_TEX = [64, 64] as const;
export const CAPE_TEX = [64, 32] as const;

/** The 12 body parts. Arm width differs between classic (4) and slim (3); the arm hugs the body edge (x = 4) in both cases, so a slim arm's centre is half a unit closer. (The legacy renderer shifted it a further 0.5 into the body: fixed.) */
export function playerParts(slim: boolean): PartDef[] {
  const aw = slim ? 3 : 4;
  const p = (name: PartName, w: number, h: number, d: number, u: number, v: number, x: number, y: number, z: number, inflate: number, overlay: boolean): PartDef =>
    ({ name, size: [w, h, d], uv: [u, v], pivot: [x, y, z], inflate, overlay, tex: SKIN_TEX });
  return [
    p('head', 8, 8, 8, 0, 0, 0, 28, 0, 0, false),
    p('hat', 8, 8, 8, 32, 0, 0, 28, 0, 0.55, true),
    p('body', 8, 12, 4, 16, 16, 0, 18, 0, 0, false),
    p('jacket', 8, 12, 4, 16, 32, 0, 18, 0, 0.3, true),
    p('rightArm', aw, 12, 4, 40, 16, -(4 + aw / 2), 18, 0, 0, false),
    p('rightSleeve', aw, 12, 4, 40, 32, -(4 + aw / 2), 18, 0, 0.28, true),
    p('leftArm', aw, 12, 4, 32, 48, 4 + aw / 2, 18, 0, 0, false),
    p('leftSleeve', aw, 12, 4, 48, 48, 4 + aw / 2, 18, 0, 0.28, true),
    p('rightLeg', 4, 12, 4, 0, 16, -2, 6, 0, 0, false),
    p('rightPants', 4, 12, 4, 0, 32, -2, 6, 0, 0.28, true),
    p('leftLeg', 4, 12, 4, 16, 48, 2, 6, 0, 0, false),
    p('leftPants', 4, 12, 4, 0, 48, 2, 6, 0, 0.28, true),
  ];
}

/** Cape: 10x16x1 on the 64x32 cape texture, hanging from the shoulders behind the body (its outer face looks backwards, see render). */
export const CAPE_PART: PartDef = { name: 'cape', size: [10, 16, 1], uv: [0, 0], pivot: [0, 24, -2.5], inflate: 0, overlay: false, tex: CAPE_TEX };

export type Face = 'front' | 'back' | 'right' | 'left' | 'top' | 'bottom';
export const FACES: readonly Face[] = ['front', 'back', 'right', 'left', 'top', 'bottom'];

/** UV rect (x, y, w, h in texture pixels) of a cuboid face with the standard Minecraft unwrap. */
export function faceUv(face: Face, w: number, h: number, d: number, u: number, v: number): [number, number, number, number] {
  switch (face) {
    case 'front': return [u + d, v + d, w, h];
    case 'back': return [u + d + w + d, v + d, w, h];
    case 'right': return [u, v + d, d, h];
    case 'left': return [u + d + w, v + d, d, h];
    case 'top': return [u + d, v, w, d];
    case 'bottom': return [u + d + w, v, w, d];
  }
}

export interface Geometry { pos: number[]; uvs: number[]; norms: number[]; idx: number[] }

/** Cuboid centred on the origin: 6 faces x 4 vertices, 36 indices, UVs normalised by the texture size. */
export function boxGeometry(w: number, h: number, d: number, u: number, v: number, inflate = 0, tex: readonly [number, number] = SKIN_TEX): Geometry {
  const x = w / 2 + inflate, y = h / 2 + inflate, z = d / 2 + inflate;
  const faces: { n: Vec3; c: Vec3[]; f: Face }[] = [
    { f: 'front', n: [0, 0, 1], c: [[-x, -y, z], [x, -y, z], [x, y, z], [-x, y, z]] },
    { f: 'back', n: [0, 0, -1], c: [[x, -y, -z], [-x, -y, -z], [-x, y, -z], [x, y, -z]] },
    { f: 'right', n: [-1, 0, 0], c: [[-x, -y, -z], [-x, -y, z], [-x, y, z], [-x, y, -z]] },
    { f: 'left', n: [1, 0, 0], c: [[x, -y, z], [x, -y, -z], [x, y, -z], [x, y, z]] },
    { f: 'top', n: [0, 1, 0], c: [[-x, y, z], [x, y, z], [x, y, -z], [-x, y, -z]] },
    { f: 'bottom', n: [0, -1, 0], c: [[-x, -y, -z], [x, -y, -z], [x, -y, z], [-x, -y, z]] },
  ];
  const pos: number[] = [], uvs: number[] = [], norms: number[] = [], idx: number[] = [];
  let base = 0;
  for (const f of faces) {
    const [uu, vv, uw, uh] = faceUv(f.f, w, h, d, u, v);
    const px = (n: number) => n / tex[0], py = (n: number) => n / tex[1];
    const corners = [[px(uu), py(vv + uh)], [px(uu + uw), py(vv + uh)], [px(uu + uw), py(vv)], [px(uu), py(vv)]];
    for (let i = 0; i < 4; i++) { pos.push(...f.c[i]); uvs.push(...corners[i]); norms.push(...f.n); }
    idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
    base += 4;
  }
  return { pos, uvs, norms, idx };
}

export const partGeometry = (p: PartDef): Geometry => boxGeometry(p.size[0], p.size[1], p.size[2], p.uv[0], p.uv[1], p.inflate, p.tex);

/** A legacy skin (64x32, or HD 2:1) has no left limbs / second layer: true when the texture must be expanded to square. */
export const isLegacySkin = (width: number, height: number): boolean => width === height * 2 && width >= 64;

/* ---- tiny mat4 (column-major, same maths as the legacy renderer) ---- */
type M4 = Float32Array;
const identity = (): M4 => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
function perspective(fovy: number, aspect: number, near: number, far: number): M4 {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
}
function translate(m: M4, x: number, y: number, z: number): M4 {
  const o = new Float32Array(m);
  o[12] = m[0] * x + m[4] * y + m[8] * z + m[12];
  o[13] = m[1] * x + m[5] * y + m[9] * z + m[13];
  o[14] = m[2] * x + m[6] * y + m[10] * z + m[14];
  o[15] = m[3] * x + m[7] * y + m[11] * z + m[15];
  return o;
}
function rotateY(m: M4, a: number): M4 {
  const c = Math.cos(a), s = Math.sin(a), o = new Float32Array(m);
  for (let i = 0; i < 4; i++) { const a0 = m[i], a2 = m[i + 8]; o[i] = a0 * c - a2 * s; o[i + 8] = a0 * s + a2 * c; }
  return o;
}
function rotateX(m: M4, a: number): M4 {
  const c = Math.cos(a), s = Math.sin(a), o = new Float32Array(m);
  for (let i = 0; i < 4; i++) { const a1 = m[i + 4], a2 = m[i + 8]; o[i + 4] = a1 * c + a2 * s; o[i + 8] = a2 * c - a1 * s; }
  return o;
}
export const m4 = { identity, perspective, translate, rotateX, rotateY };

const VERT = `
attribute vec3 aPos; attribute vec2 aUV; attribute vec3 aNormal;
uniform mat4 uProj; uniform mat4 uView; uniform mat4 uModel;
varying vec2 vUV; varying float vLight;
void main() {
  vec3 n = normalize(mat3(uModel) * aNormal);
  float diff = max(dot(n, normalize(vec3(-0.45, 0.75, 0.9))), 0.0);
  vLight = 0.62 + 0.38 * diff;
  vUV = aUV;
  gl_Position = uProj * uView * uModel * vec4(aPos, 1.0);
}`;
const FRAG = `
precision mediump float;
varying vec2 vUV; varying float vLight; uniform sampler2D uTex;
void main() {
  vec4 c = texture2D(uTex, vUV);
  if (c.a < 0.02) discard;
  gl_FragColor = vec4(c.rgb * vLight, c.a);
}`;

export type Pose = 'idle' | 'walking';
export type TextureSource = HTMLImageElement | ImageBitmap | HTMLCanvasElement;

interface GlPart { def: PartDef; pos: WebGLBuffer; uv: WebGLBuffer; norm: WebGLBuffer; idx: WebGLBuffer; count: number }

function compile(gl: WebGLRenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type);
  if (!sh) throw new Error('WebGL: no shader');
  gl.shaderSource(sh, src); gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(`Shader failed: ${gl.getShaderInfoLog(sh)}`);
  return sh;
}

/** Square 64x64 (or HD) skin from a legacy 2:1 skin: the right limbs are mirrored into the left slots. */
export function expandLegacy(src: TextureSource, doc: Document = document): HTMLCanvasElement {
  const w = (src as { width: number }).width;
  const s = w / 64;
  const c = doc.createElement('canvas');
  c.width = w; c.height = w;
  const ctx = c.getContext('2d');
  if (!ctx) return c;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(src, 0, 0);
  const mirror = (sx: number, sy: number, bw: number, bh: number, dx: number, dy: number) => {
    ctx.save();
    ctx.translate((dx + bw) * s, dy * s); ctx.scale(-1, 1);
    ctx.drawImage(c, sx * s, sy * s, bw * s, bh * s, 0, 0, bw * s, bh * s);
    ctx.restore();
  };
  mirror(0, 16, 16, 16, 16, 48);
  mirror(40, 16, 16, 16, 32, 48);
  return c;
}

export const IDLE_ROTATION = 0.5;
export const MAX_PITCH = 0.5;

export class SkinRenderer {
  readonly gl: WebGLRenderingContext;
  slim: boolean;
  rotation = IDLE_ROTATION;
  pitch = 0;
  autoRotate = true;
  pose: Pose = 'idle';
  time = 0;
  ready = false;
  destroyed = false;
  active = true;
  /** Called when the GL context is lost (the viewer then switches to the flat fallback). */
  onLost: (() => void) | null = null;

  #prog: WebGLProgram;
  #loc: { aPos: number; aUV: number; aNormal: number; uProj: WebGLUniformLocation | null; uView: WebGLUniformLocation | null; uModel: WebGLUniformLocation | null; uTex: WebGLUniformLocation | null };
  #skinTex: WebGLTexture | null;
  #capeTex: WebGLTexture | null = null;
  #hasCape = false;
  #parts: GlPart[] = [];
  #cape: GlPart | null = null;
  #frame = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #last = 0;
  #cleanup: (() => void) | null = null;

  constructor(readonly canvas: HTMLCanvasElement, opts: { slim?: boolean; rotation?: number } = {}) {
    const gl = canvas.getContext('webgl', { alpha: true, antialias: true, premultipliedAlpha: false });
    if (!gl) throw new Error('WebGL is not available');
    this.gl = gl;
    this.slim = !!opts.slim;
    this.rotation = opts.rotation ?? IDLE_ROTATION;
    const prog = gl.createProgram();
    if (!prog) throw new Error('WebGL: no program');
    gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, VERT));
    gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, FRAG));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(`Program link failed: ${gl.getProgramInfoLog(prog)}`);
    gl.useProgram(prog);
    this.#prog = prog;
    this.#loc = {
      aPos: gl.getAttribLocation(prog, 'aPos'), aUV: gl.getAttribLocation(prog, 'aUV'), aNormal: gl.getAttribLocation(prog, 'aNormal'),
      uProj: gl.getUniformLocation(prog, 'uProj'), uView: gl.getUniformLocation(prog, 'uView'), uModel: gl.getUniformLocation(prog, 'uModel'), uTex: gl.getUniformLocation(prog, 'uTex'),
    };
    gl.enable(gl.DEPTH_TEST); gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.enable(gl.CULL_FACE); gl.cullFace(gl.BACK);
    this.#skinTex = gl.createTexture();
    this.#buildParts();
    this.#bindInput();
    this.#queue();
  }

  #makePart(def: PartDef): GlPart {
    const gl = this.gl;
    const g = partGeometry(def);
    const mk = (target: number, data: ArrayBufferView): WebGLBuffer => {
      const b = gl.createBuffer();
      if (!b) throw new Error('WebGL: no buffer');
      gl.bindBuffer(target, b); gl.bufferData(target, data, gl.STATIC_DRAW);
      return b;
    };
    return {
      def, count: g.idx.length,
      pos: mk(gl.ARRAY_BUFFER, new Float32Array(g.pos)), uv: mk(gl.ARRAY_BUFFER, new Float32Array(g.uvs)),
      norm: mk(gl.ARRAY_BUFFER, new Float32Array(g.norms)), idx: mk(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(g.idx)),
    };
  }
  #freePart(p: GlPart): void { const gl = this.gl; gl.deleteBuffer(p.pos); gl.deleteBuffer(p.uv); gl.deleteBuffer(p.norm); gl.deleteBuffer(p.idx); }
  #buildParts(): void {
    for (const p of this.#parts) this.#freePart(p);
    this.#parts = playerParts(this.slim).map((d) => this.#makePart(d));
    if (!this.#cape) this.#cape = this.#makePart(CAPE_PART);
  }

  setSlim(slim: boolean): void { if (this.slim !== slim) { this.slim = slim; this.#buildParts(); } }
  setPose(p: Pose): void { this.pose = p; }

  #upload(tex: WebGLTexture | null, src: TextureSource): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  }
  /** Throws (SecurityError) when the image is cross-origin without CORS: the caller then uses the flat fallback. */
  setSkin(src: TextureSource): void {
    const w = (src as { width: number }).width, h = (src as { height: number }).height;
    this.#upload(this.#skinTex, isLegacySkin(w, h) ? expandLegacy(src) : src);
    this.ready = true;
  }
  setCape(src: TextureSource | null): void {
    if (!src) { this.#hasCape = false; return; }
    if (!this.#capeTex) this.#capeTex = this.gl.createTexture();
    this.#upload(this.#capeTex, src);
    this.#hasCape = true;
  }
  get hasCape(): boolean { return this.#hasCape; }

  resetView(): void { this.rotation = IDLE_ROTATION; this.pitch = 0; this.autoRotate = true; }

  #bindInput(): void {
    const c = this.canvas;
    let drag = false, lx = 0, ly = 0;
    const down = (e: PointerEvent) => { drag = true; this.autoRotate = false; lx = e.clientX; ly = e.clientY; c.style.cursor = 'grabbing'; try { c.setPointerCapture(e.pointerId); } catch { /* synthetic */ } };
    const move = (e: PointerEvent) => {
      if (!drag) return;
      this.rotation += (e.clientX - lx) * 0.012;
      this.pitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, this.pitch + (e.clientY - ly) * 0.006));
      lx = e.clientX; ly = e.clientY;
    };
    const up = (e: PointerEvent) => { drag = false; c.style.cursor = 'grab'; try { c.releasePointerCapture(e.pointerId); } catch { /* none */ } };
    const dbl = () => this.resetView();
    const key = (e: KeyboardEvent) => {
      const step = e.key === 'ArrowLeft' ? -0.15 : e.key === 'ArrowRight' ? 0.15 : 0;
      if (step) { e.preventDefault(); this.autoRotate = false; this.rotation += step; }
      else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { e.preventDefault(); this.autoRotate = false; this.pitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, this.pitch + (e.key === 'ArrowUp' ? -0.1 : 0.1))); }
      else if (e.key === 'Home' || e.key === '0') { this.resetView(); }
    };
    const lost = (e: Event) => { e.preventDefault(); this.ready = false; this.onLost?.(); };
    c.addEventListener('pointerdown', down); c.addEventListener('pointermove', move); c.addEventListener('pointerup', up); c.addEventListener('pointercancel', up);
    c.addEventListener('dblclick', dbl); c.addEventListener('keydown', key); c.addEventListener('webglcontextlost', lost);
    c.style.cursor = 'grab'; c.style.touchAction = 'none';
    this.#cleanup = () => {
      c.removeEventListener('pointerdown', down); c.removeEventListener('pointermove', move); c.removeEventListener('pointerup', up); c.removeEventListener('pointercancel', up);
      c.removeEventListener('dblclick', dbl); c.removeEventListener('keydown', key); c.removeEventListener('webglcontextlost', lost);
    };
  }

  /** DPR-aware size from the CSS box; false while the canvas has no size. */
  #resize(): boolean {
    const dpr = Math.min(globalThis.devicePixelRatio || 1, 2);
    const w = Math.floor(this.canvas.clientWidth * dpr), h = Math.floor(this.canvas.clientHeight * dpr);
    if (w === 0 || h === 0) return false;
    if (this.canvas.width !== w || this.canvas.height !== h) { this.canvas.width = w; this.canvas.height = h; }
    this.gl.viewport(0, 0, w, h);
    return true;
  }

  #queue(delay = 0): void {
    if (this.destroyed || !this.active || this.#frame || this.#timer) return;
    const go = () => { this.#timer = null; this.#frame = requestAnimationFrame(this.#tick); };
    if (delay) this.#timer = setTimeout(go, delay); else go();
  }
  #tick = (now: number): void => this.#frameLoop(now);
  #frameLoop(now: number): void {
    this.#frame = 0;
    if (this.destroyed || !this.active) return;
    this.#queue(1000 / 30);
    if (!this.ready || !this.#resize()) return;
    const dt = this.#last ? Math.min(0.1, (now - this.#last) / 1000) : 0;
    this.#last = now;
    this.render(dt);
  }

  /** One frame; `dt` seconds advance the animation clock. */
  render(dt = 0): void {
    const gl = this.gl, L = this.#loc;
    this.time += dt;
    if (this.autoRotate) this.rotation += dt * 0.35;
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.uniformMatrix4fv(L.uProj, false, perspective(Math.PI / 5, this.canvas.width / this.canvas.height, 0.1, 200));
    gl.uniformMatrix4fv(L.uView, false, translate(identity(), 0, -17.5, -70));
    gl.activeTexture(gl.TEXTURE0);
    gl.uniform1i(L.uTex, 0);

    const walking = this.pose === 'walking';
    const swing = walking ? Math.sin(this.time * 6) * 0.9 : Math.sin(this.time * 1.2) * 0.07;
    const bob = walking ? Math.abs(Math.sin(this.time * 6)) * 0.5 : 0;
    const root = rotateX(rotateY(identity(), this.rotation), this.pitch);

    const place = (name: PartName, pivot: Vec3): M4 => {
      let m = translate(root, 0, bob, 0);
      const [px, py, pz] = pivot;
      if (/^right(Arm|Sleeve)$/.test(name)) { m = translate(m, px, py + 6, pz); m = rotateX(m, swing); m = translate(m, 0, -6, 0); }
      else if (/^left(Arm|Sleeve)$/.test(name)) { m = translate(m, px, py + 6, pz); m = rotateX(m, -swing); m = translate(m, 0, -6, 0); }
      else if (/Leg|Pants/.test(name)) { const dir = name.startsWith('right') ? -1 : 1; m = translate(m, px, py + 6, pz); m = rotateX(m, walking ? swing * dir : 0); m = translate(m, 0, -6, 0); }
      else if (name === 'head' || name === 'hat') { m = translate(m, px, py, pz); m = rotateY(m, Math.sin(this.time * 0.5) * 0.12); }
      else m = translate(m, px, py, pz);
      return m;
    };
    const draw = (p: GlPart, model: M4) => {
      gl.uniformMatrix4fv(L.uModel, false, model);
      const bind = (buf: WebGLBuffer, loc: number, size: number) => { gl.bindBuffer(gl.ARRAY_BUFFER, buf); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, size, gl.FLOAT, false, 0, 0); };
      bind(p.pos, L.aPos, 3); bind(p.uv, L.aUV, 2); bind(p.norm, L.aNormal, 3);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, p.idx);
      gl.drawElements(gl.TRIANGLES, p.count, gl.UNSIGNED_SHORT, 0);
    };

    // solid parts and the cape first, then the alpha overlay layer, so blending is correct
    gl.bindTexture(gl.TEXTURE_2D, this.#skinTex);
    for (const p of this.#parts) if (!p.def.overlay) draw(p, place(p.def.name, p.def.pivot));
    if (this.#hasCape && this.#cape && this.#capeTex) {
      // hangs from the shoulders, leans back a little; the outer face (front of the cuboid) is turned to the back of the player
      const lean = walking ? 0.28 + Math.abs(Math.sin(this.time * 6)) * 0.22 : 0.09 + Math.sin(this.time * 1.2) * 0.03;
      let m = translate(root, 0, bob, 0);
      m = translate(m, 0, 24, -2.5);
      m = rotateX(m, lean);
      m = translate(m, 0, -8, 0);
      m = rotateY(m, Math.PI);
      gl.bindTexture(gl.TEXTURE_2D, this.#capeTex);
      draw(this.#cape, m);
      gl.bindTexture(gl.TEXTURE_2D, this.#skinTex);
    }
    for (const p of this.#parts) if (p.def.overlay) draw(p, place(p.def.name, p.def.pivot));
  }

  setActive(on: boolean): void {
    this.active = on;
    if (!on) { if (this.#frame) cancelAnimationFrame(this.#frame); if (this.#timer) clearTimeout(this.#timer); this.#frame = 0; this.#timer = null; return; }
    this.#last = 0; this.#queue();
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    this.setActive(false);
    this.#cleanup?.();
    for (const p of this.#parts) this.#freePart(p);
    if (this.#cape) this.#freePart(this.#cape);
    const gl = this.gl;
    if (this.#skinTex) gl.deleteTexture(this.#skinTex);
    if (this.#capeTex) gl.deleteTexture(this.#capeTex);
    gl.deleteProgram(this.#prog);
    gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}

export function webglAvailable(doc: Document = document): boolean {
  try { const c = doc.createElement('canvas'); return !!(c.getContext('webgl') || c.getContext('experimental-webgl')); } catch { return false; }
}
