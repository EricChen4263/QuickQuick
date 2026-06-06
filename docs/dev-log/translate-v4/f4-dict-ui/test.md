---
id: TV4-F4-S01-test
type: test_report
level: 小功能
parent: TV4-F4
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F4-A01]
---

# TV4-F4 测试报告（动态证伪）· 前端词典展示组件 DictEntryView

> tester 动态证伪（前端 vitest + @testing-library/react，含一次 maxTurns 续跑）。tester 无 Write，编排器据其结论落盘。变异经 cp 备份还原（禁 git checkout）。

## 一、命中校验（RTK verbose 取原始输出，防假绿）
3 冻结测试真命中（✓ 行确证）：`dict_result_renders_phonetic_and_definitions`、`dict_component_renders_examples_and_audio`（DictEntryView.test.tsx）、`plain_result_renders_translated_text`（TranslateWorkspace.test.tsx）。该两文件 `Tests 6 passed`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | DictEntryView 不渲染音标 | dict_result_renders_phonetic_and_definitions | 如期红 |
| B | DictEntryView 释义整块不渲染 | dict_result_renders_phonetic_and_definitions | 如期红 |
| C | DictEntryView 不渲染例句和音频 | dict_component_renders_examples_and_audio | 如期红 |
| D | TranslateWorkspace 分流改坏（plain 走 DictEntryView） | plain_result_renders_translated_text | 如期红（+ dict 渲染测试也红）|
| E（锚定） | 读断言 | — | 全为具体值强断言：`/həˈləʊ/`、`哈罗，喂`、`招呼声`、`Hello, how are you?`、audio.src=`https://example.com/hello.mp3`、`你好，世界` + `queryByTestId("dict-phonetic")` not in document，非弱断言 |

变异 A–D 全红，判别力充分。

## 三、边界探测（冻结/边界用例覆盖）
- 可选区块不渲染：`audio:null`→无 dict-audio；`phonetic:null/examples:[]/inflections:[]`→对应区块不渲染；`pos:null`→无 dict-pos 标签但释义仍渲染。均不报错、不渲染空标签。
- narrowing 安全：TranslateWorkspace `result.kind==="dict"` 后才访问 entry，else 访问 translated；TS 可判别联合 + tsc 通过印证无越界。

## 四、全绿 + 抗 flaky + tsc
`pnpm test` 全量连跑 3× 均 `54 files / 471 passed`（无 flaky）；`tsc --noEmit` 0 error。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致（M TranslateWorkspace.tsx/translate.css/trans-popover.test.tsx + 新增 DictEntryView.tsx/.test.tsx/TranslateWorkspace.test.tsx + 无关未跟踪），变异 A–D 经 cp 还原，无 git checkout。

## 门禁结论：**通过（放行）**
