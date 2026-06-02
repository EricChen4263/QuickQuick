---
id: V5-F5-report
type: feature_report
level: 大功能
parent: V5
children: [f5-s01-lang-dropdown]
created: 2026-06-02T00:00:00Z
status: 已闭合
author: orchestrator
---

# 大功能报告 · V5-F5 翻译方向语言下拉选择

## 目标
用户驱动的 UI 增强：把翻译页方向栏从「静态药丸 `auto ⇄ zh` + 交换按钮」改为「左右两个语言下拉框」，并让源语真正可选（含自动检测 + 常用多语言）。

## 关键产品决策（用户确认）
- **语言范围**：常用多语言（自动检测 / 中 / 英 / 日 / 韩 / 法 / 德 / 西 / 俄）。源语下拉含「自动检测」，目标语下拉不含（目标必须具体）。
- **去掉交换按钮**：只留左右两个下拉框。
- **已知边界（用户接受）**：自动检测仅能分 zh/en（含 CJK→zh，否则→en），其余语言需显式选源；多语言靠 MyMemory langpair 透传实现。

## 小功能闭合清单

| 小功能 | 内容 | 验证 | 锚点 commit |
|---|---|---|---|
| s01 语言下拉框 | 后端：`translate_text` 加 `source` 参数，新增 `resolve_direction_with_source`（显式源非空非auto→跳过检测）；多语言 langpair 透传。前端：新建 `languages.ts`，DirBar 两个 `<select>` 取代药丸+swap，TranslatePage source(默认auto)/target(默认zh) state，`translateText(text,target?,source?)`，删 handleSwap | tester 4 变异全红（A 显式源覆盖检测/B impl贯通/C前端第三参/D目标不含auto）+ 边界四点；reviewer 1 高危 CSS（下拉箭头定位缺失）**已收口**；后端 310 + 前端全绿、tsc 干净 | （见提交）|

## 关键决策与亮点
- **请求/响应方向分离**：顶部两个下拉框 = 用户**选择**的请求方向；译文区 `源 → 目标` 标签仍显示后端**实际**用的方向（响应）。二者有意分离，自动检测时尤其有用（用户选 auto，结果区显示后端检测出的真实源语）。
- **后端最小侵入**：`resolve_direction` 保留，新增 `resolve_direction_with_source` 处理显式源；`is_explicit_source` 谓词 trim 判空 + 排除 `AUTO_SOURCE` 常量，空/全空白源安全回退检测，不拼空 langpair。
- **多语言可信度**：MyMemory `build_request` 对源/目标都走 `map_lang_for_provider`，ja/ko/fr/de/es/ru 走原样透传分支，拼出合法 langpair；tester 加透传单测佐证。
- **诚实边界**：自动检测只分 zh/en——这是既有 `detect_lang` 的能力上限，未夸大；显式选源可绕过检测用全部语言。

## 流程纪要
- tester 因网络/额度在收尾环节连续截断两次（Phase3 边界 + 写 test.md），靠"增量写 test.md + 编排器只读核对边界喂事实 + 派极小接续 agent"补完完整三板斧证据，未降低门禁标准。
- reviewer 抓出的高危（语言下拉箭头 CSS 定位缺失致视觉破损）经独立审查暴露——coder 套了 `.wrap` 结构却漏了 `.lang-selects` 对应定位规则，是"独立 reviewer 防自评自过"的真实兑现。

## 待用户手动确认（manual，不阻塞代码门禁）
- 重启 `pnpm tauri dev`（改了 Rust + 前端），翻译页方向栏应显示左右两个语言下拉框（无交换按钮），箭头正常叠在右侧。
- 选具体源语（如「日文」）+ 目标语（如「韩文」）翻译一段日文，验证 MyMemory 是否返回韩文（多语言直通真实可用）。
- 源选「自动检测」时，中文→英文 / 英文→中文 默认方向是否正常；译文区标签是否显示后端检测出的真实源语。
