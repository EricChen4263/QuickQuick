---
id: V5-F4-S15-review
type: review
level: 小功能
parent: V5-F4
created: 2026-06-03T00:00:00Z
status: 通过
commit: ad8cc62
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 热键运行时即时生效修复（V5-F4-S15）

## 审查范围

- `src-tauri/src/lib.rs`：新增纯函数 `popover_label_for_action` + 可复用函数 `register_action_shortcut`；`register_hotkeys` 重构为循环复用；新增两条单元测试。
- `src-tauri/src/ipc/settings.rs`：`set_hotkey` 命令扩展为「读旧键 → 持久化 → unregister 旧键 → register 新键」完整流程。

参照：项目规范、code-standards（code-general + Rust）。

---

## 问题清单

### Critical（高危，阻断放行）

无。

---

### Important（中优先级）

无达到报告门槛（置信度 ≥ 80）的问题。

---

### Low（低优先级）

无达到报告门槛（置信度 ≥ 80）的问题。

---

## 逐维度核查

### 1. 接线无漂移

**结论：等价重构，行为与原实现一致。**

`popover_label_for_action` 返回值：`History → "clip-popover"`、`Translate → "trans-popover"`。
与 `src-tauri/capabilities/default.json` 中的 `"windows": ["main", "clip-popover", "trans-popover"]` 完全一致，无 label 拼写错位。

`register_hotkeys` 改为循环后，`History` / `Translate` 两个动作的注册顺序与原双 `if let Err` 块相同（数组字面量保序），回调逻辑通过 `register_action_shortcut` 委托，行为等价。启动期失败仍仅 `eprintln!` 降级，不 panic，不向上传播，保持原设计语义。

### 2. set_hotkey 命令正确性

**结论：流程正确，borrow/move 无问题。**

`app: AppHandle` 为值类型，`&app` 多次 immutable 借用（`resolve_config_path`、`SystemHotkeyRegistrar`、`app.global_shortcut()`、`register_action_shortcut`）在同一作用域内合法，Rust borrow 规则满足。

`old != accelerator` 守卫防止自注销（相同键时跳过 `unregister`）。错误映射：持久化失败提前 `?` 返回，运行时注册失败映射为含"持久化已完成，重启后生效"提示的字符串，前端可据此给用户合理反馈。

### 3. 次序稳健性（「先 unregister 再 register」）

**结论：可接受（已知边界），不建议强制修改。**

当前次序：`unregister(old)` → `register_action_shortcut(new)`。
若 `register` 失败，旧键已注销，该动作两键皆失效至重启。

评估：
- `on_shortcut` 调用底层 `m.0.register(shortcut)` 注册的是全新的 OS 快捷键（`new != old` 守卫已保证），全新键在正常 OS 状态下注册不应失败。失败场景仅限 OS 层异常（系统崩溃/快捷键服务不可用），属极低概率事件。
- 即使失败，持久化已完成（新键写入 hotkey.json），重启后新键自动生效，`register_hotkeys` 启动期会读文件重新注册。
- 错误已通过返回值反馈前端，用户可感知并决策。

「先 register(new) 再 unregister(old)」确实更健壮（避免短暂双键并存以外的失效窗口），但考虑到失败概率极低、持久化兜底、重启兜底均到位，现有实现的风险敞口处于可接受范围。**建议以注释形式记录此已知边界**，但不作为本次必改项。

### 4. 冲突检测交互（re-save 相同键）

**结论：既有行为，本次未引入新问题。**

`SystemHotkeyRegistrar::register` 调用 `is_registered(accelerator)`，检查的是 plugin 内部 HashMap（非 OS 层）。该行为在本次改动之前就已存在，diff 未触碰 `SystemHotkeyRegistrar` 实现。

re-save 相同键时：`is_registered(same_key)` 返回 true → `set_hotkey_impl` 返回 `AlreadyInUse` → `?` 处函数提前返回，后续 unregister/register 均不执行。这意味着 re-save 相同键在前端会收到"已被占用"错误——此为既有问题，不是本次引入的回归。`old != accelerator` 守卫位于 `set_hotkey_impl` 成功之后，无法到达；该行为路径由 `set_hotkey_impl` 提前截断。

### 5. 规范符合性

| 检查项 | 结论 |
|--------|------|
| 函数 ≤ 50 行 | 合规（`register_action_shortcut` 11 行，`set_hotkey` 含注释约 30 行，`register_hotkeys` 约 25 行） |
| 嵌套 ≤ 3 层 | 合规（最深 for → if let Err，2 层） |
| 注释写「为什么」 | 合规（`set_hotkey` 函数头注释说明流程与各步语义；`register_action_shortcut` 注释说明共用目的与调用方策略差异） |
| 无魔术值 | 合规（label 字面量集中在 `popover_label_for_action` 一处，与 capabilities 配置对应，注释说明必须一致） |
| 纯函数可测性 | 合规（`popover_label_for_action` 无副作用，已有两条单元测试覆盖两个分支） |
| 测试命名行为化 | 合规（`popover_label_for_history_action_returns_clip_popover` 等）|
| 测试 AAA 结构 | 合规 |
| 无装饰性分隔注释 | 合规 |
| `pub` 可见性 | `popover_label_for_action` 和 `register_action_shortcut` 均为 `pub`，供 `ipc/settings.rs` 跨模块调用，合理 |

---

## 总结论

**通过（放行）。**

本次修复目标明确（改热键后不生效需重启），实现质量高：

1. **重构正确**：`popover_label_for_action` + `register_action_shortcut` 抽取干净，`register_hotkeys` 循环行为等价，DRY 达成，label 字面量与前端 capabilities 一致。

2. **set_hotkey 流程完整**：「读旧键 → 持久化 → unregister 旧键 → register 新键」次序合理，borrow/move 无问题，错误映射完整。

3. **稳健性边界已知可接受**：「先 unregister 再 register」次序在极低概率 OS 异常下存在双键失效窗口，但有持久化+重启双重兜底，风险敞口可接受；建议补注释记录此已知边界（非必改）。

4. **冲突检测 re-save 问题属既有行为**，本次未引入回归。

无高危、无中优必改项，可直接提交。
