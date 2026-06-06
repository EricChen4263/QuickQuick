---
id: TV4-F2-S01-review
type: review_report
level: 小功能
parent: TV4-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F2-A01]
author: code-reviewer
---

# TV4-F2-S01 代码审查报告：ECDICT + 有道词典模式（JSON 词条→Dict）

## 审查结论

**APPROVE** — 无置信度 ≥80 的 Critical 或 Important 问题。改动质量良好，可放行合并。

---

## 审查范围

| 文件 | 改动类型 |
|---|---|
| `src-tauri/src/translate/providers.rs` | 新增 EcdictProvider / YoudaoDictProvider + 辅助纯函数 + 9 个单测 |
| `src-tauri/src/translate/credential.rs` | youdao_dict 复用 youdao schema；ecdict 免 key 落默认分支 |
| `src-tauri/tests/translate.rs` | 注册表数量 19→21；keyless_ids 补 ecdict |

---

## 发现项

### Critical（阻塞）

无。

### Important（非阻塞建议）

**I-1：`build_provider` doc comment 未同步新增的 ecdict / youdao_dict**

- 文件：`src-tauri/src/translate/providers.rs` 行 14–24
- 说明：函数顶部 doc comment 列举各 provider 的凭据字段，本次新增的 `ecdict`（免 key）和 `youdao_dict`（`app_key`/`app_secret`）均未补入。bing 也存在同样遗漏（预存问题，非本次引入）。
- 影响：仅文档不同步，功能无影响，维护者参照注释时会遗漏。
- 建议修复：在 doc comment 中补入以下条目：
  - `ecdict`：无凭据（免 key）
  - `youdao_dict`：`app_key`（必填）、`app_secret`（必填，同 youdao）
- 置信度：82（文档与实现明确不同步，但不影响运行时行为）

---

## 解析正确性

### ECDICT（providers.rs 行 1288–1321）

- **音标**：取 `v["phonetic"]`，空串过滤为 `None` — 正确。
- **translation 词性分组**：按 `\n` 分行 → `group_definitions_by_pos` → `split_pos_prefix` + `is_pos_token` 识别 `n.`/`vt.`/`adj.` 等前缀 — 逻辑正确，中文释义（如"冰川，冰河"）中词性前缀与内容之间有空格，`split_once(char::is_whitespace)` 可正确切分。
- **exchange 词形**：`parse_ecdict_exchange` 按 `/` 分割、取 `:` 后值；无冒号项原样保留；空串输出空列表 — 正确。
- **未收录空词条**：`translation` 为空串时报 `ParseError` 不 panic — 正确；`null` 值同样经 `unwrap_or("")` 安全处理。
- **tester 变异 A/D 全红**：音标取错字段 / exchange 取冒号前值均被测试捕获，断言判别力充分。

### 有道词典（providers.rs 行 1120–1148，1158–1193）

- **isWord 分流**：`v["isWord"].as_bool().unwrap_or(false)` 处理 JSON boolean，有道 API 文档确认该字段为 boolean 类型，无歧义。
- **basic 存在性判别**：`v["basic"].is_object()` 对 null/bool/number/array 均返回 false，回退 Plain — 健壮；tester 变异 C 路径分析验证了真实判别力来自 basic 字段存在性（C2 覆盖）。
- **音标优先级**：`us-phonetic → phonetic → uk-phonetic`，`find_map` 保证优先级顺序 — 正确。
- **explains 词性分组**：复用 `group_definitions_by_pos` — DRY 正确。
- **wfs 词形**：`wf.name: value` 格式（如"复数: glaciers"）；`wf.value` 缺失时跳过；ECDICT 存纯值、有道存"名: 值"，格式有差异但均在 `inflections: Vec<String>` 语义范围内，前端按 String 渲染无 schema 冲突。
- **签名复用**：`youdao_sign` 入参与 `YoudaoProvider` 完全一致，不另起算法 — 正确。
- **tester 变异 B/C2 全红**：isWord 反转 / 回退输出值改坏均被捕获，判别力充分。

---

## 设计符合性（设计文档 §二.2.4）

