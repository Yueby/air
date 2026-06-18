use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use air_error::AppResult;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlatformFamily {
    Windows,
    Macos,
    Linux,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CpuArch {
    X86_64,
    Aarch64,
    Armv7,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    Plain,
    GzipSingle,
    TarGzip,
    Zip,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreAsset {
    pub file_name: String,
    pub download_url: String,
    pub sha256: Option<String>,
    pub platform: PlatformFamily,
    pub arch: CpuArch,
    pub archive: ArchiveFormat,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreRelease {
    pub version: String,
    pub assets: Vec<CoreAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostPlatform {
    pub family: PlatformFamily,
    pub arch: CpuArch,
}

impl HostPlatform {
    pub fn current() -> Self {
        Self {
            family: current_family(),
            arch: current_arch(),
        }
    }

    pub fn executable_name(&self) -> &'static str {
        match self.family {
            PlatformFamily::Windows => "mihomo.exe",
            _ => "mihomo",
        }
    }
}

#[async_trait]
pub trait CoreReleaseProvider: Send + Sync {
    // release provider 只负责发现发布版本，下载、校验和落盘由 acquire 模块统一处理。
    async fn latest_release(&self, platform: &HostPlatform) -> AppResult<CoreRelease>;
}

pub fn parse_asset_metadata(
    file_name: &str,
    download_url: String,
    sha256: Option<String>,
) -> CoreAsset {
    let lower = file_name.to_ascii_lowercase();
    CoreAsset {
        file_name: file_name.to_owned(),
        download_url,
        sha256,
        platform: parse_platform(&lower),
        arch: parse_arch(&lower),
        archive: parse_archive_format(&lower),
    }
}

pub fn choose_asset<'a>(release: &'a CoreRelease, host: &HostPlatform) -> Option<&'a CoreAsset> {
    release
        .assets
        .iter()
        .find(|asset| asset.platform == host.family && asset.arch == host.arch)
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.platform == host.family)
        })
}

pub fn parse_archive_format(file_name: &str) -> ArchiveFormat {
    if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        ArchiveFormat::TarGzip
    } else if file_name.ends_with(".zip") {
        ArchiveFormat::Zip
    } else if file_name.ends_with(".gz") {
        ArchiveFormat::GzipSingle
    } else {
        ArchiveFormat::Plain
    }
}

pub fn install_file_name(path: &Path, host: &HostPlatform) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.starts_with("mihomo"))
        .unwrap_or_else(|| host.executable_name())
        .to_owned()
}

fn parse_platform(lower: &str) -> PlatformFamily {
    if lower.contains("windows") || lower.contains("win") {
        PlatformFamily::Windows
    } else if lower.contains("darwin") || lower.contains("macos") {
        PlatformFamily::Macos
    } else if lower.contains("linux") {
        PlatformFamily::Linux
    } else {
        PlatformFamily::Unknown
    }
}

fn parse_arch(lower: &str) -> CpuArch {
    if lower.contains("amd64") || lower.contains("x86_64") {
        CpuArch::X86_64
    } else if lower.contains("arm64") || lower.contains("aarch64") {
        CpuArch::Aarch64
    } else if lower.contains("armv7") {
        CpuArch::Armv7
    } else {
        CpuArch::Unknown
    }
}

fn current_family() -> PlatformFamily {
    // 平台探测集中在这里，业务代码使用 HostPlatform，避免散落 cfg 条件。
    match std::env::consts::OS {
        "windows" => PlatformFamily::Windows,
        "macos" => PlatformFamily::Macos,
        "linux" => PlatformFamily::Linux,
        _ => PlatformFamily::Unknown,
    }
}

fn current_arch() -> CpuArch {
    match std::env::consts::ARCH {
        "x86_64" => CpuArch::X86_64,
        "aarch64" => CpuArch::Aarch64,
        "arm" => CpuArch::Armv7,
        _ => CpuArch::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_asset_file_names() {
        let asset = parse_asset_metadata(
            "mihomo-windows-amd64-v1.19.0.zip",
            "https://example.test/a.zip".into(),
            None,
        );
        assert_eq!(asset.platform, PlatformFamily::Windows);
        assert_eq!(asset.arch, CpuArch::X86_64);
        assert_eq!(asset.archive, ArchiveFormat::Zip);

        let asset = parse_asset_metadata(
            "mihomo-linux-arm64-compatible-v1.19.0.gz",
            "https://example.test/a.gz".into(),
            None,
        );
        assert_eq!(asset.platform, PlatformFamily::Linux);
        assert_eq!(asset.arch, CpuArch::Aarch64);
        assert_eq!(asset.archive, ArchiveFormat::GzipSingle);
    }

    #[test]
    fn chooses_exact_platform_and_arch_first() {
        let release = CoreRelease {
            version: "v1".into(),
            assets: vec![
                parse_asset_metadata("mihomo-linux-arm64.gz", "a".into(), None),
                parse_asset_metadata("mihomo-linux-amd64.gz", "b".into(), None),
            ],
        };
        let host = HostPlatform {
            family: PlatformFamily::Linux,
            arch: CpuArch::X86_64,
        };

        assert_eq!(choose_asset(&release, &host).unwrap().download_url, "b");
    }
}
