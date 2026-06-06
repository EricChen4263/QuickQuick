---
id: RT1-F1-S02-test
type: test_report
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 86897a7
acceptance_ids: [RT1-F1-A02]
evidence:
  - src-tauri/tests/richtext_capture.rs
author: tester
---

# 测试报告 · RT1-F1-S02 捕获层读 HTML + 变化检测

## 运行的测试命令
```
rtk proxy cargo test --test richtext_capture   # 绕 RTK 取原始逐测试输出
# 变异 sanity：cp 备份 → 改 → 跑 → 从备份还原（禁 git checkout）
```

## 结果
**通过**（命中校验无假绿 + 2 变异全 RED + 连跑 5 次无 flaky）

## 用例清单 + 结果
| 用例 | 结果 | 对应验收项 |
|---|---|---|
| composite_hash_differs_when_html_differs | pass | RT1-F1-A02 |
| snapshot_to_clips_propagates_html | pass | RT1-F1-A02 |

## 变异 sanity（每个变异后从 cp 备份还原）
| 变异 | 预期 | 实测 |
|---|---|---|
| 删 `composite_hash_bytes` 的 html 拼接段 | hash 差异测试 RED | FAILED：html None vs Some 哈希相同 ✓ |
| `snapshot_to_clips` 把 html 写死 None | 透传测试 RED | FAILED：left None vs right Some("<b>hello world</b>") ✓ |

## 覆盖率
覆盖 A02：html 参与变化检测哈希（差异/相同/None-vs-Some）+ snapshot→CapturedItem html 透传。read() 真 arboard 读取归 manual_confirm。

## 边界探测
text 相同/html None-vs-Some/全 None→None/仅 html 有值/超长 html 不 panic/确定性重复——均 OK。
发现良性边界：`html=None` 与 `html=Some("")` 哈希相同（空段无法区分）。reviewer 定性 confidence 30 非缺陷（arboard 无 html 返 Err→None，不产生 Some("")），不阻塞 A02；可后续加标志字节优化。

## 失败项详情（与本项无关）
- `traffic_light_position_returns_centered_coords` — 预存在失败，编排器另行处理，不计入本项判定；coder 未新增其它失败。

## 结论
RT1-F1-A02 PASS。git status 开工/结束逐行一致（cp 还原无残留）。
