---
id: F6-S13-review
type: review
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · dev/release 数据目录隔离（F6-S13）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/ipc/settings.rs` | 新增 `pub fn apply_dev_subdir(base, is_debug)`；`resolve_config_dir` 改用它决定最终目录 |
| `src-tauri/src/lib.rs` | 5 处 `app_config_dir()` 直调全改走 `ipc::settings::resolve_config_dir` |
| `src-tauri/tests/config_dir_isolation_test.rs` | 新增 2 条纯路径逻辑集成单测 |

参照标准：项目规范 + code-standards skill（函数≤50行 / 注释写"为什么" / cfg 对称 / 命名描述性）。

---

## 重点检查判定

### 1. release 红线：已发布用户零迁移（通过）

`apply_dev_subdir(base, false)` 直接返回 `base.to_path_buf()`，等价原值。
`resolve_config_dir` 在 release 构建（`cfg!(debug_assertions)=false`）下行为：取 `app_config_dir()` → 不追加子目录 → `create_dir_all` → 返回根目录，与原 `app.path().app_config_dir()` + 手动 `create_dir_all` 完全等价。
release 路径不变，已发布 v0.1.0 用户数据无迁移压力。**通过**。

### 2. 降级语义保留（通过）

逐处核对 lib.rs 5 处改动后的错误处理路径：

| 调用位置 | 原降级语义 | 改后语义 |
|---|---|---|
| `setup_app_db`（~193 行） | `app_config_dir()` Err → eprintln + None | `resolve_config_dir` Err → eprintln + None，一致 |
| `start_clipboard_poll` 捕获轮询（~277 行） | `app_config_dir()` Err → `.ok()` → None → `unwrap_or_else(默认值)` | 同路径，一致 |
| `apply_autostart_preference`（~313 行） | `app_config_dir()` Err → eprintln + return | `resolve_config_dir` Err → eprintln + return，一致 |
| `register_hotkeys`（~445 行） | `app_config_dir()` Err → `.ok()` → None → `unwrap_or_default()` | 同路径，一致 |
| `init_capture_state`（~476 行） | `app_config_dir()` Err → `.ok()` → None → `unwrap_or_default()` | 同路径，一致 |

无任何处被改成 unwrap/panic 或吞掉错误。**通过**。

### 3. 去重正确性：create_dir_all 无遗漏无重复（通过）

- `setup_app_db` 原有手动 `create_dir_all` 已删除，`resolve_config_dir` 内含（settings.rs:242），无遗漏。
- `apply_autostart_preference` 原有手动 `create_dir_all` 已删除，同上。
- `register_hotkeys` 原有 `.map(|dir| { let _ = std::fs::create_dir_all(&dir); dir.join(...) })` 已简化，同上。
- `start_clipboard_poll` 和 `init_capture_state` 原来就无手动 `create_dir_all`，不受影响。
- `resolve_config_dir` 单点执行一次 `create_dir_all`，无重复调用。**通过**。

### 4. API 可见性评估（观察，非阻塞）

`apply_dev_subdir` 被声明为 `pub`（crate 外可见），原因是集成测试文件 `tests/config_dir_isolation_test.rs` 需通过 `use quickquick_lib::ipc::settings::apply_dev_subdir` 导入。

评估：由于 `ipc::settings` 模块和 `ipc` mod 均已是 `pub`，此函数实际对所有下游 crate 公开，超出其「内部测试辅助」的意图范围。

可选的更低成本方案：添加 `#[doc(hidden)]` 标注，明示这是内部辅助函数、不属于稳定公开 API；或改为 `pub(crate)` 并将测试改用内联 `#[cfg(test)]` 模块（需评估 TDD 守卫钩子兼容性）。

鉴于 coding.md 中已注明「本仓 TDD 守卫钩子不识别内联 `#[cfg(test)]`」，当前 `pub` + `tests/` 的方案是合理权衡。建议补 `#[doc(hidden)]` 以降低 API 面误导风险，但不强求、不阻塞。**置信度 65，不阻塞**。

### 5. 注释 stale 问题（通过，轻微误导）

