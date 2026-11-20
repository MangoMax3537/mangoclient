//! The machine the game will run on, as far as rules and natives care.

use mango_java::{Arch, Os, Platform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    Osx,
    Linux,
}

impl HostOs {
    /// The name Mojang's rules and `natives` maps use.
    pub fn rule_name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Osx => "osx",
            Self::Linux => "linux",
        }
    }

    pub fn classpath_separator(self) -> char {
        if self == Self::Windows {
            ';'
        } else {
            ':'
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch {
    X64,
    Arm64,
    X86,
    Arm,
}

const ALIASES_X64: &[&str] = &["x64", "x86_64", "x86-64", "amd64"];
const ALIASES_ARM64: &[&str] = &["arm64", "aarch64"];
const ALIASES_X86: &[&str] = &["x86", "x86_32", "i386"];
const ALIASES_ARM: &[&str] = &["arm", "arm32"];

impl HostArch {
    /// Spelling compared against `os.arch` in rules (`x86` is 32-bit Intel).
    pub fn rule_name(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
            Self::X86 => "x86",
            Self::Arm => "arm",
        }
    }

    /// Every spelling of this architecture found in native classifiers.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::X64 => ALIASES_X64,
            Self::Arm64 => ALIASES_ARM64,
            Self::X86 => ALIASES_X86,
            Self::Arm => ALIASES_ARM,
        }
    }

    pub fn is_known_alias(name: &str) -> bool {
        [ALIASES_X64, ALIASES_ARM64, ALIASES_X86, ALIASES_ARM]
            .iter()
            .any(|list| list.contains(&name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEnv {
    pub os: HostOs,
    pub arch: HostArch,
    /// Kernel release, matched by `os.version` regex rules. Empty when unknown.
    pub os_version: String,
}

impl HostEnv {
    pub fn new(os: HostOs, arch: HostArch) -> Self {
        Self {
            os,
            arch,
            os_version: String::new(),
        }
    }

    pub fn current() -> Self {
        let p = Platform::current();
        let os = match p.os {
            Os::Windows => HostOs::Windows,
            Os::Mac => HostOs::Osx,
            Os::Linux => HostOs::Linux,
        };
        let arch = match p.arch {
            Arch::X64 => HostArch::X64,
            Arch::Aarch64 => HostArch::Arm64,
            Arch::X86 => HostArch::X86,
            Arch::Arm => HostArch::Arm,
        };
        // Best effort: only Linux exposes the release as a file.
        let os_version = std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        Self {
            os,
            arch,
            os_version,
        }
    }
}
