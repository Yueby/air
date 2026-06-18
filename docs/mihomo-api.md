# mihomo REST API 文档

本文档根据 `hub/route/` 下的路由注册和 handler 梳理。路径以外部控制器根路径为基准，例如 `http://127.0.0.1:9090`。`/configs/` 与 `/configs` 在客户端可按实际 HTTP 路由兼容性处理，本文统一写作不带末尾 `/` 的形式。

## 通用约定

### 认证

当外部控制器配置了 `secret` 时，除条件挂载的 UI 静态资源外，API 请求需要：

| 位置 | 字段 | 含义 |
| --- | --- | --- |
| Header | `Authorization: Bearer <secret>` | 常规 HTTP API 认证 |
| Query | `token=<secret>` | 仅 WebSocket 请求可用，浏览器 WebSocket 不能设置自定义 Header |

Unix socket 与 Windows named pipe 监听在代码中不使用 `secret` 认证。

### 通用错误返回

多数 JSON API 失败时返回：

```json
{
  "message": "错误信息"
}
```

常见错误：

| HTTP 状态码 | message | 含义 |
| --- | --- | --- |
| `400` | `Body invalid` | JSON body 或参数格式错误 |
| `401` | `Unauthorized` | 认证失败 |
| `404` | `Resource not found` | 资源不存在 |
| `408/504` | `Timeout` 或具体错误 | 请求或测速超时 |
| `413` | `payload exceeds 1MB limit` | storage payload 超过限制 |
| `500/503` | 具体错误 | 内部错误、更新失败或服务不可用 |

### 通用返回

| 返回 | 含义 |
| --- | --- |
| `204 No Content` | 操作成功，无响应体 |
| `{"status":"ok"}` | 操作成功，通常用于重启、升级类接口 |

## 通用对象

### Proxy 对象

`/proxies`、`/group`、`/providers/proxies` 相关接口会返回 Proxy 对象。基础字段来自 `adapter.Proxy.MarshalJSON()`，不同代理类型或代理组还可能附加字段。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 代理或代理组名称 |
| `type` | string | 代理类型，如 `Direct`、`Reject`、`Selector`、`URLTest`、`Fallback`、`LoadBalance`、`Shadowsocks` 等 |
| `id` | string | 代理实例 UUID，基础出站适配器提供 |
| `udp` | bool | 是否支持 UDP |
| `uot` | bool | 是否支持 UDP over TCP |
| `xudp` | bool | 是否启用 XUDP |
| `tfo` | bool | 是否启用 TCP Fast Open |
| `mptcp` | bool | 是否启用 MPTCP |
| `smux` | bool | 是否启用 smux |
| `interface` | string | 该代理指定的出站网卡 |
| `routing-mark` | number | Linux routing mark |
| `provider-name` | string | 代理来源 provider 名称 |
| `dialer-proxy` | string | 该代理使用的前置拨号代理 |
| `alive` | bool | 最近一次健康检查或测速是否可用 |
| `history` | array | 默认测试 URL 的延迟历史 |
| `history[].time` | string | 测速记录时间，RFC3339 JSON 时间 |
| `history[].delay` | number | 延迟，单位毫秒；`0` 常表示失败 |
| `extra` | object | 按测试 URL 记录的额外测速状态 |
| `extra.<url>.alive` | bool | 指定测试 URL 的可用性 |
| `extra.<url>.history` | array | 指定测试 URL 的延迟历史 |

代理组字段：

| 字段 | 类型 | 适用类型 | 含义 |
| --- | --- | --- | --- |
| `now` | string | `Selector`、`URLTest`、`Fallback` | 当前选中或当前生效的节点 |
| `all` | string[] | 代理组 | 代理组内可选节点名称 |
| `testUrl` | string | 代理组 | 健康检查/测速 URL |
| `expectedStatus` | string | `URLTest`、`Fallback`、`LoadBalance` | 期望 HTTP 状态码范围表达式 |
| `fixed` | string | `URLTest`、`Fallback` | 手动固定的节点名称，空表示自动 |
| `hidden` | bool | 代理组 | 是否在 UI 中隐藏 |
| `icon` | string | 代理组 | UI 图标 URL 或标识 |

