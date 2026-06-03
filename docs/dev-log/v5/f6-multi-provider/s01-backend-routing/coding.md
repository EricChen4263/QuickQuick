# coding — f6-multi-provider / s01-backend-routing

## 任务
批次 A：后端动态路由。按 selected_provider 动态构造对应 provider 并注入凭据，替换硬编码 MyMemory。

## 改动文件
- `src-tauri/src/translate/providers.rs`：新增 `build_provider` 纯函数
- `src-tauri/src/translate/credential.rs`：提升 `MockCredStore` 为模块级 `#[cfg(test)] pub`
- `src-tauri/src/ipc/settings.rs`：`resolve_config_path` 改 `pub(crate)`
- `src-tauri/src/ipc/translate.rs`：`translate_text_impl` 扩展签名，动态路由，命令层传 settings_path + KeyringCredStore
- `src-tauri/tests/ipc_translate.rs`：更新集成测试适配新签名，本地定义 `LocalMockCredStore`

## TDD 记录
- RED 1：`build_provider` 8 测试，`E0425: cannot find function`（功能未实现）
- GREEN 1：实现 `build_provider`，8 测试全绿
- RED 2：`translate_text_impl` 5 新测试 + 3 旧测试，`E0061: 5 arguments but 7`
- GREEN 2：扩展签名 + 动态实现，5 测试全绿
- 补测（tester 打回闭合）：原测试只验 selected=mymemory→id==mymemory（恒等盲区，改回硬编码也过）。补 `translate_text_impl_selected_deepl_free_writes_deepl_provider_id_in_history`（deepl_free + MockCredStore 预置 auth_key + FakeExecutor → SQL 断言历史 provider_id=="deepl_free"），实证「改回硬编码→变红」。全量 329→330。纯补测，生产逻辑零改。

## 最终签名
```rust
pub fn build_provider(
    provider_id: &str,
    credentials: &[(String, String)],
) -> Result<Box<dyn TranslateProvider>, String>

pub fn translate_text_impl(
    conn: &Connection,
    exec: &dyn HttpExecutor,
    text: &str,
    configured_source: Option<&str>,
    configured_target: Option<&str>,
    settings_path: &Path,
    cred_store: &dyn CredStore,
) -> Result<TranslateResultDto, String>
```

## 验收结果

- `cargo test`：329 passed，0 failed
- `cargo build`：exit 0
- `cargo fmt -p quickquick --check`：exit 0
- `cargo clippy -p quickquick`：No issues found，exit 0
- `cargo test ipc_translate`：6 passed（集成测试子串真命中）
- `cargo test build_provider`：8 passed
- `cargo test translate_text_impl`：5 passed
- 无装饰性分隔注释、无 TODO/FIXME 残留
- 断言验具体值，无旁路

## 改动范围确认
仅改动以下文件（不含前端、lib.rs invoke_handler、docs 外的其他文件）：
- `src-tauri/src/translate/providers.rs`
- `src-tauri/src/translate/credential.rs`（仅放开 MockCredStore 至模块级，不改逻辑）
- `src-tauri/src/ipc/settings.rs`（仅 resolve_config_path 改 pub(crate)）
- `src-tauri/src/ipc/translate.rs`
- `src-tauri/tests/ipc_translate.rs`
