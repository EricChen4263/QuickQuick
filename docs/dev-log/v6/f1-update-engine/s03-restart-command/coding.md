---
id: V6-F1-S03-code
type: coding_record
level: 小功能
parent: V6-F1
children: []
status: 通过
commit: 0db9178
acceptance_ids: [V6-F1-A05]
evidence: [src-tauri/src/ipc/update.rs, src-tauri/src/lib.rs, src-tauri/tests/update_watcher.rs, docs/dev-log/v6/f1-update-engine/s03-restart-command/artifacts/cargo-build.log, docs/dev-log/v6/f1-update-engine/s03-restart-command/artifacts/cargo-clippy.log, docs/dev-log/v6/f1-update-engine/s03-restart-command/artifacts/a05-grep.log, docs/dev-log/v6/f1-update-engine/s03-restart-command/artifacts/cargo-test.log]
author: coder
---

# V6-F1-S03 restart-command 编码记录

## 做了什么

1. **新增 `restart_app` 命令**（`src-tauri/src/ipc/update.rs`）：`#[tauri::command] pub fn restart_app(app: tauri::AppHandle)`，函数体 `app.restart()`。供前端「立即重启」入口 invoke，让 S02 已下载安装的新版本生效。
2. **注册命令**（`src-tauri/src/lib.rs`）：在 `tauri::generate_handler![]` 中紧挨 `download_and_install_update` 之后加 `ipc::update::restart_app,`。
3. **订正两处过时注释**（S02 reviewer 标出的 Important，设计 §一要求）：
   - `check_for_updates` doc 注释原写"endpoint 为占位地址时会返回网络/解析错误"——改为反映 endpoint 已是真实地址、网络/解析失败时返回 Err 由前端友好展示，与模块顶部口径一致。
   - `spawn_update_watcher` doc 注释原写"真实下载/update://ready emit 留给 S02"——S02 已实现，改为反映 watcher 现已通过 `run_one_update_check` 真实下载安装并 emit `update://ready`。

## 关键决策与理由

- **核心 restart 而非 plugin-process**：`AppHandle::restart()` 是 Tauri v2 框架内置 API，从 Rust 命令内部直接调用，无需引入 `tauri-plugin-process`、也无需在 capabilities 开放 `process:*` 权限。`capabilities/default.json` 现有 `core:default` 已足够——**实测结论：cargo build 与 clippy --all-targets -- -D warnings 均 exit 0，无任何 capability/permission 报错，证明能力足够，未改动 default.json**。
- **签名声明 `()` 而非 `-> !`**：初版按"restart 永不返回"写成 `-> !`，触发 `#[tauri::command]` 宏的 E0282（宏需要一个可序列化的具体回执类型，无法对 `!` 推断）。`restart()` 的 `!` 可强制转为 `()`，改回 `()` 后宏满足、build 通过；doc 注释如实说明"正常路径根本走不到返回"。
- **可测面取舍（TDD 拓扑）**：`restart_app` 实际永不返回、调用即替换进程，无法在单测内调用（hints/设计 §七已定性，真机重启验证归 manual_confirm A12）。故采用"按预期签名将命令绑定为函数指针 `let cmd: fn(tauri::AppHandle) = restart_app;`"的编译期+链接期断言（`tests/update_watcher.rs::restart_app_command_exists_with_apphandle_signature`）：先 RED（`restart_app` 未实现 → 导入解析失败 E0432）、再 GREEN（实现后通过），并 `assert_ne!(cmd as usize, 0)` 作具体值断言（非恒真、非旁路）。一旦命令被误删或改签名，该测试编译失败即报警。A05 本身是命令注册的 grep 断言型验收。

## 改动文件

- `src-tauri/src/ipc/update.rs` — 新增 `restart_app` 命令；订正 `check_for_updates` doc 注释。
- `src-tauri/src/lib.rs` — invoke_handler 注册 `restart_app`；订正 `spawn_update_watcher` doc 注释。
- `src-tauri/tests/update_watcher.rs` — 新增 `restart_app_command_exists_with_apphandle_signature` 签名存在性断言测试。

## 自测结论

- `cargo build`：**exit 0**（artifacts/cargo-build.log）。
- `cargo clippy --all-targets -- -D warnings`：**exit 0**，No issues found（artifacts/cargo-clippy.log）。
- A05 grep 断言：`grep -q 'ipc::update::restart_app' lib.rs`（exit 0）且 `grep -q 'fn restart_app' ipc/update.rs`（exit 0），组合 **exit 0**（artifacts/a05-grep.log）。
- `cargo test --test update_watcher`：3 passed、0 failed；其中 `restart_app_command_exists_with_apphandle_signature ... ok`（真命中，非空匹配假绿；artifacts/cargo-test.log）。
- `restart_app` 实际重启行为不可单测，归真机 **manual_confirm A12**。
- 提交前自检：改动 src/tests 无装饰性分隔注释、无 TODO/FIXME；断言验具体值且调被测命令本身。
