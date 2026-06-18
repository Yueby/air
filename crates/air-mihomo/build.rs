use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde::Deserialize;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const MIHOMO_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/MetaCubeX/mihomo/releases/latest";
const GEODATA_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/MetaCubeX/meta-rules-dat/releases/latest";
const GEODATA_FILES: &[&str] = &[
    "geoip.dat",
    "geosite.dat",
    "country.mmdb",
    "GeoLite2-ASN.mmdb",
];

#[derive(Debug, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Copy)]
struct TargetPlatform {
    os: &'static str,
    arch: &'static str,
    dir_os: &'static str,
    dir_arch: &'static str,
    executable_name: &'static str,
    archive_extension: &'static str,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=AIR_FORCE_MIHOMO_DOWNLOAD");
    println!(
        "cargo:rerun-if-changed={}",
        workspace_path("mihomo").display()
    );

    ensure_mihomo_archives();
}

fn ensure_mihomo_archives() {
    let target = env::var("TARGET").expect("TARGET should be set by cargo");
    let Some(platform) = target_platform(&target) else {
        println!("cargo:warning=unsupported mihomo target {target}; skip bundled core download");
        ensure_geodata_archive();
        return;
    };

    let force_download = env::var("AIR_FORCE_MIHOMO_DOWNLOAD").is_ok_and(|value| value == "1");
    let mihomo_archive = mihomo_archive_path(platform);
    let geodata_archive = workspace_path("mihomo/geodata.zip");

    if force_download || !mihomo_archive.is_file() {
        download_mihomo_archive(platform, &mihomo_archive);
    }

    if force_download || !geodata_archive.is_file() {
        download_geodata_archive(&geodata_archive);
    }
}

fn target_platform(target: &str) -> Option<TargetPlatform> {
    let os = if target.contains("windows") {
        ("windows", "win", "mihomo.exe", "zip")
    } else if target.contains("linux") {
        ("linux", "linux", "mihomo", "gz")
    } else if target.contains("darwin") {
        ("darwin", "macos", "mihomo", "gz")
    } else {
        return None;
    };

    let arch = if target.starts_with("x86_64") {
        ("amd64", "x64")
    } else if target.starts_with("aarch64") {
        ("arm64", "arm64")
    } else {
        return None;
    };

    Some(TargetPlatform {
        os: os.0,
        arch: arch.0,
        dir_os: os.1,
        dir_arch: arch.1,
        executable_name: os.2,
        archive_extension: os.3,
    })
}

fn mihomo_archive_path(platform: TargetPlatform) -> PathBuf {
    workspace_path("mihomo")
        .join(platform.dir_arch)
        .join(platform.dir_os)
        .join("mihomo.zip")
}

fn download_mihomo_archive(platform: TargetPlatform, output_path: &Path) {
    // 由真正消费 include_bytes! 的 crate 在编译前准备资源，避免 workspace 构建时 air-mihomo
    // 先于 air-desktop 编译，导致 CI 在 cargo check 阶段就因为缺少本地归档而失败。
    let release = fetch_release(MIHOMO_LATEST_RELEASE_API);
    let asset = release
        .assets
        .iter()
        .find(|asset| matches_mihomo_asset(&asset.name, platform))
        .unwrap_or_else(|| {
            panic!(
                "failed to find mihomo release asset for {}-{}",
                platform.os, platform.arch
            )
        });
    println!("cargo:warning=downloading {}", asset.name);

    let bytes = download_bytes(&asset.browser_download_url);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
    }

    let executable = if platform.archive_extension == "zip" {
        extract_executable_from_zip(&asset.name, &bytes)
    } else {
        decode_gzip(&asset.name, &bytes)
    };
    write_single_file_zip(output_path, platform.executable_name, &executable);
}

