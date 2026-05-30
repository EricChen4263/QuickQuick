---
id: V1-F2-S05-review
type: review
level: 小功能
parent: V1-F2
children: []
created: 2026-05-31T10:30:00Z
status: 通过
commit: WIP
acceptance_ids: [V1-F2-A11]
evidence: []
author: code-reviewer
---

# 审查记录 · 收藏：★置顶 + 豁免清理（V1-F2-S05）

## 审查范围
- `src-tauri/src/db.rs`：新增 is_favorite 列、ClipRow、set_favorite、list_ordered、cleanup_keep_recent；全仓装饰注释清理。
- `src-tauri/tests/clipboard.rs`：新增 favorite_pin_sorted_first（A11 置顶）、favorite_exempt_from_cleanup（A11 豁免）。
参照：code-standards + 设计§九.2（★置顶）+ §五（收藏永远豁免）。

## 问题清单

### Critical
无。

### Important
**[I-01] `cleanup_keep_recent` 的 `.filter_map(|r| r.ok())` 静默吞错（置信度 88）**
- 位置：`src-tauri/src/db.rs`（cleanup_keep_recent 收集 id 列表处）
- 问题：query_map 迭代中若某行 `rusqlite::Error`，filter_map(r.ok()) 静默丢弃 → all_ids 偏少 → keep_count 裁剪偏差（清理不足）。与 list_ordered 的 `for row in rows { result.push(row?); }` 传播惯例不一致。
- 修复：改两阶段 `for r in rows { all_ids.push(r?); }`，错误用 `?` 传播。

**[I-02] `favorite_pin_sorted_first` 无法单独验证 `is_favorite DESC`（置信度 85）**
- 位置：`src-tauri/tests/clipboard.rs`（favorite_pin_sorted_first）
- 问题：set_favorite 同时刷新 last_modified_utc。测试 A(旧)→B(新)→set_favorite(A) 后 A 的时间戳已比 B 新；即便 SQL 删掉 `is_favorite DESC` 仅 `last_modified_utc DESC`，A 仍排第一——测试对"丢失 is_favorite 排序"的错误实现零排除能力。
- 修复：set_favorite(A) 后、list_ordered 前 `bump_to_top(&conn, &id_b)` 把 B 刷到最新，强制只能靠 is_favorite DESC 使 A 排第一。

## 逐维度核查（通过项）
置顶排序 `ORDER BY is_favorite DESC, last_modified_utc DESC` 语义正确 ✓；豁免清理 `WHERE is_deleted=0 AND is_favorite=0`（收藏不入候选、绝不删）✓；set_favorite 参数化 + 刷新 last_modified + WHERE is_deleted=0 守卫 ✓；is_favorite 列 IF NOT EXISTS 幂等不破 V0 ✓；ClipRow 最小字段集 ✓；新增三函数无裸 unwrap（除 I-01 的 filter_map）、≤50 行、命名规范 ✓；装饰注释 grep（src-tauri/src+tests+src）全净 ✓；无 TODO/FIXME ✓；favorite_exempt_from_cleanup AAA + cleaned>=1 + 收藏 is_deleted=0 精确验证 ✓。

## 总结论
**未过（打回）。** 修复 I-01（错误 `?` 传播）+ I-02（测试 bump B 强制 is_favorite 排序生效）后复审。无 Critical，装饰注释已全净，预期一次复查通过。

---

## 复审结论（第2轮 · 2026-05-31）

**status: 通过**

- **I-01 已解决**：`cleanup_keep_recent` 行迭代改 `for r in rows { all_ids.push(r?); }`，错误 `?` 传播，与 list_ordered 惯例一致，不再静默吞错。
- **I-02 已解决**：`favorite_pin_sorted_first` 在 set_favorite(A) 后追加 `bump_to_top(&conn,&id_b)` 使 B 时间戳更新；若 SQL 丢失 `is_favorite DESC` 则 B 排第一断言失败，测试对错误实现有排除力。
无新引入≥80 高危；favorite 两测试覆盖 A11 置顶+豁免语义。
