---
id: TV2-F3-S01-review
type: review
level: 小功能
parent: TV2-F3
children: []
created: 2026-06-06T03:30:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F3-A01, TV2-F5-A01]
evidence: []
author: code-reviewer
---

# 审查结论 · TV2-F3 腾讯 TC3 + 阿里 HMAC（keyed 翻译源）

## 审查维度

按项目规范（code-standards skill + 〇章不抄 pot 约定）逐项核对：
格式/命名/函数长度/嵌套深度/注释/类型/性能/测试/安全/签名正确性/许可合规。

**签名独立验证结果**（Python 独立复算，与 Rust 测试向量交叉核对）：
- **腾讯 TC3**：CanonicalRequest 格式（`POST\n/\n\n{headers}\n{signedHeaders}\n{hash}`）、CanonicalHeaders 中 header name 与 value 均小写（符合腾讯官方 API 鉴权文档"头部 key 和 value 统一转成小写"规范）、三层密钥派生（`TC3+secret_key → date → service → tc3_request`）、Authorization 头格式——全部与官方文档一致，Python 重算签名 `cc913306...decaf` 完全匹配测试向量。
- **阿里 HMAC-SHA1**：参数字典序排序、RFC3986 编码（unreserved 字符集 `A-Za-z0-9-_. ~` 与官方规范一致）、StringToSign 构造（`METHOD&encode(/)&encode(canonical)`）、HMAC-SHA1 key 为 `secret+"&"`、Base64 编码——全部正确，Python 重算 `+uwyBbn3LNXWPJOuNcXCiWB/32k=` 完全匹配。
- **unix_secs_to_utc_parts**（Howard Hinnant civil_from_days 算法）：对 1970-01-01、2023-11-14、2024-02-29（闰年）、2024-04-01 等边界全部正确，与 Python datetime 交叉核对无误差。

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| — | 无置信度 ≥ 80 问题 | — | — |

**不报（置信度 < 80）的观察项说明**：

1. `map_tencent_error` 第 1339 行存在 `code == "RequestLimitExceeded" || code.starts_with("RequestLimitExceeded")` 的冗余——第一个条件完全被第二个包含，但不影响正确性，纯属写法冗余；置信度 30（不阻塞）。

2. 阿里云 `build_request` 使用 `http://` 而非 `https://`（第 1427 行）——`accesskey_id` 和 `Signature` 明文走 HTTP，有中间人风险。但该端点与协议在设计文档 `docs/design/translation-sources-pot.md` 表格和 `coding.md` 中已明确记载 `GET http://mt.cn-hangzhou.aliyuncs.com/`，属冻结设计决策；审查范围内不计为此次引入缺陷，置信度 40（已冻结设计，不打回）。

3. `serde_json::json!` 在无 `preserve_order` feature 时序列化为 BTreeMap 字母序（非声明序）——腾讯 `build_body` 实际产出字段顺序与 `tencent_tc3_signature_deterministic` 测试中手写 payload 字段顺序不同，但两者解耦设计正确：确定性签名测试验证纯函数；`tencent_build_and_parse` 测试通过提取 `X-TC-Timestamp` 重算签名进行自洽比对，覆盖了 build 流程内部一致性；置信度 20（非缺陷，设计已自洽）。

## 是否合规

**完全合规**。逐项检查结果：

- **函数长度 ≤ 50 行**：`tencent_tc3_sign` 39 行、`alibaba_hmac_sign` 27 行、`AlibabaProvider::build_request` 42 行、`TencentProvider::build_request` 20 行，均在限制内。
- **嵌套 ≤ 3 层**：所有新函数无超过 3 层嵌套，全程 early return 风格。
- **命名**：函数名动词+名词（`tencent_tc3_sign`、`alibaba_hmac_sign`、`unix_secs_to_utc_parts`），常量 `UPPER_SNAKE`（`TENCENT_HOST`、`ALIBABA_ACTION` 等），完全符合 Rust 惯用。
- **注释**：所有公共函数/常量附文档注释，内容解释"为什么"（算法来源、设计决策）；无死代码注释；无装饰性分隔符。
- **签名安全**：`secret_key`/`access_key_secret` 不入 `eprintln`/`println`/`dbg!`/`log::` 零匹配；HMAC 中间密钥（`secret_date`/`secret_service`/`secret_signing`）均为局部 `Vec<u8>`，函数返回后随作用域释放；`build_provider` 缺字段错误消息仅含字段名，不含字段值。
- **凭据 schema**：tencent `secret_id`（非密）+`secret_key`（密）、alibaba `accesskey_id`（非密）+`accesskey_secret`（密），`needs_key=true`、`is_unofficial=false`，完全符合 TV2-F5-A01 要求。
- **测试质量**：所有测试 AAA 结构清晰、行为化命名；签名确定性测试锚定具体 hex/Base64 值（非弱断言）；`build_and_parse` 测试覆盖 success/auth_error/rate_limit/parse_error 四路径；`missing_field_returns_err` 坐实错误消息不含脏密钥值。
- **许可合规**：所有新函数注释标明来源为腾讯云/阿里云官方文档（非 pot GPL-3.0 源码）；未发现任何与 pot 代码相关的实现雷同，属独立实现。
- **既有源未改**：仅追加新 provider，`registry`/`build_provider` 的既有源分支未改动，符合"不越界"约束。
- **新增依赖合理**：`hmac = "0.12"`（通用 HMAC）、`sha1 = "0.10"`（阿里 HMAC-SHA1），版本与 `sha2 = "0.10"` 同 digest 生态一致，`cargo check` 通过。

## 结论

**通过**。无置信度 ≥ 80 的 Critical 或 Important 问题。TC3 三层密钥派生、阿里 HMAC-SHA1+Base64、unix_secs_to_utc_parts 公历换算算法均经 Python 独立交叉验证正确，测试向量确定性锚定，安全约定（密钥不入日志/错误消息）严格遵守，代码规范全面符合。

---

**APPROVE**
