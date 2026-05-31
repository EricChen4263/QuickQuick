---
id: V2-F2-S06-review
type: review
level: 小功能
parent: V2-F2
children: []
created: 2026-05-31T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F2-A09]
evidence: []
author: code-reviewer
---

# 代码审查报告 · V2-F2-S06 MyMemory Provider 适配

## 审查范围
- `src-tauri/src/translate/providers.rs`（MyMemory 完整实现 + map_mymemory_error + percent_encode*）+ `tests/providers.rs`（A09 8 用例）
依据：code-standards + 设计§4.1 薄 provider + §4.2 MyMemory。

## Critical
无。

## Important
**[I-3] responseStatus 字符串类型静默降级为 200，错误响应误判为成功（功能 bug，置信度 80）**
- 位置：`providers.rs`（`v["responseStatus"].as_u64().unwrap_or(200)`）。MyMemory 部分错误响应 responseStatus 为字符串（"403"），as_u64→None→默认 200→进成功路径，若 translatedText 存在错误占位则把错误当译文返回。
- 修复：解析 Number 或 String 两种形态（`s.parse::<u64>()`）；无法解析时归 ParseError，不默认 200。

**[I-2] A09-6 测试断言过宽 `Quota|Auth` 失去证伪能力（测试质量，置信度 85）**
- 位置：`tests/providers.rs`（403+"FREE TRANSLATIONS" 应精确 Quota，但断言接受 Auth）。若实现错归 Auth 测试仍过。
- 修复：断言只接受 `TranslateError::Quota(_)`。

**[I-1] 自造 percent-encoding 容量低估 + 维护风险（非阻塞建议，置信度 82）**
- 位置：`providers.rs`（`String::with_capacity(s.len())` 对多字节 UTF-8 低估致多次重分配；功能正确但自造轮子）。
- 修复（择一）：引入 `percent_encoding` crate（已是间接依赖）替代；或最小修 `with_capacity(s.len()*3)` 避免重分配（保留 `|` 不编码）。

## 通过项
无裸 unwrap/panic（生产）；thiserror 规范；无装饰注释/TODO；capability id=mymemory/needs_key=false；| 不被编码（percent_encode_langpair b"|"）；薄 provider 三件不越界；parse 错误归一（429→RateLimit/5xx→ServerError/非法 JSON→ParseError）；测试 AAA、headless、A09-1/2/3/4/7/8 非恒真。

## 结论
**未过（打回）。** 修 I-3（responseStatus 字符串解析不误判）+ I-2（断言只 Quota）+ I-1（percent crate 或容量修）后复审。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-3 已解决**：responseStatus 改 match Number/String（`s.parse::<u64>()`），解析失败归 ParseError 不默认 200；新增 A09-9 字符串 "403"+quota→Quota 用例。
- **I-2 已解决**：quota_exceeded 断言收窄为仅 `TranslateError::Quota(_)`。
- **I-1 已解决**：percent_encode 容量改 `s.len()*3`，| 不编码逻辑不变。
MyMemory 三件契约未破坏；无新引入≥80 高危；providers 9 + 全量 94 绿。
