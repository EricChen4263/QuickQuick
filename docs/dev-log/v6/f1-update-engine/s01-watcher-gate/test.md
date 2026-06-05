---
id: V6-F1-S01-test
type: test_report
level: 小功能
parent: V6-F1
status: 通过
commit: 32c2806
acceptance_ids: [V6-F1-A01, V6-F1-A02, V6-F1-A04]
---

# 测试报告 · watcher 判定逻辑（S01）

## 开工快照（git status --porcelain）

```
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml
 M src-tauri/src/ipc/update.rs
 M src-tauri/src/lib.rs
?? AGENTS.md
?? docs/design/auto-update.html
?? docs/design/auto-update.md
?? docs/design/quickquick-simplified-ui.html
?? docs/design/quickquick-simplified-ui.md
?? docs/dev-log/hints.md
?? docs/dev-log/v6/
?? src-tauri/tests/update_watcher.rs
```

---

## 1. 命中校验（杀假绿）

runner: `cd src-tauri && cargo test <精确测试名>`，每次单独过滤，确认 N≥1（非空匹配）。

### A01 · update_watcher_should_check_when_enabled

```
test ipc::update::tests::update_watcher_should_check_when_enabled ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.00s
```

- 命中：是（N=1，153 filtered out，非空匹配）

### A02 · update_watcher_should_skip_when_disabled

```
test ipc::update::tests::update_watcher_should_skip_when_disabled ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.00s
```

- 命中：是（N=1，非空匹配）

### A04 · update_watcher_dedupes_after_ready

```
test ipc::update::tests::update_watcher_dedupes_after_ready ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.00s
```

- 命中：是（N=1，非空匹配）

**命中校验结论：三项全部真命中，无假绿。**

备注：should_check 是纯函数（无排序/时间戳/共享资源竞争），无 flaky 风险，单轮即可。

---

## 2. 变异 sanity（杀恒真/旁路）

被测文件：`src-tauri/src/ipc/update.rs`
备份方式：`cp update.rs /tmp/update.rs.bak`，验完 `cp /tmp/update.rs.bak update.rs`（禁用 git checkout/restore）

### 变异①：去掉 `&& !already_ready`（`auto_update_enabled && !already_ready` → `auto_update_enabled`）

目的：断言 `update_watcher_dedupes_after_ready`（A04）如期变红。

改坏后运行结果：

```
test ipc::update::tests::update_watcher_dedupes_after_ready ... FAILED
---- ipc::update::tests::update_watcher_dedupes_after_ready stdout ----
thread 'ipc::update::tests::update_watcher_dedupes_after_ready' (2760868) panicked at src/ipc/update.rs:89:9
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.00s
```

- 如期变红：是
- 已从备份复原：是

### 变异②：改成恒 `true`（`auto_update_enabled && !already_ready` → `true`）

目的：断言 `update_watcher_should_skip_when_disabled`（A02）如期变红。

改坏后运行结果：

```
test ipc::update::tests::update_watcher_should_skip_when_disabled ... FAILED
---- ipc::update::tests::update_watcher_should_skip_when_disabled stdout ----
thread 'ipc::update::tests::update_watcher_should_skip_when_disabled' (2765461) panicked at src/ipc/update.rs:83:9
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.00s
```

- 如期变红：是
- 已从备份复原：是

### 工作树还原自证（结束时 git status --porcelain）

```
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml
 M src-tauri/src/ipc/update.rs
 M src-tauri/src/lib.rs
?? AGENTS.md
?? docs/design/auto-update.html
?? docs/design/auto-update.md
?? docs/design/quickquick-simplified-ui.html
?? docs/design/quickquick-simplified-ui.md
?? docs/dev-log/hints.md
?? docs/dev-log/v6/
?? src-tauri/tests/update_watcher.rs
```

与开工快照逐行一致，工作树干净还原。

**变异 sanity 结论：两次变异均如期让对应测试变红，测试具有真实判别力，非恒真/旁路。**

---

## 3. 边界探测

### 3.1 四种布尔组合全覆盖分析

`should_check(enabled, already_ready)` 的真值表完整性：

| enabled | already_ready | 期望结果 | 覆盖位置 |
|---------|---------------|----------|----------|
| true    | false         | true     | A01（内联）+ 集成测试 |
| false   | false         | false    | A02（内联）+ 集成测试 |
| false   | true          | false    | A02（内联，assert!(!should_check(false, true))）+ 集成测试 |
| true    | true          | false    | A04（内联）+ 集成测试 |

全部四种组合均有测试覆盖（3 个内联单测 + 集成测试 `should_check_follows_enabled_and_not_ready` 四合一验证），无覆盖缺口。

### 3.2 watcher 健壮性代码审读

**读开关失败是否安全回退：**
`read_auto_update_enabled` 在 `Err` 分支返回 `false` 并 `eprintln!` 记录。保守处理正确——读失败时不贸然发起网络请求，不 panic。

**updater 错误是否不 panic：**
`run_one_update_check` 对 updater 初始化失败（`app.updater()` 返回 `Err`）和网络检查失败（`updater.check().await` 返回 `Err`）均走 `eprintln!` + `return`/match arm，不 panic，符合要求。

**时序常量与设计一致性：**
- `UPDATE_FIRST_CHECK_DELAY_SECS = 8`（设计：8s，让启动 I/O 先沉淀）— 一致
- `UPDATE_POLL_INTERVAL_SECS = 21600`（设计：6h = 21600s）— 一致
- 集成测试 `watcher_timing_matches_design_contract` 锁定这两个值，防误改

**额外定向边界（验收未覆盖的合成用例）：**

集成测试 `should_check_follows_enabled_and_not_ready` 已把四种组合合并为一个测试，但这与三个内联单测存在语义等价，不是新的覆盖缺口。

以下是潜在但未被测试的侥幸场景（属于 S02 范围，本轮 S01 不要求）：
- `already_ready` 被置位后，再次调用 `should_check` 仍返回 false（这是 A04 已覆盖的内容）
- `already_ready` 从 `Arc<AtomicBool>` 跨轮持久，线程安全性由 Rust 类型系统保证

**发现的边界问题：无。** 实现健壮，与设计约定一致。

---

## 4. Artifacts

- `artifacts/test-a01.log` — A01 精确命中日志
- `artifacts/test-a02.log` — A02 精确命中日志
- `artifacts/test-a04.log` — A04 精确命中日志
- `artifacts/mutant1-a04.log` — 变异①（去 already_ready）后 A04 FAILED 证据
- `artifacts/mutant2-a02.log` — 变异②（恒 true）后 A02 FAILED 证据

---

## 5. 门禁结论

**通过。**

- A01 `update_watcher_should_check_when_enabled` — 真命中，ok
- A02 `update_watcher_should_skip_when_disabled` — 真命中，ok；变异②证明有判别力
- A04 `update_watcher_dedupes_after_ready` — 真命中，ok；变异①证明有判别力
- 四种布尔组合全覆盖（无缺口）
- watcher 健壮性通过代码审读：读失败保守回退，updater 错误不 panic，时序常量与设计一致
- 工作树与开工快照逐行一致，无业务代码残留改动
