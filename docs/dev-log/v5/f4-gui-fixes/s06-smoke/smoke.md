---
id: f4-gui-fixes-s06-smoke
title: GUI 启动冒烟验证（自动化可达上限）
status: 冒烟通过（像素/交互验证仍需用户）
commit: PENDING
date: 2026-06-02
---

# GUI 启动冒烟验证

## 做法
`PATH=$HOME/.cargo/bin:$PATH pnpm tauri dev` 后台启动，捕获完整 stdout/stderr 到日志，确认应用能真实 boot、无启动 panic、各 setup 步骤无错误，随后关闭进程。

## 结果（实跑证据）
- vite dev server 就绪：`VITE v5.4.21 ready` @ http://localhost:1420。
- Rust 全量编译通过：`Finished dev profile ... in 9.04s` → `Running target/debug/quickquick`。
- 进程存活确认：`pgrep target/debug/quickquick` → pid 44724（启动后持续运行，非崩溃退出）。
- 全日志扫描 `panic / thread panicked / 找不到 main / 注册失败 / 创建失败 / tray / hotkey` 关键字：**零命中**。
- 即：app 成功启动，updater / autostart / global-shortcut 三插件编译并初始化，窗口与托盘 setup（`tray::setup_tray` 等）无错误日志。
- 验证后 `pkill` 干净关闭 dev 进程。

## 结论
**启动冒烟通过**——代码能构建成可运行二进制并正常 boot，无启动期崩溃/初始化错误。这是无人值守可达的运行时验证上限。

## 仍需用户人眼/交互确认（物理上无法由 agent 完成）
自动截图受 macOS 屏幕录制权限限制、键盘注入需 Accessibility 授权、视觉/交互需真人操作，以下必须用户在运行的 app 上确认：
1. **托盘图标**：菜单栏显示几何环双 Q（非白圆）。
2. **popover 浮层**：⌘⇧V 剪贴板浮层 / ⌘⇧T 翻译浮层弹出、毛玻璃透明、失焦消失、Esc 关闭、键盘流（↑↓/Enter 粘贴/Alt+Enter 复制）、trans 自动翻译与展开跳转。
3. **真实自动粘贴**：系统设置授予「辅助功能」后，选条目按 Enter 是否真把内容 ⌘V 粘到目标 App（含焦点回归、100ms 延时是否充足）。
4. **一键翻译/图片阈值/主题** 等视觉与交互。

> 提示：改了 tauri.conf / Cargo.toml / Rust 代码后需重启 `pnpm tauri dev` 重新构建才生效。
