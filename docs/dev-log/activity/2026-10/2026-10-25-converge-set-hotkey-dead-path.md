---
status: done
category: implementation
subject: P6-3 残留处置 · 收敛 set_hotkey 死路径（删 set_hotkey_impl，改键测试迁到生产实现）
summary: set_hotkey_impl 生产零调用、只被 6 处测试调用（清单计 7 次引用含 import），按裁定删除；随之 HotkeyConfig::rebind 与 HotkeyRegistrar trait 失去唯一生产调用方，一并删除，tests/hotkey.rs 的 3 条 rebind 用例退场、剩余用例改写为不依赖注册的配置读写。改键/冲突的全部行为断言迁到生产路径 set_hotkey_runtime_impl（用既有 RecordingRuntimeRegistrar fake），并新增「先注册新键 → 落盘 → 注销旧键」的顺序断言。死代码清单 185 → 184（仅测试引用 16 → 15），名字集合差异只有 set_hotkey_impl 消失、零新增。变异验证：删掉同键早退分支 → 2 条测试红（含 1 条迁移后的新测试），还原后 15 passed 且 diff 逐字节为空。
evidence:
  commits:
    - "c61135a refactor(hotkey): 删除生产零调用的 set_hotkey_impl，改键测试迁移到运行时实现"
  commands:
    - "cargo test --test ipc_settings --test hotkey"
    - "cargo test"
    - "cargo clippy --all-targets -- -D warnings"
    - "cargo fmt --all --check"
    - "rustfmt --edition 2021 --check src/hotkey.rs src/ipc/settings.rs tests/hotkey.rs tests/ipc_settings.rs"
    - "node scripts/rust-dead-code-inventory.mjs"
  urls: []
---

# AI 活动记录 · 收敛 set_hotkey 死路径

## 背景

`cargo` 侧两条并行的改键实现：`ipc::settings::set_hotkey_impl`（加载配置 → `HotkeyConfig::rebind`
做注册校验 → save，**不做任何运行时注册**）与 `ipc::settings::set_hotkey_runtime_impl`（加载配置 →
同键早退 → 运行时注册新键 → 落盘（失败则注销新键回滚）→ 注销旧键）。生产命令
`set_hotkey` 只调后者，前者零生产调用、仅被 `tests/ipc_settings.rs` 的 6 处调用养着
——`scripts/rust-dead-code-inventory.mjs` 里它属于「仅测试引用」16 项之一（P6-3 登记的
「测试测的是一条生产不走的路径」）。

lead 已裁定：删掉这条死路径，把测试意图迁到生产路径上，行为不变。

## 删除清单

| 符号 | 位置 | 删除依据 |
| --- | --- | --- |
| `set_hotkey_impl` | `src/ipc/settings.rs` | 生产零调用（grep 全仓仅 tests 6 处调用 + import） |
| `HotkeyConfig::rebind` | `src/hotkey.rs` | 唯一生产调用方就是 `set_hotkey_impl`，删除后零生产调用 |
| `HotkeyRegistrar` trait | `src/hotkey.rs` | 唯一生产消费者是 `rebind` 的入参，删除后零生产调用 |
| `AlwaysOkRegistrar` / `ConflictRegistrar` | `tests/ipc_settings.rs` | 只为 `set_hotkey_impl` 而存在的两个 fake |
| `hotkey_rebind_translate_isolates_field` | `tests/hotkey.rs` | 整条用例以 `rebind` 为主体 |
| `hotkey_rebind_main_isolates_field` | `tests/hotkey.rs` | 同上 |
| `hotkey_conflict_rejected` | `tests/hotkey.rs` | 同上（冲突断言走 `HotkeyRegistrar`） |

`HotkeyConfig` 的 `load` / `save` / `Default` / `get_accelerator` / `set_accelerator` 全部保留：
`load`/`save` 仍被 `lib.rs::register_hotkeys` 与 `get_hotkeys_impl` 使用。

### 一处刻意保留（待裁决）

