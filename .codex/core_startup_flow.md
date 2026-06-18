# mihomo 内核启动流程梳理

本文描述当前源码中 mihomo 从“用户请求启动”到 external-controller 可用的链路。主要源码位于 `crates/air-app/src/router.rs` 及其 `router/` 子模块、`crates/air-app/src/services.rs`、`crates/air-mihomo/src/service.rs`、`crates/air-mihomo/src/process.rs`、`crates/air-mihomo/src/detect.rs`、`crates/air-platform/src/core_service.rs` 和 `crates/air-platform/src/elevated_process.rs`。

## 启动入口

会触发 `AppCommand::StartCore` 的入口：

- 应用启动后设置 `start_core_after_launch = true`。
- 进程带 `--air-start-core-after-elevation`。
- 状态栏内核按钮右键菜单中的“启动内核”开关。
- 托盘菜单“启动内核”。
- 其他 UI 代码调用 `Shell::dispatch_command(AppCommand::StartCore)`。

停止入口包括托盘菜单、退出收尾、状态栏内核菜单和 UI 命令。重启入口主要来自其他 UI 命令或配置保存后的运行态重载流程。

## 命令路由

`Shell::dispatch_command()` 只把命令交给 `AppCommandRouter`。router 会：

1. 分配 `CommandId`。
2. 为长任务注册 `CancellationToken`。
3. 通过 `AppRuntime::spawn_command_with_token()` 放入后台 Tokio runtime。
4. 发送 `CommandStarted` / `CommandFinished`。
5. 失败时写入脱敏后的 `last_error` 并发送 `UserVisibleError`。

核心启动、停止、重启共享 `core` 取消 key，新核心任务会取消旧任务。

## StartCore 流程

`execute_command()` 收到 `StartCore` 后：

1. 检查取消。
2. 调用 `services.launch_config().await`。
3. 如果 `requires_admin = true`，Windows 主路径要求内核服务已安装；未安装时通知用户并返回 unsupported。
4. 设置 snapshot runtime 为 `Starting`。
5. 调用 `services.mihomo.start(config).await`。
6. 成功后 `apply_mihomo_status()` 投影到 `AppSnapshot`。
7. 失败时尝试同步当前 `MihomoService` 状态并返回错误。

## launch_config

`AppServices::launch_config()` 做启动前准备：

1. 读取 `config/core.common.config.yaml`。
2. 根据 `tun.enable` 判断是否需要管理员权限。
3. 调用 `MihomoService::prepare()` 检测可用 runtime。
4. 生成待写入的 `config/core.runtime.config.yaml` 内容，写入同工作目录临时 YAML，并用 mihomo 二进制执行 `-d <cache_dir/core> -t -f <temp yaml>` 校验。
5. 校验通过后写出 `config/core.runtime.config.yaml`。
6. 返回 `ProcessLaunchConfig`。

`ProcessLaunchConfig`：

- `binary_path`：检测到的 mihomo 可执行文件。
- `config_path`：`config/core.runtime.config.yaml`。
- `working_dir`：`cache_dir/core`。
- `console_log_path`：`data_dir/logs/core.log`。
- `safe_paths`：`config_dir`、`data_dir`、`cache_dir`。
- `requires_admin`：由 TUN 开关决定。

普通启动命令：

```text
<mihomo> -d <cache_dir/core> -f <config_dir/core.runtime.config.yaml>
```

写入前校验命令：

```text
<mihomo> -d <cache_dir/core> -t -f <temporary runtime yaml>
```

如果 `safe_paths` 非空，会注入 `SAFE_PATHS`。Windows 用 `;` 分隔，非 Windows 用 `:` 分隔。

## 运行配置写出

`write_runtime_config_validated()` 当前主路径：

1. 读取用户配置。
2. 为运行配置补 DNS 最小默认值，不回写用户配置。
3. 读取 enabled 订阅缓存。
4. 解析每个缓存为 `ConfigDocument`。
5. 调用 `CoreConfigStore::merged_runtime_config()` 合并策略相关 section。
6. 如果 `override_script_enabled = true`，读取 `data/override.js` 并执行 QuickJS 覆写。
7. 序列化待写入 YAML 到临时文件，并通过 mihomo `-d -t -f` 做最终校验。
8. 校验通过后由 `CoreConfigStore::write_runtime_document()` 原子写入最终 runtime 配置。

