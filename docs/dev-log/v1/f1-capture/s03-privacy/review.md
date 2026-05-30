---
id: V1-F1-S03-review
type: review
level: 小功能
parent: V1-F1
children: []
created: 2026-05-31T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A06, V1-F1-A07, V1-F1-A08]
evidence: []
author: code-reviewer
---

# 审查报告 · V1-F1-S03 隐私门控

## 审查范围
- `src-tauri/src/privacy.rs`（SkipReason/ExcludeList/CapturePolicy/should_skip）
- `src-tauri/src/clipboard.rs`（ClipboardSnapshot 加 is_concealed/source_app；poll_once_with_policy）
- `src-tauri/tests/privacy.rs`（A06/A07）、`src-tauri/tests/clipboard.rs`（A08 + S01/S02 兼容）、`lib.rs`

## 发现问题

### Important
#### I-1 装饰性分隔注释残留（code-general 硬规则，置信度 100）
- 文件 `src-tauri/tests/clipboard.rs` 违规行：15、50、74、102、121、155、207（`// ── ... ──`）。
- 依据：code-general "禁装饰性分隔注释 ═══/───/━━━"，`──` 属 `───` 变体。S03 本次触及该文件（新增 A08），应一并清理遗留违规。
- 修复：删除全部 `// ── ... ──` 分隔行（测试函数名/doc 已述意图，无需装饰标题）。

#### I-2 A05 节标题覆盖 A08 函数体（错位，置信度 85）
- 文件 `src-tauri/tests/clipboard.rs` 第 207-225 行：第 207 行标题 `V1-F1-A05 bump_no_new_record`，其下函数实为 A08 `pause_stops_capture`，真正 A05 在第 245 行，标题与函数错位。
- 修复：A08 两个暂停测试移至 A05 之后/文件末尾，或补正确 A08 标注（非装饰形式）。

## 正确性核查（通过项）
should_skip 判定序 paused→is_concealed→excluded→self_marker→None 与设计§三#6 一致 ✓；**不做内容启发式**（concealed_no_heuristic 反证：内容像密码但 is_concealed=false 不跳过）✓；source_app None 不误触发排除（if let Some 短路）✓；跳过时 last_seen 仍推进（先于跳过判定）✓；has_self_marker 由 should_skip 第4步覆盖 ✓；ClipboardSnapshot 新字段 FakeBackend 默认 false/None 保 s01/s02 兼容 ✓；无裸 unwrap/panic（生产）✓；无 TODO/FIXME ✓；mod 暴露正确 ✓。

## 结论
**未过（打回）。** 逻辑实现正确；打回仅针对格式硬规则违反：清理 tests/clipboard.rs 7 处装饰分隔注释（I-1）+ 修正 A05/A08 节标题与函数错位（I-2）后复审。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-1 已解决**：privacy.rs/clipboard.rs/tests 三文件 `──/═══/━━━` 全量扫描零匹配，装饰分隔注释彻底清除。
- **I-2 已解决**：tests/clipboard.rs 函数顺序 A01→A02→A03→reset→A04→A05→A08（pause 系列在 bump_no_new_record 之后），无标题/函数错位；各断言逻辑与实现层严格对应未弱化。
- 无新引入≥80 高危。
