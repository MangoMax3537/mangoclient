//! A tiny checker for the generated TypeScript types: it parses the subset of
//! TypeScript that ts-rs emits and validates real JSON values against it, so a
//! shape drift between Rust and `ui/src/lib/generated` fails a test.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub fn generated_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/lib/generated")
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Null,
    Str,
    Num,
    Bool,
    Lit(String),
    Array(Box<Ty>),
    Record(Box<Ty>),
    Union(Vec<Ty>),
    Object(Vec<Prop>),
    Named(String),
    Never,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: String,
    pub optional: bool,
    pub ty: Ty,
}

fn strip_comments(src: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if b[i] == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<char> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) {
        assert!(
            self.eat(c),
            "expected `{c}` at {} in `{}`",
            self.i,
            self.context()
        );
    }
    fn context(&self) -> String {
        self.s[self.i.saturating_sub(30)..(self.i + 30).min(self.s.len())]
            .iter()
            .collect()
    }
    fn ident(&mut self) -> String {
        self.ws();
        let start = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_alphanumeric() || self.s[self.i] == '_') {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }
    fn quoted(&mut self, q: char) -> String {
        self.i += 1;
        let start = self.i;
        while self.s[self.i] != q {
            self.i += 1;
        }
        let v: String = self.s[start..self.i].iter().collect();
        self.i += 1;
        v
    }
    fn ty(&mut self) -> Ty {
        let mut members = vec![self.primary()];
        while self.eat('|') {
            members.push(self.primary());
        }
        if members.len() == 1 {
            members.remove(0)
        } else {
            Ty::Union(members)
        }
    }
    fn primary(&mut self) -> Ty {
        match self.peek() {
            Some('\'') => Ty::Lit(self.quoted('\'')),
            Some('"') => Ty::Lit(self.quoted('"')),
            Some('(') => {
                self.i += 1;
                let t = self.ty();
                self.expect(')');
                t
            }
            Some('{') => {
                self.i += 1;
                let mut props = Vec::new();
                loop {
                    if self.eat('}') {
                        break;
                    }
                    if self.peek() == Some('[') {
                        // `{ [key in K]: V }`
                        while self.peek() != Some(']') {
                            self.i += 1;
                        }
                        self.i += 1;
                        self.eat('?');
                        self.expect(':');
                        let v = self.ty();
                        self.eat(',');
                        self.expect('}');
                        return Ty::Record(Box::new(v));
                    }
                    let name = match self.peek() {
                        Some('"') => self.quoted('"'),
                        Some('\'') => self.quoted('\''),
                        _ => self.ident(),
                    };
                    let optional = self.eat('?');
                    self.expect(':');
                    let ty = self.ty();
                    props.push(Prop { name, optional, ty });
                    if !self.eat(',') {
                        self.eat(';');
                    }
                }
                Ty::Object(props)
            }
            _ => {
                let id = self.ident();
                assert!(!id.is_empty(), "unexpected token in `{}`", self.context());
                match id.as_str() {
                    "null" => Ty::Null,
                    "string" => Ty::Str,
                    "number" => Ty::Num,
                    "boolean" => Ty::Bool,
                    "never" => Ty::Never,
                    "Array" => {
                        self.expect('<');
                        let t = self.ty();
                        self.expect('>');
                        Ty::Array(Box::new(t))
                    }
                    "Record" => {
                        self.expect('<');
                        let _k = self.ty();
                        self.expect(',');
                        let t = self.ty();
                        self.expect('>');
                        Ty::Record(Box::new(t))
                    }
                    _ => Ty::Named(id),
                }
            }
        }
    }
}

pub fn parse_ty(src: &str) -> Ty {
    let mut p = Parser {
        s: strip_comments(src).chars().collect(),
        i: 0,
    };
    let t = p.ty();
    p.ws();
    assert!(
        p.i >= p.s.len() || p.s[p.i] == ';',
        "trailing input: {}",
        p.context()
    );
    t
}

/// The declaration of `name` in the generated directory.
pub fn load(name: &str) -> Ty {
    load_in(name, name)
}