边界：

- `core.common.config.yaml` 是用户配置。
- `core.runtime.config.yaml` 是 mihomo 实际启动和重载使用的配置。
- 订阅或运行态 API 返回的临时状态不能写回用户配置。

## runtime 检测

`MihomoRuntimeDetector::detect_runtime()` 候选顺序：

1. 用户配置目录。
2. 托管核心目录 `cache_dir/core`。
3. 系统 `PATH`。

候选文件名：

- Windows：`mihomo.exe` 和 `mihomo`。
- 非 Windows：`mihomo`。

检测内容：

- 候选文件存在性和可执行性。
- 运行 `mihomo -v`，默认超时 3 秒。
- 从 stdout/stderr 解析 semver 版本。
- 只有当前 snapshot 已是 `Running` 时才额外检查 controller 可达性，避免启动前误报。

## 构建期资源缓存与运行期释放

`crates/air-mihomo/build.rs` 在首次构建时准备本地压缩包：

- 从 GitHub `MetaCubeX/mihomo` latest release 下载当前 target 对应的 mihomo 压缩包，缓存到 `mihomo/<arch>/<os>/mihomo.zip`。
- 从 GitHub `MetaCubeX/meta-rules-dat` latest release 下载 `geoip.dat`、`geosite.dat`、`country.mmdb`、`GeoLite2-ASN.mmdb`，重新打包为 `mihomo/geodata.zip`。
- `mihomo/` 被 `.gitignore` 忽略，源码仓库不再提交上游二进制和 geodata。

应用服务初始化会释放这些构建期缓存资源：

- 当前 target 的 `mihomo/<arch>/<os>/mihomo.zip` 中的可执行文件解压到 `cache_dir/core/mihomo.exe` 或 `cache_dir/core/mihomo`。
- `mihomo/geodata.zip` 中的 geodata 解压到 `cache_dir/core`。
- 已存在的文件不会覆盖。
- zip 解压使用 `enclosed_name()` 和目标文件名检查，避免路径穿越。

默认已有本地压缩包时不会重新联网下载；需要强制更新时可删除 `mihomo/` 或设置 `AIR_FORCE_MIHOMO_DOWNLOAD=1` 后重新构建。

## MihomoService 状态机

`MihomoService::start(config)`：

1. 校验 `binary_path` 和 `config_path`。
2. 设置 phase 为 `Starting`。
3. 调用 `MihomoProcessManager::start_process(config)`。
4. 启动后等待 `/version` 健康检查。
5. 健康检查成功：phase 设为 `Running`，清空错误，刷新 runtime 可达状态。
6. 健康检查失败：尝试停止刚启动进程，phase 设为 `Failed`，记录诊断。
7. 进程创建失败：phase 设为 `Failed` 并返回错误。

健康检查：

- 启动后等待约 `350ms`。
- 总超时约 `8s`。
- 每 `200ms` 调用一次 `/version`。
- 如果进程在 controller 可用前退出，提前判定启动失败。

## 进程启动方式

### 普通子进程

`MihomoProcessManager` 直接用 Tokio child 启动 mihomo，stdout/stderr 脱敏后写入 `core.log`。写入行包含本地 RFC3339 时间戳和 stdout/stderr 前缀；日志页读取时会忽略时间戳前缀，仍按流向映射 info/error。
Windows 下普通子进程会加入 GUI 持有的 kill-on-close JobObject；如果 GUI 被强杀，系统关闭 JobObject 句柄时会连带结束 mihomo。

### Windows 内核服务

Windows TUN 主路径要求安装 `AirMihomoCore` 服务。已安装时：

