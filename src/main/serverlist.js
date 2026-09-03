'use strict';
const fsp = require('fs/promises');
const path = require('path');
const zlib = require('zlib');
const serverIcons = require('./servericons');

const TAG = {
  end: 0, byte: 1, short: 2, int: 3, long: 4, float: 5, double: 6,
  byteArray: 7, string: 8, list: 9, compound: 10, intArray: 11, longArray: 12,
};
const MAX_COLLECTION = 2_000_000;

class Reader {
  constructor(buffer) { this.buffer = buffer; this.offset = 0; }
  take(size) {
    if (size < 0 || this.offset + size > this.buffer.length) throw new Error('Truncated NBT data');
    const start = this.offset;
    this.offset += size;
    return start;
  }
  byte() { return this.buffer.readInt8(this.take(1)); }
  ubyte() { return this.buffer.readUInt8(this.take(1)); }
  short() { return this.buffer.readInt16BE(this.take(2)); }
  ushort() { return this.buffer.readUInt16BE(this.take(2)); }
  int() { return this.buffer.readInt32BE(this.take(4)); }
  long() { return this.buffer.readBigInt64BE(this.take(8)); }
  float() { return this.buffer.readFloatBE(this.take(4)); }
  double() { return this.buffer.readDoubleBE(this.take(8)); }
  string() {
    const size = this.ushort();
    const start = this.take(size);
    return decodeModifiedUtf8(this.buffer.subarray(start, start + size));
  }
  length() {
    const value = this.int();
    if (value < 0 || value > MAX_COLLECTION) throw new Error('Invalid NBT collection length');
    return value;
  }
}

function readPayload(reader, type) {
  switch (type) {
    case TAG.byte: return reader.byte();
    case TAG.short: return reader.short();
    case TAG.int: return reader.int();
    case TAG.long: return reader.long();
    case TAG.float: return reader.float();
    case TAG.double: return reader.double();
    case TAG.byteArray: {
      const size = reader.length();
      const start = reader.take(size);
      return Buffer.from(reader.buffer.subarray(start, start + size));
    }
    case TAG.string: return reader.string();
    case TAG.list: {
      const elementType = reader.ubyte();
      const size = reader.length();
      const items = Array.from({ length: size }, () => readPayload(reader, elementType));
      return { elementType, items };
    }
    case TAG.compound: {
      const entries = [];
      for (;;) {
        const childType = reader.ubyte();
        if (childType === TAG.end) break;
        const name = reader.string();
        entries.push({ type: childType, name, value: readPayload(reader, childType) });
      }
      return entries;
    }
    case TAG.intArray: return Array.from({ length: reader.length() }, () => reader.int());
    case TAG.longArray: return Array.from({ length: reader.length() }, () => reader.long());
    default: throw new Error(`Unsupported NBT tag ${type}`);
  }
}

function parseNbt(buffer) {
  const reader = new Reader(buffer);
  const type = reader.ubyte();
  if (type !== TAG.compound) throw new Error('Minecraft server list has no compound root');
  const root = { type, name: reader.string(), value: readPayload(reader, type) };
  if (reader.offset !== buffer.length) throw new Error('Unexpected bytes after NBT root');
  return root;
}

function sized(size, write) {
  const buffer = Buffer.allocUnsafe(size);
  write(buffer);
  return buffer;
}

/** NBT is written through Java's DataOutput.writeUTF, which uses modified
 * UTF-8 (not Node's standard UTF-8). In particular, emoji are encoded as two
 * three-byte surrogate code units. Decoding them as ordinary UTF-8 silently
 * replaces characters and can make a rewritten servers.dat unusable. */
function encodeModifiedUtf8(value) {
  const bytes = [];
  const input = String(value);
  for (let i = 0; i < input.length; i++) {
    const code = input.charCodeAt(i);
    if (code >= 0x0001 && code <= 0x007f) {
      bytes.push(code);
    } else if (code <= 0x07ff) {
      bytes.push(0xc0 | ((code >> 6) & 0x1f), 0x80 | (code & 0x3f));
    } else {
      bytes.push(0xe0 | ((code >> 12) & 0x0f), 0x80 | ((code >> 6) & 0x3f), 0x80 | (code & 0x3f));
    }
  }
  return Buffer.from(bytes);
}

