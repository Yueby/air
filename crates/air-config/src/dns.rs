//! DNS 配置的领域编辑模型。
//!
//! YAML 模型负责保留 mihomo 的原始结构；本模块负责 GUI 表单需要的字段分组、DNS
//! 服务器表达式识别、格式校验和写回。`fake-ip-filter-mode: rule` 会让 fake-ip 的命中逻辑
//! 接近路由规则的自上而下匹配：规则顺序、MATCH 兜底和 rule-provider 的 behavior 都会影响
//! 最终返回 fake-ip 还是 real-ip，错误配置可能导致直连域名被污染为 fake-ip，或代理域名提前
//! 暴露真实解析结果，因此这里只做诊断提示，不自动改写用户规则。

mod nameserver;
mod policy;
mod settings;
mod validator;
mod view_model;

pub use policy::{DnsNameserverPolicySettings, DnsPolicyValueStyle};
pub use settings::DnsConfigSettings;
pub use validator::has_dns_error;
pub use view_model::{
    DnsBooleanFormValue, DnsConfigFormViewModel, DnsFakeIpViewModel, DnsGeneralViewModel,
    DnsNameserverProtocol, DnsNameserverViewModel, DnsPolicyListViewModel, DnsPolicyRuleViewModel,
    DnsUpstreamViewModel,
};

pub const DNS_ENHANCED_MODES: &[&str] = &["normal", "redir-host", "fake-ip"];
pub const FAKE_IP_FILTER_MODES: &[&str] = &["blacklist", "whitelist", "rule"];
pub const DNS_CACHE_ALGORITHMS: &[&str] = &["lru", "arc"];
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConfigDiagnostic, ConfigDiagnosticSeverity, ConfigDocument};
    use serde_yaml::Value;

    fn docs_document() -> ConfigDocument {
        ConfigDocument::parse(include_str!("../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
    }

    fn has_diagnostic_at(
        diagnostics: &[ConfigDiagnostic],
        severity: ConfigDiagnosticSeverity,
        path: &str,
    ) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == severity && diagnostic.path == path)
    }

    #[test]
    fn extracts_dns_fields_from_docs_config() {
        let document = docs_document();
        let settings = DnsConfigSettings::from_document(&document.typed);

        assert_eq!(settings.enable, Some(false));
        assert_eq!(settings.listen.as_deref(), Some("0.0.0.0:53"));
        assert_eq!(settings.enhanced_mode.as_deref(), Some("fake-ip"));
        assert_eq!(settings.fake_ip_range.as_deref(), Some("198.18.0.1/16"));
        assert!(settings.fake_ip_filter.contains(&"*.lan".to_string()));
        assert_eq!(settings.respect_rules, Some(false));
        assert!(
            settings
                .default_nameserver
                .contains(&"tls://223.5.5.5:853".to_string())
        );
        assert!(
            settings
                .nameserver
                .contains(&"https://doh.pub/dns-query".to_string())
        );
        assert!(
            settings
                .nameserver_policy
                .iter()
                .any(|policy| policy.matcher == "geosite:cn,private,apple")
        );
        assert!(
            settings
                .nameserver_policy
                .iter()
                .any(|policy| policy.matcher == "www.baidu.com,+.google.cn")
        );
    }

    #[test]
    fn preserves_complex_policy_keys_and_value_shapes_on_writeback() {
        let document = ConfigDocument::parse(
            r#"
dns:
  nameserver-policy:
    "geosite:cn,private,apple":
      - https://doh.pub/dns-query
    "www.baidu.com,+.google.cn": [223.5.5.5, https://dns.alidns.com/dns-query]
    "geosite:category-ads-all": rcode://success
"#,
        )
        .expect("dns policy should parse");
        let mut typed = document.typed.clone();
        let mut settings = DnsConfigSettings::from_document(&typed);
        settings.nameserver.push("tcp://1.1.1.1".into());
        settings.apply_to_document(&mut typed);

        let dns = typed.dns.expect("dns should be written");
        assert!(
            dns.nameserver_policy
                .contains_key("geosite:cn,private,apple")
        );
        assert!(
            dns.nameserver_policy
                .contains_key("www.baidu.com,+.google.cn")
        );
        assert_eq!(
            dns.nameserver_policy.get("geosite:category-ads-all"),
            Some(&Value::String("rcode://success".to_string()))
        );
        assert_eq!(dns.nameserver, vec!["tcp://1.1.1.1"]);
    }

    #[test]
    fn validates_dns_format_errors() {
        let settings = DnsConfigSettings {
            listen: Some("[::1:53".into()),
            enhanced_mode: Some("bad-mode".into()),
            fake_ip_range: Some("fdfe:dcba::1/64".into()),
            fake_ip_range6: Some("198.18.0.1/16".into()),
            nameserver: vec!["https:///dns-query".into()],
            nameserver_policy: vec![DnsNameserverPolicySettings::new(
                "bad matcher",
                vec!["udp://".into()],
            )],
            ..Default::default()
        };

        let diagnostics = settings.validate();

        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.listen"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.enhanced-mode"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.fake-ip-range"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.fake-ip-range6"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.nameserver[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.nameserver-policy[0].matcher"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "dns.nameserver-policy[0].nameservers[0]"
        ));
        assert!(has_dns_error(&diagnostics));
    }

    #[test]
    fn prepares_view_model_with_nameserver_protocols_and_policy_preview() {
        let settings = DnsConfigSettings {
            enable: Some(true),
            listen: Some("127.0.0.1:53".into()),
            enhanced_mode: Some("fake-ip".into()),
            fake_ip_filter_mode: Some("rule".into()),
            nameserver: vec![
                "tls://1.1.1.1:853".into(),
                "https://dns.alidns.com/dns-query#h3=true".into(),
                "tcp://8.8.8.8#Proxy".into(),
            ],
            nameserver_policy: vec![DnsNameserverPolicySettings {
                matcher: "geosite:cn,private".into(),
                nameservers: vec!["https://doh.pub/dns-query".into()],
                value_style: DnsPolicyValueStyle::Sequence,
                passthrough: None,
            }],
            ..Default::default()
        };

        let view_model = DnsConfigFormViewModel::from(&settings);

        assert!(view_model.general.enable.value);
        assert!(view_model.fake_ip.rule_mode_risk);
        assert_eq!(
            view_model.upstream.nameserver[0].protocol,
            DnsNameserverProtocol::Tls
        );
        assert!(view_model.upstream.nameserver[1].force_h3);
        assert!(view_model.upstream.nameserver[2].has_route_hint);
        assert_eq!(
            view_model.policies.nameserver_policy[0].value_preview,
            "https://doh.pub/dns-query"
        );
        assert!(has_diagnostic_at(
            &view_model.diagnostics,
            ConfigDiagnosticSeverity::Info,
            "dns.fake-ip-filter-mode"
        ));
    }
}
