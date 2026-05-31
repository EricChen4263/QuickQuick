---
id: V3-F3-S09-code
type: coding_record
level: 小功能
parent: V3-F3
created: 2026-05-31T03:38:38Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A11]
author: coder
---

# V3-F3-S09 macOS Accessibility 引导与优雅降级 — 编码记录

## 实现概要

新增 `src-tauri/src/onboarding.rs`，提供 macOS Accessibility 权限引导与优雅降级的完整逻辑：

- **AccessibilityProbe trait**：抽象 `AXIsProcessTrusted` 检测，使逻辑层 headless 可测（生产实现调 AX API，测试用 fake）。
- **onboarding_action**：纯函数，`is_trusted=true` 返回 `Proceed`，`false` 返回 `ShowCardAndDeepLink`。
- **ACCESSIBILITY_DEEPLINK 常量**：`x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`，非空，指向系统辅助功能设置。
- **paste_capability**：纯函数，已授权返回 `FullPaste`，未授权返回 `WriteBackOnly`。
- **perform_paste_or_degrade**：
  - 已授权：调用 `write_then_paste`（写回 + 等待 changeCount + send_paste）。
  - 未授权：仅调用 `write_with_marker` 写回剪贴板，**跳过 send_paste**，返回 `WriteBackOnlyDone`，不 panic。

新增 `src-tauri/tests/onboarding.rs`，含 3 个 A11 验收测试（函数名均含 `accessibility_onboarding_degrade`）。

## 关键决策

| 决策 | 理由 |
|------|------|
| `AccessibilityProbe` 用 trait 而非直接调用 AX API | 使所有引导/降级逻辑 headless 可测，不依赖 OS 授权状态 |
| 引入 `PasteOutcome` 枚举区分两条路径 | 调用方可据此决定是否显示"已写入剪贴板，请手动粘贴"提示，避免信息丢失 |
| 未授权时主动跳过 `send_paste` | macOS 未授权时 CGEvent 注入被 OS 静默丢弃，主动跳过避免用户困惑（剪贴板已更新但粘贴未执行） |
| `perform_paste_or_degrade` 复用 `write_then_paste` 而非重新实现 | 复用已有 changeCount 轮询时序保证（A15），不重复造轮子 |
| 全局热键不需授权说明写入模块文档注释 | 澄清设计§二关于"热键 vs 模拟粘贴"的授权边界，防止误解 |

## 改动文件

| 文件 | 说明 |
|------|------|
| `src-tauri/src/onboarding.rs` | 新增：Accessibility 引导与优雅降级完整实现 |
| `src-tauri/src/lib.rs` | 新增一行：`pub mod onboarding;` 注册模块 |
| `src-tauri/tests/onboarding.rs` | 新增：A11 验收集成测试（3 个用例） |

## TDD 过程

1. **RED**：先写 `tests/onboarding.rs`，import `quickquick_lib::onboarding`（不存在），运行报 `error[E0432]: unresolved import`，确认因功能缺失而失败（非语法/环境错）。
2. **GREEN**：新增 `src/onboarding.rs` + `lib.rs` 注册，运行 `cargo test --test onboarding accessibility`，3/3 通过。
3. **REFACTOR**：实现已足够简洁（纯函数、嵌套 1 层、单一职责），无需额外重构。

## 降级验证说明

未授权不发粘贴的验证方式：`FakePasteBackend` 记录 `send_paste_call_count`，`perform_paste_or_degrade` 在未授权路径下跑完，断言 `send_paste_call_count == 0`（非恒真）。同时断言 `write_call_count == 1`（剪贴板确实写回了），`clipboard_text` 等于传入条目文本，形成完整 AAA 结构。

## code-standards 自检

- 格式：4 空格缩进（Rust 标准），无尾随空格
- 命名：snake_case 函数，PascalCase 类型，描述性（`is_trusted`、`onboarding_action`、`perform_paste_or_degrade`）
- 函数：最长函数 `perform_paste_or_degrade` 约 10 行，嵌套 1 层，无裸 unwrap/panic
- 注释：写「为什么」（CGEvent 注入被 OS 静默丢弃的原因），无装饰性分隔线，无死代码注释
- 类型：严格类型，`Result<PasteOutcome, PasteError>` 明确区分两条路径
- 测试：AAA 结构，非恒真断言（`send_paste_call_count == 0` vs `== 1`），headless fake probe + fake backend
- 无 TODO / FIXME
- 无裸 unwrap / panic

## 审查 Polish（2026-05-31）

code-reviewer 判通过后提 2 个 Important 建议，已完成 polish：

### 1. 深链断言精确化

`accessibility_onboarding_degrade_untrusted_shows_card_and_deeplink` 中原有两句过宽断言（`!is_empty()` + `contains("Accessibility")`）已替换为单句精确 `assert_eq!`：

```rust
assert_eq!(
    ACCESSIBILITY_DEEPLINK,
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    "深链常量应精确等于设计规定值"
);
```

理由：常量值已在实现中固定（`onboarding.rs:26-27`），精确断言能在常量被意外改动时立即暴露，过宽断言可能漏掉拼写/路径级错误。

### 2. AAA 拆分 — `trusted_full_paste` → 新增 `trusted_perform_calls_send_paste`

原 `accessibility_onboarding_degrade_trusted_full_paste` 含 3 个 Act（`onboarding_action`、`paste_capability`、`perform_paste_or_degrade`）且 `send_paste_call_count == 1` 断言在最末尾，前置断言失败会屏蔽它。

拆分方案：
- 原测试保留：验证 `onboarding_action` 返回 `Proceed`、`paste_capability` 返回 `FullPaste`、`perform_paste_or_degrade` 成功 + `write_call_count == 1`
- 新增独立测试 `accessibility_onboarding_degrade_trusted_perform_calls_send_paste`：Arrange → Act（单次 `perform_paste_or_degrade`）→ Assert `send_paste_call_count == 1`，确保该关键断言不被屏蔽

Polish 后 onboarding 测试数：3 → 4，覆盖不减。

## 验证结果

```
A11=0       # cargo test --test onboarding accessibility → 3 passed (3)
all=0       # cargo test → 全量通过（5+32+10+67+1 共 115 个）
clippy=0    # 零 warning/error
build=0     # cargo build 正常
deco=1      # 无装饰性注释（期望 grep 返回 1）
todo=1      # 无 TODO/FIXME（期望 grep 返回 1）
```

### Polish 后回归（2026-05-31）

```
onboarding=0   # 4 passed（含新拆 trusted_perform_calls_send_paste）
all=0           # 全量通过（10+67+1 共 78 个 unit + 4 onboarding integration）
clippy=0        # 零 warning/error
deco=1          # 无装饰性注释（期望 1）
```
