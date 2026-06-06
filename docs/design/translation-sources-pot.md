# QuickQuick 翻译源对齐 pot 设计方案

> 日期：2026-06-05
> 范围：把 QuickQuick 的翻译源从现有 4 家（MyMemory / 百度 / DeepL / Google）整体重做，对齐开源翻译软件 **pot（pot-app / pot-desktop）** 的全部内置翻译源；让用户**不填 key 即可获得准确翻译**，并提供丰富可选源。
> 性质：跨会话大工程，走 `/goal` 自主分批 + `docs/dev-log/` 留痕续跑；本文是其**冻结设计源**，分批实现的版本验收标准从本文机械派生。
> 状态：**已冻结**（2026-06-06 用户确认：默认 **Lingva** · **全 21 源** V1→V4 · **接受非官方接口**并加标注 · **独立重写、不抄 pot 代码**）

---

## 修订记录 · Post-freeze 变更（2026-06-07）

> 本文 2026-06-06 冻结并驱动了 TV1–TV4 实现。下列变更发生在冻结之后，针对真机采证暴露的问题与一次 license 复核，已在分支 `feat/translate-sources-cleanup`（commits 85375b8 / 83aded5 / ca573b1 / 3d7990f / 4f1c5f0）实现并通过 feature-dev Phase 6 验证（make verify 全绿 + release 518 passed + 5 变异体全杀 + 审查无阻塞）。**本节为当前实现的权威说明，其下冻结正文保留为历史设计记录。**

### A. GPL 衍生排查结论（实证复核 〇章原则）

实拉 pot-desktop 源码，逐个比对全部 23 个 provider 的端点/常量/算法/响应字段，**结论：23 源全部独立实现，无 GPL copyleft 传染风险**。最硬证据：Bing 词典 appid 两边取值不同（我方 `8F6F50E7…` vs pot `371E7B2A…`）、百度/有道等 HTTP 方法不同（我方 POST vs pot GET）、实现语言不同（Rust vs JS）。〇章「独立重写、不抄 pot 代码」原则经实证成立。pot-desktop 及其翻译插件均为 GPL-3.0。

### B. 去 pot 服务器化（白嫖治理，非 license 问题）

真机采证发现两处端点直连 pot 私人服务器——著作权不保护 URL，这属「用别人的服务器」而非代码侵权，但不可持续（pot 可随时切断，ECDICT 已 405）：

1. **ECDICT**：原 `POST pot-app.com/api/dict`（真机 405 已失效）→ **改本地内置离线 SQLite**。词库取 skywind3000/ECDICT（**MIT 协议，可自由分发**），精简 76 万词条、只保留 word/phonetic/translation/exchange 四列（≈40–60MB）。实现：新增 `EcdictDb` DAO + `EcdictProvider` override `translate()` 走本地只读查询、零网络；`is_unofficial` 改为 false。生成器 `src-tauri/tools/gen_ecdict_db.py` 入 git；真库与源 csv 不入 git（gitignore）；CI 拉固定 commit（SHA `bc015ed`）的 csv 生成 db 后打包。
2. **默认源 Lingva**：端点 `lingva.pot-app.com`（pot 自建实例）→ **默认源改为 `google_free`**（直连 Google gtx 接口，不依赖任何第三方志愿实例）；Lingva 保留为可选源，端点迁至公共实例 `lingva.ml`。

### C. 下架失效且违 ToS 的词典源

真机采证：**Bing 词典 401**（硬编码 appid 被微软拒）、**剑桥 403**（HTML 抓取被反爬封）。两者既违各自 ToS 又已失效，**直接下架移除**（连带删除 `scraper` 依赖）。存量用户若选中已删源，由 `resolve_provider_or_fallback` 自动回退默认源。

### 变更后的当前状态（覆盖下方冻结正文的对应处）

- **内置源数量：21**（原 23 实现 − 下架 bing_dict / cambridge 共 2 个）。
- **默认源：`google_free`**（不再是 Lingva）。
- **词典源：ECDICT（本地离线）+ 有道词典（需 key）** 两个；Bing 词典 / 剑桥已移除。
- **遗留真机采证**（headless 不可验）：真库生成 + bundle 随包 + e2e 查词命中。

---

## 〇、实现总原则：独立重写，不抄 pot 代码（许可红线，最高优先级）