### Proxy Provider 对象

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | Provider 名称 |
| `type` | string | Provider 类型，代理 provider 固定为 `Proxy` |
| `vehicleType` | string | Provider 来源类型：`File`、`HTTP`、`Compatible`、`Inline` |
| `proxies` | Proxy[] | Provider 下的代理列表 |
| `testUrl` | string | 健康检查 URL |
| `expectedStatus` | string | 健康检查期望 HTTP 状态码范围 |
| `updatedAt` | string | 最近更新时间，部分 provider 可能省略 |
| `subscriptionInfo` | object | HTTP 订阅返回 `subscription-userinfo` 后解析出的流量信息，可能省略 |
| `subscriptionInfo.Upload` | number | 已上传字节数 |
| `subscriptionInfo.Download` | number | 已下载字节数 |
| `subscriptionInfo.Total` | number | 总流量字节数 |
| `subscriptionInfo.Expire` | number | 过期时间戳 |

### Rule Provider 对象

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `behavior` | string | 规则行为：`Domain`、`IPCIDR`、`Classical` |
| `format` | string | 规则格式：`YamlRule`、`TextRule`、`MrsRule`，Inline provider 可能省略 |
| `name` | string | Provider 名称 |
| `ruleCount` | number | 规则数量 |
| `type` | string | Provider 类型，规则 provider 固定为 `Rule` |
| `vehicleType` | string | Provider 来源类型 |
| `updatedAt` | string | 最近更新时间 |
| `payload` | string[] | Inline provider 的内联规则内容，可能省略 |

### Connection Snapshot 对象

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `downloadTotal` | number | 核心启动以来累计下载字节数 |
| `uploadTotal` | number | 核心启动以来累计上传字节数 |
| `memory` | number | 当前记录的 RSS 内存字节数 |
| `connections` | Connection[] | 当前连接列表 |

Connection 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | string | 连接 UUID |
| `metadata` | object | 连接元数据 |
| `upload` | number | 当前连接累计上传字节数 |
| `download` | number | 当前连接累计下载字节数 |
| `start` | string | 连接开始时间 |
| `chains` | string[] | 实际代理链 |
| `providerChains` | string[] | 对应 provider 链 |
| `rule` | string | 命中的规则类型 |
| `rulePayload` | string | 命中的规则 payload |

Metadata 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `network` | string | 网络类型：`tcp`、`udp`、`all`、`invalid` |
| `type` | string | 入站类型，如 `HTTP`、`SOCKS5`、`TUN` 等 |
| `sourceIP` / `destinationIP` | string | 源/目标 IP |
| `sourceGeoIP` / `destinationGeoIP` | string[] | 源/目标 GeoIP 查询结果 |
| `sourceIPASN` / `destinationIPASN` | string | 源/目标 ASN 信息 |
| `sourcePort` / `destinationPort` | string | 源/目标端口，JSON 中以字符串编码 |
| `inboundIP` / `inboundPort` | string | 入站监听地址与端口 |
| `inboundName` | string | 入站名称 |
| `inboundUser` | string | 入站认证用户 |
| `host` | string | 域名目标 |
| `dnsMode` | string | DNS 模式 |
| `uid` | number | 进程 UID |
| `process` | string | 进程名 |
| `processPath` | string | 进程路径 |
| `specialProxy` | string | 特殊代理标记 |
| `specialRules` | string | 特殊规则标记 |
| `remoteDestination` | string | 远端目标 |
| `dscp` | number | DSCP 值 |
| `sniffHost` | string | 嗅探得到的 host |

## 接口清单

### `GET /`

作用：健康检查/欢迎接口。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `hello` | string | 固定为 `mihomo` |

示例：

```json
{"hello":"mihomo"}
```

### `GET /version`

作用：获取核心版本信息。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `meta` | bool/string | Meta 标识，来自常量 `C.Meta` |
| `version` | string | 核心版本，来自常量 `C.Version` |

