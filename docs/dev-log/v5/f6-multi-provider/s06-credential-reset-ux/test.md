---
id: s06-credential-reset-ux
title: 凭据清除/重置 UX 测试留痕
status: passed
commit: 13d7f9b
date: 2026-06-04
---

# 测试留痕：凭据清除/重置 UX（s06）· 动态证伪

## 命中校验
- 前端 `pnpm test --run`：**435 passed (47 files)**；`tsc --noEmit` exit 0
- `cargo test -p quickquick`：**347 passed**；`cargo build -p quickquick` exit 0
- 后端 delete 专项 9 测试全绿（delete_credentials 删 secret/删 DB/幂等/未知 provider→Err、delete_secret 各 store、impl 层 3 测）
- 前端 CredentialForm 清除相关 + TranslateSourcePanel 回退测试命中

## 变异 sanity（均如期红→复绿）
- 后端A（delete_credentials 跳过 store.delete_secret）→「删除后 get_secret 返 None」变红。
- 前端A（去掉 window.confirm 守卫）→「取消确认时不调 delete」变红。
- 前端D（isConfigured 恒 true 渲染清除按钮）→「未配置/默认不显示清除按钮」两测变红。
- 后端B（emit）：glue 不可单测，注明。
- 备份还原后 git status 一致。

## 静态核对
- delete 路径不打印凭据值（SQL 参数/keychain account 仅 provider_id+field_key，无 secret 值）。
- 幂等：KeyringCredStore NoEntry→Ok、delete_from_db 0 行→Ok、MockCredStore remove 不存在 key 无副作用。
- emit 仅 with_db 成功（`?`）后调用。
- confirm=false 第一行 return，不触发任何 IPC（变异验证）。
- 跨端命令名/参数键一致：`delete_provider_credentials` / `providerId`→`provider_id`。

## reviewer Important(85) 已修
`handleCredentialSaved` 缺 `.catch()`：补 `.catch()` 保守回退（刷新失败时把该 provider 从 configuredIds 移除→徽标回「待配置」，isMounted 守卫 + console.error）+ 新增回退测试（清除后 getProviderCredentials reject → 徽标变「待配置」）。前端从 434→435。

## 门禁结论
**PASS（放行）**
- 前端 435 + cargo 347 全绿、build exit 0
- 变异全有测试守护、红复绿如期
- 安全（不泄漏凭据）、幂等、emit 时机、confirm 守卫、跨端契约均核对
- Important(85) 已修并补测
- 工作树无残留业务代码改动
