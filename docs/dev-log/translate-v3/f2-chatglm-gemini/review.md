---
id: TV3-F2-S01-review
type: review_report
level: 小功能
parent: TV3-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV3-F2-A01]
author: code-reviewer
---

# TV3-F2-S01 审查报告：ChatGLM JWT HS256 + Gemini URL参key

## 最终审查结论：APPROVE（通过）

复审确认 Critical C1（ChatGLM Authorization 缺 Bearer 前缀）已闭合，无其余 Critical 或 Important 问题。

---

## 〇、复审结论（2026-06-06）

**C1 已闭合**，改判 APPROVE。

**复审核查点**（只读 diff 确认）：

- `providers.rs:2572`：`format!("Bearer {}", self.authorization())` ——Bearer 前缀正确，与 OpenAI provider 行 2311 `format!("Bearer {}", self.api_key)` 风格完全一致。
- `providers.rs:4857-4861`：`chatglm_build_request_and_parse` 补入 `assert!(auth.starts_with("Bearer "), …)` 独立守卫——位于 `matches('.').count() == 2` 断言之前，优先捕获前缀缺失。
- tester 复验：改回裸 token 变异 → chatglm_build_request_and_parse 如期红，证明补强断言有判别力；chatglm_jwt_hs256_deterministic 测纯函数裸 token 不受 Bearer 前缀影响仍绿；debug×3 + release 均 241 passed；clippy exit 0。

C1 之外的所有通过项（JWT 正确性、安全、Gemini、credential、未抄 pot、代码规范）均为纯只读代码，本次修复未触及，结论不变。

---

## 一、Critical 发现（首轮打回，已闭合）

### C1 · ChatGLM Authorization 头缺 `Bearer ` 前缀（置信度 95）【已修复闭合】

**文件**：`src-tauri/src/translate/providers.rs`（首轮位于行 2569，修复后挪至 2572）

**首轮问题**：`ChatGlmProvider::build_request` 把 `self.authorization()` 的返回值直接用作 `Authorization` 头值。`authorization()` 返回裸 JWT token（如 `eyJhbGciOiJIUzI1NiIsInNpZ25fdHlwZSI6IlNJR04ifQ.xxx.yyy`），实际发出的头为：

```
Authorization: eyJhbGciOiJIUzI1NiIsInNpZ25fdHlwZSI6IlNJR04ifQ.xxx.yyy
```

**智谱官方文档（docs.bigmodel.cn）明确要求**：

```
Authorization: Bearer <your_token>
```

无论是简单 API Key 模式还是 JWT Token 模式，均须加 `Bearer ` 前缀。所有真实 ChatGLM 请求因此会返回 401 鉴权失败。

**与项目 OpenAI provider 对比**（行 2310-2312，同文件同一 LLM 分类下的参照实现）：

```rust
// OpenAI（正确）
format!("Bearer {}", self.api_key)

// ChatGLM 修复前（错误）
self.authorization()  // 裸 JWT，无 Bearer 前缀
```

**测试盲点**：`chatglm_build_request_and_parse` 仅断言 `auth.matches('.').count() == 2`——裸 JWT `xxx.yyy.zzz` 含 2 个点通过，`Bearer xxx.yyy.zzz` 也含 2 个点，两种情况均通过，测试对此差异是盲的。

**修复内容（已核实）**：

```rust
// providers.rs:2572（修复后）
("Authorization".to_string(), format!("Bearer {}", self.authorization())),
```

```rust
// providers.rs:4858-4861（补强断言，已核实）
assert!(
    auth.starts_with("Bearer "),
    "Authorization 应带 Bearer 前缀，实际：{auth}"
);
```

**闭合状态**：复审已通过，C1 闭合。

---

## 二、Important 发现

（无。）

---

## 三、设计符合性核查