### `GET /traffic`

作用：持续输出实时流量统计。普通 HTTP 与 WebSocket 都支持。

入参：无。

出参：每秒输出一条 JSON。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `up` | number | 最近 1 秒上传字节数 |
| `down` | number | 最近 1 秒下载字节数 |
| `upTotal` | number | 累计上传字节数 |
| `downTotal` | number | 累计下载字节数 |

### `GET /memory`

作用：持续输出内存占用。普通 HTTP 与 WebSocket 都支持。

入参：无。

出参：每秒输出一条 JSON。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `inuse` | number | 当前 RSS 内存字节数；首次输出会被置为 `0` |
| `oslimit` | number | OS 内存限制，当前固定为 `0` |

### `GET /logs`

作用：订阅运行日志。普通 HTTP 与 WebSocket 都支持。

Query 入参：

| 字段 | 类型 | 必填 | 默认 | 含义 |
| --- | --- | --- | --- | --- |
| `level` | string | 否 | `info` | 最低日志级别：`debug`、`info`、`warning`、`error`、`silent` |
| `format` | string | 否 | 空 | 为 `structured` 时返回结构化日志 |

默认格式出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `type` | string | 日志级别/类型 |
| `payload` | string | 日志文本 |

结构化格式出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `time` | string | 当前时间，格式为 `HH:mm:ss` |
| `level` | string | 日志级别，`warning` 会转换为 `warn` |
| `message` | string | 日志文本 |
| `fields` | array | 结构化字段，当前实现为空数组 |

### `GET /configs`

作用：获取当前通用配置。

入参：无。

出参：`config.General` 对象。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `port` | number | HTTP 入站端口 |
| `socks-port` | number | SOCKS 入站端口 |
| `redir-port` | number | redir 入站端口 |
| `tproxy-port` | number | TProxy 入站端口 |
| `mixed-port` | number | mixed 入站端口 |
| `tun` | object | TUN 入站配置 |
| `tuic-server` | object | TUIC 服务端配置 |
| `ss-config` | string | Shadowsocks 入站配置字符串 |
| `vmess-config` | string | VMess 入站配置字符串 |
| `authentication` | string[] | HTTP/SOCKS/mixed 入站认证记录 |
| `skip-auth-prefixes` | string[] | 跳过认证的 CIDR 前缀 |
| `lan-allowed-ips` | string[] | LAN 允许访问的 IP/CIDR |
| `lan-disallowed-ips` | string[] | LAN 禁止访问的 IP/CIDR |
| `allow-lan` | bool | 是否允许 LAN 访问入站 |
| `bind-address` | string | 入站绑定地址 |
| `inbound-tfo` | bool | 入站 TFO 开关 |
| `inbound-mptcp` | bool | 入站 MPTCP 开关 |
| `mode` | string | 代理模式：`global`、`rule`、`direct` |
| `unified-delay` | bool | 是否统一延迟测试逻辑 |
| `log-level` | string | 日志级别 |
| `ipv6` | bool | 是否启用 IPv6 解析 |
| `interface-name` | string | 默认出站网卡 |
| `routing-mark` | number | 默认 routing mark |
| `geox-url` | object | Geo 数据库下载地址 |
| `geo-auto-update` | bool | 是否自动更新 Geo 数据库 |
| `geo-update-interval` | number | Geo 自动更新间隔 |
| `geodata-mode` | bool | 是否使用 geodata 模式 |
| `geodata-loader` | string | geodata loader |
| `geosite-matcher` | string | geosite 匹配器 |
| `tcp-concurrent` | bool | TCP 并发拨号开关 |
| `find-process-mode` | string | 进程查找模式：`strict`、`always`、`off` |
| `sniffing` | bool | 嗅探开关 |
| `global-client-fingerprint` | string | 全局 TLS 指纹 |
| `global-ua` | string | 全局 User-Agent |
| `etag-support` | bool | 资源请求是否支持 ETag |
| `keep-alive-idle` | number | TCP keepalive idle 秒数 |
| `keep-alive-interval` | number | TCP keepalive interval 秒数 |
| `disable-keep-alive` | bool | 是否禁用 keepalive |

