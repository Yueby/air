# UI 优化基础约束

本文件是第 5 阶段页面优化的共享约束。后续 `044` 到 `048` 页面任务应优先复用
`crates/air-ui/src/components/` 中的封装，再按页面需求补少量本地组合函数。

## 第 4 阶段控件盘点

- 日志、代理组、运行态规则、连接、订阅、配置编辑、覆写和设置页都已经统一通过
  `crates/air-ui/src/icons.rs` 渲染通过 `rust-embed` 注册的 SVG 图标；后续页面禁止新增手写 SVG 或散落字符图标。
- 设置页的布尔开关原来是自绘按钮，后续统一替换为 `gpui-component::switch::Switch`。
  配置编辑页的 `Option<bool>` 是三态字段，可以保留 chip，但不能伪装成二元 Switch。
- 多数页面使用 `overflow_y_scroll()`，配置编辑页局部已使用 `ScrollableElement::overflow_y_scrollbar()`。
  后续滚动容器统一走 `components::vertical_scroll_area` 或直接使用
  `gpui_component::scroll::ScrollableElement`，并由 `components::enforce_visible_scrollbars`
  将滚动条展示模式固定为 `ScrollbarShow::Always`。
- 各页面短反馈统一映射到右下角全局通知；页面内容区不再插入 Alert 或手写提示块。
- 全局短反馈必须通过 `components::push_global_notice` 派发
  `gpui-component::notification::Notification`，不能在页面内自行绘制 toast。

## 视觉基线

- 间距使用 4px 栅格：4、8、12、16、20px；紧凑列表行优先 8/12px 内边距。
- 圆角上限为 8px；列表项、按钮和输入附属控件默认 6px，密集 chip 可用 4px。
- 页面块只使用 1px 边框和主题色背景，不叠套卡片。重复条目可以使用独立卡片或列表行。
- hover 只改变背景或边框，不改尺寸；focus 状态必须保留边框或主题焦点色。
- 选中态使用 `palette.active` / `palette.active_text`，危险操作使用 `palette.danger`。
- 空态、加载态和错误态保持为 view model 输出的纯状态，渲染层只选择图标、文字和 Alert 级别。

## 动画策略

- 页面切换：约 120ms，轻微透明度或位移变化，不阻塞路由状态更新。
- 块 hover：约 90ms，仅做背景/边框过渡。
- 筛选结果变化：约 100ms，优先保持列表尺寸稳定，避免行高跳动。
- 弹窗出现和关闭：约 150ms，遮罩和面板同步渐变；确认动作必须先派发 command，再由事件回填关闭状态。

## View Model 边界

UI 层只展示 view model 和收集输入。搜索、排序、筛选、订阅导入校验、关闭连接、代理选择、
测速、配置保存等动作必须通过 domain 层状态方法或 `AppCommand` 派发，不在 GPUI 回调里直接
访问 storage、mihomo API 或进程服务。
