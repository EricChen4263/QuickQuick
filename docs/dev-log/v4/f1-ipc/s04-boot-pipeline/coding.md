# S04 启动数据管道 — 编码留痕

## 改动文件清单

| 文件 | 改动说明 |
|------|---------|
| `src-tauri/src/pipeline.rs` | 新建：两个纯函数 open_app_db / capture_and_ingest，依赖注入，可单测 |
| `src-tauri/src/lib.rs` | 注册 pub mod pipeline；register_hotkeys 改读持久化 hotkey.json；命令注册 invoke_handler；setup 接线 AppDb + 轮询线程 |
| `src-tauri/src/ipc/mod.rs` | 补全子模块文档注释（translate、settings） |
| `src-tauri/src/ipc/settings.rs` | I-01：修正 get_exclude_list_impl / get_selected_provider_impl Errors 文档 |
| `src-tauri/tests/boot_pipeline.rs` | 新建：4 个 boot_pipeline_* 集成测试（RED-GREEN 验证通过） |

## 关键实现决策

### pipeline.rs 纯函数注入设计
open_app_db 与 capture_and_ingest 均接受 trait 对象（&dyn KeyProvider / &dyn ClipboardBackend），
不持有全局状态。好处：headless 单测完全无 OS 依赖，生产接线只在 lib.rs setup 一处完成。

### ArboardBackend 哈希策略
change_count 用 FNV-1a 64-bit 稳定哈希（与 db.rs text_hash 一致的算法），哈希每次读取结果，
与上次哈希比对变化则内部计数+1。使用显式稳定算法，符合 code-standards §6「持久化键/哈希
用显式稳定算法」（此处虽为运行期内存值，但保持规范一致性）。

### 轮询线程方案
SQLCipher Connection 非 Send，采用 arc::Mutex 包裹的 AppDb state（Tauri manage 保证
跨线程），轮询线程内通过 app_handle.state::<AppDb>() 取得，每次迭代 lock 后操作，用后立即释放。

### register_hotkeys 持久化修复
改为先尝试从 app_config_dir()/hotkey.json 加载，文件不存在或读取失败时回退 default()，
使用户改键重启后生效。

### I-01 文档修正（仅注释）
get_exclude_list_impl / get_selected_provider_impl 的 Errors 文档改为真实描述：
实际调用 load_or_default 吞错，永不返回 Err。

### I-02 文档补全
ipc/mod.rs 子模块列表补 translate / settings；lib.rs 模块头补 ipc / settings / pipeline。

## 假设 / 未决（归 pending-manual）

以下行为无法在 headless CI 中自动验证，需手动 QA：

1. **真实 keychain 开库**：KeychainKeyProvider.get_or_create_key() 触及真实 macOS Keychain，
   首次运行弹出授权对话框，需手动验证成功取得密钥并打开 SQLCipher 库。
2. **arboard 真实捕获**：ArboardBackend.read() 读取真实系统剪贴板，需 GUI 环境。
   headless 无法测，不为其编写自动化测试。
3. **轮询线程存活验证**：背景线程 500ms 间隔持续运行，需通过 App UI 手动复制内容
   验证历史面板条目出现。
4. **命令真实 invoke 往返**：12 个 Tauri 命令需通过前端 invoke 验证序列化/反序列化正确。
5. **热键持久化读取**：register_hotkeys 修复后，需手动通过 UI 改键 → 重启 → 验证新热键生效。

## 实跑结论

```
test boot_pipeline_open_db ... ok
test boot_pipeline_no_change_none ... ok
test boot_pipeline_ingest_visible ... ok
test boot_pipeline_dedup_bumped ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

cargo check 通过（exit:0），arboard 依赖拉取成功。

## code-standards 自检

- [x] 格式：4 空格缩进，行宽 < 120，文件末换行
- [x] 函数：单一职责，open_app_db 13 行 / capture_and_ingest 16 行，均 << 50 行；参数 <= 4
- [x] 嵌套：最深 2 层（match + map_err），不超 3 层
- [x] 命名：open_app_db（动词+名词）、capture_and_ingest（动词+名词）；无无意义缩写
- [x] 注释：写"为什么"（为何 change_count 用 &mut、为何错误转 String）；无死代码
- [x] 类型：无 any/dynamic 逃逸；IngestOutcome 显式类型
- [x] 安全：密钥不入日志；不硬编码密钥（固定测试密钥仅在测试文件 FixedKeyProvider 内）
- [x] 持久化哈希：ArboardBackend 用 FNV-1a（与 db.rs 一致的显式稳定算法）
- [x] 无 TODO/FIXME 残留（grep 验证通过）
- [x] 无装饰性分隔注释（grep 验证通过）
- [x] 测试 AAA 结构；断言具体值（content == "hello pipeline"）；非恒真
- [x] TDD：先写测试（RED 因模块不存在编译失败）→ 实现（GREEN 4 passed）→ REFACTOR（无需）

## 修订 R1（I-1/I-2）

### 改动文件清单

| 文件 | 改动说明 |
|------|---------|
| `src-tauri/src/ipc/mod.rs` | `AppDb` 改为持有 `Option<Connection>`；新增 `pub fn with_db` helper |
| `src-tauri/src/ipc/clipboard.rs` | 3 个命令（list/delete/toggle_favorite）改用 `with_db`；import 补 `with_db` |
| `src-tauri/src/ipc/translate.rs` | 2 个命令（translate_text/list_translate_history）改用 `with_db`；import 补 `with_db` |
| `src-tauri/src/lib.rs` | `setup_app_db` 重构：无论开库成败都调用 `app.manage(AppDb(Mutex::new(...)))`；轮询线程适配 `Option`（None 时 continue）；修正注释 |
| `src-tauri/src/pipeline.rs` | `ArboardBackend::new()` 加 `#[must_use]` 标注 |
| `src-tauri/tests/ipc_clipboard.rs` | 追加 2 个 None 守卫测试：`ipc_clipboard_with_db_none_returns_db_unavailable_err` / `ipc_clipboard_with_db_some_executes_closure_ok` |

