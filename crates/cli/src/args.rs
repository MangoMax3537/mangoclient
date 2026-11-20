//! Tiny argument parser: positionals, `--flag`, `--key value`, `--key=value`.

use crate::error::{usage, Res};

#[derive(Debug, Default)]
pub struct Args {
    items: Vec<String>,
}

impl Args {
    pub fn new(items: Vec<String>) -> Self {
        Self { items }
    }

    /// Remove and return the next positional (not starting with `--`).
    pub fn positional(&mut self) -> Option<String> {
        let idx = self.items.iter().position(|a| !a.starts_with("--"))?;
        Some(self.items.remove(idx))
    }

    pub fn flag(&mut self, name: &str) -> bool {
        match self.items.iter().position(|a| a == name) {
            Some(i) => {
                self.items.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn value(&mut self, name: &str) -> Res<Option<String>> {
        let prefix = format!("{name}=");
        if let Some(i) = self.items.iter().position(|a| a.starts_with(&prefix)) {
            let item = self.items.remove(i);
            return Ok(Some(item[prefix.len()..].to_string()));
        }
        if let Some(i) = self.items.iter().position(|a| a == name) {
            self.items.remove(i);
            if i < self.items.len() && !self.items[i].starts_with("--") {
                return Ok(Some(self.items.remove(i)));
            }
            return usage(format!("{name} needs a value"));
        }
        Ok(None)
    }

    pub fn number(&mut self, name: &str) -> Res<Option<u64>> {
        match self.value(name)? {
            Some(v) => v
                .parse::<u64>()
                .map(Some)
                .or_else(|_| usage(format!("{name} needs a whole number, got {v:?}"))),
            None => Ok(None),
        }
    }

    /// Fail on anything not consumed.
    pub fn finish(self) -> Res<()> {
        match self.items.first() {
            Some(extra) => usage(format!("unexpected argument {extra:?}")),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Args {
        Args::new(v.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn parses_mixed() {
        let mut x = a(&["My Pack", "--mc", "1.21", "--dry-run", "--timeout-secs=5"]);
        assert_eq!(x.positional().as_deref(), Some("My Pack"));
        assert_eq!(x.value("--mc").unwrap().as_deref(), Some("1.21"));
        assert!(x.flag("--dry-run"));
        assert_eq!(x.number("--timeout-secs").unwrap(), Some(5));
        assert!(x.finish().is_ok());
    }

    #[test]
    fn missing_value_and_extras() {
        let mut x = a(&["--mc"]);
        assert!(x.value("--mc").is_err());
        assert!(a(&["--bogus"]).finish().is_err());
        assert!(a(&["--n", "x"]).number("--n").is_err());
    }
}
