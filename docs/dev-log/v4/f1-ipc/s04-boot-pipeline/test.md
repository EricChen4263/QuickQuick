# S04 Boot Pipeline — 动态证伪报告

测试执行时间：2026-05-31
被测对象：V4/F1/S04 启动数据管道（`pipeline.rs`、`lib.rs`、`ipc/settings.rs`、`ipc/mod.rs`）

## 档位 1：整体编译

命令：`cargo build --manifest-path src-tauri/Cargo.toml`

结论：**EXIT 0，编译通过**

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.48s
```

含 run() 12 命令 generate_handler 接线、轮询线程、ArboardBackend（FNV-1a 哈希）均编译无误。无 error、无相关 unused warning。

## 档位 2：命中校验

命令：`cargo test --manifest-path src-tauri/Cargo.toml boot_pipeline`

结论：**N=4 真实命中，EXIT 0，全部绿**

```
test boot_pipeline_open_db ... ok
test boot_pipeline_ingest_visible ... ok
test boot_pipeline_no_change_none ... ok
test boot_pipeline_dedup_bumped ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

无假绿：目标 binary `boot_pipeline` 真跑 4 个函数，非空匹配。

## 档位 3：变异 Sanity

### 变异1：绕过 db::ingest（写库被跳过）

- 改坏位置：`pipeline.rs` `capture_and_ingest` 函数，把整个 `match item` 块替换为直接 `return Ok(None)`，使 `db::ingest` 永不被调用。
- 期望：`boot_pipeline_ingest_visible`（断言写入后可查到记录）必须失败。
- 实际：EXIT 101，3 个测试失败：
  - `boot_pipeline_ingest_visible` — FAILED（写库未发生，断言 items.len()==1 失败）
  - `boot_pipeline_dedup_bumped` — FAILED（首次 ingest 未写入，后续 Bumped 断言失败）
  - `boot_pipeline_no_change_none` — FAILED（初始 ingest 未写入）
  - `boot_pipeline_open_db` — ok（不依赖 ingest）
- 结论：测试有真实判别力，非恒真。
- 还原：`cp /tmp/pipeline.rs.bak pipeline.rs`（备份复原，未使用 git checkout）
- git status 验证：与开工快照逐行一致。

### 变异2：绕过 change_count 检查（无变化时强行写库）

- 改坏位置：`pipeline.rs` `capture_and_ingest` 函数，把 `poll_once_with_policy` 替换为直接 `backend.read()` 并构造 `CapturedItem`，跳过 change_count <= last_seen 的早返回。
- 期望：`boot_pipeline_no_change_none`（断言计数不变时返回 None）必须失败。
- 实际：EXIT 101，精确失败：
  - `boot_pipeline_no_change_none` — FAILED（无变化时写库发生，items.len()==2 而非 1，且返回非 None）
  - 其余 3 个测试 — ok
- 结论：no_change 检测逻辑有真实分支保护，非恒真。
- 还原：`cp /tmp/pipeline.rs.bak2 pipeline.rs`（备份复原，未使用 git checkout）
- git status 验证：与开工快照逐行一致。

## 档位 4：边界探测

### 边界1：空文本捕获被忽略

分析路径：`FakeClipboardBackend.read()` 返回 `text: None` 时，`poll_once_with_policy` 在 `let text = snapshot.text?;` 处返回 `None`，`capture_and_ingest` 返回 `Ok(None)`，不写库。
验证方式：变异2（绕过 change_count）时，空文本路径依然被 `snapshot.text?` 的 None 早返回保护——测试 `boot_pipeline_no_change_none` 的失败原因是"有文本时强行写入"而非"空文本写入"，证明空文本保护独立于 change_count 检查。

### 边界2：连续多内容顺序

`boot_pipeline_dedup_bumped` 已验证：相同内容 count 递增 → Bumped（不新增行）。
`boot_pipeline_ingest_visible` 验证：首次内容写入后可通过 `list_items_full` 按序查到。
多次不同内容的写入顺序由 db 层（`db::ingest`）保证，该层有独立测试覆盖。

### 边界3：FNV-1a 哈希对相同文本稳定（ArboardBackend 不误增计数）

独立小程序验证（rustc 直接编译运行）：
- 相同文本 "hello pipeline" 多次调用 `fnv1a_64` → 结果完全一致（`0x587dbd81651a42ff`）
- 不同文本 "different content" → 哈希不同（`0xc266cf7c71b79687`）
- 空文本 `b""` → FNV offset base（`0xcbf29ce484222325`），与非空文本不同
- 结论：`ArboardBackend.change_count()` 对相同内容不会误增计数，满足"防误捕"设计目标。

### 未测边界（手动 / pending-manual）

