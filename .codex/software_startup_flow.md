# 软件启动流程梳理

本文描述 Air GUI 进程从入口到首屏可交互的启动链路。主要源码位于 `crates/air-desktop/src/main.rs`、`crates/air-app/src/lib.rs`、`crates/air-ui/src/shell.rs` 和 `crates/air-app/src/services.rs`。

## 顶层入口

`crates/air-desktop/src/main.rs` 声明进程级全局分配器 `mimalloc::MiMalloc`，然后先调用 `air_app::run_service_entrypoints()` 处理服务/helper 分流，再通过 `air_app::prepare_gui_launch()` 准备单实例和启动参数，最后调用 `air_ui::launch()`。如果返回错误，打印到 stderr 并以状态码 `1` 退出。release 构建下启用 `windows_subsystem = "windows"`，Windows 桌面版不会默认弹出控制台窗口。

## 启动分流

`air_app::run_service_entrypoints()` 先初始化 tracing，然后按命令行参数分流：

1. `--air-core-service`：进入 Windows 内核服务进程，不初始化 GPUI。
2. `--air-elevated-service-helper`：进入 Windows 服务安装/卸载提权 helper，不初始化 GPUI。
3. `--air-elevated-core-helper`：进入 Windows 提权核心 helper，不初始化 GPUI。
4. 普通 GUI：先通过 `platform::single_instance` 抢占本地单实例监听；如果已有 GUI 实例在运行，当前进程只发送恢复已有窗口请求并退出，不再初始化 GPUI。
5. 首个普通 GUI 进程读取 `--air-start-core-after-elevation` 形成 `force_start_core`，由 `air-desktop` 调用 `air_ui::launch(force_start_core, single_instance_events)`。

这些 helper/service 分支必须在 GPUI 初始化前返回，避免后台进程创建桌面窗口。

release 构建下 `telemetry::init_tracing()` 会准备 `data_dir/logs/air.log`。日志写入前通过 `telemetry::log_retention` 做日期归档和清理，只保留当天和前两天的 `air*.log` 文件；debug 构建仍输出到控制台。

## GPUI 初始化

`crates/air-ui/src/shell.rs::launch()`：

1. 调用 `gpui_platform::application()`。
2. 从 `crates/air-ui/assets/` 注册图标、emoji 和品牌资源。
3. 初始化 `gpui_component::init(cx)`。
4. 调用 `components::enforce_visible_scrollbars(cx)` 固定滚动条策略。
5. 配置全局 notification placement。
6. 创建主窗口：
   - 默认大小 `1080x720`。
   - 最小大小 `860x560`。
   - app id 为 `air`。
   - 使用 gpui-component `TitleBar`。
   - client-side window decorations。
7. 创建 `Shell` entity。
8. 注册窗口关闭回调和 app quit 回调。
9. 创建 `Root::new(shell, window, cx)` 并激活应用。

## Shell 初始化

`Shell::new(window, cx, force_start_core, single_instance_events)` 是 GUI 首屏装配中心。

默认激活路由是订阅页。普通可见启动时只立即创建订阅页和全局状态栏所需的轻量状态；如果 `silent_start` 使窗口启动后立即隐藏到托盘，则跳过订阅投影和配置草稿预加载，恢复窗口时再按当前路由装载页面状态：

- 订阅页表单输入和订阅投影。
- 日志页监控状态，仅在 `Logs` 路由聚焦时订阅实时速率；其他页面和隐藏窗口不再常驻 `/traffic` 流。
- 配置编辑页的基础草稿，用于后续进入配置分支时复用当前用户配置来源。

规则、覆写、代理组、连接、日志、设置和配置编辑页的输入控件、SelectState、代码编辑器和订阅回调都在对应路由激活时按需创建，离开路由或隐藏到托盘时释放。这些状态只服务窗口渲染和局部交互，不直接持有 storage、HTTP client 或进程句柄。自定义标题栏中间直接暴露订阅、代理、连接、规则、覆写和设置六个主入口；日志页是隐藏路由，只能通过状态栏内核按钮右键菜单进入，不出现在标题栏菜单。

## AppServices 装配

`load_app_backing()` 调用 `AppServices::new()`。

成功时返回：

- `SettingsStore`
- 当前 `AppSettings`
- `AppCommandRouter`
- 初始 `AppSnapshot`
- `AppStateStore`

失败时 UI 降级启动：

- 使用默认 `AppSettings`。
- 使用空 `AppSnapshot`。
- 没有 command router，用户命令只记录日志，不能执行真实链路。

`AppServices::with_paths()` 装配顺序：

1. `AppPaths::resolve()` 获取平台目录。
2. `paths.init()` 创建 config/data/cache、订阅缓存、核心、日志和备份目录。
3. 非测试构建释放构建期缓存的核心和 geodata：
   - `crates/air-mihomo/build.rs` 首次构建时从 GitHub latest release 下载当前 target 对应的 mihomo 压缩包到 `mihomo/<arch>/<os>/mihomo.zip`。
   - `crates/air-mihomo/build.rs` 下载 meta-rules-dat 的常用 geodata 文件并打包为 `mihomo/geodata.zip`。
   - `mihomo/` 被 `.gitignore` 忽略，仓库不再提交上游二进制和 geodata 压缩包。
