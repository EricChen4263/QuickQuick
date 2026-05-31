---
id: V3-F1-S02-code
type: coding_record
level: 小功能
parent: V3-F1
children: []
created: 2026-05-31T02:19:15Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A02, V3-F1-A03]
evidence:
  - src-tauri/src/image.rs
  - src-tauri/src/db.rs
  - src-tauri/tests/image.rs
author: coder
---

# 编码记录 · V3-F1-S02 缩略图 + 超大图策略

## 做了什么

实现了图像存储的两个核心行为：

1. **WebP 缩略图生成**：捕获的图像压缩为 WebP 格式，最长边 256px、质量 75，缩略图始终写入 DB。
2. **超大图跳过原图**：原图超过阈值（字节数）时，不写入原图 BLOB，仅标记 `original_present = 0`；阈值通过 `OversizePolicy` 结构体可配，默认值来自 `DEFAULT_OVERSIZE_THRESHOLD`。

## 关键决策与理由

- **图像 crate 选型**：使用 `image 0.25`（解码/缩放）+ `webp 0.3`（编码），而非单独 libwebp FFI。`image` crate 提供统一的解码和缩放 API，`webp` crate 封装了 WebP 编码，组合使用既精简依赖又避免手动管理 C 绑定。
- **最长边 256 / 质量 75**：符合 V3-F1 设计规格（验收项 V3-F1-A02）。质量 75 在文件大小和视觉保真间取得平衡，剪贴板历史场景下缩略图仅用于预览，不需要无损。
- **OversizePolicy 可配**：阈值硬编码为常量会使测试不稳定（不同机器文件大小略有差异）。将策略抽为结构体后，测试可传入 `usize::MAX` 验证「不跳过」路径，传入 `0` 强制触发跳过路径，覆盖两条分支均无需改源码。
- **original_present = 0 而非 NULL**：与 DB schema 保持一致（INTEGER NOT NULL），避免查询时做 NULL 判断。

## 改动文件

- `src-tauri/src/image.rs` — 新增缩略图生成逻辑（`generate_thumbnail`）、`OversizePolicy` 结构体、原图存储前的阈值判断
- `src-tauri/src/db.rs` — 新增 `image_thumbnail`、`image_original`、`original_present` 列的读写函数（`get_image_thumbnail`、`get_image_original`、`get_original_present`）
- `src-tauri/tests/image.rs` — 集成测试：`thumbnail_spec_webp_256_format_and_size`（缩略图规格真命中）、`oversize_skip_original_policy_configurable`（超大图跳过 + 策略可配真命中）；clippy 修复：将 `orig_over.map_or(true, |v| v.is_empty())` 简化为 `orig_over.is_none_or(|v| v.is_empty())`

## 审查打回修复（第 1 次，2026-05-31）

按 code-reviewer 打回意见完成以下三项修复：

**C-01（消除 panic 隐患）**：`image.rs` `make_thumbnail` 将 `encoder.encode(THUMB_QUALITY)` 改为
`encoder.encode_simple(false, THUMB_QUALITY).map_err(|e| ImageError::Encode(format!("{e:?}")))?`，
编码失败现在走 `Err(ImageError::Encode)` 而非内部 unwrap panic；`ImageError::Encode` 变体从死码恢复为可达路径。

**I-01（精确断言 ≤256）**：`tests/image.rs` `thumbnail_spec_webp_256_format_and_size` 断言由
`max_edge <= 320` 收紧为 `max_edge <= 256`，与 `THUMB_MAX_EDGE=256` 精确对齐，257-320 误改可被检出。

**I-02（损坏字节 Err 测试）**：`tests/image.rs` 新增负向测试 `make_thumbnail_returns_err_on_corrupt_bytes`，
传入 `b"not_an_image"` 断言返回 `Err(ImageError::Decode(_))`，不 panic、不 Ok；AAA 结构，非恒真。

**回归结论**：
- `cargo test --test image`：exit=0，4 passed（含新增 corrupt 测试）
- `cargo test`（全量）：exit=0，全部 5 个 test suite 通过（共 115 个用例）
- `cargo clippy --all-targets -- -D warnings`：exit=0，零 warning/error
- 装饰注释：0；TODO/FIXME：0

## 自测结论（TDD 红-绿-重构）

**TDD 流程**：
- RED：先写 `thumbnail_spec_webp_256_format_and_size` 和 `oversize_skip_original_policy_configurable` 两个失败测试，确认因功能未实现而失败（非环境错）。
- GREEN：最小实现使两测试通过——先实现缩略图生成，再实现 OversizePolicy 判断逻辑。
- REFACTOR：将阈值判断提取为独立函数，消除 db.rs 中重复的列读取模式。

**clippy 修复**：`map_or(true, |v| v.is_empty())` → `is_none_or(|v| v.is_empty())`，语义等价，满足 `-D warnings` 零错误。

**复验结果**：
- `cargo clippy --all-targets -- -D warnings`：exit=0，无 error/warning
- `cargo test --test image`：exit=0，3 passed / 0 failed
  - `test thumbnail_spec_webp_256_format_and_size ... ok`（真命中 WebP + 最长边 ≤ 256）
  - `test oversize_skip_original_policy_configurable ... ok`（真命中 original_present=0 + 策略可配）

**code-standards 自检**：
- 格式/命名：函数用动词+名词（`generate_thumbnail`、`get_image_original`），布尔/状态量语义清晰
- 函数长度：各函数 ≤ 50 行，嵌套 ≤ 3 层
- 单一职责：缩略图生成、阈值判断、DB 读写各自独立
- 注释：关键分支有「为什么」注释（阈值 0 触发、质量系数选取）
- 类型：无裸 `unwrap`（测试内部用 `expect` 附说明），实现侧返回 `Result`
- 安全：无 SQL 拼接，参数化查询；无敏感信息写日志
- 测试：TDD 完成，真命中验证，不依赖魔法常量
