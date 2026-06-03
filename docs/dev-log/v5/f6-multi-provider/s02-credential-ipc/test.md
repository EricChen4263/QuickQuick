---
id: s02-credential-ipc
title: 凭据存取 IPC 测试留痕
status: passed
commit: e838919
date: 2026-06-03
---

# 测试留痕：多翻译源·批次B 凭据 IPC（s02）· 动态证伪

## 命中校验
- `cargo test -p quickquick` 全量：**336 passed / 0 failed**（批次A 330 → +6）
- `cargo build -p quickquick`：exit 0（3 命令注册 + Tauri 参数反序列化编译通过）

## 变异 sanity（三变异，均如期红→复绿）
- **变异A（安全·secret 泄漏）**：`get_provider_credentials_impl` secret 分支 `value: None` 改成 `value: saved_value.clone()`（回明文）→ 测试 `get_provider_credentials_impl_secret_field_value_is_always_none` **FAILED**（"secret 字段 value 永远应为 None"）→ 还原复绿。**安全断言有测试专项守护**。
- **变异B（schema 映射）**：`get_provider_credential_schema_impl` 改返 `vec![]` → `..._baidu_returns_two_fields` FAILED（0≠2）→ 还原复绿。
- **变异C（set 落库旁路）**：`set_provider_credentials_impl` 改 `Ok(())` 不存 → `..._persists_and_loadable`（None≠Some）+ `..._unknown_field_returns_err` 同时 FAILED → 还原复绿。

## 安全核对
- get 路径 secret 分支硬编码 `value: None`，明文仅用于 `.is_some()` 布尔，**变异A 证明有测试守护**。
- set 错误路径仅 `e.to_string()`（CredError 只含字段名不含值），无 log/println 打印 values。
- **secret 不回明文 / 不入日志：确认**。

## 门禁结论
**PASS（放行）**
- 全量 336 全绿、build exit 0
- 三变异均有测试守护（含安全断言 A）
- 安全红线通过
- 工作树备份还原后与开工逐行一致，无残留

> keyed provider 真能填 key→存→翻译走该源，需批次 C 接通前端后 GUI 实测。
