---
status: done
category: implementation
subject: 热键 OS 占用冲突的错误文案统一为中文（复用 HotkeyError::AlreadyInUse）
summary: 运行时注册器的错误映射把「已被占用」类折叠成中文「热键已被占用，无法绑定」（复用 hotkey.rs 里保留的 HotkeyError::AlreadyInUse，使其重新有生产构造者），其它错误保持「热键运行时注册失败: {英文原文}」不变，原始英文错误 eprintln 留痕。实施中发现裁定的前提需修正：插件边界已把底层结构化的 AlreadyRegistered/FailedToRegister 折叠成字符串，且 macOS 上非媒体键的占用冲突实际走 FailedToRegister("RegisterEventHotKey failed for …") 而非 AlreadyRegistered——故判据按上游文案前缀识别并同时覆盖两条签名，否则 macOS 主平台上的裁定意图会落空。测试：3 条单元测试锁映射（含正向对照）+ 1 条集成测试锁「中文文案原样透传到命令层」。变异：删掉映射分支与判据函数 → 2 条单元测试红、正向对照仍绿；还原后 diff 逐字节为空。死代码清单 186 → 185，唯一名字差异是 HotkeyError 从「仅本文件引用」参考项消失（它重新获得了跨文件生产构造者），零新增候选。
evidence:
  commits:
    - "e300951 fix(hotkey): 运行时注册的「已被占用」冲突文案统一为中文并留痕原始错误"
  commands:
    - "cargo test ipc_settings"
    - "cargo test --test ipc_settings --test hotkey"
    - "cargo clippy --all-targets -- -D warnings"
    - "rustfmt --edition 2021 --check src/ipc/settings.rs tests/ipc_settings.rs"
    - "node scripts/rust-dead-code-inventory.mjs"
  urls: []
---

# AI 活动记录 · 热键冲突文案统一为中文

## 背景

task-10 收敛 `set_hotkey` 死路径时核对了冲突文案，结论是**存在差异**并提请裁决：

| 路径 | 文案 |
| --- | --- |
| 旧死路径 `HotkeyConfig::rebind` → `HotkeyError::AlreadyInUse`（task-10 已删） | 「热键已被占用，无法绑定」 |
| 现役 `TauriRuntimeHotkeyRegistrar::register_action_shortcut` | 「热键运行时注册失败: HotKey already registered: …」（英文原文） |

前端 `src/main-window/settings/rebind.ts` 的 `validateRebind` 只覆盖「同应用内两个动作撞车」，
它自己产出「已被占用」；「被别的 App 占用」这条只能靠后端文案，于是中文 UI 会漏出一句英文。
lead 裁定：统一为中文，复用 `HotkeyError::AlreadyInUse`，其它错误保持原映射，英文原文 eprintln 留痕。

## 实施中发现裁定前提需要修正（重要）

裁定文本写的是「把 OS 占用冲突（`AlreadyRegistered`）映射为中文」。核对上游代码后有两层偏差：

1. **变体信息在插件边界就丢了**：`tauri_plugin_global_shortcut::Error` 只有
   `GlobalHotkey(String)` / `RecvError` / `Tauri` 三个变体，底层 `global_hotkey::Error`
   是经 `From<global_hotkey::Error>` 取 `to_string()` 折进来的。因此在
   `ipc/settings.rs` 这层**拿不到 `AlreadyRegistered` 变体**，只能按 Display 文本判断。
2. **macOS 上「被别的 App 占用」根本不是 `AlreadyRegistered`**：底层注册器在 macOS 上对
   非媒体键不做重复检查，直接调 Carbon `RegisterEventHotKey`；组合键被其它应用占用时它返回
   `eventHotKeyExistsErr`，于是错误是 `FailedToRegister("RegisterEventHotKey failed for …")`。
   `AlreadyRegistered` 只在媒体键（macOS）、X11 重复注册和 Windows 重复注册时出现。

