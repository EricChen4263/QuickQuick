---
id: TV4-F3-S01-review
type: review_report
level: 小功能
parent: TV4-F3
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F3-A01]
author: code-reviewer
---

# TV4-F3-S01 代码审查报告：Bing 词典 JSON + 剑桥 HTML scraper + 非词回退

## 审查结论

**APPROVE** — 无置信度 ≥80 的 Critical 或 Important 问题。改动质量良好，可放行合并。

---

## 审查范围

| 文件 | 改动类型 |
|---|---|
| `src-tauri/Cargo.toml` | 新增 `scraper = "0.27"` 依赖 |
| `src-tauri/src/translate/providers.rs` | 新增 `BingDictProvider` / `CambridgeProvider` + 11 个解析纯函数 + 10 个单测；`build_provider` + `registry` 21→23；I-1 doc 修复 |
| `src-tauri/tests/translate.rs` | 注册表数量 21→23；`keyless_ids` 补 `bing_dict` / `cambridge` |

---

## 发现项

### Critical（阻塞）

无。

### Important（置信度 ≥80，非阻塞建议）

无。

### 低置信度观察（< 80，不报）

以下两点置信度均低于 80，不构成阻塞或建议项，仅记录供参考：

1. **CSS selector 重复 parse（置信度 72）**：`parse_cambridge_def_block` 对每个 `.def-block` 元素调用 3 次 `Selector::parse`（`.pos`/`.def`/`.trans`），对含 N 个块的页面总开销为 3N+3 次 parse。但此路径是网络 IO 绑定的一次性操作（词典查询非热路径），实际影响微弱，不构成 Important 问题。若未来性能敏感，可将选择器提取为惰性全局常量（`once_cell` 或 `std::sync::OnceLock`）。

2. **`bing_meaning_text` 多 fragment 无分隔符拼接（置信度 68）**：`richDefinitions[*].fragments[*].text` 直接 collect 为 String 无空格分隔；若 Bing 返回多 fragment 则拼接无分隔符。当前 fixture 及已观测响应每条 meaning 均为单 fragment，此场景实际罕见。设计文档 §二.2.4 明确要求"拼接"，符合规范意图。

---

## 解析正确性

### Bing 词典 JSON（providers.rs 行 1429–1490）

- **音标**：`pronunciations[*].transcriptions[*].transcription` 逐层 `find_map` 取首个，遇空数组/缺字段均返回 `None` 不 panic — 正确。
- **释义（词性分组）**：`meaningGroups[*]` 迭代 → `parse_bing_meaning_group` → `partsOfSpeech[*].name` 取词性，`meanings[*].richDefinitions[*].fragments[*].text` collect 为释义文本 — 字段路径与 coding.md 字段映射完全一致。
- **变形**：`inflections[*].displayText` filter_map 取字符串，缺字段跳过不 panic — 正确。
- **非词/未收录**：`value` 为空数组 → `ok_or_else` 返回明确 `ParseError`；音标+释义均空的退化词条再设第二道防线 — 深防御正确，tester 变异 D 已验两道同时失效才为假绿，单道失效仍由另一道兜底，符合设计预期。

### 剑桥词典 HTML（providers.rs 行 1557–1645）

- **音标**：`select_first_text(&doc, ".ipa")` 取首个 `.ipa` 文本，与设计文档 §二.2.4 一致。
- **音频**：`source[type="audio/mpeg"]` 取 `src` → `absolutize_cambridge_url` 补全；相对路径加 `CAMBRIDGE_ORIGIN` 前缀，已是 `http://`/`https://` 开头则原样返回 — 健壮，tester 变异 C 已验。
- **释义块**：`.def-block` → `parse_cambridge_def_block` → `.pos`（词性）/ `.def`（英文释义）/ `.trans`（汉译），无内容的块跳过（返回 `None`）— 与设计文档一致。
- **非词/无结果**：无 `.def-block` → `ParseError`；HTML5 畸形容错由 html5ever 天然处理，不 panic — 正确，tester 变异 B 已验。
- **`expect` 使用**：选择器字符串为编译期固定合法 CSS，`Selector::parse` 失败属编程错误而非运行时数据问题，用 `expect` 暴露优于静默吞掉 — 决策合理，代码注释已说明理由。

---

## 设计符合性（设计文档 §二.2.4 + §四 + §五.V4）

| 要求 | 实现 | 结论 |
|---|---|---|
| Bing 词典 `bing.com/api/v6/dictionarywords/search` + 硬编码 appid | `BING_DICT_BASE` + `BING_DICT_APPID` 常量，URL 含 `appid=` 参数 | 符合 |
| Bing 词典 音标/按词性分组释义/变形 → `Dict` | `parse_bing_dict_entry` 三字段均完整提取 | 符合 |
| 剑桥 `dictionary.cambridge.org/search/.../direct/` HTML | `CAMBRIDGE_SEARCH_BASE` 常量，GET 请求 | 符合 |
| 剑桥 音标/音频/释义（仅英文输入）→ `Dict` | `.ipa`/`source[type=audio/mpeg]`/`.def-block` 均实现 | 符合 |
| 音频相对 URL → 绝对地址 | `absolutize_cambridge_url` 检查前缀后补 `CAMBRIDGE_ORIGIN` | 符合 |
| 非词回退：明确 `ParseError` 不 panic 不返垃圾 | 两道防线 + HTML 无结果检查均返回带中文提示的 `ParseError` | 符合 |
| 两源 `needs_key=false`、`is_unofficial=true` | `capability()` 声明与 `keyless_ids` 测试均一致 | 符合 |
| registry 21→23（pot 全集） | `registry()` 精确 23 项，`static_registry_lists_twenty_three_providers` 断言同步 | 符合 |
| 独立重写、不抄 pot 代码 | 见下节 | 符合 |

