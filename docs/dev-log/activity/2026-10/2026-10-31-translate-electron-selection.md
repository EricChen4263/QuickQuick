# 翻译：Electron 应用（DSH）选中文字取词修复

## 现象
- 在「文本编辑」选中文字按翻译热键可以译；在 DeepSeek Harness（Electron）里选中同样无法译，只能降级到剪贴板。

## 根因（实测）
- Electron/Chromium 默认不构建无障碍树。关闭 `AXManualAccessibility` 时，DSH 的 `AXFocusedUIElement` 返回 `-25212`（kAXErrorNoValue），即使双击选中了网页文字，AX 里也没有任何 `AXSelectedText`。
- 设置应用元素的 `AXManualAccessibility = true` 后，焦点变为 `AXWebArea` 并能读到选中文字；原生应用（如 TextEdit）对该属性回应 `kAXErrorAttributeUnsupported`，无副作用。
- 首次开启后焦点元素约 2 秒才可见（实测），因此热键路径不能同步等待。

## 改动
- `src-tauri/src/selection.rs`：新增 `warm_web_accessibility`（按 pid 只成功或确认不支持一次，瞬时错误下次再试）；热键读取前顺带预热；新增后台预热入口 `warm_application_in_background` / `warm_focused_application_in_background`。
- `src-tauri/src/lib.rs`：前台应用切换时后台预热新应用；启动时预热当前前台应用（应用早于 QuickQuick 启动、之后不再切换的情况）。

## 验证
- `cargo clippy --lib -- -D warnings` 通过；`cargo test --lib` 593 项通过；`rustfmt --check` 通过。
- 真机（macOS 27，DSH 0.2.1-rc.5）：切到 DSH 后等待预热，双击选中「工作」，按 `⌘⇧T`，浮窗显示「来源：选中」，译文 `work`。
- 首次预热日志偶现 `-25204`（kAXErrorCannotComplete，应用在 0.3s 超时内未响应），属预期，未记入已预热，下次激活或热键时重试。

## 已知取舍
- 预热对每个切到前台的应用都会设置 `AXManualAccessibility`。Chromium 系浏览器（Chrome/Edge）也会因此开启无障碍树，有一定的内存与 CPU 开销；这是为取词可靠性付出的代价，待用户确认是否接受。
- 只有 Electron/Chromium 类应用受影响；原生应用行为不变。

## 未覆盖
- 未验证浮窗定位在网页选区上的精确落点（AXBoundsForRange 在网页区域返回有效矩形，但未对照视觉位置）。
- Windows 取词仍未实现（本期不做）。