pot-desktop 采用 **GPL-3.0（强 copyleft）**；QuickQuick 目前无 LICENSE 文件（默认「保留所有权利」/专有）。**直接复制 pot 源代码会使 QuickQuick 被迫整体以 GPL-3.0 开源（传染）**——故本项目**绝不复制 pot 的任何源代码**，也不近似改写其代码结构/表达。

合法做法（用户 2026-06-06 已确认）：

- **支持与 pot 相同的全部翻译源**，但**用 Rust 完全独立实现**。
- **协议细节的来源（关键）**：
  - **需 key 的官方源**（百度/百度专业/腾讯云/阿里/火山/彩云/小牛/官方 DeepL/有道/OpenAI/ChatGLM/Gemini）→ 一律照**各厂商官方 API 文档**实现，**完全不参考 pot 代码**。
  - **免 key 非官方接口**（Google gtx / Bing edge / DeepL-free / Yandex / Transmart / Lingva / Bing 词典 / 剑桥）→ 其 HTTP 端点、参数、签名算法属**功能性事实 / 互操作信息**，不受著作权保护；按这些**公开事实**独立编写原创 Rust，不照搬 pot 的具体实现表达。
- **溯源锚**：每个源在代码注释里标注其**官方 API 文档 URL**（而非 pot 源码 URL）。
- pot 仅作为「**应支持哪些源**」的功能清单参考，**不作为代码来源**；实现期不打开/不粘贴 pot 的 `index.jsx` 代码。

> 法律依据：API 端点/参数/签名算法/响应字段=方法与事实，对接同一第三方服务属互操作，不受版权保护；而 pot 的**源代码表达**受版权保护并受 GPL 约束，故规避。本项目后续若要开源，许可由作者另行决定，不因翻译源实现被动绑定 GPL。

---

## 一、背景与诉求

### 现状

QuickQuick 当前有 4 个翻译源（`src-tauri/src/translate/providers.rs`）：

| 源 | 免 key | 质量 |
|---|---|---|
| MyMemory | ✅ 默认 | ❌ 差——众包 TM，常返垃圾 |
| 百度 / DeepL / Google | ❌ 需 key | 好，但要注册 key |

**实测痛点**（2026-06-05，真机）：默认源 MyMemory 把「冰川」翻成 **"Bing Chuan"**（拼音）。直接 curl MyMemory API 确认：其官方 `responseData.translatedText` 返回的就是一条 `quality=0` 的用户投稿垃圾（投稿人 mariusrvar01@gmail.com），更好的 "Glaciate"（quality=80，Wikipedia）反被 `match` 相似度排序压在后面。**这不是解析 bug，是免费众包源的数据本身脏。**

### 诉求

用户要求：**「将应用里的翻译源全部统一为 pot 里的所有翻译源」**，且期望**不填 key 也能用**。

### 现实约束（必须先对齐认知）

「不填 key 也能用」**只对部分源成立**。基于 pot 源码调研（直接抓取 `github.com/pot-app/pot-desktop` master 分支 `src/services/translate/` 各源实现）：

- **真正免 key**：用免费 / 自建 / 非官方网页接口，无需注册。
- **必须官方 key**：厂商官方 API（注册、限额 / 付费）。
- **免 key 但非官方**：靠抓 token / 伪装客户端 / 网页抓取，**随对方改版即可能失效**，须如实标注。

---

## 二、源清单（对齐 pot，共 21 个内置源）

> 证据：pot-desktop `src/services/translate/` 各源 `index.jsx` / `Config.jsx` / `info.ts`。

### 2.1 机器翻译 · 免 key

| 源 | 端点 / 机制 | 取译文字段 | 稳定性 |
|---|---|---|---|
| **Lingva** | `lingva.pot-app.com/api/v1/{from}/{to}/{text}` GET，无认证 | `data.translation` | **A 级**：pot 自建实例，最稳，不依赖第三方 |
| **Google（免费）** | `translate.google.com/translate_a/single?client=gtx` GET | `result[0][*][0]` 拼接 | B 级：非官方，准但有失效险（实测「冰川→glacier」✅） |
| **Bing（免费）** | 两步：`edge.microsoft.com/translate/auth` 抓 token → `api-edge.cognitive.microsofttranslator.com/translate` POST | `[0].translations[0].text` | B 级：非官方 Edge 接口，需抓 token |
| **DeepL（free 模式）** | `www2.deepl.com/jsonrpc` POST（JSON-RPC + 空格混淆） | `result.texts[0].text` | B 级：非官方，有频率限制 |
| **Transmart（腾讯交互翻译·匿名）** | `transmart.qq.com/api/imt` POST，匿名可用 | `auto_translation[*]` 拼接 | B 级：非官方，可选填 user/token 提限额 |
| **Yandex（免费）** | `translate.yandex.net/api/v1/tr.json/translate` POST，伪装 Android | `text[0]` | B 级：非官方绕过，有封禁险 |