---

## 安全审查

- **`BING_DICT_APPID` 硬编码合规**：该值为 Bing 网页端公开可观测的客户端标识，非用户密钥、非签名密钥；常量有完整注释说明来源和性质，与 Yandex 会话 id / Transmart client_key 同等处理，不违反密钥不入库约定。
- **HTML 解析无 JS 执行**：`scraper = "0.27"` 基于 html5ever + cssparser + selectors，无 v8/boa/quickjs 等 JS 引擎（tester 已通过 `cargo tree -p scraper` 验证依赖链），解析仅取文本/属性，无脚本注入风险。
- **无调试打印**：`providers.rs` 全文 `eprintln`/`println!`/`dbg!`/`log::` 零匹配（grep 已验）。
- **错误消息无凭据泄露**：`ParseError` 文案均为固定中文描述，不含任何凭据值；两源均免 key，无需 sentinel 测试（正确跳过）。

---

## 未抄 pot 代码确认

实现注释均标注 `不参考任何第三方源码` 并说明端点来源为公开接口形态观测；代码结构与 F1/F2 既有模式（薄 provider 三件职责 + 解析纯函数拆分）完全同构，无 pot 代码表达痕迹。

---

## I-1 doc 修复复核

`build_provider` doc comment 行 15 已补入 `bing_dict`、`cambridge`（免 key），行 18 已补入 `youdao_dict`（同有道 key）。与 F2 审查标注的遗漏项（`bing`/`ecdict`/`bing_dict`/`cambridge` 免 key、`youdao_dict` 同有道 key）完全对应，修复完整。match 分支与 doc 枚举一致。

---

## 代码规范符合性

- **函数行数**：所有新增纯函数均 ≤ 30 行（最大为 `parse_cambridge_html` ≈ 31 行），远低于 50 行上限。
- **嵌套深度**：最深处（`bing_meaning_text` 的多层 `filter_map`/`flatten`）≤ 3 层，符合规范。
- **DRY**：`select_first_text` 与 `select_first_text_in` 复用 `non_empty_text`；Bing 解析纯函数拆分 4 层（entry → pronunciation → meaning_group → meaning_text），与 F2 模式一致。
- **命名**：纯函数均遵循「动词+名词」命名（`parse_bing_dict_entry`、`select_cambridge_audio`、`absolutize_cambridge_url` 等），无 `tmp`/`x`/`flag`。
- **注释写「为什么」**：`expect` 使用理由、`BING_DICT_APPID` 来源与安全性说明、`mkt` 参数作用、非词两道防线设计均有说明注释。
- **无魔术值**：CSS 选择器字符串虽未提为全局常量，但均在紧邻调用处并附有 `expect` 说明（选择器字面量不对应业务含义模糊的数字，处理方式可接受）。
- **无 TODO/FIXME/死代码/装饰性分隔注释**。
- **公共 API 文档**：`BingDictProvider`、`CambridgeProvider` 均有完整 `///` doc comment，含端点格式、字段映射、appid 性质说明。

---

## 测试充分性

| 测试 | 覆盖点 | 状态 |
|---|---|---|
| `bing_dict_parses_json_to_dict` | 音标/名词释义/动词释义/变形完整验证（锚定具体值） | 通过 |
| `bing_dict_parse_invalid_json_returns_parse_error` | 非法 JSON → ParseError（不 panic） | 通过 |
| `bing_dict_build_request_hits_endpoint_with_appid` | URL 含端点/appid/待查词 | 通过 |
| `registry_contains_bing_dict_free_unofficial` | needs_key=false + is_unofficial=true + 免 key 构造成功 | 通过 |
| `cambridge_parses_html_to_dict` | 音标/音频绝对 URL/英文释义/汉译（锚定具体值） | 通过 |
| `cambridge_build_request_hits_search_endpoint` | URL 含剑桥 search 端点/待查词 | 通过 |
| `registry_contains_cambridge_free_unofficial` | needs_key=false + is_unofficial=true + 免 key 构造成功 | 通过 |
| `dict_source_falls_back_or_hints_on_non_word` | Bing 空数组 ParseError + 剑桥无结果 ParseError + 两源空/异常响应均 Err（含非空 msg 断言） | 通过 |
| `static_registry_lists_twenty_three_providers`（集成） | 注册表恰好 23 家 | 通过 |
| `static_registry_keyed_providers_need_key`（集成） | keyless_ids 含 bing_dict/cambridge | 通过 |

变异 sanity（tester 已验）：A–D 全红，debug×5 无 flaky，release 双绿，clippy 零警告（513 passed）。

---

## 结论

**APPROVE** — 实现正确、安全、与设计文档 §二.2.4/§四/§五.V4 完全符合、测试充分、I-1 修复完整。无需修改，可放行合并。
