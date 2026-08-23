/**
 * Line-icon set. 16x16 grid, 1.5px stroke, no fill, currentColor, so an icon
 * inherits the colour and size of whatever text it sits next to.
 *
 *   icon('play')            -> <svg …>
 *   icon('trash', { size }) -> same at a different box size
 */
(function () {
  'use strict';

  const PATHS = {
    // --- navigation
    home:      '<path d="M2.5 6.6 8 2.3l5.5 4.3V13a.5.5 0 0 1-.5.5H3a.5.5 0 0 1-.5-.5Z"/><path d="M6.3 13.5V9.2h3.4v4.3"/>',
    grid:      '<rect x="2.5" y="2.5" width="4.6" height="4.6" rx="1"/><rect x="8.9" y="2.5" width="4.6" height="4.6" rx="1"/><rect x="2.5" y="8.9" width="4.6" height="4.6" rx="1"/><rect x="8.9" y="8.9" width="4.6" height="4.6" rx="1"/>',
    user:      '<circle cx="8" cy="5.6" r="2.7"/><path d="M2.9 13.6c0-2.4 2.3-3.9 5.1-3.9s5.1 1.5 5.1 3.9"/>',
    package:   '<path d="M8 2.2 13.6 5v6L8 13.8 2.4 11V5Z"/><path d="M2.4 5 8 7.8 13.6 5M8 7.8v6"/>',
    server:    '<rect x="2.4" y="3" width="11.2" height="4.2" rx="1"/><rect x="2.4" y="8.8" width="11.2" height="4.2" rx="1"/><path d="M4.8 5.1h.01M4.8 10.9h.01"/>',
    sliders:   '<path d="M2.6 4.6h10.8M2.6 8h10.8M2.6 11.4h10.8"/><circle cx="6" cy="4.6" r="1.5" fill="var(--surface)"/><circle cx="10.4" cy="8" r="1.5" fill="var(--surface)"/><circle cx="5.2" cy="11.4" r="1.5" fill="var(--surface)"/>',

    // --- actions
    play:      '<path d="M5.4 3.3 12.2 8l-6.8 4.7Z"/>',
    stop:      '<rect x="4.4" y="4.4" width="7.2" height="7.2" rx="1"/>',
    plus:      '<path d="M8 3.2v9.6M3.2 8h9.6"/>',
    minus:     '<path d="M3.2 8h9.6"/>',
    close:     '<path d="M4.2 4.2l7.6 7.6M11.8 4.2l-7.6 7.6"/>',
    square:    '<rect x="3.6" y="3.6" width="8.8" height="8.8" rx="1.2"/>',
    check:     '<path d="M3.4 8.4 6.4 11.4 12.6 4.9"/>',
    pencil:    '<path d="M10.7 2.9 13.1 5.3 5.6 12.8H3.2V10.4Z"/><path d="M9.3 4.3 11.7 6.7"/>',
    copy:      '<rect x="5.6" y="5.6" width="7.9" height="7.9" rx="1.2"/><path d="M3.6 10.4H3a.5.5 0 0 1-.5-.5V3a.5.5 0 0 1 .5-.5h6.9a.5.5 0 0 1 .5.5v.6"/>',
    folder:    '<path d="M2.4 12.4V4.1a.5.5 0 0 1 .5-.5h3.2l1.4 1.8h5.6a.5.5 0 0 1 .5.5v6.5a.5.5 0 0 1-.5.5H2.9a.5.5 0 0 1-.5-.5Z"/>',
    trash:     '<path d="M2.9 4.4h10.2M6.1 4.4V2.9h3.8v1.5M4.5 4.4l.6 8.2a.5.5 0 0 0 .5.5h4.8a.5.5 0 0 0 .5-.5l.6-8.2"/>',
    refresh:   '<path d="M13 8a5 5 0 1 1-1.7-3.8"/><path d="M13.3 2.4v2.6h-2.6"/>',
    search:    '<circle cx="7.1" cy="7.1" r="4.4"/><path d="M10.3 10.3 13.4 13.4"/>',
    download:  '<path d="M8 2.7v7.1M5.3 7.2 8 9.9l2.7-2.7M3.2 13.3h9.6"/>',
    external:  '<path d="M9.3 3h3.7v3.7M12.9 3.1 7.6 8.4"/><path d="M11.2 9.6v3.1a.5.5 0 0 1-.5.5H3.3a.5.5 0 0 1-.5-.5V5.3a.5.5 0 0 1 .5-.5h3.1"/>',
    terminal:  '<rect x="2.3" y="3.1" width="11.4" height="9.8" rx="1.2"/><path d="M4.9 6.6 6.9 8.6 4.9 10.6M8.7 10.8h3"/>',
    logout:    '<path d="M6.2 13.2H3.4a.5.5 0 0 1-.5-.5V3.3a.5.5 0 0 1 .5-.5h2.8"/><path d="M9.8 5.2 12.6 8l-2.8 2.8M12.4 8H6"/>',
    image:     '<rect x="2.3" y="3.1" width="11.4" height="9.8" rx="1.2"/><circle cx="6" cy="6.4" r="1.1"/><path d="M2.6 11.2 5.9 8.3l2.3 2 2.1-1.8 3.1 2.7"/>',
    file:      '<path d="M9.1 2.6H4.3a.5.5 0 0 0-.5.5v9.8a.5.5 0 0 0 .5.5h7.4a.5.5 0 0 0 .5-.5V5.6Z"/><path d="M9.1 2.6v3h3.1M6 8.6h4M6 10.7h4"/>',
    chart:     '<path d="M2.6 2.9v10.2h10.8"/><path d="M5.3 10.9V7.8M8 10.9V4.6M10.7 10.9V6.7"/>',
    link:      '<path d="M6.8 9.2a2.6 2.6 0 0 0 3.9.3l1.7-1.7a2.6 2.6 0 0 0-3.7-3.7l-1 1"/><path d="M9.2 6.8a2.6 2.6 0 0 0-3.9-.3L3.6 8.2a2.6 2.6 0 0 0 3.7 3.7l1-1"/>',
    upload:    '<path d="M8 10.4V3.3M5.3 6 8 3.3 10.7 6M3.2 13.3h9.6"/>',

    compass:   '<circle cx="8" cy="8" r="5.7"/><path d="M10.6 5.4 9.3 9.3 5.4 10.6 6.7 6.7Z"/>',
    layers:    '<path d="M8 2.2 14 5.1 8 8 2 5.1Z"/><path d="m2 8 6 2.9L14 8M2 10.9l6 2.9 6-2.9"/>',
    arrowLeft:  '<path d="M13 8H3.3M7.4 3.9 3.2 8l4.2 4.1"/>',
    arrowRight: '<path d="M3 8h9.7M8.6 3.9 12.8 8l-4.2 4.1"/>',
    panelRight: '<rect x="2.2" y="2.9" width="11.6" height="10.2" rx="1.4"/><path d="M10.1 2.9v10.2"/>',

    // --- indicators
    chevronRight: '<path d="M6.3 3.4 10.9 8l-4.6 4.6"/>',
    chevronDown:  '<path d="M3.4 6.1 8 10.7l4.6-4.6"/>',
    users:     '<circle cx="6.2" cy="5.6" r="2.4"/><path d="M1.9 13.2c0-2.2 1.9-3.5 4.3-3.5s4.3 1.3 4.3 3.5"/><path d="M10.6 3.5a2.4 2.4 0 0 1 0 4.6M11.6 9.9c1.5.4 2.5 1.5 2.5 3.3"/>',
    signal:    '<path d="M2.8 12.7v-2.1M6.3 12.7V8.2M9.7 12.7V5.8M13.2 12.7V3.4"/>',
    heart:     '<path d="M8 13.1C6.7 12 2.7 9.5 2.7 6.5a2.8 2.8 0 0 1 5.3-1.3 2.8 2.8 0 0 1 5.3 1.3c0 3-4 5.5-5.3 6.6Z"/>',
    clock:     '<circle cx="8" cy="8" r="5.5"/><path d="M8 4.7V8l2.2 1.6"/>',
    alert:     '<path d="M7.1 2.9 1.9 11.9a.5.5 0 0 0 .4.8h11.4a.5.5 0 0 0 .4-.8L8.9 2.9a.5.5 0 0 0-.9 0Z"/><path d="M8 6.2v3M8 11.1h.01"/>',
    info:      '<circle cx="8" cy="8" r="5.6"/><path d="M8 7.3v3.6M8 5.2h.01"/>',
    globe:     '<circle cx="8" cy="8" r="5.6"/><path d="M2.6 8h10.8"/><path d="M8 2.4c1.5 1.6 2.3 3.5 2.3 5.6S9.5 12 8 13.6C6.5 12 5.7 10.1 5.7 8S6.5 4 8 2.4Z"/>',
    shield:    '<path d="M8 2.4 13 4.1v3.7c0 2.7-2 4.7-5 5.8-3-1.1-5-3.1-5-5.8V4.1Z"/>',

    // --- settings cards
    monitor:   '<rect x="2.2" y="3.1" width="11.6" height="7.7" rx="1.2"/><path d="M6.1 13.1h3.8M8 10.8v2.3"/>',
    gauge:     '<path d="M2.7 11.5a6 6 0 1 1 10.6 0"/><path d="M8 8.6 10.7 6"/>',
    coffee:    '<path d="M2.9 4.3h8.4v4.2a3.2 3.2 0 0 1-3.2 3.2H6.1a3.2 3.2 0 0 1-3.2-3.2Z"/><path d="M11.3 5.6h.9a1.7 1.7 0 0 1 0 3.4h-.9"/><path d="M2.6 13.7h9"/>',
    window:    '<rect x="2.2" y="2.8" width="11.6" height="10.4" rx="1.2"/><path d="M2.2 5.9h11.6M4.4 4.3h.01M6.3 4.3h.01"/>',
    database:  '<ellipse cx="8" cy="4" rx="4.9" ry="1.9"/><path d="M3.1 4v8c0 1 2.2 1.9 4.9 1.9s4.9-.9 4.9-1.9V4"/><path d="M12.9 8c0 1-2.2 1.9-4.9 1.9S3.1 9 3.1 8"/>',
  };

  /** Cover tiles for profiles: no emoji, just a stable colour per profile.
   *  Muted so they sit beside the green accent without competing with it. */
  const TILE_COLORS = ['#3f7d6a', '#4a6fa5', '#8a5a86', '#a05252', '#7a7f45', '#5f6b7a'];

  /**
   * The app mark: a mango, drawn in its own colours rather than currentColor.
   * Kept out of PATHS because it is filled artwork, not a line icon.
   */
  /**
   * The app mark.
   *
   * This used to be three ellipses drawn here, standing in for a logo we did
   * not have. It is the real one now, and one file answers for all of it: the
   * window icon, the taskbar and the corner of the statusbar are the same PNG,
   * so the mark can never drift from the icon on the desktop.
   */
  function logoMark(size = 22) {
    return `<img class="logo-mark" src="assets/icon.png" width="${size}" height="${size}" alt="" aria-hidden="true" />`;
  }

  function icon(name, { size = 16, cls = '' } = {}) {
    const body = PATHS[name];
    if (!body) return '';
    return `<svg class="ic ${cls}" width="${size}" height="${size}" viewBox="0 0 16 16" `
      + `fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" `
      + `stroke-linejoin="round" aria-hidden="true">${body}</svg>`;
  }

  /** Pick a deterministic tile colour so a profile keeps its identity. */
  function tileColor(seed) {
    let h = 0;
    for (const ch of String(seed || '')) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
    return TILE_COLORS[h % TILE_COLORS.length];
  }

  window.icons = { icon, logoMark, tileColor, TILE_COLORS, has: (n) => Boolean(PATHS[n]) };
})();
