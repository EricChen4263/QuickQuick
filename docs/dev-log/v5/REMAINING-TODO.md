# QuickQuick UI 改造 · 剩余待办（留待下个会话接续）

> 记录时间：2026-06-02。本会话按 `~/Downloads/index.html`、`popover.html`、`quickquick-icon-final-2.html` 三份设计稿，走 feature-dev 流程推进 UI 改造。
> 当前 git HEAD：`60a9645`（工作树干净）。下个会话从这里接续。

---

## 一、本会话已完成（已提交）

| 提交 | 内容 |
|------|------|
| `8432186` | 里程碑1：设计系统底座 + 外壳（OKLch token / themeStore 三档主题 / AppShell-SideBar-ThemeSwitch / FOUC 防闪） |
| `ae0227b` | 里程碑2：主窗三页视觉重塑（剪贴板/翻译/设置 照设计稿，外科手术式 class 化，逻辑零改动；EmptyState/DirBar/OnboardCard/设置4件套；token 全迁移删 compat） |
| 多个 `fix(ui)` | 用户肉眼验证驱动的视觉修复：预览操作按钮补齐、页面高度链、三页滚动收敛+整页 overflow 锁死、列表点击选中、复制接真、去侧边栏 hint、翻译源未配置置灰、列表行去删除按钮/去多余收藏星/图片项加收藏 |
| `c2d5b1a` | 自绘标题栏 Overlay（titleBarStyle:Overlay + hiddenTitle，TitleBar 组件 Quick 品牌蓝 + 拖拽区） |
| `7a7f341` | 翻译方向条 swap（点双箭头交换源/目标语重新翻译，源 auto 时禁用） |
| `0686e88` | 应用图标 + 菜单栏托盘图标（几何环双 Q 定稿；rsvg-convert + tauri icon 全套；tray.png 单色模板 + icon_as_template） |
| `347a8c8`/`0c2089c`/`afac6d5`/`1ca26af`/`5b4db1d`/`60a9645` | 里程碑3：补后端 IPC + 设置接真（设置域12命令+系统域4命令；CaptureState 运行时状态；serde 兼容；ipc-client 18封装；themeStore 双轨；隐私/存储/粘贴接真；排除名单运行时即时生效修复） |

**测试基线**：前端 270 测试全绿，后端 `cargo test` 全绿（cargo 在 `$HOME/.cargo/bin/cargo`），`pnpm build` exit 0。

---

## 二、剩余主任务：里程碑4 — popover 热键浮层窗口（未实现，架构已定）

照 `~/Downloads/popover.html`：① 剪贴板快速选取浮层（Raycast 风，720px 毛玻璃，搜索+左列表右预览+底部快捷键 footer，⌘⇧V）；② 选中即译迷你翻译浮窗（⌘⇧T）。

### 已确定的架构方案（feature-dev Phase4 产出，直接据此实现）

**1. 窗口架构：两个独立 webview 窗口 + main 不变**
- `clip-popover`（720×480）、`trans-popover`（320×~200）：均 `decorations:false / transparent:true / alwaysOnTop:true / skipTaskbar:true / visible:false / resizable:false`。
- main 窗口保持现状（托盘/Dock 完整管理窗口）。两 popover 各一窗口（尺寸差异大、可能同屏共存，不切模式）。
- **懒建**：首次热键触发时 `WebviewWindowBuilder` 建窗，之后 show/hide 不 destroy。

**2. 前端入口：Vite 多入口 + 两个独立 React 根**
- `vite.config.ts` `rollupOptions.input` 新增 `src/clip-popover/index.html`、`src/trans-popover/index.html`。
- 各自 `main.tsx` → App 组件。popover 的 `base/popover.css` 设 `background: transparent`（不影响 main 不透明背景，故必须独立入口而非同 bundle 路由）。
- dev 模式 url 指向 `http://localhost:1420/clip-popover/` 等；prod 指独立 HTML。Tauri 窗口 url 可被 `WebviewWindowBuilder::url()` 代码覆盖（dev 用 `#[cfg(dev)]` 判断）。

