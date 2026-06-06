---
id: TV2-F1-S01-code
type: coding_record
level: 小功能
parent: TV2-F1
status: done
commit: 29bae0c
acceptance_ids: [TV2-F1-A01, TV2-F5-A01]
---

# TV2-F1-S01 编码留痕：百度专业（baidu_field）+ 有道（youdao）需 key 翻译源

## 做了什么

新增两个需 key（官方 API）翻译源，严格 TDD（红→绿→重构）：

- **baidu_field**（百度专业 / 领域翻译，id `baidu_field`，needs_key=true，is_unofficial=false）
  - 端点 `POST https://fanyi-api.baidu.com/api/trans/vip/fieldtranslate`
  - 签名 `MD5(appid + q + salt + field + secret_key)`（比基础百度多拼入领域 field）
  - 请求 body：`q/from/to/appid/salt/domain(=field)/sign`，q/appid/field 走 percent-encode
  - parse：`trans_result[*].dst` 换行拼接；`error_code` → 复用基础百度错误码归一（`map_baidu_error`）
  - 凭据 schema：`app_id`（非密）、`secret_key`（密）、`field`（非密，领域如 it/finance）
- **youdao**（有道智云，id `youdao`，needs_key=true，is_unofficial=false）
  - 端点 `POST https://openapi.youdao.com/api`，signType=v3
  - 签名 `SHA256(appKey + truncate(q) + salt + curtime + appSecret)`
  - truncate：字符数 ≤ 20 用全文，否则「前 10 字符 + 字符长度 + 后 10 字符」（按 `chars()` 取，避免中文按字节切分 panic）
  - 请求 body：`q/from/to/appKey/salt/sign/signType=v3/curtime`
  - parse：`translation[*]` 换行拼接；`errorCode != "0"` → `map_youdao_error` 归一
  - 凭据 schema：`app_key`（非密）、`app_secret`（密）

接入（wiring）：
- `providers.rs::build_provider` 加 `baidu_field` / `youdao` 分支（缺必填字段→明确中文错误，不含字段值）
- `providers.rs::registry()` 追加两源 capability
- `credential.rs::credential_schema` 加两源 schema
- `lang.rs::map_lang_for_provider` 加两源语言码映射（新增 `map_for_baidu_field` / `map_for_youdao`）

## 签名实现决策

- **签名抽为 pub 纯函数**：`baidu_field_sign(appid,q,salt,field,secret)`、`youdao_sign(app_key,q,salt,curtime,app_secret)`、`youdao_truncate(q)`。
  纯函数使签名算法可对**固定输入**断言**确定哈希值**（手算/`md5`、`shasum -a 256`、Python hashlib 三方核对），不依赖运行时随机 salt / 真实时间。
- **时间可测性**：`current_unix_secs()` 隔离真实时间读取，`youdao_sign` 接收 `curtime` 形参保持纯；单测对固定 `salt+curtime` 断言确定 SHA256。
- **truncate 按字符不按字节**：`chars().collect()` 后切片，中文长文本不会 panic；边界单测覆盖恰 20（全文）、21（截断）。
- **不动既有源**：baidu_field 单独写 `map_for_baidu_field`（按官方文档繁中 `cht`、日语 `jp`），不改既有 `map_for_baidu`。

## 确定签名值（核对）

- `baidu_field_sign("appid123","hello","12345","it","secret")` = `0ddfb12f98655a716cc509c2538a4386`
- `youdao_sign("app123","hello","saltX","1700000000","sec456")` = `de9f455414aeb5c0057ad78813f9be70ff0ef07f9ea70cf53ee90169860871a2`
- 长文本 truncate(`this is a very long text over twenty chars`→`this is a 42enty chars`) 签名 = `b61bfa28f19cdff1f69386cb076d25c13902bc855a97e91364dbad6fe76c3c3b`

## 官方文档来源（非 pot）

- 百度专业：百度翻译开放平台「领域翻译 / fieldtranslate」API 文档（端点·签名·domain 参数·trans_result.dst）。
- 有道：有道智云「自然语言翻译服务」API 文档（端点·signType=v3 签名·truncate 规则·translation 取译文·errorCode 列表）。
- 许可红线：两源签名/协议按各厂商官方 API 文档公开规范独立实现，**未复制/未参考 pot（GPL-3.0）源代码**。

## 改动文件

- `src-tauri/Cargo.toml`：显式声明 `sha2 = "0.10"`（已在 lockfile，作 tauri 传递依赖，不新增下载）。
- `src-tauri/src/translate/providers.rs`：两 Provider 结构体 + 签名纯函数 + build_provider 分支 + registry。
- `src-tauri/src/translate/credential.rs`：两源 credential_schema。
- `src-tauri/src/translate/lang.rs`：两源语言码映射。
- `src-tauri/tests/providers.rs`：跨 crate 签名确定性集成测试。

## 自测（红→绿）

- RED：`cargo test --lib baidu_field` 编译失败（`BaiduFieldProvider`/`YoudaoProvider`/`baidu_field_sign`/`youdao_sign`/`youdao_truncate` 未实现）——功能未实现，非语法/环境错。证据 `artifacts/lib-red.log`。
- GREEN：`cargo test --lib -- baidu_field youdao` → 13 passed；`credential_schema_for_v2_keyed_sources` 单独 1 passed；集成 2 passed。证据 `artifacts/lib-green.log`。
- 全量 `cargo test` / `cargo test --release` 连跑 3× + `cargo clippy --all-targets -- -D warnings` 见 artifacts。

## 安全（TV2-A-SEC）

- 签名计算、build_request、build_provider 错误消息均**不打印 secret_key/app_secret**；providers.rs 无 eprintln/println/log/dbg 打印密钥。
- secret 字段（secret_key/app_secret）经 credential schema 标 is_secret=true，存 DbCredStore 加密库。
</content>
