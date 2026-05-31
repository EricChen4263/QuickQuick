---
id: V2-F3-S09-code
type: coding_record
level: 小功能
parent: V2-F3
children: []
created: 2026-05-31T01:39:48Z
status: 通过
commit: WIP
acceptance_ids: [V2-F3-A13, V2-F3-A14, V2-F3-A15]
evidence:
  - src-tauri/src/translate/lang.rs
  - src-tauri/src/translate/history.rs
  - src-tauri/src/translate/mod.rs
  - src-tauri/src/db.rs
  - src-tauri/tests/translate.rs
  - src-tauri/tests/schema.rs
  - src/translate/translate-actions.ts
  - src/translate/translate-actions.test.ts
author: coder
---

# 编码记录 · 翻译面板逻辑（S09）

## 做了什么

实现翻译面板核心逻辑三项：A13 智能双向方向复用 `resolve_direction` 完成源语自动检测与方向决策；A14 建立独立 `translate_history` 表并通过 `translate_clip_item` 支持剪贴板条目一键翻译写入历史；A15 以纯函数 `TranslateAction` 类型与 `resolveTranslateAction` 定义译文操作集（copy / speak / switch_target / switch_source_retranslate / save_history）。

## 关键决策与理由

- **A13 复用 `resolve_direction`，不另造检测函数**：`resolve_direction` 在 `lang.rs` 中已对 `detect_lang` 进行了单次扫描且支持 `configured_target` 覆盖，A13 的验收场景（自动双向 + 用户配置目标覆盖）与该函数语义完全吻合，直接在集成测试中针对性补写 4 条专属用例，避免重复逻辑。

- **A14 翻译历史与剪贴板分表**：`translate_history` 作为独立表写入 `db.rs` 的 `ensure_schema`，与 `clip_items` 严格隔离；`translate_clip_item` 读 `clip_items` 原文后写入 `translate_history`，两表互不混入，满足独立存储语义，也避免翻译记录污染剪贴板历史的 GC / 软删逻辑。

- **A14 `translate_clip_item` 不直接接收原文参数**：原文从 `clip_items` 表内部读取（带 `is_deleted=0` 过滤），保证一键翻译操作的数据来源可信，防止调用方传入与剪贴板实际内容不一致的原文。

- **A15 操作集纯函数建模**：`TranslateAction` 用 TS 联合类型定义，`resolveTranslateAction` 不抛异常、非法命令返回 `null`，`availableActions` 返回有序列表兼顾 UI 渲染与完整性断言。纯函数无副作用，无需 mock，测试直接断言返回值。

- **A15 不抛异常原则**：UI 层的命令字符串来源复杂（事件系统、外部注入），采用返回 `null` 而非 `throw` 的防御设计，调用方可按需处理未知操作，不会因拼写错误崩溃。

## 改动文件

- `src-tauri/src/translate/lang.rs` — 已有 `resolve_direction` / `detect_lang`，A13 无需新增实现，仅在集成测试中补写专属用例；`#[cfg(test)]` 单元测试覆盖 `normalize_zh_variant` / `is_cjk_char`
- `src-tauri/src/translate/history.rs` — 新增文件（A14）：实现 `add_translate_history`、`translate_clip_item`、`translate_history_count` 三个函数，全部使用参数化查询
- `src-tauri/src/db.rs` — 在 `ensure_schema` 中新增 `translate_history` 建表 DDL（A14），7 列：`id / source_text / translated_text / source_lang / target_lang / provider_id / created_utc`
- `src-tauri/tests/translate.rs` — 新增 A13 专属集成测试 4 条（`smart_direction_*`）、A14 专属集成测试 2 条（`translate_history_separate_*`）
- `src-tauri/tests/schema.rs` — 新增 `schema_preembed_translate_history_table_exists_with_required_columns` 1 条，验证 `translate_history` 7 列预埋（计入 schema 10 回归）
- `src/translate/translate-actions.ts` — 新增文件（A15）：定义 `TranslateAction` 联合类型、`availableActions`、`resolveTranslateAction` 三个纯函数导出
- `src/translate/translate-actions.test.ts` — 新增文件（A15）：对 `availableActions` 和 `resolveTranslateAction` 共 8 条测试，覆盖 5 个合法操作 + 非法命令 null + 空字符串 null

## 自测结论（TDD 红-绿-重构）

**TDD 循环**

- A13：先在 `translate.rs` 中写 4 条 `smart_direction_*` 失败测试（`resolve_direction` 存在但无专属断言），确认测试因验收语义不足而失败；`resolve_direction` 已有完整实现，补充后测试立即绿；重构无需改动（函数已符合单职责）。
- A14：先写 `translate_history_separate_*` 2 条失败测试（`translate_history` 表不存在，`history.rs` 不存在），确认因缺少实现而失败；依次实现 `ensure_schema` DDL → `history.rs` 三函数，测试转绿；重构提取 `current_utc_ms` 辅助函数消除重复的 `SystemTime` 调用。
- A15：先写 `translate-actions.test.ts` 8 条失败测试（模块不存在），确认 import 失败；实现 `translate-actions.ts` 三导出后测试全绿；重构用 `Set` 替代线性遍历提升 `resolveTranslateAction` 可扩展性。

**测试通过数量**

| 验收项 | 测试数 | 结果 |
|--------|--------|------|
| A13 智能双向方向 | 4 | 全部通过 |
| A14 翻译历史独立表 | 2 | 全部通过 |
| A15 译文操作集 | 8 | 全部通过 |
| schema 回归（含 translate_history） | 10 | 全部通过 |

**code-standards 自检**

- 格式/命名：Rust `snake_case`，TS `camelCase`，类型 `PascalCase`，布尔无 `is_` 前缀误用，函数均为「动词+名词」形式
- 函数长度：所有函数均不超过 30 行，远低于 50 行上限；嵌套不超过 2 层
- 单一职责：`history.rs` 三函数各自单一职责；`translate-actions.ts` 仅做命令映射
- 注释：均写「为什么」（如参数化查询防注入、`configured_target` 覆盖语义），无装饰性分隔符，无注释掉的死代码
- 类型安全：Rust 全部显式类型；TS 使用 `TranslateAction` 联合类型，无 `any`
- SQL 安全：全部使用 `rusqlite::params![]` 参数化查询，无字符串拼接 SQL
- clippy：0 警告（`cargo clippy` 通过）
- TODO/FIXME：无遗留
- 不可变优先：`translate-actions.ts` 使用 `new Set` 不原地 mutate；`availableActions` 每次返回新数组
- 性能：`resolveTranslateAction` 用 `Set.has` O(1) 查找，避免 `Array.includes` 线性扫描
