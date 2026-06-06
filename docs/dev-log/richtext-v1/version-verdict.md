---
id: RT1-verdict
type: version_verdict
level: 版本
parent: null
children: [RT1-F1-report, RT1-F2-report]
created: 2026-06-07T00:00:00Z
status: 条件性通过
commit: 0222931
acceptance_ids: [RT1-F1-A01, RT1-F1-A02, RT1-F1-A03, RT1-F1-A04, RT1-F2-A01, RT1-F2-A02, RT1-A-TEST, RT1-A-QUAL, RT1-A-SEC, RT1-A-LOG, RT1-M01]
evidence: []
author: producer
---

# 版本裁决报告 · RT1 剪贴板富文本（HTML）保真

> 由独立制作人 agent（Opus）产出，独立重跑、不信下层报告。codex 异构裁判交叉复核无否决。

## 逐项对照表（核心）
| 验收项 | 结果 | 证据出处 | 备注 |
|---|---|---|---|
| RT1-F1-A01 存储层 | pass | rtk proxy cargo test (db.rs) | 命中 ingest_richtext_roundtrip… / html_column_migration_idempotent… / dedup_by_plaintext_unchanged / ingest_backfills_html… 全 ok；读源码确认补写升级逻辑 |
| RT1-F1-A02 捕获 | pass | rtk proxy cargo test (pipeline.rs) | 命中 composite_hash_differs_when_html_differs / snapshot_to_clips_propagates_html |
| RT1-F1-A03 IPC 透出 | pass | rtk proxy cargo test (ipc/clipboard.rs) | 命中 list_clip_items_exposes_html_content_for_richtext / …html_null_for_plaintext |
| RT1-F1-A04 还原 | pass | rtk proxy cargo test (ipc/system.rs) | 命中 fetch_paste_item_includes_html / copy_clip_assembles_text_and_html |
| RT1-F2-A01 渲染 | pass | vitest --reporter=verbose | 命中 clip_preview_renders_sanitized_richtext / …plaintext_unchanged / popover_…richtext |
| RT1-F2-A02 复制改 IPC | pass | vitest --reporter=verbose | 命中 copy_button_invokes_copy_clip_to_clipboard / plaintext_copy_not_regressed / popover Alt+Enter |
| RT1-A-TEST 全量绿 | pass | cargo test debug+release ×3 + pnpm test ×3 | 后端 35 套件 0 failed、前端 477 passed；traffic_light…centered_coords 现 PASS（已同步）；连跑无 flaky |
| RT1-A-QUAL 工程质量 | pass | clippy / tsc / grep | clippy -D warnings exit 0；tsc 0 错；23 改动文件无 TODO/FIXME；dompurify 入 package.json |
| RT1-A-SEC 安全 | pass | grep + 测试 + tauri.conf.json | 2 处 dangerouslySetInnerHTML 全经 sanitizeRichHtml(DOMPurify)；恶意 payload(script/onerror/javascript:/iframe)断言剥离；后端原样保存；CSP 未放开 script-src |
| RT1-A-LOG 留痕 | pass | find docs/dev-log/richtext-v1 | F1×4 + F2×2 三联齐、2 feature-report、commit 真 hash 无占位 |
| RT1-M01 真机往返 | 待采证 | manual-richtext-roundtrip.png（缺） | manual_confirm + real_device，arboard GUI/CGEvent headless 采不了，并入待采证、不参与 done |

## 覆盖检查
| 类别 | 状态 |
|---|---|
| 功能正确性 | covered |
| 测试充分性 | covered |
| 工程质量 | covered |
| 性能 | N/A（html 读取/清洗开销可忽略） |
| UI还原度 | covered |
| 资源规范 | N/A（dompurify 为 npm 依赖，html_content 随整库 SQLCipher 加密） |
| 安全 | covered |
| 留痕产出 | covered |
| 人工确认点 | covered |

7 个 covered 类别每个都有匹配 category 验收项，无空洞声明。

## 未决审美项（并入全局 pending-manual.yaml，不阻塞）
- RT1-M01 — docs/dev-log/richtext-v1/manual-richtext-roundtrip.png（真机富文本往返：复制→保存为 richtext→预览显格式→粘贴/复制还原→纯文本编辑器退纯文本）

## 打回 / 熔断记录
| 小功能 | 打回次数 | 是否熔断阻塞 |
|---|---|---|
| RT1-F2-S02 | 1（过时注释 TV1-RETRO-1 + 测试 as 断言禁 any，已解复审 APPROVE） | 否 |
| 其余小功能 | 0 | 否 |

未达熔断上限(3)，无 status:阻塞。

## 总裁决
**条件性通过（版本 done）**
- 10 个客观验收项全 pass + 覆盖完整无空洞 + 无熔断 + git 前后一致 + codex 异构裁判无否决。
- RT1-M01 非阻塞待采证，需真机/人工补采。

## 裁决锚
- commit: `0222931`
- criteria_freeze: `RT1-criteria@2026-06-07`

## 制作人"没下场"证据
- git 起始/结束快照逐行一致（均仅 `?? AGENTS.md`），commit 未变，全程只读。
