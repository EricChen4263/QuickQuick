---
id: V5-F4-S04-review
type: review
level: 小功能
parent: V5-F4
children: []
created: 2026-06-02T00:00:00Z
status: 通过
commit: 001ccd8
acceptance_ids: [V5-F4-S04-9a, V5-F4-S04-9b]
author: code-reviewer
---

# 审查记录 · V5-F4-S04 真实自动粘贴（9a+9b）规范审查

## 审查范围

`src-tauri/src/macos_paste.rs`（新文件）+ `src-tauri/src/ipc/system.rs`（大改）+
`src-tauri/Cargo.toml` + `src-tauri/src/lib.rs` + `src-tauri/tests/macos_backends.rs`

diff 区间：56634d7..143ff71

依据：code-standards + 项目规范（禁死代码、禁 panic 生产路径、函数≤50行、A15 changeCount 时序保证）

---

## 重要问题（Important）

### [I-01] macOS trusted 路径绕过 A15 changeCount 轮询（置信度 90）

**位置**：`src-tauri/src/ipc/system.rs` 第270–285行，`run_paste_with_backend` macOS 分支

**描述**：macOS 分支直接执行 `backend.write_with_marker(item)` → `hide_panel_and_wait(app)` → `backend.send_paste()`，跳过了 `paste_orchestrate` / `perform_paste_or_degrade` / `write_then_paste` 中的 changeCount 轮询（`wait_for_count_increase`）。

`write_then_paste`（`paste.rs` 第103–115行）的设计保证（A15）是：写入后轮询 `change_count` 直到 > 写前值，确认 OS 已接受写入，才调用 `send_paste`，否则返回 `Timeout`，不盲发粘贴。

macOS 生产路径绕过这一保证，在写入尚未被 OS 接受时就发 Cmd+V，会粘贴到旧内容。非 macOS Fallback 分支调用 `paste_orchestrate` → `perform_paste_or_degrade` → `write_then_paste`，有完整保证；macOS 生产分支反而没有，形成逻辑不一致。

注意：`paste_orchestrate` 中的 T8/T9/T10 单测验证的是 Fallback 路径逻辑，并不测试 macOS 生产路径。

**修复方向**：macOS 分支应让 `MacOsPasteBackend` 经过 `paste_orchestrate`（或直接 `perform_paste_or_degrade`），
在 trusted=true 的 `write_then_paste` 内轮询 changeCount 后再 `send_paste`，
窗口 hide 可在轮询成功后、`send_paste` 前插入，或接受"先 hide 再等 changeCount"的顺序（后者可能略增延时但功能正确）。

---

### [I-02] `paste_to_front_impl` 是真实死代码，公开导出无调用路径（置信度 95）

**位置**：`src-tauri/src/ipc/system.rs` 第160–178行

**描述**：9b 重构后 `paste_to_front_impl` 函数保留，注释说"供测试复用（传入 fake probe/backend）"，但实际：
- 不在任何命令路径中被调用（`paste_to_front` 命令走 `fetch_paste_item` + `run_paste_with_backend`）
- 不在任何 `#[cfg(test)]` 测试块中被调用（单测直接调 `paste_orchestrate`）
- 没有 `#[cfg(test)]` 保护，没有 `#[allow(dead_code)]`
- 函数体使用 `arboard::Clipboard::new()` 原始构造（而非 fake backend），无法接受注入的 fake probe/backend，注释描述与实现不符

`pub fn` 声明使其进入公开 API，构成误导性暴露。项目规范禁止死代码留存。

**修复方向**：删除 `paste_to_front_impl`，或若需保留一个 DB 层的测试辅助入口，改为接受 probe/backend 参数并加 `#[cfg(test)]` 保护。

---

### [I-03] `MacOsPasteBackend::new()` 和 `FallbackPasteBackend::new()` 在命令调用路径中 panic（置信度 85）

**位置**：
- `src-tauri/src/macos_paste.rs` 第77–81行（`MacOsPasteBackend::new`）
- `src-tauri/src/macos_paste.rs` 第182–189行（`FallbackPasteBackend::new`）

**描述**：两个 `new()` 都使用 `.expect(...)` 处理 `arboard::Clipboard::new()` 失败。调用路径为 `paste_to_front` 命令 → `run_paste_with_backend` → `MacOsPasteBackend::new()`，即每次用户触发粘贴命令都会执行这段代码。

- 注释说"调用方（lib.rs setup）应在 GUI 环境下调用"，但实际调用位置是命令处理器，不是 setup 阶段。
- GUI 进程剪贴板偶发初始化失败（如 macOS 剪贴板服务短暂不可用）会导致整个 Tauri 进程 panic（崩溃），而非优雅降级。
- 应改为 `Result` 返回并在 `run_paste_with_backend` 中 map_err 到 `eprintln` + 返回 "write_back_only"，保持一致的降级语义。

**修复方向**：`MacOsPasteBackend::new() -> Result<Self, String>`；在 `run_paste_with_backend` 中处理 `Err`：打 `eprintln` 日志，返回 `"write_back_only".to_string()`。

---

## 低优先级 / 建议（置信度 < 80，仅供参考）

### [L-01] `objc2-foundation` 依赖声明但在 `macos_paste.rs` 中未使用（置信度 75）

**位置**：`src-tauri/Cargo.toml` 第48行

