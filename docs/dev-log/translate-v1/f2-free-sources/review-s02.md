---
id: TV1-F2-S02-review
type: review
level: 小功能
parent: TV1-F2
children: []
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F2-A01]
evidence: []
author: code-reviewer
---

# 审查结论 · TV1-F2-S02：新增 Yandex + Transmart 两免 key 翻译源

## 一、审查范围与依据

改动文件：
- `src-tauri/src/translate/providers.rs`：新增 `YandexProvider`、`TransmartProvider`（capability / build_request / parse_response）及 `map_yandex_error`；registry() / build_provider() 追加两源；新增 8 个单元测试。
- `src-tauri/src/translate/lang.rs`：新增 `map_for_yandex`、`map_for_transmart`，接入 `map_lang_for_provider`；新增 2 个映射测试。
- `src-tauri/tests/translate.rs`：免 key 集合补 `yandex`/`transmart`；注册表计数 5→7。

对照依据：
- 设计文档 `docs/design/translation-sources-pot.md`（§〇 许可红线、§二.2.1 Yandex/Transmart）
- 验收标准 TV1-F2-A01（四源 build_request/parse_response 正确）、TV1-A-SEC（安全）
- 规范：code-standards skill + code-general.md（函数≤50行/嵌套≤3层/注释写 why/安全红线）

## 二、审查维度逐项核查

### 2.1 许可合规（GPL-3.0 红线）

- `YandexProvider` 和 `TransmartProvider` 均为原创 Rust 实现，与 pot 的 JavaScript 实现（async fetch、import/export 模块结构）在代码表达上完全不同。
- providers.rs 注释明确标注：「按 Yandex translate v1 tr.json 公开互操作协议事实独立实现，不参考任何第三方源码」（第 231 行）；「按 transmart.qq.com/api/imt 公开互操作协议事实独立实现，不参考任何第三方源码」（第 337 行）。
- 两处注释均未引用 pot 的 URL 或代码路径；端点、参数、响应字段均属功能性事实 / 互操作信息，按设计文档§〇合规取用。
- `lingva.pot-app.com` 是服务域名（Lingva 实例的托管地址），不是 pot 源代码引用，合规。
- 结论：GPL-3.0 许可红线合规，未抄 pot 代码。

### 2.2 parse / build_request 正确性与错误处理

**YandexProvider**

- `build_request`（第 264-287 行，23 行）：url 拼接 `id={session_id}-0-0&srv=android` 与 coding.md 实测结论（id 去连字符 uuid + `-0-0`、`srv=android`）完全对应；body `text={percent_encode}&lang={src}-{tgt}` 符合 Yandex body 单参协议。
- `parse_response`（第 289-316 行，27 行）：先检查 `v["code"].as_u64()` 非 200 走 `map_yandex_error`，再取 `v["text"].as_array()`；非法 JSON、code!=200、text 缺失、text 全空四条错误路径均映射为 `TranslateError` 具体变体，无 `unwrap`/`expect`/`panic`（仅有 `unwrap_or("unknown")` 作兜底默认值，不触发 panic）。
- `map_yandex_error`（第 322-329 行，8 行）：401/402/403/405 → Auth，404/413 → TooLong，422/501 → Unsupported，429 → RateLimit，其余 → ServerError；覆盖已知 Yandex 错误码语义，`_` 兜底无遗漏。

**TransmartProvider**

- `build_request`（第 378-403 行，25 行）：用 `serde_json::json!` 构造 body（第 383 行），自动转义文本内的引号/反斜杠，无手拼 JSON 风险。`client_key` 由 `anonymous_client_key()` 生成随机值，经 `serde_json::json!` 序列化后 JSON 安全。
- `parse_response`（第 406-437 行，31 行）：先检查 `v["header"]["ret_code"]` 非 "succ" 走 ServerError，再取 `v["auto_translation"].as_array()`；非法 JSON、ret_code 错误、缺 auto_translation、全空串四条路径全覆盖，无 panic 路径。
- 当响应无 `header` 字段时，`v["header"]["ret_code"].as_str()` 返回 None，`if let Some` 跳过错误检查，继续尝试读 `auto_translation`——这是"宽容解析"策略，与真实 Transmart 协议（成功响应必有 header）一致，不属于逻辑错误。

函数行数/嵌套：YandexProvider build_request 23 行、parse_response 27 行；TransmartProvider build_request 25 行、parse_response 31 行；map_for_yandex 9 行、map_for_transmart 9 行。均符合≤50 行、嵌套≤3 层要求。

