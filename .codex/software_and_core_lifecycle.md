# 软件启停与内核启停流程

本文梳理 Air GUI 进程、Windows helper/service 和 mihomo 核心的完整生命周期。启动细节见 `.codex/software_startup_flow.md`，核心启动配置细节见 `.codex/core_startup_flow.md`。

## 进程形态

同一个 Air 可执行文件有四种运行形态：

| 形态 | 触发参数 | 作用 | GUI |
| --- | --- | --- | --- |
| 普通 GUI 主进程 | 无特殊参数 | 创建窗口、装配 app services、响应用户操作 | 是 |
| Windows 内核服务进程 | `--air-core-service` | 作为 Windows Service 启动并托管 mihomo | 否 |
| Windows 服务 helper | `--air-elevated-service-helper` | UAC 后安装或卸载 Windows 服务 | 否 |
| Windows 提权核心 helper | `--air-elevated-core-helper` | UAC 后启动 mihomo 并转储 stdout/stderr | 否 |

helper/service 分支在 GPUI 初始化前返回，避免后台进程创建窗口。

## 普通 GUI 生命周期

启动：

```text
main()
  -> app::run()
  -> telemetry::init_tracing()
  -> 参数分流
  -> 单实例检查；已有 GUI 实例时请求恢复已有窗口并退出
  -> ui::launch(force_start_core, single_instance_events)
  -> Shell::new()
  -> AppServices::new()
  -> dispatch_startup_prepare()
```

运行期长期活动：

- AppEvent 事件泵。
- 单实例恢复请求监听循环；后续 GUI 启动进程只唤醒已有窗口并退出。
- 订阅到期检查循环。
- 托盘事件循环。
- Logs/Connections/ProxyGroups/Rules 根据路由和 runtime 状态按需启动或停止后台任务；`/traffic` 监控在内核运行且窗口未挂起时维护，用于状态栏跨页网速。

关闭：

- 关闭行为为 `Tray` 时隐藏窗口，不退出应用，不停止核心。
- 关闭行为为 `Exit` 或托盘退出时，同步停止核心后退出。
- app quit 回调也会兜底停止核心。

## AppCommand 生命周期

所有 UI 操作通过 `Shell::dispatch_command()` 进入 `AppCommandRouter`：

1. 分配 `CommandId`。
2. 为长任务登记取消令牌。
3. 在 `AppRuntime` 上 spawn 后台 future。
4. 发出 `CommandStarted`。
5. 执行业务逻辑。
6. 成功时清空 snapshot last_error。
7. 失败时写入脱敏 last_error，并发出 `UserVisibleError`。
8. 发出 `CommandFinished`。

长任务取消 key：

| 命令 | key |
| --- | --- |
| `StartCore` / `StopCore` / `RestartCore` | `core` |
| `InstallCoreService` / `UninstallCoreService` | `core-service` |
| `StartLogMonitoring` / `StopLogMonitoring` | `log-monitoring` |
| `StartTrafficMonitoring` / `StopTrafficMonitoring` | `traffic-monitoring` |
| `StartConnectionsMonitoring` / `StopConnectionsMonitoring` | `connections-monitoring` |
| `UpdateSubscription { id }` | `subscription:{id}` |
| `RefreshDueSubscriptions` | `subscriptions-scheduler` |

同 key 新任务会取消旧任务。

## 内核启动

`StartCore`：

```text
StartCore
  -> launch_config()
  -> 如果 Windows TUN 且服务未安装，提示并失败
  -> RuntimeStatus::Starting
  -> MihomoService::start(config)
  -> MihomoProcessManager::start_process(config)
  -> /version 健康检查
  -> RuntimeStatus::Running
```

`launch_config()` 会重新读取用户配置、订阅缓存和覆写脚本，写出 `core.runtime.config.yaml`。

## Windows TUN 和内核服务

Windows 上 `core_service_required_for_admin_launch()` 返回 true。当前策略是：

