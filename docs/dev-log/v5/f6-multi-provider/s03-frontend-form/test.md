---
id: s03-frontend-form
title: 前端 schema 驱动凭据表单 测试留痕
status: passed
commit: e838919
date: 2026-06-03
---

# 测试留痕：多翻译源·批次C 前端凭据表单（s03）· 动态证伪

## 命中校验
- `pnpm test --run`：**Test Files 47 passed / Tests 413 passed**
- `pnpm tsc --noEmit`：exit 0，无类型错误
- 专项：credential-utils 4、CredentialForm 5、TranslateSourcePanel 4，全命中

## 变异 sanity（三变异，均如期红→复绿）
- **变异A（isProviderConfigured 恒真）**：改 `.every(()=>true)` → 「缺 required→false」测试全红 → 还原复绿。
- **变异B（空 secret 覆盖保护）**：守卫加 `&& false`（空 secret 也写入）→ 「保存时空 secret 不传」测试精确变红 → 还原复绿。
- **变异C（永不展开表单）**：`if(false && needsKey)` → 「选 baidu 出现凭据表单」测试精确变红 → 还原复绿。
- 备份 /tmp 还原后 git status 与开工逐行一致。

## 静态核对
- invoke 命令名 + 参数键正确：`{providerId}` / `set_provider_credentials {providerId, values}`，values 是对象非 [[k,v]]。
- 空 secret 保护有测试守护（变异B 精确捕获）。
- secret 前端按 value:null/isSet 处理，password input，不显明文。

## 盲区（需 GUI 实测，非单测可覆盖）
- Tauri 参数名 camelCase→snake_case 真实映射、HashMap 序列化是否到达 Rust handler。
- 后端真实加密存取凭据、schema 内容与前端类型对齐。
- setProviderCredentials 成功后徽标刷新流（真实 IPC 回调）。
> reviewer 已静态对后端 settings.rs 逐项核对命令名/参数键/DTO camelCase **全部一致**，运行时风险显著降低，但最终仍以 GUI 实测为准。

## 门禁结论
**PASS（放行）**
- 全量 413 全绿、tsc 干净
- 三变异均有测试守护
- 跨端契约静态一致（reviewer 复核）
- 工作树还原干净，无残留
