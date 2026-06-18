# 数据流转与存储结构梳理

本文按当前源码梳理 Air 的数据来源、事件边界、持久化文件和运行态文件。重点源码位于 `crates/air-app/`、`crates/air-storage/`、`crates/air-settings/`、`crates/air-config/`、`crates/air-mihomo/` 和 `crates/air-ui/src/shell.rs`。

## 总体分层

```text
UI Shell / Page State
  -> AppCommand
  -> AppCommandRouter
  -> AppServices
  -> air-storage / air-config / air-mihomo / air-platform
  -> AppEvent / AppSnapshot
  -> UI Shell / Page State
```

边界原则：

- UI 只提交 `AppCommand`，不直接读写用户文件、不创建 HTTP client、不持有 mihomo 进程。
- app 层负责编排命令、取消、快照投影、服务调用和错误脱敏。
- config/mihomo 层负责 YAML 模型、结构化诊断、领域转换、external-controller 协议和纯 view model。
- app/mihomo 层在写入 mihomo 配置前调用 mihomo 二进制做最终校验。
- `air-storage` 层负责语义目录、原子写入、备份和安全路径。
- `air-mihomo` 层封装 external-controller HTTP/stream 协议，以及与 mihomo 相关的领域模型和生命周期服务。
- `AppSnapshot` 是 UI 顶层只读投影，唯一写入口是 `AppStateStore`。

## 平台目录

`AppPaths::resolve()` 使用：

```rust
ProjectDirs::from("org.air", "", "Air")
```

语义目录：

```text
config_dir/
  app.config.toml
  core.common.config.yaml
  core.runtime.config.yaml
  subscriptions/
    index.json
    <subscription-id>.yaml

data_dir/
  logs/
    air.log
    air-YYYY-MM-DD.log
    core.log
    core-YYYY-MM-DD.log
  backups/
    app.config.toml.bak
    core.common.config.yaml.bak
    core.runtime.config.yaml.bak
    override.js.bak
    subscriptions/
      <subscription-id>-<timestamp>.yaml.bak
  override.js

cache_dir/
  core/
    mihomo.exe              # Windows 可由构建期缓存 zip 释放
    mihomo                  # Linux/macOS 可由构建期缓存 zip 释放，或由用户手动放置候选
    geoip.dat / geosite.dat / ...
```

注意：构建期下载的 mihomo 压缩包和 geodata 会由 `crates/air-mihomo/build.rs` 缓存到仓库工作区的 `mihomo/` 目录，但该目录被 `.gitignore` 忽略，不属于用户运行态数据。订阅缓存当前放在 `config_dir/subscriptions`，不是早期任务文档中的 `cache_dir/subscriptions`；核心工作目录当前放在 `cache_dir/core`，不是 `data_dir/cores`。`air.log` 是 release 构建的软件日志，`core.log` 是 mihomo stdout/stderr 转储；二者跨日期写入时会归档为 `*-YYYY-MM-DD.log`，并只保留当天和前两天的日志文件。

## 写入策略

`FileStore` 是主要写入基础设施：

1. 路径必须在 root 内。
2. 拒绝 `..` 父目录逃逸。
3. 写入前如果目标存在，复制到 backups 目录。
4. 使用同目录临时文件。
5. 写完 flush。
6. `persist` 到目标路径，形成原子替换。

订阅缓存删除会先复制到 `data/backups/subscriptions/<id>-<timestamp>.yaml.bak`。

## 应用设置

持久化文件：

```text
config/app.config.toml
```

模型：`AppSettings`

- `theme`
- `language`
- `restore_window`
- `start_core_after_launch`
- `autostart`
- `silent_start`
- `override_script_enabled`
- `proxy_delay_test_url`
- `close_window_behavior`
- `window`

读取/写入：

- `SettingsStore::ensure_exists()` 在缺失时写入默认 TOML。
- `SettingsStore::save()` 校验后通过 `FileStore` 原子写入并备份。
- `proxy_delay_test_url` 默认值为 `http://cp.cloudflare.com/generate_204`；代理页单节点和代理组测速时由 app router 从应用设置读取，空值在使用时回退到默认地址。
- `autostart` 变更时 `Shell` 同步调用 `platform::autostart::set_enabled()`。
- `silent_start` 只控制启动后是否隐藏到托盘，不写入平台自启命令参数。

当前限制：

