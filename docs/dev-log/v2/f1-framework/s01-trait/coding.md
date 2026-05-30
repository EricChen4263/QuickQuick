---
id: V2-F1-S01-code
type: coding_record
level: 小功能
parent: V2-F1
children: []
created: 2026-05-30T23:51:49Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A01, V2-F1-A08]
evidence:
  - src-tauri/src/translate/mod.rs
  - src-tauri/src/translate/providers.rs
  - src-tauri/tests/translate.rs
author: coder
---

# 编码记录 · S01 翻译 provider 可插拔框架骨架

## 做了什么

实现薄 provider 契约（`TranslateProvider` trait，三件职责）与编译期静态注册表（`registry()` 返回 4 家 provider 能力声明），满足 V2-F1-A01 和 V2-F1-A08 验收项。

## 薄 provider 三件契约

`TranslateProvider` trait 恰好暴露三个方法，不多不少：

| 方法 | 职责 |
|------|------|
| `capability(&self) -> ProviderCapability` | 声明 provider 元数据（id/name/needs_key） |
| `build_request(&self, req: &TranslateRequest) -> ProviderHttpRequest` | 将统一请求转为该 provider 的 HTTP 调用描述（不发网络） |
| `parse_response(&self, raw: &str) -> Result<TranslateResponse, TranslateError>` | 将原始响应/错误解析回统一结果 |

## 横切关注点下沉说明

以下能力**不在 trait 上**，由核心框架后续小功能横切实现：

- **缓存**（s05）：键 = `(text, source_lang, target_lang, provider_id)`，落 DB + LRU 淘汰
- **限流与重试**（s03）：同源瞬时错误退避重试，跨源切换显式
- **凭据读取**（s04）：provider 声明字段 schema，secret 进 keychain
- **超时与取消**（s03）：框架统一执行 HTTP、管理超时，连续触发只认最新

这样 provider 实现者只需关心「怎么构造请求」和「怎么解析响应」，测试时无需 mock 框架层。

## 静态注册表 4 家

`registry()` 编译期列出，无运行时反射：

| id | name | needs_key |
|----|------|-----------|
| `mymemory` | MyMemory | `false`（默认源） |
| `baidu` | 百度翻译 | `true` |
| `deepl_free` | DeepL Free | `true` |
| `google` | Google 翻译 | `true` |

## 关键决策与理由

- **`ProviderHttpRequest` 描述符模式而非直接发网络**：provider 只返回请求描述结构体，框架统一执行 HTTP。这样超时/取消/重试可集中在框架层，provider 测试 100% headless，无需 mock HTTP 客户端。
- **`Lang` 薄包装而非 enum**：s02 细化前用 `String` 持有 BCP-47 串，避免过早设计 enum 变体。s02 再做归一验证与 provider 映射表。
- **`TranslateError` 占位三变体**：`ParseError / NetworkError / ProviderError` 够用于 s01 测试；s03 细化为 quota/auth/ratelimit/unsupported/tooLong/serverError 等，不破坏 s01 契约。
- **`providers.rs` 中 build_request/parse_response 为 s06/s07 占位**：真实签名构造（百度 MD5 签名、DeepL Auth-Key header 等）在对应小功能实现，s01 只需能编译且 trait 契约可断言。

## 改动文件

- `src-tauri/src/translate/mod.rs` — 新增：核心类型（Lang/TranslateRequest/TranslateResponse/ProviderCapability/ProviderHttpRequest/TranslateError）+ TranslateProvider trait
- `src-tauri/src/translate/providers.rs` — 新增：registry() 静态注册表 + 4 家 provider stub 实现
- `src-tauri/src/lib.rs` — 追加 `pub mod translate;` 注册模块
- `src-tauri/tests/translate.rs` — 新增：12 个集成测试覆盖 A01/A08

## 自测结论（TDD 红-绿-重构）

**RED**：先写 `tests/translate.rs`，引用 `quickquick_lib::translate` 模块，`cargo test` 报 `unresolved import`——确认是模块缺失而非语法错误。

**GREEN**：创建 `translate/mod.rs`（类型 + trait）和 `translate/providers.rs`（registry + 4 家 stub），在 `lib.rs` 注册模块。首次 clippy 发现 `providers.rs` 中 `Lang` 未使用，移除后 clippy 零警告。12 个测试全绿。

**REFACTOR**：代码已足够简洁（每个方法 < 20 行、嵌套 ≤ 2 层），无需额外重构。

**code-standards 自检：**

- 格式：4 空格缩进（Rust 官方），行宽 < 100 字符，文件末尾换行
- 函数：单一职责，每个实现方法 ≤ 15 行，嵌套 ≤ 2 层
- 命名：描述性（`build_request`/`parse_response`/`registry`），`needs_key` 用 `is`-前缀语义布尔字段
- 注释：trait 文档说明「为什么不在 trait 上」，无装饰性分隔符（无 ───/═══/━━━）
- 类型：公共 API 全部显式类型，无裸 `unwrap`，thiserror 枚举错误
- 测试：AAA 结构，行为化命名（如 `provider_contract_parse_response_returns_error_on_invalid_json`），headless 无网络
- 安全：无密钥入库，无 panic，错误用 `?` 传播
- 无 TODO/FIXME 遗留
