---
id: V5-F1-S02-review
type: review
level: 小功能
parent: V5-F1
children: []
created: 2026-06-01T08:00:00Z
status: 通过
commit: 977d361
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 图片剪贴板捕获层（V5-F1-S02）

## 备注

本阶段 coder 因中断未产出 coding.md，此为正常情况，不影响本次审查结论。

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/clipboard.rs` | diff | 新增 `RawImageData` / `CapturedClip` / `rgba_to_png` / `rgba_to_png_for_test` / `snapshot_to_clips`；poll_once / poll_once_with_policy 返回类型改为 `Vec<CapturedClip>` |
| `src-tauri/src/pipeline.rs` | diff | `compute_composite_hash`（复合 FNV-1a 指纹）；`ArboardBackend.read` 补图片读取；`capture_and_ingest` 返回类型改为 `Vec<IngestOutcome>` |
| `src-tauri/src/lib.rs` | diff | `start_clipboard_poll` 适配新返回类型，`let _ = outcomes` 消费 |
| `src-tauri/tests/capture_image.rs` | 新增 | 10 个集成测试覆盖 poll 各场景 + rgba_to_png + capture_and_ingest |
| `src-tauri/tests/clipboard.rs` | diff | 适配 Vec 返回类型，增加 `first_text` 辅助函数 |
| `src-tauri/tests/boot_pipeline.rs` | diff | 适配 Vec 返回类型，更新断言方式 |
| `src-tauri/tests/privacy.rs` | diff | ClipboardSnapshot 构造补 `image: None` 字段 |

参照：Rust 规范（函数≤50行/单一职责/SQL参数化/Result别panic/持久化哈希显式稳定算法/注释写为什么/禁装饰注释/禁TODO_FIXME/错误处理完整）、code-standards、项目规范。

---

## 问题清单

### Important

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| **Important** | `capture_and_ingest` 遍历 `clips` 使用 `?`，任一条 DB 写入失败即返回 `Err` 并中断后续条目写入。但已写入条目不回滚，且 `last_seen` 在 `poll_once_with_policy` 里已推进，下次轮询不会重试。混合复制下文本写成功、图片写失败时，产生**部分写入**：文本入库、图片永久丢失，调用方 `lib.rs` 只 `eprintln` 不做重试，无任何补偿路径。 | `src-tauri/src/pipeline.rs:204-218` | 规范：错误处理完整；原子性语义。**修复建议**：用 SAVEPOINT 包裹整个 `clips` 写入循环：进入循环前 `conn.execute_batch("SAVEPOINT capture_ingest")?`，循环内 `?` 失败时先 `ROLLBACK TO SAVEPOINT capture_ingest`（忽略 rollback 失败）再 `RELEASE SAVEPOINT capture_ingest` 后返回 `Err`；循环正常结束后 `RELEASE SAVEPOINT capture_ingest`。这样两条写入要么全部提交要么全部回滚，与"同一次复制产生的 Text+Image 独立入库"的注释语义一致。 |
| **Important** | `src-tauri/tests/capture_image.rs` 第 124、246、276 行存在 3 处 `// ─── ... ───` 格式的装饰性横线分隔注释，项目规范与 code-standards 明确禁止此类装饰性分隔（含测试文件）。 | `src-tauri/tests/capture_image.rs:124,246,276` | 规范：`code-general.md`——"禁装饰性分隔注释（含测试文件）"。**修复**：删除 3 行 `// ─── ... ───` 注释。如需分组说明可改为普通单行注释，如 `// poll_once_with_policy 系列测试`。 |

**P1 置信度：85**

分析：DB 写入失败场景确实罕见（磁盘满/I/O 错误），但一旦发生，混合图文的 Text 已提交而 Image 丢失，且无任何重试或补偿路径。注释描述"有新内容 → 逐项分发写库"未声明原子性保证，但用户视角同一次复制的两条目应同时出现或同时不出现。当前实现的原子性缺口在语义上是真实缺陷。

**P2 置信度：100**

---

## 低于阈值的观察项（不阻断，备忘）

**`compute_composite_hash` 哈希碰撞（置信度约 55%）**

`text=None` 与 `text=Some("")`（空字符串）在同一图片内容下产生相同 combined 字节序列（均为 `[0xFF] + img_bytes`），哈希值相同，导致 `change_count` 不递增，视为"无变化"。但在实际使用中：arboard `get_text()` 在纯图片剪贴板上返回 `Err`（映射到 `None`），用户极少在剪贴板中存放空字符串文本；此场景实践影响极低，置信度不足 80%，不阻断。

**`rgba_to_png_for_test` 是否可改为单元测试（置信度约 35%）**

`pub #[doc(hidden)]` 导出生产模块 API 仅供测试使用，替代方案是将 `rgba_to_png` 测试内联到 `clipboard.rs` 内的 `#[cfg(test)]` mod 直接访问私有函数。但 `capture_image.rs` 是集成测试（`src-tauri/tests/` 目录），必须通过 pub API 访问；`#[doc(hidden)]` 是 Rust 社区惯用的测试辅助暴露方式，函数无副作用，安全性不受影响。属于设计选择，无规范约束，不阻断。

**`let _ = outcomes` 消费方式（置信度约 20%）**

`lib.rs` 中 `let _ = outcomes` 消费 `Vec<IngestOutcome>`，注释说明"每项已写库，此处仅用于调试可扩展"。这是标准 Rust 惯用法，可接受。

---

## 逐维度核查

### 1. 函数行数与单一职责