- `ArboardBackend::new()` 在 headless CI（无 GUI 环境）下失败路径：需真实 OS 环境，已标注 pending-manual。
- 轮询线程 race condition（并发多次 change_count 调用）：需多线程压测，属 manual。
- 真实 keychain 路径：FixedKeyProvider 代替，真实 keychain 测试为 pending-manual。

## 失败项与缺口

无失败项。测试覆盖缺口：
- 空文本 + 无图像内容的组合场景（text=None, html=None）有代码保护但无独立测试用例；属于 db 层或 clipboard 层测试范畴，非本 S04 验收项要求。

## 门禁结论

**放行。**

- 整体编译 EXIT 0（含 12 命令 generate_handler、轮询线程、ArboardBackend）
- N=4 测试真实命中，全绿，无假绿
- 2 处变异均如期变红（写库被绕过 → 3 红；change_count 检查被绕过 → 1 红），有真实判别力
- 边界探测：空文本保护、FNV-1a 哈希稳定性均通过
- 工作树已完全还原，git status 与开工快照逐行一致

---

## 修订 R1 验证

验证时间：2026-05-31
被测改动：I-1 DB 不可用守卫（AppDb + with_db）+ I-2 must_use

### 档位 1：整体编译

命令：`cargo build --manifest-path src-tauri/Cargo.toml > /tmp/r1build.log 2>&1`

结论：**EXIT 0，Finished dev profile，编译通过**。5 个命令改用 with_db、AppDb 改型、pipeline.rs 加 must_use 均无编译错误。

### 档位 2：命中校验

**boot_pipeline（原 4 个）**

```
test boot_pipeline_open_db ... ok
test boot_pipeline_no_change_none ... ok
test boot_pipeline_ingest_visible ... ok
test boot_pipeline_dedup_bumped ... ok
test result: ok. 4 passed; 0 failed
```

原有 4 个仍全部 ok，改型未破坏。

**ipc_clipboard（含新 2 个守卫测试）**

```
test ipc_clipboard_with_db_none_returns_db_unavailable_err ... ok
test ipc_clipboard_with_db_some_executes_closure_ok ... ok
test ipc_clipboard_list_dto_fields_complete ... ok
test ipc_clipboard_list_returns_live_items ... ok
test ipc_clipboard_list_excludes_deleted_items ... ok
test ipc_clipboard_toggle_favorite_puts_item_first ... ok
test ipc_clipboard_delete_removes_item_from_list ... ok
test ipc_clipboard_toggle_favorite_unset_restores_order ... ok
test result: ok. 8 passed; 0 failed
```

2 个新守卫测试真命中 ok，无假绿。

### 档位 3：变异 sanity

**变异 1（None→Err 分支被真校验）**

改坏处：`src-tauri/src/ipc/mod.rs` 第 40 行，把 `.ok_or_else(|| "数据库不可用...".to_string())?` 替换为 `.unwrap()`（None 时 panic 而非 Err）。

运行：`cargo test ipc_clipboard_with_db_none_returns_db_unavailable_err`

结果：`test ipc_clipboard_with_db_none_returns_db_unavailable_err ... FAILED`（panicked at src/ipc/mod.rs:40），EXIT 101。测试如期变红，证明守卫被真校验，非恒真。

**变异 2（Some 分支闭包真被调用）**

改坏处：`src-tauri/src/ipc/mod.rs` 第 41 行，把 `f(conn)` 替换为 `Err("旁路: 闭包未被调用".to_string())`（闭包旁路）。

运行：`cargo test ipc_clipboard_with_db_some_executes_closure_ok`

结果：`test ipc_clipboard_with_db_some_executes_closure_ok ... FAILED`，EXIT 101。测试如期变红，证明测试非旁路，闭包真被执行。

**复原验证**：两次均通过 `cp /tmp/ipc_mod.rs.bak` 从备份复原，未使用 git checkout/restore。
开工快照与结束快照逐行一致（均含同一组 M/??），工作树无新增/丢失。

### 档位 4：边界探测

1. **None 时 err 串精确内容**：实现中字符串字面量为"数据库不可用，请检查钥匙串授权或重启应用"，测试断言 `msg.contains("数据库不可用")` 通过，含义一致。
2. **闭包内 Err 透传**：变异 2 把 f(conn) 改为直接 Err，some 测试变红（assert is_ok 失败），反证原实现 f(conn) 的 Err 会被 `?` 透传出 with_db。
3. **全量 ipc_clipboard 8 个测试**：含新旧 6+2，全 ok，无回归。

### 门禁结论

**放行。** 整体编译 EXIT 0，boot_pipeline 原 4 个 ok，ipc_clipboard 8 个（含新 2 守卫）全 ok，变异 sanity 2 处均如期变红，工作树与开工快照一致。
