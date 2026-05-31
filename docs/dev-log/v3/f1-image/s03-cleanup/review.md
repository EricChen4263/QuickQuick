---
id: V3-F1-S03-review
type: review
level: 小功能
parent: V3-F1
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A04]
evidence: []
author: code-reviewer
---

# Review · V3-F1-S03 分级清理 + 三降级态归一

## 审查范围
- `src-tauri/src/image.rs`（tiered_cleanup/strip_original/total_image_bytes/is_degraded/CleanupPolicy/strip_oldest_originals/delete_oldest_nonfavorite_rows）+ `db.rs`(clip_images 加 is_favorite) + `tests/image.rs` + `tests/schema.rs`
依据：code-standards + 设计§五#4 + 关键架构收敛。

## 维度核查（通过）
分级两级顺序正确（strip→判总量→整条删）；收藏豁免（两级 SQL 含 is_favorite=0，测试验证收藏 strip 后 original_present=1 整条存活）；三态归一（is_degraded 单点判 original_present=0，测试验证 strip 后 is_degraded=true）；CleanupPolicy 可配；is_favorite 进 ensure_schema 预埋+schema 断言；SQL 参数化；无裸 unwrap/panic；无装饰注释/TODO；测试 AAA 非恒真；函数 ≤50 行。

## 问题清单
### Important
**[I-1] strip_original 缺 `AND is_deleted=0` 守卫（必修，置信度 82）**
- 位置：`image.rs` strip_original（UPDATE 无软删过滤，公开 API 可在 is_deleted=1 行静默 strip，语义不一致）。
- 修复：UPDATE 加 `WHERE id=?1 AND is_deleted=0`。

**[I-2] strip_oldest_originals 每条全表扫描 O(N²)（置信度 80）**
- 位置：`image.rs` strip_oldest_originals（每 strip 一条调 total_image_bytes 全表扫描）。
- 修复：本地累计已释放字节（strip 前读 length(original)），减少全表扫描次数。

**[I-3] 测试 limit 隐含假设 oldest_png.len()==DB original 大小（置信度 80）**
- 位置：`tests/image.rs`（`limit = total_before - oldest_png.len()`，依赖存储不转码假设）。
- 修复：从 DB 查 `length(original)` 作 oldest_orig_size，或补注释固化假设。

## 结论
**未过（打回）。** 必修 I-1（软删守卫）；建议同步 I-2（性能累计）+ I-3（DB 查长度）。核心逻辑正确。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-1 已解决**：strip_original UPDATE 加 `AND is_deleted=0` 软删守卫。
- **I-2 已解决**：strip_oldest_originals 改候选查询同取 length(original) + 循环前一次基准 + 本地递减估算，O(N²) 全表扫消除，清理语义不变。
- **I-3 已解决**：测试 oldest_orig_size 改 `SELECT length(original) FROM clip_images WHERE id` 从 DB 查，消除内存==DB字节假设。
分级两级/收藏豁免/三态归一核心未破坏；无新增高危。（注：第二级 delete_oldest_nonfavorite_rows 仍逐行 total_image_bytes，本轮非打回范围、v1 规模可接受。）
