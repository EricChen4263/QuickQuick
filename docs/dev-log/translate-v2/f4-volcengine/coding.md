---
id: TV2-F4-S01-code
type: coding_record
level: 小功能
parent: TV2-F4
status: done
commit: PENDING
acceptance_ids: [TV2-F4-A01, TV2-F5-A01]
---

# TV2-F4 火山引擎翻译（volcengine · AWS SigV4 风格四层 HMAC-SHA256）编码留痕

## 做了什么

新增需 key 翻译源 **volcengine（火山引擎翻译）**，id `volcengine`、`needs_key=true`、`is_unofficial=false`（官方 API）。
按火山引擎官方签名方法 V4 文档独立实现 AWS SigV4 风格四层 HMAC-SHA256 签名，**未复制 / 未参考 pot 源码**，
**未改动任何既有源**。

改动落点：

- `src/translate/providers.rs`
  - 新增 `VolcengineProvider`（端点 `POST https://open.volcengineapi.com/?Action=TranslateText&Version=2020-06-01`，
    body `{SourceLanguage,TargetLanguage,TextList}`，parse `TranslationList[].Translation` 拼接，
    `ResponseMetadata.Error` → `TranslateError`）。
  - 新增纯函数 `volcengine_sigv4_sign(access_key_id, secret_access_key, region, payload, timestamp) -> authorization`。
  - 新增 `map_volcengine_error`（签名/鉴权→Auth、限流→RateLimit、配额→Quota）。
  - 新增时间格式化工具 `unix_secs_to_compact_utc`（X-Date `YYYYMMDDThhmmssZ`）、
    `unix_secs_to_compact_date`（短日期 `YYYYMMDD`）、`sha256_digest`（payloadHash/CanonicalRequest 哈希复用）。
  - `registry()` 追加 volcengine；`build_provider()` 加 volcengine 分支（缺必填→明确错误不含字段值；
    region 选填回退默认 `cn-north-1`）。
- `src/translate/credential.rs`
  - `credential_schema("volcengine")`：access_key_id（非密·必填）、secret_access_key（密·必填）、
    region（非密·选填，默认 cn-north-1）。
  - 测试 `credential_schema_for_v2_keyed_sources` 扩充覆盖 volcengine 三字段。
- `src/translate/lang.rs`
  - `map_lang_for_provider` 加 volcengine 分支；新增 `map_for_volcengine`（zh→zh、zh-TW→zh-Hant、en→en）。
- `tests/translate.rs`
  - 注册表计数 14→15，测试名 `static_registry_lists_fourteen_providers`→`...fifteen_providers`。

## SigV4 实现决策

火山引擎签名是 AWS SigV4 风格，但与 AWS 有几处关键差异（火山特有，已在代码注释标明）：

1. **算法标识** 为 `HMAC-SHA256`（非 AWS 的 `AWS4-HMAC-SHA256`）。
2. **credentialScope 以 `request` 结尾**（非 AWS 的 `aws4_request`）。
3. **签名密钥首层直接用 `secret_access_key`** 派生：`HMAC(HMAC(HMAC(HMAC(SAK, date), region), service), "request")`，
   **不加 "AWS4" 前缀**（AWS 首层 key 为 `"AWS4"+secret`）。
4. **X-Date** 为紧凑 `YYYYMMDDThhmmssZ`、credentialScope 短日期 `YYYYMMDD`。
5. CanonicalHeaders 固定签名头集（按字母序）`content-type;host;x-content-sha256;x-date`，与实际发送头逐字一致。

签名抽为纯函数，对固定 access_key/secret/region/payload/timestamp 断言确定 Authorization，
锚定**独立 Python 按官方文档手算的具体签名 hex**（非弱断言）：
`Signature=dac06f9e1be8667102fc1dfe025834cc9da68f2359b28084891b8e03ee332a61`。

## 官方文档 + 测试向量来源

- 接口：volcengine.com/docs/4640/65067（机器翻译 TranslateText）
- 签名 V4：volcengine.com/docs/6369/67269（签名方法 V4）
- 公共错误码：volcengine.com/docs/6369/67270
- 测试参照向量由 `/tmp/volc_sigv4.py` 独立实现（按官方文档逐步骤手算，**非 pot 源码**）产出，
  固定输入 timestamp=1700000000（UTC X-Date 20231114T221320Z）下与 Rust 实现逐字一致。

## 复用 F3 工具

复用 providers.rs 既有 `hmac_sha256` / `to_hex_lower` / `unix_secs_to_utc_parts`（F3 TencentProvider TC3 派生时引入）。
SigV4 与 TC3 同属「四层 HMAC 派生 + SHA256」，工具共用，无重复造轮子。
新增的 `sha256_digest` 是把 Tencent 内联的 `Sha256::digest` 抽成共享小工具供火山复用。

## 自测（红→绿）

- RED：先写 `volcengine_sigv4_signature_deterministic` / `volcengine_build_and_parse` 等测试，
  `cargo test --lib volcengine` 报 `cannot find function volcengine_sigv4_sign` / `cannot find type VolcengineProvider`（功能缺失）。
- GREEN：实现后 `rtk proxy cargo test --lib volcengine` → 5 passed；
  `credential_schema_for_v2_keyed_sources` → 1 passed；`static_registry_lists_fifteen_providers` → 1 passed。
- 全量 `cargo test` / `cargo test --release` 全绿、连跑 3×；`cargo clippy --all-targets -- -D warnings` exit 0。
  原始证据见 artifacts/。

## 安全（TV2-A-SEC）

secret_access_key 与四层派生中间密钥（k_date/k_region/k_service/k_signing）均不入 eprintln/println/log/dbg!/错误消息；
缺字段错误消息只含字段名不含值（`build_provider_volcengine_missing_secret_returns_err` 断言脏值不泄露）。
凭据走 DbCredStore（secret 入加密库、非密入 provider_config），沿用既有路由。
