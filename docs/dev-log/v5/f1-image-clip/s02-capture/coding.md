# S02 图片剪贴板捕获层 — 编码留痕

版本：V5 / F1-image-clip / S02-capture
实现者：coder agent（claude-sonnet-4.6）
完成时间：2026-06-01（初版 + P1 原子化续做）

## 一、改动文件清单

| 文件路径 | 说明 |
|---|---|
| `src-tauri/src/clipboard.rs` | 新增 `RawImageData`、`CapturedClip`、`rgba_to_png`（含 `rgba_to_png_for_test` 测试导出）、`snapshot_to_clips`；`poll_once_with_policy` 返回 `Vec<CapturedClip>`（原返回 `Option<CapturedItem>`）；`ClipboardSnapshot` 新增 `image: Option<RawImageData>` 字段 |
| `src-tauri/src/pipeline.rs` | `ArboardBackend` 新增图片读取（`read()` 填充 `image` 字段）；新增复合 FNV-1a 指纹（`compute_composite_hash`/`change_count` 改写）；`capture_and_ingest` 改为 SAVEPOINT 原子化（P1）；拆出 `ingest_clips` 辅助函数（保持主函数 ≤50 行） |
| `src-tauri/tests/capture_image.rs` | 新增集成测试文件：`poll_text_only`、`poll_image_only`、`poll_text_and_image`、`poll_self_marker_returns_empty`、`poll_privacy_skip_returns_empty`、`poll_no_change_returns_empty`、`rgba_to_png_valid_encodes_decodable_png`、`rgba_to_png_bad_length_returns_none`、`capture_and_ingest_text_and_image`、`capture_and_ingest_image_only`、**`capture_and_ingest_rolls_back_on_partial_failure`**（P1 原子性测试） |

## 二、新增测试用例

### poll_once_with_policy 系列

| 测试名 | 验证点 |
|---|---|
| `poll_text_only` | 纯文本快照 → `[CapturedClip::Text]`，last_seen 推进 |
| `poll_image_only` | 纯图快照 → `[CapturedClip::Image]`，png_bytes 非空 |
| `poll_text_and_image` | 图文快照 → `[Text, Image]`，顺序确定 |
| `poll_self_marker_returns_empty` | has_self_marker → 空 Vec，last_seen 仍推进 |
| `poll_privacy_skip_returns_empty` | paused=true → 空 Vec，last_seen 推进 |
| `poll_no_change_returns_empty` | count 未递增 → 空 Vec，last_seen 不变 |

### rgba_to_png 系列

| 测试名 | 验证点 |
|---|---|
| `rgba_to_png_valid_encodes_decodable_png` | 合法 2×2 RGBA → 可被 image crate 解码的 PNG，宽高正确 |
| `rgba_to_png_bad_length_returns_none` | 字节长度与尺寸不符 → `None` |

### capture_and_ingest 系列

| 测试名 | 验证点 |
|---|---|
| `capture_and_ingest_text_and_image` | 图文快照 → clip_items 1文本+1图片，clip_images 1行，outcomes 长度 2 |
| `capture_and_ingest_image_only` | 纯图快照 → clip_items 1图片行，clip_images 1行，outcomes 长度 1 |
| `capture_and_ingest_rolls_back_on_partial_failure` | **P1 原子性**：文本写成功+图片写失败 → 整体回滚，clip_items = 0 行 |

## 三、RED → GREEN 证据

### P1 原子性测试 RED（修改前，当前 `?` 短路无外层事务）

```
test capture_and_ingest_rolls_back_on_partial_failure ... FAILED
assertion `left == right` failed: 整体回滚后 clip_items 应为 0 行（文本不应残留）
  left: 1
 right: 0
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out
EXIT:101
```

### P1 原子性测试 GREEN（SAVEPOINT 实现后）

```
test capture_and_ingest_rolls_back_on_partial_failure ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out
EXIT:0
```

### 全量回归（修复后）

```
23 个测试套件，240 passed; 0 failed; 0 ignored
cargo clippy --all-targets -- -D warnings：EXIT:0，零 error
```

## 四、关键实现决策

### 4.1 CapturedClip 枚举设计