`HotkeyError::AlreadyInUse` 现在**零构造者**。保留未删，理由：

1. 它是 `pub enum` 的一个变体（错误分类声明），不是带逻辑的死路径；rustc/clippy 不报，清单脚本
   只扫 `pub fn/struct/enum/trait` 声明、不扫变体，故也不会新增候选。
2. 它是旧路径那句用户可见文案「热键已被占用，无法绑定」的唯一载体。本任务明确要求不得静默
   改文案，而运行时路径的文案与之不同（见下节），统一与否需要 lead 决策——删掉它等于把可选
   的映射目标一起删了。
3. 删除 `pub` 错误变体超出 lead 的裁定范围（裁定只点名 `set_hotkey_impl` / `rebind` / `HotkeyRegistrar`）。

若 lead 决定「统一文案到运行时路径」或「彻底不要这个错误种类」，删这一个变体即可，无连带影响。

## 测试迁移映射表

旧测试名 → 新测试名（文件内 `#[test]` 名）。所有断言都在生产路径 `set_hotkey_runtime_impl` 上，
fake 用 `tests/ipc_settings.rs` 既有的 `RecordingRuntimeRegistrar`。

### `tests/ipc_settings.rs`

| 旧测试名 | 新测试名 | 迁移后的等价断言 |
| --- | --- | --- |
| `ipc_settings_set_hotkey_then_get_returns_new_value` | `ipc_settings_set_history_hotkey_runtime_then_get_returns_new_value` | ① 改 history 后读回新值；② translate / main 保持默认。**新增**：③ 事件序列 `register:History:<new>` → `unregister:<old>`（先注册、落盘成功才注销）；④ 同键早退（重复设置同一键不再产生 register/unregister 事件） |
| `ipc_settings_set_main_hotkey_then_get_returns_new_value` | `ipc_settings_set_main_hotkey_runtime_then_get_returns_new_value` | ① 改 main 后读回新值；② history / translate 保持默认（字段隔离） |
| （无，见"字段隔离"一行说明） | `ipc_settings_set_translate_hotkey_runtime_then_get_returns_new_value` | 承接旧 `hotkey_rebind_translate_isolates_field` 的字段隔离意图：改 translate 只改 translate，history/main 不动；注销的也是 translate 自己的旧键 |
| `ipc_settings_set_hotkey_conflict_rejected` | `ipc_settings_hotkey_conflict_rejected_runtime` | ① 冲突键返回 `Err`；② 注册失败的错误被**原样上抛**（未吞、未改写）；③ 冲突时配置不变（失败的 translate 没落盘、history 不受影响）；④ 失败发生在落盘与注销之前（旧键不注销）。**未迁移**的旧断言：`err_msg.contains("已被占用")`——见下节文案差异 |
| `ipc_settings_runtime_register_failure_keeps_config_and_old_shortcut` | 同名保留 | 仅前置动作从 `set_hotkey_impl` 改为 `set_hotkey_runtime_impl`；断言不变（配置不改、旧键不注销） |
| `ipc_settings_runtime_same_accelerator_is_noop` | 同名保留 | 仅前置动作同上；断言不变（no-op 成功、事件为空） |

### `tests/hotkey.rs`

| 旧测试名 | 新测试名 | 迁移后的等价断言 |
| --- | --- | --- |
| `hotkey_defaults_and_rebind` | `hotkey_defaults_and_save_load_roundtrip` | ① 三个默认键正确；② save/load 往返一致。原用例里「rebind 后内存配置更新」的一半由上面 3 条 runtime 用例覆盖 |
| `hotkey_defaults_and_rebind`（同一用例的另一半） | `hotkey_custom_accelerator_survives_save_load_roundtrip` | 「改键后 save/load 往返仍不丢、不串字段」：写一份非默认 history 的 JSON → load → save 到新路径 → 二次 load，三字段逐个校验 |
| `hotkey_rebind_translate_isolates_field` | `ipc_settings_set_translate_hotkey_runtime_then_get_returns_new_value` | 改 Translate 只改 Translate 字段，History/Main 不变 |
| `hotkey_rebind_main_isolates_field` | `ipc_settings_set_main_hotkey_runtime_then_get_returns_new_value` | 改 Main 只改 Main 字段，History/Translate 不变 |
| `hotkey_conflict_rejected` | `ipc_settings_hotkey_conflict_rejected_runtime` | 冲突 → `Err`；原配置不变。**未迁移**：`contains("已被占用")` |
| `hotkey_load_legacy_config_without_main_uses_default_main` | 同名保留 | 未受影响 |