1. GUI 通过 `platform::core_service::start_core_service()` 启动服务。
2. 服务进程是同一个 Air 可执行文件，参数为 `--air-core-service`。
3. 服务读取传入的 config/data/cache 目录，启动 `cache_dir/core/mihomo.exe`。
4. 服务注入 `SAFE_PATHS`，写 `core.log`。
5. 服务携带 GUI owner pid；owner 消失时请求自身停止并终止 mihomo。
6. 服务托管的 mihomo 子进程加入服务持有的 kill-on-close JobObject；如果服务进程被强制终止，mihomo 会随 JobObject 关闭而结束。

服务安装时会写入固定 DACL：普通交互用户/Users 仅可查询、启动、停止服务，`SYSTEM` 和 `Administrators` 保留删除、改 DACL 和接管等维护权限。卸载路径会在旧服务 ACL 锁死导致 `OpenService`/`DeleteService` 拒绝访问时，先通过提权 helper 修复 ACL，必要时接管服务对象后再删除。

普通子进程、提权 helper 和 Windows 内核服务共享 `telemetry::log_retention` 保留策略：当前日志文件为 `core.log`，跨日期写入时归档为 `core-YYYY-MM-DD.log`，只保留当天和前两天的核心输出日志。

### Windows 提权 helper

如果走提权 helper 路径，Air 使用 `ShellExecuteExW` 的 `runas` 启动当前可执行文件，参数包含：

- `--air-elevated-core-helper`
- `--program <mihomo path>`
- `--cwd <cache_dir/core>`
- `--log <data_dir/logs/core.log>`
- 多个 `--env KEY=VALUE`
- 多个 `--arg ...`

helper 不创建 GUI，只启动 mihomo、转储日志、等待退出并返回退出码。

## StopCore

`StopCore`：

1. 检查取消。
2. 设置 snapshot runtime 为 `Stopping`。
3. 调用 `MihomoService::stop()`。
4. `MihomoProcessManager::stop_process()`：
   - 普通 child：等待短超时，未退出则 kill。
   - 提权 helper：等待短超时，未退出则 kill helper。
   - Windows service：调用 `stop_core_service()`，并处理 `START_PENDING` / `STOP_PENDING` / 1061 不能接受控制等过渡态；等待超时后按服务 PID 终止服务进程。
5. 成功后服务 phase 变为 `Ready`，UI runtime 投影为 `Idle`。

应用退出不会等待异步 command result，而是同步调用 `AppServices::stop_core_before_exit()`，并在必要时补停 Windows 内核服务。退出兜底不因第一段托管 stop 失败而提前返回，会继续执行服务状态查询和停止；失败后允许后续 quit 回调再次尝试。

## RestartCore

运行中重启：

1. 重新写出 runtime 配置。
2. 调用 mihomo `POST /restart`。
3. mihomo Windows 下会启动一个新的同路径同参数进程后让旧进程退出；非 Windows 走 `syscall.Exec` 原地替换。
4. Windows 普通子进程和服务托管路径都会保留 kill-on-close JobObject，旧子进程退出时不释放 JobObject，避免把 mihomo 自行拉起的新进程误杀。
5. Windows 服务托管路径保留服务进程，旧子进程退出时不结束服务。
6. 发出成功通知。

未运行时重启：

1. 重新生成 `ProcessLaunchConfig`。
2. 走 `MihomoService::restart(config)`，即 stop + start。

## 启动后的运行态接入

核心 `Running` 后由 UI 路由焦点触发：

- Logs 隐藏路由：`StartLogMonitoring` 每秒读取 `data_dir/logs/core.log` 新增内容并回填日志页。
- 全局运行中：`StartTrafficMonitoring` 维持状态栏速率。
- Connections：`StartConnectionsMonitoring` 订阅 `/connections?interval=500`。
- ProxyGroups：`RefreshProxies` 调用 `/group`。
- Rules：`RefreshRules` 调用 `/rules`。

## 当前限制

- Windows TUN 主路径依赖安装内核服务；未安装时启动会失败并提示。
- macOS/Linux 的 TUN 提权、服务化和权限处理仍未实现。
- `core.runtime.config.yaml` 是运行配置；用户直接编辑的是 `core.common.config.yaml`。
- base64 订阅转换未实现，无法参与运行配置合并。
