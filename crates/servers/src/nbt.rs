//! Minimal NBT reader/writer with Java modified UTF-8 strings.

use crate::error::{Result, ServersError};
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::io::Read;

pub const TAG_END: u8 = 0;
pub const TAG_BYTE: u8 = 1;
pub const TAG_SHORT: u8 = 2;
pub const TAG_INT: u8 = 3;
pub const TAG_LONG: u8 = 4;
pub const TAG_FLOAT: u8 = 5;
pub const TAG_DOUBLE: u8 = 6;
pub const TAG_BYTE_ARRAY: u8 = 7;
pub const TAG_STRING: u8 = 8;
pub const TAG_LIST: u8 = 9;
pub const TAG_COMPOUND: u8 = 10;
pub const TAG_INT_ARRAY: u8 = 11;
pub const TAG_LONG_ARRAY: u8 = 12;

const MAX_COLLECTION: usize = 2_000_000;
const MAX_DEPTH: usize = 512;
/// Upper bound for decompressed servers.dat (guards against gzip bombs).
const MAX_DECOMPRESSED: u64 = 128 * 1024 * 1024;

/// An NBT payload. Compounds keep entry order; lists keep their element type.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List { element_type: u8, items: Vec<Value> },
    Compound(Vec<Entry>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Value {
    pub fn tag(&self) -> u8 {
        match self {
            Self::Byte(_) => TAG_BYTE,
            Self::Short(_) => TAG_SHORT,
            Self::Int(_) => TAG_INT,
            Self::Long(_) => TAG_LONG,
            Self::Float(_) => TAG_FLOAT,
            Self::Double(_) => TAG_DOUBLE,
            Self::ByteArray(_) => TAG_BYTE_ARRAY,
            Self::String(_) => TAG_STRING,
            Self::List { .. } => TAG_LIST,
            Self::Compound(_) => TAG_COMPOUND,
            Self::IntArray(_) => TAG_INT_ARRAY,
            Self::LongArray(_) => TAG_LONG_ARRAY,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
}

/// A named compound child.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub value: Value,
}

impl Entry {
    pub fn new(name: impl Into<String>, value: Value) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    pub fn string(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(name, Value::String(value.into()))
    }
}

/// Root compound with its (usually empty) name.
#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub name: String,
    pub entries: Vec<Entry>,
}

/// Find a compound child by name.
pub fn field<'a>(entries: &'a [Entry], name: &str) -> Option<&'a Entry> {
    entries.iter().find(|e| e.name == name)
}

/// Encode a string as Java modified UTF-8 (NUL as C0 80, astral chars as two
/// three-byte surrogate sequences).
pub fn encode_modified_utf8(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    for unit in value.encode_utf16() {
        match unit {
            0x0001..=0x007f => out.push(unit as u8),
            0x0000 | 0x0080..=0x07ff => {
                out.push(0xc0 | ((unit >> 6) & 0x1f) as u8);
                out.push(0x80 | (unit & 0x3f) as u8);
            }
            _ => {
                out.push(0xe0 | ((unit >> 12) & 0x0f) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3f) as u8);
                out.push(0x80 | (unit & 0x3f) as u8);
            }
        }
    }
    out
}

/// Decode Java modified UTF-8. Unpaired surrogates become U+FFFD.
pub fn decode_modified_utf8(bytes: &[u8]) -> Result<String> {
    let bad = ServersError::InvalidModifiedUtf8;
    let cont = |b: Option<&u8>| match b {
        Some(&b) if b & 0xc0 == 0x80 => Some(u16::from(b & 0x3f)),
        _ => None,
    };
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let first = bytes[i];
        i += 1;
        if first > 0 && first <= 0x7f {
            units.push(u16::from(first));
        } else if first & 0xe0 == 0xc0 {
            let c = cont(bytes.get(i)).ok_or(ServersError::InvalidModifiedUtf8)?;
            i += 1;
            let code = (u16::from(first & 0x1f) << 6) | c;
            if code != 0 && code < 0x80 {
                return Err(bad);
            }
            units.push(code);
        } else if first & 0xf0 == 0xe0 {
            let c1 = cont(bytes.get(i)).ok_or(ServersError::InvalidModifiedUtf8)?;
            let c2 = cont(bytes.get(i + 1)).ok_or(ServersError::InvalidModifiedUtf8)?;
            i += 2;
            let code = (u16::from(first & 0x0f) << 12) | (c1 << 6) | c2;
            if code < 0x800 {
                return Err(bad);
            }
            units.push(code);
        } else {
            return Err(bad);
        }
    }
    Ok(String::from_utf16_lossy(&units))
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(ServersError::Truncated)?;
        if end > self.buf.len() {
            return Err(ServersError::Truncated);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    fn string(&mut self) -> Result<String> {
        let size = usize::from(u16::from_be_bytes(self.array()?));
        decode_modified_utf8(self.take(size)?)
    }

    fn length(&mut self) -> Result<usize> {
        let v = self.i32()?;
        usize::try_from(v)
            .ok()
            .filter(|&n| n <= MAX_COLLECTION)
            .ok_or(ServersError::InvalidLength)
    }

    fn payload(&mut self, tag: u8, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(ServersError::TooDeep);
        }
        Ok(match tag {
            TAG_BYTE => Value::Byte(i8::from_be_bytes(self.array()?)),
            TAG_SHORT => Value::Short(i16::from_be_bytes(self.array()?)),
            TAG_INT => Value::Int(self.i32()?),
            TAG_LONG => Value::Long(i64::from_be_bytes(self.array()?)),
            TAG_FLOAT => Value::Float(f32::from_be_bytes(self.array()?)),
            TAG_DOUBLE => Value::Double(f64::from_be_bytes(self.array()?)),
            TAG_BYTE_ARRAY => {
                let n = self.length()?;
                Value::ByteArray(self.take(n)?.to_vec())
            }
            TAG_STRING => Value::String(self.string()?),
            TAG_LIST => {
                let element_type = self.u8()?;
                let n = self.length()?;
                if n > 0 && (element_type == TAG_END || element_type > TAG_LONG_ARRAY) {
                    return Err(ServersError::UnsupportedTag(element_type));
                }
                let mut items = Vec::new();
                for _ in 0..n {
                    items.push(self.payload(element_type, depth + 1)?);
                }
                Value::List {
                    element_type,
                    items,
                }
            }
            TAG_COMPOUND => {
                let mut entries = Vec::new();
                loop {
                    let child = self.u8()?;
                    if child == TAG_END {
                        break;
                    }
                    let name = self.string()?;
                    let value = self.payload(child, depth + 1)?;
                    entries.push(Entry { name, value });
                }
                Value::Compound(entries)
            }
            TAG_INT_ARRAY => {
                let n = self.length()?;
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(self.i32()?);
                }
                Value::IntArray(v)
            }
            TAG_LONG_ARRAY => {
                let n = self.length()?;
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(i64::from_be_bytes(self.array()?));
                }
                Value::LongArray(v)
            }
            other => return Err(ServersError::UnsupportedTag(other)),
        })
    }
}

