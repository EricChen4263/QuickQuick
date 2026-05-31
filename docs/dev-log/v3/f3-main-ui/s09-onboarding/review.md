---
id: V3-F3-S09-review
type: review
level: 小功能
parent: V3-F3
children: []
created: 2026-05-31T05:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F3-A11]
evidence: []
author: code-reviewer
---

# V3-F3-S09 macOS Accessibility 引导与优雅降级 — 审查记录

## 审查范围
- `src-tauri/src/onboarding.rs`（AccessibilityProbe trait/OnboardingAction/PasteCapability/onboarding_action/paste_capability/perform_paste_or_degrade/ACCESSIBILITY_DEEPLINK）+ `tests/onboarding.rs`
依据：code-standards + 设计§二（Accessibility 引导+降级）。

## 总结论
**通过。** Critical 维度全合格；2 条 Important 测试质量建议已于 polish 落实。

## Critical 维度核查（通过）
- **未授权不崩溃/降级不发粘贴**：perform_paste_or_degrade 未授权分支仅 write_with_marker、不触达 send_paste、返回 Ok(WriteBackOnlyDone) 无 panic；测试 `untrusted_write_back_only_no_paste` 以 `send_paste_call_count==0` 精确守护（对比授权 ==1，非恒真）。
- **检测抽象**：AccessibilityProbe trait 仅 is_trusted()，与 AXIsProcessTrusted 解耦，FakeAccessibilityProbe 两态 headless 测。
- **引导决策**：onboarding_action 未授权→ShowCardAndDeepLink/授权→Proceed；ACCESSIBILITY_DEEPLINK = `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility` 指向辅助功能（§二）。
- code-standards：无裸 unwrap/panic、无装饰注释、无 TODO、函数 ≤10 行、命名规范。

## Important 建议（已于 polish 修复）
- **建议1（深链断言过宽，置信度 82）**：原 `contains("Accessibility")` → 已改 `assert_eq!(ACCESSIBILITY_DEEPLINK, "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility", ...)` 精确断言。
- **建议2（trusted 测试 AAA 混合，置信度 80）**：原 trusted_full_paste 三 Act 混合致 send_paste 断言可能被前置屏蔽 → 已拆出独立测试 `accessibility_onboarding_degrade_trusted_perform_calls_send_paste`（单 Act 断言 send_paste_call_count==1），覆盖 3→4。

## 验收项覆盖
| 验收 | 测试 |
|---|---|
| A11 未授权卡片+深链 | untrusted_shows_card_and_deeplink |
| A11 未授权降级 WriteBackOnly 不发粘贴不崩 | untrusted_write_back_only_no_paste（send_paste_call_count==0） |
| A11 授权 FullPaste+send_paste | trusted_full_paste + trusted_perform_calls_send_paste（==1） |

## 结论
**通过。** A11 三路覆盖（未授权卡片深链/未授权降级不发粘贴不崩/授权完整粘贴），核心安全验证；2 Important 测试质量建议已 polish 落实。
