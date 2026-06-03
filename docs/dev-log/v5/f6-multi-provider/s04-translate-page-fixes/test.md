---
id: s04-translate-page-fixes
title: 翻译页修复 测试留痕
status: passed
commit: 9dcf287
date: 2026-06-03
---

# 测试留痕：翻译页修复（s04）· 动态证伪

## 命中校验
- 前端 `pnpm test --run`：**427 passed (47 files)**；`tsc --noEmit` exit 0
- `cargo test -p quickquick`：**336 passed**；`cargo build -p quickquick` exit 0
- DirBar.test.tsx 15 全绿；translate-page.test.tsx 25 全绿

## 变异 sanity（四变异，均如期红→复绿）
| 变异 | 改坏 | 结果 |
|---|---|---|
| ③-A | catch 改回固定文案「翻译失败」 | 「显示真实错误(百度翻译签名错误)」测试变红 → 复绿 |
| ③-B | 错误块 class 改回 `.tx-error`（回顶部） | 「错误在结果区/无顶部 .tx-error」测试变红 → 复绿 |
| ①-A | DirBar disabled 改回 `p.needsKey`（忽略 configuredIds） | 「已配置 keyed 源不 disabled」测试变红 → 复绿 |
| ①-B | 注释掉 PROVIDER_CONFIG_CHANGED_EVENT 监听 | 「收到事件→解禁」测试变红(超时) → 复绿 |

## 静态核对
- 事件名两端一致：events.ts / settings.rs 均 `"provider-config-changed"`。
- emit 仅 set_provider_credentials_impl 成功后调用（`?` 提前返回不误发）。
- handleTranslate：`err instanceof Error ? err.message : 兜底`，真实错误透传。

## 门禁结论
**PASS（放行）**
- 全量前端 427 + cargo 336 全绿、build exit 0
- 四变异均有测试守护、红复绿如期
- 事件名一致、错误透传、emit 时机正确
- 工作树无残留业务代码改动

> ② 百度翻译报错本身待 GUI 重试捕获真实错误后另行精修，本 s04 未碰 provider 实现。
