---
id: TV1-F4-S01-test
type: test_report
level: 小功能
parent: TV1-F4
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F4-A01]
evidence:
  - src-tauri/src/translate/providers.rs
  - src/panels/translate/label-degrade.test.tsx
  - docs/dev-log/translate-v1/f4-label-degrade/artifacts/fe-green.log
---

# TV1-F4-S01 测试报告（动态证伪）· 非官方标注 + 降级提示

> tester 动态证伪（跨 Rust+前端，含一次 maxTurns 续跑）。tester 无 Write，本报告由编排器据其返回结论落盘。

## 一、命中校验（杀假绿）
- Rust（`cargo test --lib`）：`capability_is_unofficial_flags_free_sources_only ... ok`、`get_translate_providers_impl_exposes_is_unofficial ... ok`（各 1 passed）。
- 前端（`pnpm test --reporter=verbose`）：label-degrade.test.tsx 3 用例（含 `nonofficial_source_label_and_degrade_hint`）全通过；全量 52 files / 465 passed。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | providers.rs lingva is_unofficial true→false | capability_is_unofficial_flags_free_sources_only | 如期红 |
| B | settings.rs ProviderDto is_unofficial 恒 false | get_translate_providers_impl_exposes_is_unofficial | 如期红 |
| C | DirBar 去掉「⚠ 非官方」标注 | 标注用例 | 如期红（另两用例仍绿，路径不经此） |
| D | TranslateWorkspace 降级条件恒 false | 降级提示用例 | 如期红（另两用例仍绿） |

四处全红，测试有真实判别力。

## 三、边界探测
- **双向断言**：官方源 baidu 选中→`not.toHaveTextContent("非官方")`（无标注）；baidu 失败→alert `not.toHaveTextContent("非官方")`（无降级提示，仅原始错误）。正向+否定均覆盖，无单向遗漏。
- 既有用例未污染：DirBar.test.tsx 夹具置 isUnofficial=false（只测列出/禁用，标注由新测试独立覆盖，未削弱）；translate-page 全量 465 绿。

## 四、抗 flaky
cargo 集成连跑 3× + lib 188 passed；pnpm 连跑 2× 均 465 passed。无 flaky。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，4 变异经 cp 还原，未用 git checkout/restore。

## 门禁结论：**通过（放行）**
