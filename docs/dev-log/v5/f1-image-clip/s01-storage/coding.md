# S01 图片剪贴板存储层 — 编码留痕

版本：V5 / F1-image-clip / S01-storage
实现者：coder agent（claude-sonnet-4.6）
完成时间：2026-06-01（初版）；门禁打回修复：2026-06-01

## 一、改动文件清单

### 初版（原始实现）

| 文件路径 | 说明 |
|---|---|
| `src-tauri/src/db.rs` | 新增 `ingest_image_as_clip` 公开函数 + 两个私有辅助函数（`insert_image_clip`、`try_insert_image_clip`）+ 测试辅助（`make_test_png`、`make_test_conn`）+ T1/T2/T3 三个测试 |

### 门禁打回修复（tester+reviewer 打回，四项必修）

| 文件路径 | 说明 |
|---|---|
| `src-tauri/src/db.rs` | ① UPDATE 移入 SAVEPOINT 内（事务原子性）② UPDATE 行数校验 ③ clippy collapsible_match 修复 ④ 新增 T4 孤立行领养测试 |

## 二、新增测试用例

| 测试名 | 验证点 |
|---|---|
| `ingest_image_as_clip_inserts_item_and_links_image` | T1：首次入库→clip_items 恰好 1 行(kind='image', content 含 '×')，clip_images 恰好 1 行且 clip_item_id = item_id（非 NULL），返回 `Inserted` |
| `ingest_image_as_clip_dedup_bumps_timestamp` | T2：同 PNG 二次调用→返回 `Bumped`，clip_items/clip_images 均仍 1 行，last_modified_utc >= 首次值 |
| `ingest_image_as_clip_invalid_png_rolls_back` | T3：空字节（非法 PNG）→函数返回 Err，clip_items 和 clip_images 无孤立行（SAVEPOINT 回滚生效） |
| `ingest_image_as_clip_adopts_orphaned_image_row` | T4：clip_item_id=NULL 孤立行领养→clip_images 仍 1 行（图片数据复用），clip_item_id 已补写为新 item_id，clip_items 新增 1 行，返回 `Inserted` |

## 三、RED → GREEN 证据

### RED（函数未实现时的编译错误）

```
error[E0425]: cannot find function `ingest_image_as_clip` in this scope（出现 4 次）
error: could not compile `quickquick` (lib test) due to 4 previous errors
```

### GREEN（初版实现后目标测试）

```
running 3 tests
test db::tests::ingest_image_as_clip_invalid_png_rolls_back ... ok
test db::tests::ingest_image_as_clip_inserts_item_and_links_image ... ok
test db::tests::ingest_image_as_clip_dedup_bumps_timestamp ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.01s
```

### GREEN（门禁打回修复后，含 T4 全量回归）

```
test db::tests::ingest_image_as_clip_inserts_item_and_links_image ... ok
test db::tests::ingest_image_as_clip_dedup_bumps_timestamp ... ok
test db::tests::ingest_image_as_clip_invalid_png_rolls_back ... ok
test db::tests::ingest_image_as_clip_adopts_orphaned_image_row ... ok
test result: ok. 28 passed; 0 failed; 0 ignored（lib tests）
（所有子模块总计全绿，EXIT:0）
cargo clippy --all-targets -- -D warnings：EXIT:0，零 error
```

## 四、关键实现决策

### 4.1 函数分三层，各 ≤50 行

- `ingest_image_as_clip`（约 30 行）：哈希查重，命中走 bump，未命中委托 `insert_image_clip`
- `insert_image_clip`（约 25 行）：SAVEPOINT 生命周期管理，成功 RELEASE、失败 ROLLBACK
- `try_insert_image_clip`（约 30 行）：INSERT clip_items + `ingest_image_with_policy` + UPDATE clip_item_id（三步全在 SAVEPOINT 保护内）

### 4.2 SAVEPOINT 而非 Transaction

`ingest_image_with_policy` 接受 `&Connection`（不带 `&Transaction`），若用 `conn.transaction()` 会因借用冲突无法将同一连接传入。SAVEPOINT 通过 `execute_batch` 字符串操作，不涉及借用，可在同一连接上嵌套，完全规避问题。

### 4.3 clip_item_id 补写时序（修正）

**原实现（错误）**：UPDATE 在 `RELEASE SAVEPOINT` 之后执行，脱离事务保护。若 RELEASE 成功但 UPDATE 失败，clip_items 已提交但 clip_images.clip_item_id 仍为 NULL，造成半关联孤立状态。

