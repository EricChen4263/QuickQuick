---
id: V3-F1-S01-review
type: review
level: 小功能
parent: V3-F1
children: []
created: 2026-05-31T16:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A01]
evidence: []
author: code-reviewer
---

# 审查记录 · V3-F1-S01 图片捕获入库（BLOB 拆分 + 原图无损 + 字节哈希判重）

## 审查范围
- `src-tauri/src/image.rs`（image_hash/ingest_image/get_image_original/get_image_thumbnail/IngestImageOutcome）+ `db.rs`(clip_images 加 image_hash 列) + `mod.rs` + `tests/image.rs` + `tests/schema.rs`(断言)
依据：code-standards + 设计§五#1/§十。

## 通过性核查
原图无损（original BLOB 逐字节取回相等）✓；BLOB 拆分（thumbnail/original 两字段各自取回）✓；字节哈希判重（image_hash FNV-1a 确定性，WHERE image_hash AND is_deleted=0，同→Bumped/不同→Inserted）✓；image_hash 列进 ensure_schema 预埋 + schema 断言 ✓；SQL 全参数化 ✓；无裸 unwrap/panic ✓；函数 ≤50 行；无装饰注释/TODO；image_capture_lossless_split 三分支 8 断言 AAA 非恒真。

## 问题清单（Important，无 Critical）
**[I-01] ingest_image 写入 clip_item_id=NULL，GC 级联与写入路径脱节（置信度 85）**
- 位置：`image.rs` ingest_image INSERT。clip_images.clip_item_id 声明 ON DELETE CASCADE 但 ingest 不写该列→NULL，GC 删 clip_items 不级联到这批图片行；schema 级联测试手动指定 clip_item_id 与真实写入路径不同。
- 修复（polish，无需改实现）：ingest_image 文档注释明确声明"当前阶段 clip_item_id 不写入(NULL)、GC 级联不适用本路径；与 clip_item_id 绑定及对应 GC 留待分级清理 story V3-F1-A04 补全"，防误判。

**[I-02] "不同字节→不同哈希"测试样本判别力弱（置信度 80）**
- 位置：`image.rs` 单测（`[1,2,3]` vs `[3,2,1]` 倒序对 FNV 必然不等）。
- 修复：替换为末位差异（`[1,2,3]` vs `[1,2,4]`）/空序列/单字节边界，增强判别力（非恒真）。

## 结论
**未过（打回）。** 修 I-01（文档声明 clip_item_id 缺口）+ I-02（边界测试样本）后复审。核心实现合格。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-01 已解决**：ingest_image 文档注释新增「clip_item_id 缺口声明」小节（NULL/GC 级联不生效/留待 V3-F1-A04），三项要求全覆盖。
- **I-02 已解决**：新增 `image_hash_differs_on_last_byte_only`（[1,2,3] vs [1,2,4]）+ `image_hash_empty_and_single_byte_boundary`（空/单字节）边界用例，判别力充分非恒真。
核心（原图无损/拆分/字节哈希判重/SQL 参数化）未破坏，无新增高危。