function decodeModifiedUtf8(buffer) {
  const units = [];
  for (let i = 0; i < buffer.length;) {
    const first = buffer[i++];
    if (first > 0 && first <= 0x7f) {
      units.push(first);
      continue;
    }
    if ((first & 0xe0) === 0xc0) {
      if (i >= buffer.length || (buffer[i] & 0xc0) !== 0x80) throw new Error('Invalid modified UTF-8');
      const code = ((first & 0x1f) << 6) | (buffer[i++] & 0x3f);
      if (code !== 0 && code < 0x80) throw new Error('Invalid modified UTF-8');
      units.push(code);
      continue;
    }
    if ((first & 0xf0) === 0xe0) {
      if (i + 1 >= buffer.length || (buffer[i] & 0xc0) !== 0x80 || (buffer[i + 1] & 0xc0) !== 0x80) {
        throw new Error('Invalid modified UTF-8');
      }
      const code = ((first & 0x0f) << 12) | ((buffer[i] & 0x3f) << 6) | (buffer[i + 1] & 0x3f);
      i += 2;
      if (code < 0x800) throw new Error('Invalid modified UTF-8');
      units.push(code);
      continue;
    }
    throw new Error('Invalid modified UTF-8');
  }
  // Avoid apply/spread argument limits for a large but valid NBT string.
  let result = '';
  for (let i = 0; i < units.length; i += 8192) result += String.fromCharCode(...units.slice(i, i + 8192));
  return result;
}

function stringBuffer(value) {
  const text = encodeModifiedUtf8(value);
  if (text.length > 0xffff) throw new Error('NBT string is too long');
  const length = sized(2, (out) => out.writeUInt16BE(text.length));
  return Buffer.concat([length, text]);
}

function collectionLength(value) {
  if (!Number.isSafeInteger(value) || value < 0 || value > MAX_COLLECTION) throw new Error('Invalid NBT collection length');
  return sized(4, (out) => out.writeInt32BE(value));
}

function writePayload(type, value) {
  switch (type) {
    case TAG.byte: return sized(1, (out) => out.writeInt8(value));
    case TAG.short: return sized(2, (out) => out.writeInt16BE(value));
    case TAG.int: return sized(4, (out) => out.writeInt32BE(value));
    case TAG.long: return sized(8, (out) => out.writeBigInt64BE(BigInt(value)));
    case TAG.float: return sized(4, (out) => out.writeFloatBE(value));
    case TAG.double: return sized(8, (out) => out.writeDoubleBE(value));
    case TAG.byteArray: {
      const bytes = Buffer.from(value);
      return Buffer.concat([collectionLength(bytes.length), bytes]);
    }
    case TAG.string: return stringBuffer(value);
    case TAG.list: return Buffer.concat([
      Buffer.from([value.elementType]),
      collectionLength(value.items.length),
      ...value.items.map((item) => writePayload(value.elementType, item)),
    ]);
    case TAG.compound: return Buffer.concat([
      ...value.map((entry) => Buffer.concat([
        Buffer.from([entry.type]), stringBuffer(entry.name), writePayload(entry.type, entry.value),
      ])),
      Buffer.from([TAG.end]),
    ]);
    case TAG.intArray: return Buffer.concat([
      collectionLength(value.length),
      ...value.map((item) => sized(4, (out) => out.writeInt32BE(item))),
    ]);
    case TAG.longArray: return Buffer.concat([
      collectionLength(value.length),
      ...value.map((item) => sized(8, (out) => out.writeBigInt64BE(BigInt(item)))),
    ]);
    default: throw new Error(`Unsupported NBT tag ${type}`);
  }
}

function encodeNbt(root) {
  return Buffer.concat([
    Buffer.from([root.type]),
    stringBuffer(root.name || ''),
    writePayload(root.type, root.value),
  ]);
}

function field(compound, name, type) {
  return compound.find((entry) => entry.name === name && (!type || entry.type === type));
}

function setString(compound, name, value) {
  const current = field(compound, name);
  if (current) {
    current.type = TAG.string;
    current.value = value;
  } else {
    compound.push({ type: TAG.string, name, value });
  }
}

/** Minecraft's legacy formatting parser renders this gold almost exactly like
 * the launcher's #ffad32 Mango accent; reset immediately so only the star is
 * coloured and the partner name keeps Vanilla's normal white. */
function featuredName(name) {
  return `§6★ §r${name}`;
}

function normalizeAddress(address) {
  return String(address || '').trim().toLowerCase().replace(/\.$/, '').replace(/:25565$/, '');
}

function emptyServerList() {
  return {
    type: TAG.compound,
    name: '',
    value: [{ type: TAG.list, name: 'servers', value: { elementType: TAG.compound, items: [] } }],
  };
}

async function readServerListFile(file) {
  const compressed = await fsp.readFile(file);
  const raw = compressed[0] === 0x1f && compressed[1] === 0x8b
    ? zlib.gunzipSync(compressed)
    : compressed;
  return parseNbt(raw);
}