### 2.2 机器翻译 · 需 key

| 源 | 端点 | 鉴权 | 取译文 | 凭据字段 |
|---|---|---|---|---|
| **百度** | `fanyi-api.baidu.com/.../translate` GET | `MD5(appid+text+salt+secret)` | `trans_result[*].dst` | `appid`、`secret` |
| **百度专业** | `.../fieldtranslate` GET | MD5 串多加 `field` | `trans_result[*].dst` | `appid`、`secret`、`field` |
| **腾讯云 TMT** | `tmt.tencentcloudapi.com` POST | TC3-HMAC-SHA256 | `Response.TargetText` | `secret_id`、`secret_key` |
| **阿里** | `mt.cn-hangzhou.aliyuncs.com/...` GET | HMAC-SHA1 + Base64 | `Data.Translated` | `accesskey_id`、`accesskey_secret` |
| **火山** | `open.volcengineapi.com/?Action=TranslateText` POST | AWS SigV4 四层 HMAC-SHA256 | `TranslationList[*].Translation` | `appid`、`secret`、`region` |
| **彩云** | `api.interpreter.caiyunai.com/v1/translator` POST | `x-authorization: token {token}` | `target[0]` | `token` |
| **小牛** | `api.niutrans.com/.../translation` POST | 请求体内 `apikey` | `tgt_text` | `apikey` |
| **DeepL（api 模式）** | `api-free.deepl.com/v2/translate` 或 `api.deepl.com` POST | `Authorization: DeepL-Auth-Key {key}` | `translations[0].text` | `authKey` |
| **有道** | `openapi.youdao.com/api` GET | SHA256 signType=v3 | `translation[*]`（兼词典 `basic`） | `appkey`、`key` |

### 2.3 LLM 对话翻译

| 源 | 端点 | 鉴权 | 配置字段 | 免 key |
|---|---|---|---|---|
| **OpenAI** | 可配置，默认 `api.openai.com/v1/chat/completions` POST | `Bearer {key}` / Azure `api-key` | `requestPath`、`apiKey`、`model`、`service`、`promptList`、`stream` | ❌ |
| **ChatGLM（智谱）** | `open.bigmodel.cn/api/paas/v4/chat/completions` POST | JWT HS256（`{id}.{secret}` 拆分） | `apiKey`、`model`、`promptList` | ❌ |
| **Gemini Pro** | `generativelanguage.googleapis.com/.../gemini-pro:generateContent?key=` POST | key 作 URL 参数 | `apiKey`、`requestPath`、`promptList` | ❌ |
| **Ollama（本地）** | `localhost:11434`（可配置） | 无 | `requestPath`、`model`、`promptList` | ✅ 本地，需自部署 |

### 2.4 词典

| 源 | 端点 | 返回形态 | 免 key |
|---|---|---|---|
| **Bing 词典** | `bing.com/api/v6/dictionarywords/search`（硬编码 appid） | 音标 + 按词性分组释义 + 变形 | ✅ 非官方 |
| **剑桥词典** | `dictionary.cambridge.org/search/...` GET | HTML 解析：音标 + 音频 + 释义；仅英文输入 | ✅ 网页抓取，有反爬险 |
| **ECDICT** | `pot-app.com/api/dict` POST | 英汉词条完整结构 | ✅ pot 自建 |
| **有道（词条模式）** | 同有道翻译，`isWord===true` 时 | 音标 + explains + 词形 | ❌（同有道 key） |

---

## 三、关键产品决策（★含待确认项）

### 决策 1 · 默认源 ✅ 已定：**Lingva**（Google 作并列免 key 备选/兜底）

去掉 MyMemory 后必须有一个**免 key 默认源**。候选：

