# 第三方声明

本仓库中的 Air 源码使用 MIT License。项目也使用、引用或交互了若干第三方项目和资源。

## 运行时与数据

- [mihomo](https://github.com/MetaCubeX/mihomo) 会在构建期按当前 target 下载，并作为外部核心进程运行。下载得到的压缩包会缓存在被忽略的 `mihomo/` 目录中，不会提交到本仓库。
- [meta-rules-dat](https://github.com/MetaCubeX/meta-rules-dat) release 资产会在构建期作为 geodata 下载，并缓存在被忽略的 `mihomo/` 目录中。

如果要发布包含这些产物的安装包或二进制包，请先核对上游项目的许可证和再分发条款。

## UI 与框架

- [GPUI](https://github.com/zed-industries/zed) 和 `gpui_platform` 直接来自 Zed 源码仓库。
- [gpui-component](https://github.com/longbridge/gpui-component) 作为组件库使用。
- UI SVG 图标基于 [Lucide](https://lucide.dev/) 的图标命名和视觉语言。
- Emoji SVG 资源使用 Twemoji 风格资源，用于在不同平台上提供一致的显示效果。

## 参考文档

- `docs/config.yaml` 是 mihomo 配置参考夹具。文件中的示例密码、密钥、token 和 URL 都是文档示例，不是真实项目凭据。
- `docs/mihomo-api.md` 记录 Air 使用到的 mihomo external-controller API。
