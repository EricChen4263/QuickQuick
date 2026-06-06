---
id: TV4-F3-S01
type: coding
parent: TV4-F3
commit: 554abad
acceptance_ids: [TV4-F3-A01]
---

# TV4-F3-S01 编码留痕：Bing 词典（JSON）+ 剑桥（HTML scraper）+ 非词回退

## 概述

新增两个词典源到翻译框架，让注册表达到 pot 全集 23 源：

- **Bing 词典（`bing_dict`）**：`GET bing.com/api/v6/dictionarywords/search`（硬编码 appid，免 key，非官方 JSON 接口），parse JSON → `Dict`。
- **剑桥词典（`cambridge`）**：`GET dictionary.cambridge.org/search/...`（HTML 用 `scraper` crate 解析，免 key，网页抓取，仅英文输入），parse HTML → `Dict`。
- **非词回退**：两源在「非词输入 / 解析无结果」时返回带明确中文提示的 `TranslateError::ParseError`，不 panic、不返垃圾。

复用 F1 地基（`TranslateResponse::Dict { entry }` + `DictEntry`/`PosDefinition`）与 F2 模式（薄 provider 三件职责 + 解析纯函数拆分）。

## 改动文件

- `src-tauri/Cargo.toml`：新增 `scraper = "0.27"`（仅用于剑桥 HTML 解析，基于 html5ever，不执行 JS）。
- `src-tauri/src/translate/providers.rs`：
  - 新增 `BingDictProvider` + parse 纯函数（`parse_bing_dict_entry` / `bing_transcription` / `parse_bing_meaning_group` / `bing_meaning_text`）。
  - 新增 `CambridgeProvider` + parse 纯函数（`parse_cambridge_html` / `parse_cambridge_def_block` / `select_cambridge_audio` / `absolutize_cambridge_url` / `select_first_text` / `select_first_text_in` / `non_empty_text`）。
  - `build_provider` match 新增 `bing_dict` / `cambridge` 分支（免 key）。
  - `registry()` 新增两源能力声明（21 → 23）。
  - 新增冻结测试 `bing_dict_parses_json_to_dict` / `cambridge_parses_html_to_dict` / `dict_source_falls_back_or_hints_on_non_word` + Bing/剑桥错误分支 + build_request 命中 + registry 断言测试（用录制 JSON / HTML fixture）。
  - **I-1 修复**：`build_provider` doc 注释补齐缺失项（bing/ecdict/bing_dict/cambridge 免 key、youdao_dict 同有道 key），与实际 match 分支一致。
- `src-tauri/tests/translate.rs`：
  - `static_registry_lists_twenty_one_providers` → `static_registry_lists_twenty_three_providers`（断言 23）。
  - `keyless_ids` 补 `bing_dict` / `cambridge`。
- `src-tauri/src/translate/credential.rs`：**无需改动**——bing_dict/cambridge 均免 key，落入既有 `_ => vec![]` 兜底。

## 字段映射

### Bing 词典 JSON → DictEntry
- 音标 `phonetic`：`value[0].pronunciations[*].transcriptions[*].transcription`（首个）。
- 释义 `definitions`（按词性分组）：`value[0].meaningGroups[*]`，词性取 `partsOfSpeech[*].name`，
  释义文本取 `meanings[*].richDefinitions[*].fragments[*].text` 拼接。
- 变形 `inflections`：`value[0].inflections[*].displayText`。
- needs_key=false、is_unofficial=true（硬编码 appid 客户端标识，非官方接口）。

### 剑桥 HTML → DictEntry（scraper CSS 选择器）
- 音标 `phonetic`：`.ipa` 首个文本。
- 音频 `audio`：`source[type="audio/mpeg"]` 的 `src`（相对路径补全为 `https://dictionary.cambridge.org` 绝对地址）。
- 释义 `definitions`：`.def-block` 各块，词性取块内 `.pos`，释义合并块内 `.def`（英文）+ `.trans`（汉译）。
- needs_key=false、is_unofficial=true（网页抓取，有反爬风险）。

