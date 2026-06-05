---
id: V6-F1-S02-test
type: test_report
level: 小功能
parent: V6-F1
status: 通过
commit: 0db9178
acceptance_ids: [V6-F1-A03]
---

# 测试报告 · download-install 薄封装（S02）

## 开工快照（git status --porcelain）

```
 M docs/dev-log/v6/f1-update-engine/s01-watcher-gate/coding.md
 M docs/dev-log/v6/f1-update-engine/s01-watcher-gate/review.md
 M docs/dev-log/v6/f1-update-engine/s01-watcher-gate/test.md
 M src-tauri/src/ipc/update.rs
 M src-tauri/src/lib.rs
?? AGENTS.md
?? docs/design/quickquick-simplified-ui.html
?? docs/design/quickquick-simplified-ui.md
?? docs/dev-log/v6/f1-update-engine/s02-download-install/
```

---

## 1. 命中校验（杀假绿）

runner: `cd src-tauri && cargo test <精确测试名>`，确认 N≥1（非空匹配）。

### A03 · update_ready_payload_carries_version（精确过滤）

```
cargo test: 1 passed, 397 filtered out (30 suites, 0.00s)
```

- 命中：是（N=1，397 filtered out，非空匹配）

### update 前缀整组（顺带确认无遗漏）

```
cargo test: 7 passed, 391 filtered out (30 suites, 0.00s)
```

- 整组 update 相关测试 7 passed（含 A03 + S01 的 A01/A02/A04 三项 + 其余），无失败。

**命中校验结论：A03 真命中，无假绿。整组 7 项全通过。**

备注：`build_ready_payload` 是纯函数，无排序/时间戳/共享资源竞争，无 flaky 风险，单轮即可。

---

## 2. 变异 sanity（杀恒真/旁路）

被测文件：`src-tauri/src/ipc/update.rs`
备份方式：`cp src/ipc/update.rs /tmp/update_s02.rs.bak`，验完 `cp /tmp/update_s02.rs.bak src/ipc/update.rs`（禁用 git checkout/restore）

### 变异①：`version: version.to_string()` → `version: String::new()`

目的：断言 A03 对版本字符串有真实判别力（非恒真、非旁路 build_ready_payload）。

改坏后运行结果：

```
---- ipc::update::tests::update_ready_payload_carries_version stdout ----
assertion `left == right` failed
  left: ""
 right: "1.2.3"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 154 filtered out
```

- 如期变红：是
- 已从备份复原（MD5 d7f4d0fe67252df2a3467c17116510b1 一致）：是

### 变异②：`version: version.to_string()` → `version: "9.9.9".to_string()`（硬编码不同值）

目的：进一步确认测试检查的是入参透传（非任意字符串），且对不同硬编码值均能拦截。

改坏后运行结果：

```
---- ipc::update::tests::update_ready_payload_carries_version stdout ----
assertion `left == right` failed
  left: "9.9.9"
 right: "1.2.3"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 154 filtered out
```

- 如期变红：是
- 已从备份复原（MD5 d7f4d0fe67252df2a3467c17116510b1 一致）：是

### 工作树还原自证（结束时 git status --porcelain）

```
 M docs/dev-log/v6/f1-update-engine/s01-watcher-gate/coding.md
 M docs/dev-log/v6/f1-update-engine/s01-watcher-gate/review.md
 M docs/dev-log/v6/f1-update-engine/s01-watcher-gate/test.md
 M src-tauri/src/ipc/update.rs
 M src-tauri/src/lib.rs
?? AGENTS.md
?? docs/design/quickquick-simplified-ui.html
?? docs/design/quickquick-simplified-ui.md
?? docs/dev-log/v6/f1-update-engine/s02-download-install/
```

与开工快照逐行一致，工作树干净还原。

**变异 sanity 结论：两次变异均如期让 A03 变红，`build_ready_payload` 版本透传逻辑具有真实判别力，非恒真/旁路。**