### Option 包裹方案（I-1）

原 `AppDb(pub Mutex<Connection>)` 改为 `AppDb(pub Mutex<Option<Connection>>)`。
`setup_app_db` 原先开库失败时提前 `return`，导致 `app.manage` 从未被调用，
Tauri dispatch 层在前端 invoke 时因状态未注册而 panic。
新方案：无论开库成功与否，均执行 `app.manage(AppDb(Mutex::new(conn_opt)))`，
成功时 `conn_opt = Some(conn)`，失败时 `conn_opt = None`（eprintln 记录原因）。

### with_db helper 签名

```rust
pub fn with_db<T>(
    db: &AppDb,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String>
```

lock → `as_ref().ok_or_else(|| "数据库不可用，请检查钥匙串授权或重启应用".to_string())` → 调 `f`。
5 个命令统一通过此函数处理 None，绝不 panic。

### 5 个命令改写清单

- `list_clip_items`：`with_db(&state, |conn| list_clip_items_impl(conn).map_err(...))`
- `delete_clip_item`：`with_db(&state, |conn| delete_clip_item_impl(conn, &id).map_err(...))`
- `toggle_favorite_clip`：`with_db(&state, |conn| toggle_favorite_clip_impl(conn, &id, favorite).map_err(...))`
- `translate_text`：`with_db(&state, |conn| translate_text_impl(conn, &UreqExecutor, &text, target.as_deref()))`
- `list_translate_history`：`with_db(&state, |conn| list_translate_history_impl(conn).map_err(...))`

纯函数 impl（接 `&Connection`）签名不变，复用不变。

### 新测试

- `ipc_clipboard_with_db_none_returns_db_unavailable_err`：构造 `AppDb(Mutex::new(None))`，断言 `with_db` 返回 `Err` 且含"数据库不可用"
- `ipc_clipboard_with_db_some_executes_closure_ok`：构造 `AppDb(Mutex::new(Some(conn)))`，断言 `with_db` 返回 `Ok`，列表为空

### 编译 / 测试结论

```
test ipc_clipboard_with_db_none_returns_db_unavailable_err ... ok
test ipc_clipboard_with_db_some_executes_closure_ok ... ok
test ipc_clipboard_list_dto_fields_complete ... ok
test ipc_clipboard_list_returns_live_items ... ok
test ipc_clipboard_toggle_favorite_puts_item_first ... ok
test ipc_clipboard_list_excludes_deleted_items ... ok
test ipc_clipboard_delete_removes_item_from_list ... ok
test ipc_clipboard_toggle_favorite_unset_restores_order ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

test boot_pipeline_open_db ... ok
test boot_pipeline_dedup_bumped ... ok
test boot_pipeline_no_change_none ... ok
test boot_pipeline_ingest_visible ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo build: Finished dev profile, EXIT 0
```

### code-standards 自检（R1）

- [x] 格式：4 空格缩进，无装饰性分隔注释（grep 验证）
- [x] 函数：`with_db` 8 行 << 50 行；单一职责
- [x] 嵌套：with_db 内最深 2 层（lock + ok_or_else），不超 3 层
- [x] 命名：`with_db`（介词短语，Rust 惯用）；`conn_opt` 明确含义
- [x] 注释：写"为什么"（为何始终 manage、为何 Option 包裹）；修正了与实际行为相悖的旧注释
- [x] 安全：无敏感信息入日志；无硬编码密钥
- [x] 无 TODO/FIXME 残留
- [x] 断言非恒真：None 守卫测试断言具体错误子串"数据库不可用"；Some 测试断言列表为空
- [x] TDD：RED（编译失败，unresolved import `with_db`）→ GREEN（8 passed）→ REFACTOR（无需）
- [x] I-2：`#[must_use]` 标注在函数级别，含说明消息，符合 Rust 惯例
