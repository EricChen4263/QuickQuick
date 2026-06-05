---
id: TV1-F4-S01-review
type: review
level: 小功能
parent: TV1-F4
children: []
created: 2026-06-05T19:03:09Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F4-A01]
evidence: []
author: code-reviewer
---

# 审查结论 · 非官方源 UI 标注 + 失败降级提示（TV1-F4-S01）

## 审查维度

依据项目规范（AGENTS.md / CLAUDE.md）+ code-standards skill，逐条对照：

- **格式**：Rust 4 空格缩进、TS/TSX 2 空格缩进，符合项目既有风格；行宽未见超限；文件末尾换行完整。
- **命名**：`is_unofficial`（Rust snake_case）→ `isUnofficial`（camelCase，`serde rename_all = "camelCase"`）→ TS `isUnofficial`，三层命名贯通一致；布尔用 `is` 前缀符合规范；常量 `UNOFFICIAL_DEGRADE_HINT` 全大写 snake，符合 TS 约定。
- **函数单一职责 / 长度**：`DirBar`、`TranslateWorkspace` 各自职责清晰；所有函数未见超 50 行。
- **注释**：写「为什么」而非「是什么」；第 65 行 `needs_key` 旧注释（MyMemory→lingva）已订正；无死代码注释；无装饰性横线分隔。
- **类型**：无 `any` 逃逸；`Provider` 接口新增 `isUnofficial: boolean` 显式声明；Rust DTO `is_unofficial: bool` 公共字段。
- **魔术串**：前端降级文案以 `UNOFFICIAL_DEGRADE_HINT` 具名常量承载，非魔术字符串；符合 code-standards §6。
- **零侵入设计**：DirBar `providerOptions` 仅在 `useMemo` 内部追加标注后缀，不新增 prop，无接口扩散。
- **测试**：AAA 结构完整；测试名描述行为（`nonofficial_source_label_and_degrade_hint: ...`）；命中测试（label 渲染 + 降级提示）、反面断言（官方源无提示）均覆盖。
- **DirBar.test.tsx 夹具改 isUnofficial=false 的合理性**：注释明确说明理由（「标注渲染由 label-degrade.test.tsx 独立覆盖」），隔离意图清晰，不属于削弱断言，而是职责分离，合规。
- **安全**：无密钥入库，无日志打印敏感信息；`uuid::Uuid::new_v4()` 为 CSPRNG 生成，仅用于请求 ID / session ID，非安全敏感值，用法合规。
- **未动翻译逻辑**：diff 确认本小功能只增加 `is_unofficial` 布尔字段及其透传，未修改任何翻译核心流程（`build_request` / `parse_response` / 重试 / 缓存等），回归风险极低。
- **未抄 pot**：实现独立，注释均引用协议事实来源，符合设计文档 §〇 许可红线。

## 发现问题（置信度 ≥ 80 才报）

经逐文件核查，无置信度 ≥ 80 的 Critical 或 Important 问题。

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| — | 无 | — | — |

**补充核查说明（低于阈值，不阻塞）：**

- `mod.rs` 第 70 行注释写「官方 keyed 源 baidu/deepl_free/google 为 false」，而设计文档 §2.1 的「DeepL（free 模式）」实为免 key 非官方接口（www2.deepl.com/jsonrpc）；但代码中 `deepl_free` 实现的是 `api-free.deepl.com/v2/translate`（需 Auth Key 的官方 API），对应设计文档 §2.2「DeepL api 模式」，是 V1 历史遗留命名。`is_unofficial: false` + `needs_key: true` 与实现一致，非本小功能改动范围，不报为问题。置信度 65，低于阈值。

## 是否合规

所有本小功能改动文件均符合项目规范与 code-standards：

1. **is_unofficial 分类正确性**：5 个免 key 非官方源（lingva/google_free/yandex/transmart/bing）= `true`；3 个官方 keyed 源（baidu/deepl_free/google）= `false`，与设计文档 §二 免 key 源清单（5 源均在 §2.1 非官方栏）完全对齐。
2. **DTO 透传链路完整**：`ProviderCapability.is_unofficial` → `ProviderDto.is_unofficial`（camelCase 序列化）→ `Provider.isUnofficial`（TS），三层一致无断口。
3. **前端标注零侵入**：`DirBar.tsx` 仅在 `useMemo` 内派生 label 后缀，未新增 prop，不污染现有 Select 接口。
4. **降级提示从既有 props 派生**：`TranslateWorkspace.tsx` 使用 `providers.some(p => p.id === selectedProviderId && p.isUnofficial)` 从现有 `providers` / `selectedProviderId` props 派生 `isSelectedUnofficial`，无冗余新 prop。
5. **DirBar.test.tsx 夹具改 `isUnofficial: false`**：合理隔离——组件级测试只验行为（列出/禁用/选中），标注功能由专项测试 `label-degrade.test.tsx` 完整覆盖，职责分离清晰。
6. **具名常量**：`UNOFFICIAL_DEGRADE_HINT` 承载降级文案，符合 code-standards §6 禁魔术字符串要求。

## 结论

**通过**。

本小功能（TV1-F4-S01）改动精确、范围收敛：仅新增 `is_unofficial` 布尔元数据及其 UI 呈现，未触碰任何翻译核心逻辑；命名/类型/注释/测试均符合项目规范与 code-standards；前端实现零侵入既有组件接口；DirBar 测试夹具改动有明确注释说明隔离理由，不构成断言削弱。tester 动态证伪已通过（命中 + 变异全红 + 边界 + 既有用例未污染），静态审查无阻塞项。

**APPROVE**