**修正后（正确）**：UPDATE 移入 `try_insert_image_clip` 内部，紧跟 `ingest_image_with_policy` 之后，三步写（INSERT clip_items → ingest_image_with_policy → UPDATE clip_item_id）全部在 SAVEPOINT 保护内。任一步失败，调用方统一 ROLLBACK，不留孤立行。

### 4.4 UPDATE 行数校验（新增）

UPDATE 后检查 `affected != 1`，返回 `DbError::Other`。防止 image_id 不存在时静默半关联——此类情形在正常流程下不应发生，若命中说明逻辑出现意外，应报错而非静默通过。

### 4.5 collapsible_match 修复

原嵌套 `if let Some((_img_id, maybe_item_id)) = existing { if let Some(item_id) = maybe_item_id { ... } }` 合并为单层 `if let Some((_img_id, Some(item_id))) = existing { ... }`。语义不变：仅当 clip_item_id 非 NULL 时才视为有效命中并 bump；孤立行（None 内层）自然落到 insert_image_clip 路径。

### 4.6 去重 Bumped 路径：只 bump clip_item，不 bump clip_images

`last_modified_utc` 刷新发生在 `clip_items` 行（通过复用现有 `bump_to_top`），使列表排序逻辑保持一致（列表按 clip_items.last_modified_utc 排序）。clip_images 的 last_modified_utc 由 `ingest_image_with_policy` 内部 bump，不重复操作。

### 4.7 历史孤立行领养路径（T4 覆盖）

若 `clip_images.clip_item_id` 为 NULL（孤立行），`ingest_image_as_clip` 视为未命中，走 `insert_image_clip`。`ingest_image_with_policy` 因 image_hash 重复，命中孤立行返回 `Bumped(old_image_id)`；`try_insert_image_clip` 拿到该 image_id，UPDATE 将其关联到本次新建的 item_id。图片数据（thumbnail/original）复用不重复存储，clip_items 新增 1 行，返回 `Inserted`。

### 4.8 `make_test_png` 辅助用 image crate 动态生成

不依赖外部文件，保持测试自包含。生成 RGBA 空图（全透明像素），PNG 格式可被 `make_thumbnail` 正常解码，满足 T1/T2/T4 需求。T3 用空字节 `b""` 直接触发解码失败，不需要构造特殊损坏数据。

## 五、假设与未决项

- **本阶段只做存储层**：捕获（arboard 监听）、IPC 命令、前端展示均不在本阶段范围，留后续 story。
- **混合内容（文本+图片）策略已敲定**：两条独立条目分别走各自路径，本函数不处理混合逻辑。
- **`ingest_image_with_policy` 签名未修改**：保持原有签名，通过 `UPDATE clip_images SET clip_item_id` 补写外键，不破坏既有测试。
- **OversizePolicy 使用默认值（20 MiB）**：调用方如需自定义可传入，本阶段用 `Default::default()` 满足测试需要。

## 六、code-standards 自检（修复后）

| 检查项 | 结果 |
|---|---|
| 装饰性分隔注释（grep `──\|═══\|━━━\|=====`） | 无命中 |
| TODO/FIXME 残留（grep `TODO\|FIXME`） | 无残留 |
| 函数 ≤50 行 | 三函数均达标 |
| 嵌套 ≤3 层 | 最深 2 层（if let + match），达标 |
| SQL 参数化 | 全部使用 `rusqlite::params![]`，无字符串拼接 |
| 哈希用显式稳定算法 | 复用 `image::image_hash`（FNV-1a 64-bit，已有稳定实现） |
| ORDER BY 确定性兜底 | 本函数无 SELECT 排序；测试断言用精确行数/字段值，不依赖顺序 |
| 无 panic / unwrap | 所有路径返回 `Result`，`expect` 仅在测试代码中 |
| 注释写「为什么」 | 各函数注释说明设计意图（SAVEPOINT 原因、补写时序在 SAVEPOINT 内、孤立行领养逻辑） |
| UPDATE 行数校验 | `affected != 1` 返回 `DbError::Other`，防静默半关联 |
| clippy | `cargo clippy --all-targets -- -D warnings` EXIT:0，零 error |
| 全量回归 | `cargo test` 全绿，28 passed，EXIT:0 |
