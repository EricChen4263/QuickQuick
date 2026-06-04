---
id: V5-F1-S03-review
type: review
level: 小功能
parent: V5-F1
children: []
created: 2026-06-01T10:00:00Z
status: 通过
commit: 977d361
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 图片剪贴板 IPC 暴露层（V5-F1-S03）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/Cargo.toml` | diff | `[dependencies]` 新增 `base64 = "0.22"` |
| `src-tauri/src/db.rs` | diff | 新增 `ClipItemRowWithImage` 结构体 + `list_items_with_images` 函数；新增 `ingest_image_as_clip` + `insert_image_clip` + `try_insert_image_clip` + T1/T2/T3/T4 |
| `src-tauri/src/ipc/clipboard.rs` | diff | `ClipItemDto` 加 `thumbnail_data_url`/`image_id`；`list_clip_items_impl` 改用 `list_items_with_images` 并编码 WebP data URL；新增 `get_clip_image_original_impl` + `get_clip_image_original` 命令 + 3 个单测 |
| `src-tauri/src/lib.rs` | diff | `generate_handler!` 注册 `get_clip_image_original` |

参照：Rust 规范（函数≤50行/SQL参数化/ORDER BY确定性/Result别panic/注释写为什么/禁装饰线/禁TODO/错误处理完整）、code-standards、项目规范。

---

## 问题清单

### Critical

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| **Critical** | `get_clip_image_original_impl` 对降级图（`original_present=0`）返回语义错误的 data URL，前端无法区分。降级图写库时 `original` 列存空 BLOB（`b""`/`X''`），`get_image_original` 对此返回 `Ok(Some(vec![]))`（非 `None`）。函数对 `Some(bytes)` 一律做 base64 编码，产生 `"data:image/png;base64,"`（空 base64），前端以为有原图但渲染失败。`ClipItemDto` 未暴露 `original_present` 字段，前端无法提前判断原图是否可用，只能在调用 `get_clip_image_original` 拿到 `Some` 时发现图片无效。 | `src-tauri/src/ipc/clipboard.rs:152-157` | 规范：错误处理完整；函数语义清晰。**修复方案（二选一）**：① 简单修复：在 `Some(bytes)` 分支加 `if bytes.is_empty() { return Ok(None); }`，与文档注释"未找到时返回 `Ok(None)`"的精神一致（降级图语义上等同于无原图）；② 根本修复：修改 `get_image_original`（`image.rs:276`）的 SQL 加 `AND original_present = 1`，使降级行返回行数为 0，`optional()` 自然返回 `None`，调用方零修改。推荐②，一处修复，全调用方受益。 |

**置信度：88**

分析：`ingest_image_with_policy`（image.rs:177-181）超大图路径写入 `stored_original = b""`（空切片），`strip_original`（image.rs:391）后续降级写 `X''`（等效空 BLOB）。rusqlite 0.32 中 `row.get::<_, Vec<u8>>` 对空 BLOB 调用 `as_blob()` 返回 `Ok(&[])`，最终 `to_vec()` 得 `Ok(vec![])`；`.optional()` 包裹的是 `QueryReturnedNoRows` 错误，对找到行但 BLOB 为空的情况不返回 `None`。当前测试（T2 用 2×2 PNG，远小于 20MB 阈值）不覆盖超大图场景，不能暴露此问题。

---

### Important

（无高置信度 Important 级问题）

---

## 低于阈值的观察项（不阻断，备忘）

**重复 `use base64::Engine` 导入**（置信度约 75%）

`src-tauri/src/ipc/clipboard.rs` 行 12 有模块级 `use base64::Engine;`，`get_clip_image_original_impl` 函数体内（行 149）再次 `use base64::Engine;`。Rust 允许函数作用域内重声明 trait，编译器去重不报错，但存在冗余。若 clippy 开启 `unused_imports` 检查，可能报 warning（实际取决于 clippy 配置）。不阻断，可顺手删除函数内那条 `use`。

**`Ok(outcomes)` 绑定后立即 `let _ = outcomes`**（置信度约 65%）

`lib.rs:173-176` 写法功能正确，但可直接写 `Ok(_)` 忽略返回值，略繁琐。不影响行为，不阻断。

**`list_items_with_images` LEFT JOIN 无 UNIQUE(clip_item_id) 防护**（置信度约 70%）

`clip_images` 表无 `UNIQUE(clip_item_id)` 约束，若未来数据异常导致一个 `clip_item_id` 对应多张未软删 clip_images 行，LEFT JOIN 会产生重复 clip_items 行。当前业务逻辑（去重入库）保证 1:1，实际不会触发。不阻断。

---

## 逐维度核查

### 1. get_image_original 对降级图的处理（见 Critical 项）

`get_image_original` SQL 未过滤 `original_present=0`；`get_clip_image_original_impl` 未对空字节做守卫。需修复。

### 2. base64 编码正确性

- STANDARD engine（RFC 4648）用于 data URL，正确。
- 缩略图用 `data:image/webp;base64,{b64}` 前缀（`thumbnail` 由 `make_thumbnail` 生成 WebP），MIME 正确。
- 原图用 `data:image/png;base64,{b64}` 前缀（`png_bytes` 以 PNG 格式写入，不做格式转换），MIME 正确。
- 缩略图 BLOB 小（WebP 压缩后通常 <100KB），base64 编码无显著性能隐患。原图最大 20MB（`DEFAULT_MAX_ORIGINAL`），base64 膨胀约 33% 至 ~27MB，在内存中一次性编码较重，但这是有意的设计取舍（超大图已被 strip，不会进入该路径）。**通过。**

### 3. list_items_with_images SQL

- LEFT JOIN 条件 `cimg.clip_item_id = ci.id AND cimg.is_deleted = 0`：把软删过滤下推到 JOIN 条件而非 WHERE，文本条目不会因此被过滤掉。**正确。**
- 排序：`ORDER BY ci.is_favorite DESC, ci.last_modified_utc DESC, ci.rowid DESC`，三级有确定性兜底（rowid DESC）。**通过。**
- 所有参数化：SQL 无参数（全量查询），无注入风险。**通过。**
- 文本条目 `thumbnail`/`image_id` 为 NULL 时，`row.get::<_, Option<Vec<u8>>>(6)` 和 `row.get::<_, Option<String>>(5)` 对 NULL 返回 `Ok(None)`，映射正确。**通过。**

### 4. DTO 一致性

- `#[serde(rename_all = "camelCase")]` 将 `thumbnail_data_url` 序列化为 `thumbnailDataUrl`，`image_id` 为 `imageId`。与命令文档注释一致。**通过。**
- `get_clip_image_original` 命令参数 `image_id: String`，Tauri 前端 invoke 传 `image_id` 的 snake_case；Tauri 命令参数默认 camelCase 解析，实际前端需传 `imageId`。这是 Tauri 框架的标准行为，不是 bug。**通过。**

