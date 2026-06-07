---
id: RT1-F2-S03-test
type: test_report
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F2-S03]
evidence:
  - src/panels/clipboard/rich-link.test.ts
  - src/panels/clipboard/rich-link-click.test.tsx
author: tester
---

# 测试报告 · RT1-F2-S03 富文本链接点击走外部浏览器

## 运行的测试命令
```
rtk proxy pnpm test ; cd src-tauri && rtk proxy cargo test
pnpm exec tsc --noEmit
# 变异 sanity：cp 备份 → 改 → 跑 → 从备份还原（禁 git checkout）
```

## 结果
**通过**（命中无假绿 + 3 变异全 RED + 连跑 3 次无 flaky）

## 用例清单 + 结果
| 用例 | 结果 | 对应 |
|---|---|---|
| resolve_rich_link_filters_non_http_schemes（含 file://、javascript:、data:） | pass | RT1-F2-S03 |
| rich_link_click_opens_external_and_prevents_default | pass | RT1-F2-S03 |
| rich_link_click_ignores_non_link_target | pass | RT1-F2-S03 |
| 点击链接内子元素 closest 命中 / 点非链接返 null | pass | RT1-F2-S03 |

## 变异 sanity（cp 备份还原）
| 变异 | 预期 | 实测 |
|---|---|---|
| 删 `preventDefault()` | prevents_default 测试 RED | FAILED：defaultPrevented=false ✓ |
| 去 scheme 白名单（直接返 href） | filters_non_http 测试 RED | FAILED：file:// 被放行 ✓ |
| 删 null guard（非链接也 open） | ignores_non_link 测试 RED | FAILED：openExternalUrl 被调 1 次 ✓ |

## 覆盖率
覆盖：链接点击 preventDefault + openExternalUrl、非链接放行、scheme 白名单（http/https/mailto 放行，file://、javascript:、data:、无 href 拒）、closest 子元素命中。openExternalUrl 真实开浏览器属 GUI/插件，归 RT1-M01 manual_confirm。

## 全量
pnpm test 482 passed（57 文件）；cargo test 531 passed/0 failed（含 boot_smoke 守卫，加插件后能编译）；tsc 0 错。连跑 3 次无 flaky。

## 结论
RT1-F2-S03 PASS。git status 开工/结束逐行一致（cp 还原无残留）。
