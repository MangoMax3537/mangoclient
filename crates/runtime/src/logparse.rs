//! log4j XML event parser and UTF-8 chunk decoder.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Error,
}

const OPEN: &str = "<log4j:Event";
const CLOSE: &str = "</log4j:Event>";

fn stack_re() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s+at |Exception |Caused by:|Error:|java\.lang\.|.*\bFATAL\b)").ok()
    })
    .as_ref()
}

fn attr(xml: &str, name: &str) -> String {
    let pattern = format!(r#"\b{name}="([^"]*)""#);
    Regex::new(&pattern)
        .ok()
        .and_then(|re| re.captures(xml).map(|c| c[1].to_string()))
        .unwrap_or_default()
}

fn cdata(xml: &str, tag: &str) -> String {
    let pattern = format!(r"(?s)<log4j:{tag}><!\[CDATA\[(.*?)\]\]></log4j:{tag}>");
    Regex::new(&pattern)
        .ok()
        .and_then(|re| re.captures(xml).map(|c| c[1].to_string()))
        .unwrap_or_default()
}

fn clock(millis: i64) -> String {
    use chrono::{DateTime, Local};
    let dt = DateTime::from_timestamp_millis(millis)
        .map(|d| d.with_timezone(&Local))
        .unwrap_or_else(Local::now);
    dt.format("%H:%M:%S").to_string()
}

/// Turn one `<log4j:Event>` block into console lines.
fn emit_event(xml: &str, emit: &mut dyn FnMut(&str, LogLevel)) {
    let level = attr(xml, "level");
    let stamp = attr(xml, "timestamp").parse::<i64>().unwrap_or(0);
    let time = if stamp != 0 {
        clock(stamp)
    } else {
        clock(chrono::Utc::now().timestamp_millis())
    };
    let throwable = cdata(xml, "Throwable");
    let message = cdata(xml, "Message");
    let body = if throwable.is_empty() {
        message
    } else {
        format!("{message}\n{}", throwable.trim_end())
    };
    let kind = if level == "ERROR" || level == "FATAL" {
        LogLevel::Error
    } else {
        LogLevel::Info
    };
    let mut lines = body.split('\n');
    let first = lines.next().unwrap_or("");
    emit(
        &format!("[{time}] [{}/{level}]: {first}", attr(xml, "thread")),
        kind,
    );
    // Stack-trace lines keep their own indentation.
    for line in lines {
        emit(line, kind);
    }
}

/// Reassembles log4j XML events across chunks; other lines pass through.
#[derive(Debug, Default)]
pub struct LogParser {
    buffer: String,
}

impl LogParser {
    pub fn new() -> Self {
        Self::default()
    }

    fn flush_plain(text: &str, emit: &mut dyn FnMut(&str, LogLevel)) {
        for line in text.split('\n') {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            let level = if stack_re().is_some_and(|re| re.is_match(line)) {
                LogLevel::Error
            } else {
                LogLevel::Info
            };
            emit(line, level);
        }
    }

    /// Feed decoded text; complete lines and events are emitted, a partial
    /// trailing line or event is held back.
    pub fn feed(&mut self, chunk: &str, emit: &mut dyn FnMut(&str, LogLevel)) {
        self.buffer.push_str(chunk);
        loop {
            let Some(start) = self.buffer.find(OPEN) else {
                if let Some(nl) = self.buffer.rfind('\n') {
                    Self::flush_plain(&self.buffer[..nl], emit);
                    self.buffer.drain(..=nl);
                }
                return;
            };
            if start > 0 {
                Self::flush_plain(&self.buffer[..start], emit);
                self.buffer.drain(..start);
                continue;
            }
            let Some(end) = self.buffer.find(CLOSE) else {
                return;
            };
            let stop = end + CLOSE.len();
            emit_event(&self.buffer[..stop], emit);
            self.buffer.drain(..stop);
        }
    }

    /// Flush whatever is left (the game has exited).
    pub fn finish(&mut self, emit: &mut dyn FnMut(&str, LogLevel)) {
        let rest = std::mem::take(&mut self.buffer);
        // A truncated event is shown raw rather than lost.
        Self::flush_plain(&rest, emit);
    }
}

/// Incremental UTF-8 decoder: a chunk may end in the middle of a character.
#[derive(Debug, Default)]
pub struct Utf8Chunker {
    pending: Vec<u8>,
}

impl Utf8Chunker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.pending.clear();
    }

    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        let mut data = std::mem::take(&mut self.pending);
        loop {
            match std::str::from_utf8(&data) {
                Ok(s) => {
                    out.push_str(s);
                    return out;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    // valid_up_to is always a char boundary.
                    out.push_str(&String::from_utf8_lossy(&data[..valid]));
                    match e.error_len() {
                        Some(n) => {
                            out.push('\u{FFFD}');
                            data.drain(..valid + n);
                        }
                        None => {
                            // Incomplete sequence at the end: wait for more.
                            data.drain(..valid);
                            self.pending = data;
                            return out;
                        }
                    }
                }
            }
        }
    }

    /// Flush a dangling partial sequence as a replacement character.
    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            String::new()
        } else {
            self.pending.clear();
            "\u{FFFD}".into()
        }
    }
}