没有迁移去向的两条断言，原因只有一个：**生产路径产生不出「已被占用」这个字符串**，详见下节。

## 用户可见错误文案核对（结论：存在差异，且不是本次改动引入）

冲突时两条路径的错误串：

| 路径 | 产生处 | 实际文案 |
| --- | --- | --- |
| 旧路径（已删） | `HotkeyConfig::rebind` → `HotkeyError::AlreadyInUse` 的 Display | 「热键已被占用，无法绑定」 |
| 生产路径（现役） | `TauriRuntimeHotkeyRegistrar::register_action_shortcut`：`format!("热键运行时注册失败: {e}")`，`e` 来自 `crate::register_action_shortcut` | 「热键运行时注册失败: HotKey already registered: ...」（`global_hotkey::Error::AlreadyRegistered` 的英文 Display，见 `global-hotkey-0.8.0/src/error.rs:27`） |

前端 `src/` 不依赖后端这句文案：

- `src/main-window/settings/rebind.ts` 的 `validateRebind` 在**前端预校验**阶段自己产出
  「已被占用」（比较三行热键是否撞车），`HotkeyPanel.tsx:119-123` 用它写 `conflictError`，
  `setHotkey` 根本不会被调用；后端错误串走另一分支 `setSaveError(err.message)`
  （`HotkeyPanel.tsx:126-133`），原样展示在 `role="alert"` 里。
- 因此 F2 验收里「绑定失败返回'已被占用'」在**同应用内两动作撞车**场景由前端文案满足；
  「被其它应用占用」场景下后端给的是英文原文，与旧路径的措辞不一致。

**结论**：本次删除**没有**改动任何用户可见文案——旧路径的文案生产上从未到达过用户
（那条路径零生产调用）。差异是既存的、本次只是把「两条文案」收敛成「一条」，并在测试里
把这条事实写清楚（迁移后的冲突用例只锁「错误原样上抛」，不锁措辞）。

**建议**：若要统一，最小改动是在 `TauriRuntimeHotkeyRegistrar::register_action_shortcut` 里把
底层错误映射成含「已被占用」中文前缀的串（或在 `global_hotkey::Error::AlreadyRegistered` 分支
前置 `HotkeyError::AlreadyInUse` 的文案），让 F2 验收文案在 OS 占用场景也成立；这属于**改用户
可见文案**的产品决定，不在本次授权内，故未做。

## 验证（实跑）

主工作区含他人未提交改动（`db.rs`/`tray.rs`/`lib.rs`/`ipc/*`），故另在 `git worktree add` 的
干净 HEAD 上应用本次补丁做隔离复现。

### 隔离 worktree（HEAD + 本次补丁）

```
cargo test --test ipc_settings --test hotkey
  hotkey:       test result: ok. 3 passed; 0 failed
  ipc_settings: test result: ok. 12 passed; 0 failed
cargo test            → lib 491 passed; 全部集成套件 0 failed
cargo clippy --all-targets -- -D warnings → Finished，无 error/warning
rustfmt --edition 2021 --check <本次 4 个文件> → exit 0
```

`cargo fmt --all --check` **仓级不干净**，但与本改动无关：干净 HEAD（未打补丁）上同样 exit 1，
差异文件只有 `src-tauri/src/tray.rs`（HEAD 本身未格式化，且不在本次写入范围）；主工作区另多出
他人未提交的 `db.rs`。本次 4 个文件的 rustfmt 检查 exit 0。

