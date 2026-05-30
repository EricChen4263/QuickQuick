---
id: V0-F2-S02-code
type: coding_record
level: 小功能
parent: V0-F2
children: []
created: 2026-05-30T20:59:05Z
status: 通过
commit: WIP
acceptance_ids: [V0-F2-A01, V0-F2-A02]
evidence:
  - src-tauri/src/hotkey.rs
  - src-tauri/src/lib.rs
  - src-tauri/tests/hotkey.rs
author: coder
---

# 编码记录 · V0-F2-S02 全局热键配置与冲突检测

## 做了什么

新增 `hotkey` 模块，实现热键默认值（`CmdOrCtrl+Shift+V` / `CmdOrCtrl+Shift+T`）、改键持久化（serde_json 落单文件、load/save 往返）、以及通过 `HotkeyRegistrar` trait 抽象的冲突检测——冲突时拒绝保存、错误含"已被占用"，不崩溃。

## 关键决策与理由

- **`HotkeyRegistrar` trait 抽象**：将系统热键注册 API（Tauri global shortcut，需 GUI 运行时）与业务逻辑隔离，使冲突路径可在 headless 集成测试中用 fake 实现覆盖，无需启动 GUI。否定了直接调用系统 API 的方案（无法 headless 单测）。
- **先注册后写配置**：`rebind` 内先调用 `registrar.register()`，成功才更新内存字段。失败时 `?` 提前返回，配置字段完全不变，满足 V0-F2-A02"拒绝保存"要求。
- **thiserror 枚举化错误**：`HotkeyError` 含 `AlreadyInUse`（Display 中文"热键已被占用"）、`SerdeError`、`IoError`，符合项目规范（错误用 thiserror 枚举，可失败路径不 unwrap）。
- **serde_json pretty 落盘**：配置文件人类可读，便于用户手动排查，代价仅多几个空格，值得。

## 改动文件

- `src-tauri/src/hotkey.rs` — 新增：`HotkeyAction` 枚举、`HotkeyError`（thiserror）、`HotkeyRegistrar` trait、`HotkeyConfig`（default/get_accelerator/rebind/save/load）
- `src-tauri/src/lib.rs` — 在内部文档注释后追加 `pub mod hotkey;`，暴露模块供集成测试使用
- `src-tauri/tests/hotkey.rs` — 新增集成测试：`hotkey_defaults_and_rebind`（V0-F2-A01）、`hotkey_conflict_rejected`（V0-F2-A02），含 `AlwaysOkRegistrar` 和 `ConflictRegistrar` 两个 fake 实现

## 自测结论（TDD 红-绿-重构）

**RED**：先写 `src-tauri/tests/hotkey.rs`，引用 `quickquick_lib::hotkey` 尚不存在的模块；`cargo test hotkey` 以编译错 `E0432: unresolved import` 失败，确认是功能未实现导致，不是测试本身的错误。

**GREEN**：新增 `src-tauri/src/hotkey.rs` 并在 `lib.rs` 注册 `pub mod hotkey;`；再次运行测试，两个目标测试 `hotkey_defaults_and_rebind` 和 `hotkey_conflict_rejected` 均 `ok`，exit=0。

**REFACTOR**：clippy `-D warnings` 通过（0 警告）；无 TODO/FIXME；函数均 ≤50 行，嵌套 ≤3 层；build 绿。

**code-standards 逐项自检**：
- 格式：4 空格缩进（Rust 官方），行宽 ≤120，文件末尾换行，符合。
- 函数：单一职责；`rebind`/`save`/`load` 均 ≤20 行；参数 ≤4；嵌套 ≤2 层（match 内无嵌套）。
- 命名：`HotkeyAction`（PascalCase 枚举）、`get_accelerator`（动词+名词 snake_case）、`is_*` 前缀无布尔量需命名。
- 注释：模块级 `//!` 说明设计要点（why），函数 `///` 含 Errors 段；无死代码注释。
- 类型：`HotkeyError` 枚举覆盖所有可失败路径，无裸 `unwrap`；公共接口显式类型。
- 安全：无密钥、无 SQL、无用户输入注入风险（热键字符串由调用方传入，不经数据库）。
- 测试：AAA 结构，测试名描述行为（`hotkey_defaults_and_rebind`、`hotkey_conflict_rejected`）；按审查补 Translate rebind 覆盖测试（`hotkey_rebind_translate_isolates_field`，验证只改 Translate 字段、不串 History）。
- 提交：待提交时按 Conventional Commits `feat(hotkey): ...` 前缀。