| 方案 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| **Lingva** | 最稳（pot 自建实例）、纯 GET 实现最简 | 依赖 pot-app 服务器可用性；译质=Google 引擎 | **推荐**：稳定优先，开箱即准 |
| **Google 免费** | 最准、覆盖语种最广 | 非官方 `gtx` 接口，Google 可随时封 | 备选 / 兜底 |

> 倾向：**默认 Lingva，Google 作为并列首选**。两者都免 key，失败时可互为兜底。最终默认源请你拍板。

### 决策 2 · 纳入范围 ✅ 已定：**全 21 源**（V1→V4 全做）

你的要求是「pot 全部源」。本文按 **全部 21 源** 规划并**分批**实现。若你想砍掉某类（如 LLM 或词典），在审阅时勾掉即可——这会同步删减对应版本：

- 机翻免 key（6）+ 机翻需 key（9）= **15 个机器翻译源**（核心，建议必做）
- LLM（4）：OpenAI / ChatGLM / Gemini / Ollama（形态差异大，需模型 + Prompt 配置）
- 词典（4）：Bing 词典 / 剑桥 / ECDICT / 有道词典（返回词条，需独立展示组件）

### 决策 3 · 非官方接口的呈现 ✅ 已定：**接受纳入 + 加标注**

Google / Bing / DeepL-free / Yandex / Transmart / Bing 词典 / 剑桥都是非官方接口。建议：

- UI 上对这些源标注「⚠ 非官方接口，可能失效」小字。
- 调用失败时给明确降级提示（而非笼统「翻译失败」），并可建议切换到 Lingva / 其它源。
- 文档/设置页声明使用风险（第三方 ToS）。

### 决策 4 · 已确认（无需你额外拍板）

- **移除 MyMemory**（用户明确要求）。
- **需 key 源未配置则置灰**——沿用现有 DirBar 行为（`isProviderConfigured`），不阻塞免 key 源。
- **凭据安全**沿用现有 keychain / 文件密钥库机制，key 不入库、不入日志。

---

## 四、架构改造（关键，决定可行性）

当前 provider 抽象（`TranslateProvider` trait）= `capability()` + `build_request()`（**单次** HTTP）+ `parse_response()`，由核心框架统一执行 HTTP / 重试 / 超时。pot 的源有几类**突破单次-请求-即-解析**的模型，必须先扩展抽象：

| 突破点 | 涉及源 | 抽象改造 |
|---|---|---|
| **多步请求**（先握手再翻译） | Bing（抓 token） | trait 支持「provider 自行编排多次 HTTP」：把 HTTP 执行器（`HttpExecutor`）注入 provider，让其内部按需多次调用，而非只声明一个 request。 |
| **非 JSON 解析** | 剑桥（HTML） | `parse_response` 不假定 JSON；HTML 解析用轻量选择器（如 `scraper` crate）。 |
| **流式 / 不同响应体** | LLM 4 源 | 先做**非流式**（一次性取 `choices[0].message.content`），流式列入 YAGNI（除非你要）。 |
| **结果形态非「单段译文」** | 词典 4 源 | `TranslateResponse` 从「单一 translated 字符串」扩展为枚举：`Plain(String)` \| `Dict(DictEntry{音标,释义[],例句[]...})`；前端按类型分别渲染。 |
| **签名多样** | 百度/腾讯/阿里/火山/有道/ChatGLM | 各自实现签名（MD5 / HMAC-SHA1 / TC3 / SigV4 / SHA256 / JWT），复用现成 crate（已依赖 `md5`；HMAC/SHA 用 `hmac`+`sha1`/`sha2`，JWT 手搓 HS256）。 |

> 这意味着 **V1 必须先做抽象重构**（含一个多步源 Bing 验证抽象够用），后续源才能平滑接入。重构要保证现有 4 源（迁移期）行为不回归——以现有 provider 测试为安全网。

---

## 五、分批 / 版本规划（goal-dev 版本边界）

> 每个版本结束由独立 `producer` 裁决；验收标准从本文机械派生，冻结后不改。

### V1 · 架构铺垫 + 免 key 机翻核心（含移除 MyMemory）
- provider trait 扩展（多步请求 / HttpExecutor 注入 / 结果枚举骨架）。
- 实现：**Lingva、Google、Bing、DeepL-free、Transmart、Yandex**（6 个免 key 机翻）。
- 移除 MyMemory；默认源切到决策 1 选定者；`settings.json` 旧 `selected_provider="mymemory"` 迁移到新默认。
- 非官方源失效降级提示 + UI 标注。
- 验收：每源对固定样例（如「冰川→glacier / glacier→冰川」）返回正确；默认源开箱可用；迁移正确；现有翻译/历史不回归。

