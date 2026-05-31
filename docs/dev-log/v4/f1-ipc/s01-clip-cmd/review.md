---
id: V4-F1-S01-review
type: review
level: 小功能
parent: V4-F1
children: []
created: 2026-05-31T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V4-F1-A01, V4-F1-A14]
author: code-reviewer
---

# 审查记录 · 剪贴板 IPC 命令层（V4-F1-S01）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/ipc/mod.rs` | 新建 | `AppDb(Mutex<Connection>)` 托管状态类型 |
| `src-tauri/src/ipc/clipboard.rs` | 新建 | `ClipItemDto` + 3 命令 + 3 impl + `validate_id` |
| `src-tauri/src/db.rs` | diff 部分 | 新增 `ClipItemRow` 与 `list_items_full` |
| `src-tauri/src/lib.rs` | 新增一行 | `pub mod ipc;` |
| `src-tauri/tests/ipc_clipboard.rs` | 新建 | impl 层集成测试（覆盖 V4-F1-A01） |
| `src-tauri/tests/ipc_validation.rs` | 新建 | 参数校验测试（覆盖 V4-F1-A14） |

参照：设计文档§九.3（剪贴板列表+管理）、V4-F1-A01/A14、code-standards。

---

## 问题清单

### Critical

无。

### Important

无（所有潜在关注点经静态分析后置信度均低于 80%，见下方逐项说明）。

---

## 逐维度核查

### 1. Mutex 锁中毒处理

`state.0.lock().map_err(|e| format!("锁获取失败: {e}"))`

中毒（PoisonError）时正确传播为 `Err(String)` 返回给前端，不 panic，不裸 `unwrap`。
`PoisonError<MutexGuard<Connection>>` 的 Display 输出为固定字符串 "poisoned lock: another task failed while holding a mutex"，不包含 Connection 内部状态或数据内容。三个命令均采用相同模式，一致。**通过。**

### 2. DbError→String 映射与敏感信息泄露

当前 IPC 调用链：
- `list_clip_items_impl` → `list_items_full` → `prepare`/`query_map` → 只可能产生 `DbError::Sqlite`
- `delete_clip_item_impl` → `soft_delete` → `execute` → 只可能产生 `DbError::Sqlite`
- `toggle_favorite_clip_impl` → `set_favorite` → `execute` → 只可能产生 `DbError::Sqlite`

`DbError::Corrupt`（含 `backup_path` 文件路径）和 `DbError::Io`（含系统路径）均不在以上链路中产生——这些变体只出现于 `open_or_create`/`open_or_recover`，而这三个 impl 函数均不调用开库函数。`DbError::Other` 在本层只有 `validate_id` 产生，消息为固定中文字符串"id 不能为空或全空白"，无敏感信息。`DbError::Sqlite` 的 Display 为 rusqlite 运行时错误码与 SQL 错误描述，连接已建立后的查询错误不含文件路径，且无密钥材料。**无敏感泄露风险，通过。**

### 3. `list_items_full` SQL 正确性

SQL 为静态字符串，无用户输入拼接：

```sql
SELECT id, content, kind, is_favorite, last_modified_utc
FROM clip_items
WHERE is_deleted = 0
ORDER BY is_favorite DESC, last_modified_utc DESC
```

- 过滤条件：`is_deleted = 0` 正确排除软删条目
- 排序：`is_favorite DESC, last_modified_utc DESC` 与设计文档"收藏优先，组内最近"语义一致
- 参数：无（`query_map([], ...)` 空参数绑定），无注入面
- **通过。**

### 4. `content` 列 NULL 处理

Schema 定义 `content TEXT`（可为 NULL）；`list_items_full` 以 `row.get::<_, Option<String>>(1)?.unwrap_or_default()` 处理，NULL 映射为空字符串 `""`，不报错。`ClipItemDto.content: String` 前端收到 `""` 为合法值。**通过。**

### 5. serde camelCase 与前端契约

`#[serde(rename_all = "camelCase")]` 作用下各字段序列化名：
- `id` → `id`
- `content` → `content`
- `kind` → `kind`
- `is_favorite` → `isFavorite`
- `last_modified_utc` → `lastModifiedUtc`

