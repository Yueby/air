use std::fmt;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use air_config::model::{MihomoConfigDocument, ProxyKind, ProxyNode};

use super::display::ProxyNodeDisplay;
use super::preview::{normalize_optional_string, yaml_value_preview};
use super::protocols::ProxyProtocolSettings;
/// 一组代理节点。集合层只处理 `proxies` section，不负责代理组引用校验。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProxyNodeCollection {
    pub nodes: Vec<ProxyNodeSettings>,
}

impl ProxyNodeCollection {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            nodes: document
                .proxies
                .iter()
                .map(ProxyNodeSettings::from_config)
                .collect(),
        }
    }

    /// 将节点写回完整配置文档。名称引用是否仍合法交给后续合并/校验任务统一处理。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        document.proxies = self
            .nodes
            .iter()
            .map(ProxyNodeSettings::to_config)
            .collect();
    }

    pub fn find(&self, name: &str) -> Option<&ProxyNodeSettings> {
        self.nodes.iter().find(|node| node.common.name == name)
    }
}

/// 单个代理节点的领域表示。
///
/// `raw` 不参与派生 Debug，防止密码、私钥、token 等敏感配置被日志或测试失败输出带出。
#[derive(Clone, PartialEq)]
pub struct ProxyNodeSettings {
    pub common: ProxyNodeCommonSettings,
    pub protocol: ProxyProtocolSettings,
    raw: ProxyNode,
}

impl ProxyNodeSettings {
    pub fn from_config(node: &ProxyNode) -> Self {
        Self {
            common: ProxyNodeCommonSettings::from_config(node),
            protocol: ProxyProtocolSettings::from_config(node),
            raw: node.clone(),
        }
    }

    pub fn to_config(&self) -> ProxyNode {
        let mut node = self.raw.clone();
        self.common.apply_to_config(&mut node);
        node
    }

    /// 返回原始节点副本，供订阅/仓储任务在需要完整 YAML 结构时继续处理。
    pub fn raw_node(&self) -> ProxyNode {
        self.to_config()
    }

    pub fn rename(&mut self, new_name: impl Into<String>) {
        let new_name = new_name.into();
        self.common.name = new_name.clone();
        self.raw.name = new_name;
    }

    pub fn cloned_as(&self, new_name: impl Into<String>) -> Self {
        let mut cloned = self.clone();
        cloned.rename(new_name);
        cloned
    }

    pub fn redacted_display(&self) -> ProxyNodeDisplay {
        ProxyNodeDisplay::from(self)
    }
}

impl fmt::Debug for ProxyNodeSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProxyNodeSettings")
            .field("common", &self.common)
            .field("protocol", &self.protocol)
            .field("raw", &"<preserved>")
            .finish()
    }
}

/// 跨协议公共字段。端口、server 等字段保留 YAML Value，是为了兼容示例里占位列表和未来表达式。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyNodeCommonSettings {
    pub name: String,
    pub kind: ProxyKind,
    pub server: Option<Value>,
    pub port: Option<Value>,
    pub udp: Option<bool>,
    pub dialer_proxy: Option<String>,
    pub skip_cert_verify: Option<bool>,
}

impl ProxyNodeCommonSettings {
    fn from_config(node: &ProxyNode) -> Self {
        Self {
            name: node.name.clone(),
            kind: node.kind.clone(),
            server: node.server.clone(),
            port: node.port.clone(),
            udp: node.udp,
            dialer_proxy: node.dialer_proxy.clone(),
            skip_cert_verify: node.skip_cert_verify,
        }
    }

    fn apply_to_config(&self, node: &mut ProxyNode) {
        node.name = self.name.clone();
        node.kind = self.kind.clone();
        node.server = self.server.clone();
        node.port = self.port.clone();
        node.udp = self.udp;
        node.dialer_proxy = normalize_optional_string(self.dialer_proxy.as_deref());
        node.skip_cert_verify = self.skip_cert_verify;
    }

    pub fn endpoint_preview(&self) -> String {
        match (&self.server, &self.port) {
            (Some(server), Some(port)) => {
                format!(
                    "{}:{}",
                    yaml_value_preview(server),
                    yaml_value_preview(port)
                )
            }
            (Some(server), None) => yaml_value_preview(server),
            (None, Some(port)) => format!(":{}", yaml_value_preview(port)),
            (None, None) => String::new(),
        }
    }
}
