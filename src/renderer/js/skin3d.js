/**
 * Dependency-free WebGL renderer for the Minecraft player model.
 * Handles classic + slim arms, the transparent overlay ("layer 2") parts and
 * capes, with drag-to-rotate and a gentle idle animation.
 */
(function () {
  'use strict';

  const VERT = `
    attribute vec3 aPos;
    attribute vec2 aUV;
    attribute vec3 aNormal;
    uniform mat4 uProj;
    uniform mat4 uView;
    uniform mat4 uModel;
    varying vec2 vUV;
    varying float vLight;
    void main() {
      vec3 n = normalize(mat3(uModel) * aNormal);
      // Fixed key light from the upper front-left, plus ambient fill.
      float diff = max(dot(n, normalize(vec3(-0.45, 0.75, 0.9))), 0.0);
      vLight = 0.62 + 0.38 * diff;
      vUV = aUV;
      gl_Position = uProj * uView * uModel * vec4(aPos, 1.0);
    }`;

  const FRAG = `
    precision mediump float;
    varying vec2 vUV;
    varying float vLight;
    uniform sampler2D uTex;
    void main() {
      vec4 c = texture2D(uTex, vUV);
      if (c.a < 0.02) discard;
      gl_FragColor = vec4(c.rgb * vLight, c.a);
    }`;

  const TEX = 64;

  /** Minecraft's cuboid UV unwrap, in skin pixels. */
  function boxGeometry(w, h, d, u, v, inflate = 0) {
    const x = w / 2 + inflate;
    const y = h / 2 + inflate;
    const z = d / 2 + inflate;

    const px = (n) => n / TEX;
    const faces = [
      // [normal, corners(4), uv rect]
      { n: [0, 0, 1], c: [[-x, -y, z], [x, -y, z], [x, y, z], [-x, y, z]], uv: [u + d, v + d, w, h] },        // front
      { n: [0, 0, -1], c: [[x, -y, -z], [-x, -y, -z], [-x, y, -z], [x, y, -z]], uv: [u + d + w + d, v + d, w, h] }, // back
      { n: [-1, 0, 0], c: [[-x, -y, -z], [-x, -y, z], [-x, y, z], [-x, y, -z]], uv: [u, v + d, d, h] },        // right
      { n: [1, 0, 0], c: [[x, -y, z], [x, -y, -z], [x, y, -z], [x, y, z]], uv: [u + d + w, v + d, d, h] },     // left
      { n: [0, 1, 0], c: [[-x, y, z], [x, y, z], [x, y, -z], [-x, y, -z]], uv: [u + d, v, w, d] },             // top
      { n: [0, -1, 0], c: [[-x, -y, -z], [x, -y, -z], [x, -y, z], [-x, -y, z]], uv: [u + d + w, v, w, d] },    // bottom
    ];

    const pos = [], uvs = [], norms = [], idx = [];
    let base = 0;
    for (const f of faces) {
      const [uu, vv, uw, uh] = f.uv;
      const corners = [
        [px(uu), px(vv + uh)],
        [px(uu + uw), px(vv + uh)],
        [px(uu + uw), px(vv)],
        [px(uu), px(vv)],
      ];
      for (let i = 0; i < 4; i++) {
        pos.push(...f.c[i]);
        uvs.push(...corners[i]);
        norms.push(...f.n);
      }
      idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
      base += 4;
    }
    return { pos, uvs, norms, idx };
  }

  // ---- tiny mat4 ----------------------------------------------------------

  const m4 = {
    identity: () => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
    perspective(fovy, aspect, near, far) {
      const f = 1 / Math.tan(fovy / 2);
      const nf = 1 / (near - far);
      return new Float32Array([
        f / aspect, 0, 0, 0,
        0, f, 0, 0,
        0, 0, (far + near) * nf, -1,
        0, 0, 2 * far * near * nf, 0,
      ]);
    },
    translate(m, x, y, z) {
      const o = new Float32Array(m);
      o[12] = m[0] * x + m[4] * y + m[8] * z + m[12];
      o[13] = m[1] * x + m[5] * y + m[9] * z + m[13];
      o[14] = m[2] * x + m[6] * y + m[10] * z + m[14];
      o[15] = m[3] * x + m[7] * y + m[11] * z + m[15];
      return o;
    },
    rotateY(m, a) {
      const c = Math.cos(a), s = Math.sin(a);
      const o = new Float32Array(m);
      for (let i = 0; i < 4; i++) {
        const a0 = m[i], a2 = m[i + 8];
        o[i] = a0 * c - a2 * s;
        o[i + 8] = a0 * s + a2 * c;
      }
      return o;
    },
    rotateX(m, a) {
      const c = Math.cos(a), s = Math.sin(a);
      const o = new Float32Array(m);
      for (let i = 0; i < 4; i++) {
        const a1 = m[i + 4], a2 = m[i + 8];
        o[i + 4] = a1 * c + a2 * s;
        o[i + 8] = a2 * c - a1 * s;
      }
      return o;
    },
    rotateZ(m, a) {
      const c = Math.cos(a), s = Math.sin(a);
      const o = new Float32Array(m);
      for (let i = 0; i < 4; i++) {
        const a0 = m[i], a1 = m[i + 4];
        o[i] = a0 * c + a1 * s;
        o[i + 4] = a1 * c - a0 * s;
      }
      return o;
    },
    scale(m, x, y, z) {
      const o = new Float32Array(m);
      for (let i = 0; i < 4; i++) { o[i] *= x; o[i + 4] *= y; o[i + 8] *= z; }
      return o;
    },
  };

  function compile(gl, type, src) {
    const sh = gl.createShader(type);
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      throw new Error(`Shader failed: ${gl.getShaderInfoLog(sh)}`);
    }
    return sh;
  }

  class SkinViewer {
    constructor(canvas, opts = {}) {
      this.canvas = canvas;
      this.gl = canvas.getContext('webgl', { alpha: true, antialias: true, premultipliedAlpha: false });
      if (!this.gl) throw new Error('WebGL is not available');
      this.slim = Boolean(opts.slim);
      this.rotation = opts.rotation ?? 0.5;
      this.pitch = 0;
      this.autoRotate = opts.autoRotate !== false;
      this.walking = false;
      this.time = 0;
      this.destroyed = false;
      this._setup();
      this._bindInput();
      this._loop = this._loop.bind(this);
      requestAnimationFrame(this._loop);
    }

    _setup() {
      const gl = this.gl;
      const prog = gl.createProgram();
      gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, VERT));
      gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, FRAG));
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
        throw new Error(`Program link failed: ${gl.getProgramInfoLog(prog)}`);
      }
      gl.useProgram(prog);
      this.prog = prog;

      this.loc = {
        aPos: gl.getAttribLocation(prog, 'aPos'),
        aUV: gl.getAttribLocation(prog, 'aUV'),
        aNormal: gl.getAttribLocation(prog, 'aNormal'),
        uProj: gl.getUniformLocation(prog, 'uProj'),
        uView: gl.getUniformLocation(prog, 'uView'),
        uModel: gl.getUniformLocation(prog, 'uModel'),
        uTex: gl.getUniformLocation(prog, 'uTex'),
      };

      gl.enable(gl.DEPTH_TEST);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      gl.enable(gl.CULL_FACE);
      gl.cullFace(gl.BACK);

      this.texture = gl.createTexture();
      this._buildParts();
    }

    /** Rebuild geometry: arm width differs between classic (4) and slim (3). */
    _buildParts() {
      const armW = this.slim ? 3 : 4;
      const armOffset = this.slim ? 0.5 : 0;

      // [name, w,h,d, u,v, pivotX,pivotY,pivotZ, inflate, overlay?]
      const defs = [
        ['head', 8, 8, 8, 0, 0, 0, 28, 0, 0, false],
        ['hat', 8, 8, 8, 32, 0, 0, 28, 0, 0.55, true],
        ['body', 8, 12, 4, 16, 16, 0, 18, 0, 0, false],
        ['jacket', 8, 12, 4, 16, 32, 0, 18, 0, 0.3, true],
        ['rightArm', armW, 12, 4, 40, 16, -(4 + armW / 2) + armOffset, 18, 0, 0, false],
        ['rightSleeve', armW, 12, 4, 40, 32, -(4 + armW / 2) + armOffset, 18, 0, 0.28, true],
        ['leftArm', armW, 12, 4, 32, 48, (4 + armW / 2) - armOffset, 18, 0, 0, false],
        ['leftSleeve', armW, 12, 4, 48, 48, (4 + armW / 2) - armOffset, 18, 0, 0.28, true],
        ['rightLeg', 4, 12, 4, 0, 16, -2, 6, 0, 0, false],
        ['rightPants', 4, 12, 4, 0, 32, -2, 6, 0, 0.28, true],
        ['leftLeg', 4, 12, 4, 16, 48, 2, 6, 0, 0, false],
        ['leftPants', 4, 12, 4, 0, 48, 2, 6, 0, 0.28, true],
      ];

      const gl = this.gl;
      this.parts = defs.map(([name, w, h, d, u, v, px, py, pz, inflate, overlay]) => {
        const geo = boxGeometry(w, h, d, u, v, inflate);
        const buffers = {
          pos: gl.createBuffer(), uv: gl.createBuffer(), norm: gl.createBuffer(), idx: gl.createBuffer(),
        };
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.pos);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(geo.pos), gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.uv);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(geo.uvs), gl.STATIC_DRAW);
        gl.bindBuffer(gl.ARRAY_BUFFER, buffers.norm);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(geo.norms), gl.STATIC_DRAW);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffers.idx);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(geo.idx), gl.STATIC_DRAW);
        return { name, buffers, count: geo.idx.length, pivot: [px, py, pz], overlay, size: [w, h, d] };
      });
    }

    setSlim(slim) {
      if (this.slim === slim) return;
      this.slim = slim;
      this._buildParts();
    }

    /** `src` is a data: URL or any same-origin image source. */
    setSkin(src, slim) {
      if (slim !== undefined) this.setSlim(slim);
      const img = new Image();
      img.onload = () => {
        const gl = this.gl;
        // Legacy 64x32 skins have no second layer or left limbs; upscale so the
        // shared UV table still points at the right pixels.
        const source = img.height === 32 ? this._expandLegacy(img) : img;
        gl.bindTexture(gl.TEXTURE_2D, this.texture);
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        this.ready = true;
      };
      img.onerror = () => { this.ready = false; };
      img.src = src;
    }

    _expandLegacy(img) {
      const c = document.createElement('canvas');
      c.width = 64; c.height = 64;
      const ctx = c.getContext('2d');
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(img, 0, 0);
      // Mirror the right arm/leg into the left slots.
      const copy = (sx, sy, w, h, dx, dy) => {
        const data = ctx.getImageData(sx, sy, w, h);
        const flipped = ctx.createImageData(w, h);
        for (let y = 0; y < h; y++) {
          for (let x = 0; x < w; x++) {
            const s = (y * w + (w - 1 - x)) * 4;
            const d = (y * w + x) * 4;
            for (let i = 0; i < 4; i++) flipped.data[d + i] = data.data[s + i];
          }
        }
        ctx.putImageData(flipped, dx, dy);
      };
      copy(0, 16, 16, 16, 16, 48);   // right leg -> left leg
      copy(40, 16, 16, 16, 32, 48);  // right arm -> left arm
      return c;
    }

    _bindInput() {
      let dragging = false;
      let lastX = 0, lastY = 0;
      const down = (e) => {
        dragging = true;
        this.autoRotate = false;
        lastX = e.clientX ?? e.touches[0].clientX;
        lastY = e.clientY ?? e.touches[0].clientY;
        this.canvas.style.cursor = 'grabbing';
      };
      const move = (e) => {
        if (!dragging) return;
        const x = e.clientX ?? e.touches?.[0]?.clientX;
        const y = e.clientY ?? e.touches?.[0]?.clientY;
        this.rotation += (x - lastX) * 0.012;
        this.pitch = Math.max(-0.5, Math.min(0.5, this.pitch + (y - lastY) * 0.006));
        lastX = x; lastY = y;
      };
      const up = () => { dragging = false; this.canvas.style.cursor = 'grab'; };

      this.canvas.addEventListener('mousedown', down);
      window.addEventListener('mousemove', move);
      window.addEventListener('mouseup', up);
      this.canvas.addEventListener('touchstart', down, { passive: true });
      window.addEventListener('touchmove', move, { passive: true });
      window.addEventListener('touchend', up);
      this.canvas.addEventListener('dblclick', () => {
        this.rotation = 0.5; this.pitch = 0; this.autoRotate = true;
      });
      this.canvas.style.cursor = 'grab';
      this._cleanup = () => {
        window.removeEventListener('mousemove', move);
        window.removeEventListener('mouseup', up);
        window.removeEventListener('touchmove', move);
        window.removeEventListener('touchend', up);
      };
    }

    _resize() {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const w = Math.floor(this.canvas.clientWidth * dpr);
      const h = Math.floor(this.canvas.clientHeight * dpr);
      if (w === 0 || h === 0) return false;
      if (this.canvas.width !== w || this.canvas.height !== h) {
        this.canvas.width = w;
        this.canvas.height = h;
      }
      this.gl.viewport(0, 0, w, h);
      return true;
    }

    _loop(now) {
      if (this.destroyed) return;
      requestAnimationFrame(this._loop);
      if (!this.ready || !this._resize()) return;

      const dt = this._last ? (now - this._last) / 1000 : 0;
      this._last = now;
      this.time += dt;
      if (this.autoRotate) this.rotation += dt * 0.35;

      const gl = this.gl;
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

      const aspect = this.canvas.width / this.canvas.height;
      gl.uniformMatrix4fv(this.loc.uProj, false, m4.perspective(Math.PI / 5, aspect, 0.1, 200));

      // Frame the 32-unit-tall model with a little headroom.
      let view = m4.identity();
      view = m4.translate(view, 0, -17, -62);
      gl.uniformMatrix4fv(this.loc.uView, false, view);

      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
      gl.uniform1i(this.loc.uTex, 0);

      // Idle: subtle breathing sway; walking: classic limb swing.
      const swing = this.walking ? Math.sin(this.time * 6) * 0.9 : Math.sin(this.time * 1.2) * 0.07;
      const bob = this.walking ? Math.abs(Math.sin(this.time * 6)) * 0.5 : 0;

      const root = m4.rotateX(m4.rotateY(m4.identity(), this.rotation), this.pitch);

      // Solid parts first, then the alpha overlay layer, so blending is correct.
      for (const pass of [false, true]) {
        for (const part of this.parts) {
          if (part.overlay !== pass) continue;
          let model = m4.translate(root, 0, bob, 0);
          const [px, py, pz] = part.pivot;

          if (part.name.startsWith('right') && /Arm|Sleeve/.test(part.name)) {
            model = m4.translate(model, px, py + 6, pz);
            model = m4.rotateX(model, swing);
            model = m4.translate(model, 0, -6, 0);
          } else if (part.name.startsWith('left') && /Arm|Sleeve/.test(part.name)) {
            model = m4.translate(model, px, py + 6, pz);
            model = m4.rotateX(model, -swing);
            model = m4.translate(model, 0, -6, 0);
          } else if (/Leg|Pants/.test(part.name)) {
            const dir = part.name.startsWith('right') ? -1 : 1;
            model = m4.translate(model, px, py + 6, pz);
            model = m4.rotateX(model, this.walking ? swing * dir : 0);
            model = m4.translate(model, 0, -6, 0);
          } else if (part.name === 'head' || part.name === 'hat') {
            model = m4.translate(model, px, py, pz);
            model = m4.rotateY(model, Math.sin(this.time * 0.5) * 0.12);
          } else {
            model = m4.translate(model, px, py, pz);
          }

          gl.uniformMatrix4fv(this.loc.uModel, false, model);
          this._drawPart(part);
        }
      }
    }

    _drawPart(part) {
      const gl = this.gl;
      const bind = (buf, loc, size) => {
        gl.bindBuffer(gl.ARRAY_BUFFER, buf);
        gl.enableVertexAttribArray(loc);
        gl.vertexAttribPointer(loc, size, gl.FLOAT, false, 0, 0);
      };
      bind(part.buffers.pos, this.loc.aPos, 3);
      bind(part.buffers.uv, this.loc.aUV, 2);
      bind(part.buffers.norm, this.loc.aNormal, 3);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, part.buffers.idx);
      gl.drawElements(gl.TRIANGLES, part.count, gl.UNSIGNED_SHORT, 0);
    }

    setWalking(on) { this.walking = on; }

    destroy() {
      this.destroyed = true;
      this._cleanup?.();
    }
  }

  window.SkinViewer = SkinViewer;
})();
