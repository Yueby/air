//! base64 节点订阅解析。
//!
//! 远程订阅除了 mihomo/Clash YAML，还有一种极常见的“分享链接订阅”：HTTP 正文是一段
//! base64 文本，解码后是按行排列的 `ss://`、`vmess://`、`trojan://`、`vless://` 等分享链接。
//! 本模块负责把这类内容转换成 [`ProxyNode`] 列表，行为对齐 mihomo 的 `common/convert`
//! (`ConvertsV2Ray`)：
//!
//! - 正文先尝试按 base64 / base64url 解码，失败则回退当作明文分享链接列表；
//! - 逐行解析，空行、注释、无法识别的行一律跳过而不是整体失败；
//! - 每条链接按 scheme 分派到对应协议解析器，映射到跨协议的 [`ProxyNode`] 字段，协议专属
//!   参数写入 `extensions` 原样保留；
//! - 重名节点追加 `-01`、`-02` 后缀去重。
//!
//! 本模块只做“纯转换”，不产生诊断日志；诊断与 `ParsedSubscription` 组装留在 `update.rs`，
//! 以便复用其脱敏日志入口。

use std::collections::{BTreeMap, BTreeSet};

use air_config::model::{ProxyKind, ProxyNode};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use serde_yaml::{Mapping, Value};

/// 分享链接批量转换结果。
///
/// `skipped` 统计“识别出 scheme 但解析失败”的行；`unsupported_schemes` 收集出现过但当前
/// 未实现的协议 scheme，供上层生成一条聚合的告警诊断，避免逐行刷屏。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShareLinkConversion {
    pub proxies: Vec<ProxyNode>,
    pub skipped: usize,
    pub unsupported_schemes: BTreeSet<String>,
}

/// 从订阅正文提取“分享链接明文”。
///
/// 优先按 base64 解码（兼容标准/URL-safe、有无填充、含换行的情况）；只有当解码结果确实包含
/// `://` 时才认定为分享链接负载。若正文本身就是明文分享链接列表（部分机场直接返回明文），
/// 也直接返回。两者都不满足时返回 `None`，交由上层报“无法识别”。
pub fn extract_share_link_payload(content: &str) -> Option<String> {
    if let Some(decoded) = decode_base64_relaxed(content) {
        if let Ok(text) = String::from_utf8(decoded) {
            if contains_supported_share_link(&text) {
                return Some(text);
            }
        }
    }

    if contains_supported_share_link(content) {
        return Some(content.to_string());
    }

    None
}

/// 把分享链接明文按行转换为节点列表。
pub fn convert_share_links(payload: &str) -> ShareLinkConversion {
    let mut result = ShareLinkConversion::default();
    // 记录已使用的节点名及其重复次数，用于稳定去重。
    let mut used_names: BTreeMap<String, usize> = BTreeMap::new();

    for raw_line in payload.lines() {
        let line = raw_line.trim();
        // 跳过空行与不含链接的注释行；含 `://` 的行即使以 # 开头也继续尝试解析。
        if line.is_empty() || (line.starts_with('#') && !line.contains("://")) {
            continue;
        }
        let Some((scheme, _)) = line.split_once("://") else {
            continue;
        };

        let scheme = scheme.trim().to_ascii_lowercase();
        // scheme 必须是安全的 URI scheme 名，避免把普通文本或错误页中 `...://` 前的大段
        // 内容记录到诊断里，进而泄漏 URL/token 等敏感信息。
        if !is_safe_scheme(&scheme) {
            result.skipped += 1;
            continue;
        }
        let parsed = match scheme.as_str() {
            "ss" => parse_shadowsocks(line),
            "ssr" => parse_shadowsocksr(line),
            "vmess" => parse_vmess(line),
            "vless" => parse_vless(line),
            "trojan" => parse_trojan(line),
            "hysteria2" | "hy2" => parse_hysteria2(line),
            "hysteria" => parse_hysteria(line),
            "tuic" => parse_tuic(line),
            other => {
                result.unsupported_schemes.insert(other.to_string());
                continue;
            }
        };

        match parsed {
            Some(mut node) => {
                node.name = dedupe_name(&mut used_names, node.name);
                result.proxies.push(node);
            }
            None => result.skipped += 1,
        }
    }

    result
}

/// 判断文本中是否至少包含一条当前支持的分享链接。
///
/// 只检查行首或空白后的 URL token，不把任意 `https://`、错误页 URL 或 YAML 标量误判成节点
/// 订阅。这样 `SubscriptionParser` 在普通 YAML 语法错误时仍能返回真实的 YAML 诊断。
fn contains_supported_share_link(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim_start_matches(|ch: char| ch.is_whitespace() || ch == '#');
        let Some((scheme, _)) = trimmed.split_once("://") else {
            return false;
        };
        is_supported_scheme(&scheme.to_ascii_lowercase())
    })
}

