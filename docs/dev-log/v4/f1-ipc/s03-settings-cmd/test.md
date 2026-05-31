# V4-F1-S03 设置 IPC 命令 — 动态证伪测试报告

**日期**: 2026-05-31  
**被测对象**: `src-tauri/src/settings.rs` + `src-tauri/src/ipc/settings.rs`  
**测试文件**: `src-tauri/tests/ipc_settings.rs`  
**验收命令**: `cargo test --manifest-path src-tauri/Cargo.toml ipc_settings`

**开工 git 快照**:
```
 M src-tauri/src/ipc/mod.rs
 M src-tauri/src/lib.rs
?? docs/dev-log/v4/f1-ipc/s03-settings-cmd/
?? src-tauri/src/ipc/settings.rs
?? src-tauri/src/settings.rs
?? src-tauri/tests/ipc_settings.rs
```

**结束 git 快照**（与开工逐行一致，工作树已还原）:
```
 M src-tauri/src/ipc/mod.rs
 M src-tauri/src/lib.rs
?? docs/dev-log/v4/f1-ipc/s03-settings-cmd/
?? src-tauri/src/ipc/settings.rs
?? src-tauri/src/settings.rs
?? src-tauri/tests/ipc_settings.rs
```

---

## 档位 1：命中校验（杀假绿）

**命令**: `cargo test --manifest-path src-tauri/Cargo.toml ipc_settings`  
**结果**: `test result: ok. 7 passed; 0 failed` — N=7，真命中，无假绿。

7 个测试函数全部命中：
1. `ipc_settings_get_translate_providers_contains_mymemory ... ok`
2. `ipc_settings_selected_provider_invalid_id_rejected ... ok`
3. `ipc_settings_exclude_list_roundtrip ... ok`
4. `ipc_settings_set_hotkey_conflict_rejected ... ok`
5. `ipc_settings_exclude_list_empty_roundtrip ... ok`
6. `ipc_settings_set_hotkey_then_get_returns_new_value ... ok`
7. `ipc_settings_selected_provider_valid_id_roundtrip ... ok`

**结论**: 命中校验 PASS。

---

## 档位 2：变异 sanity（杀恒真/旁路）

### 变异 1：禁用 registry 校验

- **改坏**: `set_selected_provider_impl` 中 `if !is_valid {` 改为 `if false {`
- **备份**: `cp src-tauri/src/ipc/settings.rs /tmp/ipc_settings.rs.bak`
- **预期**: `ipc_settings_selected_provider_invalid_id_rejected` 变红
- **结果**: `test ipc_settings_selected_provider_invalid_id_rejected ... FAILED` — 如期变红
- **复原**: `cp /tmp/ipc_settings.rs.bak src-tauri/src/ipc/settings.rs` + git 快照一致

### 变异 2：set_exclude_list 不写入传入 list

- **改坏**: `settings.excluded_apps = list;` 改为 `settings.excluded_apps = vec![];`
- **备份**: 同上已备份
- **预期**: `ipc_settings_exclude_list_roundtrip` 变红
- **结果**: `test ipc_settings_exclude_list_roundtrip ... FAILED` — 如期变红
- **复原**: 从备份还原 + git 快照一致

### 变异 3：rebind 冲突错误不传播

- **改坏**: `.map_err(|e| e.to_string())?;` 改为 `.ok();`，使 rebind 错误被吞掉
- **预期**: `ipc_settings_set_hotkey_conflict_rejected` 变红
- **结果**: `test ipc_settings_set_hotkey_conflict_rejected ... FAILED` — 如期变红
- **复原**: 从备份还原 + git 快照一致

**结论**: 3 处变异全部如期变红，测试具有真实判别力，非恒真/旁路。变异 sanity PASS。

---

## 档位 3：边界探测

临时向测试文件追加 3 个边界测试（探测后已从备份复原测试文件）：

### B1：两字段共存（互不覆盖）

- **场景**: `set_exclude_list(["com.example.app"])` → `set_selected_provider("mymemory")` → 读回两字段
- **预期**: `excluded_apps` 仍为 `["com.example.app"]`，`selected_provider` 为 `"mymemory"`
- **结果**: `boundary_two_fields_coexist_in_settings ... ok`
- **结论**: `set_selected_provider_impl` 正确 load_or_default 后只修改 `selected_provider` 字段，不覆盖 `excluded_apps`。两字段共存无问题。

### B2：超长排除名单（100 项）往返

- **场景**: 写入 100 项 app id，读回验证内容一致
- **结果**: `boundary_large_exclude_list_roundtrip ... ok`
- **结论**: 无 panic，序列化/反序列化正常。

### B3：含特殊字符的 app 名往返（空格、中文、引号、反斜杠）

- **场景**: `["com.example.app with spaces", "com.example.中文应用", "com.example.app\"quoted\"", "com.example.app\\backslash"]`
- **结果**: `boundary_special_chars_in_exclude_list ... ok`
- **结论**: serde_json 正确处理特殊字符，无数据损失。

### B4：非法 action 字符串（parse_action）

- `parse_action` 为 private fn，无法从集成测试直接调用。
- 逻辑审查：`parse_action` 中 `other => Err(format!("未知 action：{other}，合法值为 history / translate"))` 明确覆盖了非法值路径，返回 Err（非 panic）。
- 命令层 `set_hotkey` 调用 `parse_action(&action)?`，Err 会正确传播给前端。
- **结论**: 非法 action 字符串优雅返回 Err，不 panic。无真实缺陷。

**边界探测汇总**: 所有边界均优雅处理，未发现真实缺陷。

---

## 失败项与缺口

无失败项。无覆盖缺口。

---

## 门禁结论

**放行。**

三档全部通过：
- 命中校验：7/7 passed，真命中，无假绿
- 变异 sanity：3/3 处如期变红，测试有真实判别力
- 边界探测：两字段互不覆盖、超长名单、特殊字符均正确处理
- 工作树还原确认：结束快照与开工快照逐行一致

V4-F1-S03 可进入下一任务。
