# 架构边界

`air` 现在是 Rust workspace，按稳定边界拆成多个 crate。依赖方向表达架构约束：`air-ui` 收集输入并展示状态，调用 `air-app` 的命令入口；`air-app` 负责任务调度、状态快照和事件广播；`air-mihomo` 统一承载 mihomo 进程、external-controller API、订阅/代理/规则领域模型与相关业务逻辑；`air-storage`、`air-platform` 是外部资源边界。

## Workspace 结构

```text
crates/
  air-desktop/      # 最终 air binary、allocator、构建期 mihomo 下载和 Windows 可执行文件图标资源
  air-app/          # 命令路由、后台 runtime、服务装配、事件和 snapshot
  air-ui/           # GPUI shell、页面、组件、UI assets 和前端状态
  air-mihomo/       # mihomo 检测、释放、进程、API、领域模型与生命周期服务
  air-config/       # mihomo YAML 模型、解析、校验、合并和 override.js
  air-storage/      # AppPaths、FileStore、配置/订阅/覆写/应用设置持久化
  air-platform/     # tray/autostart/service/UAC/window/single-instance
  air-settings/     # app.config.toml 的纯模型和兼容反序列化
  air-telemetry/    # tracing、脱敏、日志保留和内存采样
  air-error/        # AppError/AppResult 和错误枚举
```

## 允许方向

- `air-desktop -> air-app + air-ui + air-platform + air-telemetry`
- `air-ui -> air-app + air-config + air-mihomo + air-settings`
- `air-app -> air-mihomo + air-storage + air-config + air-platform + air-settings + air-telemetry`
- `air-mihomo -> air-config + air-platform + air-telemetry + air-error`
- `air-storage -> air-config + air-mihomo + air-settings + air-error`
- `air-config -> air-error + air-telemetry`
- `air-platform -> air-error + air-telemetry`
- `air-settings -> air-error`
- `air-telemetry -> air-error`

## 禁止方向

- `air-app` 不依赖 `air-ui`；GUI 启动由 `air-desktop` 在服务/helper 分流后调用 `air_ui::launch()`。
- `air-config` 不依赖 `air-storage` 或 `air-platform`；配置层只保留轻量平台枚举用于诊断。
- `air-mihomo` 不依赖 `air-storage`；订阅更新通过 trait 抽象缓存读写，`air-storage` 为 `SubscriptionStore` 实现 trait。
- `air-storage`、`air-mihomo`、`air-platform` 不依赖 `air-ui` 或具体页面状态。
- `air-ui` 生产代码不直接依赖 `air-storage`；需要构造临时路径的 UI 单元测试可以使用 dev-dependency。
- GUI 回调不直接启动 mihomo 进程、不直接合并配置、不直接发 HTTP 请求；这些行为必须通过 `air-app` 命令进入服务层。

## 资源边界

- `crates/air-ui/assets/icons/`、`crates/air-ui/assets/emoji/`、`crates/air-ui/assets/brand/` 只服务 UI 渲染，由 `air-ui` 通过 `rust-embed` 注册。
- `crates/air-desktop/assets/app-icon.png` 只服务最终桌面产物图标，由 `air-desktop/build.rs` 在 Windows 构建时转换并嵌入。
- workspace root 不再放 UI 图标、emoji 或桌面图标资源。

## 异步边界

后台任务由 `air_app::runtime::AppRuntime` 管理。命令返回 `CommandResult`，状态变化通过 `AppEvent` 广播给 UI。长任务必须持有 `CancellationToken`，取消语义由具体服务检查并转换为统一错误。