4. 创建 `AppRuntime` 多线程 Tokio runtime 和 `AppEvent` broadcast。
5. 创建 `SettingsStore` 并确保 `config/app.config.toml` 存在。
6. 创建 `CoreConfigStore` 并确保 `config/core.common.config.yaml` 存在。
7. 从当前配置派生 external-controller endpoint 和 secret。
8. 创建 `MihomoHttpClient`、`MihomoProcessManager`、`MihomoRuntimeDetector`、`MihomoService`。
9. 创建 `SubscriptionStore`、`OverrideScriptStore`、`MihomoClientFactory`。
10. 创建初始 `AppSnapshot`，包含 active profile 标签、controller 地址、配置校验摘要和 Windows 内核服务状态。
11. 创建 `AppStateStore`。

注意：当前主路径不使用历史 `ProfileStore`，active profile 只是 UI 来源标签 `core.common.config.yaml`。

## 页面初始数据

Shell 初始化时构造首屏和全局展示必需的轻量页面状态；静默启动隐藏到托盘时，页面级状态先保持为空以压低隐藏态内存：

- 配置编辑页：优先读取 `services.current_profile_document()`，实际来源为 `CoreConfigStore::load_user_config()`；失败时记录日志并使用空文档状态，不再把示例配置作为生产降级数据。
- 订阅页：进入页面时创建表单输入并读取 `SubscriptionController::load_projection()`；失败后为空状态。
- 代理组页：进入页面时才读取当前 enabled 订阅缓存构造页面状态；没有激活订阅缓存时为空状态，不再回退到用户配置。
- 规则页：空状态，运行中进入页面后由 `/rules` 刷新。
- 连接页：空状态，运行中进入页面后订阅 `/connections`。
- 覆写页：进入页面时读取 `data/override.js`，缺失时使用默认函数。
- 设置页：从 `AppSettings` 和当前核心配置 draft 构造。
- 监控状态：默认空状态，由 stream 事件填充。

## AppEvent 订阅

如果 router 和 snapshot store 可用，Shell 会订阅 `services.runtime.subscribe()`：

1. 后台任务等待 `AppEvent`。
2. 收到事件后在 GPUI window 上下文调用 `shell.apply_app_event(event)`。
3. 如果 broadcast lag，直接读取最新 snapshot 并发送 `SnapshotChanged`。
4. 如果事件通道关闭或 window update 失败，事件泵退出。

事件回填集中在 Shell 边界，页面 reducer 不直接访问 app service。

## 启动后的后台循环

Shell 初始化后会启动：

- 单实例恢复请求监听循环：后续 GUI 启动进程会通过本地 IPC 请求当前窗口显示；窗口隐藏到托盘或最小化时走恢复路径，窗口已可见时只前置窗口，不重建页面状态。
- 订阅到期检查循环：每 `60s` 检查一次；窗口隐藏到托盘期间跳过派发 `RefreshDueSubscriptions`。
- 托盘事件循环：Windows 支持显示/隐藏窗口、启动/停止核心和退出。
- 监控/连接/代理组/规则刷新不是固定循环，而由路由焦点和 runtime 状态通过 reconcile 方法触发。

当前没有独立的连接 HTTP 轮询循环；连接页主要由 stream 和手动刷新驱动。

## 启动后自动命令

`dispatch_startup_prepare(router, start_core)`：

```text
start_core = force_start_core || settings.start_core_after_launch

if start_core:
  dispatch(StartCore)
else:
  dispatch(PrepareCore)

dispatch(RefreshDueSubscriptions)
```

默认启动只做核心准备和订阅到期检查，不自动启动 mihomo。

## 托盘和启动隐藏

`create_tray()` 调用 `platform::tray::start_tray()`：

- Windows 支持托盘菜单。
- 不支持的平台返回 disabled handle。

启动后如果 `settings.silent_start = true` 且托盘可用，窗口初次显示后会隐藏到托盘。`silent_start` 不等同于平台自启；平台自启由 `settings.autostart` 独立控制。

## 关闭流程

窗口关闭回调按 `close_window_behavior` 分流：

- `Tray`：调用 `platform::window::hide_window(window)`，返回 `false` 阻止窗口真正关闭。
- `Exit`：调用 `Shell::stop_core_before_app_exit()`，再允许窗口关闭。

托盘 `Quit` 和 `cx.on_app_quit` 也会调用 `stop_core_before_app_exit()`。

退出收尾不派发异步 `StopCore`，而是直接调用：

```text
router.services().stop_core_before_exit()
```

该函数在 `AppRuntime` 内同步等待 `MihomoService::stop()`，并在必要时补停 Windows 内核服务。

## 当前启动链路风险点

- `AppServices::new()` 失败会让 UI 降级启动，但所有真实命令不可用。
- 配置编辑页后端不可用时只保留空状态和日志提示；示例配置夹具已限制在测试构建中。
- 多 profile 不是当前主路径。
- 非 Windows 平台的托盘、提权、TUN、自启和服务能力仍多为降级实现。

## 页面状态内存策略

当前实现只保留当前激活路由所需的页面状态和输入 runtime。Shell::navigate() 会销毁离开的页面 view model、输入控件、订阅回调、运行列表、日志、弹窗和编辑草稿，并在进入新路由时从 app service、配置仓储、订阅投影或默认脚本重新装载。窗口关闭到托盘或静默启动隐藏到托盘时，会先挂起页面事件回填并立即停止页面相关监控/定时任务，延迟约 `3s` 后再销毁全部页面状态；从托盘恢复窗口时统一销毁旧状态并仅按当前路由重新装载。
