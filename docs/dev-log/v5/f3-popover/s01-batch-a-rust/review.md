---
id: V5-F3-S01-review
type: review
level: 小功能
parent: V5-F3
children: []
created: 2026-06-02T00:00:00Z
status: 审查通过
commit: fb6a690
acceptance_ids: []
evidence: []
author: code-reviewer
---

# 审查记录 · V5-F3-S01 里程碑4 Popover · Batch A 骨架

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/popover.rs` | 新增：popover 懒建 / 触发 / 失焦隐藏模块 |
| `src-tauri/src/lib.rs` | 热键改指向 popover，删除旧 trigger_window |
| `src-tauri/src/window_pos.rs` | 新增 compute_window_position_for_width + wrapper + 新单测 |
| `src-tauri/Cargo.toml` | 新增 macos-private-api feature |
| `src-tauri/tauri.conf.json` | 新增 macOSPrivateApi: true |
| `vite.config.ts` | 新增两个 popover 多入口 |
| `src/clip-popover/*` | 新增：ClipPopoverApp / main.tsx / popover.css / index.html |
| `src/trans-popover/*` | 新增：TransPopoverApp / main.tsx / trans-popover.css / index.html |

## 总结论

**审查通过（无未决高危）。** 发现 1 个低优先级问题 + 2 条建议，均不阻断功能，无需打回。

---

## 问题清单

### 低优先级问题

#### L-01 · `compute_window_position` 无真实调用方，`#[allow(dead_code)]` 掩盖信号

- 文件：`src-tauri/src/window_pos.rs:23`　置信度 80
- 描述：该函数在本次提交后，整个 `src-tauri/src/` 内唯一用它的地方是其自身定义；`tray.rs` 也不调用它。`#[allow(dead_code)]` 注释说明"供 tray / dock 触发逻辑调用"，但 tray.rs 当前并未调用。Rust 编译器会为此发出 dead_code 警告，`#[allow]` 直接压制。如果未来 tray 确实要用，属正常预留；但若该函数是历史遗产且无未来计划，应删除而非 allow。
- 现状：功能无影响，纯代码质量问题。
- 建议：若 Batch B/C 确认 tray 不再用此 wrapper，删除函数并移除 allow；若确有复用计划，在注释里明确"Batch X 将由 tray 调用"，保留合理。

---

## 建议（不计入必改）

#### S-01 · lib.rs 模块级文档注释缺少 `popover` 子模块条目

- 文件：`src-tauri/src/lib.rs:1-13`　置信度 75
- 描述：`popover` 模块已在 L28 声明并在热键处被调用，但模块顶部的子模块列表（`hotkey / ipc / pipeline / ...`）未列出 `popover`，文档与实现轻微脱节。
- 建议：在子模块列表末尾补一行 `/// - popover：popover 浮层窗口懒建、触发与失焦隐藏`。

#### S-02 · 两个 popover 的 CSS 文件命名不对称

- 文件：`src/clip-popover/popover.css`、`src/trans-popover/trans-popover.css`
- 描述：clip-popover 侧 CSS 叫 `popover.css`，trans-popover 侧叫 `trans-popover.css`，命名风格不统一（应同为 `popover.css` 或同为 `clip-popover.css` / `trans-popover.css`）。功能无影响，纯命名一致性。
- 建议：统一为各自目录内 `popover.css`（与 clip-popover 侧保持一致）。

---

## 通过项确认

| 审查点 | 结论 |
|---|---|
| `get_or_create` 已存在路径不重复 build/不重复注册监听器 | 通过：复用时 `return Some(existing)` 直接返回，`register_focus_lost_hide` 仅在 `Ok(w)` 分支首次建窗时调用，无泄漏 |
| 未知 label 降级完整 | 通过：`let Some(spec) = ... else { eprintln!; return; }`，不 panic |
| 懒建失败降级完整 | 通过：`Err(e)` 分支 `eprintln!` + `None`，调用方链 `let Some(window) = ... else { return; }` |
| `macos-private-api` 三件套一致 | 通过：Cargo.toml feature + tauri.conf.json macOSPrivateApi:true + WebviewWindowBuilder.transparent(true) 齐全；main 窗口未设 transparent（默认 false） |
| `compute_window_position_for_width` 保持原 main 窗口行为 | 通过：wrapper 仍传 WINDOW_WIDTH=400，语义等价 |
| `trigger_window` 删除干净 | 通过：lib.rs 无残留引用，`Emitter` import 已清除，无死代码 |
| 热键 → popover 映射正确 | 通过：history→clip-popover / translate→trans-popover，与 POPOVER_SPECS label 一致 |
| vite.config.ts 多入口含 main | 通过：`main: resolve(__dirname, "index.html")` 仍在 input 中 |
| popover URL 路径与 dist 结构一致 | 通过：WebviewUrl::App("src/clip-popover/index.html") 在生产下指向 dist/src/clip-popover/index.html，与 rollup 输出一致；开发模式下 Vite 从项目根提供 /src/clip-popover/index.html |
| Esc→hide 实现 | 通过：两个 main.tsx 均监听 keydown Escape，调用 getCurrentWindow().hide()，失败 console.warn |
| popover.css 透明毛玻璃 | 通过：html/body background:transparent + #root backdrop-filter + var(--glass) |
| subscribe 内存泄漏风险 | 可接受：subscribe 在模块顶层调用，生命周期等同于 webview 进程，不 unsub 是 intentional；无明显错误 |
| 无硬编码密钥 / console.log 噪音 | 通过：无 console.log，console.warn 用于 hide 失败降级，合规 |
| 无裸 unwrap / panic | 通过：所有可失败操作均 if let Err / match + eprintln 或 unwrap_or_else |
| 函数长度 / 嵌套 / 命名 | 通过：最长函数 get_or_create_popover 约 30 行，嵌套 ≤2 层，命名清晰 |
| 新增单测覆盖新参数分支 | 通过：3 个新单测验证 width=720/320 的 x 坐标及 y 与 width 无关 |