fn is_supported_scheme(scheme: &str) -> bool {
    matches!(
        scheme,
        "ss" | "ssr" | "vmess" | "vless" | "trojan" | "hysteria2" | "hy2" | "hysteria" | "tuic"
    )
}

fn is_safe_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

/// 若节点名为空则用占位名；再按出现次数追加 `-NN` 后缀，保证同一订阅内节点名唯一。
fn dedupe_name(used: &mut BTreeMap<String, usize>, name: String) -> String {
    let base = if name.trim().is_empty() {
        "proxy".to_string()
    } else {
        name
    };
    let counter = used.entry(base.clone()).or_insert(0);
    let resolved = if *counter == 0 {
        base.clone()
    } else {
        format!("{base}-{:02}", *counter)
    };
    *counter += 1;
    resolved
}

// ------------------------------------------------------------------
// 各协议解析器：解析失败时返回 None（该行被计入 skipped）。
// ------------------------------------------------------------------

/// `ss://` 兼容两种形态：
/// 1. SIP002：`ss://base64(method:password)@host:port?plugin=...#name`
/// 2. 全量 base64：`ss://base64(method:password@host:port)#name`
fn parse_shadowsocks(line: &str) -> Option<ProxyNode> {
    let after = line.strip_prefix("ss://")?;
    let (before_fragment, fragment) = split_fragment(after);
    let name = fragment.map(percent_decode).unwrap_or_default();
    let (main, query) = split_query(before_fragment);

    let (method, password, host, port);
    if let Some((userinfo, host_port)) = main.rsplit_once('@') {
        // SIP002：userinfo 是 base64(method:password)，也兼容明文。
        let creds = decode_userinfo(userinfo)?;
        let (m, p) = creds.split_once(':')?;
        let (h, port_str) = host_port.rsplit_once(':')?;
        method = m.to_string();
        password = p.to_string();
        host = h.to_string();
        port = port_str.parse::<u16>().ok()?;
    } else {
        // 全量 base64 形态：整体解码后为 method:password@host:port。
        let decoded = decode_base64_relaxed(main)?;
        let text = String::from_utf8(decoded).ok()?;
        let (creds, host_port) = text.rsplit_once('@')?;
        let (m, p) = creds.split_once(':')?;
        let (h, port_str) = host_port.rsplit_once(':')?;
        method = m.to_string();
        password = p.to_string();
        host = h.to_string();
        port = port_str.trim().parse::<u16>().ok()?;
    }

    let mut node = base_node(ProxyKind::Shadowsocks, name, &host, port);
    node.cipher = Some(Value::from(method));
    node.password = Some(Value::from(password));
    node.udp = Some(true);

    // 解析 SIP002 plugin 参数，例如 plugin=obfs-local;obfs=http;obfs-host=...
    for (key, value) in query_pairs(query) {
        if key == "plugin" {
            apply_ss_plugin(&mut node, &value);
        }
    }

    Some(node)
}

/// 解析 SS 的 plugin 描述串，映射到 `plugin` 与 `plugin-opts`。
fn apply_ss_plugin(node: &mut ProxyNode, plugin: &str) {
    let mut parts = plugin.split(';');
    let Some(raw_name) = parts.next() else {
        return;
    };
    let mut opts: BTreeMap<String, Value> = BTreeMap::new();
    let mut tls = false;
    for part in parts {
        if part == "tls" {
            tls = true;
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            opts.insert(k.to_string(), Value::from(v.to_string()));
        }
    }

    // 归一化插件名：obfs-local 与 simple-obfs 都对应 mihomo 的 obfs 插件。
    let plugin_name = match raw_name {
        "obfs-local" | "simple-obfs" => "obfs",
        other => other,
    };
    node.plugin = Some(plugin_name.to_string());

    // 统一整理为 mihomo plugin-opts 键：mode/host/path/tls。
    let mut plugin_opts: BTreeMap<String, Value> = BTreeMap::new();
    if plugin_name == "obfs" {
        if let Some(mode) = opts.get("obfs") {
            plugin_opts.insert("mode".to_string(), mode.clone());
        }
        if let Some(host) = opts.get("obfs-host") {
            plugin_opts.insert("host".to_string(), host.clone());
        }
    } else {
        if let Some(mode) = opts.get("mode") {
            plugin_opts.insert("mode".to_string(), mode.clone());
        }
        if let Some(host) = opts.get("host") {
            plugin_opts.insert("host".to_string(), host.clone());
        }
        if let Some(path) = opts.get("path") {
            plugin_opts.insert("path".to_string(), path.clone());
        }
        if tls {
            plugin_opts.insert("tls".to_string(), Value::from(true));
        }
    }
    node.plugin_opts = plugin_opts;
}

