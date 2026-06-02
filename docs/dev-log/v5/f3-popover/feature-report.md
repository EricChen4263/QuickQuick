---
id: V5-F3-report
type: feature_report
level: 大功能
parent: V5
children: [f3-popover-s01, f3-popover-s02, f3-popover-s03-b1, f3-popover-s04-b2, f3-popover-s05-c1, f3-popover-s06-c2, f3-popover-s07-d]
created: 2026-06-02T00:00:00Z
status: 已闭合
author: orchestrator
---

# 大功能报告 · V5-F3 popover 热键浮层窗口（里程碑4）

## 目标
照设计稿 `~/Downloads/popover.html` 实现两个独立的毛玻璃浮层窗口：
- **clip-popover**（⌘⇧V，720×480）：Raycast 风剪贴板快速选取——搜索 + 左分组列表 + 右预览 + 底部快捷键 footer + 键盘流。
- **trans-popover**（⌘⇧T，320×200）：迷你翻译浮窗——自动翻译剪贴板最新条 + 方向/译文/复制/朗读/展开 + 空降级。
main 窗口保持现状（托盘/Dock 管理），不受影响。

## 小功能闭合清单（均三联留痕齐 + tester 动态证伪过 + reviewer 无未决高危）

| 小功能 | 内容 | 验证结果 | 锚点 commit |
|---|---|---|---|
| s01 Batch A1 Rust 骨架 | window_pos 宽度参数化(`compute_window_position_for_width`) + popover.rs 懒建/失焦 hide/trigger_popover + lib.rs 热键改指向 + 透明窗口(macos-private-api) | cargo test 全绿(window_pos 7) | fb6a690 / 273c548(收口) |
| s02 Batch A2 前端脚手架 | Vite 多入口(main+clip+trans) + 两个独立 React 入口占位 + 毛玻璃壳 + Esc→hide + themeStore | pnpm build 三入口 exit0 | fb6a690 |
| s03 Batch B1 clip 结构 | KindIcon 提共享组件 + grouping.ts 三纯函数(TDD) + ClipPopoverApp 真数据+搜索+分组(收藏/今天/更早) + List/Preview/Footer + 720 布局 | 283 测试绿 | 856f9a4 |
| s04 Batch B2 clip 键盘流 | keyboard-nav.ts(advanceSelection 复用 moveHighlight,TDD) + ↑↓选择/Enter pasteToFront/Alt+Enter writeToClipboard/成功才 hide + actions mock 测试 | 301 测试绿 | 3174754 / 9942fab(收口) |
| s05 Batch C1 trans 翻译 | source-text.ts(pickLatestText,TDD) + TransPopoverApp 挂载自动翻译(五态) + MiniTranslate + 空降级 | 311 测试绿 | d5b4c78 |
| s06 Batch C2 trans 展开 | handleExpand 跨窗口跳 main 翻译页 + 获焦重读(tauri://focus) + retranslate.ts(shouldRetranslate 去重,TDD) | 316 测试绿 | 53e064b / 4a6c111(收口) |
| s07 Batch D 测试补齐 | PopoverList(5)/PopoverPreview(6)/MiniTranslate(5) 隔离渲染测试 + 全量验证门禁 | 335 测试绿 + cargo 全绿 | ce98b31 |

## 关键架构决策
- **两个独立 webview 窗口 + 懒建**：clip/trans 各一窗口（decorations:false / transparent:true / alwaysOnTop / skipTaskbar / visible:false / resizable:false），首次热键触发时 `WebviewWindowBuilder` 建窗并注册失焦 hide，之后 show/hide 不销毁。main 不变。
- **Vite 多入口 + 路径一致性**：popover 入口 HTML 放 `src/{clip,trans}-popover/index.html`，`WebviewUrl::App("src/.../index.html")` 在 dev(serve)/prod(dist) 路径一致，避开 MPA URL 失配陷阱。
- **毛玻璃 = CSS backdrop-filter + 透明窗口**：启用 `macos-private-api` feature + `macOSPrivateApi:true` + `transparent(true)`，root 元素 `backdrop-filter: blur(28px) saturate(1.8)` + `var(--glass)`；不用 window-vibrancy / Tauri effects(macOS 已知 bug)。
- **trans 取词 v1 降级（偏离架构，已记录）**：原架构设想 Rust 读剪贴板后 emit `translate-text`，但首次懒建时事件早于前端监听注册存在竞态。改为 **trans-popover 前端自读 `listClipItems()[0]`**（挂载 + 获焦各一次，`shouldRetranslate` 去重），零 Rust 改动、无竞态。操作序列 Cmd+C → ⌘⇧T 保证剪贴板已稳定。后续可升级 AXSelectedText（架构不变）。
- **窗口定位复用**：`compute_window_position_for_width(window, width)` 参数化（main 仍 400 零变化，clip 720，trans 320）。
- **复用既有资产**：themeStore / tokens.css(--glass) / ipc-client(listClipItems/translateText/pasteToFront) / moveHighlight / writeToClipboard(navigator.clipboard) / speakText(Web Speech)。

## 测试与验证（最终全量门禁，Batch D）
- 前端 `pnpm test`：**335 passed / 40 文件**（含本大功能新增 grouping/keyboard-nav/source-text/retranslate/window_pos-width/actions/trans/隔离渲染共 60+ 用例）。
- `pnpm exec tsc --noEmit`：0 错误。
- `pnpm build`：exit 0，三入口产出。
- 后端 `cargo test`：全套 0 failed（含 window_pos width 用例）；`cargo check` exit 0。
- 各批 tester 动态证伪累计 17+ 处变异如期变红（收藏短路/搜索/isToday/键盘移动/粘贴参数/route payload/翻译渲染/去重/分组标题/收藏 badge/复制回调等），证明测试有真实判别力，无恒真假绿。
- reviewer 各批无未决高危；中级发现（图片 Alt+Enter 写空串、错误态卡死、并发互斥、aria-label、防御性取词）均已收口修复并补回归测试。

## 待用户手动确认（manual，不阻塞代码门禁）
- **GUI 视觉与交互实机验证**：自动截图受屏幕录制权限限制，需用户 `pnpm tauri dev`（cargo 需在 PATH 或 `$HOME/.cargo/bin/cargo`）肉眼确认：两 popover 热键弹出/毛玻璃透明效果/失焦消失/Esc 关闭/clip 键盘流粘贴/trans 自动翻译与展开跳转。**改了 tauri.conf/Cargo.toml/新窗口需重启 tauri dev 生效。**

## 已知 v1 限制（留作后续增强）
- trans-popover「展开」仅跳转主窗翻译页，**不预填文本**（需主窗 App↔TranslatePage 状态管线改造）。
- Enter 对图片条目走 pasteToFront 会被后端拒绝（图片粘贴未实现），现走 catch 不 hide。
- `paste_to_front` 仍为 `write_back_only`（仅写回剪贴板，自动 ⌘V 注入待接 Accessibility/CGEvent）——属里程碑3 遗留技术债。
