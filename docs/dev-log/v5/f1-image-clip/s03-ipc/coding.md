# S03 IPC 暴露层 — 编码留痕

阶段：Phase 5 实现（coder agent）
功能：图片剪贴板·能看 — 第 3 阶段：IPC 暴露层
日期：2026-06-01

## 改动清单

| 文件 | 改动说明 |
|---|---|
| `src-tauri/Cargo.toml` | `[dependencies]` 加 `base64 = "0.22"` |
| `src-tauri/src/db.rs` | 新增 `ClipItemRowWithImage` 结构体、`list_items_with_images` 函数（LEFT JOIN clip_images，保留 `list_items_full` 不变） |
| `src-tauri/src/ipc/clipboard.rs` | `ClipItemDto` 加 `thumbnail_data_url`/`image_id` 两字段；`list_clip_items_impl` 改用 `list_items_with_images` 并编码 WebP data URL；新增 `get_clip_image_original_impl` 纯函数 + `get_clip_image_original` Tauri 命令；新增 3 个 TDD 单测 |
| `src-tauri/src/lib.rs` | `generate_handler!` 注册 `ipc::clipboard::get_clip_image_original` |

## 新增测试

位置：`src-tauri/src/ipc/clipboard.rs` `#[cfg(test)] mod tests`

| 测试名 | 验证行为 |
|---|---|
| `list_clip_items_impl_image_has_thumbnail_data_url_and_image_id` | 图片 DTO 的 `thumbnailDataUrl` 以 `data:image/webp;base64,` 开头、`imageId` 非 None；文本 DTO 两字段均 None |
| `get_clip_image_original_impl_returns_data_url_for_existing_image` | 存在的 image_id 返回 `Some`，以 `data:image/png;base64,` 开头 |
| `get_clip_image_original_impl_returns_none_for_missing_id` | 不存在的 id 返回 `Ok(None)` |

## TDD RED → GREEN 证据

**RED（失败确认）**
```
error[E0432]: unresolved import `base64`
error[E0433]: cannot find `ClipKind` in `clipboard`
error[E0609]: no field `thumbnail_data_url` on type `&ClipItemDto`
error[E0609]: no field `image_id` on type `&ClipItemDto`
error: could not compile `quickquick` (lib test) due to 9 previous errors
EXIT:101
```
失败原因：字段/模块均未实现，符合 TDD RED 要求。

**GREEN（全部通过）**
```
running 3 tests
test ipc::clipboard::tests::get_clip_image_original_impl_returns_none_for_missing_id ... ok
test ipc::clipboard::tests::get_clip_image_original_impl_returns_data_url_for_existing_image ... ok
test ipc::clipboard::tests::list_clip_items_impl_image_has_thumbnail_data_url_and_image_id ... ok
test result: ok. 3 passed; 0 failed; 0 ignored
```

## 全量回归

全量 `cargo test`（含新增 3 个）：**全部通过，0 failed**。
（lib 套件含新测试共 31 passed，整体所有套件合计约 263 passed）

## clippy

```
cargo clippy --all-targets -- -D warnings
EXIT:0   （0 error，0 warning）
```

## base64 联网情况

未联网。`base64 = "0.22"` 已在 `Cargo.lock`（传递依赖），cargo 直接使用本地缓存，无需网络访问。

## 关键实现决策

1. **保留 `list_items_full` 不变**：规格要求，避免破坏现有测试；新建 `list_items_with_images` 平行提供带图版本。
2. **LEFT JOIN 而非子查询**：与规格 SQL 对齐；文本条目 `image_id`/`thumbnail` 自然为 NULL，映射到 Rust `Option::None`。
3. **base64 编码位置**：在 IPC 层（`list_clip_items_impl`、`get_clip_image_original_impl`）完成编码，db 层只负责存取 BLOB，保持职责分离。
4. **`use base64::Engine`**：`base64 0.22` 需通过 trait `Engine` 才能调用 `.encode()`，在模块顶部导入，避免重复写 use 语句。

## reviewer 打回修复（M1 Critical · 2026-06-01）

**问题**：`get_clip_image_original_impl` 对降级图（`original_present=0`，`original=X''` 空 BLOB）的处理有缺陷——`get_image_original` 返回 `Some(vec![])` 而非 `None`，导致产出空 data URL `"data:image/png;base64,"`，前端图片加载失败。

**修复位置**：`src-tauri/src/ipc/clipboard.rs` 第 150–156 行（`get_clip_image_original_impl`）

**修复方式**：用 match guard `Some(bytes) if !bytes.is_empty()` 把空 BLOB 与 `None` 统一归入 `_ => Ok(None)`，附注释说明降级语义。不改 `image.rs` 的 `get_image_original`（最低风险）。

**TDD RED → GREEN（T4）**：

RED（修复前）：
```
thread '...get_clip_image_original_impl_returns_none_for_downgraded_image' panicked at src/ipc/clipboard.rs:322:9:
降级图（空 BLOB）应返回 None，实际返回：Some("data:image/png;base64,")
test result: FAILED. 0 passed; 1 failed; finished in 0.00s   EXIT:101
```

GREEN（修复后）：
```
cargo test get_clip_image_original_impl_returns_none_for_downgraded_image
1 passed, 242 filtered out   EXIT:0
```

**全量回归（含 T4）**：244 passed, 0 failed（23 suites）。

**clippy**：`cargo clippy --all-targets -- -D warnings` → 0 error，0 warning，EXIT:0。

---

## 自检清单（code-standards）

- 装饰性分隔注释：`grep -rnE '──|═══|━━━|=====' ...` 无命中
- TODO/FIXME：无残留
- 函数 ≤ 50 行：所有新增函数符合（最长为 `list_items_with_images`，约 30 行）
- 嵌套 ≤ 3 层：符合
- 断言验具体值：验 `starts_with("data:image/webp;base64,")` 和 `starts_with("data:image/png;base64,")`，非恒真
- SQL ORDER BY 确定性：沿用 `rowid DESC` 兜底，与现有规范一致
- 安全：无硬编码密钥、无用户输入直接拼 SQL（参数化查询）
- 只做 IPC 层，未碰前端 React 组件
