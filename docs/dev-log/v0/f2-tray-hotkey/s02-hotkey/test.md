---
id: V0-F2-S02-test
type: test_report
level: 小功能
parent: V0-F2
children: []
created: 2026-05-30T21:01:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F2-A01, V0-F2-A02]
evidence: [/tmp/ts02.log, /tmp/ts02cl.log]
author: tester
---

# 测试报告 · S02 全局热键配置与冲突检测

## 运行的测试命令

```bash
# A01 + A02：热键集成测试
cargo test --manifest-path src-tauri/Cargo.toml hotkey > /tmp/ts02.log 2>&1

# 工程质量：clippy 零警告
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings > /tmp/ts02cl.log 2>&1

# 工程质量：无遗留 TODO/FIXME
grep -rn 'TODO\|FIXME' src-tauri/src
```

## 结果

**通过**

- `cargo test hotkey`：exit 0，2 passed / 0 failed
- `cargo clippy -- -D warnings`：exit 0，无任何警告
- `grep TODO\|FIXME`：exit 1（无匹配，符合预期）

## 用例清单 + 结果

| 用例 | 结果 | 对应验收项 |
|---|---|---|
| `hotkey_defaults_and_rebind` | pass | V0-F2-A01 |
| `hotkey_conflict_rejected` | pass | V0-F2-A02 |

**附加验证：**

- `hotkey_defaults_and_rebind`：断言默认 `History=CmdOrCtrl+Shift+V`、`Translate=CmdOrCtrl+Shift+T`；rebind 后内存配置更新；JSON save/load 往返一致（使用 tempfile 临时目录，无副作用）。
- `hotkey_conflict_rejected`：`ConflictRegistrar` 对目标键返回 `AlreadyInUse`；测试断言 `result.is_err()` 成立，且 `err_msg.contains("已被占用")` 成立；原配置值与冲突前一致，未被改动。

## 覆盖率

本次覆盖范围：

| 代码路径 | 覆盖情况 |
|---|---|
| `HotkeyConfig::default()` 默认值 | 覆盖（A01 断言两默认值） |
| `HotkeyConfig::rebind()` 成功路径 | 覆盖（A01 AlwaysOkRegistrar） |
| `HotkeyConfig::rebind()` 冲突失败路径 | 覆盖（A02 ConflictRegistrar） |
| `HotkeyConfig::save()` + `load()` | 覆盖（A01 往返持久化） |
| `HotkeyError::AlreadyInUse` Display 文本含"已被占用" | 覆盖（A02 assert contains） |
| 冲突时原配置不变（副作用隔离） | 覆盖（A02 assert 原值不变） |

**覆盖缺口：**

- 真实 Tauri GlobalShortcut API 的系统级注册路径未覆盖（headless CI 环境不具备 GUI，属已知豁免；trait 抽象设计目的即隔离此路径）。
- `HotkeyError::SerdeError` / `HotkeyError::IoError` 错误路径无专项用例（非本验收项范围，可在后续补充）。
- `HotkeyAction::Translate` 的 rebind 路径未测试（当前仅测 History；低风险，match 分支对称）。

## 失败项详情

无。所有用例通过，无失败。

## 结论

两个验收项（V0-F2-A01、V0-F2-A02）全部通过，clippy 零警告，无遗留 TODO/FIXME。

**允许进入下一任务（放行）。**
