---
id: TV2-F4-S01-review
type: review
level: 小功能
parent: TV2-F4
children: []
created: 2026-06-06T04:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV2-F4-A01, TV2-F5-A01]
evidence: []
author: code-reviewer
---

# 审查结论 · TV2-F4 火山引擎翻译（volcengine · AWS SigV4 风格四层 HMAC-SHA256）

## 审查维度

按项目规范（code-standards skill + 设计文档〇章不抄 pot 约定）逐项核对：
SigV4 签名正确性（四层 HMAC-SHA256 派生 + CanonicalRequest 格式 + Authorization 组装）、
时间换算正确性（unix_secs_to_compact_utc/date 与既有 unix_secs_to_utc_parts 一致性）、
签名安全（密钥与中间派生密钥不入日志/错误消息）、凭据 schema 正确性（is_secret 路由、
region 选填回退）、许可合规（官方文档来源、未复制 pot）、代码规范（函数长度/嵌套/命名/注释）、
既有源不越界。

**SigV4 签名 Python 独立交叉验证结果**（按火山引擎官方文档 volcengine.com/docs/6369/67269 手算）：

- **CanonicalRequest 格式**：`POST\n/\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}`，其中 `canonical_headers` 末尾含尾 `\n`，加上 format 中再一个 `\n`，形成双重换行（`\n\n`）分隔 signed_headers。此格式与 F3 腾讯 TC3 实现采用相同约定（L1323 `"POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}"`），是已在 F3 经真实 API 间接验证的一致约定，不是孤立选择。
- **StringToSign**：`HMAC-SHA256\n{x_date}\n{credential_scope}\n{hashed_canonical}`，算法标识符 `HMAC-SHA256`（非 AWS 的 `AWS4-HMAC-SHA256`），符合火山特有规范。
- **四层密钥派生**：`HMAC(HMAC(HMAC(HMAC(secretAccessKey, date), region), service), "request")`，首层直接用 `secretAccessKey`（无 "AWS4" 前缀，火山特有）；`credential_scope` 以 `/request` 结尾（非 AWS `/aws4_request`）；全部符合文档。
- **Authorization 格式**：`HMAC-SHA256 Credential={akid}/{scope}, SignedHeaders={headers}, Signature={hex}`，格式正确。
- **Python 独立复算**（固定 access_key_id=`AKLTtest_access_key_id_123`、secret=`test_secret_access_key_abc`、region=`cn-north-1`、payload 含 `hello`、timestamp=1700000000）得到签名 `dac06f9e1be8667102fc1dfe025834cc9da68f2359b28084891b8e03ee332a61`，与 Rust 测试向量（L3780）逐字一致。
- **unix_secs_to_compact_utc/date**：直接复用既有 `unix_secs_to_utc_parts`（Howard Hinnant civil_from_days 算法，F3 已验证对闰年/跨年边界精确）。timestamp=1700000000 → UTC `20231114T221320Z` / `20231114`，Python `datetime.utcfromtimestamp` 独立确认一致。

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| — | 无置信度 ≥ 80 问题 | — | — |

**不报（置信度 < 80）的观察项说明**：

1. **`build_request` 中 `payload_hash` 计算冗余**（L1589 和签名函数内 L1667 各算一次 SHA256，结果必然相同）：置信度 0（非 bug，两次输入完全相同，结果一致；冗余计算对签名前/传 HTTP 的单次调用性能影响可忽略）。

2. **CanonicalHeaders 双重换行格式**（见上方分析）：置信度 30（理论上与 AWS SigV4 标准规范写法不同，但与 F3 腾讯 TC3 既有实现完全一致，且测试向量经独立 Python 验证正确；如火山服务器端实现了标准单换行语义，真实 API 调用会返回 SignatureDoesNotMatch，属可在集成/手工验证阶段发现的可能问题，但无静态证据证明实际不兼容，且 acceptance 已将真实 API 核验列为 manual_confirm）。不阻塞。

3. **`map_volcengine_error` 中 `code.contains("Signature")` 可能匹配到 SignatureDoesNotMatch 等非精确枚举**（L1706）：置信度 20（火山错误码关键词匹配方式与 F3 map_tencent_error 约定一致；官方文档 volcengine.com/docs/6369/67270 中鉴权错误码均含 "Signature"/"Auth"/"AccessKey"，设计选择合理）。

