---
id: f3-popover-s01-batch-a-rust
title: 里程碑4 popover · Batch A1：Rust 骨架
status: 实现完成
commit: fb6a690
date: 2026-06-02
---

## 改动文件

- `src-tauri/src/window_pos.rs`：`center_top_position` 增加 `width` 参数；新增 `compute_window_position_for_width` 公开函数；保留 `compute_window_position` 为 wrapper（加 `#[allow(dead_code)]` 标注为保留 API）；现有测试补 `WINDOW_WIDTH` 参数；新增 3 个测试覆盖 width=720/320 用例。
- `src-tauri/src/popover.rs`（新建）：封装 clip-popover（720×480）和 trans-popover（320×200）的懒建/定位/显示/失焦隐藏；用 `PopoverSpec` 结构体消除两份重复配置；`trigger_popover` / `get_or_create_popover` / `register_focus_lost_hide` 三层职责分离。
- `src-tauri/src/lib.rs`：新增 `mod popover;`；热键回调从 `trigger_window` 改为 `popover::trigger_popover`；删除死函数 `trigger_window`；移除不再使用的 `Emitter` import。

## TDD 红绿记录

RED：先在 `window_pos.rs` 测试里写调用 5 参数版 `center_top_position` 的 3 个新测试，`cargo test window_pos` 因编译错误（参数数量不符）失败确认红。GREEN：给 `center_top_position` 加 `width` 参数、抽出 `compute_window_position_for_width`、修复旧测试参数，全 7 个测试通过。

## 测试结果

```
test result: ok. 67 passed; 0 failed（lib）
全套：全部通过，exit=0
window_pos 模块：7 passed（含 3 个新增 width 用例）
```

## 偏离架构的地方（均已在 Batch A 内闭合修正）

- `.transparent(true)` 一度未启用（缺 `macos-private-api` feature）→ **已补回**：启用 `macos-private-api` cargo feature + `tauri.conf.json` `macOSPrivateApi: true` + 恢复 `.transparent(true)`，`cargo check`/`cargo test` 全绿。毛玻璃透明窗口为架构核心，不降级。
- 窗口 url 一度为 `clip-popover/index.html` → **A2 改为 `src/clip-popover/index.html`**（Vite MPA 入口在 `src/` 下时 dev/prod 路径均带 `src/`，须一致）。

## 衔接提示（给 tester）

- popover 窗口失焦即自动 hide，前端无需额外处理关闭逻辑。
- 本批可单测的核心逻辑是 `window_pos::center_top_position` 的宽度参数化；popover 窗口的懒建/定位/失焦/毛玻璃属 GUI 行为，需手动验证（pnpm tauri dev）。
