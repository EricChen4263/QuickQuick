---
id: s04-auto-paste-test
title: 真实自动粘贴 动态证伪
status: 测试通过
commit: dbc198c
date: 2026-06-02
---

# 真实自动粘贴 (9a+9b) 动态证伪报告

## 命中校验

**cargo test 全量运行（1次，均为单测无并发 flaky 风险）**

- lib 单元测试：81 passed，0 failed
- 集成测试套件：全部通过（autostart、boot、clipboard、db、hotkey、image、ipc_clipboard、ipc_settings、ipc_translate、ipc_validation、keyprovider、macos_backends、onboarding、paste、portable、privacy、providers、schema、translate 合计多套）
- doc-test：1 passed

**三态测试命中确认（ipc::system::tests）**

- T6 `map_outcome_full_paste_done_returns_full_paste` ... ok
- T7 `map_outcome_write_back_only_done_returns_write_back_only` ... ok
- T8 `paste_orchestrate_trusted_normal_returns_full_paste` ... ok
- T9 `paste_orchestrate_untrusted_returns_write_back_only` ... ok
- T10 `paste_orchestrate_trusted_timeout_returns_write_back_only` ... ok

onboarding 集成测试命中确认（tests/onboarding.rs）：

- `accessibility_onboarding_degrade_trusted_full_paste` ... ok
- `accessibility_onboarding_degrade_trusted_perform_calls_send_paste` ... ok
- `accessibility_onboarding_degrade_untrusted_shows_card_and_deeplink` ... ok
- `accessibility_onboarding_degrade_untrusted_write_back_only_no_paste` ... ok

macos_backends 构造测试（tests/macos_backends.rs）：

- `accessibility_probe_impl_exists_and_constructable` ... ok
- `paste_backend_impl_exists_and_constructable` ... ok

无空匹配假绿，全部 N>=1 真命中。

## 变异 sanity

每处变异均先 `cp <文件> /tmp/<文件>.bak` 备份，还原用备份 `cp /tmp/<文件>.bak <文件>`，不使用 `git checkout`。

### 变异 A：map_outcome FullPasteDone 映射篡改

- 改动：`src/ipc/system.rs` 第 138 行 `PasteOutcome::FullPasteDone => "full_paste"` 改为 `=> "write_back_only"`
- 跑 T6 (`map_outcome_full_paste_done_returns_full_paste`)：**FAILED** - left="write_back_only" right="full_paste"
- 跑 T8 (`paste_orchestrate_trusted_normal_returns_full_paste`)：**FAILED** - 同因
- 结论：如期变红，测试有判别力
- 还原：从 /tmp/system.rs.bak 复原，grep 确认 `FullPasteDone => "full_paste"` 恢复

### 变异 B：paste_orchestrate 超时分支篡改

- 改动：`src/ipc/system.rs` 第 156 行 `Err(_timeout) => "write_back_only"` 改为 `=> "full_paste"`
- 跑 T10 (`paste_orchestrate_trusted_timeout_returns_write_back_only`)：**FAILED** - left="full_paste" right="write_back_only"
- 结论：如期变红，超时路径有独立测试覆盖
- 还原：从 /tmp/system.rs.bak 复原

### 变异 C：perform_paste_or_degrade trusted 分支路由篡改

- 改动：`src/onboarding.rs` trusted 分支中把 `write_then_paste(backend, item)?` 改为 `backend.write_with_marker(item)`，`Ok(PasteOutcome::FullPasteDone)` 改为 `Ok(PasteOutcome::WriteBackOnlyDone)`（模拟路由对调）
- 跑 T8 (`paste_orchestrate_trusted_normal_returns_full_paste`)：**FAILED** - left="write_back_only" right="full_paste"
- 结论：如期变红，trusted 路由与 send_paste 调用断言均能拦截此类错误
- 还原：从 /tmp/onboarding.rs.bak 复原，grep 确认 `write_then_paste` 与 `FullPasteDone` 恢复

### 变异 D（只读判断）：macos_backends 构造测试有效性

- `paste_backend_impl_exists_and_constructable` 在 macOS 上真实调用 `change_count()`、`write_with_marker()`，并断言 `count_after > count_before`
- 不是空壳 `let _ = Struct` 测试，能检验 NSPasteboard 写操作确实触发 changeCount 递增
- 判断：有效，不需变异

## OS 边界隔离说明

以下操作属于 OS 硬边界，无法 headless 单测，本次证伪不覆盖：

- `CGEvent` Cmd+V 键盘注入实际触发（需运行 GUI + 授权 AXIsProcessTrusted）
- `AXIsProcessTrusted()` 真实返回值（headless 下恒为 false）
- 窗口 hide（需 Tauri AppHandle + 主线程）

测试覆盖的是决策逻辑（非 OS 注入本身）：`FakeProbe.is_trusted()` 控制分支，`FakeBackend.send_paste_called` 检验路由；超时路径通过 `FakeBackend::frozen_count()`（changeCount 不递增）触发 Timeout 错误，经 `paste_orchestrate` Err 臂映射为 `"write_back_only"`。整条链真实有效，OS 不可测部分被正确隔离在 fake 之外。

## git 干净证明

开工快照：`git status --porcelain` 输出空（clean）。
结束快照：`git status --porcelain` 输出空（clean）。
两次快照逐行一致。全部变异均通过备份还原，工作树无残留改动。