### V2 · 需 key 机翻（9 源）
- 实现：百度、百度专业、腾讯 TMT、阿里、火山、彩云、小牛、官方 DeepL、有道。
- 各源凭据 schema + 签名实现 + 设置页凭据表单（沿用现有凭据 UI）。
- 未配置置灰；配置后端到端可译。
- 验收：每源签名正确（可用 mock/录制响应做单测，真实 key 走 manual_confirm）；凭据安全（不入库/日志）。

### V3 · LLM 对话翻译（4 源）
- 实现：OpenAI、ChatGLM、Gemini、Ollama（非流式）。
- 配置：base_url / key / model / 可编辑 Prompt（默认翻译 Prompt）。
- 验收：可配置并返回翻译；Ollama 本地连通；Prompt 变量替换正确。

### V4 · 词典（4 源）
- 实现：Bing 词典、剑桥、ECDICT、有道词典模式。
- `DictEntry` 结果类型 + 前端词典展示组件（音标/释义/例句/发音）。
- 验收：词条结构正确解析与展示；非词输入回退普通翻译或提示。

> V3 / V4 视决策 2 的勾选可整版砍掉。

---

## 六、迁移与兼容

- **设置迁移**：读取 `selected_provider`，若为 `mymemory`（或任何已删除 id）→ 回退到新默认源，写回 settings。
- **历史兼容**：`translate_history` 里旧的 `provider_id`（如 mymemory）只用于展示，不影响新翻译；展示未知 id 时降级显示原始字符串。
- **凭据兼容**：现有 baidu/deepl_free/google 凭据保留，对应到新清单（DeepL 旧 `deepl_free` → 新「DeepL·api 模式」；google 旧官方 key 源 → 保留为「Google 官方」或并入，待实现期定）。

---

## 七、风险与对策

| 风险 | 对策 |
|---|---|
| 非官方接口失效（Google/Bing/DeepL-free/Yandex/Transmart/Bing 词典/剑桥） | 多免 key 源互为兜底；失效给明确提示；默认源选最稳的 Lingva |
| 复杂签名实现出错（火山 SigV4 / 腾讯 TC3 / 阿里 HMAC） | 逐源以**厂商官方 API 文档**为权威参照 + 录制真实响应做解析单测；签名用成熟 crate |
| 第三方 ToS / 法务 | 非官方接口在文档与设置页如实标注「非官方、使用风险自负」 |
| **GPL 许可传染** | **绝不复制 pot（GPL-3.0）源代码**；独立重写、仅参照官方文档与公开协议事实（见〇章）。保护 QuickQuick 许可自由 |
| 抽象重构波及现有源 | V1 以现有 provider 测试为回归安全网；重构与新源分步提交 |
| 工作量大、跨会话 | goal-dev 分版本 + dev-log 留痕续跑；每版可独立交付价值 |

---

## 八、不做（YAGNI / 边界）

- 不做 pot 的 **插件系统**（`.potext`）、**OCR**、**截图翻译**、**外部调用**。
- LLM **流式输出**暂不做（一次性返回即可，除非你要）。
- 不做翻译源的**自动测速 / 自动切换**（pot 有，列为后续可选）。
- 不复刻 pot 的 UI，仅复刻**翻译源能力**，沿用 QuickQuick 现有翻译页交互。

---

## 九、确认结论（2026-06-06 冻结）

| # | 决策 | 结论 |
|---|---|---|
| 1 | 默认源 | **Lingva**（Google 并列免 key 备选/兜底） |
| 2 | 范围 | **全 21 源**，V1→V4 全做 |
| 3 | 非官方接口 | **接受纳入**，UI 加「⚠ 非官方可能失效」标注 + 失败降级提示 |
| 4 | 实现方式 | **独立重写、不抄 pot 代码**（见〇章）：需 key 源照厂商官方文档、免 key 源按公开协议事实，注释官方文档 URL；pot 仅作功能清单参考 |

本文已冻结，按 V1→V4 走 goal-dev 分批实现。

---

*QuickQuick 翻译源对齐 pot 设计方案 · 2026-06-06 · 已冻结*
