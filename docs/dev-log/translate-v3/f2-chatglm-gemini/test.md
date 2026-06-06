---
id: TV3-F2-S01-test
type: test_report
level: 小功能
parent: TV3-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV3-F2-A01]
---

# TV3-F2 测试报告（动态证伪）· ChatGLM JWT HS256 + Gemini URL参key

> tester 动态证伪。共两次打回（编译 bug + 缺 Bearer 前缀），均被硬门禁抓出、coder 修复后复验通过。tester 无 Write，编排器据其结论落盘。变异经 cp 备份还原（MD5 校验，禁 git checkout）。

## 〇、首轮打回（已闭环）
首轮 tester 命中校验即发现工作树**编译失败**：`GeminiProvider::build_request` 的 `format!` URL 串 `"{}/v1beta/models/{}:generateContent"` 仅 2 占位符却传 3 参数（`error: argument never used`，exit 101），且 URL 漏带 `?key=`（正确性 bug）。coder 自报 487 passed 与实树不符（polish 后只 cargo check 未实跑全量 test）。打回 #1 → coder 修复格式串为 `?key={}` 三占位符 + 补 URL 含 key 断言守卫 + 实跑全量测试。

## 〇.2、第二次打回（reviewer Critical，已闭环）
tester 首轮复验放行后，code-reviewer 静态审查抓到 Critical：`ChatGlmProvider::build_request`（providers.rs:2569）Authorization 头直接用裸 JWT，**缺 `Bearer ` 前缀**（智谱官方文档要求 `Bearer <token>`，已引证），真实请求会 401；测试盲点 `auth.matches('.').count()==2` 对裸 JWT 与 `Bearer xxx.yyy.zzz` 都通过抓不到。打回 #2 → coder 修复为 `format!("Bearer {}", self.authorization())`（行 2572）+ 补 `auth.starts_with("Bearer ")` 断言（行 4859）。
tester 二次复验（聚焦）：回归守卫变异（改回裸 token）→ chatglm_build_request_and_parse 如期红，证明新断言有判别力；chatglm_jwt_hs256_deterministic 测的是 chatglm_jwt() 纯函数裸 token、不受 Bearer 影响仍绿；debug×3+release 241 passed；clippy 0；工作区一致。

## 一、命中校验（RTK 完整路径取原始输出，防假绿）
4 冻结测试真命中（各 1 passed）：`chatglm_jwt_hs256_deterministic`、`chatglm_build_request_and_parse`、`gemini_build_request_url_key_and_parse`、`gemini_parse_error_response`。
注：测试嵌套于 `translate::providers::tests::`，裸短串会 0 passed 假绿，已用完整模块路径确认。

## 二、变异 sanity（cp 备份还原，MD5 校验一致，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | chatglm_jwt 的 HMAC secret 改 `wrong_secret` | chatglm_jwt_hs256_deterministic | 如期红 |
| B | base64url_no_pad 改带填充 URL_SAFE | chatglm_jwt_hs256_deterministic | 如期红 |
| C | ChatGLM parse `message.content`→`message.text` | chatglm_build_request_and_parse | 如期红 |
| D | Gemini parse `parts[0].text`→`parts[0].output` | gemini_build_request_url_key_and_parse | 如期红 |
| E | Gemini URL 去掉 `?key={}`（回归修复前 bug） | gemini_build_request_url_key_and_parse | 如期红 |
| F | map_gemini_error `api key not valid`→ServerError | gemini_parse_error_response | 如期红 |

A–F 全红。**A/B 反循环论证**：测试锚定独立 Python 复算的参照 token 常量 `eyJhbGci...p-yF6cb9...`（非自产输出），改坏 HMAC 输入/填充后与常量失配。**E 修复回归守卫**：URL 去 `?key={}` 后双断言（全等 + `contains("?key=")`）均命中，证明新补守卫能拦此 format! 漏占位符类 bug 复发。

## 三、边界探测（6 临时用例，跑后 cp 还原 MD5 一致）
- **panic 安全**：ChatGLM/Gemini parse 喂非法 JSON / 缺字段 / 空 candidates / 缺 parts 均返 ParseError 不 panic。
- **错误分类**：Gemini UNAUTHENTICATED→Auth / RESOURCE_EXHAUSTED→RateLimit / 503→ServerError；ChatGLM 1002→Auth / 1302→RateLimit / 1112→Quota。
- **密钥不泄露**（hints TV2-RETRO-1）：sentinel `SENTINEL_DEADBEEF`/`SENTINELID` 不出现在任何错误消息或 Authorization 头；**Gemini key 在 URL query，错误消息不回显含 key 的 URL**；JWT Authorization 三段式不明文含 id/secret；grep providers.rs `eprintln|println|log::|dbg!` 零匹配。
- ChatGLM apiKey 无 `.` 分隔时不 panic，仍产出三段 JWT。

## 四、debug + release 双绿 + 抗 flaky
`cargo test` debug 连跑 3× 均 `241 passed; 0 failed`（每箱用例数逐轮一致，无 flaky）+ `cargo test --release` `241 passed`；`cargo clippy --all-targets -- -D warnings` exit 0、0 warning；无 TODO/FIXME。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致；变异 A–F + 6 边界用例每次 cp 还原后 MD5 均 `7decf6f6...`，全程无 git checkout，未留改动。

## 门禁结论：**通过（放行）**
