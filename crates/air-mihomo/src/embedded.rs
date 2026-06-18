use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use air_error::{AppResult, ProcessError, StorageError};

#[derive(Clone, Copy)]
struct EmbeddedCore {
    executable_name: &'static str,
    archive_bytes: &'static [u8],
}

#[derive(Clone, Copy)]
struct EmbeddedGeodata {
    archive_bytes: &'static [u8],
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const EMBEDDED_CORE: Option<EmbeddedCore> = Some(EmbeddedCore {
    executable_name: "mihomo.exe",
    archive_bytes: include_bytes!("../../../mihomo/x64/win/mihomo.zip"),
});

#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
const EMBEDDED_CORE: Option<EmbeddedCore> = Some(EmbeddedCore {
    executable_name: "mihomo.exe",
    archive_bytes: include_bytes!("../../../mihomo/arm64/win/mihomo.zip"),
});

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const EMBEDDED_CORE: Option<EmbeddedCore> = Some(EmbeddedCore {
    executable_name: "mihomo",
    archive_bytes: include_bytes!("../../../mihomo/x64/linux/mihomo.zip"),
});

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const EMBEDDED_CORE: Option<EmbeddedCore> = Some(EmbeddedCore {
    executable_name: "mihomo",
    archive_bytes: include_bytes!("../../../mihomo/arm64/linux/mihomo.zip"),
});

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const EMBEDDED_CORE: Option<EmbeddedCore> = Some(EmbeddedCore {
    executable_name: "mihomo",
    archive_bytes: include_bytes!("../../../mihomo/x64/macos/mihomo.zip"),
});

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const EMBEDDED_CORE: Option<EmbeddedCore> = Some(EmbeddedCore {
    executable_name: "mihomo",
    archive_bytes: include_bytes!("../../../mihomo/arm64/macos/mihomo.zip"),
});

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
const EMBEDDED_CORE: Option<EmbeddedCore> = None;

const EMBEDDED_GEODATA: EmbeddedGeodata = EmbeddedGeodata {
    archive_bytes: include_bytes!("../../../mihomo/geodata.zip"),
};

pub fn ensure_embedded_core_installed(cores_dir: &Path) -> AppResult<Option<PathBuf>> {
    let Some(core) = EMBEDDED_CORE else {
        return Ok(None);
    };
    install_embedded_core(cores_dir, core)
}

pub fn ensure_embedded_geodata_installed(cores_dir: &Path) -> AppResult<Vec<PathBuf>> {
    install_embedded_geodata(cores_dir, EMBEDDED_GEODATA)
}

fn install_embedded_core(cores_dir: &Path, core: EmbeddedCore) -> AppResult<Option<PathBuf>> {
    let target = cores_dir.join(core.executable_name);
    if target.is_file() {
        return Ok(None);
    }

    fs::create_dir_all(cores_dir).map_err(StorageError::Io)?;
    let temp = target.with_extension(format!(
        "{}.tmp",
        target
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bin")
    ));

    // 构建脚本首次构建时从 GitHub 下载当前 target 的压缩包，仓库不再提交上游二进制；
    // 运行时仍只从本地压缩包释放，避免应用启动依赖网络，并用包内文件名校验阻断路径穿越。
    write_embedded_zip_entry_to_file(core.archive_bytes, core.executable_name, &temp)?;
    set_executable_permission(&temp)?;
    fs::rename(&temp, &target).map_err(StorageError::Io)?;
    Ok(Some(target))
}

fn write_embedded_zip_entry_to_file(
    archive_bytes: &[u8],
    executable_name: &str,
    target: &Path,
) -> AppResult<()> {
    let reader = std::io::Cursor::new(archive_bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|error| ProcessError::Install(error.to_string()))?;
    let entry_index = find_executable_entry_index(&mut archive, executable_name)?;
    let mut entry = archive
        .by_index(entry_index)
        .map_err(|error| ProcessError::Install(error.to_string()))?;
    let mut output = File::create(target).map_err(StorageError::Io)?;
    io::copy(&mut entry, &mut output).map_err(StorageError::Io)?;
    output.flush().map_err(StorageError::Io)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable_permission(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;

    // GitHub 发布包在不同平台上可能来自 zip 或 gzip；统一在释放后补执行位，
    // 避免 Linux/macOS 首次运行时只检测到文件存在却无法执行。
    let mut permissions = fs::metadata(path).map_err(StorageError::Io)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(StorageError::Io)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable_permission(_path: &Path) -> AppResult<()> {
    Ok(())
}

fn find_executable_entry_index<R: io::Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    executable_name: &str,
) -> AppResult<usize> {
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| ProcessError::Install(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let raw_name = entry.name().to_string();
        if raw_name
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(executable_name))
        {
            if entry.enclosed_name().is_none() {
                return Err(
                    ProcessError::Install("内嵌核心压缩包包含不安全的核心路径".into()).into(),
                );
            }
            return Ok(index);
        }
    }
    Err(ProcessError::Install(format!("内嵌核心压缩包中未找到 {executable_name}")).into())
}

fn install_embedded_geodata(cores_dir: &Path, geodata: EmbeddedGeodata) -> AppResult<Vec<PathBuf>> {
    fs::create_dir_all(cores_dir).map_err(StorageError::Io)?;
    let reader = std::io::Cursor::new(geodata.archive_bytes);
    let mut archive =
        ZipArchive::new(reader).map_err(|error| ProcessError::Install(error.to_string()))?;
    let mut installed = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ProcessError::Install(error.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let enclosed = entry.enclosed_name().ok_or_else(|| {
            ProcessError::Install("内嵌 geodata 压缩包包含不安全的数据路径".into())
        })?;
        let target = cores_dir.join(enclosed);
        if target.is_file() {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }
        let temp = target.with_extension(format!(
            "{}.tmp",
            target
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("data")
        ));

        // geodata 是核心工作目录内的运行数据；逐文件检测并原子写入，既避免覆盖用户更新的数据，
        // 也允许缺失单个文件时只补齐缺口。
        let mut output = File::create(&temp).map_err(StorageError::Io)?;
        io::copy(&mut entry, &mut output).map_err(StorageError::Io)?;
        output.flush().map_err(StorageError::Io)?;
        fs::rename(&temp, &target).map_err(StorageError::Io)?;
        installed.push(target);
    }

    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    fn zip_bytes(file_name: &str, bytes: &[u8]) -> &'static [u8] {
        zip_bytes_many(&[(file_name, bytes)])
    }

