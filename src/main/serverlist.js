'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const zlib = require('zlib');

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
    return this.buffer.toString('utf8', this.take(size), this.offset);
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

function stringBuffer(value) {
  const text = Buffer.from(String(value), 'utf8');
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

async function readServerList(file) {
  if (!fs.existsSync(file)) return emptyServerList();
  const compressed = await fsp.readFile(file);
  const raw = compressed[0] === 0x1f && compressed[1] === 0x8b
    ? zlib.gunzipSync(compressed)
    : compressed;
  return parseNbt(raw);
}

async function writeServerList(file, root) {
  const tmp = `${file}.${process.pid}.tmp`;
  await fsp.mkdir(path.dirname(file), { recursive: true });
  await fsp.writeFile(tmp, zlib.gzipSync(encodeNbt(root)));
  await fsp.rename(tmp, file);
}

/** Put partnered servers first in the real Minecraft multiplayer list. Existing
 * entries and their icons/settings are retained; matching addresses are moved
 * instead of duplicated. */
async function ensureFeaturedServers(gameDir, servers) {
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
    setString(existing, 'name', `★ ${server.name}`);
    setString(existing, 'ip', server.address);
    featured.push(existing);
  }
  const rest = items.filter((item) => !featuredAddresses.has(normalizeAddress(field(item, 'ip', TAG.string)?.value)));
  listTag.value.items = [...featured, ...rest];
  await writeServerList(file, root);
  return file;
}

module.exports = {
  TAG, parseNbt, encodeNbt, readServerList, writeServerList, ensureFeaturedServers,
};
