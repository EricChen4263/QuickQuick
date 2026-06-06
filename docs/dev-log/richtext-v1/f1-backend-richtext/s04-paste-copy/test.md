---
id: RT1-F1-S04-test
type: test_report
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A04]
evidence:
  - src-tauri/tests/richtext_paste_copy.rs
author: tester
---

# 测试报告 · RT1-F1-S04 还原：粘贴 + 复制（后端）

## 运行的测试命令
```
rtk proxy cargo test --test richtext_paste_copy   # 命中校验
pnpm exec tsc --noEmit                             # 前端类型
# 变异 sanity：cp 备份 → 改 → 跑 → 从备份还原（禁 git checkout）
```

## 结果
**通过**（命中无假绿 + 2 变异全 RED + 连跑 3 次无 flaky + 边界守卫齐）

## 用例清单 + 结果
| 用例 | 结果 | 对应验收项 |
|---|---|---|
| fetch_paste_item_includes_html | pass | RT1-F1-A04 |
| copy_clip_assembles_text_and_html | pass | RT1-F1-A04 |

## 变异 sanity（cp 备份还原）
| 变异 | 预期 | 实测 |
|---|---|---|
| fetch_paste_item 构造 html 写死 None | 两测试 RED | FAILED：None != Some("<b>hello</b>")/Some("<i>world</i>") ✓ |
| SQL `SELECT ... NULL` 截断 html | 两测试 RED | FAILED：NULL→None 断言失败 ✓ |

## 覆盖率
覆盖 A04：粘贴取数含 html（富文本/纯文本 None）、复制取数组装。边界守卫（空 id/不存在 id/图片 id 返 Err 不 panic）由 system.rs 单元测试覆盖。arboard set().html 实写归 RT1-M01 manual_confirm。

## 边界探测
空 id→Err、不存在 id→Err、图片 id→Err（均不 panic）、纯文本 html=None——全 OK。连跑 3 次无 flaky。

## 失败项详情（与本项无关）
- `tests/traffic_light.rs::traffic_light_position_returns_centered_coords` — 预存在失败，coder 如实报告未误报；编排器另行同步（eff3f36 遗留 stale 测试）。

## 结论
RT1-F1-A04 PASS。tsc 0 错。git status 开工/结束逐行一致（cp 还原无残留）。
