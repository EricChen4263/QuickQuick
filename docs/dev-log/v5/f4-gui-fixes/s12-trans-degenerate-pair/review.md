---
id: V5-F4-S12-review
type: review
level: 小功能
parent: V5-F4
created: 2026-06-03T00:00:00Z
status: 通过
commit: pending
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 翻译同语种退化对守卫（V5-F4-S12）

## 审查范围

- `src-tauri/src/translate/lang.rs`：`resolve_direction` 与 `resolve_direction_with_source` 新增同语种退化对守卫；新增 6 个单测。

参照：项目规范、code-standards（code-general + Rust）、既有 lang.rs 约定。

---

## 问题清单

### Critical（高危，阻断放行）

无。

---

### Important（中优先级，建议修但不阻塞放行）

**[I-01] `resolve_direction` 与 `resolve_direction_with_source` 守卫逻辑重复，且前者在生产路径中已无调用（置信度 82）**

- 位置：`lang.rs:76-79`（resolve_direction 守卫）、`lang.rs:116-119`（resolve_direction_with_source 守卫）
- 问题：两处守卫 `match configured_target { Some(t) if t.as_str() != source.as_str() => t, _ => default_target }` 逻辑完全相同，独立复制维护。检索全仓库，`resolve_direction`（非 `_with_source` 变体）**在生产代码中零调用**——唯一调用方是 `tests/translate.rs` 的集成测试（及本次新增的 `lang_norm` 单测）；生产唯一路径为 `translate_text_impl → resolve_direction_with_source`。
- 规范依据：code-general DRY 原则；函数并行维护意味着未来若守卫逻辑需要修改（如支持 locale 变体规范化），两处必须同步，存在遗漏风险。
- 建议（不阻塞当前放行）：将 `resolve_direction` 改为委托实现：
  ```rust
  pub fn resolve_direction(text: &str, configured_target: Option<Lang>) -> (Lang, Lang) {
      resolve_direction_with_source(text, None, configured_target)
  }
  ```
  这样守卫逻辑集中于 `resolve_direction_with_source`，`resolve_direction` 退化为薄包装，消除重复，集成测试语义不变（`configured_source=None` 走检测路径，行为完全等价）。
- 说明：coder 报告称 `resolve_direction` 仅测试用，但函数为 `pub`，未加 `#[cfg(test)]` 或 `#[deprecated]` 标注，未来可能被误用，建议同步标注或重构。

---

### Low（低优先级，不阻塞放行）

无达到报告阈值（置信度 ≥ 80）的低优问题。

以下为置信度未达标（< 80）的观察，仅供参考：

- **变体边界（置信度 65）**：若前端将来传入 `target="zh-CN"` 而检测源语为 `"zh"`，两者字面量不等（`"zh-CN" != "zh"`），守卫不介入；但 `map_lang_for_provider("mymemory", ...)` 中 `normalize_zh_variant` 会把两者均归一为 `"zh-CN"`，实际 langpair = `"zh-CN|zh-CN"` 仍是退化对。当前前端下拉框只传 `"zh"` 或 `"auto"`，此路径现实中不会触发，属已知边界而非当前需修复的缺陷。若未来前端扩展 locale 选项（如区分简繁）需同步处理。

---

## 逐维度核查

### 1. 退化对消除不变量（第 1 点）

**结论：守卫后 target 恒≠source，不变量成立。**

`default_target` 的构建逻辑为：

```rust
let default_target = if source.as_str() == "zh" {
    Lang::new("en")   // "en" != "zh"
} else {
    Lang::new("zh")   // "zh" != source（source 此时为非 "zh" 的任意值）
};
```

数学归纳：
- 若 source = "zh"，则 default_target = "en"，"en" != "zh"，恒成立。
- 若 source = 任意非 "zh" 值（如 "en"、"ja"、"fr" 等），则 default_target = "zh"，"zh" != source，恒成立（source 不可能同时是 "zh" 和非 "zh"）。

守卫触发时（`configured_target` 缺失或与 source 相等）用 `default_target` 替代，而 default_target 恒≠source。不变量在所有 source 取值下均成立。

