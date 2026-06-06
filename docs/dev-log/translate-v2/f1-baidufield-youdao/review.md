---
id: TV2-F1-S01-review
type: review
level: 小功能
parent: TV2-F1
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F1-A01, TV2-F5-A01]
author: code-reviewer
---

# TV2-F1 审查报告：百度专业（baidu_field）+ 有道（youdao）

## 审查范围

- `src-tauri/src/translate/providers.rs`（新增 `BaiduFieldProvider`、`YoudaoProvider` 及相关纯函数、build_provider/registry 追加）
- `src-tauri/src/translate/credential.rs`（baidu_field/youdao schema）
- `src-tauri/src/translate/lang.rs`（map_for_baidu_field/map_for_youdao）
- `src-tauri/Cargo.toml`（sha2 提为直接依赖）
- 照项目规范（设计文档 §〇/§二.2.2/§三.决策4）+ code-standards + 验收 TV2-F1-A01 / TV2-F5-A01 / TV2-A-SEC

---

## 审查维度

### 1. 签名正确性

**baidu_field_sign**

- 串拼顺序：`appid + q + salt + field + secret_key`，与百度翻译开放平台「领域翻译」API 文档一致（比基础百度多拼入 `field`）。
- 黄金值手算交叉验证（Python hashlib）：
  - 输入 `appid123hello12345itsecret` → MD5 = `0ddfb12f98655a716cc509c2538a4386` ✓（与测试断言一致）

**youdao_sign / youdao_truncate**

- 串拼顺序：`appKey + truncate(q) + salt + curtime + appSecret`，与有道智云 API 文档 §计算签名一致。
- truncate 规则：`chars().collect()` 按字符计数；len ≤ 20 返回全文；否则 `chars[..10] + len.to_string() + chars[len-10..]`，中文长文本不 panic。
- 边界断言手算交叉验证：
  - 恰 20 字符返回全文 ✓
  - 21 字符：前10=`1234567890`、后10=`2345678901`，结果 `1234567890212345678901` ✓（与测试断言一致）
- SHA256 黄金值手算（Python hashlib）：
  - 短文本 `app123hellosaltX1700000000sec456` → `de9f455414aeb5c0057ad78813f9be70ff0ef07f9ea70cf53ee90169860871a2` ✓
  - 长文本 truncate(`this is a 42enty chars`) → `b61bfa28f19cdff1f69386cb076d25c13902bc855a97e91364dbad6fe76c3c3b` ✓

**curtime 隔离**：`current_unix_secs()` 独立函数，`youdao_sign` 接收 `curtime` 形参保持纯函数可测，`SystemTime::now()` 失败时回退 `0` 而非 panic。

### 2. 签名安全

- providers.rs 全文 grep `eprintln|println|log::|tracing::|warn!|error!|info!|debug!` → **0 匹配**，密钥不入任何日志路径。
- `build_provider` 错误消息不含凭据值（测试 `build_provider_baidu_field_missing_required_fields_returns_err` 断言 secret 不出现在 err 消息中）。
- salt 由 `uuid::Uuid::new_v4()` CSPRNG 生成，非硬编码。
- 签名中间变量 `input`（含 secret）为局部 `String`，函数返回后自动 drop，无残留泄漏路径。

### 3. 凭据 schema

- `baidu_field`：`app_id`（is_secret=false, required=true）、`secret_key`（is_secret=true, required=true）、`field`（is_secret=false, required=true）— 3 字段，标记正确。
- `youdao`：`app_key`（is_secret=false, required=true）、`app_secret`（is_secret=true, required=true）— 2 字段，标记正确。
- `capability().needs_key=true`、`is_unofficial=false`，与官方 API 身份一致。

### 4. 许可合规（GPL 红线）

- 代码注释标注端点来源为「百度翻译开放平台「领域翻译」API 文档」和「有道智云「自然语言翻译服务」API 文档」（官方文档而非 pot 源码 URL），与设计文档 §〇 溯源锚要求方向一致。
- 整个 providers.rs 既有源（Lingva/Google/Yandex/Transmart/Bing 等）注释风格均为端点描述而无文档页超链接，新增源延续既有风格，属全文件一致性问题，不构成本次增量的新引入缺陷。
- 签名算法为功能性事实（MD5/SHA256 + 官方参数串），不涉及 pot 代码表达。

### 5. 既有源不回归

- `baidu_sign` 仍保持 4 参（`appid/q/salt/secret_key`），`baidu_field_sign` 另开 5 参函数，互不影响。
- `map_for_baidu_field` 独立函数，未改 `map_for_baidu`；`map_for_youdao` 亦独立。
- `registry()` 追加在末尾，不改已有源顺序。

### 6. 工程质量

- 无 `unwrap()`/`expect()` 在生产路径：parse 走 `map_err`；`concat_baidu_dst` 走 `ok_or_else`；`current_unix_secs` 走 `unwrap_or(0)`。
- 函数行数：`build_request`（BaiduField ≈20 行）、`parse_response`（≈12 行）、`youdao_truncate`（≈8 行）均在 50 行内；嵌套深度 ≤ 3。
- 无装饰性分隔注释（grep 全空）。
- clippy `-D warnings` exit 0（tester 报告确认）。
- 无遗留 TODO/FIXME。

### 7. 测试充分性（静态视角）

- 签名确定性：`baidu_field_sign_and_build`、`youdao_sign_v3_and_build` 各含固定输入→确定哈希断言，算法可单测证伪。
- truncate 边界：恰 20（全文）、21（截断）均有断言。
- parse 成功/多段/error_code/非法 JSON 分支：`baidu_field_parse`、`youdao_parse` 各 4 路径覆盖。
- build_provider 缺字段/全字段/secret 不泄露：3 个独立测试。
- registry 含新源/needs_key/is_unofficial：2 个独立测试。
- 凭据 schema is_secret 标记：`credential_schema_for_v2_keyed_sources` 覆盖全字段。

---

## 发现问题汇总

审查未发现置信度 ≥ 80 的 Critical 或 Important 问题。

以下是置信度 < 80 的观察（仅供参考，不阻塞）：

**观察 A（置信度 45）**：设计文档 §〇 要求「注释里标注官方 API 文档 URL」，当前注释给出的是端点 URL（`POST https://fanyi-api.baidu.com/api/trans/vip/fieldtranslate`）而非文档页链接（如 `https://fanyi-api.baidu.com/product/113`）。但既有所有源（Lingva/Google/Bing 等）注释风格均如此，这是全文件既有风格一致性问题，非本次新增缺陷，不构成阻塞。

**观察 B（置信度 40）**：有道 `parse_response` 的 `errorCode` 字段仅以 `as_str()` 处理字符串形式，未覆盖数字形式。有道官方文档中 `errorCode` 定义为字符串（如 `"0"`, `"101"`），实测返回均为字符串，此边界属理论而非实测问题，置信度低。

---

## 结论

**两签名算法正确性经手算交叉验证（Python hashlib 三方核对）确认；安全路径（密钥不入日志/错误消息）、凭据 schema、既有源不回归、工程规范均满足；tester 动态证伪（8/8 命中 + 变异 A-E 全红 + 边界 + debug/release 三次连跑 + clippy 0）已通过门禁。无置信度 ≥ 80 问题。**

---

**APPROVE**
