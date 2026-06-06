---
id: TV4-F4-S01
type: coding
parent: TV4-F4
commit: PENDING
acceptance_ids: [TV4-F4-A01]
---

# TV4-F4-S01 前端词典展示组件 编码留痕

## 目标
方案 A（pot 全源对齐）收官小功能：前端按 TranslateResult 的 `kind` 分流渲染，
Plain→原译文（不回归），Dict→新增词典展示组件按 DictEntry 渲染音标/按词性分组释义/例句/发音/变形。

## 改动文件清单
- 新增 `src/panels/translate/DictEntryView.tsx`：词典词条展示组件。
- 新增 `src/panels/translate/DictEntryView.test.tsx`：组件单测（4 例，含 3 冻结中 2 例 + 边界）。
- 新增 `src/panels/translate/TranslateWorkspace.test.tsx`：结果分流单测（2 例，含冻结 plain_result_renders_translated_text）。
- 改 `src/panels/translate/TranslateWorkspace.tsx`：import DictEntryView；结果区按 `result.kind === "dict"` 分流（dict→DictEntryView，plain→既有 .tx-out 译文）。
- 改 `src/panels/translate/translate.css`：新增 `.dict-*` 样式（复用既有 token，无新增 token）。
- 改 `src/trans-popover/trans-popover.test.tsx`：I-1 修复——RESULT_B mock 补 `kind: "plain" as const`。

## 组件设计
- `DictEntryView({ entry })` 纯展示组件，无状态/无副作用。
- 各可选区块仅在有值时渲染（音标/例句/变形/发音），空值不占位：
  - 音标 phonetic：非 null 且非空 → `.dict-phonetic`（data-testid=dict-phonetic）。
  - 释义 definitions：按词性 pos 分组；有 pos 渲染 `.dict-pos`（data-testid=dict-pos）标签，无 pos 只列 meanings；meanings 逐条列表。
  - 例句 examples：非空数组 → `.dict-examples`（data-testid=dict-examples），左侧 accent 竖条点缀。
  - 变形 inflections：非空 → `.dict-inflections`（data-testid=dict-inflections），顿号连接。
  - 发音 audio：非 null 且非空 → 原生 `<audio controls>`（data-testid=dict-audio），无音频不渲染。
- key 用稳定 index（静态一次性渲染列表，无重排），不引入额外依赖。

## Plain/Dict 分流
- 复用既有可判别联合 `TranslateResult = TranslatePlainResult | TranslateDictResult`（F1 地基，ipc-client.ts）。
- TranslateWorkspace 结果区：`result.kind === "dict"` narrowing 安全访问 `result.entry`；否则渲染 `result.translated`（原 .tx-out 路径，Plain 不回归）。
- 译文头 `译文 · src → target` 两路共用，保持一致。

## I-1 修复
- F1 reviewer 标记：trans-popover.test.tsx 的 RESULT_B 缺 `kind` 与可判别联合不一致。补 `kind: "plain" as const`，与同文件 MOCK_RESULT 一致。

## 冻结测试命中（3/3）
- `dict_result_renders_phonetic_and_definitions` —— DictEntryView.test.tsx ✓
- `dict_component_renders_examples_and_audio` —— DictEntryView.test.tsx ✓
- `plain_result_renders_translated_text` —— TranslateWorkspace.test.tsx ✓

## 坑 / 决策
- audio 用原生 `<audio controls>` 而非自造播放按钮：零依赖、可播放入口语义明确、无障碍友好；真网播放归 manual_confirm（TV4-M01）。
- 不引入图标/渐变等装饰，遵项目 Fjord 风格用既有 token（--accent/--mono/--surface-2/--border 等）。
- vitest 配置：environment=jsdom，setupFiles=src/test-setup.ts（含 jest-dom）。

## 校验
- pnpm test 全量：见 artifacts/pnpm-test-full.log（贴真实 passed 数）。
- pnpm tsc --noEmit：0 error。
- 后端未动；共享类型未改（复用 F1 既有 DictEntry/TranslateResult）。
