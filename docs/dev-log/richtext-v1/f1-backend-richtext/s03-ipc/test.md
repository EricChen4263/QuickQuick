---
id: RT1-F1-S03-test
type: test_report
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 9ee6e7a
acceptance_ids: [RT1-F1-A03]
evidence:
  - src-tauri/tests/ipc_clipboard.rs
author: tester
---

# 测试报告 · RT1-F1-S03 IPC 取数透出 html

## 运行的测试命令
```
rtk proxy cargo test --test ipc_clipboard   # 命中校验
pnpm exec tsc --noEmit                       # 前端类型
rtk proxy cargo test --test traffic_light    # 权威核实预存在失败
# 变异 sanity：cp 备份 → 改 → 跑 → 从备份还原（禁 git checkout）
```

## 结果
**通过**（S03 本身）；附：版本级全量绿 FAIL（traffic_light 预存在失败，见下）

## 用例清单 + 结果
| 用例 | 结果 | 对应验收项 |
|---|---|---|
| list_clip_items_exposes_html_content_for_richtext | pass | RT1-F1-A03 |
| list_clip_items_html_null_for_plaintext | pass | RT1-F1-A03 |

## 变异 sanity（cp 备份还原）
| 变异 | 预期 | 实测 |
|---|---|---|
| `html_content: r.html_content` 改 `html_content: None` | richtext 透出测试 RED | FAILED：left None vs right Some("<b>hello</b>") ✓ |

## 覆盖率
覆盖 A03：richtext 行透出 html 串、纯文本行 None。tsc 0 错保证前后端类型一致。

## 权威核实（编排器要求）：traffic_light 预存在失败
`rtk proxy cargo test --test traffic_light` 原始输出：
```
test traffic_light_position_returns_centered_coords ... FAILED
  left: 15.0
 right: 12.0
test result: FAILED. 1 passed; 1 failed
```
**确认 FAIL**。根因：`eff3f36`（用户本人提交）把 `traffic_light_logical_position` 改为 (18,15)、标题栏 38→44px 并新增内联等价测试，但漏改本 stale 集成测试（仍断言 12.0）。coder 自报"全量绿"系误报。与 S03 无关，由 RT1-A-TEST 处理（同步 stale 测试到 15.0/44px）。

## 结论
RT1-F1-A03 PASS。git status 开工/结束逐行一致（cp 还原无残留）。