- 如果配置启用 TUN，启动前要求已安装内核服务。
- 未安装服务时返回 `UnsupportedCommand`，并通过通知提示用户在设置页安装内核服务。
- 已安装服务时，进程管理器通过 `platform::core_service::start_core_service()` 启动服务。

服务进程：

1. Air 以 `--air-core-service` 被 SCM 启动。
2. 服务读取固定传入的 config/data/cache 目录。
3. 启动 `cache_dir/core/mihomo.exe -d cache_dir/core -f config_dir/core.runtime.config.yaml`。
4. 写入 `SAFE_PATHS`。
5. stdout/stderr 脱敏写入 `data_dir/logs/core.log`，写入前按日期归档并清理，只保留最近三天的 `core*.log` 文件。
6. 服务携带 GUI owner pid；owner 消失时服务请求自停并 kill mihomo。
7. 服务托管的 mihomo 子进程放入 kill-on-close JobObject；服务进程被强制终止时，JobObject 句柄关闭会连带结束 mihomo。

服务安装/卸载：

- `InstallCoreService` / `UninstallCoreService` 进入 app router。
- 普通权限下通过 `--air-elevated-service-helper` 触发 UAC。
- 服务 DACL 只授予交互用户/Users 查询、启动、停止能力，不授予修改配置或删除服务；`SYSTEM` 和 `Administrators` 保留完整服务维护权限，包括 `DELETE`、`WRITE_DAC` 和 `WRITE_OWNER`。
- 卸载旧版本安装且 ACL 已锁死的服务时，提权 helper 会先尝试修复服务 DACL；若仍无法写入 DACL，会短暂启用 `SeTakeOwnershipPrivilege` 将服务所有者恢复为 `Administrators`，再写回标准 DACL 并调用 `DeleteService`。

## 提权核心 helper

提权 helper 仍保留用于需要单独 UAC 启动核心的路径：

- GUI 使用 `ShellExecuteExW` + `runas` 启动当前 Air 可执行文件。
- 参数带 `--air-elevated-core-helper`、程序路径、工作目录、日志路径、环境变量和 mihomo 参数。
- helper 只启动 mihomo、转储 stdout/stderr、等待退出并返回退出码；转储日志同样走三天保留策略。
- Windows 下 helper 尝试把 mihomo 加入 kill-on-close Job Object。

普通 Windows TUN 用户路径应优先走内核服务。

## 内核停止

`StopCore`：

```text
StopCore
  -> RuntimeStatus::Stopping
  -> MihomoService::stop()
  -> MihomoProcessManager::stop_process()
  -> RuntimeStatus::Idle
```

停止策略：

- 普通 Tokio child：等待短超时，超时 kill。
- 提权 helper child：等待短超时，超时 kill helper。
- Windows service child：调用 `stop_core_service()`；如果服务处于 `START_PENDING` / `STOP_PENDING` 或返回 1061 不能接受控制，会先等待状态收敛，超时后按 SCM 暴露的服务 PID 终止服务进程。
- 没有 child 时停止命令视为成功。

退出应用时走 `stop_core_before_exit()`，同步等待停止完成，并按服务状态补停 Windows 服务。退出兜底会尽量执行完整链路：即使托管 `MihomoService::stop()` 报错，也会继续查询并停止 Windows 服务；只有全部兜底执行后仍失败才记录错误。普通 Windows 子进程和服务托管 mihomo 都使用 kill-on-close JobObject 覆盖 GUI 或服务进程被任务管理器强杀的场景。

## 内核重启

运行中：