/// `ssr://base64(host:port:protocol:method:obfs:base64(password)/?params)`
fn parse_shadowsocksr(line: &str) -> Option<ProxyNode> {
    let after = line.strip_prefix("ssr://")?;
    let decoded = decode_base64_relaxed(after)?;
    let text = String::from_utf8(decoded).ok()?;
    let (head, query) = match text.split_once("/?") {
        Some((head, query)) => (head, query),
        None => (text.as_str(), ""),
    };

    // head = host:port:protocol:method:obfs:base64(password)
    let parts: Vec<&str> = head.split(':').collect();
    if parts.len() < 6 {
        return None;
    }
    let host = parts[0].to_string();
    let port = parts[1].parse::<u16>().ok()?;
    let protocol = parts[2].to_string();
    let method = parts[3].to_string();
    let obfs = parts[4].to_string();
    // 密码段可能自身含 ':'，用 join 复原后再 base64 解码。
    let password_b64 = parts[5..].join(":");
    let password = decode_base64_relaxed(&password_b64)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or(password_b64);

    let mut name = String::new();
    let mut obfs_param = String::new();
    let mut protocol_param = String::new();
    for (key, value) in query_pairs(query) {
        // SSR 的参数值也是 base64（url-safe）。
        let decoded = decode_base64_relaxed(&value)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or(value);
        match key.as_str() {
            "remarks" => name = decoded,
            "obfsparam" => obfs_param = decoded,
            "protoparam" => protocol_param = decoded,
            _ => {}
        }
    }

    let mut node = base_node(ProxyKind::ShadowsocksR, name, &host, port);
    node.cipher = Some(Value::from(method));
    node.password = Some(Value::from(password));
    node.udp = Some(true);
    node.extensions
        .insert("protocol".to_string(), Value::from(protocol));
    node.extensions
        .insert("obfs".to_string(), Value::from(obfs));
    if !obfs_param.is_empty() {
        node.extensions
            .insert("obfs-param".to_string(), Value::from(obfs_param));
    }
    if !protocol_param.is_empty() {
        node.extensions
            .insert("protocol-param".to_string(), Value::from(protocol_param));
    }
    Some(node)
}

/// `vmess://base64(JSON)`（v2rayN 形态，最常见）。
fn parse_vmess(line: &str) -> Option<ProxyNode> {
    let after = line.strip_prefix("vmess://")?;
    let decoded = decode_base64_relaxed(after)?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    let name = json_string(&json, "ps").unwrap_or_default();
    let host_field = json_string(&json, "add")?;
    let port = json_port(&json, "port")?;

    let mut node = base_node(ProxyKind::Vmess, name, &host_field, port);
    node.uuid = json_string(&json, "id").map(Value::from);
    node.cipher = Some(Value::from(
        json_string(&json, "scy").unwrap_or_else(|| "auto".to_string()),
    ));
    // alterId 默认 0；mihomo YAML 使用 camelCase 键，放入 extensions 原样输出。
    let alter_id = json_string(&json, "aid")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    node.extensions
        .insert("alterId".to_string(), Value::from(alter_id));
    node.udp = Some(true);

    // tls：字段值为 "tls" 时启用。
    let tls_enabled = json_string(&json, "tls")
        .map(|value| value.eq_ignore_ascii_case("tls"))
        .unwrap_or(false);
    if tls_enabled {
        node.tls = Some(true);
    }
    if let Some(sni) = json_string(&json, "sni").filter(|value| !value.is_empty()) {
        node.servername = Some(sni);
    }
    let alpn = json_string(&json, "alpn").unwrap_or_default();
    if !alpn.is_empty() {
        node.alpn = split_alpn(&alpn);
    }

    // 传输层：net 决定网络类型；host/path 落到对应 *-opts。
    let network = json_string(&json, "net").unwrap_or_else(|| "tcp".to_string());
    let transport_host = json_string(&json, "host").unwrap_or_default();
    let transport_path = json_string(&json, "path").unwrap_or_default();
    apply_vmess_transport(&mut node, &network, &transport_host, &transport_path);

    Some(node)
}

