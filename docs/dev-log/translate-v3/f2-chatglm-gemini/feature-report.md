---
id: TV3-F2-report
type: feature_report
level: 大功能
parent: TV3
children: [TV3-F2-S01-code, TV3-F2-S01-test, TV3-F2-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: 9f92e25
acceptance_ids: [TV3-F2-A01]
---

# TV3-F2 大功能验收报告：ChatGLM JWT HS256 + Gemini URL参key

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV3-F2-S01 | ChatGlmProvider（智谱 open.bigmodel.cn，手搓 JWT HS256、Authorization: Bearer，复用 build_chat_body）+ GeminiProvider（generativelanguage，key 作 URL ?key=、contents/parts 异构 body）+ base64url_no_pad + schema | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV3-F2-A01 | **pass** | chatglm JWT HS256 签名确定性（独立 Python 复算锚定具体 token）+ chatglm/gemini build+parse 命中 + 错误分支 + 变异 A–F 红 |

## 门禁（两次打回均闭环）
- **打回 #1（tester，编译 bug）**：GeminiProvider URL format! 漏占位符（2 占位 3 参，exit 101）+ URL 漏 ?key=。coder 修复格式串 + 补 URL 含 key 断言守卫。
- **打回 #2（reviewer，Critical C1）**：ChatGLM Authorization 缺 Bearer 前缀（智谱官方文档要求，真请求会 401）。coder 修复 `format!("Bearer {}")` + 补 starts_with("Bearer ") 断言。
- 终态：tester 动态证伪通过（4 冻结命中 + 变异 A–F 全红含 E 修复回归守卫 + 6 边界 panic 安全 + sentinel 不泄密含 Gemini URL key 不泄露 + JWT 三段式 + debug×3/release 241 passed + clippy 0），code-reviewer APPROVE（JWT 正确性 Python 复算一致、未抄 pot、Bearer 闭合、registry 17→19 同步无回归）。

## 关键决策
- ChatGLM 复用 render_prompt + build_chat_body（OpenAI 兼容），仅鉴权（JWT）与端点不同。
- JWT exp/timestamp 做成入参 → 签名确定可锚定；请求路径才注入真实时间（TTL 1h）。
- Gemini body 异构：render_prompt 得 messages 后 build_gemini_body 转 contents/parts（system→systemInstruction）。
- key 在 URL query 用 percent_encode，错误消息不回显含 key 的 URL。

## 结论：**通过**（A01 objective pass；真网真译 manual 待 TV3-M01 采证）。