结论：若只映射 `AlreadyRegistered`，本项目主平台 macOS 上最典型的「被别的 App 抢走」场景
仍然会漏英文——**裁定的意图会落空**。故实现按上游文案前缀识别，并同时覆盖两条签名
（`HotKey already registered` 与 `RegisterEventHotKey failed for`）。这是一处对裁定技术前提的
修正，不是范围扩张：只影响「已被占用」这一类的识别覆盖面。

## 改动

`src-tauri/src/ipc/settings.rs`（唯一生产文件，新增 2 个私有函数 + 换 1 处 `map_err`）：

- `is_hotkey_conflict_error(&str) -> bool`（:388）：用 `CONFLICT_SIGNATURES` 两条上游常量级
  英文前缀判断是否属「已被占用」类，不随系统语言变化。
- `map_runtime_register_error(&str) -> String`（:406）：
  - 命中冲突 → `eprintln!` 留痕英文原文，返回 `HotkeyError::AlreadyInUse.to_string()`
    （即「热键已被占用，无法绑定」，中文文案单一来源仍在 `hotkey.rs`）；
  - 其它 → 原映射 `format!("热键运行时注册失败: {error}")` 不变，英文原文仍随返回串给用户。
- `register_action_shortcut` 的 `map_err`（:427）改调上面的映射函数。

`HotkeyError::AlreadyInUse` 因此**重新有了生产构造者**（task-10 刻意保留它正是为这一步）。

未改前端：`rebind.ts` / `HotkeyPanel.tsx` 一行未动，前端错误展示路径（`role="alert"` 直接渲染
后端 message）无需改动即可生效。

## 测试

`src/ipc/settings.rs`（既有 `mod tests` 内新增 3 条单元测试，函数名带 `ipc_settings` 以便
`cargo test ipc_settings` 命中）：

- `ipc_settings_map_runtime_register_error_already_registered_is_chinese`（:1823）：
  `HotKey already registered: …` → 逐字等于「热键已被占用，无法绑定」，并断言结果**不含**
  `already registered`（英文不得漏给用户）。文案按字面量锁定而非比对
  `HotkeyError::AlreadyInUse.to_string()`：这是用户可见契约，改错误枚举 Display 的人
  不能顺带悄悄改 UI 文案。
- `ipc_settings_map_runtime_register_error_macos_carbon_conflict_is_chinese`（:1841）：
  macOS Carbon 占用形态同样折叠为中文（这条正是上面「偏差 2」的回归锁）。
- `ipc_settings_map_runtime_register_error_other_errors_keep_original_mapping`（:1853）：
  **正向对照**——`Unknown scancode for F99` → 「热键运行时注册失败: Unknown scancode for F99」，
  证明不是「所有错误都被改成中文」。

`src-tauri/tests/ipc_settings.rs`（新增 1 个 fake + 1 条集成测试）：

- `OccupiedConflictRegistrar`（:67）：注册调用直接返回中文冲突文案的 fake runtime。
- `ipc_settings_runtime_conflict_message_reaches_caller_verbatim`（:260）：断言
  `set_hotkey_runtime_impl` 把注册器给出的中文文案**一字不改**透传给命令层返回值
  （前端直接展示它，中间任何一层加前缀都会破坏验收口径）。这条用 fake 注入，因此与
  映射函数解耦——它守的是「透传层」，不是「映射层」。

任务判定：**丢弃**旧路径那种「冲突语义靠 `contains("已被占用") || contains("冲突")` 宽松匹配」的
写法（task-10 已随死路径删除），改为逐字锁定 + 正向对照。

## 变异验证（实做，隔离 worktree）

把 `map_runtime_register_error` 的冲突分支整段删掉（并把随之失去调用方的判据函数一起删，
否则只会变成 `dead_code` 警告而不是行为变异），保持其余不动：

```
test ipc::settings::tests::ipc_settings_map_runtime_register_error_already_registered_is_chinese ... FAILED
  assertion `left == right` failed: 已占用类错误应折叠为中文文案
  left: "热键运行时注册失败: HotKey already registered: HotKey { id: 1, mods: SUPER | SHIFT, key: KeyC }"
  right: "热键已被占用，无法绑定"
test ipc::settings::tests::ipc_settings_map_runtime_register_error_macos_carbon_conflict_is_chinese ... FAILED
  left: "热键运行时注册失败: RegisterEventHotKey failed for CmdOrCtrl+Shift+C"
  right: "热键已被占用，无法绑定"
test result: FAILED. 1 passed; 2 failed
```