/// 依据 vmess 的 `net` 字段设置 network 与传输 opts。
fn apply_vmess_transport(node: &mut ProxyNode, network: &str, host: &str, path: &str) {
    match network {
        "ws" | "httpupgrade" => {
            node.network = Some(Value::from("ws"));
            let mut ws = Mapping::new();
            if network == "httpupgrade" {
                // mihomo 用 ws-opts.v2ray-http-upgrade 表达 V2Ray HTTPUpgrade 传输，不能把
                // `httpupgrade` 静默退化成普通 websocket。
                ws.insert(Value::from("v2ray-http-upgrade"), Value::from(true));
            }
            if !path.is_empty() {
                ws.insert(Value::from("path"), Value::from(path.to_string()));
            }
            if !host.is_empty() {
                let mut headers = Mapping::new();
                headers.insert(Value::from("Host"), Value::from(host.to_string()));
                ws.insert(Value::from("headers"), Value::Mapping(headers));
            }
            if !ws.is_empty() {
                node.extensions
                    .insert("ws-opts".to_string(), Value::Mapping(ws));
            }
        }
        "grpc" => {
            node.network = Some(Value::from("grpc"));
            if !path.is_empty() {
                let mut grpc = Mapping::new();
                grpc.insert(
                    Value::from("grpc-service-name"),
                    Value::from(path.to_string()),
                );
                node.extensions
                    .insert("grpc-opts".to_string(), Value::Mapping(grpc));
            }
        }
        "h2" => {
            node.network = Some(Value::from("h2"));
            let mut h2 = Mapping::new();
            if !path.is_empty() {
                h2.insert(Value::from("path"), Value::from(path.to_string()));
            }
            if !host.is_empty() {
                h2.insert(
                    Value::from("host"),
                    Value::Sequence(vec![Value::from(host.to_string())]),
                );
            }
            if !h2.is_empty() {
                node.extensions
                    .insert("h2-opts".to_string(), Value::Mapping(h2));
            }
        }
        other => {
            // tcp 及其它类型：仅记录 network，专属参数留给上游按需扩展。
            node.network = Some(Value::from(other.to_string()));
        }
    }
}

/// `vless://uuid@host:port?security=...&type=...&sni=...&flow=...#name`
fn parse_vless(line: &str) -> Option<ProxyNode> {
    let url = url::Url::parse(line).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port()?;
    let name = url.fragment().map(percent_decode).unwrap_or_default();
    let uuid = percent_decode(url.username());
    if uuid.is_empty() {
        return None;
    }

    let mut node = base_node(ProxyKind::Vless, name, &host, port);
    node.uuid = Some(Value::from(uuid));
    node.udp = Some(true);

    let params: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
    let security = params.get("security").map(String::as_str).unwrap_or("none");
    if matches!(security, "tls" | "reality" | "xtls") {
        node.tls = Some(true);
    }
    if let Some(sni) =
        query_param(&params, &["sni", "serverName"]).filter(|value| !value.is_empty())
    {
        node.servername = Some(sni.clone());
    }
    if is_truthy_query(query_param(
        &params,
        &[
            "allowInsecure",
            "allowinsecure",
            "insecure",
            "skip-cert-verify",
        ],
    )) {
        node.skip_cert_verify = Some(true);
    }
    if let Some(fp) = params.get("fp").filter(|value| !value.is_empty()) {
        node.client_fingerprint = Some(Value::from(fp.clone()));
    }
    if let Some(alpn) = params.get("alpn").filter(|value| !value.is_empty()) {
        node.alpn = split_alpn(alpn);
    }
    if let Some(flow) = params.get("flow").filter(|value| !value.is_empty()) {
        node.extensions
            .insert("flow".to_string(), Value::from(flow.clone()));
    }
    // reality 公钥/短 ID。
    if security == "reality" {
        let mut reality = Mapping::new();
        if let Some(pbk) = params.get("pbk").filter(|value| !value.is_empty()) {
            reality.insert(Value::from("public-key"), Value::from(pbk.clone()));
        }
        if let Some(sid) = params.get("sid") {
            reality.insert(Value::from("short-id"), Value::from(sid.clone()));
        }
        if !reality.is_empty() {
            node.extensions
                .insert("reality-opts".to_string(), Value::Mapping(reality));
        }
    }

    let network = params.get("type").map(String::as_str).unwrap_or("tcp");
    let ws_host = query_param(&params, &["host"]).cloned().unwrap_or_default();
    let ws_path = query_param(&params, &["path"]).cloned().unwrap_or_default();
    let service_name = query_param(&params, &["serviceName", "servicename"])
        .cloned()
        .unwrap_or_default();
    apply_vx_transport(&mut node, network, &ws_host, &ws_path, &service_name);

    Some(node)
}

