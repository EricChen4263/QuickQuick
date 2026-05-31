---
id: V3-F1-S02-review
type: review
level: 小功能
parent: V3-F1
children: []
created: 2026-05-31T08:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A02, V3-F1-A03]
evidence: []
author: code-reviewer
---

# 审查记录 · V3-F1-S02 缩略图 WebP 规格 + 超大图处理

## 审查范围
- `src-tauri/src/image.rs`（make_thumbnail/scale_to_max_edge/OversizePolicy/ingest_image_with_policy）+ Cargo.toml(image 0.25/webp 0.3) + `tests/image.rs`
依据：code-standards（无裸 unwrap/panic、禁装饰注释）+ 设计§五#2/#3。

## 通过性核查
A02 WebP 魔数(RIFF/WEBP)断言真命中；最长边 scale_to_max_edge(THUMB_MAX_EDGE=256,Lanczos3,不放大小图)；质量 THUMB_QUALITY=75；A03 超大图 `original.len()>max` 存 b""+original_present=0、阈值可配(10/usize::MAX 两值验)；SQL 参数化；无装饰注释/TODO；函数 ≤50 行；ImageError thiserror；ingest_image 兼容。

## 问题清单
### Critical
**[C-01] make_thumbnail 编码路径裸 unwrap，编码失败 panic 而非 Err（置信度 92）**
- 位置：`image.rs`（`encoder.encode(THUMB_QUALITY)`）。`webp::Encoder::encode()` 内部 `self.encode_simple(false,quality).unwrap()` + `WebPConfig::new().unwrap()` 均裸 unwrap → 编码失败 panic。`ImageError::Encode` 文档声明但成死码（当前路径不可达），违反"无裸 unwrap/panic"。
- 修复：改 `encoder.encode_simple(false, THUMB_QUALITY).map_err(|e| ImageError::Encode(format!("{e:?}")))?`（返回 Result），使错误走 Err 非 panic。

### Important
**[I-01] 缩略图尺寸断言过宽(≤320)未精确覆盖 256（置信度 82）**
- 位置：`tests/image.rs`（仅 `assert!(max_edge<=320)`，THUMB_MAX_EDGE=256，257-320 误改无法检出）。
- 修复：增 `assert!(max_edge<=256)` 或 THUMB_MAX_EDGE 改 pub const 测试引用。

**[I-02] 缺损坏字节解码 Err 测试（置信度 80）**
- 位置：`tests/image.rs`（仅合法 PNG，无损坏字节 Err 路径覆盖）。
- 修复：增负向测试 `make_thumbnail(b"not_an_image")` 断言 `Err(ImageError::Decode(_))` 非 panic/Ok。

## 结论
**未过（打回）。** 修 C-01（encode_simple 返回 Result 不 panic）+ I-01（精确 ≤256 断言）+ I-02（损坏字节 Err 测试）后复审。核心功能逻辑正确。

---

## 复审结论（2026-05-31）

**status = 通过**

- **C-01 已解决**：make_thumbnail 改 `encoder.encode_simple(false,THUMB_QUALITY).map_err(|e| ImageError::Encode(...))?`，编码失败走 Err 非 panic，ImageError::Encode 非死码。
- **I-01 已解决**：缩略图尺寸断言收紧为 `max_edge<=256`（匹配 THUMB_MAX_EDGE）。
- **I-02 已解决**：新增 `make_thumbnail_returns_err_on_corrupt_bytes`（损坏字节→Err(Decode) 非 panic/Ok）。
WebP/超大图/原图无损核心未破坏；无新引入高危；image 4 测试全过。
