use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::Path;

use async_trait::async_trait;
use serde_yaml::Value;

use air_error::{AppResult, ConfigError};

use super::{ConfigDocumentService, MihomoConfigDocument};

/// 一份已加载的 mihomo YAML 配置。
///
/// `source` 保留原始文本，用于无业务修改时做到字节级写回；`raw` 保留 YAML 值树，便于比较语义；
/// `typed` 则是 GUI 和领域层使用的强类型模型。serde_yaml 不保存注释，因此只要经过格式化输出，
/// 注释和锚点样式都会丢失，调用方应在 UI 中把这类写回标记为“规范化保存”。
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigDocument {
    pub source: String,
    pub raw: Value,
    pub typed: MihomoConfigDocument,
    original_typed: MihomoConfigDocument,
}

impl ConfigDocument {
    /// 从 YAML 文本解析配置，同时建立原始树和强类型文档，保证后续修改前有可比较的基线。
    pub fn parse(source: impl Into<String>) -> Result<Self, ConfigError> {
        let source = source.into();
        let raw = parse_raw_yaml(&source).map_err(|error| {
            tracing::error!(
                target: "air::validation",
                scope = "config-yaml-parse",
                stage = "raw-yaml",
                bytes = source.len(),
                error = %air_telemetry::redaction::redact_log_value(&error.to_string()),
                "configuration YAML parse validation failed"
            );
            error
        })?;
        let typed = parse_typed_yaml(&source).map_err(|error| {
            tracing::error!(
                target: "air::validation",
                scope = "config-yaml-parse",
                stage = "typed-model",
                bytes = source.len(),
                error = %air_telemetry::redaction::redact_log_value(&error.to_string()),
                "configuration YAML typed validation failed"
            );
            error
        })?;
        tracing::info!(
            target: "air::validation",
            scope = "config-yaml-parse",
            bytes = source.len(),
            "configuration YAML parse validation completed"
        );
        Ok(Self {
            source,
            raw,
            original_typed: typed.clone(),
            typed,
        })
    }

    /// 用新的强类型文档替换内容。此路径代表业务层已经做过修改，因此清空 `source`，避免误用旧文本。
    pub fn with_typed(typed: MihomoConfigDocument) -> Result<Self, ConfigError> {
        let raw = serde_yaml::to_value(&typed)
            .map_err(|error| ConfigError::InvalidDocument(error.to_string()))?;
        Ok(Self {
            source: String::new(),
            raw,
            original_typed: typed.clone(),
            typed,
        })
    }

    /// 将当前文档转换为待落盘 YAML。未改动时保留原文；改动后输出 serde_yaml 的规范格式。
    pub fn to_yaml_string(&self) -> Result<String, ConfigError> {
        if self.typed == self.original_typed && !self.source.is_empty() {
            return Ok(self.source.clone());
        }
        serde_yaml::to_string(&self.typed)
            .map_err(|error| ConfigError::InvalidDocument(error.to_string()))
    }
}

/// 基于普通文件系统的 YAML 配置服务。
///
/// 保存采用“先序列化、再写同目录临时文件、最后原子替换”的顺序：序列化或写临时文件失败时不会碰原文件。
#[derive(Clone, Debug, Default)]
pub struct YamlConfigDocumentService;

impl YamlConfigDocumentService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConfigDocumentService for YamlConfigDocumentService {
    async fn load(&self, path: &Path) -> AppResult<ConfigDocument> {
        let source = fs::read_to_string(path)
            .map_err(|error| ConfigError::InvalidDocument(format!("读取 YAML 失败: {error}")))?;
        Ok(ConfigDocument::parse(source)?)
    }

    async fn save(&self, path: &Path, document: &ConfigDocument) -> AppResult<()> {
        let bytes = document.to_yaml_string()?.into_bytes();
        atomic_write(path, &bytes)
            .map_err(|error| ConfigError::InvalidDocument(format!("写入 YAML 失败: {error}")))?;
        Ok(())
    }
}

pub fn load_yaml_file(path: &Path) -> Result<ConfigDocument, ConfigError> {
    let source = fs::read_to_string(path)
        .map_err(|error| ConfigError::InvalidDocument(format!("读取 YAML 失败: {error}")))?;
    ConfigDocument::parse(source)
}

pub fn save_yaml_file(path: &Path, document: &ConfigDocument) -> Result<(), ConfigError> {
    let bytes = document.to_yaml_string()?.into_bytes();
    atomic_write(path, &bytes)
        .map_err(|error| ConfigError::InvalidDocument(format!("写入 YAML 失败: {error}")))
}

pub fn parse_raw_yaml(source: &str) -> Result<Value, ConfigError> {
    serde_yaml::from_str(source).map_err(|error| ConfigError::InvalidDocument(error.to_string()))
}

pub fn parse_typed_yaml(source: &str) -> Result<MihomoConfigDocument, ConfigError> {
    serde_yaml::from_str(source).map_err(|error| ConfigError::InvalidDocument(error.to_string()))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "目标路径缺少父目录"))?;
    fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.flush()?;

    #[cfg(not(windows))]
    {
        if let Ok(metadata) = fs::metadata(path) {
            temp.as_file().set_permissions(metadata.permissions())?;
        }
    }

    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/config.yaml")
    }

    #[test]
    fn docs_config_roundtrips_semantically() {
        let source = fs::read_to_string(fixture_path()).expect("sample config should be readable");
        let document = ConfigDocument::parse(source).expect("sample config should parse");
        let serialized = document
            .to_yaml_string()
            .expect("unchanged document should serialize");
        let reparsed = ConfigDocument::parse(serialized).expect("serialized config should parse");

        assert_eq!(document.raw, reparsed.raw);
        assert_eq!(document.typed, reparsed.typed);
        assert!(!reparsed.typed.proxies.is_empty());
        assert!(!reparsed.typed.proxy_groups.is_empty());
    }

    #[test]
    fn modified_document_saves_semantically_equivalent_yaml() {
        let source = fs::read_to_string(fixture_path()).expect("sample config should be readable");
        let mut document = ConfigDocument::parse(source).expect("sample config should parse");
        document.typed.global.mixed_port = Some(19090);

        let serialized = document
            .to_yaml_string()
            .expect("modified document should serialize");
        let reparsed = ConfigDocument::parse(serialized).expect("modified config should parse");

        assert_eq!(reparsed.typed.global.mixed_port, Some(19090));
    }

    #[test]
    fn failed_save_does_not_overwrite_original_file() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let blocked_parent = temp.path().join("blocked-parent");
        let target = blocked_parent.join("config.yaml");
        fs::create_dir_all(&blocked_parent).expect("parent dir should be created");
        fs::write(&target, "mixed-port: 10801\n").expect("original config should be written");

        let document = ConfigDocument::parse("mixed-port: 19090\n").expect("config should parse");
        let error = save_yaml_file(&blocked_parent, &document).expect_err("directory save fails");

        assert!(matches!(error, ConfigError::InvalidDocument(_)));
        assert_eq!(
            fs::read_to_string(&target).expect("original config should remain readable"),
            "mixed-port: 10801\n"
        );
    }
}