## 是否合规

**完全合规**。逐项检查结果：

- **函数长度 ≤ 50 行**：`volcengine_sigv4_sign` L1658–L1699 = 41 行，`map_volcengine_error` L1705–L1719 = 14 行，`sha256_digest` L1738–1741 = 3 行，`VolcengineProvider::build_request` L1585–1611 = 26 行，`VolcengineProvider::build_body` L1559–1572 = 13 行，`VolcengineProvider::parse_response` L1614–1641 = 27 行，均 ≤ 50 行。
- **嵌套 ≤ 3 层**：所有新函数无超过 2 层嵌套，full early-return 风格。
- **命名**：函数名动词+名词（`volcengine_sigv4_sign`、`map_volcengine_error`、`unix_secs_to_compact_utc`、`unix_secs_to_compact_date`、`sha256_digest`），常量 `UPPER_SNAKE`（`VOLCENGINE_HOST`、`VOLCENGINE_ACTION`、`VOLCENGINE_DEFAULT_REGION` 等），struct `VolcengineProvider`，完全符合 Rust 惯用命名。
- **注释**：所有公开函数/常量附文档注释；注释内容解释"为什么"（算法来源、火山与 AWS 差异、选填字段回退逻辑）；无装饰性分隔符（grep 0 匹配）；无死代码注释；无 TODO/FIXME（全仓库 grep 0 匹配）。
- **签名安全**（TV2-A-SEC）：`secret_access_key` 与四层中间密钥（`k_date`/`k_region`/`k_service`/`k_signing`）均为纯局部 `Vec<u8>`，无 `eprintln`/`println`/`dbg!`/`log::` 打印（tester grep 0 匹配）；`build_provider` 缺字段错误消息只含字段名（如"volcengine 未配置 SecretAccessKey"），不含字段值；`build_provider_volcengine_missing_secret_returns_err` 测试坐实脏值 `sak_secret_must_not_leak` 不出现在错误消息中。
- **凭据 schema**（TV2-F5-A01）：`access_key_id`（非密·必填）、`secret_access_key`（密·必填）、`region`（非密·选填），与腾讯 `secret_id`/`secret_key` 同一约定；`needs_key=true`、`is_unofficial=false`；region 选填有默认回退 `cn-north-1`；schema 完全符合要求。
- **测试质量**：AAA 结构清晰，行为化命名；签名确定性测试锚定完整 64 位 hex（非弱断言）；`volcengine_build_and_parse` 覆盖成功/多段拼接/Auth错误/RateLimit错误/非法 JSON 五条路径；`missing_secret_returns_err` 覆盖缺字段安全约定；变异 sanity A-D 全红（tester 确认）。
- **DRY / 工具复用**：`unix_secs_to_compact_utc`/`unix_secs_to_compact_date` 均直接调用 F3 引入的 `unix_secs_to_utc_parts`；`hmac_sha256`/`to_hex_lower`/`sha256_digest` 均为既有共享工具，无重复实现。
- **许可合规**（设计〇章）：所有新函数注释标明来源为火山引擎官方文档 URL（非 pot GPL-3.0 源码）；未发现与 pot 代码的实现雷同；`volcengine_sigv4_sign` 是纯独立 Rust 实现。
- **既有源不越界**：`registry()`/`build_provider()` 仅追加火山分支，既有 14 源分支未改动；`static_registry_lists_fifteen_providers` 验证 registry 计数 14→15 恰好，无多余改动。
- **无新增 clippy 警告**：tester 报告 `cargo clippy --all-targets -- -D warnings` exit 0。

## 结论

**通过**。无置信度 ≥ 80 的 Critical 或 Important 问题。

SigV4 四层 HMAC-SHA256 派生、CanonicalRequest 构造、时间换算均经 Python 独立交叉验证与 Rust 测试向量完全一致；签名安全约定（密钥/中间派生密钥不入日志/错误消息）严格遵守；凭据 schema 路由正确；许可红线（独立实现、官方文档来源）已坚守；代码规范全面符合（函数≤50行/嵌套≤3/命名/注释/无 TODO）；既有 14 源零破坏。

---

**APPROVE**