fn matches_mihomo_asset(name: &str, platform: TargetPlatform) -> bool {
    let expected_prefix = format!("mihomo-{}-{}-", platform.os, platform.arch);
    if !name.starts_with(&expected_prefix) {
        return false;
    }
    if !name.ends_with(&format!(".{}", platform.archive_extension)) {
        return false;
    }

    // 上游会同时发布兼容版和针对特定 CPU 的优化包；这里继续锁定通用包，避免构建产物拆得过细。
    !name.contains("-go")
        && !name.contains("-compatible-")
        && !name.contains("-v1-v")
        && !name.contains("-v2-v")
        && !name.contains("-v3-v")
}

fn ensure_geodata_archive() {
    let geodata_archive = workspace_path("mihomo/geodata.zip");
    if !geodata_archive.is_file() {
        download_geodata_archive(&geodata_archive);
    }
}

fn download_geodata_archive(output_path: &Path) {
    // geodata release 没有统一 zip，这里在构建期整理成项目自己的归档格式，保持运行时释放逻辑稳定。
    let release = fetch_release(GEODATA_LATEST_RELEASE_API);
    let mut files = Vec::new();
    for file_name in GEODATA_FILES {
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == *file_name)
            .unwrap_or_else(|| panic!("failed to find geodata release asset {file_name}"));
        println!("cargo:warning=downloading {}", asset.name);
        files.push((*file_name, download_bytes(&asset.browser_download_url)));
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
    }
    write_multi_file_zip(output_path, &files);
}

fn fetch_release(url: &str) -> GithubRelease {
    http_client()
        .get(url)
        .send()
        .unwrap_or_else(|error| panic!("failed to request {url}: {error}"))
        .error_for_status()
        .unwrap_or_else(|error| panic!("github api returned error for {url}: {error}"))
        .json()
        .unwrap_or_else(|error| panic!("failed to parse github release from {url}: {error}"))
}

fn download_bytes(url: &str) -> Vec<u8> {
    http_client()
        .get(url)
        .send()
        .unwrap_or_else(|error| panic!("failed to download {url}: {error}"))
        .error_for_status()
        .unwrap_or_else(|error| panic!("download returned error for {url}: {error}"))
        .bytes()
        .unwrap_or_else(|error| panic!("failed to read download body from {url}: {error}"))
        .to_vec()
}

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("air-build-script")
        .build()
        .expect("failed to build HTTP client")
}

fn decode_gzip(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut decoder = GzDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .unwrap_or_else(|error| panic!("failed to decode gzip asset {name}: {error}"));
    output
}

fn extract_executable_from_zip(name: &str, bytes: &[u8]) -> Vec<u8> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .unwrap_or_else(|error| panic!("failed to read zip asset {name}: {error}"));
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .unwrap_or_else(|error| panic!("failed to read zip entry from {name}: {error}"));
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_ascii_lowercase();
        if entry_name.ends_with(".exe") || entry_name.rsplit('/').next() == Some("mihomo") {
            let mut output = Vec::new();
            entry.read_to_end(&mut output).unwrap_or_else(|error| {
                panic!("failed to extract executable from {name}: {error}")
            });
            return output;
        }
    }
    panic!("failed to find executable entry in zip asset {name}");
}

fn write_single_file_zip(output_path: &Path, file_name: &str, bytes: &[u8]) {
    write_multi_file_zip(output_path, &[(file_name, bytes.to_vec())]);
}

fn write_multi_file_zip(output_path: &Path, files: &[(&str, Vec<u8>)]) {
    let temp = output_path.with_extension("zip.tmp");
    let output = File::create(&temp)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", temp.display()));
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default();
    for (name, bytes) in files {
        writer
            .start_file(*name, options)
            .unwrap_or_else(|error| panic!("failed to start zip entry {name}: {error}"));
        writer
            .write_all(bytes)
            .unwrap_or_else(|error| panic!("failed to write zip entry {name}: {error}"));
    }
    writer
        .finish()
        .unwrap_or_else(|error| panic!("failed to finish {}: {error}", temp.display()));
    fs::rename(&temp, output_path)
        .unwrap_or_else(|error| panic!("failed to move {}: {error}", output_path.display()));
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
