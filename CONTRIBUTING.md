# 贡献指南

感谢参与 Air 开发。Air 是一个 Rust workspace，包含原生 GPUI 桌面界面和 mihomo 服务层。改动应尽量限定在拥有对应行为的 crate 中。

## 开发环境

要求：

- 支持 Rust 2024 edition 的稳定 Rust 工具链。
- Git，以及 Cargo 依赖拉取所需的网络访问。
- 首次构建需要网络访问，以便 `build.rs` 将 mihomo/geodata 下载到被忽略的 `mihomo/` 目录。
- 当前 Windows 是主要开发平台。

常用命令：

```powershell
cargo fmt --check
cargo check
cargo test
cargo run -p air-desktop
```

## 架构边界

- `air-ui` 负责展示和局部交互状态，并通过 `air-app` 命令入口触发业务行为。
- `air-app` 负责命令路由、任务调度、事件、快照和服务装配。
- 配置解析、合并、进程管理、API 请求、订阅下载和平台能力应放在 domain、storage、core、API 或 platform crate 中。
- 不要让 `air-app` 依赖 `air-ui`。
- 不要让 `air-config` 或 `air-domain` 依赖 `air-storage`。
- 平台差异应集中在 `air-platform`，不要把 `cfg(target_os)` 分散到业务逻辑中。

更多说明见 [docs/architecture.md](docs/architecture.md)。

## 代码规范

- 新增 Rust 代码应包含必要且具体的中文注释，用于解释设计意图、边界条件、平台差异或复杂逻辑。
- 配置解析和保存必须保留 mihomo 未知字段。
- 用户配置写入应使用 `FileStore` 的原子写入和备份行为。
- 日志和 UI 通知必须脱敏 secret、订阅 URL、认证头、代理密码和本地敏感路径。
- 修复 bug 时应尽量先补一个聚焦的失败测试；如果无法自动化，请在 PR 中说明原因。

## Pull Request

提交 PR 前请运行：

```powershell
cargo fmt --check
cargo check
cargo test
```

如果改动影响 GUI、托盘、服务、TUN、打包或进程行为，请在 PR 中说明目标平台。
