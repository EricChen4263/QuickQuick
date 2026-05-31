# Phase 6 动态证伪报告 — V4/F1/S02 翻译 IPC 命令

验收项：A02 verify = `cargo test --manifest-path src-tauri/Cargo.toml ipc_translate`

---

## 开工 git 快照

```
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml
 M src-tauri/src/ipc/mod.rs
 M src-tauri/src/translate/history.rs
?? docs/dev-log/v4/f1-ipc/s02-translate-cmd/
?? src-tauri/src/ipc/translate.rs
?? src-tauri/tests/ipc_translate.rs
```

---

## 档位一：命中校验

命令：`cargo test --manifest-path src-tauri/Cargo.toml ipc_translate`

结论：**通过**

- `test result: ok. 6 passed; 0 failed` — 恰好 6 个，无假绿
- 6 个命中函数名：
  1. `ipc_translate_empty_text_returns_error_without_calling_executor`
  2. `ipc_translate_whitespace_text_returns_error_without_calling_executor`
  3. `ipc_translate_chinese_text_produces_zh_to_en_direction`
  4. `ipc_translate_english_text_produces_en_to_zh_direction`
  5. `ipc_translate_writes_to_history_after_success`
  6. `ipc_translate_list_history_returns_entries_in_desc_order`
- 网络隔离确认：测试文件全程使用 `FakeExecutor`，无 `UreqExecutor` 联网调用

---

## 档位二：变异 sanity

### 变异一：改坏空校验（`if text.trim().is_empty()` → `if false`）

- 改前备份：`cp src-tauri/src/ipc/translate.rs /tmp/translate.rs.bak`
- 改坏方式：sed 将 `if text.trim().is_empty()` 替换为 `if false`
- 跑测试：`cargo test ipc_translate_empty_text`、`cargo test ipc_translate_whitespace`
- 结果：**两个测试如期变红**（EXIT_CODE=101）
  - `ipc_translate_empty_text_returns_error_without_calling_executor ... FAILED`：`assertion left == right failed: 空文本不应触发执行器`（call_count 断言失败）
  - `ipc_translate_whitespace_text_returns_error_without_calling_executor ... FAILED`：`assertion left == right failed: 全空白文本不应触发执行器`
  - 证明：call_count=0 断言被真正执行，测试非恒真/非旁路
- 复原：`cp /tmp/translate.rs.bak src-tauri/src/ipc/translate.rs`，验证 `text.trim().is_empty()` 恢复

### 变异二：移除 `add_translate_history` 调用（历史不被写入）

- 改坏方式：Python 替换将整个 `add_translate_history(...)` 调用块替换为空语句
- 跑测试：`cargo test ipc_translate_writes_to_history`
- 结果：**测试如期变红**（EXIT_CODE=101）
  - `ipc_translate_writes_to_history_after_success ... FAILED`：`assertion left == right failed: 翻译后历史条目数应 +1`
  - 证明：`count_after == count_before + 1` 断言真正检查了 DB 写入
- 复原：`cp /tmp/translate.rs.bak src-tauri/src/ipc/translate.rs`，验证 `add_translate_history` 调用恢复

### 变异 sanity 总结

- 已还原：是
- 结束 git 快照与开工逐行一致：是（见"结束快照"节）

---

## 档位三：边界探测

临时测试文件放入 `tests/boundary_explore.rs`（用后删除），全程 FakeExecutor 无网络。

| 边界场景 | 测试函数 | 结果 |
|---|---|---|
| 超长 text（"Hello " × 2000，约 12KB） | `boundary_very_long_text_does_not_panic` | ok（不 panic，FakeExecutor call_count=1） |
| 纯标点 `!@#$%^&*()` | `boundary_punctuation_only_text_direction_is_en_to_zh` | ok（source=en, target=zh） |
| 纯数字 `123456` | `boundary_numeric_text_direction_is_en_to_zh` | ok（source=en, target=zh） |
| FakeExecutor 返回非法 JSON `NOT_VALID_JSON{{{` | `boundary_invalid_json_response_returns_err_not_panic` | ok（返回 Err，非 panic） |
| 空库 `list_translate_history_impl` | `boundary_empty_db_list_history_returns_empty_vec` | ok（返回空 vec） |

全部 5 passed；临时测试文件已删除。**未发现真实缺陷。**

---

## 结束 git 快照

```
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml
 M src-tauri/src/ipc/mod.rs
 M src-tauri/src/translate/history.rs
?? docs/dev-log/v4/f1-ipc/s02-translate-cmd/
?? src-tauri/src/ipc/translate.rs
?? src-tauri/tests/ipc_translate.rs
```

与开工快照逐行一致，工作树还原干净。

---

## 门禁结论

**放行**

所有三档全通过，无打回项，无覆盖缺口，无真实缺陷。
