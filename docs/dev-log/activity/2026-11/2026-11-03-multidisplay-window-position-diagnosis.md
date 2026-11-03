# 多显示器唤出落点错屏 —— 只排查不改

## 背景
- 用户报告：多显示器下应用唤出时不显示在**当前焦点所在显示器**，热键（⌘⇧C / ⌘⇧T / ⌘⇧M）与菜单栏唤出都有同样问题。
- 明确要求：先排查，不改代码。本卡只记录诊断结论与证据，不含任何改动。

## 结论（一句话）
两条唤出路径的病因不同：
1. **popover 路径（⌘⇧C / ⌘⇧T）**：`window_pos.rs` 把 tao 的 `cursor_position()` 返回值当成「物理像素」去和 `MonitorSpec` 的物理边界比，实际它恒用**主屏 scale** 放大过，混合 DPI 下必然错屏 → 落到 `primary_monitor()` 兜底 = 用户看到的主屏。
2. **主窗口路径（⌘⇧M / 托盘菜单）**：`show_and_focus_window` **完全没有定位代码**，只 `show()` + `set_focus()`，窗口停在上次的 frame，与光标所在屏无关。

## 证据链

### 1. 本机真实几何（Swift/CoreGraphics 实测）
| 屏 | NSScreen.frame(点) | scale | CGDisplayBounds | PixelsWide/High |
|---|---|---|---|---|
| Built-in（主屏） | (0,0,2056,1329) | 2.0 | (0,0,2056,1329) | 2056 / 1329 |
| HP Z27 | (2056,-111,2560,1440) | 2.0 | (2056,0,2560,1440) | 2560 / 1440 |
| HP P27q G4 | (4616,-111,2560,1440) | 1.0 | (4616,0,2560,1440) | 2560 / 1440 |

关键事实：`CGDisplayBounds` 与 `CGDisplayPixelsWide/High` **都是点**，不是后备像素（只有 `CGDisplayModeGetPixelWidth` 是像素）。

### 2. tao 侧两个实现（tao 0.35.3）
- `util::cursor_position()`：`(point.x, main.pixels_high - mouse.y).to_physical(primary_monitor().scale_factor())` —— 恒乘**主屏** scale。
- `MonitorHandle::position()/size()`：`CGDisplayBounds.origin × 本屏 scale` / `pixels_wide,high × 本屏 scale`。
- 两侧不是同一个坐标空间 → `find_monitor_at` 的比较无效。

### 3. 复刻验证（逐屏模拟，`/tmp/qqprobe/live.swift`）
```
鼠标在 Built-in 中心(点 1028,664) → 比较值(2056,1329)  → Built-in  ✅
鼠标在 HP Z27 中心(点 3336,720)  → 比较值(6672,1440)  → HP Z27    ✅
鼠标在 HP P27q 中心(点 5896,720) → 比较值(11792,1440) → nil       ❌ 回退主屏
```
判定成立的充要条件是「光标屏 scale == 主屏 scale」：本机 2x 外接屏碰巧判对，1x 屏必然判错并回退主屏。

### 4. 调用点
- `src-tauri/src/window_pos.rs:39-46` 取 cursor 并注释「刻意留在物理空间」——该前提不成立。
- `src-tauri/src/window_pos.rs:50-58` `find_monitor_at` 判错后 `.or_else(primary_monitor)` / `monitors.first()` 兜底 → 主屏。
- `src-tauri/src/tray.rs:671-692` `show_and_focus_window` 只有 show + activate + set_focus，**无任何 set_position / center**。
- `src-tauri/src/lib.rs:871-880`（⌘⇧M）与 `tray.rs:382`（菜单项）/ `tray.rs:402`（左键）三条唤出入口全部汇入同一个 `show_and_focus_window`。
- 全仓 `set_position` 只有 `popover.rs:278` 一处 —— 主窗口从未被定位过。

## 需要区分的两个「当前屏」
- `cursor_position()` 用的是**鼠标**所在屏；用户说的是「**焦点**所在显示器」。
- 两者在键盘操作（鼠标停在别屏）时会分叉，这是第二个独立缺陷，修 cursor 单位问题不会一并解决。

## 未覆盖 / 待定
- 未在真机逐屏复现用户操作（本机实测光标当前恰好停在 2x 屏，1x 屏的分支由复刻脚本推断）。建议真机把鼠标移到 HP P27q 后按 ⌘⇧C 验证。
- 未实测 Siri 式「菜单栏唤出」时 AppKit 是否会自行把窗口搬到当前屏（当前代码无此逻辑，但系统行为未取证）。
- 上游至今未修：tao `dev`(0.37.2) 两处实现未变，针对性 PR tao#1272 已 closed 未合并 → 升级 tao 无法解决。
- 修复方向已调研（AppKit `NSEvent.mouseLocation` + `NSScreen.screens` + 目标屏 `visibleFrame`，参考 Maccy `PopupPosition.swift`），本次按用户要求**不动手**。

## 复现脚本
- `/tmp/qqprobe/probe.swift`：三屏几何与光标原值。
- `/tmp/qqprobe/live.swift`：复刻 tao 判屏逻辑并逐屏模拟。