`geox-url` 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `geo-ip` | string | GeoIP 数据库 URL |
| `mmdb` | string | MMDB 数据库 URL |
| `asn` | string | ASN 数据库 URL |
| `geo-site` | string | GeoSite 数据库 URL |

`tun` 常见字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `enable` | bool | 是否启用 TUN |
| `device` | string | TUN 设备名 |
| `stack` | string | TUN 栈：`gVisor`、`System`、`Mixed` |
| `dns-hijack` | string[] | DNS 劫持目标 |
| `auto-route` | bool | 是否自动配置路由 |
| `auto-detect-interface` | bool | 是否自动探测出口网卡 |
| `mtu` | number | MTU |
| `gso` | bool | Generic Segmentation Offload 开关 |
| `gso-max-size` | number | GSO 最大分段大小 |
| `inet4-address` / `inet6-address` | string[] | TUN IPv4/IPv6 地址前缀 |
| `iproute2-table-index` | number | Linux iproute2 路由表索引 |
| `iproute2-rule-index` | number | Linux iproute2 rule 索引 |
| `auto-redirect` | bool | 自动重定向开关 |
| `auto-redirect-input-mark` | number | 自动重定向入站 mark |
| `auto-redirect-output-mark` | number | 自动重定向出站 mark |
| `auto-redirect-iproute2-fallback-rule-index` | number | fallback rule 索引 |
| `loopback-address` | string[] | loopback 地址 |
| `strict-route` | bool | 严格路由 |
| `route-address` / `route-address-set` | array | 需要路由的地址或规则集 |
| `route-exclude-address` / `route-exclude-address-set` | array | 排除路由的地址或规则集 |
| `include-interface` / `exclude-interface` | string[] | 包含/排除的网卡 |
| `include-uid` / `exclude-uid` | number[] | 包含/排除的 UID |
| `include-uid-range` / `exclude-uid-range` | string[] | 包含/排除的 UID 范围 |
| `exclude-src-port` / `exclude-dst-port` | number[] | 排除的源/目标端口 |
| `exclude-src-port-range` / `exclude-dst-port-range` | string[] | 排除的源/目标端口范围 |
| `include-android-user` | number[] | Android 用户 ID 白名单 |
| `include-package` / `exclude-package` | string[] | Android 包名包含/排除列表 |
| `include-mac-address` / `exclude-mac-address` | string[] | MAC 地址包含/排除列表 |
| `endpoint-independent-nat` | bool | Endpoint Independent NAT |
| `udp-timeout` | number | UDP 超时 |
| `disable-icmp-forwarding` | bool | 是否禁用 ICMP 转发 |
| `file-descriptor` | number | Android/VPN 场景传入的 TUN fd |
| `inet4-route-address` / `inet6-route-address` | string[] | IPv4/IPv6 路由地址 |
| `inet4-route-exclude-address` / `inet6-route-exclude-address` | string[] | IPv4/IPv6 排除路由地址 |
| `recvmsgx` / `sendmsgx` | bool | Darwin 特定优化开关 |

### `PATCH /configs`

作用：热更新部分运行时配置。embed mode 下不会注册。

