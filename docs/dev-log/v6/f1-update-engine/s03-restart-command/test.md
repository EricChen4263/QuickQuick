---
id: V6-F1-S03-test
type: test_report
level: 小功能
parent: V6-F1
status: 通过
commit: 0db9178
acceptance_ids: [V6-F1-A05]
---

# V6-F1-S03 restart-command 测试报告

## 命中校验

### 签名存在性测试（精确过滤）

命令：`cargo test restart_app_command_exists_with_apphandle_signature`

结果：
```
test restart_app_command_exists_with_apphandle_signature ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out
```

N=1，非空匹配，真命中。

### 整组 update 测试

命令：`cargo test update`

结果：所有套件均 `test result: ok`，无 FAILED。最大命中组 7 passed（来自内联测试）。

### A05 grep 断言

```
grep -q 'ipc::update::restart_app' src-tauri/src/lib.rs   → exit 0 (PASS)
grep -q 'fn restart_app' src-tauri/src/ipc/update.rs      → exit 0 (PASS)
```

## 变异 sanity

### 变异①：删除 lib.rs 中注册行

- 备份：`cp src-tauri/src/lib.rs /tmp/lib.rs.bak`
- 变异：删除 `ipc::update::restart_app,` 行（行 178）
- 验证：`grep -q 'ipc::update::restart_app' lib.rs` → exit 1，A05 grep 如期变红 (PASS)
- 补充：`cargo check` 仍编译通过——这符合预期。Rust invoke_handler 宏删行不是编译错误；A05 的判别力来自 grep 断言，不依赖编译失败。
- 复原：`cp /tmp/lib.rs.bak src-tauri/src/lib.rs`，恢复注册行，grep 重验通过。

### 变异②：update.rs 函数改名 restart_app → restart_app_x

- 备份：`cp src-tauri/src/ipc/update.rs /tmp/update.rs.bak`
- 变异：`pub fn restart_app` 改为 `pub fn restart_app_x`
- 验证：`cargo test restart_app_command_exists_with_apphandle_signature` 编译失败：

  ```
  error[E0433]: could not find `__tauri_command_name_restart_app` in `update`
  error[E0433]: could not find `__cmd__restart_app` in `update`
  error: could not compile `quickquick` (lib test) due to 2 previous errors
  ```

  编译失败如期（测试有真实判别力，非恒真/旁路，PASS）。
- 复原：`cp /tmp/update.rs.bak src-tauri/src/ipc/update.rs`，重验 grep 通过。

## 边界探测

### 实现分析

`restart_app` 实现极简：`app.restart()`，单行，无 panic 风险，无可失败路径，无参数，无状态依赖。

**panic 风险**：无。`app.restart()` 是 Tauri 核心内置 API，替换当前进程，实际永不返回，不会 panic。

**权限**：`capabilities/default.json` 已含 `core:default`（第 7 行），无需 `plugin-process` 或 `process:*`。clippy --all-targets 通过，未见缺失 permission 相关错误，佐证权限配置足够。

**关于 `assert_ne!(cmd as usize, 0)` 的恒真性说明**：

此断言技术上确实近恒真（函数指针地址不可能为零），但其真实价值**不在运行期值检验，而在编译期签名绑定**：

```rust
let cmd: fn(tauri::AppHandle) = restart_app;
```

这一行做了两件事：①要求 `restart_app` 在导入路径上存在（否则链接失败）；②要求其签名精确匹配 `fn(tauri::AppHandle)`（否则类型检查失败）。`assert_ne!` 只是强制 `cmd` 被"使用"以避免 `unused_variables` 告警，并非核心断言语义。这是一种合理的编译期守卫惯用法，不判为缺陷，但文档已如实说明。

**语义保留检查（等价变换）**：

将测试中局部变量 `cmd` 重命名为 `fn_ptr`（等价变换），重跑签名测试仍全绿——证明测试依赖行为语义（函数签名绑定），而非表层名字。通过。

### 合成边界用例

本命令无参数、无条件分支、无可测边界输入（AppHandle 由框架注入，不由调用者控制）。唯一运行路径是调用 `app.restart()`，无其他分支可合成。无可合成的边界用例，如实说明。

## Artifacts

- `artifacts/test-signature-raw.log` — 签名存在性测试原始输出
- `artifacts/test-update-group-raw.log` — 整组 update 测试原始输出
- `artifacts/mutation1-build.log` — 变异①后 cargo check 输出
- `artifacts/mutation2-test.log` — 变异②后编译失败原始输出
- `artifacts/clippy.log` — A06 clippy 输出

## 门禁结论

**放行。**

- A05 命中：签名存在性测试 1 passed，grep 双断言 exit 0，全部通过。
- 变异 sanity：变异①（删注册行）→ A05 grep 变红；变异②（改函数名）→ 编译失败。两处均如期变红，判别力确认。
- A06 clippy：exit 0，无 warning。
- 边界探测：无 panic 风险，权限配置足够，语义保留检查通过。恒真断言已说明其编译期守卫本质（非运行期缺陷）。
- 工作树自证：结束时 `git status --porcelain` 与开工逐行一致，无新增/丢失未提交改动。