/// Parse an uncompressed NBT document (root must be a compound).
pub fn parse_nbt(buf: &[u8]) -> Result<Root> {
    let mut r = Reader { buf, pos: 0 };
    if r.u8()? != TAG_COMPOUND {
        return Err(ServersError::InvalidNbt("root is not a compound"));
    }
    let name = r.string()?;
    let Value::Compound(entries) = r.payload(TAG_COMPOUND, 0)? else {
        return Err(ServersError::InvalidNbt("root is not a compound"));
    };
    if r.pos != buf.len() {
        return Err(ServersError::InvalidNbt("unexpected bytes after root"));
    }
    Ok(Root { name, entries })
}

/// Parse gzip-compressed or raw NBT, detected by the gzip magic.
pub fn parse_nbt_maybe_gzip(data: &[u8]) -> Result<Root> {
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut raw = Vec::new();
        GzDecoder::new(data)
            .take(MAX_DECOMPRESSED + 1)
            .read_to_end(&mut raw)?;
        if raw.len() as u64 > MAX_DECOMPRESSED {
            return Err(ServersError::InvalidNbt("decompressed data too large"));
        }
        parse_nbt(&raw)
    } else {
        parse_nbt(data)
    }
}

fn write_string(out: &mut Vec<u8>, s: &str) -> Result<()> {
    let bytes = encode_modified_utf8(s);
    let len = u16::try_from(bytes.len()).map_err(|_| ServersError::StringTooLong)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&bytes);
    Ok(())
}

fn write_len(out: &mut Vec<u8>, n: usize) -> Result<()> {
    if n > MAX_COLLECTION {
        return Err(ServersError::InvalidLength);
    }
    out.extend_from_slice(&(n as i32).to_be_bytes());
    Ok(())
}

fn write_payload(out: &mut Vec<u8>, value: &Value) -> Result<()> {
    match value {
        Value::Byte(v) => out.extend_from_slice(&v.to_be_bytes()),
        Value::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Value::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Value::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Value::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Value::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Value::ByteArray(v) => {
            write_len(out, v.len())?;
            out.extend_from_slice(v);
        }
        Value::String(s) => write_string(out, s)?,
        Value::List {
            element_type,
            items,
        } => {
            if items.iter().any(|i| i.tag() != *element_type) {
                return Err(ServersError::InvalidNbt("list element type mismatch"));
            }
            out.push(*element_type);
            write_len(out, items.len())?;
            for item in items {
                write_payload(out, item)?;
            }
        }
        Value::Compound(entries) => {
            for e in entries {
                out.push(e.value.tag());
                write_string(out, &e.name)?;
                write_payload(out, &e.value)?;
            }
            out.push(TAG_END);
        }
        Value::IntArray(v) => {
            write_len(out, v.len())?;
            for i in v {
                out.extend_from_slice(&i.to_be_bytes());
            }
        }
        Value::LongArray(v) => {
            write_len(out, v.len())?;
            for i in v {
                out.extend_from_slice(&i.to_be_bytes());
            }
        }
    }
    Ok(())
}

/// Encode a root compound to uncompressed NBT bytes.
pub fn encode_nbt(root: &Root) -> Result<Vec<u8>> {
    let mut out = vec![TAG_COMPOUND];
    write_string(&mut out, &root.name)?;
    for e in &root.entries {
        out.push(e.value.tag());
        write_string(&mut out, &e.name)?;
        write_payload(&mut out, &e.value)?;
    }
    out.push(TAG_END);
    Ok(out)
}

/// Encode and gzip-compress.
pub fn encode_nbt_gzip(root: &Root) -> Result<Vec<u8>> {
    let raw = encode_nbt(root)?;
    let mut out = Vec::new();
    GzEncoder::new(raw.as_slice(), Compression::default()).read_to_end(&mut out)?;
    Ok(out)
}