Body：JSON 对象，所有字段均可选。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `port` | number | 重建 HTTP 入站端口 |
| `socks-port` | number | 重建 SOCKS 入站端口 |
| `redir-port` | number | 重建 redir 入站端口 |
| `tproxy-port` | number | 重建 TProxy 入站端口 |
| `mixed-port` | number | 重建 mixed 入站端口 |
| `tun` | object | 重建 TUN 入站配置，字段见下表 |
| `tuic-server` | object | 重建 TUIC 服务端配置，字段见下表 |
| `ss-config` | string | 重建 Shadowsocks 入站配置 |
| `vmess-config` | string | 重建 VMess 入站配置 |
| `tcptun-config` | string | 可被 JSON 解码，但当前 handler 未实际应用该字段 |
| `udptun-config` | string | 可被 JSON 解码，但当前 handler 未实际应用该字段 |
| `allow-lan` | bool | 设置 LAN 访问开关 |
| `skip-auth-prefixes` | string[] | 设置跳过认证的 CIDR 前缀 |
| `lan-allowed-ips` | string[] | 设置 LAN 允许 IP/CIDR |
| `lan-disallowed-ips` | string[] | 设置 LAN 禁止 IP/CIDR |
| `bind-address` | string | 设置入站绑定地址 |
| `mode` | string | 设置代理模式：`global`、`rule`、`direct` |
| `log-level` | string | 设置日志级别：`debug`、`info`、`warning`、`error`、`silent` |
| `ipv6` | bool | 设置是否启用 IPv6 解析 |
| `sniffing` | bool | 设置嗅探开关 |
| `tcp-concurrent` | bool | 设置 TCP 并发拨号 |
| `find-process-mode` | string | 设置进程查找模式：`strict`、`always`、`off` |
| `interface-name` | string | 设置默认出站网卡 |

PATCH `tun` 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `enable` | bool | 是否启用 TUN；注意该字段不是指针，传入 `tun` 对象但省略它时会按 `false` 处理 |
| `device` | string | TUN 设备名 |
| `stack` | string | TUN 栈 |
| `dns-hijack` | string[] | DNS 劫持目标 |
| `auto-route` | bool | 自动路由 |
| `auto-detect-interface` | bool | 自动探测网卡 |
| `mtu` | number | MTU |
| `gso` | bool | GSO 开关 |
| `gso-max-size` | number | GSO 最大大小 |
| `inet6-address` | string[] | IPv6 地址前缀 |
| `iproute2-table-index` | number | iproute2 路由表索引 |
| `iproute2-rule-index` | number | iproute2 rule 索引 |
| `auto-redirect` | bool | 自动重定向 |
| `auto-redirect-input-mark` | number | 入站 mark |
| `auto-redirect-output-mark` | number | 出站 mark |
| `auto-redirect-iproute2-fallback-rule-index` | number | fallback rule 索引 |
| `loopback-address` | string[] | loopback 地址 |
| `strict-route` | bool | 严格路由 |
| `route-address` / `route-address-set` | array | 路由地址/规则集 |
| `route-exclude-address` / `route-exclude-address-set` | array | 排除路由地址/规则集 |
| `include-interface` / `exclude-interface` | string[] | 包含/排除网卡 |
| `include-uid` / `exclude-uid` | number[] | 包含/排除 UID |
| `include-uid-range` / `exclude-uid-range` | string[] | 包含/排除 UID 范围 |
| `include-android-user` | number[] | Android 用户 ID |
| `include-package` / `exclude-package` | string[] | Android 包名包含/排除 |
| `include-mac-address` / `exclude-mac-address` | string[] | MAC 地址包含/排除 |
| `endpoint-independent-nat` | bool | Endpoint Independent NAT |
| `udp-timeout` | number | UDP 超时 |
| `file-descriptor` | number | TUN fd |
| `inet4-route-address` / `inet6-route-address` | string[] | IPv4/IPv6 路由地址 |
| `inet4-route-exclude-address` / `inet6-route-exclude-address` | string[] | IPv4/IPv6 排除路由地址 |
| `recvmsgx` / `sendmsgx` | bool | Darwin 特定开关 |

PATCH `tuic-server` 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `enable` | bool | 是否启用 TUIC 服务端；注意该字段不是指针，传入对象但省略它时会按 `false` 处理 |
| `listen` | string | 监听地址 |
| `token` | string[] | token 列表 |
| `users` | object | 用户映射 |
| `certificate` | string | 证书路径 |
| `private-key` | string | 私钥路径 |
| `congestion-controller` | string | 拥塞控制算法 |
| `max-idle-time` | number | 最大空闲时间 |
| `authentication-timeout` | number | 认证超时 |
| `alpn` | string[] | ALPN 列表 |
| `max-udp-relay-packet-size` | number | UDP relay 包最大大小 |
| `cwnd` | number | 拥塞窗口 |
| `bbr-profile` | string | BBR profile |