/// `trojan://password@host:port?sni=...&type=...#name`
fn parse_trojan(line: &str) -> Option<ProxyNode> {
    let url = url::Url::parse(line).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port()?;
    let name = url.fragment().map(percent_decode).unwrap_or_default();
    let password = percent_decode(url.username());
    if password.is_empty() {
        return None;
    }

    let mut node = base_node(ProxyKind::Trojan, name, &host, port);
    node.password = Some(Value::from(password));
    node.udp = Some(true);
    // trojan 基于 TLS，默认开启。
    node.tls = Some(true);

    let params: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(sni) =
        query_param(&params, &["sni", "serverName"]).filter(|value| !value.is_empty())
    {
        node.sni = Some(sni.clone());
    }
    if is_truthy_query(query_param(
        &params,
        &[
            "allowInsecure",
            "allowinsecure",
            "insecure",
            "skip-cert-verify",
        ],
    )) {
        node.skip_cert_verify = Some(true);
    }
    if let Some(alpn) = params.get("alpn").filter(|value| !value.is_empty()) {
        node.alpn = split_alpn(alpn);
    }
    if let Some(fp) = params.get("fp").filter(|value| !value.is_empty()) {
        node.client_fingerprint = Some(Value::from(fp.clone()));
    }

    let network = params.get("type").map(String::as_str).unwrap_or("tcp");
    let ws_host = query_param(&params, &["host"]).cloned().unwrap_or_default();
    let ws_path = query_param(&params, &["path"]).cloned().unwrap_or_default();
    let service_name = query_param(&params, &["serviceName", "servicename"])
        .cloned()
        .unwrap_or_default();
    apply_vx_transport(&mut node, network, &ws_host, &ws_path, &service_name);

    Some(node)
}

/// vless/trojan 共用的传输层映射（ws / grpc）。
fn apply_vx_transport(node: &mut ProxyNode, network: &str, host: &str, path: &str, service: &str) {
    match network {
        "ws" | "httpupgrade" => {
            node.network = Some(Value::from("ws"));
            let mut ws = Mapping::new();
            if network == "httpupgrade" {
                // mihomo 通过 ws-opts.v2ray-http-upgrade 区分 HTTPUpgrade 与普通 websocket。
                ws.insert(Value::from("v2ray-http-upgrade"), Value::from(true));
            }
            if !path.is_empty() {
                ws.insert(Value::from("path"), Value::from(path.to_string()));
            }
            if !host.is_empty() {
                let mut headers = Mapping::new();
                headers.insert(Value::from("Host"), Value::from(host.to_string()));
                ws.insert(Value::from("headers"), Value::Mapping(headers));
            }
            if !ws.is_empty() {
                node.extensions
                    .insert("ws-opts".to_string(), Value::Mapping(ws));
            }
        }
        "grpc" => {
            node.network = Some(Value::from("grpc"));
            let service_name = if !service.is_empty() { service } else { path };
            if !service_name.is_empty() {
                let mut grpc = Mapping::new();
                grpc.insert(
                    Value::from("grpc-service-name"),
                    Value::from(service_name.to_string()),
                );
                node.extensions
                    .insert("grpc-opts".to_string(), Value::Mapping(grpc));
            }
        }
        "tcp" => {}
        other => {
            node.network = Some(Value::from(other.to_string()));
        }
    }
}

/// `hysteria2://password@host:port?sni=...&obfs=...#name`（含 `hy2://` 别名）。
fn parse_hysteria2(line: &str) -> Option<ProxyNode> {
    let url = url::Url::parse(line).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port().unwrap_or(443);
    let name = url.fragment().map(percent_decode).unwrap_or_default();

    let mut node = base_node(ProxyKind::Hysteria2, name, &host, port);
    let password = percent_decode(url.username());
    if !password.is_empty() {
        node.password = Some(Value::from(password));
    }

    let params: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(sni) = params.get("sni").filter(|value| !value.is_empty()) {
        node.sni = Some(sni.clone());
    }
    if matches!(
        params.get("insecure").map(String::as_str),
        Some("1") | Some("true")
    ) {
        node.skip_cert_verify = Some(true);
    }
    if let Some(alpn) = params.get("alpn").filter(|value| !value.is_empty()) {
        node.alpn = split_alpn(alpn);
    }
    if let Some(obfs) = params.get("obfs").filter(|value| !value.is_empty()) {
        node.extensions
            .insert("obfs".to_string(), Value::from(obfs.clone()));
    }
    if let Some(pwd) =
        query_param(&params, &["obfs-password", "obfsPassword"]).filter(|value| !value.is_empty())
    {
        node.extensions
            .insert("obfs-password".to_string(), Value::from(pwd.clone()));
    }
    if let Some(pin) = params.get("pinSHA256").filter(|value| !value.is_empty()) {
        node.fingerprint = Some(pin.clone());
    }
    Some(node)
}