- `language` 仅支持 `ZhCn`。
- Windows 自启已接入当前用户 Run 注册表；macOS/Linux 开启自启返回 unsupported。
- 系统通知尚未实现。

## 核心配置

用户配置：

```text
config/core.common.config.yaml
```

运行配置：

```text
config/core.runtime.config.yaml
```

`CoreConfigStore::ensure_user_config_exists()` 会在用户配置缺失时写入内置默认配置。默认配置包含 `mixed-port: 9870`、`external-controller: 127.0.0.1:9090`、DNS、TUN、sniffer、geo 更新间隔和空代理/规则列表。

保存流程：

1. UI 配置表单生成 YAML。
2. `AppCommand::SaveConfig` 进入 router。
3. `ConfigDocument::parse()` 解析并保留未知字段。
4. 将待写入用户配置序列化到临时 YAML，并通过 `<mihomo> -d <cache_dir/core> -t -f <temp yaml>` 校验。
5. 校验通过后调用 `CoreConfigStore::save_user_config()` 原子写入 `core.common.config.yaml`。
6. `AppServices::apply_core_config_projection()` 刷新 snapshot。
7. 如果核心正在运行，重新写出 runtime 配置并调用 mihomo `PUT /configs` 重载。

格式边界：业务修改后由 `serde_yaml` 规范化输出，不保证保留注释、锚点样式和原始排版。

## 运行配置合并

运行配置在启动、重启、保存配置、选择订阅、启用覆写脚本或保存覆写脚本时写出。手动刷新订阅只更新订阅缓存和投影，不写出 `core.runtime.config.yaml`，也不在核心运行中触发 mihomo reload。

主路径：

```text
CoreConfigStore::load_user_config()
  -> ensure_runtime_dns_defaults()
  -> SubscriptionStore::load_sources() + read_cached_content()
  -> CoreConfigStore::merged_runtime_config()
  -> apply_override_script()              # 如果启用覆写脚本
  -> mihomo -d <cache_dir/core> -t -f <temp yaml>
  -> CoreConfigStore::write_runtime_document()
```

`CoreConfigStore::write_runtime_document()` 不再执行 Air 内置配置语义校验；写入前的最终阻断校验以 mihomo 二进制返回结果为准。YAML 解析、模型转换和覆写脚本执行失败仍属于应用自身输入/转换错误。

合并内容：

- 订阅 `proxies` 按名称替换或追加。
- 订阅 `proxy-groups` 按名称替换或追加。
- 订阅 `proxy-providers`、`rule-providers` 扩展到用户配置。
- 订阅 `rules` 非空时追加。
- 订阅 `sub-rules` 扩展到用户配置。

安全边界：

- 订阅中的 `external-controller` 等运行控制字段不会覆盖用户配置。
- 只有 enabled 订阅参与运行配置合并；禁用订阅的缓存可以手动更新，但不会因此被激活或合入 runtime。
- `core.runtime.config.yaml` 是 mihomo 实际启动/重载使用的文件，不是用户手写主配置。
- external-controller 返回的运行态选择、规则启停、连接状态不能直接写回 YAML。

## 覆写脚本

持久化文件：

```text
data/override.js
```

状态开关位于：

```text
config/app.config.toml -> override-script-enabled
```

执行边界：

- 使用 `rquickjs`，内存限制为 16 MiB。
- 脚本必须导出函数或声明 `function override(subscriptionName, config)`。
- 输入是订阅名和合并后的 runtime config JSON 对象。
- 不暴露本地文件、网络和 Air 内部状态。
- 调试预览只返回 YAML，不写入 `core.runtime.config.yaml`。

## 订阅数据

持久化文件：

```text
config/subscriptions/index.json
config/subscriptions/<subscription-id>.yaml
data/backups/subscriptions/<subscription-id>-<timestamp>.yaml.bak
```

`SubscriptionStore` 保存：

- 订阅源元数据：id、名称、URL、本地文件来源、更新间隔、user agent、请求头、proxy、enabled、source kind。
- 缓存元数据：正文路径、etag、last-modified、最近更新结果、成功/失败时间。

URL 导入：

1. 校验 id 和 http/https URL。
2. 使用 staging store 下载并解析。
3. 下载和解析成功后才写入真实 store。
4. 写入 YAML 缓存和 index。
5. 通过 `SubscriptionStateChanged` 回填 UI。

本地 YAML 导入：

