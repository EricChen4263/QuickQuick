---
id: V0-F1-S04-code
type: coding_record
level: 小功能
parent: V0-F1
children: []
created: 2026-05-30T21:30:08Z
status: 通过
commit: WIP
acceptance_ids: [V0-F1-A05]
evidence:
  - src-tauri/src/autostart.rs
  - src-tauri/src/lib.rs
  - src-tauri/tests/autostart.rs
author: coder
---

# 编码记录 · V0-F1-S04 自启动偏好配置

## 做了什么

新增 `AutostartConfig` 偏好配置模型（`src-tauri/src/autostart.rs`），实现自启动开关的默认值（默认开）、JSON 持久化读写（save/load），并提供 `load_or_default` 供 setup 阶段安全调用。模块以 headless 可测方式与真实 OS LaunchAgent 注册完全解耦。

## 关键决策与理由

- **偏好模型与插件调用解耦**：`AutostartConfig` 不持有也不调用 `tauri-plugin-autostart` 句柄。真实注册由 setup 层读取偏好后决定调用时机。这使本模块可在无 GUI / 无 Tauri App 实例的集成测试中直接验证，避免测试触发真实 OS LaunchAgent 副作用。
- **风格完全对齐 hotkey.rs**：错误用 `thiserror` 枚举（`AutostartError`）、持久化用 `serde_json::to_string_pretty` / `from_str`、函数签名 `save(&self, path: &Path)` / `load(path: &Path)` 与现有约定一致，降低认知成本。
- **`load_or_default` 薄封装**：首次启动时配置文件尚不存在，直接 `load` 会返回 `IoError`。提供该方法让 setup 侧一行代码处理"文件不存在 → 回退默认"的场景，不把错误处理散落到调用方。
- **`pub enabled` 字段而非 getter/setter**：配置是简单数据对象，无不变式需要封装；直接公开字段与 hotkey.rs 的内部私有字段不同，因为 autostart 只有一个布尔配置项，公开字段更简洁，调用方改值后调 `save` 即可。

## 改动文件

- `src-tauri/src/autostart.rs` — 新增，实现 `AutostartConfig`（Default=true）、`AutostartError`、`save`/`load`/`load_or_default`
- `src-tauri/src/lib.rs` — 新增 `pub mod autostart;` 声明；新增 `apply_autostart_preference` 函数；setup 闭包调用该函数读取偏好并按 `enabled` 调用插件 `enable()`/`disable()`
- `src-tauri/tests/autostart.rs` — 新增集成测试：`autostart_default_on`（默认开）+ `autostart_persist_read_write`（改值→save→load 往返）+ `autostart_load_or_default_when_file_not_exist`（文件不存在回退默认开）

## 自测结论（TDD 红-绿-重构）

**RED**：先写 `tests/autostart.rs`，引用 `quickquick_lib::autostart::AutostartConfig`；运行确认因模块不存在报 `E0432: unresolved import` 而失败（不是语法错误）。

**GREEN**：实现 `src/autostart.rs` + 注册 `pub mod autostart`；运行 `cargo test autostart` 得：
```
test autostart_default_on ... ok
test autostart_persist_read_write ... ok
test result: ok. 2 passed; 0 failed
```

**REFACTOR**：新增 `load_or_default` 消除调用方重复的 `unwrap_or_default` 样板；重跑测试仍全绿。

**按审查修复 I-1（setup 消费偏好并 enable/disable）+ I-2（补 load_or_default 测试）**：

- I-1：在 `lib.rs` 新增 `apply_autostart_preference(app: &mut tauri::App)`，通过 `app.path().app_config_dir()` 获取配置路径（目录不存在时 `create_dir_all`，失败仅 eprintln 不 panic），调用 `autostart::AutostartConfig::load_or_default(&path)` 读取偏好，用 `use tauri_plugin_autostart::ManagerExt` 的 `app.autolaunch()` 按 `enabled` 调 `enable()`/`disable()`；调用失败仅 eprintln 不 panic。setup 闭包首行调用此函数。
- I-2：在 `tests/autostart.rs` 新增 `autostart_load_or_default_when_file_not_exist`：用 `tempfile::tempdir()` 建临时目录并拼一个不存在的文件名，断言 `load_or_default` 返回 `enabled=true`。

回归验证（证据）：
```
check=0     # cargo check 零错误
clippy=0    # cargo clippy -- -D warnings 零警告
autostart=0 # autostart_default_on/persist_read_write/load_or_default_when_file_not_exist 3 passed
all_rust=0  # 全量 Rust 测试 12 passed; 0 failed
todo=1      # grep 无匹配（exit 1 = 未找到）
```

**code-standards 自检**：
- 格式/命名：模块名 `autostart`、结构体 `AutostartConfig`、错误枚举 `AutostartError`，均为描述性名称；布尔字段 `enabled` 符合语义；函数名 `apply_autostart_preference` 为「动词+名词」。
- 函数长度：`apply_autostart_preference` 约 20 行，远低于 50 行上限；`save`/`load`/`load_or_default` 各 1-3 行。
- 嵌套深度：最深 2 层，低于 3 层上限。
- 注释写"为什么"：每个函数文档注释说明设计意图与错误语义；无注释掉的死代码。
- 类型安全：全程 `Result<_, AutostartError>`，无裸 `unwrap`/`panic`；`load_or_default` 的 `unwrap_or_default` 是有意为之的回退，有注释说明；`apply_autostart_preference` 所有错误路径均 eprintln 不 panic。
- 安全：无硬编码密钥，无用户输入直接使用，无敏感信息打印。
- 测试：AAA 结构，无恒真断言，每个断言有中文说明，测试名含验收项语义。
- clippy：`cargo clippy -- -D warnings` 零警告。
- build：`cargo check` 零错误。
- 无 TODO/FIXME 遗留。
