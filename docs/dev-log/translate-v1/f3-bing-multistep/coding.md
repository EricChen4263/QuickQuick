---
id: TV1-F3-S01-code
type: coding_record
level: 小功能
parent: TV1-F3
status: done
commit: PENDING
acceptance_ids: [TV1-F3-A01]
---

# TV1-F3-S01 编码留痕：provider 多步请求架构扩展 + Bing 免 key 源

## 一、目标与范围

设计文档 `docs/design/translation-sources-pot.md` §四（多步请求 / HttpExecutor 注入）+ §二.2.1（Bing 两步机制）。
验收 `TV1-F3-A01`：provider 抽象支持多步请求（注入 HTTP 执行器）；Bing 源先取 edge auth token、再 POST 翻译，对录制样例正确解析出译文。

**许可红线遵守**：Bing edge 接口协议按公开互操作事实独立实现，未复制/打开 pot（GPL-3.0）任何源代码。注释来源标「公开互操作协议事实（非 pot 源码）」。

## 二、现有执行框架分析

- `TranslateProvider` trait（`src-tauri/src/translate/mod.rs`）原为薄三件职责：`capability()` + `build_request()`（单次 HTTP 描述符）+ `parse_response()`。HTTP 执行由核心框架横切。
- `HttpExecutor` trait 原定义在 `ipc::translate`，生产实现 `UreqExecutor`（同步 ureq、10s 超时）、测试用 `FakeExecutor`（返回固定串）。
- 执行流在 `translate_text_impl`（`src-tauri/src/ipc/translate.rs`）：手动三步 `build_request → exec.execute → parse_response`。
- 既有 7 源全部单步：lingva / google_free / yandex / transmart（免 key）+ baidu / deepl_free / google（需 key）。

**架构定位问题**：`translate` 默认方法签名要引用 `HttpExecutor`，但 trait 在 mod.rs、`HttpExecutor` 原在 ipc 层 → 模块依赖反转。解决：把 `HttpExecutor` trait **上移到 `translate::mod`（核心框架层）**，`ipc::translate` 改为 `use crate::translate::HttpExecutor`，生产实现 `UreqExecutor` 仍留 ipc 层。这是正确分层（执行器抽象属翻译核心、ureq 实现属 IO 边界）。

## 三、curl 实测 Bing 两步（可用，已录样例）

证据：`artifacts/bing-auth-token-head.txt`、`artifacts/bing-translate-sample.json`。

- **步骤1** `GET https://edge.microsoft.com/translate/auth` → HTTP 200，纯文本 JWT（788 字节，`eyJhbG…` ES256 头）。
- **步骤2** `POST https://api-edge.cognitive.microsofttranslator.com/translate?api-version=3.0&from={src}&to={tgt}`，Header `Authorization: Bearer {token}` + `Content-Type: application/json`，body `[{"Text":"glacier"}]` → HTTP 200，`[{"translations":[{"text":"冰川","to":"zh-Hans"}]}]`。
- 反向/繁体实测均 200：`zh-Hans→en` 得 "Hello World"；`en→zh-Hant` 得 "哈囉，世界"。**确认 Bing 区分简繁：简中 zh-Hans、繁中 zh-Hant**。

结论：**Bing 完全可用**（不暂缓）。

## 四、架构扩展决策

1. `HttpExecutor` trait 上移到 `translate::mod`（核心层）；`ipc::translate` 删本地定义、改 `use`。`UreqExecutor`/`FakeExecutor` 仍在 ipc 层（实现未动），全部既有引用兼容。
2. `TranslateProvider` 新增**带默认实现**的方法：
   ```rust
   fn translate(&self, req, executor: &dyn HttpExecutor) -> Result<TranslateResponse, TranslateError> {
       let http_req = self.build_request(req);
       let raw = executor.execute(&http_req)?;
       self.parse_response(&raw)
   }
   ```
   默认实现与重构前手动三步**逐字等价** → 既有 7 源零改动自动适配。
3. `translate_text_impl` 执行流：手动三步 → `provider.translate(&req, exec)`，对外行为不变。

**既有源零回归证据**：架构改造后单独跑既有 lib 测试 178 passed（`artifacts/` 内 run1 日志包含），未改任何单步源代码。