### 主工作区（最终状态）

```
cargo test --test ipc_settings --test hotkey → 15 passed（ipc_settings 12 + hotkey 3）
cargo test                                  → 491 passed (lib) + 各集成套件全绿
cargo clippy --all-targets -- -D warnings   → Finished，无 error/warning
```

### 死代码清单（同一工作树口径，改前 → 改后）

| 口径 | 候选合计 | 零引用 | 仅测试引用 | 仅本文件引用 |
| --- | --- | --- | --- | --- |
| 干净 HEAD worktree（改前） | 185 | 0 | 16 | 169 |
| 干净 HEAD worktree + 补丁（改后） | 184 | 0 | 15 | 169 |
| 主工作区（含他人改动，改前） | 187 | 0 | 16 | 171 |
| 主工作区（含他人改动，改后） | 186 | 0 | 15 | 171 |

`set_hotkey_impl` 从清单彻底消失；以 `--all` 输出对比候选**名字集合**，差异只有
`set_hotkey_impl` 一行（`diff` 输出 `157d156 < set_hotkey_impl`），**无新增候选**。

## 变异验证（实做，隔离 worktree）

把 `set_hotkey_runtime_impl` 里的同键早退分支删掉（只删 `if old_accelerator == accelerator
{ return Ok(()); }`，其余不动）：

```
test ipc_settings_set_history_hotkey_runtime_then_get_returns_new_value ... FAILED
  assertion failed: 同键早退不应新增注册/注销事件：
  ["register:History:...H", "unregister:...C", "register:History:...H", "unregister:...H"]
  left: 4 / right: 2
test ipc_settings_runtime_same_accelerator_is_noop ... FAILED
  no-op 不应调用运行时 register/unregister
test result: FAILED. 10 passed; 2 failed
```

红的两条里有一条是**迁移后的新测试**（`ipc_settings_set_history_hotkey_runtime_then_get_
returns_new_value`），说明迁移后的用例真的能捕获同键早退的丢失，而不是换个地方躺着。
`tests/hotkey.rs` 3 条全绿（本就不涉及注册，符合预期）。

还原后：`ipc_settings.rs` 的 diff 与变异前**逐字节一致**（`diff` 两份 patch hunk 无输出），
同一命令复跑 3 passed + 12 passed。

## 未做 / 未验证

- **`docs/dev-log/v0/acceptance.yaml` 出现两处悬空 test_id 引用**（本次写入范围不含该文件，
  未修改）：
  - `V0-F2-A01`（line 118）`ref: hotkey_defaults_and_rebind` → 建议改为
    `hotkey_defaults_and_save_load_roundtrip`（同文件、同 `cargo test ... hotkey` runner）。
  - `V0-F2-A02`（line 133）`ref: hotkey_conflict_rejected` + `path: src-tauri/tests/hotkey.rs`
    → 建议改为 `ipc_settings_hotkey_conflict_rejected_runtime` +
    `path: src-tauri/tests/ipc_settings.rs`（runner 的 `hotkey_conflict` 过滤仍能命中新名字，
    但 `cargo test --manifest-path src-tauri/Cargo.toml hotkey_conflict` 需确认 filter 生效）。
    同时 A02 的 assertion 文案「绑定失败返回'已被占用'错误」需要 lead 决定是否按上节建议统一。
  - `docs/design/v04-work-items.md` P6-3（line 338 / 348）仍描述旧状态，由 lead 统一回写。
- 历史 dev-log（`docs/dev-log/v0|v4|v5/**`）里提到 `set_hotkey_impl` / `rebind` /
  `hotkey_defaults_and_rebind` 的段落**未改**——它们是「当时发生了什么」的记录，不是现行契约。
- 真机未验证：未在 macOS 上实跑「被其它应用占用的热键」来观察真实错误串（文案结论来自
  `global-hotkey` 源码而非实测弹窗）。
- Windows 分支未编译验证（沿用既有口径）。
- 未跑前端门禁（本次未触及 `src/`）。