### 2.3 JSON 构造安全（手拼 vs serde_json::json!）

- **YandexProvider**：body 为 URL-encoded 表单字符串（`text=...&lang=...`），text 经 `percent_encode` 处理，lang 对为 ASCII 语言码（如 `en-zh`），无注入面；未使用手拼 JSON。
- **TransmartProvider**：body 完全由 `serde_json::json!` 构造（第 383-393 行），验收了「无手拼 JSON」要求。
- **uuid 随机源**：`uuid::Uuid::new_v4()` 使用 `getrandom` crate（Cargo.toml `features = ["v4"]`），底层调用 OS CSPRNG（Linux `/dev/urandom`、macOS `getentropy`、Windows `CryptGenRandom`）；`id` / `client_key` 属非安全客户端标识（非密钥/salt/nonce），设计文档明确定性，用随机 uuid 防去重即可，用法合规。

### 2.4 不越界：既有源实现未被改动

- git diff 确认 `LingvaProvider`、`GoogleFreeProvider`、`BaiduProvider`、`DeepLFreeProvider`、`GoogleProvider` 的 capability / build_request / parse_response 实现行均未被改动。
- `build_provider` 仅在 "yandex" / "transmart" 两个新臂添加分支，既有臂顺序和逻辑不变。
- `registry()` 在 google_free 之后追加两源，其余顺序不变；lingva 仍在首位（默认源顺序符合设计文档）。

### 2.5 tests/translate.rs 免 key 集合判别力

- `static_registry_lists_seven_providers`：断言 `providers.len() == 7`（精确等于），若误删或忘记注册新源即失败，非恒真。
- `static_registry_keyed_providers_need_key`：免 key 集合 `["lingva","google_free","yandex","transmart"]` 对全部 provider 双向约束——集合内断言 needs_key=false，集合外断言 needs_key=true；若 F3 新增免 key 源忘记补集合，集合外断言将立即失败（needs_key=false ≠ true）。两条测试判别力充分，非被改弱成恒真。

### 2.6 安全（TV1-A-SEC）

- providers.rs 全文无 eprintln / println / log:: / tracing:: 输出（grep 确认 0 命中），不打印待译文本或译文。
- `build_provider("yandex", &[])` 和 `build_provider("transmart", &[])` 均直接 `Ok(Box::new(XxxProvider::new()))` 返回，credentials 切片完全未读，不访问凭据存储。
- `needs_key: false` 标注正确，UI 层据此不触发凭据输入流程。
- `anonymous_client_key` / `session_id` 注释明确说明属非安全客户端标识，不作密钥处理，无需 zeroize，逻辑自洽。
- 结论：TV1-A-SEC 满足。

### 2.7 代码规范（注释 / 清洁度）

- 注释均写"为什么"：「实测必须用 srv=android 且 id 为去连字符 uuid 加 -0-0 后缀才返回 200」说明 Yandex 特殊参数原因；「用 serde_json::json! 构造 body，自动正确转义文本（避免手拼 JSON 注入风险）」说明 Transmart 构造方式选择原因；`map_for_yandex` 注释说明 Yandex zh 不分简繁的来源。
- grep 确认：无装饰性横线分隔注释（`═══`/`───`/`━━━`/`=====`）、无 TODO/FIXME、无死注释。
- 模块 doc 注释第 1 行「编译期静态注册表与 4 家 provider 的完整实现」描述已随 S01 过时（现为 7 家）。此为存量遗留问题（S01 审查时已存在），本次 S02 未新引入、未加剧，属 pre-existing，按审查规则不计入本次置信度评分。

## 三、高置信度问题清单

经逐项核查，未发现置信度 ≥80 的问题。

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| — | 无 | — | — |

## 四、审查结论

TV1-F2-S02 改动符合项目规范与 code-standards 要求：

- **许可合规**：两源实现原创，注释标注公开协议来源（非 pot 源码 URL），GPL-3.0 红线未触碰。
- **正确性**：parse / build_request 端点、参数、译文字段路径与 curl 实测 ground truth 一致；错误路径全覆盖，无 unwrap panic。
- **JSON 安全**：Transmart 用 `serde_json::json!` 构造 body 无手拼风险；Yandex body 为 URL-encoded 表单；uuid 随机源为 OS CSPRNG。
- **不越界**：既有 5 源实现行为无变化。
- **测试判别力**：注册表计数精确断言（7），免 key 集合双向约束，非恒真。
- **安全 TV1-A-SEC**：无凭据读取，无敏感信息打印，needs_key=false 标注正确。

**APPROVE**
