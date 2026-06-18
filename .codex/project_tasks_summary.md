# 项目任务汇总与当前状态

更新时间：2026-05-27

本文档汇总原 `.codex/tasks/` 下 `000` 到 `056` 的阶段任务。拆分任务文档已不再作为维护入口；后续开发以当前源码、`AGENTS.md` 和 `.codex` 下的流程文档为准。

## 总体结论

项目已经从“任务拆分推进”进入“源码维护和发布补齐”阶段。第 0 到第 6 阶段的主链路已基本落地，第 7 阶段完成了部分 Windows 平台能力，但用户文档、诊断导出、CI 打包和跨平台完善仍未收尾。

当前能力覆盖：

- Rust/GPUI 桌面应用骨架、命令路由、后台 runtime、事件总线和快照仓储。
- mihomo 检测、嵌入核心/geodata 释放、进程启动停止、健康检查、Windows UAC/helper 和 Windows 内核服务托管。
- mihomo external-controller HTTP 与 stream 客户端，覆盖配置、代理、代理组、规则、provider、日志、流量、内存和连接。
- mihomo YAML 模型、未知字段保留、结构化诊断、全局/TUN/sniffer/DNS 表单模型、订阅合并和 JS 覆写脚本。
- 订阅源、订阅缓存、URL 导入、本地 YAML 导入、手动/定时更新、取消更新、排序和选择。
- GPUI 主界面、订阅、代理组、连接、运行态规则、隐藏日志页、覆写脚本和设置/配置编辑页面。
- Windows 托盘、窗口隐藏/恢复、开机自启、核心服务安装卸载、服务启动停止和进程图标缓存。

## 阶段汇总

### 第 0 阶段：项目地基

对应任务：`000` 到 `004`。

已完成内容：

- 建立 Rust 2024 项目、模块导出、统一 `AppError` / `AppResult`、tracing 初始化和敏感信息脱敏。
- 建立 `AppPaths`、`FileStore`、原子写入、备份目录和后台 `AppRuntime`。
- 建立 `AppCommand`、`AppEvent`、命令 id、取消令牌和基础命令生命周期。

当前源码变化：

- `AppPaths::resolve()` 使用 `ProjectDirs::from("org.air", "", "Air")`。
- 核心工作目录位于 `cache_dir/core`，订阅缓存目录位于 `config_dir/subscriptions`。

### 第 1 阶段：mihomo 核心与 API

对应任务：`005` 到 `010`。

已完成内容：

- 核心候选发现、可执行校验、`mihomo -v` 版本解析和 controller 可达性检测。
- 核心发布包模型、下载/校验/解压边界、Windows/Linux/macOS 平台元数据。
- 进程命令构建、`SAFE_PATHS` 注入、stdout/stderr 脱敏写入 `core.log`、核心/软件日志三天保留、停止超时和状态机。
- HTTP API 和 stream 客户端，覆盖 `/version`、`/configs`、代理、provider、规则、连接、日志、流量、内存等接口。
- `MihomoService` 生命周期编排和启动后 `/version` 健康检查。

当前源码变化：

- 构建期由 `build.rs` 从 GitHub latest release 下载当前 target 对应的 mihomo 压缩包和 geodata 到 gitignore 忽略的 `mihomo/` 目录；运行时从这些本地压缩包释放到 `cache_dir/core`。
- `RestartCore` 在内核运行时优先写出 runtime 配置后调用 mihomo `POST /restart`，未运行时走 stop/start 服务流程。

### 第 2 阶段：配置模型与持久化

对应任务：`011` 到 `018`。

已完成内容：

- `MihomoConfigDocument` 顶层模型和 `model/` 子模块，覆盖全局、DNS、TUN、sniffer、inbound、proxy、provider、rule 等主流配置。
- `ConfigDocument` YAML 解析、保留未知字段、结构化诊断和往返保存。
- 全局、TUN、sniffer、DNS 领域设置和 UI 表单模型。

当前源码变化：

- 历史 `ConfigProfile` / `ProfileStore` 不再是当前源码主路径；当前主路径只有 `core.common.config.yaml` 和 `core.runtime.config.yaml`。
- DNS `fallback-filter` 等细分字段已进入配置编辑/设置表单，DNS 新协议仍以 warning 保留。

### 第 3 阶段：代理、代理组、规则、订阅

对应任务：`019` 到 `026`。

已完成内容：

- 代理节点模型、未知协议保留、节点仓储、排序筛选、CRUD 和引用影响分析。
- 代理组模型、成员来源解析、运行态选择映射、健康检查和 provider/filter/exclude-filter 相关字段。
- 规则模型、sub-rules、运行态禁用 patch payload、rule provider 模型和更新服务边界。
- 订阅源模型、URL/请求头脱敏、缓存元数据、更新结果、HTTP 下载、ETag/Last-Modified、304 复用和失败保留旧缓存。
- 配置合并流水线可用于预览和更完整的冲突处理。

当前源码变化：

- 启动主路径的运行配置合并由 `CoreConfigStore::merged_runtime_config()` 执行，只合入订阅的策略相关 section。
- 手动刷新订阅只更新订阅缓存，禁用订阅也可以刷新；定时刷新仍只处理 enabled 远程订阅，只有 enabled 订阅参与 runtime 合并。
- base64 节点订阅转换仍未实现；预留解析器会返回 `base64-parser-reserved` 诊断。

### 第 4 阶段：GUI 与可视化

对应任务：`027` 到 `036`。

已完成内容：

- GPUI/gpui-component 主窗口、路由、自定义标题栏菜单、状态栏、图标资源和页面容器。
- 仪表盘、监控状态模型、代理页、代理组页、规则页、配置编辑页、订阅页、设置页、连接页的第一版 UI 和 view model。

