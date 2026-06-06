---
id: TV2-F3-S01-code
type: coding_record
level: 小功能
parent: TV2-F3
status: done
commit: 29bae0c
acceptance_ids: [TV2-F3-A01, TV2-F5-A01]
---

# TV2-F3 腾讯云 TMT + 阿里翻译（云厂商 HMAC 签名）编码记录

## 做了什么

新增两个需 key 官方翻译源，均按厂商**官方 API 文档公开规范独立实现**（绝未复制/打开 pot GPL-3.0 源码）：

1. **tencent**（腾讯云机器翻译 TMT，`needs_key=true`、`is_unofficial=false`）
   - 端点 `POST https://tmt.tencentcloudapi.com`，签名 **TC3-HMAC-SHA256**。
   - Action=`TextTranslate`、Version=`2018-03-21`、Region=`ap-guangzhou`。
   - parse 取 `Response.TargetText`；`Response.Error.Code` 归一为 `TranslateError`。
2. **alibaba**（阿里云机器翻译，`needs_key=true`、`is_unofficial=false`）
   - 端点 `GET http://mt.cn-hangzhou.aliyuncs.com/`，RPC 风格 **HMAC-SHA1 + Base64** 签名。
   - Action=`TranslateGeneral`、Version=`2018-10-12`。
   - parse 取 `Data.Translated`；`Code != "200"` 归一为 `TranslateError`。

配套接入：
- `providers.rs` registry 追加两源；`build_provider` 加 `tencent`/`alibaba` 分支（缺必填字段→明确中文错误，不含字段值）。
- `credential.rs` 加两源 schema（tencent：secret_id 非密/secret_key 密；alibaba：accesskey_id 非密/accesskey_secret 密）+ 扩充 `credential_schema_for_v2_keyed_sources` 断言。
- `lang.rs` 加 `map_for_tencent`（zh/zh-TW）、`map_for_alibaba`（zh/zh-tw 小写）+ 映射单测。
- `tests/translate.rs` 注册表计数 12→14（函数 twelve→fourteen 同步）。
- 既有源**未动**。

## 签名实现决策

- **签名抽纯函数 + 确定性单测**：
  - `tencent_tc3_sign(secret_id, secret_key, payload, timestamp) -> Authorization` 头串。
  - `alibaba_hmac_sign(method, params, access_key_secret) -> Base64 签名`。
  - 两者对**固定输入**断言确定输出（手算参照向量见下），并断言两次调用一致（纯函数确定性）。`build_request` 内用随机 nonce / 真实时间戳，测试中提取实际 timestamp 重算比对。
- **腾讯 TC3 三层密钥派生**：`HMAC(HMAC(HMAC(HMAC("TC3"+secret_key, date), "tmt"), "tc3_request"), stringToSign)`。CanonicalHeaders 固定 `content-type;host;x-tc-action`，且**实际发送头与签名头逐字一致**（content-type 含 `; charset=utf-8`）。body 用 `serde_json` 构造保证字段顺序稳定，签名对其求 SHA256 与实际发送一致。
- **阿里 RPC 签名**：参数按 key 字典序排序 → RFC3986 编码 → 拼规范化查询串 → `StringToSign = "GET&" + encode("/") + "&" + encode(canonical)` → `Base64(HMAC-SHA1(secret + "&", StringToSign))`。复用既有 `percent_encode`（RFC3986 unreserved 集），与阿里编码规范一致。
- **共用工具**：`hmac_sha256`/`hmac_sha1`（用 `hmac::SimpleHmac` + `new_from_slice`，HMAC 接受任意长度 key，不 panic）、`to_hex_lower`、`unix_secs_to_utc_parts`（自实现公历换算，避免引入 chrono；用 Howard Hinnant civil_from_days 算法处理闰年）派生 `date`/ISO8601 时间戳。

## 官方文档来源（注释已标，非 pot）

- 腾讯接口：cloud.tencent.com/document/api/551/15619（TextTranslate）
- 腾讯签名 v3：cloud.tencent.com/document/api/551/30637（TC3-HMAC-SHA256）
- 阿里接口：help.aliyun.com/document_detail/158244（机器翻译·通用版）
- 阿里 RPC 签名：help.aliyun.com/document_detail/30563（HMAC-SHA1 + Base64）

## 确定性测试向量来源

参照值由**独立 Python 实现**（按上述官方文档手写，非 pot）对固定 secret/timestamp/payload 计算：
- 腾讯：secret_id=`AKIDtest_secret_id_123`、secret_key=`test_secret_key_abc`、timestamp=`1700000000`（UTC 日期 2023-11-14）、payload=`{"SourceText":"hello",...}`，
  得 Authorization Signature=`cc913306276069356aef21567e4670d036b69e1fd30eb24e17d7c536ed7decaf`。
- 阿里：固定 13 参数集（含 SignatureNonce=`fixed-nonce-123`、Timestamp=`2023-11-14T00:00:00Z`）、secret=`test_access_secret`，
  得 Base64 签名=`+uwyBbn3LNXWPJOuNcXCiWB/32k=`。
两实现独立同源（同一官方算法分别用 Python / Rust 写），交叉核对一致。

## 新增依赖

- `hmac = "0.12"`（通用 HMAC 构造，TC3 三层派生 + 阿里 HMAC-SHA1 共用）。
- `sha1 = "0.10"`（阿里 HMAC-SHA1 摘要）。
- 复用既有 `sha2`（SHA256）、`base64`、`uuid`、`serde_json`。`cargo check` 依赖解析通过。

## 改动文件

- `src-tauri/Cargo.toml`：加 `hmac`/`sha1` 依赖（注释说明用途）。
- `src-tauri/src/translate/providers.rs`：新增 `TencentProvider`/`AlibabaProvider` + `tencent_tc3_sign`/`alibaba_hmac_sign` 纯函数 + 错误归一 + 共用签名工具；registry/build_provider 接入；新增 12 个单测。
- `src-tauri/src/translate/credential.rs`：tencent/alibaba schema + 扩充 schema 断言。
- `src-tauri/src/translate/lang.rs`：tencent/alibaba 语言码映射 + 单测。
- `src-tauri/tests/translate.rs`：注册表计数 12→14。

## 自测（红→绿）

- RED：先写测试，`tencent_tc3_sign`/`TencentProvider`/`alibaba_hmac_sign`/`AlibabaProvider` 未实现 → `cannot find function/type` 编译失败（功能缺失，非环境错）。
- GREEN：实现后目标测试全绿（原始证据见 `artifacts/tencent-alibaba-tests.log`）：
  - `tencent_tc3_signature_deterministic ... ok`
  - `tencent_build_and_parse ... ok`
  - `alibaba_hmac_signature_deterministic ... ok`
  - `alibaba_build_and_parse ... ok`
  - `credential_schema_for_v2_keyed_sources ... ok`、`static_registry_lists_fourteen_providers ... ok`
  - 两源 build/missing-field/registry/lang 映射等共 12 测 `test result: ok. 12 passed`。
- 全量 `cargo test` + `cargo test --release` 连跑 ≥3 次、`cargo clippy --all-targets -- -D warnings` exit 0：见 test.md / 下方收尾。

## 安全（TV2-A-SEC）

- secret_key/access_key_secret 不入 `eprintln`/`println`/`log`/`dbg!`/错误消息；签名中间值（三层派生密钥、StringToSign）不打印。
- build_provider 缺字段错误消息只含字段名提示、不含字段值（单测坐实脏密钥值不泄露）。
- HMAC key 为局部 `Vec<u8>`，函数返回后随作用域释放。
