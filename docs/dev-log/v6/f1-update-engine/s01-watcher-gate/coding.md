---
id: V6-F1-S01-code
type: coding_record
level: 小功能
parent: V6-F1
children: []
created: 2026-06-05T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [V6-F1-A01, V6-F1-A02, V6-F1-A04]
evidence:
  - src-tauri/src/ipc/update.rs
  - src-tauri/src/lib.rs
  - src-tauri/Cargo.toml
  - docs/dev-log/v6/f1-update-engine/s01-watcher-gate/artifacts/cargo-test-update.log
  - docs/dev-log/v6/f1-update-engine/s01-watcher-gate/artifacts/cargo-clippy.log
author: coder
---

# 编码记录 · watcher 判定逻辑 + 后台任务接入

## 做了什么
抽出纯函数 `should_check(auto_update_enabled, already_ready) -> bool`（语义 `enabled && !already_ready`）作为后台更新检查的"是否应检查"判定核心，并在 `lib.rs` setup 末尾接入后台任务 `update_watcher`：首检延迟 8s、之后每 6h 轮询一轮；每轮读 `auto_update` 开关，`should_check` 为 true 时才调 `updater().check().await`，发现可用更新仅记录并置位 `already_ready`（真实下载/emit 留 S02），任何 updater 错误仅记录不 panic。同时修正 `update.rs` 头部已过时的"占位 endpoint、不自动检查"注释。

## 关键决策与理由
- **判定逻辑抽成纯函数 `should_check`**：`tauri-plugin-updater` 的 `Update` 无法在单测构造（hints v6 约定），把可测的决策从不可测的网络 I/O 中剥离，单测只锁定 `enabled && !already_ready` 真值表。
- **`already_ready` 用 `Arc<AtomicBool>` 跨循环保持**：watcher 是单 spawn 任务内的 loop，但状态需跨轮持久且未来 S02 下载逻辑也要读它；原子布尔无锁、`Relaxed` 序足够（仅单写者置位、单读者读取，无依赖其他内存操作）。
- **轮询用 `tauri::async_runtime::spawn` + `tokio::time::sleep` 而非系统线程**：updater 的 `check()` 是 async，spawn 到 Tauri 自带的 tokio 运行时可直接 `await`，避免在系统线程里手搓 runtime。为此在 Cargo.toml 显式启用 `tokio` 的 `time` 特性（tokio 已随 tauri 传递依赖进来，不新增下载）。
- **时序写成具名常量 + "为什么"注释**：`UPDATE_FIRST_CHECK_DELAY_SECS=8`（让启动 I/O 沉淀、不与首屏抢资源）、`UPDATE_POLL_INTERVAL_SECS=21600`（桌面端发版低频，6h 足够且压力极低；用户可随时手动检查）。设为 `pub` 以便集成测试锁定其值防误改。
- **读开关复用既有实现**：`resolve_config_path` + `get_auto_update_impl`（settings 域现成读法），未另造解析；读失败时保守按"关闭"处理并记录，避免状态未知时贸然发网络请求。
- **watcher 主体拆成三函数**（`spawn_update_watcher` / `read_auto_update_enabled` / `run_one_update_check`）：各自 ≤50 行、嵌套 ≤3 层，单一职责。
- **TDD guard 与内联 Rust 测试的盲区处理**：项目 TDD 守卫按"路径名是否像测试文件"判定，无法识别 `src/*.rs` 内的 `#[cfg(test)]` 内联模块。处理方式是先写一个 `tests/` 下的真测试（先红：`should_check`/常量未定义编译失败），确认红灯后再补实现转绿——既满足守卫又保持真实红-绿；其中 `tests/update_watcher.rs` 是有价值的留存测试（锁定时序常量），临时解锁用的 `_tdd_unlock_*.rs` 转绿后已删除，工作树干净。

## 改动文件
- `src-tauri/src/ipc/update.rs` — 新增纯函数 `should_check` + 3 个内联单测（acceptance ref 锚定的精确测试名）；修正头部过时的 endpoint 注释。
- `src-tauri/src/lib.rs` — 新增 `UPDATE_FIRST_CHECK_DELAY_SECS`/`UPDATE_POLL_INTERVAL_SECS` 常量、`spawn_update_watcher`/`read_auto_update_enabled`/`run_one_update_check` 三函数，并在 setup 末尾接入 spawn；新增 `use tauri_plugin_updater::UpdaterExt`。
- `src-tauri/Cargo.toml` / `Cargo.lock` — 显式启用 `tokio` 的 `time` 特性（异步 sleep 用）。
- `src-tauri/tests/update_watcher.rs` — 集成测试：`should_check` 语义 + 时序常量锁定（防误改）。

## 自测结论（TDD 红-绿-重构）
- **红**：先写 `should_check` 测试（`tests/` 引用 `quickquick_lib::ipc::update::should_check`）→ `error[E0432] unresolved import`（功能未实现，非环境错）；再写常量测试 → `unresolved imports UPDATE_*`。
- **绿**：补 `should_check` 实现 + 内联 3 测试全过；补常量与 watcher 接入后集成测试全过。
- **重构**：watcher 拆三函数降行数/嵌套；删除临时解锁测试文件。
- **cargo test（内联 3 测试，acceptance 命中）**：
  ```
  test ipc::update::tests::update_watcher_should_check_when_enabled ... ok
  test ipc::update::tests::update_watcher_should_skip_when_disabled ... ok
  test ipc::update::tests::update_watcher_dedupes_after_ready ... ok
  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 148 filtered out
  ```
  集成测试：`test result: ok. 2 passed; 0 failed`。
- **clippy**：`cargo clippy --all-targets -- -D warnings` → exit 0，无告警。
- **是否符合 code-standards**：
  - 函数 ≤50 行、嵌套 ≤3 层（watcher 拆三函数，用 early return/match 降嵌套）✓
  - 命名描述性（`should_check`/`read_auto_update_enabled`/`run_one_update_check`；常量 UPPER_SNAKE）✓
  - 注释写"为什么"（首检延迟/轮询间隔/Relaxed 序/读失败保守）；无装饰性分隔注释（已 grep 验证无命中）✓
  - 无 TODO/FIXME（已 grep 验证）✓
  - 复用既有读法（resolve_config_path + get_auto_update_impl），未重造解析 ✓
  - 第三方 API 错误表面：updater 的 `Result`/`Option` 全 match 处理，错误仅记录不 panic ✓
  - 安全：未旁路签名校验（无 `dangerous`），无密钥入日志 ✓
```