出参：成功返回 `204 No Content`。

### `PUT /configs`

作用：重新加载整份配置。embed mode 下不会注册。

Query 入参：

| 字段 | 类型 | 必填 | 默认 | 含义 |
| --- | --- | --- | --- | --- |
| `force` | bool | 否 | `false` | 是否强制应用配置 |

Body：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `path` | string | 否 | 配置文件绝对路径；为空时使用默认配置路径 |
| `payload` | string | 否 | 配置文件内容；非空时优先解析该内容，不读取 `path` |

约束：

| 字段 | 规则 |
| --- | --- |
| `path` | 非空时必须是绝对路径，并且必须通过 `C.Path.IsSafePath` 安全检查 |
| `payload` | 按配置 YAML/JSON 内容解析 |

出参：成功返回 `204 No Content`。

### `POST /configs/geo`

作用：更新 Geo 数据库。embed mode 下不会注册。

入参：无。

出参：成功返回 `204 No Content`。

### `GET /proxies`

作用：获取所有代理。该接口会合并 `tunnel.Proxies()` 与所有 proxy provider 中的代理。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `proxies` | object | key 为代理名，value 为 Proxy 对象 |

### `GET /proxies/{name}`

作用：获取指定代理详情。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | URL path escape 后的代理名，服务端会执行 `PathUnescape` |

出参：Proxy 对象。

### `PUT /proxies/{name}`

作用：切换 Selector 类代理组的当前节点。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 代理组名称 |

Body：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `name` | string | 是 | 要切换到的子代理名称 |

约束：目标 `{name}` 必须实现 `SelectAble`，否则返回 `400`。

出参：成功返回 `204 No Content`。

### `DELETE /proxies/{name}`

作用：取消非 `Selector` 的可选代理组固定节点，让其恢复自动选择。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 代理组名称 |

出参：成功返回 `204 No Content`。如果目标不是可取消固定的代理组，返回 `400`。

### `GET /proxies/{name}/delay`

作用：对指定代理执行 URL 延迟测试。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 代理名称 |

Query 入参：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `url` | string | 是 | 测试 URL，仅支持 `http`/`https` |
| `timeout` | number | 是 | 超时时间，毫秒；解析为 16 位整数 |
| `expected` | string | 否 | 期望 HTTP 状态码范围表达式，空表示不限制 |

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `delay` | number | 延迟，单位毫秒 |

### `GET /group`

作用：获取所有代理组。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `proxies` | Proxy[] | 实现 `ProxyGroup` 的代理列表 |

### `GET /group/{name}`

作用：获取指定代理组详情。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 代理组名称 |

出参：Proxy 对象。若该代理不是代理组，返回 `404`。

### `GET /group/{name}/delay`

作用：对代理组内所有节点执行 URL 延迟测试，并返回每个节点的延迟。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 代理组名称 |

Query 入参：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `url` | string | 是 | 测试 URL |
| `timeout` | number | 是 | 超时时间，毫秒；解析为 32 位整数 |
| `expected` | string | 否 | 期望 HTTP 状态码范围表达式 |

出参：对象，key 为子代理名称，value 为延迟毫秒数。

```json
{
  "proxy-a": 120,
  "proxy-b": 240
}
```

### `GET /rules`

作用：获取当前规则列表。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `rules` | Rule[] | 规则列表 |

Rule 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `index` | number | 规则索引 |
| `type` | string | 规则类型 |
| `payload` | string | 规则内容 |
| `proxy` | string | 命中的代理/策略名 |
| `size` | number | GEOIP/GEOSITE 规则集记录数；其它规则为 `-1` |
| `extra` | object | RuleWrapper 额外状态，可能省略 |
| `extra.disabled` | bool | 是否禁用 |
| `extra.hitCount` | number | 命中次数 |
| `extra.hitAt` | string | 最近命中时间 |
| `extra.missCount` | number | 未命中次数 |
| `extra.missAt` | string | 最近未命中时间 |

