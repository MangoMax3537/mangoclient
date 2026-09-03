'use strict';
const net = require('net');
const dns = require('dns/promises');
const serverIcons = require('./servericons');

// ---- varint helpers --------------------------------------------------------

function writeVarInt(value) {
  const bytes = [];
  let v = value >>> 0;
  do {
    let byte = v & 0x7f;
    v >>>= 7;
    if (v !== 0) byte |= 0x80;
    bytes.push(byte);
  } while (v !== 0);
  return Buffer.from(bytes);
}

function readVarInt(buf, offset = 0) {
  let value = 0;
  let shift = 0;
  let pos = offset;
  while (true) {
    if (pos >= buf.length) return null; // need more data
    const byte = buf[pos++];
    value |= (byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) break;
    shift += 7;
    if (shift > 35) throw new Error('VarInt too long');
  }
  return { value, size: pos - offset };
}

function packetize(id, payload) {
  const body = Buffer.concat([writeVarInt(id), payload]);
  return Buffer.concat([writeVarInt(body.length), body]);
}

function writeString(str) {
  const b = Buffer.from(str, 'utf8');
  return Buffer.concat([writeVarInt(b.length), b]);
}

/** Minecraft servers advertise their real host/port through a SRV record. */
async function resolveTarget(host, port) {
  if (port) return { host, port };
  try {
    const records = await dns.resolveSrv(`_minecraft._tcp.${host}`);
    if (records.length) {
      const best = records.sort((a, b) => a.priority - b.priority)[0];
      return { host: best.name, port: best.port };
    }
  } catch { /* no SRV record, normal for most hosts */ }
  return { host, port: 25565 };
}

/** Flatten a chat-component MOTD into plain text. */
function flattenMotd(desc) {
  if (desc == null) return '';
  if (typeof desc === 'string') return desc;
  let out = desc.text || '';
  if (Array.isArray(desc.extra)) out += desc.extra.map(flattenMotd).join('');
  if (Array.isArray(desc)) out += desc.map(flattenMotd).join('');
  if (desc.translate && !out) out = desc.translate;
  return out;
}

function stripColorCodes(str) {
  return String(str).replace(/§[0-9a-fk-orA-FK-OR]/g, '');
}

/**
 * Server List Ping: handshake -> status request -> JSON response.
 * Resolves with null on any failure so the UI can just show "offline".
 */
async function queryServer(address, { timeout = 9000, protocol = 767 } = {}) {
  const [rawHost, rawPort] = String(address).split(':');
  const { host, port } = await resolveTarget(rawHost, rawPort ? Number(rawPort) : null);

  return new Promise((resolve) => {
    const started = Date.now();
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve(value);
    };

    const socket = net.createConnection({ host, port, timeout }, () => {
      const handshake = packetize(0x00, Buffer.concat([
        writeVarInt(protocol),
        writeString(rawHost),
        (() => { const b = Buffer.alloc(2); b.writeUInt16BE(port); return b; })(),
        writeVarInt(1),
      ]));
      socket.write(handshake);
      socket.write(packetize(0x00, Buffer.alloc(0)));
    });

    let buffer = Buffer.alloc(0);
    socket.on('data', (chunk) => {
      buffer = Buffer.concat([buffer, chunk]);
      try {
        const len = readVarInt(buffer, 0);
        if (!len) return;
        if (buffer.length < len.size + len.value) return; // wait for the rest

        let off = len.size;
        const pid = readVarInt(buffer, off);
        off += pid.size;
        const strLen = readVarInt(buffer, off);
        off += strLen.size;
        const json = buffer.subarray(off, off + strLen.value).toString('utf8');
        const data = JSON.parse(json);

        finish({
          online: true,
          ping: Date.now() - started,
          motd: stripColorCodes(flattenMotd(data.description)).trim(),
          players: { online: data.players?.online ?? 0, max: data.players?.max ?? 0 },
          version: data.version?.name || '',
          protocol: data.version?.protocol ?? null,
          favicon: data.favicon || null,
          host, port,
        });
      } catch {
        finish({ online: false, host, port });
      }
    });

    socket.on('timeout', () => finish({ online: false, host, port }));
    socket.on('error', () => finish({ online: false, host, port }));
    socket.on('close', () => finish({ online: false, host, port }));
    setTimeout(() => finish({ online: false, host, port }), timeout + 500);
  });
}

/** A successful first ping establishes the partner icon. Afterwards even an
 * offline result keeps that same cached icon instead of falling back to a
 * generated letter tile or adopting a transient replacement favicon. */
async function pingServer(address, options) {
  const result = await queryServer(address, options);
  if (result.favicon) serverIcons.remember(address, result.favicon);
  const favicon = serverIcons.get(address);
  return favicon ? { ...result, favicon } : result;
}

/**
 * The server shown by default. Name, icon, player count, MOTD and version all
 * come from the server's own status response, so nothing here is hardcoded
 * beyond the address itself.
 */
const FEATURED_SERVERS = [
  { id: 'vincentvanilla', name: 'VincentVanilla', address: 'vincentvanilla.net' },
];

async function pingAll(servers, onResult) {
  return Promise.all(servers.map(async (s) => {
    const result = await pingServer(s.address).catch(() => ({ online: false }));
    const merged = { ...s, status: result };
    onResult?.(merged);
    return merged;
  }));
}

module.exports = { pingServer, pingAll, FEATURED_SERVERS, stripColorCodes, flattenMotd };
