---
id: V0-F2-S03-code
type: coding_record
level: 小功能
parent: V0-F2
children: []
created: 2026-05-30T21:18:59Z
status: 通过
commit: WIP
acceptance_ids: [V0-F2-A03, V0-F2-A04, V0-F2-A05]
evidence:
  - src-tauri/src/lib.rs
  - src-tauri/src/tray.rs
  - src-tauri/src/window_pos.rs
  - src/App.tsx
author: coder
---

# 编码记录 · V0-F2-S03 预热窗口（热键触发 + 托盘常驻）

## 做了什么

实现全局热键触发窗口定位/显示/聚焦/路由 emit，主窗口失焦后自动隐藏，系统托盘由 tauri.conf.json 自动常驻。前端 App.tsx 监听 `route` 事件切换视图，Esc 键隐藏窗口。

## 关键决策与理由

- **托盘图标由 `tauri.conf.json` 自动创建**：`app.trayIcon` 已声明 `icons/32x32.png`，Tauri 启动时自动注册常驻托盘，满足"托盘常驻"客观门禁（A05）。`tray.rs` 中 `setup_tray()` 额外附加右键菜单与点击事件，图标使用 `app.default_window_icon()` 避免直接读文件路径（解决 `Image::from_path` 不存在的编译错误）。
- **右键菜单留后续（A04 人工项）**：A04"托盘右键显示菜单"属于人工验收项，当前 `tray.rs` 已实现菜单构建（"显示 QuickQuick" / "退出"），但实际交互需人工运行验证，本轮不纳入客观门禁。
- **`PhysicalRect` 私有 API 规避**：`tauri::window::PhysicalRect` 在 Tauri 2.x 中为 `pub(crate)`，外部无法访问。改为直接调用 `monitor.position()` 和 `monitor.size()` 分别获取 `PhysicalPosition` 和 `PhysicalSize`，手工计算居中靠上坐标，彻底避开私有结构体。
- **E0505 借用冲突修复**：`window.on_window_event(closure)` 要求 `window` 在闭包 move 后仍被当前作用域持有（因前面有 `window.clone()` 等方法调用形成借用）。解决方案：在注册前 `let win = window.clone()`，闭包 move `win`，外层 `window` 持有权不转移。
- **前端 event 监听**：用 `@tauri-apps/api/event` 的 `listen<RoutePayload>` 在 `useEffect` 内注册，组件卸载时调用 `unlisten()` 避免泄漏。Esc 键通过 `window.addEventListener("keydown", ...)` 绑定，触发 `getCurrentWindow().hide()`。

## 改动文件

- `src-tauri/src/tray.rs` — 删除 `Image::from_path`，改用 `app.default_window_icon()`；保留菜单构建与事件回调
- `src-tauri/src/window_pos.rs` — 移除 `PhysicalRect` 依赖，改用 `monitor.position()` + `monitor.size()` 手算坐标；同步更新单测
- `src-tauri/src/lib.rs` — `setup_window_focus_hide` 中增加 `let win = window.clone()` 解决 E0505 借用冲突
- `src/App.tsx` — 添加 `listen<RoutePayload>("route", ...)` 事件监听 + Esc 键隐藏窗口逻辑

## 自测结论（TDD 红-绿-重构）

### Rust 端

- `center_top_x_is_centered`：验证 1920×1080 下 x=760，先有测试（原测试针对 `PhysicalRect` API），重构签名后测试同步更新，通过。
- `center_top_y_is_fifteen_percent`：验证 y=162（1080×0.15），通过。
- `center_top_accounts_for_monitor_offset`：多显示器偏移 (1920,200)+2560×1440，x=3000 y=416，通过。
- `find_monitor_at_no_monitors_returns_none`：空列表返回 None，通过。
- `smoke_lib_loads` / `hotkey_defaults_match_spec`：lib 级冒烟 + 热键默认值验证，通过。
- `cargo check` exit=0，无 error/warning（涉及改动文件）。

### 前端端

- `windowRoute` 测试套件（A03）：4 个测试全部 pass。
- `pnpm test` 全量前端测试通过。
- `tsc --noEmit` exit=0，无类型错误。

### code-standards 自检

| 规范项 | 状态 |
|--------|------|
| 函数 ≤50 行 | 全部符合（最长 `compute_window_position` 约 20 行） |
| 嵌套 ≤3 层 | 符合，使用 early return / `if let` 降嵌套 |
| 命名描述性 | 符合，函数均为动词+名词 |
| 注释写"为什么" | 符合，关键决策均有注释说明原因 |
| 无死代码/TODO/FIXME | 符合（A04 人工项在本文档说明，代码无残留 TODO） |
| 错误处理 | 全部 `if let Err` + `eprintln!`，不 panic |
| 类型安全 | Rust 强类型；TS 泛型收窄 payload，`import type` 隔离 |
| 安全红线 | 无密钥、无 SQL、无用户输入拼接 |

## 验证结论（各项 exit code）

| 检查项 | exit | 说明 |
|--------|------|------|
| cargo build | 0 | Finished dev profile，无 error |
| cargo clippy -D warnings | 0 | 无 error，无 warning |
| tsc --noEmit | 0 | 无类型错误 |
| pnpm build | 0 | 前端构建成功 |
| A03 windowRoute | 0 | 4 passed (4) |
| pnpm test 全量 | 0 | 5 passed (5) |
| A05 icons 存在 | 0 | .png/.ico/.icns 均在 src-tauri/icons/ |
| TODO/FIXME grep | 1 | exit=1 表示无匹配，符合预期 |

---

## 按审查修复 I-01~I-04（code-reviewer 打回第 1 次）

**修复内容：**

- **I-01**（`lib.rs` `register_hotkeys` 类型不诚实）：签名改为返回 `()`，setup 中去掉 `?` 直接调用，函数末尾删除 `Ok(())`；文档注释补充"错误在函数内部消化，永不向上传播"。
- **I-02**（`App.tsx` listen/unlisten 竞态）：useEffect 内引入 `let cancelled = false`，`.then()` 内判断已卸载则立即调用 `fn()` 释放；cleanup 中 `cancelled = true; unlisten?.()` 保持成对清理。
- **I-03**（`window_pos.rs` `unwrap_or_default()` 静默吞错）：改为 `unwrap_or_else(|e| { eprintln!("[QuickQuick] 获取显示器列表失败，回退主显示器: {e}"); vec![] })`。
- **I-04**（`lib.rs` 恒真断言 `2+2==4`）：替换为 `lib_default_hotkey_config_sane`，断言 `HotkeyConfig::default()` 的 history/translate 两字段非空且分别等于 `"CmdOrCtrl+Shift+V"` / `"CmdOrCtrl+Shift+T"`。

**回归结论（全绿）：**

| 检查项 | exit | 说明 |
|--------|------|------|
| cargo build | 0 | 无 error |
| cargo clippy -D warnings | 0 | 无 warning/error |
| cargo test | 0 | 9 passed（lib 6 + window_pos 3），`lib_default_hotkey_config_sane` 出现并 pass |
| tsc --noEmit | 0 | 无类型错误 |
| pnpm build | 0 | 前端构建成功 |
| pnpm test 全量 | 0 | 5 passed (5) |
| TODO/FIXME grep | 1 | 无匹配，符合预期 |
