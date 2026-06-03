---
id: V5-F6-S05-review
type: review
level: 小功能
parent: V5-F6
children: []
created: 2026-06-03T05:30:00Z
status: 通过
commit: 2189d6a
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 百度签名错误修复（V5-F6-S05）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/translate/providers.rs` | diff | `build_provider` 的 `find` 闭包加 `.map(|v| v.trim()).filter(|s| !s.is_empty())`，新增 2 条 Rust 单测（+44 行 -1 行） |

参照：Rust code-standards（函数≤50行 / 嵌套≤3层 / 安全红线（日志不含凭据值）/ 测试 AAA+行为化+非弱断言）、项目规范。

---

## 核心正确性判定（明确：通过）

### trim 覆盖全部 provider 全部字段

`find` 闭包是所有 provider 字段查找的唯一入口（L29-35）。改动点：

```rust
.map(|(_, v)| v.trim())
.filter(|s| !s.is_empty())
```

覆盖路径：
- `mymemory` → `email`（可选，trim 后空 → `None` → 走无 email 路径，正确）
- `baidu` → `app_id`、`secret_key`（均必填，trim 后空 → `None` → `.ok_or_else(...)` → `Err`，正确）
- `deepl_free` → `auth_key`（必填，同上）
- `google` → `api_key`（必填，同上）

结论：trim 统一在 `find` 闭包内完成，一处改动覆盖所有 provider 所有字段，无遗漏。

### trim 后空 = 缺失路径判定（明确：正确）

数据流：`v.trim()` 返回 `&str` → `.filter(|s| !s.is_empty())` 使全空白值 filter 掉 → 整个 `find(key)` 返回 `None` → 必填字段调用 `.ok_or_else(|| "...未配置...".to_string())?` → 函数提前返回 `Err`，不构造 provider。全空白凭据不会传给任何 `ProviderNew::new()`。

### 签名根因修复验证

`BaiduProvider::build_request`（L207-234）：
- `appid` 进 body：`percent_encode(&self.app_id)`，trim 后值不含空白，不会产生 `%20`
- `secret_key` 进签名：`baidu_sign(&self.app_id, &req.text, &salt, &self.secret_key)`，签名计算的是 trim 后值，与百度服务器存储的原始 key（本不含空白）一致，54001 根因消除

`secret_key` 不出现在 body 明文，无安全暴露风险。

---

## 安全合规（通过）

错误消息（L44、L46、L51、L56）格式：`"baidu 未配置 AppID，..."` 等——只含字段语义名，不含字段值。符合「日志不打印敏感信息」规范。

---

## 函数规范（通过）

`build_provider` 函数体 L24-61，共 37 行，符合函数 ≤ 50 行规则。`find` 闭包为 5 行（L29-35），嵌套层级 2，符合嵌套 ≤ 3 层规则。

---

## 测试质量分析

### 测试 1：`build_provider_baidu_trims_whitespace_in_credentials`

AAA 结构完整，断言行为化：
- `body.contains("appid=12345")`：断言 trim 后值进了 body，直接命中根因
- `!body.contains("%2012345") && !body.contains("12345%20")`：反向断言空格未被 URL 编码带入

`secret_key` trim 效果不在 body 明文可见（只进 MD5 签名），此测试通过「构造成功 + app_id 无空白」间接覆盖了两字段均被 trim 的事实，测试设计合理。

### 测试 2：`build_provider_baidu_whitespace_only_app_id_returns_err`

AAA 结构完整，断言 `result.is_err()`，对应「trim 后空 = 缺失路径」的精确验证。

两测试均非弱断言（is_ok 兜底型），通过规范要求。

---

## 无高置信度问题

置信度达到 80 以上的问题：无。

以下为低置信度观察（不作必改要求）：

- **[置信度 60] TDD RED 注释保留**：`// TDD RED: ...` 注释（L705、L732）描述的是红阶段行为，已绿后注释仍留存，从语义上已不准确。不影响功能，属可选清理。

---

## 审查结论

**通过。** trim 改动正确覆盖全部 provider 全部字段；trim 后空字符串通过 filter → None → ok_or_else? 链路确实走到 Err，不构造无效 provider；签名根因（空白进 MD5 输入）已消除；安全规范合规；函数/嵌套指标均达标；两条新测试为行为化非弱断言，覆盖核心正向和边界路径。