/// `hysteria://host:port?peer=...&auth=...&upmbps=...#name`（Hysteria v1）。
fn parse_hysteria(line: &str) -> Option<ProxyNode> {
    let url = url::Url::parse(line).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port()?;
    let name = url.fragment().map(percent_decode).unwrap_or_default();

    let mut node = base_node(ProxyKind::Hysteria, name, &host, port);
    let params: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(peer) = params.get("peer").filter(|value| !value.is_empty()) {
        node.sni = Some(peer.clone());
    }
    if matches!(
        params.get("insecure").map(String::as_str),
        Some("1") | Some("true")
    ) {
        node.skip_cert_verify = Some(true);
    }
    if let Some(alpn) = params.get("alpn").filter(|value| !value.is_empty()) {
        node.alpn = split_alpn(alpn);
    }
    if let Some(auth) = params.get("auth").filter(|value| !value.is_empty()) {
        node.extensions
            .insert("auth-str".to_string(), Value::from(auth.clone()));
    }
    if let Some(obfs) = params.get("obfs").filter(|value| !value.is_empty()) {
        node.extensions
            .insert("obfs".to_string(), Value::from(obfs.clone()));
    }
    if let Some(protocol) = params.get("protocol").filter(|value| !value.is_empty()) {
        node.extensions
            .insert("protocol".to_string(), Value::from(protocol.clone()));
    }
    if let Some(up) = params.get("up").or_else(|| params.get("upmbps")) {
        node.extensions
            .insert("up".to_string(), Value::from(up.clone()));
    }
    if let Some(down) = params.get("down").or_else(|| params.get("downmbps")) {
        node.extensions
            .insert("down".to_string(), Value::from(down.clone()));
    }
    Some(node)
}

/// `tuic://uuid:password@host:port?congestion_control=...&alpn=...#name`
fn parse_tuic(line: &str) -> Option<ProxyNode> {
    let url = url::Url::parse(line).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port()?;
    let name = url.fragment().map(percent_decode).unwrap_or_default();

    let mut node = base_node(ProxyKind::Tuic, name, &host, port);
    let uuid = percent_decode(url.username());
    if !uuid.is_empty() {
        node.uuid = Some(Value::from(uuid));
    }
    if let Some(password) = url.password() {
        node.password = Some(Value::from(percent_decode(password)));
    }
    node.udp = Some(true);

    let params: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(cc) = params
        .get("congestion_control")
        .filter(|value| !value.is_empty())
    {
        node.extensions
            .insert("congestion-controller".to_string(), Value::from(cc.clone()));
    }
    if let Some(sni) = params.get("sni").filter(|value| !value.is_empty()) {
        node.sni = Some(sni.clone());
    }
    if let Some(alpn) = params.get("alpn").filter(|value| !value.is_empty()) {
        node.alpn = split_alpn(alpn);
    }
    if matches!(
        params.get("disable_sni").map(String::as_str),
        Some("1") | Some("true")
    ) {
        node.extensions
            .insert("disable-sni".to_string(), Value::from(true));
    }
    if let Some(mode) = params
        .get("udp_relay_mode")
        .filter(|value| !value.is_empty())
    {
        node.extensions
            .insert("udp-relay-mode".to_string(), Value::from(mode.clone()));
    }
    Some(node)
}

// ------------------------------------------------------------------
// 通用辅助函数
// ------------------------------------------------------------------

/// 构造带有公共字段（名称、类型、服务器、端口）的节点骨架。
fn base_node(kind: ProxyKind, name: String, server: &str, port: u16) -> ProxyNode {
    ProxyNode {
        name,
        kind,
        server: Some(Value::from(server.to_string())),
        port: Some(Value::from(port as u64)),
        ..ProxyNode::default()
    }
}

/// 尝试多种 base64 变体解码：先剔除所有空白（兼容换行包裹），再依次尝试标准/URL-safe、有无填充。
fn decode_base64_relaxed(input: &str) -> Option<Vec<u8>> {
    let compact: String = input.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.is_empty() {
        return None;
    }
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(compact.as_bytes()) {
            return Some(bytes);
        }
    }
    None
}