| 要求 | 实现 | 结论 |
|---|---|---|
| ECDICT `pot-app.com/api/dict` POST | `build_request` URL 固定为该端点 | 符合 |
| ECDICT 英汉词条 → Dict | `parse_response` 返回 `TranslateResponse::Dict` | 符合 |
| 有道词典复用有道 key | credential.rs `"youdao" | "youdao_dict"` 共用 schema | 符合 |
| isWord===true 且含 basic → Dict | `is_word && v["basic"].is_object()` | 符合 |
| 否则回退 Plain（translation 拼接） | `translation[*]` join | 符合 |
| registry 19→21 | 注册表精确 21 项，测试断言同步更新 | 符合 |
| ecdict needs_key=false | capability 声明 + keyless_ids 补入 | 符合 |

---

## 安全审查

- `app_secret` 字段 `is_secret: true`，经 SQLCipher 整库加密存储 — 正确。
- `providers.rs` 全文无 `eprintln`/`println!`/`dbg!`/`log::` 调用（grep 零匹配），密钥不出现在日志。
- sentinel 测试 `youdao_dict_build_request_does_not_leak_secret` 使用非空脏值 `SENTINEL_DEADBEEF` 断言 `!body.contains`，符合 hints TV2-RETRO-1 要求（不用空值占位）。
- 错误消息不含凭据值（`ok_or_else` 错误文案均为固定中文描述）。

---

## 未抄 pot 代码确认

所有含"pot"的引用均为说明性注释（"非 pot 源码"、"pot 自建公共服务"）或 `pot-app.com/api/dict` 端点 URL（互操作公开事实）。解析逻辑（ECDICT 行格式解析、有道 basic 字段提取）均为独立实现，无 pot 代码表达痕迹。

---

## 代码规范符合性

- 函数行数：`parse_youdao_basic` 36 行、`EcdictProvider::parse_response` 34 行、`YoudaoDictProvider::build_request` 29 行，均 ≤50 行。
- 嵌套深度：最深处（`parse_youdao_basic` 的 wfs 遍历）≤3 层。
- 辅助纯函数 DRY：`group_definitions_by_pos` / `split_pos_prefix` / `is_pos_token` 被 ECDICT 和有道词典共用。
- 注释写「为什么」：字段映射来源、isWord 分流理由、签名复用理由均有说明。
- 无 TODO/FIXME、无装饰性分隔注释、无死代码。
- 公共 API 文档：`EcdictProvider`、`YoudaoDictProvider`、`youdao_sign`（已有）均有 doc comment。

---

## 测试充分性

| 测试 | 覆盖点 | 状态 |
|---|---|---|
| `ecdict_build_and_parse_dict` | 端点/方法/body + 音标/词性分组/词形 | 通过 |
| `ecdict_parse_invalid_json_returns_parse_error` | 非法 JSON → ParseError | 通过 |
| `ecdict_parse_empty_word_returns_parse_error` | 空词条 → ParseError | 通过 |
| `registry_contains_ecdict_free_unofficial` | needs_key=false + is_unofficial=true + build_provider 免 key 成功 | 通过 |
| `youdao_dict_parses_basic_to_dict` | isWord+basic → Dict（音标/释义/词形） | 通过 |
| `youdao_dict_falls_back_to_plain_when_not_word` | 非词 → Plain（具体译文断言） | 通过 |
| `youdao_dict_error_code_maps_to_error` | errorCode "108" → Auth | 通过 |
| `youdao_dict_build_request_does_not_leak_secret` | sentinel 证否密钥泄露 + 签名复用验证 | 通过 |
| `build_provider_youdao_dict_missing_required_fields_returns_err` | 缺 app_secret → Err | 通过 |
| `registry_contains_youdao_dict_keyed` | needs_key=true | 通过 |
| `static_registry_lists_twenty_one_providers`（集成） | 注册表恰好 21 家 | 通过 |

变异 sanity（tester 已验）：A/B/C2/D 全红，判别力充分。debug + release 双绿，clippy 零警告。

---

## 结论

**APPROVE** — 实现正确、安全、符合设计规范、测试充分。I-1 doc comment 遗漏为非阻塞建议，不影响放行。
