# GUI 与 mihomo 内核交互梳理

本文基于当前源码梳理 GUI 如何接入、控制和观察 mihomo。若历史任务记录与源码存在差异，以当前源码为准。

## 总体链路

GUI 不直接持有 mihomo 进程句柄，也不直接发 external-controller HTTP/stream 请求。所有真实交互都经过 app 层：

```text
UI 页面 / Shell
  -> AppCommand
  -> AppCommandRouter::dispatch
  -> AppRuntime 后台任务
  -> AppServices
  -> mihomo / storage / platform
  -> AppEvent / AppSnapshot
  -> Shell::apply_app_event
  -> 页面状态刷新
```

关键文件：

- `crates/air-ui/src/shell.rs`：导航、命令派发、事件订阅和页面回填。
- `crates/air-app/src/command.rs`：GUI 可表达的 app 命令。
- `crates/air-app/src/router.rs` 及其 `router/` 子模块：命令到进程、API、订阅和配置仓储的路由。
- `crates/air-app/src/services.rs`：服务装配、配置读写、运行配置生成和 snapshot 投影。
- `crates/air-mihomo/src/service.rs` / `crates/air-mihomo/src/process.rs`：mihomo 生命周期和进程管理。
- `crates/air-mihomo/src/client.rs` / `crates/air-mihomo/src/streams.rs`：HTTP 与流式协议封装。

## 内核控制

### 启动前准备

启动时 `Shell` 调用 `dispatch_startup_prepare()`：

- 如果设置要求自动启动核心，派发 `StartCore`。
- 否则派发 `PrepareCore`，只做核心检测、配置校验和快照投影。
- 随后派发 `RefreshDueSubscriptions`。

### 启动内核

`StartCore` 路径：

```text
StartCore
  -> AppServices::launch_config()
  -> 写出 core.runtime.config.yaml
  -> Windows TUN 检查内核服务
  -> MihomoService::start()
  -> MihomoProcessManager::start_process()
  -> /version 健康检查
  -> RuntimeStatus::Running
```

启动使用两份配置：

- 用户配置：`config/core.common.config.yaml`。
- 运行配置：`config/core.runtime.config.yaml`。

### TUN、服务和提权

- 非 TUN：普通子进程启动。
- Windows TUN：优先要求安装内核服务，服务进程托管 mihomo。
- 提权 helper 仍保留，但普通用户路径应主要走内核服务。
- macOS/Linux 的提权和服务能力仍为 unsupported 或降级。

### 停止和重启

- `StopCore`：停止普通子进程、提权 helper 或 Windows 服务。
- `RestartCore`：运行中优先写出 runtime 配置并调用 mihomo `POST /restart`；未运行时走 stop + start。
- 应用退出时同步调用 `stop_core_before_exit()`，避免 GUI 退出后受管核心残留。

## external-controller 命令矩阵

`MihomoClientFactory` 每次从 `core.common.config.yaml` 派生 controller 地址和 secret。默认地址为 `http://127.0.0.1:9090`。