正向对照仍绿（说明红的是映射，不是整体塌掉）。集成用例在 lib 目标失败后未被执行，这是
**设计如此**：它用 fake 注入文案、不经过映射函数，删映射不会让它变红——两层的判据分开。

还原后 `src/ipc/settings.rs` 的 diff 与变异前**逐字节一致**（两份 patch hunk `diff` 无输出），
`cargo test ipc_settings` 复跑 3 passed（lib）+ 13 passed（集成）。

## 验证结果

主工作区有他人未提交改动，验证期间先遇到 `lib` 编译失败（`pasteboard_meta` / `pipeline` 相关
符号），随后又遇到他人未跟踪的 scratch example（`src-tauri/examples/panel_probe_scratch.rs`）
编译错误使 `cargo clippy --all-targets` 失败。这些都不在本次写入范围内，故 clippy 与死代码清单
证据取自 **`git worktree` 在干净 base 1c56748 上应用本补丁** 的隔离环境；`cargo test` 与
rustfmt 在主工作区复跑同样为绿。

```
隔离 worktree（base 1c56748 + 本补丁）
  cargo test ipc_settings                      → lib 3 passed / 集成 ipc_settings 13 passed，0 failed
  cargo test --test ipc_settings --test hotkey → 13 passed + 3 passed，0 failed
  cargo clippy --all-targets -- -D warnings    → Finished，退出 0（无 error/warning）
  rustfmt --edition 2021 --check src/ipc/settings.rs tests/ipc_settings.rs → 退出 0

主工作区（含他人改动，最终状态）
  cargo test ipc_settings → lib 3 passed + 集成 13 passed，0 failed
  rustfmt --edition 2021 --check <本次 2 个文件> → 退出 0
  cargo clippy --all-targets -- -D warnings → 失败，原因是他人的
    src-tauri/examples/panel_probe_scratch.rs（未跟踪）E0277，与本次改动无关
```

死代码清单（同一 base 1c56748 口径，改前 → 改后）：

| 口径 | 候选合计 | 零引用 | 仅测试引用 | 仅本文件引用 |
| --- | --- | --- | --- | --- |
| base 1c56748（改前） | 186 | 0 | 15 | 171 |
| base + 本补丁（改后） | 185 | 0 | 15 | 170 |

`--all` 输出对比候选名字集合，唯一差异是 `HotkeyError` 从参考项消失：

```
38d37
< HotkeyError        （base：仅本文件引用 | enum | HotkeyError | src-tauri/src/hotkey.rs:25 | 2 | 0）
```

即它从「只有本文件在用」变成「有跨文件生产构造者」，**零新增候选**，可执行两类
（零引用 / 仅测试引用）计数不变。

## 未做 / 未验证

- **真机未实测**：没在 macOS 上真的让别的 App 占住一个热键再改绑，验证弹窗里显示的是
  「热键已被占用，无法绑定」。判据来自阅读上游源码（Carbon 返回 `eventHotKeyExistsErr`
  → `FailedToRegister`），不是实跑观测。
- 判据是**字符串匹配**，属于上游文案变更就会失配的脆弱耦合。根治方向是让
  `crate::register_action_shortcut`（`lib.rs`）在变体还没被插件折叠前就分类、或改用
  `is_registered` 预检；两处都超出本次写入范围，未做。
- `docs/dev-log/v0/acceptance.yaml` 未改（按裁定：dev-log 是历史记录，不做历史改写；
  P6-3 文档里的悬空 test_id 由 lead 统一注明）。
- 前端未改、未跑前端门禁（本任务未触及 `src/`）。
- Windows 分支未编译验证（沿用既有口径）；Windows 侧重复注册会走 `HotKey already registered`
  那一支，已由单元测试覆盖判据本身，但平台行为未实机验证。