/// SS 的 userinfo 解码：优先按 base64 解出 `method:password`，否则回退百分号解码后的明文。
fn decode_userinfo(userinfo: &str) -> Option<String> {
    if let Some(bytes) = decode_base64_relaxed(userinfo) {
        if let Ok(text) = String::from_utf8(bytes) {
            if text.contains(':') {
                return Some(text);
            }
        }
    }
    let decoded = percent_decode(userinfo);
    if decoded.contains(':') {
        Some(decoded)
    } else {
        None
    }
}

/// 以 `#` 切出 fragment（节点名），返回 (fragment 前内容, fragment)。
fn split_fragment(input: &str) -> (&str, Option<&str>) {
    match input.split_once('#') {
        Some((head, fragment)) => (head, Some(fragment)),
        None => (input, None),
    }
}

/// 以 `?` 切出 query，返回 (query 前内容, query)。
fn split_query(input: &str) -> (&str, &str) {
    match input.split_once('?') {
        Some((head, query)) => (head, query),
        None => (input, ""),
    }
}

/// 解析 `a=b&c=d` 形式的 query，值做百分号解码。
fn query_pairs(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((key.to_string(), percent_decode(value)))
        })
        .collect()
}

/// 按多个别名读取 query 参数；先精确匹配，再大小写不敏感匹配，兼容不同客户端生成的分享链接。
fn query_param<'a>(params: &'a BTreeMap<String, String>, names: &[&str]) -> Option<&'a String> {
    for name in names {
        if let Some(value) = params.get(*name) {
            return Some(value);
        }
    }
    params.iter().find_map(|(key, value)| {
        names
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
            .then_some(value)
    })
}

fn is_truthy_query(value: Option<&String>) -> bool {
    matches!(
        value.map(|value| value.as_str()),
        Some("1") | Some("true") | Some("True") | Some("TRUE") | Some("yes") | Some("on")
    )
}

