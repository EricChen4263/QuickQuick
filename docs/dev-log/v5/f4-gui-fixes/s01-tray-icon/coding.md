---
id: f4-gui-fixes-s01-tray-icon
title: 修复托盘图标白圆（改 include_bytes 嵌入 + 回退不套 template）
status: 实现完成
commit: 4bd6ea1
date: 2026-06-02
---

## 根因

`tray.rs` 从 `resource_dir()/icons/tray.png` 读图，但 tray.png 未 bundle 进 resource_dir，dev/prod 均读不到 → 回退到应用图标（彩色）却仍套 `icon_as_template(true)` → macOS 把整片不透明区域渲染成实心白块。

## 改法

将 `src-tauri/src/tray.rs` 中加载逻辑改为 `Image::from_bytes(include_bytes!("../icons/tray.png"))`，编译期嵌入，不依赖 resource_dir。成功路径套 `icon_as_template(true)`（正确单色图），失败回退应用图标且**不套 template**，避免再现白圆。删除原 `from_path` / `resource_dir` / `or_else` 逻辑；更新文件头注释与实现一致；`Manager` import 因 `get_webview_window` 仍使用而保留。

## 验证结果

```
cargo check: exit 0，无 error，无 warning
cargo test:  244 passed; 0 failed; 0 ignored（含 tray_single_source_no_auto_trayicon_in_conf）
```

## GUI 效果

需用户重启 app（`cargo tauri dev` 或打包后）肉眼确认菜单栏托盘图标显示为单色双 Q 环形，而非实心白圆；明/暗菜单栏切换后图标应自动反色。
