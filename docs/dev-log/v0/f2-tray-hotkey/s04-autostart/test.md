---
id: V0-F1-S04-test
type: test_report
level: 小功能
parent: V0-F1
children: []
created: 2026-05-30T21:37:49Z
status: 通过
commit: WIP
acceptance_ids: [V0-F1-A05]
evidence: [/tmp/T4b.log, /tmp/T4t.log, /tmp/T4all.log, /tmp/T4c.log]
author: tester
---

# 测试报告 · S04 自启动偏好配置

## 运行的测试命令

```bash
# 1. 编译验证（含新增插件调用代码能否正确链接进二进制）
cargo build --manifest-path src-tauri/Cargo.toml > /tmp/T4b.log 2>&1
# exit 0

# 2. A05 专项：autostart 3 用例
cargo test --manifest-path src-tauri/Cargo.toml autostart > /tmp/T4t.log 2>&1
# exit 0

# 3. 全量 Rust 测试（回归）
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/T4all.log 2>&1
# exit 0

# 4. clippy 零警告
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings > /tmp/T4c.log 2>&1
# exit 0

# 5. 无遗留 TODO/FIXME
grep -rn 'TODO\|FIXME' src-tauri/src
# exit 1（无匹配，符合预期）
```

## 结果

**通过**

| 命令 | exit code | 说明 |
|---|---|---|
| `cargo build` | 0 | 编译成功，含 tauri-plugin-autostart 插件调用链接 |
| `cargo test autostart` | 0 | 3 passed / 0 failed |
| `cargo test`（全量） | 0 | 12 passed / 0 failed（6 lib单测 + 3 autostart + 3 hotkey） |
| `cargo clippy -- -D warnings` | 0 | 零警告 |
| `grep TODO\|FIXME` | 1 | 无匹配，符合预期（exit 1 = 未找到） |

## 用例清单 + 结果

| 用例 | 结果 | 对应验收项 | 断言内容 |
|---|---|---|---|
| `autostart_default_on` | pass | V0-F1-A05 | `AutostartConfig::default().enabled == true`（默认开） |
| `autostart_persist_read_write` | pass | V0-F1-A05 | `enabled=false` save→load 往返一致；re-save `enabled=true` 再 load 也正确 |
| `autostart_load_or_default_when_file_not_exist` | pass | V0-F1-A05 | 指向不存在文件时 `load_or_default` 返回 `enabled=true`（默认开回退） |

**验收项 V0-F1-A05 断言：** "autostart 插件接入：自启动开关配置项存在、可读写、默认开"

三个用例分别覆盖：
- `autostart_default_on`：自启动开关默认开（enabled=true）
- `autostart_persist_read_write`：可读写（JSON save/load 往返正确）
- `autostart_load_or_default_when_file_not_exist`：文件不存在时回退默认开（首次启动安全）

## 覆盖缺口

| 代码路径 | 覆盖情况 |
|---|---|
| `AutostartConfig::default()` 默认 `enabled=true` | 覆盖（`autostart_default_on`） |
| `AutostartConfig::save()` 写入 JSON | 覆盖（`autostart_persist_read_write`） |
| `AutostartConfig::load()` 读取 JSON | 覆盖（`autostart_persist_read_write`） |
| `AutostartConfig::load_or_default()` 正常路径 | 覆盖（`autostart_persist_read_write` 中通过 load 间接覆盖） |
| `AutostartConfig::load_or_default()` 文件缺失回退 | 覆盖（`autostart_load_or_default_when_file_not_exist`） |
| `apply_autostart_preference()`（lib.rs setup 调用链） | 未覆盖——该函数依赖真实 Tauri `App` 实例及 OS 侧 LaunchAgent，headless 环境无法构造；设计上通过与插件解耦将"偏好读写"与"OS注册"分开测试，属已知豁免 |
| `AutostartError::SerdeError` 错误路径 | 未覆盖（非本验收项范围；`save`/`load` 正常路径已覆盖） |

## 失败项详情

无。所有用例通过，无失败。

## 结论

验收项 V0-F1-A05 所要求的 3 个用例（default_on / persist_read_write / load_or_default_when_file_not_exist）全部通过。`cargo build` 确认新增插件调用代码可正确编译链接。全量 Rust 测试 12 passed，clippy 零警告，无遗留 TODO/FIXME。

**允许进入下一任务（放行）。**