## 非词回退语义

- **Bing 词典**：`value` 为空数组（非词/未收录）或词条音标+释义俱空 → `ParseError("Bing 词典未收录该词或非单词输入" / "Bing 词典返回空词条")`。
- **剑桥**：页面无 `.def-block`（搜索无结果/非英文词）→ `ParseError("剑桥词典未找到该词的释义（非英文单词或无结果）")`。
- 两源对完全空 / 无关 / 非法响应均返回 `Err` 不 panic（健壮性兜底，冻结测试 `dict_source_falls_back_or_hints_on_non_word` 覆盖）。
- 设计上「回退普通翻译」由上层选源/兜底链负责（本层 provider 给明确错误提示，不擅自改翻译语义）。

## 关键决策

- **appid 硬编码不违反密钥红线**：该 appid 是公开非官方接口要求的客户端标识（同 Yandex 会话 id / Transmart client_key），非用户密钥、非签名密钥，提为具名常量 `BING_DICT_APPID` 并注释来源。
- **scraper 选择器用 `expect`**：选择器字符串为编译期固定的合法 CSS，解析失败属编程错误（非运行时数据问题），用 `expect` 暴露而非把编程错误伪装成数据错误。运行时数据（HTML 内容）的缺失走 `Option`/`ParseError`，不 panic。
- **安全**：HTML 解析仅取文本/属性（`scraper` 基于 html5ever，不执行 JavaScript），无脚本注入风险。

## scraper 依赖

- 版本 `0.27`（当前稳定版），仅剑桥 `parse_cambridge_html` 使用。
- 新增后 `Cargo.lock` 变化属正常（拉入 html5ever/selectors 等传递依赖）。

## registry 23（pot 全集）

注册表顺序：lingva / google_free / yandex / transmart / bing / ecdict / **bing_dict / cambridge** / baidu / baidu_field / youdao / youdao_dict / caiyun / niutrans / tencent / alibaba / volcengine / deepl_free / google / openai / ollama / chatglm / gemini = 23。

## 验证证据

- 冻结三测试全绿：见 `artifacts/lib-full-green.log`
  - `bing_dict_parses_json_to_dict ... ok`
  - `cambridge_parses_html_to_dict ... ok`
  - `dict_source_falls_back_or_hints_on_non_word ... ok`
  - `test result: ok. 264 passed; 0 failed`（lib 单测全量）
- 注册表整合测试：`static_registry_lists_twenty_three_providers ... ok`、`static_registry_keyed_providers_need_key ... ok`（tests/translate.rs）。
- 终检（全量 cargo test / --release / clippy）结果回填于下方「终检」节。

## 坑

- **cargo test 单次只接受一个 positional TESTNAME 过滤**：多个冻结测试名无公共子串时，分别跑或跑全量 lib 后 grep 三名，不能 `cargo test a b c`（cargo 报 unexpected argument）。
- **TDD 守卫识别测试文件靠路径**：`src/translate/providers.rs` 的 inline `#[cfg(test)]` 不被守卫视作测试文件，须先让一个真测试文件（`tests/translate.rs`）的改动落盘，才能编辑 providers.rs（守卫按 `tests/`/`_test.` 等路径判定）。

## 终检（实跑，证据见 artifacts/）

- `cargo test`（全量，debug）：exit 0，全部 test 二进制 0 failed；lib 单测 `test result: ok. 264 passed; 0 failed`。证据 `artifacts/full-test.log`、`artifacts/lib-full-green.log`。
- `cargo test --release`：exit 0，32 个 `test result: ok.` 行，0 failed。证据 `artifacts/release-test.log`。
- `cargo clippy --all-targets -- -D warnings`：exit 0，无 warning/error。证据 `artifacts/clippy.log`。
- 自检：无装饰性分隔注释、无 TODO/FIXME、无 eprintln/println/log/dbg!（密钥安全）、3 个冻结测试函数真实存在且 `... ok`。
