# QuickQuick 后台不占 Dock 图标方案（macOS Accessory）

> 日期：2026-06-05
> 范围：让 QuickQuick 在 macOS 上始终不在 Dock 显示应用图标，纯靠系统托盘图标与全局热键存在/唤起，对标 Maccy / Paste 等剪贴板工具。
> 定位：本功能为单一聚焦改动，走 feature-dev 七阶段实现，本文是其冻结设计源。
> 状态：已实现（客观项 O1–O4 通过 + 规范审查 APPROVE）；真机项 M1–M3 待采证。

---

## 一、背景与诉求

QuickQuick 是常驻后台的剪贴板/翻译效率工具：开机自启、平时挂在后台，通过全局热键（`Cmd+Shift+V` / `Cmd+Shift+T`）或系统托盘图标唤起窗口。但当前它在 macOS Dock 里**常驻一个应用图标**，与「后台驻留工具」的定位不符——用户反馈其它剪贴板工具（Maccy、Paste 等）都不占 Dock 位。

**诉求**：后台运行时不在 Dock 显示图标。

---

## 二、现状分析

| 环节 | 现状 | 位置 |
|---|---|---|
| 系统托盘图标 + 菜单（显示/退出） | ✅ 已就绪，左键/菜单均可唤起窗口 | `src-tauri/src/tray.rs` |
| 失焦/关窗隐藏到后台（`stay_in_tray`） | ✅ `stay_in_tray=true` 时 `hide()` 不退出 | `src-tauri/src/lib.rs:647` |
| 全局热键唤起 | ✅ 已注册 history/translate | `src-tauri/src/lib.rs:192` |
| 进程激活辅助 `activate_app_macos()` | ✅ 已存在（popover 用），调 `NSApplication.activate()` | `src-tauri/src/popover.rs:94` |
| **macOS activation policy** | ❌ 从未设置 → 默认 `Regular`（Dock 常驻图标） | — |

**根因**：Tauri 应用默认以 `Regular` 激活策略运行，macOS 据此在 Dock 显示图标。要去掉 Dock 图标，需把激活策略改为 `Accessory`（等同 `LSUIElement` 行为：进程存在但不占 Dock、不进 Cmd-Tab）。

---

## 三、产品决策（已确认）

| 决策项 | 选定 | 含义 |
|---|---|---|
| 隐藏策略 | **始终不占 Dock（全程 Accessory）** | 启动即设 `Accessory`，恒定不变；窗口打开时也无 Dock 图标，与 Maccy/Paste 一致。 |
| 唯一唤起入口 | **托盘图标 + 全局热键** | 无 Dock 图标后，托盘左键/菜单与热键是唤起主窗口的入口（均已具备）。 |

**为何不选「动态切换」（窗口可见时显示 Dock、隐藏时去掉）**：动态切换需在每次显示/隐藏处切换 policy，macOS 上有焦点闪烁与时序竞态风险，且并非对标工具的做法；用户明确选择「始终不占」，更简单稳健。

---

## 四、技术方案

### 4.1 启动设置 Accessory 策略

在 `lib.rs` 的 `setup()` 阶段（`tray::setup_tray` / `setup_main_window_behavior` 同段）调用：

```rust
#[cfg(target_os = "macos")]
app.set_activation_policy(tauri::ActivationPolicy::Accessory);
```

- 用 `#[cfg(target_os = "macos")]` 守卫：该 API 与 Dock 概念仅 macOS 有意义，Windows/Linux 行为完全不变。
- 启动即恒定为 `Accessory`，全程不再切换。

### 4.2 关键坑：Accessory 下补足显示窗口的键盘焦点

**这是本方案的核心风险点，不可只设 policy 了事。**

`popover.rs:83-89` 已记录并解决过同一问题：tauri/tao 的 `window.set_focus()` 底层用的是 macOS 14+ **已废弃**的 `activateIgnoringOtherApps:`，在新系统（含 macOS 26.5）上该方法被忽略，仅靠 `set_focus()` 无法让进程真正激活、窗口拿不到 key 状态——**键盘事件永远进不了 webview**（搜索框打不了字）。popover 的解法是先调 `activate_app_macos()`（`NSApplication.activate()`，macOS 14+ 正式接口）再 `set_focus()`。

当前 `tray.rs::show_and_focus_window` 只做 `show() + set_focus()`，**没有 activate**。它现在能用，是因为 `Regular` 策略下进程本就是前台可激活的普通 app。**一旦切到 `Accessory`，进程不在常规激活序列里，不显式 activate 则窗口虽显示但键盘焦点拿不到。**

**解法**：让 `show_and_focus_window` 在 `set_focus()` 前先激活进程，复用已有的 `activate_app_macos()`。为避免重复造 `NSApplication` FFI（DRY），把 `activate_app_macos` 从 `popover.rs` 私有提升为 `pub(crate)`（或抽到共享小模块），由 popover 与 tray 共用同一份实现。