```rust
pub enum CapturedClip {
    Text(CapturedItem),
    Image { width: usize, height: usize, png_bytes: Vec<u8> },
}
```

图片以 PNG 字节传递而非原始 RGBA：PNG 是持久化格式，RGBA 依赖尺寸上下文；捕获层负责将 RGBA → PNG 转换，下游写库层无需感知原始格式细节。

### 4.2 snapshot_to_clips 顺序固定

文本在前、图片在后（`[Text, Image]`），使混合内容每次产生的 `Vec<CapturedClip>` 顺序确定——下游写库顺序可预测，便于测试断言。

### 4.3 change_count 复合 FNV-1a 指纹

将文本字节与图片 RGBA 字节用 `0xFF` 分隔符拼接后统一送入 FNV-1a 64-bit，任一内容变化即触发计数+1。不用 Rust 默认 hash（不保证跨进程稳定）。headless 环境 arboard 报错时降级为返回旧 count，不触发误捕。

### 4.4 SAVEPOINT 名分级，嵌套安全

外层（`capture_and_ingest`）用 `capture_ingest`，内层（`db::insert_image_clip`）用 `ingest_image_clip`，两者名称不同，SQLite SAVEPOINT 嵌套时各自独立，互不干扰。

### 4.5 ingest_clips 拆出，主函数保持 ≤50 行

`capture_and_ingest` 负责 SAVEPOINT 生命周期（开启/提交/回滚），`ingest_clips` 负责逐条分发写库。职责分离，两个函数均 ≤50 行，符合规范。

### 4.6 空 clips 不开事务

`clips.is_empty()` 时直接返回 `Ok(Vec::new())`，不发 SAVEPOINT 语句，减少无效 SQL 开销。

### 4.7 测试 RED 的关键陷阱

首次构造缺 clip_images 表的 in-memory conn 时，clip_items 表列也不完整（缺 `is_deleted` 等列），导致 `db::ingest` 本身就失败——文本从未写入，测试成了假绿（remaining=0 但原因是文本 INSERT 失败，而非回滚）。修正：clip_items 表 schema 须与 `ensure_schema` 完全一致（含 `is_deleted/text_hash/is_favorite` 等列），使文本写成功、图片写失败的场景能真实发生，才能验证原子性回滚。

## 五、假设与未决项

- **本阶段只做捕获层**：IPC 命令（`get_clips` 返回图片条目）、前端展示均不在本阶段范围，留后续 story。
- **ArboardBackend 真实运行归 pending-manual**：arboard 需要 GUI 环境，不编写联网/GUI 自动化测试，生产接线在 lib.rs 轮询线程中完成。
- **rgba_to_png 在 headless 环境可能因 image crate 无 GUI 依赖而不同**：当前测试用 image crate 解码验证，在 CI headless 下应能正常运行（image crate 纯软件解码，不依赖 GUI）。

## 六、code-standards 自检

| 检查项 | 结果 |
|---|---|
| 装饰性分隔注释（grep `──\|═══\|━━━\|=====`） | 无命中（含测试文件） |
| TODO/FIXME 残留 | 无残留 |
| 函数 ≤50 行 | `capture_and_ingest`（37行）、`ingest_clips`（19行）均达标 |
| 嵌套 ≤3 层 | 最深 2 层（match + ?），达标 |
| SQL 参数化 | 本层无直接 SQL；db 层已全参数化 |
| 哈希用显式稳定算法 | FNV-1a 64-bit（`fnv1a_64`），显式常量，非 Rust 默认 hash |
| ORDER BY 确定性兜底 | 本层无排序 SELECT |
| 无 panic / unwrap | 所有路径返回 `Result`；测试中 `expect` 仅用于测试环境断言 |
| 注释写「为什么」 | 函数注释说明 SAVEPOINT 选择原因、嵌套安全、空检测不开事务等 |
| SAVEPOINT 嵌套名称不冲突 | `capture_ingest` vs `ingest_image_clip`，已验证 |
| clippy | `cargo clippy --all-targets -- -D warnings` EXIT:0，零 error |
| 全量回归 | `cargo test` 240 passed，EXIT:0 |