`objc2-foundation = { version = "0.3", features = ["NSString"] }` 出现在 macOS target 依赖中，但 `macos_paste.rs` 没有任何 `use objc2_foundation::` 语句，只用了 `objc2_app_kit::NSPasteboard`（NSString 类型在 0.3.x 版本中可能由 objc2-app-kit 间接引入）。若编译器未发出 unused dependency 警告（Rust 1.x 默认不警告），则该依赖属冗余，可视 `cargo build` 输出决定是否清理。

### [L-02] `ax_is_process_trusted` 注释"无副作用"措辞略不准确（置信度 40）

`AXIsProcessTrusted` 在 macOS 中如果同时传入 `options` 参数并指定 `kAXTrustedCheckOptionPrompt=true` 时会触发系统授权弹窗，本代码直接调用无参版本（不传 options），无副作用描述对此签名是正确的。不构成问题，无需修改。

### [L-03] `hide_panel_and_wait` 100ms sleep 阻塞 Tauri 命令线程已在注释中说明（置信度 30）

模块顶部文档注释已写"已知限制：务实焦点方案依赖 macOS 在窗口 hide 后自动把焦点还给上一个 App"，sleep 100ms 阻塞可接受（命令线程非 UI 线程），不构成需强制修复的问题。

---

## FFI 安全性核查（macos_paste.rs）

| 项目 | 结论 |
|------|------|
| `#[link(name="ApplicationServices", kind="framework")]` 声明 | 正确，应链接 ApplicationServices framework |
| `AXIsProcessTrusted() -> bool` ABI | 在 macOS Clang ABI 下 C `bool` 与 Rust `bool` 均为 1 字节，实测编译通过；Apple SDK 实际头文件为 `Boolean`（`unsigned char`），用 `bool` 接收在此平台可行（编译器已验证） |
| 安全包装层 `ax_is_process_trusted()` | 正确，unsafe 块最小化 |
| `NSPasteboard::generalPasteboard()` / `changeCount()` 安全性 | objc2-app-kit 0.3.2 已通过 `extern_methods!` 宏将其包装为 safe fn（源码第326、342行为 `pub fn`），调用方无需 unsafe 块，代码正确 |
| NSInteger→u64 转换 `n.max(0) as u64` | 正确，64-bit macOS NSInteger 为 i64，防御性处理负值 |
| CGEvent keyDown/keyUp 配对 | 第147–149行：仅 keyDown 成功时才发 keyUp，配对正确 |
| CGEventSource 失败处理 | 第124–128行：`Err(())` 时 eprintln + return，不 panic |
| CGEvent 创建失败处理 | 第135–139行：eprintln + return false，不 panic |
| cfg 分流全平台可编译 | macOS/非macOS 分支完整，非 mac Fallback 降级语义正确（is_trusted=false，send_paste no-op） |
| 装饰性横线分隔注释 | 未发现 |

---

## 结论

**status = 未过**

必须修复再提复审：

1. **[I-01]（高）macOS trusted 路径绕过 A15 changeCount 轮询**——可能在写入未生效时粘贴旧内容，功能正确性缺陷
2. **[I-02]（中）`paste_to_front_impl` 死代码**——公开暴露、注释描述与实现不符，违反禁死代码规范
3. **[I-03]（中）`MacOsPasteBackend::new()` 在命令路径中 expect panic**——应改为 Result 返回并降级

FFI 安全包装、cfg 分流、非 mac 降级、CGEvent 事件配对均无问题。

---

## 复审（commit 001ccd8）

高危收口修复 commit 为 `001ccd8`（fix(paste): 自动粘贴 review 高危收口）；原始实现 commit 为 `143ff71`/`de538d9`。

| 初审问题 | 修复（已核实位置 文件:行号） |
|---|---|
| I-01：macOS trusted 路径绕过 A15 changeCount 轮询，可能在写入未生效时盲发 Cmd+V | 已修复。`src-tauri/src/ipc/system.rs` 约 266-282 行：trusted 分支改为 `paste::write_and_confirm(&mut backend, item)` 确认写入（内部调用 `wait_for_count_increase` 轮询 changeCount，超时返回 Err）→ 成功后 `hide_panel_and_wait(app)` → `backend.send_paste()`；Err 时返回 `"write_back_only".to_string()`，不盲发。`paste.rs` 新增 `write_and_confirm` 函数（约 92-107 行），模块注释明确描述 A15 保证。 |
| I-02：paste_to_front_impl 死代码，pub 暴露且注释描述与实现不符 | 已修复。`src-tauri/src/ipc/system.rs` 中 `paste_to_front_impl` 已删除（grep 无输出确认）。 |
| I-03：MacOsPasteBackend::new() 及 FallbackPasteBackend::new() 在命令路径中 expect panic | 已修复。`src-tauri/src/macos_paste.rs` 约 78、187 行：两个 `new()` 均改为 `Result<Self, String>` 返回；`run_paste_with_backend`（system.rs 约 258-265 行）对 `Err` 走 `eprintln` + 返回 `"write_back_only".to_string()`，优雅降级。 |
| L-01（建议）：objc2-foundation 冗余依赖 | 已处理。commit 001ccd8 的 Cargo.toml diff 中删除了 `objc2-foundation` 依赖。 |

终态：通过
