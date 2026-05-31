# V4-F1-S02 翻译 IPC 命令 — 编码留痕

## 改动文件清单

| 文件 | 改动说明 |
|---|---|
| `src-tauri/Cargo.toml` | 新增 `ureq = { version = "2", features = ["tls"] }` 依赖 |
| `src-tauri/src/translate/history.rs` | 新增 `list_translate_history(conn) -> Result<Vec<TranslateHistoryRow>, DbError>` 与 `TranslateHistoryRow` 结构体 |
| `src-tauri/src/ipc/translate.rs` | 新建：`HttpExecutor` trait、`UreqExecutor`（生产）、`FakeExecutor`（测试）、`translate_text_impl`、`list_translate_history_impl`、`#[tauri::command] translate_text`、`#[tauri::command] list_translate_history`、DTO 类型 |
| `src-tauri/src/ipc/mod.rs` | 新增 `pub mod translate;` 注册子模块 |
| `src-tauri/tests/ipc_translate.rs` | 新建集成测试（6 个函数名含 `ipc_translate`） |

## 关键实现决策

### 1. 执行器注入（HttpExecutor trait）
放在 `ipc/translate.rs` 而非 `translate/` 下，因为这是 IPC 层关注点——翻译 provider 契约（`TranslateProvider` trait）已在 `translate/` 定义，HTTP 执行器是 IPC 编排层与 provider 之间的胶水，属于 IPC 层职责。

### 2. ureq 选同步版本
SQLCipher `Connection` 不是 `Send`，跨 `await` 持有会导致编译错误。选同步 `ureq`（无 Tokio 依赖）完全规避 async 运行时与 Mutex<Connection> 的复杂度。

### 3. FakeExecutor 用 AtomicU32 计数
`FakeExecutor` 以 `&dyn HttpExecutor` 传入 impl 函数，计数器须通过共享引用可读。用 `Arc<AtomicU32>` 使 `call_count()` 在 FakeExecutor 被引用借走后仍可读取——无需 `Mutex`，开销最小。

### 4. 导入别名避免名字冲突
`history::list_translate_history` 与 Tauri 命令函数 `list_translate_history` 同名，用 `as db_list_translate_history` 别名解决冲突，不改下层函数名（下层命名已是最准确的描述）。

### 5. 方向编排复用既有 resolve_direction
直接调 `lang::resolve_direction(text, configured_target)` 得 `(source, target)`，不重复实现检测逻辑（DRY）。

### 6. 默认 provider 固定为 mymemory
S02 范围内只支持默认 provider（mymemory，无需 API Key）。多 provider 选择由后续小功能实现。

## 假设 / 未决

- **真实 UreqExecutor 网络往返需运行确认（manual）**：headless CI 无法发真实 HTTP，需手动运行 App 验证 mymemory API 可达、响应可解析。并入 pending-manual 列表。
- `configured_target` 目前签名为 `Option<&str>`，IPC 命令层入口为 `Option<String>`；若后续 S03/S04 需要持久化用户语言偏好，impl 签名不需改动（调用方负责转换）。
- `list_translate_history` 无分页，当前返回全量历史。条目数量大时需后续加 LIMIT/OFFSET（YAGNI，先实现最小可交付）。

## 验收实跑结论

```
test ipc_translate_empty_text_returns_error_without_calling_executor ... ok
test ipc_translate_whitespace_text_returns_error_without_calling_executor ... ok
test ipc_translate_chinese_text_produces_zh_to_en_direction ... ok
test ipc_translate_english_text_produces_en_to_zh_direction ... ok
test ipc_translate_writes_to_history_after_success ... ok
test ipc_translate_list_history_returns_entries_in_desc_order ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

命中子串：`ipc_translate`（6 个测试函数名全含该子串）。

## code-standards 自检

| 检查项 | 结论 |
|---|---|
| 格式：缩进/行宽/引号/分号/空行 | 4 空格缩进（Rust 惯例），行宽 ≤120，文件末尾有换行 |
| 函数：单一职责、≤50 行、参数≤5、嵌套≤3 层 | 最长函数 `translate_text_impl` 约 35 行；`execute` 约 30 行；嵌套最深 2 层 |
| 命名：描述性、动词+名词、布尔前缀 | `translate_text_impl`、`list_translate_history_impl`、`db_list_translate_history` 均符合 |
| 注释：写「为什么」、无死代码、公共接口有文档 | `///` 文档注释覆盖所有公共接口，注释说明选型原因（ureq 同步/AtomicU32） |
| 类型：无魔术值、公共接口显式类型、无逃逸类型 | DTO 用具名结构体，无 `any`/裸 `object` |
| 性能：无多余字段查询 | SQL `SELECT` 列举所有需要字段，无 `SELECT *` |
| 测试：AAA 结构、行为化命名 | 6 个测试均含行为描述命名；每个测试明确 Arrange/Act/Assert 三段 |
| 提交：Conventional Commits 前缀 | 待提交（Phase 5 完成后由主流程提交） |
| 安全：无密钥入库、参数化查询、输入校验、日志不泄密 | `add_translate_history` 参数化查询；`text` 空值在 IPC 层校验；`UreqExecutor` 无日志打印；API Key 不经过本模块 |
| 装饰性分隔注释 | 无 `═══/───/━━━/=====` 横线分隔 |
| TODO/FIXME 残留 | 无 |

## 打回修复 R2（producer V4-F1-A02 假绿）

### 根因

`list_translate_history` 的 SQL 查询仅有 `ORDER BY created_utc DESC`，当同一毫秒内连续插入多条记录时（测试用 in-memory SQLite 运行速度极快，多条插入往往落在同一毫秒），`created_utc` 值相等，SQLite 回退到未定义的自然存储顺序（通常为插入正序），导致 `history[0].source_text` 拿到最早插入的 "你好" 而非最晚插入的 "世界"，断言失败。

此前假绿的原因：偶发情况下 SQLite 在无次级排序时恰好以插入逆序返回，导致测试有时通过；producer 重跑时失败暴露了不确定性。

### 修法

将查询的 `ORDER BY` 子句由：

```sql
ORDER BY created_utc DESC
```

改为：

```sql
ORDER BY created_utc DESC, rowid DESC
```

`rowid` 是 SQLite 内置的单调递增行标识符（每次 INSERT 分配，严格递增，即使无显式 INTEGER PRIMARY KEY 也存在），同毫秒并列时按 `rowid DESC` 兜底，保证最后写入的行始终排最前，消除不确定性。

改动文件：`src-tauri/src/translate/history.rs`，第 99 行。

### 为什么此前是假绿

集成测试 `ipc_translate_list_history_returns_entries_in_desc_order` 连续插入 "你好" 和 "世界" 后调 `list_translate_history_impl`。在 in-memory SQLite 上两次插入几乎必然落在同一毫秒，`created_utc` 完全相同。原查询无次级排序，SQLite 返回顺序由内部实现决定，恰好在部分运行中以逆序返回而非正序，造成测试偶发通过（假绿）。producer 重跑暴露失败：5 passed / 1 failed。

### 实跑结论

```
test ipc_translate_list_history_returns_entries_in_desc_order ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

clippy（-D warnings）exit 0，无 error/warning。全量 cargo test exit 0，所有 test result: ok。
