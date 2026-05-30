---
id: V0-F2-S02-review
type: review
level: 小功能
parent: V0-F2
children: []
created: 2026-05-31T08:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F2-A01, V0-F2-A02]
evidence: []
author: code-reviewer
---

# 审查结论 · V0-F2-S02 全局热键配置与冲突检测

## 审查维度

依据 code-standards skill（格式/命名/函数/注释/类型/性能/测试/安全）+ 项目规范 + 设计文档§一/§二/§八。
审查文件：`src-tauri/src/hotkey.rs`（新增）、`src-tauri/src/lib.rs`（mod 暴露）、`src-tauri/tests/hotkey.rs`（新增）。

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| Important | `rebind` 的 `HotkeyAction::Translate` 分支从未被任何测试实际调用。若该 match arm 内字段赋值写错（如误写 `self.history_accelerator`），现有测试全部通过但翻译热键改键行为静默错误。置信度 85。 | `src-tauri/tests/hotkey.rs`（缺失用例） | 在 A01 测试或新增用例中补：`config.rebind(HotkeyAction::Translate, "CmdOrCtrl+Shift+Y", &registrar)`，并断言 `get_accelerator(Translate) == "CmdOrCtrl+Shift+Y"` 且 `get_accelerator(History)` 值不变。约 8–10 行，不涉及实现代码改动。 |

## 是否合规

**实现代码（hotkey.rs、lib.rs）完全合规**：

- **错误处理**：`HotkeyError` 用 `thiserror` 枚举化（`AlreadyInUse`/`SerdeError`/`IoError`）；`rebind`/`save`/`load` 全部 `?` 传播，无裸 `unwrap`/`expect`/`panic!`。
- **冲突检测语义**：`rebind` 先 `registrar.register()?`，失败时提前返回，配置字段未接触，确实拒绝保存、配置不变。`AlreadyInUse` 的 Display "热键已被占用，无法绑定" 含"已被占用"，满足设计文档§二。
- **默认热键**：`"CmdOrCtrl+Shift+V"` / `"CmdOrCtrl+Shift+T"`，与设计文档§一/§八严格一致。
- **持久化健壮性**：`save`/`load` 两步 `?` 传播，IoError/SerdeError 枚举化上抛，无吞错误。
- **格式/命名/注释**：合规（4 空格、≤120 行宽、PascalCase/snake_case、`//!` 解释 why、`///` 含 `# Errors`）。
- **无 TODO/FIXME/占位空实现**；测试 AAA 结构清晰、无恒真伪断言。

**测试存在一处覆盖缺口**：`HotkeyAction::Translate` 的 rebind 路径从未被实际触发，该 match arm 字段赋值正确性无测试保障。

## 结论

**打回。** 须补充 `rebind(HotkeyAction::Translate, ...)` 测试用例（约 8–10 行，仅测试文件改动），确认 Translate 分支实际写入 `translate_accelerator` 且不影响 `history_accelerator`。修复后重提审查。

---

## 复审结论（Round 2 · 2026-05-31）

**被复审项**：上轮 Important — Translate rebind 分支无测试覆盖　**status：通过**

补充测试 `hotkey_rebind_translate_isolates_field`（`src-tauri/tests/hotkey.rs` 第 84-108 行）完整覆盖打回要求：对 `HotkeyAction::Translate` 执行 `rebind`，断言 Translate 字段更新、History 字段保持默认不变，字段隔离得到验证。实现侧 Translate match arm 写入 `self.translate_accelerator`，与 History arm 严格区分，无串字段。新增测试 AAA 结构、三处断言均具判别力（非恒真），无新引入高危。本轮打回项已解决，V0-F2-S02 无遗留阻断项。
