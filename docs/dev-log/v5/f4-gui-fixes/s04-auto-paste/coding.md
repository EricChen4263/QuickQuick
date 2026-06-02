---
id: f4-gui-fixes-s04-9a
title: 真实自动粘贴 9a macOS 后端 FFI
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 新增依赖（仅 macOS target）

- `core-graphics = "0.25"` — CGEvent 合成 Cmd+V 键盘事件
- `objc2 = "0.6"` — Objective-C 运行时基础
- `objc2-app-kit = { version = "0.3", features = ["NSPasteboard"] }` — NSPasteboard.changeCount()
- `objc2-foundation = { version = "0.3", features = ["NSString"] }` — NSString 基础类型

均通过 `[target.'cfg(target_os = "macos")'.dependencies]` 限定，非 mac 构建不引入。

## 两个生产后端实现要点（src/macos_paste.rs）

**MacOsAccessibilityProbe**：`#[link(name="ApplicationServices", kind="framework")] extern "C" { fn AXIsProcessTrusted() -> bool; }`，包一层安全函数 `ax_is_process_trusted()`，impl AccessibilityProbe。

**MacOsPasteBackend**：
- `change_count()`：调 `NSPasteboard::generalPasteboard().changeCount()`，i64→u64（max(0)）
- `write_with_marker()`：arboard `set_text()`（与 pipeline ArboardBackend 写法一致，写入自动 bump changeCount）
- `current_text()`：新建临时 arboard 实例读取（满足 &self 签名约束）
- `send_paste()`：`CGEventSource::new(HIDSystemState)` → `CGEvent::new_keyboard_event(source, KeyCode::ANSI_V=0x09, keydown)` + `set_flags(CGEventFlagCommand=0x00100000)` → `post(CGEventTapLocation::HID)`；keyDown + keyUp 各一次

## cfg(macos) 分流与非 mac 降级

- `macos_impl` 子模块加 `#[cfg(target_os="macos")]`，`fallback_impl` 加 `#[cfg(not(target_os="macos"))]`
- 非 mac：`FallbackAccessibilityProbe.is_trusted()==false`（永远走 write_back 降级），`FallbackPasteBackend.send_paste()` 为 no-op

## 运行时说明

键盘事件注入（CGEvent）需 Accessibility 授权，运行时行为无法单测。**需用户授予辅助功能权限后 GUI 实测**；本批仅保证编译链接通过 + 既有测试全绿。

## 构建/测试结果

- `cargo build`：exit 0，0 warning，链接 ApplicationServices + CoreGraphics 框架通过
- `cargo test`：exit 0，全部测试通过（含 2 个新 macos_backends 测试）

## 给 9b 的衔接

- `system.rs paste_to_front` 接入 `perform_paste_or_degrade(MacOsAccessibilityProbe, MacOsPasteBackend, item)` 待做
- 焦点切换执行器（`focus_restore_sequence` 各步的真实 OS 激活）待做
- `MacOsPasteBackend::new()` 在 setup 阶段初始化并注入到 paste_to_front 调用链