    fn zip_bytes_many(files: &[(&str, &[u8])]) -> &'static [u8] {
        let mut output = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut output);
            let mut writer = zip::ZipWriter::new(cursor);
            for (file_name, bytes) in files {
                writer
                    .start_file(*file_name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        Box::leak(output.into_boxed_slice())
    }

    #[test]
    fn installs_embedded_core_only_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let core = EmbeddedCore {
            executable_name: "mihomo-test",
            archive_bytes: zip_bytes("mihomo-test", b"fake-mihomo"),
        };

        let first = install_embedded_core(temp.path(), core).unwrap();
        let second = install_embedded_core(temp.path(), core).unwrap();

        let target = temp.path().join("mihomo-test");
        assert_eq!(first, Some(target.clone()));
        assert_eq!(second, None);
        assert_eq!(fs::read(target).unwrap(), b"fake-mihomo");
    }

    #[test]
    fn rejects_invalid_archive_core_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let core = EmbeddedCore {
            executable_name: "mihomo-test",
            archive_bytes: b"not-zip",
        };

        let result = install_embedded_core(temp.path(), core);

        assert!(result.is_err());
        assert!(!temp.path().join("mihomo-test").exists());
    }

    #[test]
    fn installs_embedded_geodata_without_overwriting_existing_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("geoip.dat"), b"existing-geoip").unwrap();
        let geodata = EmbeddedGeodata {
            archive_bytes: zip_bytes_many(&[
                ("geoip.dat", b"new-geoip"),
                ("geosite.dat", b"new-geosite"),
                ("nested/country.mmdb", b"new-mmdb"),
            ]),
        };

        let installed = install_embedded_geodata(temp.path(), geodata).unwrap();
        let installed_names = installed
            .iter()
            .map(|path| path.strip_prefix(temp.path()).unwrap().to_path_buf())
            .collect::<Vec<_>>();

        assert_eq!(
            fs::read(temp.path().join("geoip.dat")).unwrap(),
            b"existing-geoip"
        );
        assert_eq!(
            fs::read(temp.path().join("geosite.dat")).unwrap(),
            b"new-geosite"
        );
        assert_eq!(
            fs::read(temp.path().join("nested/country.mmdb")).unwrap(),
            b"new-mmdb"
        );
        assert_eq!(installed_names.len(), 2);
        assert!(installed_names.contains(&PathBuf::from("geosite.dat")));
        assert!(installed_names.contains(&PathBuf::from("nested/country.mmdb")));
    }
}