1. 扩展名必须是 `.yaml` 或 `.yml`。
2. 读取并解析为 `ConfigDocument`。
3. 校验无 error 后写入订阅缓存。

更新与取消：

- 手动更新和定时更新都走 `SubscriptionController`；手动更新允许刷新禁用订阅缓存，定时更新只选择 enabled 远程订阅。
- 支持 ETag/Last-Modified、304 复用和失败保留旧缓存。
- 取消通过 `subscription:<id>` 取消令牌，取消结果写入订阅投影。

当前限制：

- base64 节点订阅转换仍未实现。
- `source.proxy` 会参与远程订阅下载：`DIRECT` 表示直连，`http(s)://` 与 `socks5://` / `socks5h://` 代理 URL 会作为下载出口；其他值会在下载前返回校验错误，避免出现“界面已保存但请求未生效”。

## mihomo API 数据

`MihomoClientFactory` 每次从当前用户配置读取 `external-controller` 和 `secret`，生成 HTTP / stream client。默认 controller 为 `http://127.0.0.1:9090`。

当前 app router 已接入：

- `/version`
- `GET/PATCH/PUT /configs`
- `POST /restart`
- `/group`、`/group/{name}`、`/group/{name}/delay`
- `/proxies/{group}`、`/proxies/{name}/delay`
- `/rules`、`PATCH /rules/disable`
- `/providers/rules/{name}`
- `/connections`、`DELETE /connections`、`DELETE /connections/{id}`
- `/logs`、`/traffic`、`/memory`、`/connections` stream

日志、URL query、请求体、响应体和错误信息在进入日志或 UI 前必须脱敏。

## AppSnapshot 与 AppEvent

`AppSnapshot` 字段：

- `runtime`
- `active_profile`
- `runtime_info`
- `controller_addr`
- `config_validation`
- `core_service`
- `last_error`

`AppEvent` 类别：

- 快照和运行态：`SnapshotChanged`、`RuntimeStatusChanged`
- 命令生命周期：`CommandStarted`、`CommandFinished`
- 错误和通知：`UserVisibleError`、`UserNotification`
- 流事件：`MihomoStreamEvent`
- 覆写预览：`OverridePreviewGenerated`
- 连接：`ConnectionsStateChanged`
- 规则：`RulesStateChanged`
- 代理组和测速：`ProxyGroupStateChanged`、`ProxyDelayMeasured`、`ProxyGroupDelayMeasured`
- 订阅：`SubscriptionStateChanged`、`SubscriptionYamlLoaded`、`SubscriptionUpdateCanceled`

高频日志、流量、内存、连接事件不写入 snapshot，避免 GPUI 过度刷新。

## UI 页面状态来源

- 日志页：`MihomoStreamEvent` 中由 `StartLogMonitoring` 从 `data_dir/logs/core.log` 读取的日志事件。
- 状态栏：`AppSnapshot` 和全局 `/traffic` 订阅产生的 `MihomoStreamEvent`。
- 订阅页：`SubscriptionStateProjection`、`SubscriptionStateChanged`、`SubscriptionYamlLoaded`。
- 代理组页：未运行时不展示代理组或代理节点数据；运行中 `RefreshProxies` 以 mihomo `/proxies` 返回作为唯一运行态来源，先拆分代理组和代理节点，节点名称、类型和历史延迟优先来自 `/proxies` 对应叶子节点响应；当前 enabled 订阅缓存或运行配置只提供显示顺序、成员来源和离线回退解析。`/configs` 为 `global` 模式时只显示 `GLOBAL` 组，规则模式和直连模式不显示 `GLOBAL`。
- 连接页：默认空状态；运行中由 `/connections` stream 和 `ConnectionsStateChanged` 回填。
- 规则页：默认空状态；运行中由 `/rules` 回填。
- 覆写页：`data/override.js` 和 `OverridePreviewGenerated`。
- 设置页：`AppSettings` 和当前核心配置 draft。
- 配置编辑页：优先读取 `core.common.config.yaml`；后端不可用时使用空文档状态并记录日志，测试示例配置只在 `#[cfg(test)]` helper 中可见。

## 当前风险点

- 单配置主路径已经取代历史 profile store；后续多 profile 必须重新设计。
- `config::merge` 完整流水线和当前启动合并主路径并存，切换时需要同步测试。
- base64 订阅转换缺失。
- 系统通知、导入导出、CI 打包和诊断文档缺失。
- macOS/Linux 平台能力明显弱于 Windows。
