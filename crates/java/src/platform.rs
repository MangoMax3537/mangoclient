#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X64,
    Aarch64,
    X86,
    Arm,
}

/// Target platform. The only place that looks at `cfg!`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    pub fn current() -> Self {
        let os = if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        };
        let arch = if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else if cfg!(target_arch = "x86") {
            Arch::X86
        } else if cfg!(target_arch = "arm") {
            Arch::Arm
        } else {
            Arch::X64
        };
        Self { os, arch }
    }

    /// Adoptium `os` query value.
    pub fn adoptium_os(self) -> &'static str {
        match self.os {
            Os::Windows => "windows",
            Os::Mac => "mac",
            Os::Linux => "linux",
        }
    }

    /// Adoptium `architecture` query value.
    pub fn adoptium_arch(self) -> &'static str {
        match self.arch {
            Arch::X64 => "x64",
            Arch::Aarch64 => "aarch64",
            Arch::X86 => "x86",
            Arch::Arm => "arm",
        }
    }

    pub(crate) fn exe_name(self, prefer_javaw: bool) -> &'static str {
        match (self.os, prefer_javaw) {
            (Os::Windows, true) => "javaw.exe",
            (Os::Windows, false) => "java.exe",
            _ => "java",
        }
    }
}
