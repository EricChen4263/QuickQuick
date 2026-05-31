---
id: V2-F1-S04-review
type: review
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A05]
evidence: []
author: code-reviewer
---

# 审查报告 · V2-F1-S04（凭据 schema + secret→keychain + 非密→加密DB）

## 审查范围
- `src-tauri/src/translate/credential.rs`、`mod.rs`、`db.rs`(ensure_schema)、`tests/translate.rs`(A05)
依据：code-standards（安全：密钥不入库/不入日志）+ 设计§4.1#4 + §十预埋铁律。

## 安全核心通过项
secret 绝不写 DB（is_secret 判定路由，A05-e COUNT=0 + store contains_key 双向断言）✓；全文无日志调用、CredError 不含 field_value ✓；CredentialField/CredError Debug 不泄漏凭据 ✓；SQL 全参数化 ✓；生产路径无裸 unwrap/panic ✓；CredError thiserror ✓；测试 MockCredStore+tempdir headless 不弹窗 ✓。

## 问题清单（Important，无 Critical）
**[I-1] 未知 field_key 静默降级写 DB（安全歧义，置信度 82）**
- 位置：`credential.rs` save_credentials（`schema.iter().find(...).map(|f| f.is_secret).unwrap_or(false)`）。
- 问题：field_key 不在 schema 时 is_secret 默认 false → 写 DB。若 secret 字段 key 拼错，secret 可能落库无提示，违反§4.1#4"路由必须由 schema 判定"。
- 修复：find 返回 None 时返回 CredError（新增 UnknownField 或扩展 UnknownProvider），不静默降级。

**[I-2] provider_config 表未进 db.rs ensure_schema（违§十预埋铁律，置信度 85）**
- 位置：`credential.rs` ensure_provider_config_table 按需建表，未并入 `db.rs::ensure_schema`；schema.rs 测试无 provider_config 断言（覆盖盲点）。
- 修复：把 provider_config 的 CREATE TABLE IF NOT EXISTS 并入 db.rs ensure_schema；tests/schema.rs 加 provider_config 列存在断言。

**[I-3] 缺未知 provider/field 负向用例（置信度 80）**
- 位置：tests/translate.rs A05 区，仅正向路由，无 save_credentials("nonexistent",..) / 未知 field_key 负向用例（与 I-1 呼应）。
- 修复：补负向测试：未知 provider_id / 未知 field_key 调 save_credentials 应返回错误而非静默写 DB。

## 结论
**未过（打回）。** 修 I-1（未知字段报错）+ I-2（provider_config 并入 ensure_schema + schema 断言）+ I-3（负向用例）后复审。

---

## 复审结论（第2轮 · 2026-05-31）

**status = 通过**

- **I-1 已解决**：save_credentials 未知 provider→UnknownProvider、未知 field→UnknownField（不携带 secret 值），静默写 DB 路径彻底移除。
- **I-2 已解决**：db::ensure_schema 预埋 provider_config 表（含主键），credential.rs 不独立建表；tests/schema.rs 断言三列存在。
- **I-3 已解决**：tests/translate.rs 补未知 provider/field 负向用例（返回 Err + 未知 field DB 行数=0 不降级）。
无新引入≥80 高危；secret 路由安全不变；无装饰注释/TODO。