/// Type `name` declared in `<file>.ts`.
pub fn load_in(file: &str, name: &str) -> Ty {
    let file = generated_dir().join(format!("{file}.ts"));
    let src = std::fs::read_to_string(&file).unwrap_or_else(|_| panic!("no generated type {name}"));
    let src = strip_comments(&src);
    let eq = src
        .find(&format!("export type {name} ="))
        .unwrap_or_else(|| panic!("{name}.ts has no declaration"));
    let body = &src[eq + format!("export type {name} =").len()..];
    parse_ty(body.trim().trim_end_matches(';'))
}

/// The declared property names of an object type (top level).
pub fn keys_of(name: &str) -> BTreeSet<String> {
    match load(name) {
        Ty::Object(props) => props.into_iter().map(|p| p.name).collect(),
        other => panic!("{name} is not an object type: {other:?}"),
    }
}

/// `Ok` when `value` fits `ty`; the error names the JSON path that does not.
pub fn check(value: &Value, ty: &Ty, path: &str) -> Result<(), String> {
    let bad = |what: &str| Err(format!("{path}: expected {what}, got {value}"));
    match ty {
        Ty::Null => value
            .is_null()
            .then_some(())
            .ok_or(())
            .or_else(|_| bad("null")),
        Ty::Str => value
            .is_string()
            .then_some(())
            .ok_or(())
            .or_else(|_| bad("string")),
        Ty::Num => value
            .is_number()
            .then_some(())
            .ok_or(())
            .or_else(|_| bad("number")),
        Ty::Bool => value
            .is_boolean()
            .then_some(())
            .ok_or(())
            .or_else(|_| bad("boolean")),
        Ty::Lit(l) => (value.as_str() == Some(l.as_str()))
            .then_some(())
            .ok_or(())
            .or_else(|_| bad(&format!("'{l}'"))),
        Ty::Array(t) => match value.as_array() {
            Some(items) => items
                .iter()
                .enumerate()
                .try_for_each(|(i, v)| check(v, t, &format!("{path}[{i}]"))),
            None => bad("array"),
        },
        Ty::Record(t) => match value.as_object() {
            Some(o) => o
                .iter()
                .try_for_each(|(k, v)| check(v, t, &format!("{path}.{k}"))),
            None => bad("object"),
        },
        Ty::Union(ms) => {
            let mut errs = Vec::new();
            for m in ms {
                match check(value, m, path) {
                    Ok(()) => return Ok(()),
                    Err(e) => errs.push(e),
                }
            }
            Err(format!(
                "{path}: no union member matches {value}: {}",
                errs.join(" | ")
            ))
        }
        Ty::Object(props) => {
            let Some(o) = value.as_object() else {
                return bad("object");
            };
            for k in o.keys() {
                if !props.iter().any(|p| &p.name == k) {
                    return Err(format!("{path}: key `{k}` is not declared in the TS type"));
                }
            }
            for p in props {
                match o.get(&p.name) {
                    Some(v) => check(v, &p.ty, &format!("{path}.{}", p.name))?,
                    None if p.optional => {}
                    None => return Err(format!("{path}: declared key `{}` is missing", p.name)),
                }
            }
            Ok(())
        }
        Ty::Named(n) => check(value, &load(n), path),
        Ty::Never => bad("nothing"),
    }
}

pub fn check_named(value: &Value, name: &str) -> Result<(), String> {
    check(value, &load(name), name)
}

/// `(args, result)` of a command from the generated `CommandMap`.
pub fn command_types(command: &str) -> (Ty, Ty) {
    let Ty::Object(props) = load_in("commands", "CommandMap") else {
        panic!("CommandMap is not an object")
    };
    let entry = props
        .into_iter()
        .find(|p| p.name == command)
        .unwrap_or_else(|| panic!("{command} is not in the generated CommandMap"));
    let Ty::Object(inner) = entry.ty else {
        panic!("bad CommandMap entry")
    };
    let get = |n: &str| {
        inner
            .iter()
            .find(|p| p.name == n)
            .map(|p| p.ty.clone())
            .unwrap()
    };
    (get("args"), get("result"))
}

pub fn command_names() -> BTreeSet<String> {
    let Ty::Object(props) = load_in("commands", "CommandMap") else {
        panic!("CommandMap is not an object")
    };
    props.into_iter().map(|p| p.name).collect()
}
