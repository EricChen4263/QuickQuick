---
id: V2-F2-S06-code
type: coding_record
level: 小功能
parent: V2-F2
children: []
created: 2026-05-31T00:57:59Z
status: 通过
commit: WIP
acceptance_ids: [V2-F2-A09]
evidence:
  - src-tauri/src/translate/providers.rs
  - src-tauri/tests/providers.rs
author: coder
---

# 编码记录 · MyMemory 适配（V2-F2-S06）

## 做了什么

实现 `MyMemoryProvider`，使其满足 V2-F2-A09 验收项：声明 `capability`（`id="mymemory"`、`needs_key=false`、可选 `email` 字段用于提升配额）；`build_request` 以 GET 方式构造 `https://api.mymemory.translated.net/get?q=<text>&langpair=<src|tgt>[&de=<email>]`，全程 RFC 3986 percent-encoding；`parse_response` 从 `responseData.translatedText` 提取结果，非 200 的 `responseStatus` 经 `map_mymemory_error` 归一为 `TranslateError` 变体（403+quota 文案→`Quota`，403 其他→`Auth`，429→`RateLimit`，5xx→`ServerError`，非法 JSON→`ParseError`）。

## 关键决策与理由

- **薄 provider 三件套（capability / build_request / parse_response）**：不在 provider 内执行 HTTP，而是只负责构造请求、解析响应，HTTP 层由外部统一调度。理由：保持各 provider 可纯单元测试，无需 mock 网络。
- **`percent_encode` / `percent_encode_langpair` 分离**：langpair 中的 `|` 分隔符是 MyMemory API 的语义字符，不得编码为 `%7C`；通过 `percent_encode_with_extra(s, b"|")` 单独保留，而普通参数值走严格编码的 `percent_encode`。否则 API 无法识别语言对。
- **`map_mymemory_error` 独立私有函数**：403 需按 `responseDetails` 文案区分 Quota/Auth，逻辑稍复杂，抽出独立函数保持 `parse_response` 主路径线性可读，也复用了与其他 provider 相同的错误归一模式。

## 改动文件

- `src-tauri/src/translate/providers.rs` — 新增 `MyMemoryProvider` struct 及 `TranslateProvider` 实现、`map_mymemory_error`、`percent_encode_with_extra` / `percent_encode` / `percent_encode_langpair` / `hex_upper` 工具函数
- `src-tauri/tests/providers.rs` — 新增 8 个针对 V2-F2-A09 的集成测试（A09-1 ~ A09-8），覆盖 capability、URL 编码、email 可选、成功解析、Quota/RateLimit/ParseError 路径

## 自测结论（TDD 红-绿-重构）

- TDD 过程：先按 A09 验收项逐一写失败测试（红），每条测试确认因功能未实现而失败后，再写刚好令其通过的最小实现（绿），最后抽出 `percent_encode_with_extra` 消除重复（重构），全程测试保持绿。
- code-standards 符合情况：
  - 格式：`cargo fmt` 格式化，无多余空行
  - 命名：函数遵循动词+名词风格（`build_request`、`parse_response`、`map_mymemory_error`）
  - 函数长度：最长函数 `percent_encode_with_extra` 16 行，全部 ≤ 50 行
  - 嵌套深度：最深 3 层（`percent_encode_with_extra` 内 for+if），符合 ≤ 3 层
  - 注释：文档注释说明「为什么」（配额数值来源、`|` 不可编码的原因），无装饰性分隔注释，无死代码
  - 类型：全程显式类型，无 `unwrap` 安全盲区（仅在 `unwrap_or` / `ok_or_else` 降级处理）
  - 无 TODO / FIXME 遗留
  - clippy：0 warning
- 测试结果：`cargo test --test providers` 8 tests passed，0 failed

## 按审查修复（打回第 1 次，2026-05-31）

- **I-3 responseStatus 字符串解析不误判**：`parse_response` 中将 `v["responseStatus"].as_u64().unwrap_or(200)` 改为 `match` 兼容 `Number` 与 `String` 两种形态；字符串走 `s.parse::<u64>()`，无法解析时归 0，状态 0 返回 `ParseError("responseStatus missing or unparseable")`，不再默认 200 导致误判成功。
- **I-2 Quota 断言收窄**：`provider_mymemory_parse_response_quota_exceeded`（A09-6）的断言从 `Quota(_) | Auth(_)` 改为精确 `Quota(_)`，消除因误归 `Auth` 仍能通过的空洞。同时新增 A09-9 `provider_mymemory_parse_response_quota_status_as_string`：字符串 `"403"` + quota 文案 → 归 `Quota`，直接覆盖 I-3 的回归路径。
- **I-1 容量修**：`percent_encode_with_extra` 的 `String::with_capacity(s.len())` 改为 `s.len() * 3`，避免 UTF-8 多字节字符编码时多次重分配（每个字节最多展开为 3 个字符 `%XX`）。
- 回归结果：`cargo test --test providers` 9 passed（含新增 A09-9）；全量 94 tests passed；clippy 0 warning；无装饰注释；无 TODO。
