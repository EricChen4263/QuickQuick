# S03 设置 IPC 命令 — 编码留痕

## 改动文件清单

| 文件 | 改动说明 |
|------|----------|
| `src-tauri/src/settings.rs` | 新建：`AppSettings` 结构体 + `load/save/load_or_default`，镜像 autostart.rs 持久化模式 |
| `src-tauri/src/lib.rs` | 新增 `pub mod settings;` 声明 |
| `src-tauri/src/ipc/mod.rs` | 新增 `pub mod settings;` 声明 |
| `src-tauri/src/ipc/settings.rs` | 新建：7 个命令的 impl 函数 + Tauri command 薄包装 + DTO 类型 |
| `src-tauri/tests/ipc_settings.rs` | 新建：7 个集成测试（函数名含 `ipc_settings`，覆盖所有验收路径） |

## 关键实现决策

### 1. impl 函数统一返回 `Result<_, String>`

所有 impl 函数（`get_hotkeys_impl`、`set_hotkey_impl`、`get_exclude_list_impl` 等）错误类型统一为 `String`，与 S01/S02 既有 impl 函数风格保持一致。  
内部 `HotkeyError` / `SettingsError` 在 impl 层就调 `.map_err(|e| e.to_string())` 转换，命令层直接复用，无需再次 map。

### 2. `SystemHotkeyRegistrar` 用 `is_registered` 做冲突检测

`tauri_plugin_global_shortcut::GlobalShortcut::is_registered()` 返回 `bool`（不是 Result）。  
生产 registrar 用此做只读冲突检测，不实际注册回调（回调在 lib.rs setup 阶段统一绑定，避免遗留无回调孤立注册）。

### 3. 配置路径解析留在命令层

`resolve_config_path(app, filename)` 只在命令函数中调用，是不可单测的胶水层。  
所有 impl 函数接收显式 `&Path` 参数，保证纯函数可单测性。

### 4. `AppSettings` 镜像 autostart.rs 模式

`save(path)` + `load(path)` + `load_or_default(path)` 三件套，文件不存在时静默回退默认值。  
`excluded_apps` 和 `selected_provider` 共存于同一文件（`settings.json`），set 任一字段时 load 现有状态再局部更新，保证两字段互不覆盖。

### 5. 冲突拒绝语义

`set_hotkey_impl` 调 `rebind()`，冲突时 `HotkeyError::AlreadyInUse` 的 Display 文本含"已被占用"，`.to_string()` 后透传给前端，测试直接用 `.contains("已被占用")` 断言。

### 6. provider id 校验在 impl 层

`set_selected_provider_impl` 先查 `registry()` 校验 id 合法性，非法则直接返回 Err，不触发文件写。校验逻辑在 impl 层而非命令层，使测试可以不通过 Tauri 验证边界条件。

## 假设 / 未决事项

- **热键冲突检测局限性**：生产 `SystemHotkeyRegistrar` 用 `is_registered` 只能检测当前进程已注册的热键；若热键被其他进程占用，`is_registered` 返回 `false` 但实际注册会失败。此行为由 S04 boot pipeline 实际注册时捕获，S03 命令层属合理精度。
- **settings.json 与 hotkey.json 分文件**：两个关注点分开存文件，避免 `AppSettings` 和 `HotkeyConfig` 相互干扰。S04 注册 invoke_handler 时需确认两个文件名与此处一致。
- **`get_exclude_list_impl` 不返回 Err**：当前实现用 `load_or_default`，文件损坏时静默回默认空列表。若产品需要向用户上报损坏文件，后续可改为 `load` + 显式错误处理。

## TDD 流程记录

1. **RED**：先写 `tests/ipc_settings.rs`，引用不存在的 `quickquick_lib::ipc::settings`，编译失败（`error[E0432]: unresolved import`）——确认测试因功能缺失而失败。
2. **GREEN**：依次创建 `settings.rs`、注册 `pub mod settings` 到 lib.rs 和 ipc/mod.rs、创建 `ipc/settings.rs`；修复两处编译错误（`Manager` trait 未导入、`is_registered` 返回类型误判），最终 7 个测试全部通过。
3. **REFACTOR**：统一 impl 函数错误类型为 `String`（消除命令层多余的 `.map_err`），清理装饰性注释。

## code-standards 自检

| 项目 | 结论 |
|------|------|
| 函数 ≤ 50 行 | 通过（最长函数 `set_hotkey_impl` 约 15 行） |
| 嵌套 ≤ 3 层 | 通过（最深 2 层 if-let） |
| 无装饰性分隔注释 | 通过（grep 无命中） |
| 无 TODO/FIXME | 通过（grep 无命中） |
| 命名描述性 | 通过（动词+名词、impl 后缀区分 impl/command） |
| 注释写为什么 | 通过（关键决策均有说明原因的注释） |
| 错误用 Result 不 panic | 通过（所有错误路径返回 `Result<_, String>`） |
| 公共 API 有文档注释 | 通过（所有 pub fn 均有 `///` 文档） |
| 安全：不打印敏感信息 | 通过（无 log/eprintln 含用户数据） |
| 持久化键用显式算法 | 通过（serde_json 序列化，无隐式 hash） |
| 复用优先 | 通过（复用 HotkeyConfig/AppSettings/registry，未重造） |

## 验收命令实跑结论

```
cargo test --manifest-path src-tauri/Cargo.toml ipc_settings
```

命中函数：
- `ipc_settings_set_hotkey_then_get_returns_new_value ... ok`
- `ipc_settings_set_hotkey_conflict_rejected ... ok`
- `ipc_settings_exclude_list_roundtrip ... ok`
- `ipc_settings_exclude_list_empty_roundtrip ... ok`
- `ipc_settings_selected_provider_valid_id_roundtrip ... ok`
- `ipc_settings_selected_provider_invalid_id_rejected ... ok`
- `ipc_settings_get_translate_providers_contains_mymemory ... ok`

**test result: ok. 7 passed; 0 failed; 0 ignored**