1. `services.write_runtime_config_validated()` 生成 runtime YAML，并用 mihomo `-d -t -f <temp yaml>` 校验后重写运行配置。
2. 调用 mihomo `POST /restart`。
3. mihomo Windows 实现会先 `executor.Shutdown()`，再用当前可执行文件和原参数 `exec.Command(...).Start()` 拉起新进程，最后旧进程 `os.Exit(0)`；非 Windows 走 `syscall.Exec` 原地替换。
4. Windows 普通子进程和服务托管路径都不能在旧子进程退出时释放 kill-on-close JobObject；否则 mihomo 自行拉起的新进程会被 JobObject 关闭连带杀死。
5. Windows 服务托管时，服务 worker 不能把旧子进程退出视为服务结束；服务只在收到 SCM stop/shutdown 或 owner GUI 消失时收尾。

未运行：

1. 重新生成启动配置。
2. 调用 `MihomoService::restart(config)`，即 stop + start。

## 运行期监控启停

- Logs 隐藏路由聚焦：`StartLogMonitoring`，每秒从 `data_dir/logs/core.log` 读取新增日志并回填日志页。
- Logs 离开、窗口隐藏到托盘或页面状态释放：`StopLogMonitoring`。
- 核心运行且窗口未挂起：`StartTrafficMonitoring`，为状态栏提供跨页实时流量。
- 窗口隐藏到托盘、页面状态释放或核心停止：`StopTrafficMonitoring`。
- Connections 聚焦且内核运行：`StartConnectionsMonitoring`。
- Connections 离开或核心停止：`StopConnectionsMonitoring`。
- ProxyGroups 聚焦且内核运行：`RefreshProxies`。
- Rules 聚焦且内核运行：`RefreshRules`。

## 状态投影

`MihomoServiceStatus` 投影到 `RuntimeStatus`：

| MihomoServicePhase | RuntimeStatus |
| --- | --- |
| `Idle` / `Ready` / `Preparing` | `Idle` |
| `Starting` | `Starting` |
| `Running` | `Running` |
| `Stopping` | `Stopping` |
| `Failed` | `Failed { message }` |

`AppStateStore` 在状态变化时发出 `RuntimeStatusChanged` 和 `SnapshotChanged`。

## 平台能力状态

已具备：

- Windows 托盘菜单。
- Windows 窗口隐藏/恢复。
- Windows 当前用户 Run 注册表自启。
- Windows UAC 提权 helper。
- Windows 内核服务。
- Windows 连接页进程图标缓存。

未完成：

- 系统通知。
- macOS/Linux 自启、托盘、TUN 权限和服务化。
- 用户诊断导出。

已具备的 CI 发布链路：

- GitHub Actions 在 PR 和 `main`/`master` push 上执行 Windows `cargo fmt --check`、`cargo check`、`cargo test`。
- 当 `main` 收到新的 push（包括 PR 合并后产生的提交）时，会额外构建 `air.exe` 的 release 版本，打包 zip，自动创建唯一 tag，并发布 GitHub Release。

## 当前限制

- 关闭到托盘不会停止核心；退出应用才停止受管核心。
- Windows TUN 未安装服务时不能启动核心。
- 保存配置运行中会重载，但仍不是所有 mihomo 配置项都保证热生效；必要时用户仍应重启核心。
- helper/service 共享同一个二进制，排查时必须先看命令行参数确认进程形态。

## 托盘隐藏时的页面释放

关闭行为为 Tray 或静默启动隐藏到托盘时，GUI 进程继续运行且不会停止 mihomo 核心，但 Shell 会先挂起页面事件回填，立即停止 Logs、Connections、`/traffic` 和订阅定时刷新等页面相关后台任务，并立刻销毁页面状态、输入 runtime、订阅回调和编辑草稿。页面释放后会触发 mimalloc 强制 collect；Windows 下还会请求清空当前进程工作集，让隐藏态尽量接近极低常驻内存。约 `3s` 后会再执行一次页面释放和内存收缩，用于覆盖后台流任务取消后的滞后释放。隐藏期间 app event 只更新全局 snapshot 和用户可见通知；托盘恢复窗口时统一销毁旧页面状态并仅重新装载当前路由页面和输入 runtime，后台 app runtime、snapshot store 和命令路由保持存活。
