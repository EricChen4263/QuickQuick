---
id: TV1-verdict
type: version_verdict
level: 版本
parent: TV1
created: 2026-06-06T00:00:00Z
status: 条件性通过
commit: PENDING
anchor:
  criteria_freeze: TV1-criteria@2026-06-06
---

# TV1 版本裁决报告（独立 producer）

> 由独立 producer（只读、无写权）核对冻结 acceptance、独立重跑命中校验、git 前后快照一致后产出；编排器据其结构化结论落盘。含异构裁判（codex）交叉复核。

## 裁决锚
criteria_freeze=`TV1-criteria@2026-06-06`；git 裁决前后快照逐行一致（producer 未引入改动）。

## change_log 合法性
两条均合法、非移动球门：① DeepL-free 端点实测 HTTP 429 限流（外部不可用，设计§七已接受，证据 deepl-web-probe.log）→ F2-A01 收敛 4→3 源；② F2-A01 verify.ref 裁决前回填实际 6 测试名（规范§四允许）。

## 逐项对照表
| acceptance_id | result | 证据 |
|---|---|---|
| TV1-F1-A01 | pass | `providers_registry_has_lingva_no_mymemory ... ok` |
| TV1-F1-A02 | pass | `lingva_build_request_url_and_parse_translation ... ok` |
| TV1-F1-A03 | pass | `selected_provider_migrates_unknown_to_lingva ... ok` |
| TV1-F2-A01 | pass | google_free/yandex/transmart 各 build+parse 共 6 测试 `... ok`（DeepL-free 暂缓合法） |
| TV1-F3-A01 | pass | `bing_two_step_translate_with_mock_executor ... ok` + bing parse/错误路径 ok |
| TV1-F4-A01 | pass | Rust `capability_is_unofficial_flags_free_sources_only`/`get_translate_providers_impl_exposes_is_unofficial ... ok`；前端 `nonofficial_source_label_and_degrade_hint` 3 用例通过 |
| TV1-A-TEST | pass | cargo 连跑 3× 各套件 0 failed；pnpm 连跑 3× 均 465 passed，无 flaky |
| TV1-A-QUAL | pass | clippy `-D warnings` exit 0；tsc --noEmit 无错；本版改动文件无 TODO/FIXME |
| TV1-A-SEC | pass | providers.rs 无 eprintln/println；免key源 build_provider 不读凭据，needs_key=false |
| TV1-A-LOG | pass | 4 大功能三联齐（F2 含 -s02）+ feature-report + acceptance（扁平 f*/ 结构） |
| TV1-F1-M01 / F2-M01 / F3-M01 / F4-M01 | 未决（待采证） | manual_confirm/real_device，headless 采不了真网真机 → 并入 pending |

## coverage_check
功能正确性/测试充分性/工程质量/UI还原度/安全/留痕产出/人工确认点 covered（各有匹配条目，无空洞）；性能/资源规范 N/A（合理）。

## rejections
`{id: TV1-F2-S01, count: 1, blocked: false}`：tester 抓到 2 个 F1 旧测试因新增 google_free 失败、coder 漏同步，已修复闭合。非熔断。DeepL-free 暂缓属外部端点不可用，非熔断。

## pending_manual
TV1-F1-M01 / F2-M01 / F3-M01 / F4-M01 → 待采证（并入全局 pending-manual.yaml，不阻塞 done）。

## cross_judge
engine: codex（read-only）。首轮对 A-TEST/A-QUAL 因证据缺口投 fail，producer 补齐证据（pnpm 连跑 3× / 全改动文件扫 TODO）后 codex 改判 pass。dissent: []。

## verdict：**条件性通过（版本 done）**
blocking: [] 。客观项全 pass、覆盖完整、无熔断、git 一致、dissent 空。manual 项待采证不阻塞。