新增/修改函数行数统计：`rgba_to_png` 26 行 / `snapshot_to_clips` 29 行 / `poll_once` 26 行 / `poll_once_with_policy` 23 行 / `compute_composite_hash` 23 行 / `ArboardBackend::change_count` 19 行 / `ArboardBackend::read` 32 行 / `capture_and_ingest` 26 行。全部 ≤ 50 行。职责分明：编码转换/快照拆分/轮询判定/隐私门控/复合哈希/生产后端读写/管道入库。**通过。**

### 2. 注释质量

注释均描述"为什么"：`// 将基线下调为 current，避免计数恢复后误判为变化而重复捕获`、`// 分隔符：避免 "ab"+"c" 与 "a"+"bc" 哈希值相同`、`// 直接对 RGBA 字节哈希，不转 PNG（避免编码开销）`、`// 无论是否跳过，均推进 last_seen_count，防止下次轮询重复触发`。源文件无装饰性横线，无 TODO/FIXME，无注释掉的死代码。测试文件有 3 处违规（见 P2）。**源文件通过；测试文件未过（P2）。**

### 3. 错误处理

生产代码无 `unwrap()`/`panic!`，全部使用 `?` 传播或 `.ok()` 降级；lock 失败走 `unwrap_or_else(|e| e.into_inner())` 提取毒锁内值；`rgba_to_png` 失败 `eprintln` 后返回 `None` 不 panic。测试辅助用 `.expect("...")`，允许。**原子性缺口见 P1；单路径错误处理本身通过。**

### 4. 持久化哈希算法稳定性

`compute_composite_hash` 使用 FNV-1a 64-bit（`FNV_PRIME`/`FNV_OFFSET` 显式常量），不依赖 Rust 默认 hash。图片 RGBA 字节直接参与哈希，避免编码开销，注释说明理由。**通过。**

### 5. 0xFF 分隔符确定性

UTF-8 文本字节永不含 0xFF（0xFF/0xFE 均不是合法 UTF-8 字节）；文本字节与图片字节通过 0xFF 分隔后顺序固定，文本或图片任一变化必然改变指纹。分隔符能消除文本与图片间的拼接歧义。**通过**（边缘碰撞场景见观察项，置信度不足阻断）。

### 6. `read` 中降级逻辑一致性

`change_count` lock 失败 → 返回旧 count（不递增，不触发捕获）；`read` lock 失败 → 返回全 `None` snapshot（无文本无图片，`snapshot_to_clips` 返回空 Vec）。两路降级均不触发写库。`get_image().ok()` 在无图片或 headless 时静默降级为 `None`，注释说明 arboard 返回 `ContentNotAvailable` 等错误属于正常情况。**通过。**

### 7. `lib.rs let _ = outcomes`

`capture_and_ingest` 成功时 outcomes 的每项已写库，调用方不需要进一步处理；注释解释"仅用于调试可扩展"。`let _ = outcomes` 是标准 Rust 惯用写法。**通过。**

### 8. 测试质量

`poll_text_only` / `poll_image_only` / `poll_text_and_image`：验证 Vec 长度与枚举变体，非恒真。`rgba_to_png_valid_encodes_decodable_png`：用 image crate 解码验证，有效。`rgba_to_png_bad_length_returns_none`：错误路径验证，有效。`capture_and_ingest_text_and_image` / `capture_and_ingest_image_only`：直接 SQL 查询 DB 行数验证，非恒真。boot_pipeline / clipboard / privacy 适配性修改保持原有测试语义不变。**总体测试质量良好；缺失"Text 写成功+Image 写失败"的部分失败路径测试（对应 P1）。**

---

## 结论

**打回（必改 2 项）**

### 必改项

**M1（对应 P1）**：在 `capture_and_ingest` 的写库循环前后增加 SAVEPOINT，确保同一次捕获的所有条目（Text + Image）要么全部提交要么全部回滚。建议实现：循环前 `conn.execute_batch("SAVEPOINT capture_ingest")?`，`?` 失败时先 rollback savepoint 再返回 Err，循环正常结束后 release savepoint。同步在函数 doc-comment 的 `# Errors` 段补充"写库失败时已通过 SAVEPOINT 回滚全部条目"说明。

**M2（对应 P2）**：删除 `src-tauri/tests/capture_image.rs` 第 124、246、276 行的 3 处 `// ─── ... ───` 装饰性横线分隔注释，改为普通单行说明注释或直接删除。

---

## 复审（commit 977d361）

| 初审问题 | 修复（已核实位置 文件:行号） |
|---|---|
| M1 Important：capture_and_ingest 写库循环缺 SAVEPOINT，混合图文可能部分写入 | 已修复。`src-tauri/src/pipeline.rs` 中 `capture_and_ingest` 拆分为两个函数：写库循环前执行 `conn.execute_batch("SAVEPOINT capture_ingest;")` 开保护，失败时 ROLLBACK + RELEASE，成功时 RELEASE（约 213-240 行）。内层循环拆至辅助函数 `ingest_clips`（含说明注释"在调用方的 SAVEPOINT 保护下运行，任一条失败则由调用方回滚整批"）。doc-comment 已补充原子性说明与 SAVEPOINT 选型理由。 |
| M2 Important：capture_image.rs 第 124/246/276 行存在装饰性横线注释 | 已修复。三处横线分隔符已替换为普通描述性注释，如 `// poll_once_with_policy 系列`、`// rgba_to_png 系列`、`// capture_and_ingest 系列`（已直接读 commit 977d361 对应行核实，无 `──` 字符）。 |

终态：通过
