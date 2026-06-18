use serde::{Deserialize, Serialize};

/// 规则行。
///
/// mihomo 规则语法包含逗号分隔、括号表达式和 provider 引用；011 阶段只建立文档模型，
/// 因此保留原始行，避免解析器尚未完善时改变用户书写。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleLine {
    pub raw: String,
}
