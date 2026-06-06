---
id: RT1-F2-S01-test
type: test_report
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 2b5d985
acceptance_ids: [RT1-F2-A01, RT1-A-SEC]
evidence:
  - src/panels/clipboard/ClipPreview.test.tsx
  - src/clip-popover/PopoverPreview.test.tsx
author: tester
---

# 测试报告 · RT1-F2-S01 富文本预览渲染 + DOMPurify 清洗

## 运行的测试命令
```
pnpm exec vitest run --reporter=verbose ClipPreview.test.tsx PopoverPreview.test.tsx
pnpm test            # 全量
pnpm exec tsc --noEmit
# 变异 sanity：cp 备份 → 改 → 跑 → 从备份还原（禁 git checkout）
```

## 结果
**通过**（命中无假绿 + 关键安全变异 RED + 连跑 5 次无 flaky）

## 用例清单 + 结果
| 用例 | 结果 | 对应验收项 |
|---|---|---|
| clip_preview_renders_sanitized_richtext | pass | RT1-F2-A01 |
| clip_preview_plaintext_unchanged | pass | RT1-F2-A01 |
| clip_preview_strips_malicious_html（script/onerror/javascript:/iframe） | pass | RT1-A-SEC |
| popover_preview_renders_sanitized_richtext | pass | RT1-F2-A01 |
| popover_preview_strips_malicious_html（含 iframe） | pass | RT1-A-SEC |

## 变异 sanity（cp 备份还原）
| 变异 | 预期 | 实测 |
|---|---|---|
| `sanitizeRichHtml` 改为 `return html`（去清洗） | 两个安全测试 RED | FAILED：恶意标签未剥离 ✓（证明安全断言有判别力，非恒真） |
| ClipPreview richtext 分支改渲染 `{content}`（旁路 innerHTML） | 渲染测试 RED | FAILED：无 `<b>`/`<table>` ✓ |
| iframe 断言取反 `.not.toBeNull()` | 该断言 RED | FAILED ✓（确认 querySelector('iframe') 真作用于渲染结果） |

## 覆盖率
覆盖 RT1-F2-A01（富文本渲染 + 纯文本回退，主窗口 + popover）与 RT1-A-SEC（四类恶意 payload 全剥离）。列表行不变（仍纯文本摘要）。

## 边界探测
纯文本不走 innerHTML（{content} 字面）、htmlContent undefined 走纯文本、空串 sanitize 返空——均 OK。连跑 5 次（含 2 次全量）无 flaky。

## 结论
RT1-F2-A01 + RT1-A-SEC PASS。tsc 0 错，全量 476 passed。git status 开工/结束逐行一致（cp 还原无残留）。
