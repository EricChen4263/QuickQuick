---
id: V0-F2-S03-review
type: review
level: 小功能
parent: V0-F2
children: []
created: 2026-05-31T04:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F2-A03, V0-F2-A04, V0-F2-A05]
evidence: []
author: code-reviewer
---

# 审查记录 · V0-F2-S03 预热窗口（热键触发 + 托盘常驻）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/lib.rs` | global-shortcut 注册、窗口 show/focus/emit、失焦隐藏、setup |
| `src-tauri/src/tray.rs` | 系统托盘菜单构建与事件回调 |
| `src-tauri/src/window_pos.rs` | 定位计算 + 单测 |
| `src-tauri/tauri.conf.json` | 预热窗口 visible:false + trayIcon 配置 |
| `src-tauri/capabilities/default.json` | global-shortcut 权限声明 |
| `src/App.tsx` | route 事件监听 + Esc 隐藏 |

## 总结论

**未过。** 发现 4 个 Important 级问题，无 Critical。其中 I-02（listen 竞态窗口）为前端规范强制要求，I-04（恒真断言）损害验收证据可信度，需修复后复审。H-01/H-02 为运行期人工确认项（并入 pending-manual，不打回）。

## 问题清单（Important）

### I-01 · `register_hotkeys` 返回类型与实际语义不符
- 文件：`src-tauri/src/lib.rs:53`　置信度 80
- 描述：签名返回 `Result<(), tauri::Error>`，但所有失败路径均被内部 `if let Err` 消化，函数始终返回 `Ok(())`，调用方 `.setup` 用 `?` 传播实为误导，违背类型诚实。
- 修复：改签名为返回 `()` 并在 setup 直接调用；或文档明确"内部消化，永不向上传播"。

### I-02 · `App.tsx` listen/unlisten 竞态窗口
- 文件：`src/App.tsx:14-26`　置信度 82
- 描述：`listen()` 返回 Promise，`unlisten` 异步赋值；若组件在 resolve 前卸载，cleanup 时 `unlisten` 为 undefined，监听器泄漏。前端规范要求成对清理。
- 修复：引入 `cancelled` flag，在 `.then()` 内判断已卸载则立即调用返回的 unlisten。

### I-03 · `window_pos.rs:28` `available_monitors()` 失败无日志
- 文件：`src-tauri/src/window_pos.rs:28`　置信度 80
- 描述：`unwrap_or_default()` 静默吞错；回退逻辑正确但与项目统一 `eprintln!` 风格不一致，平台异常无调试信号。
- 修复：`unwrap_or_else(|e| { eprintln!("[QuickQuick] 获取显示器列表失败，回退主显示器: {e}"); vec![] })`。

### I-04 · `lib.rs:127-131` smoke 测试为恒真断言
- 文件：`src-tauri/src/lib.rs:127-131`　置信度 85
- 描述：`assert_eq!(2 + 2, 4)` 恒真、零覆盖，名义充当 A04 后端证据但无实质。
- 修复：替换为对 `HotkeyConfig::default()` 两字段非空、或 `compute_window_position` 可测片段的实质断言（保留一例真实后端单测以续作 V0-F1-A04 后端侧）。

## 人工确认项（不打回，并入 pending-manual）

- **H-01** 失焦隐藏与托盘点击时序：托盘点击 `show+set_focus` 与 `on_window_event(Focused(false)→hide)` 可能在 focus 瞬态触发一次 hide 致窗口闪现即消。需运行期点击托盘验证窗口稳定可见。
- **H-02** 高 DPI 定位偏移：`cursor_position()` 逻辑坐标 vs `monitor.position/size()` 物理坐标混用，2x Retina 屏可能定位偏移。单测仅覆盖 1x。需 Retina 屏运行期验证落点在屏幕上中。

## 通过项确认

无裸 `unwrap()/panic!`；热键正确区分 history/translate；失焦借用安全（`window.clone()` 解 E0505）；Esc 监听成对清理；payload 类型收窄（`listen<RoutePayload>`，无 any）；windowRoute 测试未破坏；capabilities 权限最小化；无 TODO/FIXME；设计§八符合（单窗路由/活动屏上中/失焦即隐）；window_pos 单测三例非恒真。

## 结论

**打回。** 修复 I-01 ~ I-04 并 `cargo clippy -D warnings` + `pnpm test` 通过后复审；H-01/H-02 并入 pending-manual。

---

## 复审结论（2026-05-31）

**status: 通过**

逐条核查 I-01~I-04 全部已修复：
- **I-01** `register_hotkeys` 改返回 `()`、setup 直调、注释明确"内部消化不向上传播"，类型诚实问题消除。
- **I-02** `App.tsx` 引入 `cancelled` flag，`.then()` 内已卸载即调 `fn()` 释放，cleanup 置 flag + `unlisten?.()`，竞态泄漏关闭。
- **I-03** `available_monitors()` 失败改 `unwrap_or_else` + `eprintln!`，日志风格统一。
- **I-04** 恒真断言替换为 `lib_default_hotkey_config_sane`/`hotkey_defaults_match_spec` 两实质测试，对 default 两字段强断言，A04 后端证据真实有效。
未破坏既有通过项，无新引入≥80 高危。H-01/H-02 仍为 pending-manual 人工项，不影响审查通过。