`resolve_config_dir`（settings.rs:233）上方注释仍写：「此函数是不可单测的胶水层，仅供命令函数调用。」

分析：该注释指的是「此函数本身依赖 `AppHandle` 运行时，整体不可单测」，语义在技术上没有错。但经过本次改动，其核心路径逻辑（`apply_dev_subdir`）已被抽出并有单测——读者可能误以为「整个目录解析逻辑都不可测」，形成误导。

该注释不影响运行时行为，置信度不达 80，记为低置信度观察，不阻塞。**置信度 55，不阻塞**。

### 6. 规范合规性（通过）

- **函数长度**：`apply_dev_subdir` 5 行，`resolve_config_dir` 约 10 行，`setup_app_db` 约 28 行，均 ≤ 50 行。
- **cfg 对称**：`apply_dev_subdir(is_debug=true)` = `base.join("dev")`，`apply_dev_subdir(is_debug=false)` = `base.to_path_buf()`，两分支均编译、语义互补，对称正确。`setup_app_db` 中的 `#[cfg(debug_assertions)]` / `#[cfg(not(debug_assertions))]` provider 选择也保持对称。
- **注释写"为什么"**：`apply_dev_subdir` doc 注释解释了隔离原因（SQLCipher 密钥不匹配）、release 不变的零迁移理由、以及抽成纯函数的单测目的，符合要求。
- **无装饰注释**：未发现横线分隔符等装饰性注释。
- **无死代码**：无未使用代码路径。
- **命名描述性**：`apply_dev_subdir`（动词+名词）、`is_debug`（is 前缀布尔参数），符合规范。

### 7. 单测覆盖充分性（通过）

两条集成测试（`config_dir_isolation_test.rs`）覆盖 `apply_dev_subdir` 的两个分支，使用具体路径值做 AAA 断言，能有效检验两种构建类型的路径语义。`is_debug` 参数由外部传入（非 `cfg!` 宏），两分支在同一测试构建内均可执行，无条件编译盲区。**通过**。

---

## 问题列表

**无置信度 ≥80 的 Critical 或 Important 问题。**

以下为置信度 <80 的观察，供参考，不阻塞：

| 置信度 | severity | 位置 | 描述 | 建议 |
|---|---|---|---|---|
| 65 | Important | `src-tauri/src/ipc/settings.rs:253` | `apply_dev_subdir` 为 `pub` 而非 `pub(crate)`，对所有下游 crate 开放；超出其"内部测试辅助"意图 | 补 `#[doc(hidden)]` 降低 API 面误导风险；或若未来 TDD 守卫升级支持内联测试，可改回 `pub(crate)` |
| 55 | - | `src-tauri/src/ipc/settings.rs:233` | `resolve_config_dir` doc 注释「不可单测的胶水层」在核心逻辑已可测后略有误导，但技术上仍指「整体依赖运行时」 | 可将注释更新为「此函数整体依赖 AppHandle 运行时、不可直接单测；可测的纯路径逻辑已提取至 `apply_dev_subdir`」 |

---

## 无其他置信度 ≥80 问题

- release 路径完全等价原行为，零迁移风险已确认。
- 5 处降级语义（eprintln + None/return/默认值）逐一核实，无新增 unwrap/panic。
- `create_dir_all` 已去重，单点收口在 `resolve_config_dir`，无遗漏也无重复。
- `apply_dev_subdir` 实现正确：debug 分支 `base.join("dev")`，release 分支 `base.to_path_buf()`，cfg 对称。
- 单测两条覆盖两个分支，使用外部传参绕过条件编译盲区，有效性充分。
- 函数长度、命名、注释风格、无死代码、无装饰注释均符合规范。

---

## 审查结论

**通过（APPROVE）。**

改动逻辑清晰、release 红线安全（零迁移）、5 处降级语义原样保留、`create_dir_all` 去重正确、单测覆盖纯路径逻辑。两条低置信度观察（`pub` 可见性 + 注释微调）已列入问题表，不阻塞合并。

---

**VERDICT: APPROVE**

无置信度 ≥80 的 Critical 或 Important 问题。两条低置信度观察供参考，不阻塞。