### `PATCH /rules/disable`

作用：按规则索引启用或禁用规则。embed mode 下不会注册。

Body：对象，key 为规则索引，value 为是否禁用。

```json
{
  "0": true,
  "3": false
}
```

字段含义：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `<index>` | bool | `true` 表示禁用该规则，`false` 表示启用 |

说明：越界索引会被忽略；只有实现 `RuleWrapper` 的规则会被修改。

出参：成功返回 `204 No Content`。

### `GET /connections`

作用：获取连接快照。普通 HTTP 返回一次快照；WebSocket 会周期推送快照。

Query 入参：

| 字段 | 类型 | 必填 | 默认 | 含义 |
| --- | --- | --- | --- | --- |
| `interval` | number | 否 | `1000` | 仅 WebSocket 有效，推送间隔毫秒 |

出参：Connection Snapshot 对象。

### `DELETE /connections`

作用：关闭所有当前连接。

入参：无。

出参：成功返回 `204 No Content`。

### `DELETE /connections/{id}`

作用：关闭指定连接。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | string | 连接 UUID |

说明：连接不存在也返回成功。

出参：成功返回 `204 No Content`。

### `GET /providers/proxies`

作用：获取所有代理 provider。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providers` | object | key 为 provider 名称，value 为 Proxy Provider 对象 |

### `GET /providers/proxies/{providerName}`

作用：获取指定代理 provider。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providerName` | string | Provider 名称 |

出参：Proxy Provider 对象。

### `PUT /providers/proxies/{providerName}`

作用：触发指定代理 provider 更新。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providerName` | string | Provider 名称 |

出参：成功返回 `204 No Content`。

### `GET /providers/proxies/{providerName}/healthcheck`

作用：触发指定代理 provider 健康检查。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providerName` | string | Provider 名称 |

出参：成功返回 `204 No Content`。

### `GET /providers/proxies/{providerName}/{name}`

作用：获取指定代理 provider 下的某个代理详情。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providerName` | string | Provider 名称 |
| `name` | string | 代理名称 |

出参：Proxy 对象。

### `GET /providers/proxies/{providerName}/{name}/healthcheck`

作用：对指定 provider 下的某个代理执行 URL 延迟测试。实现复用 `/proxies/{name}/delay`。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providerName` | string | Provider 名称 |
| `name` | string | 代理名称 |

Query 入参：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `url` | string | 是 | 测试 URL |
| `timeout` | number | 是 | 超时时间，毫秒；解析为 16 位整数 |
| `expected` | string | 否 | 期望 HTTP 状态码范围表达式 |

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `delay` | number | 延迟，单位毫秒 |

### `GET /providers/rules`

作用：获取所有规则 provider。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `providers` | object | key 为 provider 名称，value 为 Rule Provider 对象 |

### `PUT /providers/rules/{name}`

作用：触发指定规则 provider 更新。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 规则 provider 名称 |

出参：成功返回 `204 No Content`。

### `POST /cache/fakeip/flush`

作用：清空 Fake-IP 池缓存。

入参：无。

出参：成功返回 `204 No Content`。

### `POST /cache/dns/flush`

作用：清空 DNS 缓存。

入参：无。

出参：成功返回 `204 No Content`。

### `GET /dns/query`

作用：通过当前 DNS resolver 查询域名。

Query 入参：

| 字段 | 类型 | 必填 | 默认 | 含义 |
| --- | --- | --- | --- | --- |
| `name` | string | 是 | 无 | 查询域名 |
| `type` | string | 否 | `A` | DNS 查询类型，如 `A`、`AAAA`、`CNAME`、`TXT` |

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `Status` | number | DNS Rcode |
| `Question` | array | DNS Question 原始结构 |
| `TC` | bool | Truncated |
| `RD` | bool | Recursion Desired |
| `RA` | bool | Recursion Available |
| `AD` | bool | Authenticated Data |
| `CD` | bool | Checking Disabled |
| `Answer` | RR[] | Answer 记录，可能省略 |
| `Authority` | RR[] | Authority 记录，可能省略 |
| `Additional` | RR[] | Additional 记录，可能省略 |