**3. 热键路由改造（lib.rs）**
- 现状 `trigger_window` 固定操作 main + emit "route"。改为 `popover::trigger_popover(handle, label)`：get_or_create 窗口 → 定位 → show + focus（不再 emit route）。
- history 热键 → `clip-popover`；translate 热键 → `trans-popover`。
- 新建 `src-tauri/src/popover.rs` 封装 `create_clip_popover`/`create_trans_popover`/`trigger_popover`，隔离建窗逻辑。

**4. 毛玻璃：CSS `backdrop-filter` + 透明窗口**（不用 window-vibrancy crate / 不用 Tauri effects API——后者 macOS 有已知 bug）。复用 `tokens.css` 已预留的 `--glass`/`--glass-border`。root 元素 `backdrop-filter: blur(28px) saturate(1.8)`。视觉不够再渐进增强叠 Tauri effects。

**5. 窗口行为**
- 失焦消失：popover 懒建时在 `create_*` 内注册失焦监听 → `window.hide()`（无条件，不查 stay_in_tray）。
- ESC 关闭：前端 keydown Escape → `getCurrentWindow().hide()`（各 main.tsx 注册）。
- 定位：复用 `window_pos.rs`，但需把 `compute_window_position` 重构为 `compute_window_position_for_width(window, width)`（main 传 ~400，clip 传 720，trans 传 320；保留原函数为 wrapper）。
- 键盘导航（clip-popover）：search input 上绑键盘（Spotlight 模型）；↑↓ 复用 `src/panels/history/keyboard.ts` 的 `moveHighlight`；Enter → `pasteToFront(id)` → hide；Alt+Enter → `writeToClipboard(content)` → hide；Esc → hide。

**6. 选中即译「获取选中文本」——v1 主动降级（重要决策）**
- 不做 Accessibility AXSelectedText（需 unsafe Rust + 权限弹框，高风险）；不做模拟 Cmd+C（污染剪贴板）。
- **v1 降级**：⌘⇧T 触发时直接读当前剪贴板最新一条（`listClipItems()[0]`），Rust `trigger_popover("trans-popover")` show 后 emit `translate-text` 事件 payload `{text}`，trans-popover 前端监听并自动翻译。用户操作序列：先 Cmd+C 复制 → 再 ⌘⇧T（自然，macOS Translate 也如此）。payload 空时显示「请先复制文字再按 ⌘⇧T」。
- 后续可升级 AXSelectedText（架构不变，trigger 时先尝试 AX 失败 fallback 读剪贴板）。

**7. 复用 / 新建清单**
- 直接复用：`themeStore.ts`、`tokens.css`(--glass)、`ipc-client.ts`(listClipItems/translateText/pasteToFront/writeToClipboard)、`moveHighlight`、`filterBySearch`。
- 轻改：`window_pos.rs`（width 参数化）；`ClipItemRow.tsx` 把内部 `KindIcon` 提为命名导出供复用。
- 新建前端：`src/clip-popover/{index.html,main.tsx,ClipPopoverApp.tsx,PopoverList.tsx,PopoverPreview.tsx,PopoverFooter.tsx,popover.css}`；`src/trans-popover/{index.html,main.tsx,TransPopoverApp.tsx,MiniTranslate.tsx,trans-popover.css}`。
- 新建/改 Rust：`popover.rs`（新建）；`lib.rs`（热键 handler 改指向 + 失焦辅助）；`window_pos.rs`（width 参数）；`tauri.conf.json`（新增两窗口配置）。

**8. 实现批次拆分（建议顺序）**
- **Batch A 骨架**：tauri.conf 两窗口 + popover.rs trigger 懒建+失焦 + lib.rs 热键改指向 + window_pos width 参数 + vite 多入口 + 两个 popover 占位 main.tsx。验收：热键弹窗、失焦消失、ESC 关闭。
- **Batch B clip-popover UI**：popover.css 毛玻璃 720 布局 + ClipPopoverApp 加载分组(收藏/今天)+搜索 + List/Preview/Footer + 键盘流(Enter pasteToFront/Alt+Enter 复制) + themeStore 主题同步。
- **Batch C trans-popover UI**：trans-popover.css 毛玻璃 300 + 监听 translate-text 事件自动翻译 + MiniTranslate(方向+译文+复制/朗读/展开)；展开按钮 emit 给 main 跳 translate 页；空 payload 降级文案。
- **Batch D 打磨+测试**：键盘逻辑测试(moveHighlight 已有)、PopoverList 分组渲染测试、`compute_window_position_for_width` 参数化测试、空列表/超时边界。

