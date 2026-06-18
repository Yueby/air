//! 代理节点的领域编辑模型。
//!
//! `config::model::ProxyNode` 负责 YAML 往返并保留未知字段；本模块在它之上抽取 GUI 和后续
//! 仓储层需要的业务语义。节点协议更新很快，因此每个领域节点都会私有保存原始配置，未知协议
//! 和未建模字段通过 `to_config` 原样写回，避免编辑名称或复制节点时破坏用户手写配置。

mod collection;
mod display;
mod preview;
mod protocols;
mod sensitive;

pub use collection::{ProxyNodeCollection, ProxyNodeCommonSettings, ProxyNodeSettings};
pub use display::{ProxyFieldPreview, ProxyNodeDisplay};
pub use protocols::*;
pub use sensitive::SensitiveValue;
#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;
    use air_config::model::ProxyKind;

    fn docs_collection() -> ProxyNodeCollection {
        let document = ConfigDocument::parse(include_str!("../../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse");
        ProxyNodeCollection::from_document(&document.typed)
    }

    fn protocol_for<'a>(
        collection: &'a ProxyNodeCollection,
        name: &str,
    ) -> &'a ProxyProtocolSettings {
        &collection
            .find(name)
            .unwrap_or_else(|| panic!("docs config should contain proxy `{name}`"))
            .protocol
    }

    #[test]
    fn parses_docs_config_protocol_variants() {
        let collection = docs_collection();

        assert!(matches!(
            protocol_for(&collection, "socks"),
            ProxyProtocolSettings::Socks5(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "http"),
            ProxyProtocolSettings::Http(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "snell"),
            ProxyProtocolSettings::Snell(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "gost-relay-hop"),
            ProxyProtocolSettings::GostRelay(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "ss1"),
            ProxyProtocolSettings::Shadowsocks(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "ssr"),
            ProxyProtocolSettings::ShadowsocksR(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "vmess"),
            ProxyProtocolSettings::Vmess(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "vless-tcp"),
            ProxyProtocolSettings::Vless(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "trojan"),
            ProxyProtocolSettings::Trojan(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "hysteria"),
            ProxyProtocolSettings::Hysteria(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "hysteria2"),
            ProxyProtocolSettings::Hysteria2(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "wg"),
            ProxyProtocolSettings::Wireguard(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "tailscale"),
            ProxyProtocolSettings::Tailscale(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "openvpn"),
            ProxyProtocolSettings::Openvpn(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "masque"),
            ProxyProtocolSettings::Masque(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "tuic"),
            ProxyProtocolSettings::Tuic(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "ssh-out"),
            ProxyProtocolSettings::Ssh(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "mieru"),
            ProxyProtocolSettings::Mieru(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "sudoku"),
            ProxyProtocolSettings::Sudoku(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "anytls"),
            ProxyProtocolSettings::Anytls(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "trusttunnel"),
            ProxyProtocolSettings::Trusttunnel(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "dns-out"),
            ProxyProtocolSettings::Dns(_)
        ));
        assert!(matches!(
            protocol_for(&collection, "en1-direct"),
            ProxyProtocolSettings::Direct(_)
        ));
    }

    #[test]
    fn preserves_raw_unknown_protocol_nodes() {
        let document = ConfigDocument::parse(
            r#"
proxies:
  - name: future
    type: future-protocol
    server: example.com
    port: 443
    token: keep-secret
    future-field:
      nested: true
"#,
        )
        .expect("unknown proxy protocol should parse");
        let collection = ProxyNodeCollection::from_document(&document.typed);
        let node = collection.find("future").expect("future node should exist");

        assert!(matches!(node.protocol, ProxyProtocolSettings::Raw(_)));
        assert_eq!(node.to_config(), document.typed.proxies[0]);
        assert_eq!(
            node.redacted_display()
                .fields
                .iter()
                .find(|field| field.name == "extension-fields")
                .map(|field| field.value.as_str()),
            Some("2")
        );
    }

    #[test]
    fn renames_and_clones_without_losing_raw_fields() {
        let collection = docs_collection();
        let original = collection.find("ss1").expect("ss1 should exist");
        let mut renamed = original.cloned_as("ss1-copy");
        renamed.rename("ss1-renamed");

        let config = renamed.to_config();

        assert_eq!(config.name, "ss1-renamed");
        assert_eq!(config.kind, ProxyKind::Shadowsocks);
        assert_eq!(config.password, original.to_config().password);
        assert_eq!(original.to_config().name, "ss1");
    }

    #[test]
    fn redacted_debug_and_display_hide_sensitive_values() {
        let collection = docs_collection();
        let trojan = collection.find("trojan").expect("trojan should exist");
        let wireguard = collection.find("wg").expect("wireguard should exist");
        let tuic = collection.find("tuic").expect("tuic should exist");

        let debug = format!("{trojan:?} {wireguard:?} {tuic:?}");
        assert!(!debug.contains("yourpsk"));
        assert!(!debug.contains("eCtXsJZ27"));
        assert!(!debug.contains("TOKEN"));
        assert!(debug.contains("<redacted>"));

        let display = tuic.redacted_display();
        assert!(display.fields.iter().any(|field| {
            field.name == "token" && field.sensitive && field.value == "<redacted>"
        }));
        assert!(!format!("{display:?}").contains("TOKEN"));
    }
}