RR 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | string | 记录名称 |
| `type` | number | DNS RR type 数值 |
| `TTL` | number | TTL 秒数 |
| `data` | string | 记录数据文本 |

### `GET /storage/{key}`

作用：读取持久化 JSON 存储。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `key` | string | 存储键，服务端会执行 `PathUnescape` |

出参：存入的原始 JSON。如果不存在，返回 JSON `null`。

### `PUT /storage/{key}`

作用：写入持久化 JSON 存储。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `key` | string | 存储键，最大 64 字节；超过后底层会跳过保存但接口仍可能返回成功 |

Body：任意合法 JSON，最大 1 MiB。

约束：

| 规则 | 含义 |
| --- | --- |
| body 必须是合法 JSON | `json.Valid(data)` 校验 |
| body 最大 1 MiB | 超过返回 `413` |
| key 最大 64 字节 | 超过时底层缓存跳过写入 |

出参：成功返回 `204 No Content`。

### `DELETE /storage/{key}`

作用：删除持久化 JSON 存储。

Path 入参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `key` | string | 存储键 |

出参：成功返回 `204 No Content`。

### `POST /restart`

作用：重启当前 mihomo 进程。embed mode 下不会注册。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `status` | string | 固定为 `ok`，随后后台执行重启 |

### `POST /upgrade`

作用：更新核心二进制，成功后重启。embed mode 下不会注册。

Query 入参：

| 字段 | 类型 | 必填 | 默认 | 含义 |
| --- | --- | --- | --- | --- |
| `channel` | string | 否 | 空 | 更新通道，透传给 core updater |
| `force` | bool | 否 | `false` | 是否强制更新 |

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `status` | string | 固定为 `ok` |

### `POST /upgrade/ui`

作用：下载/更新外部 UI。

入参：无。

出参：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `status` | string | 固定为 `ok` |

### `POST /upgrade/geo`

作用：更新 Geo 数据库。embed mode 下不会注册。实现与 `POST /configs/geo` 相同。

入参：无。

出参：成功返回 `204 No Content`。

## 条件接口

### DoH 接口：`GET <dohServer>` / `POST <dohServer>`

挂载条件：配置的 `dohServer` 非空且以 `/` 开头。

作用：DNS over HTTPS relay。

GET Query 入参：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `dns` | string | 是 | base64 raw URL encoding 的 DNS message |

POST Header/Body：

| 位置 | 字段 | 必填 | 含义 |
| --- | --- | --- | --- |
| Header | `Content-Type: application/dns-message` | 是 | DoH DNS message 类型 |
| Body | binary | 是 | DNS message，最多读取 65535 字节 |

出参：

| Content-Type | Body |
| --- | --- |
| `application/dns-message` | DNS wire-format message |

错误：该接口错误返回纯文本，不是通用 JSON 错误格式。

### UI 静态资源：`GET /ui`、`GET /ui/*`

挂载条件：设置了 `uiPath`。

作用：提供外部 UI 静态文件。

| 路径 | 行为 |
| --- | --- |
| `/ui` | 307 临时重定向到 `/ui/` |
| `/ui/*` | 从 `uiPath` 下读取静态文件 |

### Debug 接口：`/debug/*`

挂载条件：`isDebug == true`。

| 方法 | 路径 | 作用 | 出参 |
| --- | --- | --- | --- |
| `PUT` | `/debug/gc` | 触发 `debug.FreeOSMemory()` | 无显式响应体 |
| 多种 | `/debug/*` | Go profiler handler | 由 profiler handler 决定 |

### 外部扩展路由

`hub/route/external.go` 暴露 `Register(route ...externalRouter)`，允许其它包向主路由追加接口。此类接口不在 `hub/route/` 中静态定义，需结合调用 `route.Register(...)` 的外部代码另行梳理。