| AppCommand | API / 行为 | 回填 |
| --- | --- | --- |
| `SetRuntimeMode` | 保存本地 mode；运行中 `PATCH /configs` | 命令结果 |
| `SaveConfig` | 先用 mihomo `-d -t -f <temp yaml>` 校验待写入 YAML，再保存用户配置；运行中写 runtime 并 `PUT /configs` | 通知 + snapshot |
| `RefreshProxies` | `GET /configs` 和 `GET /proxies`；先把 `/proxies` 返回拆分为代理组和代理节点，再按运行模式过滤可见组，订阅/运行配置只提供顺序和成员来源解析 | `ProxyGroupStateChanged` |
| `SelectProxy` | `PUT /proxies/{group}` 后刷新组 | `ProxyGroupStateChanged` |
| `TestProxyDelay` | 从 `app.config.toml` 读取 `proxy_delay_test_url`，调用 `GET /proxies/{name}/delay` | `ProxyDelayMeasured` |
| `TestProxyGroupDelay` | 从 `app.config.toml` 读取 `proxy_delay_test_url`，调用 `GET /group/{name}/delay` 后刷新组 | `ProxyGroupDelayMeasured` |
| `ClearProxyGroupFixed` | `DELETE /group/{name}` 后刷新组 | `ProxyGroupStateChanged` |
| `RefreshRules` | `GET /rules` | `RulesStateChanged` |
| `DisableRule` | `PATCH /rules/disable` 后刷新规则 | `RulesStateChanged` |
| `UpdateRuleProvider` | `PUT /providers/rules/{name}` 后刷新规则 | `RulesStateChanged` |
| `RefreshConnections` | `GET /connections` | `ConnectionsStateChanged` |
| `CloseConnection` | `DELETE /connections/{id}` 后刷新 | `ConnectionsStateChanged` |
| `CloseAllConnections` | `DELETE /connections` 后刷新 | `ConnectionsStateChanged` |
| `StartLogMonitoring` | 每秒 tail `data_dir/logs/core.log` | `MihomoStreamEvent` |
| `StartTrafficMonitoring` | `/traffic`，内核运行且窗口未挂起时启动，供状态栏跨页显示网速 | `MihomoStreamEvent` |
| `StartConnectionsMonitoring` | `/connections?interval=500` stream | `MihomoStreamEvent::Connections` |
| `SetOverrideScriptEnabled` | 保存设置、写 runtime、运行中重载 | 通知 |
| `SaveOverrideScript` | 保存 `override.js`、必要时写 runtime 并重载 | 通知 |
| `DebugOverrideScript` | 预览覆写后的 YAML，不写 runtime | `OverridePreviewGenerated` |

## 页面交互

### 状态栏内核菜单与日志页

来源：

- `AppSnapshot`：运行状态、版本、controller、TUN、配置诊断和内核服务状态。
- `MihomoStreamEvent`：日志页内容和状态栏流量速率。

行为：

- 状态栏内核按钮右键菜单提供“启动内核”开关，运行中派发 `StopCore`，未运行时派发 `StartCore`。
- “启用 TUN”开关通过配置编辑路径保存 `tun.enable`，运行中保存配置仍会按常规 `SaveConfig` 流程写出 runtime 并重载。
- “查看日志”进入隐藏 `Logs` 路由；该路由不在标题栏菜单显示，聚焦时派发 `StartLogMonitoring`，离开时派发 `StopLogMonitoring`。
- “版本”只展示当前核心版本，不派发命令。

### 订阅

订阅页不直接控制 mihomo API。手动刷新只更新对应订阅缓存，禁用订阅也可以刷新；到期自动刷新只处理 enabled 远程订阅。是否合入下一次 runtime 配置仍由 enabled 状态决定：

- `ImportSubscriptionUrl`
- `ImportSubscriptionFile`
- `UpdateSubscription`
- `RefreshDueSubscriptions`
- `SelectSubscription`
- `ReorderSubscriptions`
- `DeleteSubscription`
- `SaveSubscriptionSource`
- `CancelSubscriptionUpdate`
- `LoadSubscriptionYaml`

`SelectSubscription`、删除或保存订阅源等元数据变更会影响下一次 runtime 配置；核心运行中选择订阅会重写 runtime 并重载 mihomo。`UpdateSubscription` 成功后只回填订阅投影，不直接重写运行配置或调用 mihomo reload。状态由 `SubscriptionStateChanged`、`SubscriptionYamlLoaded` 和取消事件回填。base64 节点订阅转换仍未实现。

### 代理组

运行中：

- 页面进入时派发 `RefreshProxies`。
- `/proxies` 返回同时包含代理组和代理节点；app 层先按 `all` 成员列表和组类型拆分，代理组页只消费拆分后的运行态代理组。
- 代理页节点名称、类型和历史延迟优先使用 `/proxies` 中对应叶子节点响应；当前 enabled 订阅缓存或运行配置只作为显示顺序、成员来源和离线回退锚点。
- 当 `/configs` 报告运行模式为 `global` 时，只显示 mihomo 返回的 `GLOBAL` 代理组；规则模式和直连模式不显示 `GLOBAL`。
- 选择组成员派发 `SelectProxy`。
- 单节点/组测速派发 delay 命令。
- fixed 清理派发 `ClearProxyGroupFixed`。