/// 将逗号分隔的 alpn 列表拆成字符串向量。
fn split_alpn(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// 从 serde_json 值中取字符串；数字会被转换成字符串，兼容机场把 port/aid 写成数字或字符串。
fn json_string(json: &serde_json::Value, key: &str) -> Option<String> {
    match json.get(key)? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

/// 从 serde_json 值中解析端口，兼容数字与字符串两种写法。
fn json_port(json: &serde_json::Value, key: &str) -> Option<u16> {
    match json.get(key)? {
        serde_json::Value::Number(value) => value.as_u64().and_then(|n| u16::try_from(n).ok()),
        serde_json::Value::String(value) => value.trim().parse::<u16>().ok(),
        _ => None,
    }
}

/// 轻量百分号解码：把 `%XX` 还原为字节后按 UTF-8 lossy 解释。分享链接的 fragment/参数常见此编码。
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_base64_payload_and_plaintext() {
        // base64 编码的两行分享链接。
        let encoded = STANDARD.encode("ss://YWVzLTI1Ni1nY206cGFzcw==@host:443#Node\n");
        let payload = extract_share_link_payload(&encoded).expect("should decode base64 payload");
        assert!(payload.contains("ss://"));

        // 明文分享链接直接返回。
        let plain = "trojan://pwd@example.com:443#Plain";
        assert_eq!(extract_share_link_payload(plain).as_deref(), Some(plain));

        // 普通 YAML 不应被误判为分享链接负载。
        assert!(extract_share_link_payload("proxies: []\nmixed-port: 7890").is_none());
    }

    #[test]
    fn parses_shadowsocks_sip002() {
        // base64("aes-256-gcm:secret") = YWVzLTI1Ni1nY206c2VjcmV0
        let line = "ss://YWVzLTI1Ni1nY206c2VjcmV0@1.2.3.4:8388#Hong%20Kong";
        let node = parse_shadowsocks(line).expect("ss should parse");
        assert_eq!(node.kind, ProxyKind::Shadowsocks);
        assert_eq!(node.name, "Hong Kong");
        assert_eq!(node.server, Some(Value::from("1.2.3.4".to_string())));
        assert_eq!(node.port, Some(Value::from(8388u64)));
        assert_eq!(node.cipher, Some(Value::from("aes-256-gcm".to_string())));
        assert_eq!(node.password, Some(Value::from("secret".to_string())));
    }

    #[test]
    fn parses_vmess_base64_json() {
        let json = r#"{"v":"2","ps":"Tokyo","add":"jp.example.com","port":"443","id":"uuid-1","aid":"0","net":"ws","host":"jp.example.com","path":"/ray","tls":"tls","scy":"auto"}"#;
        let line = format!("vmess://{}", STANDARD.encode(json));
        let node = parse_vmess(&line).expect("vmess should parse");
        assert_eq!(node.kind, ProxyKind::Vmess);
        assert_eq!(node.name, "Tokyo");
        assert_eq!(node.uuid, Some(Value::from("uuid-1".to_string())));
        assert_eq!(node.network, Some(Value::from("ws".to_string())));
        assert_eq!(node.tls, Some(true));
        assert!(node.extensions.contains_key("ws-opts"));
        assert_eq!(node.extensions.get("alterId"), Some(&Value::from(0u64)));
    }

    #[test]
    fn parses_trojan_and_vless() {
        let trojan = parse_trojan("trojan://pass123@t.example.com:443?sni=t.example.com&type=ws&host=t.example.com&path=/tj#Trojan")
            .expect("trojan should parse");
        assert_eq!(trojan.kind, ProxyKind::Trojan);
        assert_eq!(trojan.password, Some(Value::from("pass123".to_string())));
        assert_eq!(trojan.tls, Some(true));
        assert_eq!(trojan.sni, Some("t.example.com".to_string()));
        assert!(trojan.extensions.contains_key("ws-opts"));

        let vless = parse_vless("vless://uuid-2@v.example.com:443?security=reality&sni=v.example.com&pbk=publickey&sid=abcd&flow=xtls-rprx-vision&type=grpc&serviceName=grpcsvc#Vless")
            .expect("vless should parse");
        assert_eq!(vless.kind, ProxyKind::Vless);
        assert_eq!(vless.uuid, Some(Value::from("uuid-2".to_string())));
        assert_eq!(vless.tls, Some(true));
        assert!(vless.extensions.contains_key("reality-opts"));
        assert!(vless.extensions.contains_key("grpc-opts"));
        assert_eq!(
            vless.extensions.get("flow"),
            Some(&Value::from("xtls-rprx-vision".to_string()))
        );
    }

    #[test]
    fn converts_multiple_links_and_dedupes_names() {
        let payload = concat!(
            "ss://YWVzLTI1Ni1nY206c2VjcmV0@1.2.3.4:8388#Node\n",
            "ss://YWVzLTI1Ni1nY206c2VjcmV0@1.2.3.5:8388#Node\n",
            "\n",
            "# a comment line\n",
            "garbage-without-scheme\n",
            "unknownproto://foo@bar:1#X\n",
        );
        let conversion = convert_share_links(payload);
        assert_eq!(conversion.proxies.len(), 2);
        assert_eq!(conversion.proxies[0].name, "Node");
        assert_eq!(conversion.proxies[1].name, "Node-01");
        assert!(conversion.unsupported_schemes.contains("unknownproto"));
    }

    #[test]
    fn parses_shadowsocksr() {
        // host:port:origin:aes-256-cfb:plain:base64("pass") + remarks
        let head = "1.2.3.4:1234:origin:aes-256-cfb:plain:cGFzcw";
        let query = "remarks=U1NS";
        let raw = format!("{head}/?{query}");
        let line = format!("ssr://{}", STANDARD_NO_PAD.encode(raw));
        let node = parse_shadowsocksr(&line).expect("ssr should parse");
        assert_eq!(node.kind, ProxyKind::ShadowsocksR);
        assert_eq!(node.password, Some(Value::from("pass".to_string())));
        assert_eq!(
            node.extensions.get("protocol"),
            Some(&Value::from("origin".to_string()))
        );
        assert_eq!(
            node.extensions.get("obfs"),
            Some(&Value::from("plain".to_string()))
        );
    }

    #[test]
    fn preserves_httpupgrade_and_hysteria_auth_yaml_keys() {
        let vless = parse_vless(
            "vless://uuid@v.example.com:443?security=tls&type=httpupgrade&host=v.example.com&path=/up&allowInsecure=1#Up",
        )
        .expect("vless httpupgrade should parse");
        let ws_opts = vless
            .extensions
            .get("ws-opts")
            .and_then(Value::as_mapping)
            .expect("httpupgrade should still use ws-opts with upgrade flag");
        assert_eq!(
            ws_opts.get(Value::from("v2ray-http-upgrade")),
            Some(&Value::from(true))
        );
        assert_eq!(vless.skip_cert_verify, Some(true));

        let hysteria = parse_hysteria("hysteria://h.example.com:443?auth=secret#Hy")
            .expect("hysteria should parse");
        assert_eq!(
            hysteria.extensions.get("auth-str"),
            Some(&Value::from("secret".to_string()))
        );
        assert!(!hysteria.extensions.contains_key("auth_str"));
    }
}