当前源码变化：

- 当前源码已删除仪表盘页面；默认首屏进入订阅页。
- 日志查看能力改为隐藏 `Logs` 路由，从状态栏内核按钮右键菜单进入，不在标题栏菜单暴露。
- 独立“代理”和旧“规则”页已删除，自定义标题栏中间保留订阅、代理、连接、规则、覆写和设置六个主入口。
- `Profiles` 路由仍在代码中，但不在 `AppRoute::ALL` 标题栏菜单列表暴露。

### 第 5 阶段：UI 优化

对应任务：`043` 到 `048`。

已完成内容：

- `src/ui/OPTIMIZATION_FOUNDATION.md` 固定 UI 优化约束。
- `src/ui/components/` 提供共享 switch、notice、滚动容器、通知入口、动画时长等基础封装。
- 仪表盘页面已删除，原日志监控能力收敛为隐藏日志页；规则/代理入口合并后再收敛为运行态规则页。
- 连接页卡片化，订阅页卡片化和导入入口优化，设置页合并应用设置和核心配置编辑。

当前源码变化：

- 全局通知统一走 gpui-component notification layer。
- 设置页应用设置实际写入 `app.config.toml`，不是早期任务文档中的 `settings.json`。

### 第 6 阶段：前后端整合

对应任务：`049` 到 `056`。

已完成内容：

- `AppServices` 装配 runtime、settings、核心配置、订阅、覆写脚本、mihomo service、client factory 和 snapshot store。
- `AppStateStore` 作为 `AppSnapshot` 唯一写入入口。
- 核心检测/准备/启动/停止/重启、配置保存、运行模式热更新、订阅导入更新、代理组刷新、测速、规则刷新、规则禁用、连接刷新/关闭、流订阅等命令接入真实 app router。
- UI 事件回填统一在 `Shell::apply_app_event()`，页面状态消费 `AppEvent` 和 `AppSnapshot`。
- 启动时自动执行 `PrepareCore` 或按设置执行 `StartCore`，并检查到期订阅。
- 旧代理/规则页面和主要 fake 默认路径已清理；代理组未运行时从当前 enabled 订阅缓存构造离线态，运行中以 mihomo `/group` 返回作为可见组来源，并用订阅或运行配置保持顺序和成员解析。

当前源码变化：

- `DisableRule` 已实现，调用 `PATCH /rules/disable` 后刷新 `/rules`；状态只属于 mihomo 运行态，不写回 YAML。
- `SaveConfig` 在运行中会重新写出 runtime 配置并调用 `PUT /configs` 重载。
- 覆写脚本保存或启用变更会在需要时重写 runtime 配置并重载核心。
- `StartTrafficMonitoring` 独立于页面焦点，运行时用于跨页面状态栏速率展示；`StartLogMonitoring` 只在隐藏日志页聚焦时 tail `core.log`。

遗留边界：

- 连接页批量关闭当前仍按筛选结果生成多个 `CloseConnection`，没有直接使用 `CloseAllConnections` 按钮路径。
- 页面 fake/sample 构造已收敛为测试构建专用 helper；生产路径不再依赖连接页或配置编辑页示例夹具。

### 第 7 阶段：平台能力与发布质量

对应任务：`037` 到 `042`。

已完成或部分完成内容：

- Windows 权限检测、参数拼接、UAC 提权 helper。
- Windows 内核服务安装/卸载/查询/启动/停止；服务携带 owner pid，GUI 异常退出时服务可自停。
- Windows TUN 场景优先要求安装内核服务，GUI 保持普通权限。
- Windows 托盘菜单、窗口显示/隐藏、关闭到托盘。
- Windows 当前用户 Run 注册表开机自启，`autostart` 和 `silent-start` 分离。
- Windows 连接页进程图标提取和缓存。

未完成内容：

- 系统通知模块尚未实现。
- macOS/Linux 自启、TUN 权限、托盘、服务化和打包发布仍为 unsupported 或降级。
- 导入导出/备份恢复没有完整用户工作流。
- `README.md`、`docs/diagnostics.md`、`docs/packaging.md` 尚不存在；GitHub Actions 已具备 Windows `fmt/check/test` 和主分支 push 后自动构建、打 tag、发布 GitHub Release 的基础链路，但跨平台打包和发布说明仍未补齐。

## 当前文档入口

- `AGENTS.md`：AI 维护规则和当前源码边界。
- `.codex/software_startup_flow.md`：GUI 进程启动到首屏流程。
- `.codex/software_and_core_lifecycle.md`：软件启停、托盘、服务、helper 和内核生命周期。
- `.codex/core_startup_flow.md`：mihomo 启动配置、运行配置写出、进程/服务启动和健康检查。
- `.codex/gui_core_interaction.md`：GUI 页面如何通过 app command router 与 mihomo 交互。
- `.codex/data_flow_and_storage.md`：数据来源、事件边界、持久化文件和运行态文件。
- `src/ui/OPTIMIZATION_FOUNDATION.md`：UI 优化基础约束。

## 后续优先级

1. 补齐用户文档：`README.md`、诊断导出说明、打包说明和常见问题。
2. 继续完善 CI 和发布：在现有 Windows 校验与主分支自动 Release 基础上，补齐跨平台打包、资源校验和发布文档。
3. 继续清理 fake/test 夹具命名，尤其配置编辑和连接页的测试样例。
4. 设计导入导出/备份恢复工作流，明确哪些文件属于用户配置、订阅缓存、运行配置和日志。
5. 规划 macOS/Linux 的 TUN 权限、自启、托盘和通知能力。
6. 如需多 profile，先重新设计单配置主路径与 profile 管理的边界，再实现 UI。
