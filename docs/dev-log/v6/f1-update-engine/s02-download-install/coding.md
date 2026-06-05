---
id: V6-F1-S02-code
type: coding_record
level: 小功能
parent: V6-F1
children: []
created: 2026-06-05T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [V6-F1-A03]
evidence:
  - src-tauri/src/ipc/update.rs
  - src-tauri/src/lib.rs
  - docs/dev-log/v6/f1-update-engine/s02-download-install/artifacts/cargo-test-update.log
  - docs/dev-log/v6/f1-update-engine/s02-download-install/artifacts/cargo-clippy.log
author: coder
---

# 编码记录 · 下载安装薄封装 + 就绪事件 + 命令

## 做了什么
把 S01 中 `run_one_update_check` 的 `Ok(Some(update))` 分支（原仅 `eprintln!` + 置位）替换为真实下载安装链路。在 `ipc/update.rs` 新增：
- `pub const UPDATE_READY_EVENT: &str = "update://ready";` 与 `UpdateReadyPayload { version: String }`（`#[derive(Clone, Serialize)]`，camelCase）。
- 纯函数 `build_ready_payload(version) -> UpdateReadyPayload`（A03 命中测试 `update_ready_payload_carries_version` 内联锁定）。
- 下载安装薄封装 `download_install_and_notify(app, update, already_ready) -> Result<(), String>`：调 `update.download_and_install(on_chunk, on_finish).await`（回调空实现，静默下载、无前端进度），成功后 `app.emit(UPDATE_READY_EVENT, build_ready_payload(&version))` 并置位 `already_ready`；失败仅 `eprintln!` 记录、不 panic、不置位（留待重试）。
- watcher 入口 `download_install_for_watcher`（忽略错误、静默重试）与手动命令 `#[tauri::command] download_and_install_update`（`check()` → 有则走薄封装、无则 Ok、错误回传前端中文文案）二者复用同一薄封装（DRY）。

`lib.rs`：`Ok(Some(update))` 分支改调 `ipc::update::download_install_for_watcher`；命令注册到 `invoke_handler![]`（紧挨 `check_for_updates`）。

## 关键决策与理由
- **薄封装隔离不可测 I/O**：真实 `download_and_install` 无法在单测构造 `Update`（hints v6 约定），故仅对纯函数 `build_ready_payload` 单测，下载链路归 A12 真机覆盖。payload 抽纯函数使"事件名 + 版本号构造"可被 A03 精确锁定。
- **后台与手动共用薄封装（DRY）**：`download_install_and_notify` 返回 `Result`，后台路径 `download_install_for_watcher` 丢弃错误（静默重试），手动命令直接回传错误给前端展示——区别仅在错误处理，下载逻辑只有一份。
- **手动命令用独立 `already_ready` 标志**：后台 watcher 的去重标志由 `lib.rs` 跨轮持有；手动命令仅需薄封装的成功置位语义，新建一次性 `Arc<AtomicBool>` 即可，二者互不干扰，避免把 watcher 的私有状态泄漏到命令签名。
- **emit 失败不回滚就绪**：下载安装已成功是既成事实，emit 失败仅记录并仍置位 `already_ready`（避免重复下载），前端可走手动检查兜底。
- **on_chunk/on_finish 空实现**：本版静默下载、不做前端进度（极简），进度 emit 留作可选增强。

## 改动文件
- `src-tauri/src/ipc/update.rs` — 新增 `UPDATE_READY_EVENT` 常量、`UpdateReadyPayload`、`build_ready_payload`（+内联测试 `update_ready_payload_carries_version`）、薄封装 `download_install_and_notify`、`download_install_for_watcher`、命令 `download_and_install_update`；新增 import（`Arc`/`AtomicBool`/`tauri::Emitter`/`Update`）。
- `src-tauri/src/lib.rs` — `run_one_update_check` 的 `Ok(Some)` 分支改调薄封装；`invoke_handler![]` 注册 `download_and_install_update`（lib.rs:177）；更新过时文档注释。

## 自测结论（TDD 红-绿-重构）
- **红**：临时 `tests/_tdd_unlock_ready_payload.rs` 引用未实现的 `build_ready_payload` → `error[E0432] unresolved import`（功能未实现，非环境错）。
- **绿**：补常量/payload/构造函数 + 内联测试后转绿；删除临时解锁测试，工作树干净。
- **重构**：薄封装拆为 `_and_notify`（返回 Result，核心逻辑）+ `_for_watcher`（静默包装），后台/手动复用，去重两份下载逻辑。
- **cargo test update（A03 命中）**：
  ```
  test ipc::update::tests::update_ready_payload_carries_version ... ok
  test ipc::update::tests::update_watcher_should_check_when_enabled ... ok
  test ipc::update::tests::update_watcher_dedupes_after_ready ... ok
  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 148 filtered out
  ```
  原始输出存 artifacts/cargo-test-update.log。
- **clippy**：`cargo clippy --all-targets -- -D warnings` → exit 0，无告警（artifacts/cargo-clippy.log）。
- **是否符合 code-standards**：
  - 函数 ≤50 行、嵌套 ≤3 层（薄封装拆两层，命令用 match early return）✓
  - 命名描述性（`build_ready_payload`/`download_install_and_notify`/`download_install_for_watcher`；常量 UPPER_SNAKE）✓
  - 注释写"为什么"（薄封装隔离 I/O、emit 失败不回滚、手动独立标志）；无装饰性分隔注释（grep 验证）✓
  - 无 TODO/FIXME（grep 验证）✓
  - DRY：后台与手动共用同一薄封装，下载逻辑仅一份 ✓
  - 第三方 API 错误表面：`download_and_install`/`emit`/`check` 全 `Result` 处理，错误仅记录或回传、不 panic ✓
  - 安全：未旁路签名校验（无 `dangerous`），无密钥入日志 ✓
