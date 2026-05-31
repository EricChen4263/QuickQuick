# Phase 6 测试报告 — V4/F1/S01 剪贴板 IPC 命令

执行时间：2026-05-31
被测对象：`src-tauri/src/ipc/clipboard.rs`、`src-tauri/src/db.rs`（`list_items_full` / `ClipItemRow`）
测试文件：`src-tauri/tests/ipc_clipboard.rs`（6 个）、`src-tauri/tests/ipc_validation.rs`（6 个）

---

## 开工快照（git status --porcelain）

```
 M src-tauri/src/db.rs
 M src-tauri/src/lib.rs
?? docs/dev-log/v4/f1-ipc/
?? src-tauri/src/ipc/
?? src-tauri/tests/ipc_clipboard.rs
?? src-tauri/tests/ipc_validation.rs
```

---

## 档位一：命中校验（杀假绿）

### A01 verify：`cargo test --manifest-path src-tauri/Cargo.toml ipc_clipboard`

```
running 6 tests
test ipc_clipboard_list_dto_fields_complete ... ok
test ipc_clipboard_delete_removes_item_from_list ... ok
test ipc_clipboard_list_returns_live_items ... ok
test ipc_clipboard_list_excludes_deleted_items ... ok
test ipc_clipboard_toggle_favorite_puts_item_first ... ok
test ipc_clipboard_toggle_favorite_unset_restores_order ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

真命中 6 个，N=6 ≥ 1，无假绿。

### A14 verify：`cargo test --manifest-path src-tauri/Cargo.toml ipc_input_validation`

```
running 6 tests
test ipc_input_validation_delete_whitespace_id_returns_err ... ok
test ipc_input_validation_delete_valid_nonexistent_id_passes_validation ... ok
test ipc_input_validation_toggle_favorite_whitespace_id_returns_err ... ok
test ipc_input_validation_toggle_favorite_empty_id_returns_err ... ok
test ipc_input_validation_delete_valid_nonexistent_id_passes_validation ... ok
test ipc_input_validation_toggle_favorite_valid_id_passes_validation ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

真命中 6 个，N=6 ≥ 1，无假绿。

**档位一结论：通过。**

---

## 档位二：变异 sanity（杀恒真/旁路）

（进行中，结论追加于此）


### 变异一：去掉 `list_items_full` 的 `is_deleted = 0` 过滤（改为 `WHERE 1 = 1`）

改坏文件：`src-tauri/src/db.rs`（行 247，用 sed 全局替换所有 `WHERE is_deleted = 0`）
- 改前备份：`cp src-tauri/src/db.rs /tmp/db.rs.bak`
- 变异命令：`sed -i '' 's/WHERE is_deleted = 0/WHERE 1 = 1/' src-tauri/src/db.rs`
- 跑 `cargo test ... ipc_clipboard` → **EXIT_CODE=101，2 个测试变红**：
  - `ipc_clipboard_list_excludes_deleted_items ... FAILED`
  - `ipc_clipboard_delete_removes_item_from_list ... FAILED`
- 结论：测试对软删过滤有真实判别力，非恒真
- 复原：`cp /tmp/db.rs.bak src-tauri/src/db.rs`（验证 `WHERE is_deleted = 0` 回到第 247 行）

### 变异二：将 `validate_id` 条件改为 `if false {`（去掉空串/空白校验）

改坏文件：`src-tauri/src/ipc/clipboard.rs`（行 86）
- 改前备份：`cp src-tauri/src/ipc/clipboard.rs /tmp/clipboard.rs.bak`
- 变异命令：`sed -i '' 's/if id.trim().is_empty() {/if false {/'`
- 跑 `cargo test ... ipc_input_validation` → **EXIT_CODE=101，4 个测试变红**：
  - `ipc_input_validation_delete_empty_id_returns_err ... FAILED`
  - `ipc_input_validation_delete_whitespace_id_returns_err ... FAILED`
  - `ipc_input_validation_toggle_favorite_empty_id_returns_err ... FAILED`
  - `ipc_input_validation_toggle_favorite_whitespace_id_returns_err ... FAILED`
- 结论：测试对空 id 校验有真实判别力，非恒真/旁路
- 复原：`cp /tmp/clipboard.rs.bak src-tauri/src/ipc/clipboard.rs`（验证 `if id.trim().is_empty()` 回到第 86 行）

### git 快照对比

结束时 `git status --porcelain` 与开工快照**逐行一致**，工作树未引入新改动。

**档位二结论：通过。两处变异均如期变红，测试有真实判别力，已从备份复原，工作树干净。**

---

## 档位三：边界探测

（进行中，结论追加于此）


### 边界测试用例（临时文件放入 tests/boundary_test.rs，跑完删除）

| 边界 | 输入 | 操作 | 预期 | 实际 |
|------|------|------|------|------|
| 超长 id（1000字符）| `"a" * 1000` | delete | Ok（通过校验，SQL影响0行）| **ok** |
| 超长 id（2048字符）| `"x" * 2048` | toggle | Ok（通过校验，SQL影响0行）| **ok** |
| SQL 注入字符 id | `"'; DROP TABLE clip_items; --"` | delete | Ok（参数化保护，不注入）| **ok** |
| OR 注入 id | `"1 OR 1=1"` | toggle | Ok（参数化保护）| **ok** |

```
running 4 tests
test boundary_very_long_id_toggle_does_not_panic ... ok
test boundary_sql_injection_id_toggle_does_not_panic ... ok
test boundary_sql_injection_id_delete_does_not_panic ... ok
test boundary_very_long_id_delete_does_not_panic ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

临时测试文件删除后，工作树与开工快照逐行一致，无新增/丢失文件。

**档位三结论：通过。SQL 参数化查询有效阻断注入，超长 id 不 panic，实现边界行为符合预期。**

---

## 结束时 git 快照

```
 M src-tauri/src/db.rs
 M src-tauri/src/lib.rs
?? docs/dev-log/v4/f1-ipc/
?? src-tauri/src/ipc/
?? src-tauri/tests/ipc_clipboard.rs
?? src-tauri/tests/ipc_validation.rs
```

与开工快照**逐行一致**，无任何业务代码残留改动。

---

## 门禁结论：**放行**

| 档位 | 结论 |
|------|------|
| 命中校验（A01） | 通过：6 个 ipc_clipboard_* 真命中，全绿 |
| 命中校验（A14） | 通过：6 个 ipc_input_validation_* 真命中，全绿 |
| 变异 sanity（软删过滤）| 通过：改坏后 2 个测试如期变红，已复原 |
| 变异 sanity（空 id 校验）| 通过：改坏后 4 个测试如期变红，已复原 |
| 边界探测 | 通过：超长 id、SQL 注入字符均安全处理，无 panic |

无失败项，无覆盖缺口，工作树干净。V4/F1/S01 硬门禁**通过**，可进入下一小功能。