**9. 风险**
- 透明窗口在 macOS 各版本 blur 表现差异（实机测，必要时 `.set_shadow(true)`）。
- 懒建首次弹出 50–150ms 延迟（可接受，或 setup 阶段预建 visible:false）。
- Vite 多入口在 Tauri dev 的 devUrl（popover 窗口 url 代码动态设，`#[cfg(dev)]`）。
- 跨窗口 emit（展开→main）用 `handle.emit_to("main", ...)`。

---

## 三、里程碑3 遗留技术债（小项，可在里程碑4 后或单独处理）

1. **「一键翻译」按钮跳转**：`ClipPreview` 的 onTranslate 当前是占位（注释「里程碑3接入：跳转翻译页」）。需实现：点一键翻译 → 切 App `activeTop=translate` + 把该条目内容填入翻译输入并翻译。涉及 ClipboardPage → App 层跨页通信（提升 state 或事件）。
2. **`paste_to_front` 真实自动粘贴**：当前降级为 `write_back_only`（仅写回剪贴板）。完整需 macOS Accessibility 授权检测（AXIsProcessTrusted）+ CGEvent 注入 ⌘V + 隐藏自身窗口激活前台 App。后端 `paste.rs`/`onboarding.rs` 已有 trait 与降级分支，接 OS 实现即可。
3. **自动检查更新真实 endpoint**：`auto_update` 开关只存配置，`tauri-plugin-updater` endpoint 仍是 `placeholder.example.com`。需真实更新服务端才能接 `app.updater().check()`。
4. **存储「单张图片阈值」**：StoragePanel 中此项为静态展示，无对应 IPC（如需可配置需后端加阈值字段 + 命令）。

---

## 四、GUI 视觉验证（需用户手动）

里程碑1–3 + 图标的真实 GUI 效果需用户 `pnpm tauri dev`（注意：cargo 需在 PATH，或 `$HOME/.cargo/bin/cargo`；本机 `pnpm tauri dev` 报 cargo metadata not found 但前端 HMR 仍可看）肉眼确认。**改了 tauri.conf 窗口配置（标题栏 Overlay、图标、里程碑4 新窗口）需重启 tauri dev 才生效。** 自动截图受屏幕录制权限限制，只能用户手动截图反馈。

---

## 五、下个会话接续指南

- **流程**：继续走 feature-dev（里程碑4 已过 Phase4 架构，下次直接 Phase5 实现：coder TDD → tester 动态证伪 → code-reviewer 审查；子 agent 用 sonnet）。用户此前定：4 里程碑逐个走完整七阶段；接真深度=持久化配置+接已有 API。
- **cargo 路径**：`$HOME/.cargo/bin/cargo`（不在默认 PATH）。后端验证 `cd src-tauri && $HOME/.cargo/bin/cargo check/test`。
- **coder agent 易截断**：本会话 coder 多次在「写 dev-log/末尾验收」时输出截断。对策——任务分小批次、让 coder **不写长 dev-log**、**尽早 git 提交作为锚点**（tester 变异可安全 git checkout 还原）。tester 也多次截断，必要时只读核对 + 综合证据判定。
- **测试风格铁律**：测试文件**显式 `import { describe, it, expect, vi } from "vitest"`**（项目 22+ 文件既有约定，tsconfig 无 vitest/globals，globals 风格会 tsc 报错）。
- **设计 token**：已统一短名（--bg/--fg/--accent/--glass 等），无 --qq-* 残留。popover 直接复用。
- **dev-log 留痕**：里程碑 review 落在 `docs/dev-log/v5/f2-design-system/s0x-*/review.md`。
