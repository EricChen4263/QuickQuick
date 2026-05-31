---
id: V3-F3-S09-test
type: test_report
level: 小功能
parent: V3-F3
created: 2026-05-31T03:39:52Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A11]
author: tester
---

# V3-F3-S09 macOS Accessibility 引导与优雅降级 — 测试报告

## 运行命令

```bash
# 1. A11 验收测试（集成测试 onboarding）
cargo test --manifest-path src-tauri/Cargo.toml --test onboarding

# 2. 全量单元+集成测试
cargo test --manifest-path src-tauri/Cargo.toml

# 3. Clippy 静态检查
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# 4. 编译检查
cargo build --manifest-path src-tauri/Cargo.toml
```

## 结果汇总

| 检查项 | 退出码 | 结论 |
|--------|--------|------|
| A11 验收测试（onboarding）| 0 | 通过 |
| 全量测试 | 0 | 通过 |
| Clippy lint | 0 | 通过（零 warning） |
| cargo build | 0 | 通过 |

## A11 验收测试真命中（3/3）

测试套件 `tests/onboarding.rs`，函数名均含 `accessibility_onboarding_degrade`：

| # | 用例名 | 结果 |
|---|--------|------|
| 1 | `accessibility_onboarding_degrade_trusted_full_paste` | ok |
| 2 | `accessibility_onboarding_degrade_untrusted_shows_card_and_deeplink` | ok |
| 3 | `accessibility_onboarding_degrade_untrusted_write_back_only_no_paste` | ok |

原始输出：
```
running 3 tests
test accessibility_onboarding_degrade_trusted_full_paste ... ok
test accessibility_onboarding_degrade_untrusted_shows_card_and_deeplink ... ok
test accessibility_onboarding_degrade_untrusted_write_back_only_no_paste ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 全量测试汇总

```
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
```

共 78 个用例，0 失败，0 跳过。

## 覆盖缺口

无。本次变更（`onboarding.rs` + `tests/onboarding.rs`）已由 3 个 A11 验收测试完整覆盖，分别对应：

- 已授权路径（FullPaste 返回 Proceed）
- 未授权路径-引导卡片+深链接
- 未授权路径-仅写回不触发粘贴

## 结论

**允许进入下一任务。** A11 验收标准全部真命中（3/3），全量 78 用例零失败，clippy 零警告，build 成功。硬门禁通过。
