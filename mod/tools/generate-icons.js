'use strict';
const fs = require('fs');
const path = require('path');
const { Resvg } = require(path.join(__dirname, '..', '..', 'node_modules', '@resvg/resvg-js'));

const outDir = path.join(__dirname, '..', 'src', 'main', 'resources', 'assets', 'mangoconfig', 'textures', 'icons');
fs.mkdirSync(outDir, { recursive: true });

// Map internal icon key to Lucide icon name or custom SVG generator
const ICONS = {
  // HUD
  fps: { lucide: 'gauge' },
  ping: { lucide: 'wifi' },
  memory: { lucide: 'cpu' },
  coords: { lucide: 'map-pin' },
  direction: { lucide: 'compass' },
  biome: { lucide: 'tree-pine' },
  worldtime: { lucide: 'sun' },
  server: { lucide: 'server' },
  speed: { lucide: 'wind' },
  armour: { lucide: 'shield' },
  stats: { lucide: 'bar-chart-2' },
  saturation: { lucide: 'apple' },
  potions: { lucide: 'flask-conical' },
  items: { lucide: 'package' },
  cps: { lucide: 'mouse-pointer-click' },
  reach: { lucide: 'maximize-2' },
  combo: { lucide: 'flame' },
  keystrokes: { lucide: 'keyboard' },
  movement: { lucide: 'move' },
  session: { lucide: 'timer' },
  clock: { lucide: 'clock' },

  // Standalone / Visuals
  zoom: { lucide: 'zoom-in' },
  crosshair: { lucide: 'crosshair' },
  no_hurt_cam: { lucide: 'video-off' },
  fullbright: { lucide: 'lightbulb' },
  mango_badge: { lucide: 'badge-check' },
  lower_shield: {
    lucide: 'shield',
    extra: '<path d="M12 7v6m-2.5-2.5L12 13l2.5-2.5"/>',
  },
  no_explosions: {
    lucide: 'bomb',
    slash: true,
  },
  no_totem_pop: {
    lucide: 'sparkles',
    slash: true,
  },
  block_overlay: { lucide: 'box' },
  brightness: { lucide: 'sun-medium' },
  colour_saturation: { lucide: 'palette' },
  custom_chat: { lucide: 'message-square' },
  custom_f3: { lucide: 'terminal' },
  custom_fog: { lucide: 'cloud-fog' },
  hitboxes: { lucide: 'scan' },
  tab_scoreboard: { lucide: 'layout-list' },
  ui_scale: { lucide: 'scaling' },

  // Overlays
  hide_scoreboard: {
    lucide: 'table-properties',
    slash: true,
  },
  hide_boss_bar: {
    lucide: 'crown',
    slash: true,
  },
  hide_effects: {
    lucide: 'sparkles',
    slash: true,
  },
  hide_portal: {
    lucide: 'door-closed',
    slash: true,
  },
  hide_nausea: {
    lucide: 'waves',
    slash: true,
  },
  hide_item_name: {
    lucide: 'tag',
    slash: true,
  },

  // Home screen
  single: { lucide: 'user' },
  multi: { lucide: 'users' },
  options: { lucide: 'settings' },
  globe: { lucide: 'globe' },
  access: { lucide: 'accessibility' },
  realms: { lucide: 'crown' },
  mangoconfig: {
    svg: `<path d="M14 2c0 2-1 4-3 5"/><path d="M12 5c2-2 5-2 6-1s1 4-1 6"/><path d="M12 7c-4.2 0-7 3.5-7 8a7 7 0 0 0 14 0c0-5-2.8-8-7-8z"/>`
  },
  discord: {
    svg: `<path d="M18 6h0a14.5 14.5 0 0 0-4-1.2 9.5 9.5 0 0 0-.4 1.2 12.8 12.8 0 0 0-3.2 0 9.5 9.5 0 0 0-.4-1.2A14.5 14.5 0 0 0 6 6a15.7 15.7 0 0 0-2 9.8 14.6 14.6 0 0 0 4.5 2.2 11 11 0 0 0 1-1.6 9.4 9.4 0 0 1-1.5-.7l.3-.2a10.8 10.8 0 0 0 7.4 0l.3.2a9.4 9.4 0 0 1-1.5.7 11 11 0 0 0 1 1.6 14.6 14.6 0 0 0 4.5-2.2A15.7 15.7 0 0 0 18 6Z"/><circle cx="9.5" cy="11.5" r="1.5"/><circle cx="14.5" cy="11.5" r="1.5"/>`
  }
};

const THEMES = {
  on: '#E89A2C',      // Theme.BRAND
  off: '#C9C1B5',     // Theme.TEXT_2
  hover: '#FFFFFF',   // Theme.TEXT
  brand_hover: '#F6A93C',
  discord: '#5865F2',
  discord_hover: '#7983F5',
};

async function fetchLucide(name) {
  const url = `https://unpkg.com/lucide-static@latest/icons/${name}.svg`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Failed to fetch ${name} (${res.status})`);
  const text = await res.text();
  // Extract content inside <svg ...>(...)</svg>
  const match = text.match(/<svg[^>]*>([\s\S]*?)<\/svg>/i);
  return match ? match[1].trim() : text;
}

async function main() {
  console.log(`Generating high-res Lucide icons for ${Object.keys(ICONS).length} entries...`);
  const cache = new Map();

  for (const [key, conf] of Object.entries(ICONS)) {
    let body = conf.svg;
    if (!body && conf.lucide) {
      if (!cache.has(conf.lucide)) {
        try {
          cache.set(conf.lucide, await fetchLucide(conf.lucide));
        } catch (e) {
          console.error(`Error fetching ${conf.lucide}:`, e.message);
          continue;
        }
      }
      body = cache.get(conf.lucide);
    }
    if (conf.extra) body += conf.extra;
    if (conf.slash) body += '<line x1="2" y1="2" x2="22" y2="22" stroke-width="2.2"/>';

    // Generate color variations
    const variants = {
      on: THEMES.on,
      off: THEMES.off,
      hover: THEMES.hover,
    };
    if (key === 'discord') {
      variants.off = THEMES.discord;
      variants.hover = THEMES.discord_hover;
      variants.on = THEMES.discord_hover;
    }

    for (const [variant, color] of Object.entries(variants)) {
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
      const resvg = new Resvg(svg, {
        fitTo: { mode: 'width', value: 64 },
      });
      const png = resvg.render().asPng();
      const filename = path.join(outDir, `${key}_${variant}.png`);
      fs.writeFileSync(filename, png);
    }
    process.stdout.write(`✓ ${key} `);
  }
  console.log('\nAll high-res icons generated successfully in ' + outDir);
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