---

### 2. 合法跨语种对不受影响

测试用例覆盖了关键路径验证：

| 场景 | 输入 | 守卫是否介入 | 结果 |
|------|------|------------|------|
| zh 文本 + target=zh（退化对） | source=zh, target=zh | 介入 | target→en |
| auto + zh 文本 + target=zh | source=zh, target=zh | 介入 | target→en |
| ja→ja 退化对 | source=ja, target=ja | 介入 | target→zh |
| en 文本 + target=zh（合法） | source=en, target=zh | 不介入 | target=zh（保留） |
| ja→ko 跨语种（合法） | source=ja, target=ko | 不介入 | target=ko（保留） |

合法跨语种对（source!=target）均正确放行，无过度修正。

---

### 3. DRY 与生产调用路径

经 `grep` 核查全仓库：

- `resolve_direction_with_source`：生产调用路径 `ipc/translate.rs:184` 唯一引用，由 `translate_text_impl` 调用。
- `resolve_direction`（非 `_with_source`）：生产代码**零引用**；仅在 `tests/translate.rs` 及本次新增的 `lang.rs` 单测中使用。

两处守卫代码重复，为 DRY 违反，评估为 Important 级别（不阻塞当前放行，已在 I-01 中说明）。

---

### 4. 变体边界（第 4 点）

**结论：当前不是需修复的缺陷，属已知边界。**

守卫在 `resolve_direction_with_source` 层比较字面量（调用 `Lang::as_str()`）。若前端传 `"zh-CN"` 为 target，检测 source 为 `"zh"`，字面量不等，守卫不介入。此后 `map_lang_for_provider` 通过 `normalize_zh_variant` 归一处理，理论上仍可能产生退化对（`zh-CN|zh-CN`）。

但当前前端实现（下拉框）只传 `"zh"` 或 `"auto"`（见项目设计共识），"zh-CN" 变体从未由前端传入。此边界属有意识的架构约定范围内，不构成当前回归。建议在变体边界注释中（或 `is_explicit_source` 函数 doc 中）补充说明，以便未来扩展时提示审查者。

---

### 5. 规范符合性

| 检查项 | 结论 |
|--------|------|
| 注释写「为什么」（解释 403 背景、curl 证实） | 合规 |
| 函数 ≤ 50 行（resolve_direction: 16 行，resolve_direction_with_source: 25 行） | 合规 |
| 嵌套层数 ≤ 3 | 合规 |
| 无魔术值（守卫内仅用变量比较，无硬编码语言字符串） | 合规 |
| 测试 AAA 结构 | 合规（Arrange/Act/Assert 均有注释标注） |
| 测试行为化命名 | 合规（如 `…chinese_text_with_zh_target_falls_back_to_en`，描述行为而非实现） |
| 非弱断言（无 `assert!(true)`，均对具体值断言） | 合规 |
| 既有测试（集成测试 A02-b、A13 系列）语义兼容性 | 合规（守卫不影响 `configured_target=None` 路径，既有用例均通过） |

---

## 总结论

**通过（放行）。**

本次改动准确定位并修复了 zh→zh 退化对导致 MyMemory 403 的回归问题：

1. **退化对不变量**：`default_target` 构建逻辑保证 target 恒≠source，守卫数学上无漏网路径。
2. **合法对不受影响**：en→zh、ja→ko 等合法跨语种对在守卫条件 `t.as_str() != source.as_str()` 下直接放行，无过度修正。
3. **变体边界**：zh-CN 变体引发的潜在边界属已知范围内的架构约定（前端只传 "zh"/"auto"），当前不需修复。
4. **测试覆盖**：6 个新增单测覆盖核心回归场景（zh/auto 退化对、ja→ja、合法对不受影响），行为化命名，非橡皮图章。
5. **规范**：注释、函数长度、命名、AAA 结构均合规。

唯一建议修改项 I-01（`resolve_direction` 委托重构消除 DRY）不影响正确性，建议下一小功能顺手处理，当前可直接放行。
