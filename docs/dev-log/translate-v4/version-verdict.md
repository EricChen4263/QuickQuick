---
id: TV4-verdict
type: version_verdict
level: 版本
parent: TV4
created: 2026-06-06T00:00:00Z
status: 条件性通过
commit: c5da3b3
anchor:
  criteria_freeze: TV4-criteria@2026-06-06
---

# TV4 版本裁决报告（独立 producer）

> 由独立 producer（只读、无写权）核对冻结 acceptance、独立重跑命中校验、git 前后快照一致后产出；编排器据其结构化结论落盘。含异构裁判（codex）交叉复核。本版为方案A（pot 全源对齐）最终版本。

## 裁决锚
criteria_freeze=`TV4-criteria@2026-06-06`；受验代码 F1=`a46ce51`/F2=`7b34b76`/F3=`554abad`/F4=`304dba8`（HEAD c5da3b3）；git 裁决前后快照逐行一致（producer 未引入改动）。

## change_log 合法性
`change_log: []`——本版未改动冻结标准。

## 逐项对照表
| acceptance_id | result | 证据 |
|---|---|---|
| TV4-F1-A01 | pass | `translate_response_plain_variant_roundtrip ... ok`、`existing_providers_return_plain_no_regression ... ok`、`dict_entry_serializes_with_type_tag ... ok`（lib + tests/translate_response_refactor.rs 集成各 ok）|
| TV4-F2-A01 | pass | `ecdict_build_and_parse_dict ... ok`、`youdao_dict_parses_basic_to_dict ... ok`、`youdao_dict_falls_back_to_plain_when_not_word ... ok` |
| TV4-F3-A01 | pass | `bing_dict_parses_json_to_dict ... ok`、`cambridge_parses_html_to_dict ... ok`、`dict_source_falls_back_or_hints_on_non_word ... ok` |
| TV4-F4-A01 | pass | 前端 `dict_result_renders_phonetic_and_definitions ✓`、`dict_component_renders_examples_and_audio ✓`、`plain_result_renders_translated_text ✓` |
| TV4-A-TEST | pass | 后端 debug 连跑 3× 均 513 passed/0 failed + release 513 passed；前端连跑 3× 均 54 files/471 passed；无 flaky |
| TV4-A-QUAL | pass | clippy `-D warnings` exit 0 + tsc --noEmit exit 0；grep TODO/FIXME 0；scraper 仅剑桥 HTML 用 |
| TV4-A-SEC | pass | grep eprintln/println/log/dbg providers.rs 0 匹配；泄露断言用非空 sentinel；HTML 解析无 JS 引擎；BING_DICT_APPID 公开标识非密钥；有道 key is_secret |
| TV4-A-LOG | pass | f1/f2/f3/f4 各有 coding/test/review/feature-report，acceptance 在根，commit 全回填真 hash 无 PENDING |
| TV4-M01 | 未决（待采证） | manual_confirm/real_device，真网词条+剑桥音频+有道 key+前端展示，headless 采不了 → 并入 pending |

命中校验：均确认目标测试真被跑到（ok/✓ 且 N≥1），裸短串假绿已用完整路径规避。

## coverage_check
功能正确性/测试充分性/工程质量/安全/留痕产出/人工确认点 covered（各有匹配 category 条目）；**UI还原度 covered→TV4-F4-A01（category=UI还原度）支撑**；性能/资源规范 N/A（合理）。完整性校验通过，无空洞。

## rejections
无 fail、无打回、无熔断。两个非阻塞 reviewer I-1 跨大功能顺修闭环（F1 的 RESULT_B mock 缺 kind→F4 修；F2 的 doc 注释→F3 修）。无任一小功能达 3 次打回上限。

## pending_manual
TV4-M01 → 待采证（并入全局 pending-manual.yaml，不阻塞 done）。

## cross_judge
engine: codex（codex-cli 0.137.0，read-only）。首轮对 TV4-A-TEST 因前端仅 2 次连跑证据投否决 → producer 补跑前端第 3 次（471 passed）补足"连跑≥3"，codex 改判 pass。dissent: []（8 objective 项补证后全 pass）。

## verdict：**条件性通过（版本 done）**
blocking: [] 。8 个客观项全 pass、覆盖完整、无熔断、git 前后一致、dissent 空。manual 项（TV4-M01）待采证不阻塞。

## 里程碑
TV4 完成标志**方案A（翻译源对齐 pot）全部收官**：TV1（免key机翻）+ TV2（需key机翻）+ TV3（LLM）+ TV4（词典）四版闭合，registry 达 23 个 provider（含 pot 21 内置源 + 既有 baidu/deepl/google），DictEntry 枚举 + 前端词典组件就位。