async function readServerList(file) {
  try {
    return await readServerListFile(file);
  } catch (mainError) {
    try {
      return await readServerListFile(`${file}.mangoclient-backup`);
    } catch {
      if (mainError.code === 'ENOENT') return emptyServerList();
      throw mainError;
    }
  }
}

async function writeFileAtomic(file, contents) {
  const tmp = `${file}.${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.tmp`;
  await fsp.mkdir(path.dirname(file), { recursive: true });
  let handle;
  try {
    handle = await fsp.open(tmp, 'wx', 0o600);
    await handle.writeFile(contents);
    await handle.sync();
    await handle.close();
    handle = null;
    await fsp.rename(tmp, file);
  } catch (err) {
    if (handle) await handle.close().catch(() => {});
    await fsp.unlink(tmp).catch(() => {});
    throw err;
  }
}

async function writeServerList(file, root) {
  const contents = zlib.gzipSync(encodeNbt(root));
  // The sidecar is launcher-owned and ignored by Minecraft. Writing it first
  // means an interrupted update always leaves at least one complete list.
  await writeFileAtomic(`${file}.mangoclient-backup`, contents);
  await writeFileAtomic(file, contents);
}

/** Put partnered servers first in the real Minecraft multiplayer list. Existing
 * entries and their icons/settings are retained; matching addresses are moved
 * instead of duplicated. */
async function ensureFeaturedServers(gameDir, servers, iconStore = serverIcons) {
  const file = path.join(gameDir, 'servers.dat');
  const root = await readServerList(file);
  let listTag = field(root.value, 'servers', TAG.list);
  if (!listTag) {
    listTag = { type: TAG.list, name: 'servers', value: { elementType: TAG.compound, items: [] } };
    root.value.push(listTag);
  }
  if (listTag.value.elementType !== TAG.compound) throw new Error('Invalid Minecraft servers list');

  const items = listTag.value.items;
  const featured = [];
  const featuredAddresses = new Set(servers.map((server) => normalizeAddress(server.address)));
  for (const server of servers) {
    const address = normalizeAddress(server.address);
    const existing = items.find((item) => normalizeAddress(field(item, 'ip', TAG.string)?.value) === address) || [];
    setString(existing, 'name', featuredName(server.name));
    setString(existing, 'ip', server.address);
    const cachedIcon = iconStore.get(server.address);
    if (cachedIcon && !field(existing, 'icon', TAG.string)?.value) setString(existing, 'icon', cachedIcon);
    featured.push(existing);
  }
  const rest = items.filter((item) => !featuredAddresses.has(normalizeAddress(field(item, 'ip', TAG.string)?.value)));
  listTag.value.items = [...featured, ...rest];
  await writeServerList(file, root);
  return file;
}

/** Read Minecraft's final file without falling back to an older generation and
 * retain the first partner favicon outside servers.dat. This never writes the
 * player's server list. */
async function cacheFeaturedServerIcons(gameDir, servers, iconStore = serverIcons) {
  const file = path.join(gameDir, 'servers.dat');
  const root = await readServerListFile(file);
  const listTag = field(root.value, 'servers', TAG.list);
  if (!listTag || listTag.value.elementType !== TAG.compound) return file;
  const featuredAddresses = new Set(servers.map((server) => normalizeAddress(server.address)));
  for (const item of listTag.value.items) {
    const address = normalizeAddress(field(item, 'ip', TAG.string)?.value);
    if (!featuredAddresses.has(address)) continue;
    const icon = field(item, 'icon', TAG.string)?.value;
    if (icon) iconStore.remember(address, icon);
  }
  return file;
}

/** Bootstrap a brand-new instance only. Existing servers.dat files are never
 * rewritten during launch; MangoConfig pins the partner after Vanilla loads
 * the complete list in memory. */
async function seedFeaturedServers(gameDir, servers, iconStore = serverIcons) {
  const file = path.join(gameDir, 'servers.dat');
  try {
    await fsp.access(file);
    await cacheFeaturedServerIcons(gameDir, servers, iconStore).catch(() => {});
    return file;
  } catch (err) {
    if (err.code !== 'ENOENT') throw err;
  }
  return ensureFeaturedServers(gameDir, servers, iconStore);
}

module.exports = {
  TAG, parseNbt, encodeNbt, readServerList, writeServerList, ensureFeaturedServers,
  seedFeaturedServers, cacheFeaturedServerIcons,
  encodeModifiedUtf8, decodeModifiedUtf8, featuredName,
};