```rust
// tray.rs::show_and_focus_window（macOS：先激活进程，再聚焦窗口）
pub(crate) fn show_and_focus_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else { /* 记录返回 */ };
    if let Err(e) = window.show() { /* 记录返回 */ }
    #[cfg(target_os = "macos")]
    crate::popover::activate_app_macos();   // 顺序：先 activate 进程，后 set_focus
    if let Err(e) = window.set_focus() { /* 记录 */ }
}
```

> 顺序遵循 popover 已验证的约定：**先 activate 进程，再 `makeKeyAndOrderFront`/`set_focus`**；反序则窗口前置但进程未激活，键盘焦点仍拿不到。

### 4.3 控制流（改动后）

```
[启动] setup
   ├─ setup_tray            （已有）
   ├─ setup_main_window_behavior（已有：失焦/关窗 → hide 到托盘）
   └─ set_activation_policy(Accessory)   ★ 新增，仅 macOS → Dock 无图标

[唤起] 托盘左键/菜单「显示」 or 全局热键 or 第二实例
   └─ show_and_focus_window
        ├─ window.show()
        ├─ activate_app_macos()   ★ 新增，仅 macOS → 进程激活，键盘焦点可达
        └─ window.set_focus()
```

---

## 五、改造点（文件级）

| # | 文件 | 改动 |
|---|---|---|
| 1 | `src-tauri/src/lib.rs` | `setup()` 内新增 `#[cfg(target_os="macos")] app.set_activation_policy(ActivationPolicy::Accessory)`。 |
| 2 | `src-tauri/src/popover.rs` | `activate_app_macos` 由私有提为 `pub(crate)`（供 tray 复用，单一实现）。 |
| 3 | `src-tauri/src/tray.rs` | `show_and_focus_window` 在 `set_focus()` 前调 `activate_app_macos()`（macOS 守卫）。 |

### 不改动

- `stay_in_tray` 语义、失焦/关窗隐藏逻辑（`setup_main_window_behavior`）。
- 托盘菜单项与图标（`tray.png`、显示/退出）。
- 窗口启动显隐策略（`visible:false`）、`tauri.conf.json`、前端代码。

---

## 六、测试与验收

### 客观项（tester 可自动跑）

| # | 断言 | 验证手段 |
|---|---|---|
| O1 | 显示路径在 `set_focus()` 前调用进程激活（顺序回归守卫） | Rust 单测：对显示流程的可测逻辑/函数指针存在性断言；变异打乱顺序应失败 |
| O2 | `activate_app_macos` 提为 `pub(crate)` 后仍可被 tray 引用且编译通过 | `cargo build` + fn-pointer 类型守卫 |
| O3 | 激活策略决策点锚定为 `Accessory`（防误改回 `Regular`） | Rust 单测：具名常量/小函数返回 `Accessory` 的回归守卫 |
| O4 | 全量 Rust 测试 + clippy 无新增错误（增量口径） | `cargo test` + `cargo clippy` |
| O5 | 前端测试不回归（本功能不改前端，纯防御） | `pnpm test` |

### 真机手动确认（manual_confirm，headless 无法自动化）

| # | 断言 | 采证方式 |
|---|---|---|
| M1 | 后台运行时 Dock 不显示 QuickQuick 图标 | 真机目测 Dock |
| M2 | 托盘/热键唤起后，主窗口可见**且搜索框能正常打字**（键盘焦点到位） | 真机操作 |
| M3 | 失焦/关窗仍隐藏到托盘、进程不退出（`stay_in_tray` 语义不变） | 真机操作 |

> O1/O3 是纯 GUI/全局状态行为，单测只能锚定「决策与调用顺序」这一可测层，真实的 Dock 隐藏与键盘焦点必须真机确认（M1–M3）。这与 popover 模块「FFI 包装在 headless 不真正调用 NSApplication」的既有测试边界一致。

---

## 七、风险与回滚

- **键盘焦点回归（最高优先）**：若 4.2 的激活补足遗漏或顺序写反，表现为唤起后窗口可见但搜索框打不了字。M2 为必采证项；实现期以 popover 已验证顺序为准。
- **`set_activation_policy` API 形态**：Tauri v2 `App` 上该方法的可用性与签名以编译期为准；若该版本未暴露，则回退用底层 `NSApplication.setActivationPolicy_`（objc2-app-kit，项目已依赖）。属实现期首要验证点。
- **回滚**：三处改动均为加法且 macOS 守卫，移除即恢复原 `Regular` 行为；不触碰持久化与跨平台路径，回滚零残留。

---

*QuickQuick 后台不占 Dock 图标方案 · 2026-06-05 · 已实现 · 待真机采证*
