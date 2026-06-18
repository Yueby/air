use serde::{Deserialize, Serialize};

/// 配置诊断的严重级别。
///
/// 这里只保留通用数据结构，供配置编辑、运行时合并预览等本地流程复用。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// 面向 UI 与日志的最小诊断单元。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigDiagnostic {
    pub severity: ConfigDiagnosticSeverity,
    pub path: String,
    pub message: String,
    pub suggestion: Option<String>,
}

impl ConfigDiagnostic {
    pub fn info(
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self::new(ConfigDiagnosticSeverity::Info, path, message, suggestion)
    }

    pub fn warning(
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self::new(ConfigDiagnosticSeverity::Warning, path, message, suggestion)
    }

    pub fn error(
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self::new(ConfigDiagnosticSeverity::Error, path, message, suggestion)
    }

    fn new(
        severity: ConfigDiagnosticSeverity,
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self {
            severity,
            path: path.into(),
            message: message.into(),
            suggestion,
        }
    }
}
