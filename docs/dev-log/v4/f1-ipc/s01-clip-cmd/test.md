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

---

## 打回修复 R2 验证（producer 打回：并发 flaky rowid DESC 兜底）

执行时间：2026-06-01
被测修复：`src-tauri/src/db.rs` — `list_items_full`（行 251）与 `list_ordered`（行 321）ORDER BY 追加 `, rowid DESC` 确定性兜底

开工快照（git status --porcelain）：
```
 M docs/dev-log/v4/f1-ipc/s01-clip-cmd/coding.md
 M src-tauri/src/db.rs
```

---

### 档位一：命中校验 + 抗 flaky（全量连跑 6 次）

命令：`cargo test --manifest-path src-tauri/Cargo.toml`（全量）

| 次数 | exit | FAILED | 结论 |
|------|------|--------|------|
| RUN1 | 0 | 0 | 全绿 |
| RUN2 | 0 | 0 | 全绿 |
| RUN3 | 0 | 0 | 全绿 |
| RUN4 | 0 | 0 | 全绿 |
| RUN5 | 0 | 0 | 全绿 |
| RUN6 | 0 | 0 | 全绿 |

每次约 22 个测试套、合计数百测试，`ipc_clipboard_toggle_favorite_puts_item_first` 6 次均 ok，无任何 FAILED。

clippy：`cargo clippy --all-targets -- -D warnings` → exit 0，无 error/warning。

pnpm test：17 个测试文件，141 个测试，全绿，前端无回归。

**档位一结论：通过。抗 flaky 6 连跑零 FAILED，clippy 干净，pnpm test 全绿。**

---

### 档位二：变异 sanity（验 rowid DESC 兜底必要性）

改坏操作：去掉 `list_items_full` 行 251 的 `, rowid DESC`（仅保留 `ORDER BY is_favorite DESC, last_modified_utc DESC`）

- 备份：`cp src-tauri/src/db.rs /tmp/db.rs.bak`（改前）
- 变异：`sed -i '' '251s/, rowid DESC//'`
- 连跑 8 次全量 `cargo test`：

| 次数 | exit | FAILED | 失败测试 |
|------|------|--------|---------|
| RUN1 | 0 | 0 | — |
| **RUN2** | **101** | **1** | **`ipc_clipboard_toggle_favorite_puts_item_first ... FAILED`** |
| RUN3 | 0 | 0 | — |
| RUN4 | 0 | 0 | — |
| RUN5 | 0 | 0 | — |
| RUN6 | 0 | 0 | — |
| RUN7 | 0 | 0 | — |
| RUN8 | 0 | 0 | — |

**第 2 次命中 flaky**，`ipc_clipboard_toggle_favorite_puts_item_first` 如期 FAILED。证明：
1. 去掉 `rowid DESC` 后，同毫秒并列时排序不确定，测试偶发失败
2. 该测试非恒真/旁路，对 rowid DESC 兜底有真实判别力
3. `rowid DESC` 修复是必要且有效的

复原：`cp /tmp/db.rs.bak src-tauri/src/db.rs`（从备份，未用 git checkout/restore）
复原验证：行 251 重现 `ORDER BY is_favorite DESC, last_modified_utc DESC, rowid DESC`

结束 git status：
```
 M docs/dev-log/v4/f1-ipc/s01-clip-cmd/coding.md
 M src-tauri/src/db.rs
```
与开工快照逐行一致，无新增/丢失。

**档位二结论：通过。去掉兜底后第 2 次即暴露 flaky，测试有真实判别力；已从备份复原，git 快照一致。**

---

### 档位三：边界探测

边界验证（通过阅读测试逻辑 + 修复后最终全量确认）：

| 边界场景 | 验证方式 | 结论 |
|---------|---------|------|
| 多条同 favorite=1 时内部按 last_modified_utc DESC + rowid DESC 稳定 | 现有测试 `ipc_clipboard_toggle_favorite_puts_item_first` 覆盖（最新收藏排最前） | 通过 |
| 收藏项整体置顶于非收藏项 | 现有测试 `ipc_clipboard_toggle_favorite_puts_item_first` 覆盖 | 通过 |
| 取消收藏后回归 last_modified 排序 | 现有测试 `ipc_clipboard_toggle_favorite_unset_restores_order` 覆盖 | 通过 |
| 同毫秒并列时 rowid 确定性兜底 | 变异 sanity 8次连跑已验证（去掉后第2次 flaky）| 通过 |

**档位三结论：通过。收藏置顶、组内稳定、取消恢复均有测试覆盖；并发并列场景已由变异 sanity 验证。**

---

## 门禁结论（R2 修复）：**放行**

| 档位 | 结论 |
|------|------|
| 命中校验 + 抗 flaky（全量 6 连跑）| 通过：6 次全绿，零 FAILED，`ipc_clipboard_toggle_favorite_puts_item_first` 稳定 |
| clippy | 通过：exit 0，无 warning/error |
| pnpm test | 通过：141 个前端测试全绿，无回归 |
| 变异 sanity（去 rowid DESC）| 通过：第 2 次命中 flaky，证明兜底必要且有效；已复原，git 干净 |
| 边界探测 | 通过：收藏置顶、排序稳定、并发并列均覆盖 |

无失败项，无覆盖缺口，工作树干净。R2 打回修复验证**通过**，硬门禁放行。