## 五、Bing 实现

`src-tauri/src/translate/providers.rs` 新增 `BingProvider`（id=`bing`，needs_key=false）：

- override `translate` 做两步：
  1. `executor.execute(build_auth_request())` 取 token（纯文本 JWT）；token trim 后为空 → `TranslateError::Auth`（不 panic）。
  2. `executor.execute(build_translate_request(req, token))` POST 翻译 → 复用 `parse_response`。
  3. token 步失败短路、不发翻译请求（实测断言 seen_urls.len()==1）。
- `build_request` 默认实现退化为「无 token 的翻译步」（本源只走 translate 两步路径，不会被单步框架调）。
- `parse_response`：`[0].translations[0].text` 取译文；缺字段/空数组/非法 JSON → `ParseError`。
- 端点常量 `BING_AUTH_URL` / `BING_TRANSLATE_BASE`，注释标公开协议来源（非 pot）。
- body 用 `serde_json::json!([{ "Text": req.text }])` 自动转义，避免手拼注入。

**lang.rs**：`map_lang_for_provider` 加 `"bing" => map_for_bing`，区分简繁（zh→zh-Hans、zh-TW→zh-Hant、en→en、其余透传）。

**注册**：`build_provider` 加 `"bing"` 分支；`registry()` 追加 `BingProvider`（置于 transmart 后、baidu 前）。

**tests/translate.rs**：注册表计数 7→8（含函数改名 `static_registry_lists_eight_providers`）；免 key 集合 `keyless_ids` 补 `"bing"`。

## 六、TDD 红→绿证据

- RED：先写 `bing_*` 测试 + lang 映射测试 + RoutingFakeExecutor（按 URL 路由 canned 响应、记录 URL 与 Authorization 头）；因 `BingProvider` 不存在编译失败（预期红）。
- 中途一次失败：`bing_translate_step_failure_returns_error` 期望 RateLimit，根因是测试桩 `clone_result` 把所有 Err 损成 Network；诊断后改桩为 `RefCell<Option<Result>>` take（TranslateError 不实现 Clone），保留原错误变体。
- GREEN：`bing` 过滤跑 8 passed（含 map_lang_for_bing）：
  - `bing_two_step_translate_with_mock_executor ... ok`（两步顺序 + token 传递 + from=en&to=zh-Hans）
  - `bing_parse_response_extracts_translation_text ... ok`
  - `bing_translate_token_step_failure_returns_error_not_panic ... ok`
  - `bing_translate_empty_token_returns_auth_error ... ok`
  - `bing_translate_step_failure_returns_error ... ok`
  - `bing_is_keyless_and_built_without_credentials ... ok`
  - `registry_contains_bing_keyless ... ok`
  - `map_lang_for_bing_uses_zh_hans_and_hant ... ok`
- 全量 `cargo test`：lib 186 passed + 各集成测试全 ok，0 failed（证据 `artifacts/cargo-test-run1.log`，32 个 `test result: ok`）。
- 连跑 3× 与 clippy：见 test.md / 下方收尾。

## 七、改动文件

- `src-tauri/src/translate/mod.rs`：上移 `HttpExecutor` trait；`TranslateProvider` 加默认 `translate` 方法。
- `src-tauri/src/ipc/translate.rs`：删本地 `HttpExecutor` 定义改 `use`；执行流改调 `provider.translate`。
- `src-tauri/src/translate/providers.rs`：新增 `BingProvider`（两步 override）；build_provider/registry 接入；测试 + RoutingFakeExecutor。
- `src-tauri/src/translate/lang.rs`：`map_for_bing`（简繁映射）+ 测试。
- `src-tauri/tests/translate.rs`：注册表计数 7→8、免 key 集合补 bing。

## 八、安全 / 规范自检

- needs_key=false，build_provider("bing", &[]) 不读凭据存储（测试 `bing_is_keyless_and_built_without_credentials` 验证）。
- 错误处理不打印待译文本/译文（无 eprintln 涉敏感内容）；token 不入日志。
- 函数 ≤50 行、嵌套 ≤3 层；无装饰性分隔注释；无 TODO/FIXME。
- 端点注释标公开协议来源，未抄 pot。