转换规则符合 Tauri 惯例，与 TypeScript 接口命名对齐。**通过。**

### 6. 命令名稳定性

三个 `#[tauri::command]` 函数名：`list_clip_items`、`delete_clip_item`、`toggle_favorite_clip`。均为 snake_case，与 Tauri 默认命令名生成规则一致，与验收标准 V4-F1-A01 引用一致。S04 注册时只需列入 `generate_handler![]`，名称无需变更。**通过。**

### 7. 错误路径无 panic

- 三个命令函数均使用 `?` 传播，无裸 `unwrap`/`expect`/`panic!`
- `validate_id` 使用 early return + Err，无 panic
- impl 函数传递 `&Connection`，不拿所有权，无资源泄露

**通过。**

### 8. 代码规范符合度

- 函数长度：最长 `list_clip_items_impl` 约 14 行，所有函数 ≤ 50 行
- 嵌套：最深 2 层（符合 ≤ 3 层要求）
- 命名：`validate_id`（动词+名词）、`is_favorite`（is 前缀布尔）、无 `tmp`/`flag` 类名
- 注释：均写"为什么"，无装饰性横线分隔符，无死代码注释
- 无 TODO/FIXME 遗留
- **通过。**

### 9. 测试质量

`ipc_clipboard.rs`：5 个测试覆盖 list/delete/toggle_favorite 主路径、软删过滤、排序变化、DTO 字段完整性；使用真实临时 SQLite 库（`open_or_create` + `tempdir`），非恒真断言。

`ipc_validation.rs`：6 个测试覆盖空串/全空白/制表符换行混合/合法不存在 id 四类边界，断言正确（Err vs Ok）。

覆盖 V4-F1-A01 和 V4-F1-A14 全部断言要求。tester 已动态证伪通过（含 SQL 注入安全边界）。**通过。**

---

## 低于阈值的观察项（不阻断，备忘）

**validate_id 传入值不做 trim 后去空格再转发**（置信度约 65%）

`validate_id` 用 `id.trim().is_empty()` 做守卫，但通过校验后把原始（含前后空格的）`id` 传给 SQL 参数。若前端意外发送 `" some-uuid "` 这类含空格 id，校验通过但 SQL 匹配行数为 0——操作静默成功（`Ok(())`），对前端而言看起来正常但实际无效果。因 UUID 规范本身不含空格，且 id 来源为 DB 自生成，实际命中概率极低，不构成高危。如 S04 集成阶段前端有规范化处理则更无忧。

---

## 对 S04 / 前端的注意事项

1. **S04 注册**：`invoke_handler` 需列入 `list_clip_items`、`delete_clip_item`、`toggle_favorite_clip` 三个命令（名称已固定，本次不变）；`app.manage(AppDb(...))` 在 `setup` 闭包内完成，`AppDb` 须在注册前 manage 到 app 实例。

2. **前端 TypeScript 接口**：DTO 字段对应 `{ id: string; content: string; kind: string; isFavorite: boolean; lastModifiedUtc: number }`（camelCase）。`content` 可能为空字符串（NULL 行），前端渲染时需处理空内容展示。

3. **错误处理**：三个命令在 Rust 侧均返回 `Result<_, String>`，Tauri 会将 `Err(String)` 映射为 JS 端 reject。前端 IPC 封装层（S05）应捕获 reject 并做有意义的用户提示，不应裸抛 unknown。

4. **锁粒度**：当前 `AppDb` 是全局单 Mutex，三个命令串行持锁。S04 的捕获回调（`ingest`）也会持同一把锁，在高频捕获时可能短暂排队。如未来出现性能问题，可考虑连接池；当前阶段单连接可接受。

---

## 总结论

**无未决高危，放行。**

所有置信度 ≥ 80% 的检查项均通过。代码遵循"命令薄包装 + 可单测 impl" 模式清晰，Mutex 锁中毒处理正确，DbError→String 映射在当前调用链无敏感路径泄露，SQL 全静态无注入面，NULL content 有明确处理，camelCase 序列化与前端契约对齐，验证逻辑在边界层强制，测试覆盖主路径与安全边界，无 TODO/FIXME。

V4-F1-A01、V4-F1-A14 审查维度通过。可进入 S02（translate-cmd）。