| 检查项 | 结论 |
|---|---|
| ChatGLM 端点（docs.bigmodel.cn v4/chat/completions） | 正确：`https://open.bigmodel.cn/api/paas/v4/chat/completions` |
| ChatGLM 鉴权 JWT HS256（id.secret 拆分、手搓 header/payload/sig） | 正确；Authorization 头 Bearer 前缀已修复（C1 已闭合） |
| ChatGLM body OpenAI 兼容（model/messages/stream=false） | 正确：复用 `build_chat_body + render_prompt` |
| ChatGLM 响应取 choices[0].message.content | 正确 |
| Gemini 端点（generativelanguage.googleapis.com v1beta ...generateContent?key=） | 正确 |
| Gemini key 作 URL query 参（不进 Authorization 头） | 正确；key 经 `percent_encode` 防特殊字符 |
| Gemini body contents/parts（system→systemInstruction，user/assistant→contents） | 正确；assistant role 归一为 model |
| Gemini 响应取 candidates[0].content.parts[0].text | 正确 |
| 非流式（stream=false） | 正确：两者均非流式 |
| 独立重写，不抄 pot（§〇） | 已核查：注释均标注官方文档而非 pot URL，无 pot 代码/注释痕迹 |

---

## 四、安全核查

| 检查项 | 结论 |
|---|---|
| apiKey/secret is_secret=true 加密存储（credential.rs） | 正确：chatglm.apiKey / gemini.apiKey 均 is_secret=true |
| 无 eprintln/println/log/dbg 打印密钥 | 通过：全文 grep 零匹配 |
| 错误消息不含 apiKey（map_chatglm_error/map_gemini_error） | 正确：仅含 code/status/msg，不回显 apiKey |
| Gemini key 不进日志/错误消息（仅在 URL query） | 正确：URL 仅在 build_request 内构造并交给执行框架，错误体由 parse_response 解析 API 返回的 JSON，不回显 URL |
| JWT Authorization 不明文含 id/secret | 正确：id 经 base64url 编码后在 payload 段、secret 作 HMAC key 不出现在 token 中 |
| sentinel 脏值（SENTINEL_DEADBEEF / SENTINELID）测试 | 正确：断言 `!contains(sentinel)` 用非空脏值，符合 hints TV2-RETRO-1 |

---

## 五、JWT 正确性核查

经独立 Python（hmac+hashlib+base64）复算验证：

- **输入**：id=`test_id_12345`、secret=`test_secret_67890`、exp=`1717632000000`、timestamp=`1717631700000`
- **header JSON**（BTreeMap 字母序）：`{"alg":"HS256","sign_type":"SIGN"}`（字母序 alg < sign_type，与字面顺序一致）
- **payload JSON**（BTreeMap 字母序）：`{"api_key":"...","exp":...,"timestamp":...}`（字母序 api_key < exp < timestamp，与字面顺序一致）
- **Python 复算结果 = 代码中参照常量**：完全吻合（Match: True）
- **base64url 无填充**：正确（`URL_SAFE_NO_PAD` 引擎）
- **HMAC-SHA256 签名**：正确（复用既有 `hmac_sha256` 函数）
- **exp 单位毫秒**：正确（`now_ms + CHATGLM_JWT_TTL_MS`，TTL 3_600_000 ms = 1h）
- **反循环论证**：测试锚定独立 Python 复算的参照常量，非自产输出，符合 hints TV2 复杂签名独立复算要求

---

## 六、既有不回归核查

| 检查项 | 结论 |
|---|---|
| 注册表 17→19 与 tests/translate.rs 断言同步 | 正确：函数名、注释列举、断言值均同步更新 |
| chatglm/gemini needs_key=true | 正确：不会进入免 key 路径 |
| 既有 17 源行为未触动 | 正确：新增均为纯追加，build_provider match 分支隔离 |

---

## 七、代码规范核查

| 检查项 | 结论 |
|---|---|
| 函数 ≤ 50 行 | 通过：新增最长函数 build_gemini_body 24 行、parse_response(Gemini) 26 行 |
| 公共 API 有文档注释 | 通过：struct/impl/pub fn 均有 doc 注释标注端点/算法/参照文档 |
| 注释写「为什么」 | 通过：选择注释解释了时间注入的原因、fallback 逻辑的动机 |
| 无装饰性分隔注释 | 通过 |
| 无 TODO/FIXME | 通过（tester 已确认） |
| 无 pot 代码/注释痕迹 | 通过：注释标注官方 API 文档 URL，无 pot URL |
| 常量无魔术值 | 通过：`CHATGLM_JWT_TTL_MS`、`CHATGLM_ENDPOINT`、`GEMINI_BASE` 均具名常量并有注释 |