### 5. list_items_full 保持不变

`db::list_items_full` 函数未被修改，`boot_pipeline.rs` 和 `ipc_clipboard.rs` 集成测试中的调用路径不受影响。**通过。**

### 6. 函数行数

| 函数 | 行数 | 规范限制 |
|---|---|---|
| `list_items_with_images` | ~43 | ≤50 |
| `ingest_image_as_clip` | ~30 | ≤50 |
| `insert_image_clip` | ~34 | ≤50 |
| `try_insert_image_clip` | ~45 | ≤50 |
| `get_clip_image_original_impl` | ~10 | ≤50 |
| `list_clip_items_impl`（改动后） | ~20 | ≤50 |

均在规范内。**通过。**

### 7. 注释质量

注释均描述"为什么"（`// 查是否已有同 hash 的未软删图片行`，`// clip_item_id 为 NULL（孤立行）或无命中：走新建路径`，`// 补写外键（ingest_image_with_policy 不写 clip_item_id 字段）`，`// 整体回滚，不留孤立 clip_items 行`）。无装饰性横线，无 TODO/FIXME，无死代码注释。**通过。**

### 8. 错误处理

生产代码无 `unwrap()`/`panic!`，全部 `?` 传播。`bump_to_top` 不检查 UPDATE 影响行数（预存在行为，非本次新增）。测试辅助使用 `.expect()`，允许。**通过（降级图问题已在 Critical 项单独列出）。**

### 9. 测试质量

**db.rs T1–T4（新增）：**
- T1：首次入库 → Inserted，行数精确断言，clip_item_id 非 NULL 指向正确 item。非恒真，有效。
- T2：同 PNG 二次调用 → Bumped，行数不变，`last_modified_utc >= 首次值`（sleep 5ms 后断言合理，注释说明相等合法）。有效。
- T3：非法 PNG → Err，SAVEPOINT 回滚，两表均 0 行。有效。
- T4：手动制造孤立行 → 二次入库 → Inserted，clip_images 仍 1 行，clip_item_id 已补写新 item_id。新增，有效，弥补 S01 审查提出的 S1 建议。

**ipc/clipboard.rs T1–T3（新增）：**
- T1：图片 DTO `thumbnailDataUrl` 以 `data:image/webp;base64,` 开头，`imageId` 非 None；文本 DTO 两字段 None。有效，验证联表路径与 base64 编码。
- T2：存在的 image_id → `Some`，以 `data:image/png;base64,` 开头。仅测正常路径；**未覆盖降级图（original_present=0）场景**，与 Critical 问题对应。
- T3：不存在 id → `Ok(None)`。有效。

---

## 结论

**打回（必改 1 项）**

### 必改项

**M1（对应 Critical，置信度 88）**：修复降级图（`original_present=0`）被 `get_clip_image_original_impl` 返回为语义错误的空 data URL 的问题。

推荐修复路径：修改 `src-tauri/src/image.rs` 中 `get_image_original`（行 276-285）的 SQL，加 `AND original_present = 1` 过滤条件，使降级行不被查出，`optional()` 自然返回 `None`，调用方行为自动正确：

```rust
"SELECT original FROM clip_images WHERE id = ?1 AND is_deleted = 0 AND original_present = 1"
```

同时在 `get_image_original` 的 doc-comment 补充说明"降级图（`original_present=0`）返回 `None`"，并为降级场景补写单测（T3 级别：写入超大图 → `get_image_original` 返回 `None` → `get_clip_image_original_impl` 返回 `None`）。

备选修复（不修改 image.rs 时）：在 `get_clip_image_original_impl`（`src-tauri/src/ipc/clipboard.rs:152`）的 `Some(bytes)` 分支加守卫：

```rust
Some(bytes) if !bytes.is_empty() => {
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:image/png;base64,{b64}")))
}
Some(_) => Ok(None),  // 降级图（original_present=0），original 为空 BLOB
```

---

## 复审（commit 977d361）

| 初审问题 | 修复（已核实位置 文件:行号） |
|---|---|
| M1 Critical：get_clip_image_original_impl 对降级图（original_present=0，空 BLOB）返回语义错误的空 data URL | 已修复（备选方案）。`src-tauri/src/ipc/clipboard.rs` 约 151-157 行：`Some(bytes)` 分支加 `if !bytes.is_empty()` 守卫（`Some(bytes) if !bytes.is_empty() => { ... }`），空 BLOB 与 `None` 均走 `_ => Ok(None)` 分支，注释说明"None 或空 BLOB（降级图 original_present=0 时写入 X''）均视为无原图，前端回退缩略图"。 |

终态：通过
