---
id: V5-F6-report
type: feature_report
level: 大功能
parent: V5
created: 2026-06-03T00:00:00Z
status: 已完成（代码门禁全过，待 GUI 实测）
author: orchestrator
children: [f6-s01-backend-routing, f6-s02-credential-ipc, f6-s03-frontend-form]
---

# 大功能报告 · V5-F6 多翻译源端到端接通

## 目标
让「翻译源可配置」真正可用。现状缺陷（审计确认）：① translate 硬编码 MyMemory，切换 selected_provider 形同虚设；② 需 key 的 provider（百度/DeepL/Google）无填 key 入口（凭据框架 credential.rs 已实现但未暴露 IPC、前端无表单）。

## 用户决策
完整接通（schema 驱动）。keyed provider 需用户自备 API key。

## 分批实施

| 批次 | 内容 | 验证 | 锚点 commit |
|---|---|---|---|
| s01 后端动态路由 | translate_text_impl 改为读 selected_provider→load_credentials→build_provider 动态选源；历史写真实 provider_id（不再硬编码 mymemory）；缺 key 返回明确错误不回退 | tester 三变异（含首轮打回补测动态路由历史不变量）+ 安全(secret 不泄漏)；reviewer 通过；cargo 330 全绿 | e838919 |
| s02 凭据 IPC | 暴露 get_provider_credential_schema / get_provider_credentials（secret 只回 isSet）/ set_provider_credentials（密入 keychain，HashMap 入参） | tester 三变异（含安全断言 secret 泄漏守护）全红复绿；reviewer 通过安全红线明确；cargo 336 全绿 | e838919 |
| s03 前端 schema 驱动表单 | 选中 keyed provider 就地展开 key 表单（password+留空不修改）→保存→徽标转「已配置」；ipc-client 包装 + isProviderConfigured 纯函数 + CredentialForm 组件 | tester 三变异全红复绿、跨端契约静态一致；reviewer 通过且逐项核对命令名/参数键/DTO camelCase 与后端一致；前端 413 全绿 tsc 干净 | e838919 |

## 关键设计
- **动态路由**：build_provider(id, creds) 按 id 构造 provider 并注入凭据，字段名与 credential_schema 严格对齐；mymemory 无 key 照常；keyed 缺 key→中文 Err 不静默回退。
- **安全红线**：secret 不回显明文（get 只返 isSet）、不入日志/错误消息；密入 keychain、非密入加密 DB。

## 待 GUI 实测
- 切换翻译源后翻译实际走对应 provider（批次 C 接通后）。
- keyed provider 填 key→保存→徽标「已配置」→翻译走该源。