未运行时：

- 不展示代理组或代理节点数据，避免把订阅缓存误认为内核当前运行态。
- 内核从运行态切换到空闲、停止中或失败时，Shell 会清空代理组页运行态投影。

编辑代理组配置只修改用户配置草稿，保存走 `SaveConfig`。

### 连接

运行中：

- 页面进入派发 `StartConnectionsMonitoring`。
- 离开或核心停止派发 `StopConnectionsMonitoring`。
- 手动刷新走 `RefreshConnections`。
- 单连接关闭走 `CloseConnection`。
- 当前筛选结果批量关闭会生成多个 `CloseConnection`。

状态来源：

- `/connections` stream 的 `StreamEvent::Connections`。
- 一次性 `ConnectionsStateChanged`。
- Windows 下可根据进程路径提取并缓存图标；非 Windows 使用回退图标。

### 规则

当前标题栏菜单“规则”页是运行态规则页：

- 页面进入且内核运行时派发 `RefreshRules`。
- 列表来自 `/rules`。
- 开关调用 `DisableRule`，即 `PATCH /rules/disable`。
- 禁用状态只属于运行态，不写回用户 YAML。

旧独立代理页和旧独立规则页已删除。

### 覆写

覆写页编辑 `data/override.js`：

- `DebugOverrideScript` 生成预览，不写 runtime。
- `SaveOverrideScript` 保存脚本。
- 启用状态保存在 `app.config.toml`。
- 启用时保存会重写 runtime 配置并在运行中重载核心。

### 设置与配置

设置页承载：

- 应用偏好：主题、窗口行为、开机自启、静默启动、启动后启动核心、关闭行为。
- Windows 内核服务安装/卸载。
- mihomo 常用配置：全局、TUN、DNS、sniffer 等。

应用设置写入 `app.config.toml`；mihomo 配置写入 `core.common.config.yaml`。用户配置和运行配置写入前都会先写入临时 YAML，再交给 mihomo 二进制执行 `-d -t -f`，核心运行中保存配置会重写 runtime 并重载。

`Profiles` 路由仍保留为配置编辑分支，但不在标题栏菜单暴露；当前主路径不是多 profile。`Settings` 路由作为标题栏中间六个主入口之一暴露。

## 事件回填

`Shell::apply_app_event()` 集中消费 app 事件：

- `SnapshotChanged` / `RuntimeStatusChanged` 更新顶层快照，并重新评估监控、连接、代理组、规则刷新。
- `MihomoStreamEvent` 分发到监控页和连接页。
- `ConnectionsStateChanged` 回填连接页。
- `RulesStateChanged` 回填规则页。
- `ProxyGroupStateChanged` 和测速事件回填代理组页。
- `SubscriptionStateChanged`、`SubscriptionYamlLoaded`、取消事件回填订阅页。
- `OverridePreviewGenerated` 回填覆写页。
- 错误和通知通过统一通知入口显示。

命令生命周期由 `pending_commands` 控制 loading、重复派发抑制和弹窗状态。

## 当前缺口

- base64 节点订阅转换未实现。
- 连接页批量关闭没有使用 `CloseAllConnections` 直连按钮路径。
- 页面示例数据已收敛到测试构建专用 helper；生产路径不再调用配置编辑页或连接页 fake 夹具。
- 系统通知、导入导出、诊断导出、README 和 CI 打包发布缺失。
- 非 Windows 平台的 TUN、自启、托盘、通知和服务能力未完整设计。

## 页面状态生命周期

GUI 事件仍集中由 Shell 消费，但页面 view model 和输入 runtime 不再作为跨路由缓存使用。路由切换时离开页状态、输入控件和订阅回调会被重置，进入页按当前 snapshot、订阅投影、配置仓储或 mihomo 运行态重新拉取；托盘隐藏期间页面事件回填会立即暂停，同时停止页面相关监控和订阅定时刷新，并在约 `3s` 后销毁全部页面状态。因此页面交互不能依赖隐藏页保留筛选、弹窗、编辑器内容或临时列表。
