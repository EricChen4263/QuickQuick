# S01 剪贴板 IPC 命令层 — 编码留痕

版本：V4 / F1-IPC / S01-clip-cmd
实现者：coder agent（claude-sonnet-4.6）
完成时间：2026-05-31

## 一、改动文件清单

| 文件路径 | 说明 |
|---|---|
| `src-tauri/src/db.rs` | 新增 `ClipItemRow` 结构体与 `list_items_full()` 函数（返回带 content/kind 的完整行） |
| `src-tauri/src/ipc/mod.rs` | 新建 IPC 模块入口，声明 `AppDb` 托管状态类型 |
| `src-tauri/src/ipc/clipboard.rs` | 新建剪贴板 IPC 命令层：`ClipItemDto`、三个命令函数（`#[tauri::command]`）、三个可单测的 impl 函数、`validate_id` 校验 |
| `src-tauri/src/lib.rs` | 加入 `pub mod ipc;` |
| `src-tauri/tests/ipc_clipboard.rs` | 新建 A01 集成测试（6 个 `ipc_clipboard_*` 函数） |
| `src-tauri/tests/ipc_validation.rs` | 新建 A14 剪贴板部分集成测试（6 个 `ipc_input_validation_*` 函数） |

## 二、关键实现决策

### 2.1 命令层 / impl 层分离

每个命令 = `#[tauri::command]` 薄包装 + 可单测的纯函数 `xxx_impl(conn: &Connection, ...)`。
单测直接传临时库连接，不需要 Tauri 运行时，测试快且可靠。命令层只做 lock + DbError → String 映射。

### 2.2 新增 `list_items_full` 而非修改 `list_ordered`

`list_ordered` 只返回 `ClipRow`（id/is_favorite/last_modified_utc），不含 content/kind，已有下游测试依赖其签名。
增加新函数 `list_items_full` 返回 `ClipItemRow`（含全量字段）保持向后兼容，不破坏既有测试。

### 2.3 `validate_id` 在 IPC 层做，不在 db 层做

db 层语义是「执行 SQL」——id 不存在时 SQL 影响行数为 0 但不报错，这是正常行为。
输入合法性校验属于 API 契约（边界强制），放在 IPC 命令层的 impl 函数入口，保持 db 层职责单一。

### 2.4 `AppDb` 声明在 `ipc/mod.rs`，开库注册留 S04

`AppDb(Mutex<Connection>)` 仅声明类型。`app.manage(AppDb(...))` 与 `invoke_handler` 注册留给 S04 启动管道，避免与后续小功能的管道初始化产生竞争或重复。

### 2.5 DTO camelCase 序列化

`ClipItemDto` 使用 `#[serde(rename_all = "camelCase")]`，字段名与前端 TypeScript 接口对齐（S05 会消费这些 DTO）。

## 三、假设与未决项

- **开库与 manage 注册留 S04**：`AppDb` 已声明，但 `app.manage(AppDb(conn))` 与 `invoke_handler![]` 注册由 S04 完成。S01 的命令函数已可直接被 S04 引用。
- **kind 字段当前仅有 "text"**：图片类型由后续迭代处理；`ClipItemRow.content` 对 NULL 做了 `unwrap_or_default()`（content 列 schema 允许 NULL）。
- **S04 注册时命令名固定**：`list_clip_items` / `delete_clip_item` / `toggle_favorite_clip`，前端 S05 按此名 invoke，不得改动。

## 四、code-standards 自检

| 项目 | 结果 |
|---|---|
| 装饰性分隔注释 | 无（grep 验证） |
| 函数 ≤ 50 行 | 全部符合（最长 `list_clip_items_impl` 约 14 行） |
| 嵌套 ≤ 3 层 | 符合，无深嵌套 |
| 注释写「为什么」 | 每个函数文档注释说明设计理由 |
| 无 TODO/FIXME | grep 验证无残留 |
| 错误用 Result 不 panic | 全部用 `Result`，validate_id 返回 Err |
| 不打印敏感信息 | 命令层无 println!/eprintln!，DbError 映射只转字符串不含密钥 |
| SQL 参数化查询 | 复用 db.rs 既有参数化函数，IPC 层无裸拼 SQL |
| 命名描述性 | 函数用「动词+名词」，布尔参数用 `favorite` 语义明确 |
| 单一职责 | clipboard.rs 只做 IPC 命令层；db.rs 只做持久化；各层职责清晰 |
| 全量测试无回归 | cargo test 全部通过（EXIT:0，所有 test result: ok） |

## 五、测试结果摘要

### A01（ipc_clipboard）
```
test result: ok. 6 passed; 0 failed; 0 ignored
```
命中函数：
- `ipc_clipboard_list_returns_live_items`
- `ipc_clipboard_list_excludes_deleted_items`
- `ipc_clipboard_delete_removes_item_from_list`
- `ipc_clipboard_toggle_favorite_puts_item_first`
- `ipc_clipboard_toggle_favorite_unset_restores_order`
- `ipc_clipboard_list_dto_fields_complete`

### A14（ipc_input_validation，剪贴板部分）
```
test result: ok. 6 passed; 0 failed; 0 ignored
```
命中函数：
- `ipc_input_validation_delete_empty_id_returns_err`
- `ipc_input_validation_delete_whitespace_id_returns_err`
- `ipc_input_validation_toggle_favorite_empty_id_returns_err`
- `ipc_input_validation_toggle_favorite_whitespace_id_returns_err`
- `ipc_input_validation_delete_valid_nonexistent_id_passes_validation`
- `ipc_input_validation_toggle_favorite_valid_id_passes_validation`

## 打回修复 R2（producer V4-A-TESTS flaky 排序）

修复时间：2026-06-01
修复者：coder agent（claude-sonnet-4.6）

### 根因

`list_items_full` 与 `list_ordered` 的 ORDER BY 均为 `is_favorite DESC, last_modified_utc DESC`，缺少确定性兜底。全量并发测试下，同一测试内连续两次 `ingest` 调用的 `current_utc_ms()` 易落入同一毫秒，导致两行 `last_modified_utc` 并列，SQLite 在并列时返回顺序不定。`ipc_clipboard_toggle_favorite_puts_item_first` 断言"最新 ingest 的条目排第一"，约 1/4 概率偶发 FAILED。单独跑 `--test ipc_clipboard` 因无并发压力、每次 ingest 间距够大，稳定通过。与 V4-F1-A02 翻译历史倒序同毫秒并列是同一类根因。

### 修法

`src-tauri/src/db.rs` 中两处 ORDER BY 均追加 `, rowid DESC` 兜底：

- `list_items_full`：`ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC`
- `list_ordered`：`ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC`

`rowid` 是 SQLite 隐式单调递增整数主键，同一毫秒并列时最后插入的行 rowid 最大，排在最前，与"最新条目排最前"语义完全一致。不改测试、不加 serial_test 串行化，改的是生产排序确定性。

### 连跑 5 次全量测试结论

| 轮次 | exit | FAILED |
|---|---|---|
| run1 | 0 | 无 |
| run2 | 0 | 无 |
| run3 | 0 | 无 |
| run4 | 0 | 无 |
| run5 | 0 | 无 |

全量测试每次 5 个 test binary 共 115 个测试（5+32+10+67+1）全部通过，无 FAILED。

ipc_clipboard 集成测试单独连跑 3 次，每次 8 passed（含 `ipc_clipboard_toggle_favorite_puts_item_first`），exit 0。

clippy `--all-targets -D warnings` exit 0，无新 warning。
