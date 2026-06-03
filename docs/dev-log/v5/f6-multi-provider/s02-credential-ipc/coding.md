# coding — f6-multi-provider / s02-credential-ipc

## 任务
批次 B：凭据存取 IPC。后端 3 条凭据命令 + 注册，不碰前端（批次 C）。

## 改动文件
- `src-tauri/src/ipc/settings.rs`：新增 2 个 DTO、3 个纯 impl、3 个 Tauri 命令
- `src-tauri/src/lib.rs`：invoke_handler 追加 3 条新命令

## 真实签名（核对结果）

```
credential_schema(provider_id: &str) -> Vec<CredentialField>
  CredentialField { key: &'static str, label: &'static str, is_secret: bool, required: bool }

save_credentials(provider_id, values: &[(&str, &str)], store: &dyn CredStore, conn: &Connection)
  -> Result<(), CredError>

load_credentials(provider_id, store: &dyn CredStore, conn: &Connection)
  -> Result<Vec<(String, String)>, CredError>

CredStore trait: set_secret / get_secret
CredError: UnknownProvider / UnknownField / Keychain / Db
MockCredStore: #[cfg(test)] pub，内存 HashMap
```

## 最终签名

### DTO
```rust
pub struct CredentialFieldDto { key, label, is_secret, required }   // camelCase
pub struct CredentialValueDto { key, value: Option<String>, is_set } // camelCase; secret 字段 value=None
```

### 纯 impl
```rust
pub fn get_provider_credential_schema_impl(provider_id: &str) -> Vec<CredentialFieldDto>
pub fn get_provider_credentials_impl(provider_id, store: &dyn CredStore, conn) -> Result<Vec<CredentialValueDto>, String>
pub fn set_provider_credentials_impl(provider_id, values: HashMap<String, String>, store, conn) -> Result<(), String>
```

### Tauri 命令
```rust
pub fn get_provider_credential_schema(provider_id: String) -> Vec<CredentialFieldDto>
pub fn get_provider_credentials(state: State<AppDb>, provider_id: String) -> Result<Vec<CredentialValueDto>, String>
pub fn set_provider_credentials(state: State<AppDb>, provider_id: String, values: HashMap<String, String>) -> Result<(), String>
```

### values 入参决策
前端传 `Record<string,string>`，后端用 `HashMap<String, String>`；
impl 内 `.iter().map(|(k,v)| (k.as_str(), v.as_str()))` 转为 `&[(&str, &str)]` 对接 `save_credentials`。

## TDD 记录
- RED：追加 5 个测试到 settings.rs::tests，`cargo check` 返回 8 个 `E0425: cannot find function`
- GREEN：实现 2 DTO + 3 impl + 3 命令，`cargo check` exit 0；5 个测试各自单跑均 1 passed

### 测试覆盖
| 测试名 | 目标行为 |
|--------|----------|
| `get_provider_credential_schema_impl_baidu_returns_two_fields` | schema 返回正确字段数与 key |
| `get_provider_credential_schema_impl_unknown_returns_empty` | 未知 provider 返回空 Vec |
| `get_provider_credentials_impl_unset_fields_are_not_set` | 未存时 is_set=false，value=None |
| `get_provider_credentials_impl_secret_field_value_is_always_none` | 存后 secret value=None、is_set=true；非密 value=Some |
| `set_provider_credentials_impl_persists_and_loadable` | 保存后可 get 取回（secret 进 mock keychain） |
| `set_provider_credentials_impl_unknown_field_returns_err` | 未知 field_key 返回 Err |

## 安全实现点
- `get_provider_credentials_impl`：`if field.is_secret` 分支强制 `value: None`，不从 store 返回的 Option 赋值给前端
- 对应测试断言：`assert!(secret_field.value.is_none(), "secret 字段的 value 永远应为 None")`
- `set_provider_credentials_impl`：错误路径仅透传 `CredError::to_string()`，`CredError::UnknownField` 只含 provider 和 field 名，不含字段值（credential.rs 保证）

## 验收结果
- TDD 红/绿：确认
- `cargo test -p quickquick`：**336 passed**（+6 较批次 A 的 330）
- `cargo build -p quickquick`：exit 0（3 命令注册编译通过，Tauri 参数反序列化 OK）
- `cargo fmt -p quickquick --check`：exit 0
- `cargo clippy -p quickquick`：exit 0，无 warnings
- 无装饰性分隔注释、无 TODO/FIXME

## 改动范围确认
仅改动以下 2 个文件，不碰前端、translate.rs、docs 外其他文件：
- `src-tauri/src/ipc/settings.rs`
- `src-tauri/src/lib.rs`
