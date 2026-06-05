---
id: TV1-F1-S01-review
type: review
level: 小功能
parent: TV1-F1
children: []
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F1-A01, TV1-F1-A02, TV1-F1-A03]
evidence: []
author: code-reviewer
---

# 审查结论 · Lingva 默认源替代 MyMemory + 设置迁移（TV1-F1-S01）

## 审查维度

按 code-standards（格式/命名/函数/注释/类型/安全）+ 项目规范逐项核对。审查范围：
- `src-tauri/src/translate/providers.rs`（LingvaProvider 新增，MyMemory 移除）
- `src-tauri/src/translate/lang.rs`（`map_for_lingva`）
- `src-tauri/src/translate/credential.rs`（mymemory email schema 分支移除）
- `src-tauri/src/ipc/translate.rs`（`resolve_provider_or_fallback` + `DEFAULT_PROVIDER_ID`）
- `src-tauri/src/ipc/settings.rs`（`get_selected_provider_impl` 迁移回退）
- `src-tauri/src/settings.rs`（默认源 mymemory→lingva）
- 对应测试文件

## 发现问题（置信度 ≥ 80 才报）

### Important 级

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| Important | `tests/ipc_settings.rs` 文件头模块注释第 7 行仍写 `含 "mymemory"`，与该文件现有测试体（第 178–203 行已正确断言不含 mymemory 且含 lingva）直接矛盾，是遗留的死注释。 | `src-tauri/tests/ipc_settings.rs:7` | code-standards §5「注释不应误导」；将 `- get_translate_providers 返回非空且含 "mymemory"` 改为 `- get_translate_providers 返回非空且含 "lingva"、不含已移除的 "mymemory"` |
| Important | `tests/translate.rs` 中第 1335 行（`translate_clip_item` 测试）和第 1368 行（`translate_history_separate_add_and_retrieve` 测试）把 `"mymemory"` 作为 `provider_id` 字面量传入。这两个测试本身测的是缓存/历史数据层（不依赖注册表），使用已移除的 id 作为数据不影响正确性，但注释在这类测试中写 `"mymemory"` 会让后续维护者困惑「为什么还有 mymemory」。coding.md 明确声明这两个测试已同步（`registry/needs_key/lang_norm/credential_schema 的 mymemory 断言改 lingva；retry 测试标签改 lingva`），但第 1335、1368 行确实未同步（检查 git diff 可证），属于遗漏同步点。 | `src-tauri/tests/translate.rs:1335,1368` | 清理规范：缓存/历史层测试中 `provider_id` 字面量改为 `"lingva"`（或任意存在于注册表的 id），或在测试注释中加说明「此处 provider_id 为历史数据字符串，与注册表无关」，消除歧义。 |

### 不报项（置信度 < 80 或为预存在问题）

- **`data.translation` vs `translation` 字段路径**：设计文档 §二.2.1 表格写 `data.translation`，但实现读 `v["translation"]`（顶层字段）。经实测验证（WebFetch 确认 Lingva API 返回 `{"translation":"..."}` 顶层字段）+ coding.md 关键决策明确记录「按实测 ground truth」+ 验收测试 TV1-F1-A02 用 `{"translation":"glacier"}` 通过 + tester 变异 A 改 `v["text"]` 如期变红：实现正确，**设计文档表格是笔误**，不是代码 bug。置信度不达阈值，不升级为 Critical。
- **读路径写回副作用（磁盘写）**：`get_selected_provider_impl` 在读路径检测到旧值时写回。这是 coder 的有意设计决策，代码注释已说明「仅当发生回退才写回，避免每次读取触发磁盘写」，且在 `translate_text_impl` 调用链中每次翻译只触发一次文件读（非并发），单文件写无锁问题。此实现属于「一次性自愈」而非重复副作用，不报。
- **`resolve_provider_or_fallback` 每次调用 `registry()`**：`registry()` 返回编译期静态 Vec，无 I/O，无缓存必要性。不是性能问题。
- **`tests/translate.rs` 第 975、1026、1049、1074、1106 行缓存测试用 `"mymemory"`**：这些测试目的是验证 cache_key 哈希不变性（换 provider_id 字符串必 miss），`"mymemory"` 在此是纯字符串 fixture（不依赖注册表），使用完全合法，属预存在测试，不在本小功能范围内。
- **许可合规**：代码注释标注「Lingva 是开源 Google 翻译前端，提供无认证纯 GET HTTP 接口」，未写 pot 源码 URL，实现为原创 Rust，符合 §〇 独立重写要求。
- **安全（TV1-A-SEC）**：`providers.rs` 无 `eprintln`（grep 确认），`LingvaProvider.build_provider` 不读凭据（`needs_key=false`，`build_provider("lingva", &[])` 直接 `Ok`），错误处理未打印 `req.text`/译文。
- **函数长度**：`LingvaProvider` 三个 trait 方法（`capability` 7行、`build_request` 17行、`parse_response` 9行）均远低于 50 行上限。`resolve_provider_or_fallback` 7行，合规。
- **无 TODO/FIXME、无装饰性分隔注释**：grep 确认改动文件无命中。
- **MyMemory 移除干净度**：`NeedEmail` 枚举变体、`map_mymemory_error`、`percent_encode_langpair`、`credential_schema` 的 mymemory 分支均已移除，无悬空引用，`cargo test + clippy` 全绿可证。
- **baidu/deepl_free/google 未越界改动**：经逐行审查，三源实现逻辑无变化，仅其所在文件的 mymemory 旁支被清理。

## 是否合规

符合项目规范与 code-standards 的绝大多数条目：

- 格式：4 空格缩进（Rust 官方），无装饰性分隔符，文件末尾换行完整。
- 函数：单一职责，≤50 行，嵌套 ≤3 层，Early Return 降嵌套。
- 命名：`resolve_provider_or_fallback`（动词+名词）、`DEFAULT_PROVIDER_ID`（常量 UPPER_SNAKE）、`map_for_lingva` 等符合规范。
- 注释：解释「为什么」（为何按注册表判定而非硬编码 mymemory、为何只在回退时写回），无死代码注释。
- 类型：`pub const DEFAULT_PROVIDER_ID: &str`（显式类型），`UserPromptKind` 枚举（无魔术字符串）。
- 安全：免 key 源不读凭据，日志无敏感信息。
- 测试：AAA 结构，测试名描述行为（`providers_registry_has_lingva_no_mymemory`、`selected_provider_migrates_unknown_to_lingva`）。
- 许可合规：独立实现，注释标来源，未抄 pot 代码。

两个 Important 问题（`tests/ipc_settings.rs` 头注释遗留 + `tests/translate.rs` 两处测试未更新 provider_id）是非阻塞的注释/测试清洁度问题，不影响运行时正确性，不影响验收测试通过。

## 结论

**通过（附非阻塞建议）**

三个验收项（TV1-F1-A01/A02/A03）逻辑实现正确，许可红线无违反，安全要求满足，MyMemory 移除干净，迁移逻辑（「读路径检测到旧值才写回」）设计合理且稳健。

非阻塞建议（可在后续清洁 commit 处理，不阻塞本小功能放行）：
1. `src-tauri/tests/ipc_settings.rs:7` — 头注释第 7 行 `"含 mymemory"` 改为 `"含 lingva 不含 mymemory"`。
2. `src-tauri/tests/translate.rs:1335,1368` — 两处历史层测试的 `provider_id` 字面量 `"mymemory"` 改为 `"lingva"` 或加注释说明属数据 fixture，消除维护歧义。