---

## 3. A06 Clippy（F1 大功能级顺带验收）

命令：`cd src-tauri && cargo clippy --all-targets -- -D warnings`

```
cargo clippy: No issues found
```

- exit code：0
- 新增告警：无

---

## 4. 边界探测

### 4.1 `build_ready_payload` 边界输入

A03 仅覆盖正常版本字符串 `"1.2.3"`，审读以下边界：

- 空字符串 `""`：`build_ready_payload("")` 会构造 `version: ""`，不 panic（`to_string()` 对空字符串合法）。前端收到空版本号属语义退化，但不是程序错误——是 caller 保证非空的责任（`update.version` 由 tauri-plugin-updater 填充，实践中不为空）。此为可接受的隔离。
- 超长字符串（如 1000 字节版本号）：`to_string()` 无长度限制，不 panic，不截断，正确透传。
- Unicode 版本号（如 `"１.２.３"`）：`to_string()` 正确处理，不 panic。

无越界、panic 或静默错误风险。

### 4.2 薄封装层健壮性审读

**`download_install_and_notify` 下载失败路径：**
- 下载失败时 `eprintln!` 记录后 `return Err(...)`，不 panic，不置位 `already_ready`——留待下轮/手动重试。符合设计约定。
- `already_ready` 只在下载成功且 emit 之后置位（第 112 行），失败时不置位。重试语义正确。

**emit 失败路径：**
- emit 失败时仅 `eprintln!` 记录，`already_ready` 仍置位（第 112 行在 emit 错误处理之后，无条件执行）。
- 这是有意的设计选择（注释已说明："emit 失败不影响'已安装'事实，仍视为就绪以免重复下载"）。
- 潜在边界：已安装但前端未收到通知，用户看不到重启提示。这是 S03 重启命令的范围，S02 层面不要求处理；设计上属合理权衡，非缺陷。

**`download_install_for_watcher` 后台入口：**
- `let _ = ...` 静默忽略错误，符合"后台路径静默重试"设计约定，不 panic。

**`download_and_install_update` 手动命令：**
- 使用独立 `Arc<AtomicBool>` 局部变量，与 watcher 的 `already_ready` 完全隔离，互不干扰。
- updater 初始化失败、check 失败、下载失败均映射为 `Err(String)` 返回给前端，不 panic。

**发现的真实缺陷：无。** 实现健壮，与设计约定一致。

### 4.3 合成定向用例（验收未覆盖分支）

`build_ready_payload` 当前只有一个测试用例（`"1.2.3"`）。以下是潜在的补充用例思路：

1. **版本号原样透传（不同格式）**：`build_ready_payload("2.0.0-beta.1").version == "2.0.0-beta.1"`——验证预发版本格式不被截断。
2. **UPDATE_READY_EVENT 常量值正确性**：`assert_eq!(UPDATE_READY_EVENT, "update://ready")`——锁定事件名字符串，防误改。

这两个用例目前未在测试文件中，属覆盖缺口（语义上影响较小，但有利于防守）。回交 coder 决策是否补充（非阻塞）。

---

## 5. Artifacts

- `artifacts/test-a03.log` — A03 精确命中日志（rtk 摘要：1 passed, 397 filtered out）
- `artifacts/test-update-group.log` — update 整组命中日志（7 passed, 391 filtered out）
- `artifacts/mutant-a03.log` — 变异①②的 FAILED 证据及复原确认
- `artifacts/clippy-a06.log` — A06 clippy 日志（No issues found）

---

## 6. 门禁结论

**通过。**

- A03 `update_ready_payload_carries_version` — 真命中（N=1），ok；两次变异均如期变红，判别力已证
- A06 Clippy — exit 0，无新增告警
- 边界探测 — 无真实缺陷；覆盖缺口（版本格式变体 + 事件名常量锁定）非阻塞，建议 coder 补充
- 工作树与开工快照逐行一致，无业务代码残留改动
