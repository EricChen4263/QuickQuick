---
id: TV1-F1-report
type: feature_report
level: 大功能
parent: TV1
children: [TV1-F1-S01-code, TV1-F1-S01-test, TV1-F1-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F1-A01, TV1-F1-A02, TV1-F1-A03, TV1-F1-M01]
---

# TV1-F1 大功能验收报告：Lingva 默认源替代 MyMemory

## 范围
用免 key 的 Lingva 替代质量差的默认源 MyMemory（实测「冰川→Bing Chuan」垃圾），含默认源切换与设置迁移。本大功能由单一小功能 **TV1-F1-S01** 闭合。

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV1-F1-S01 | LingvaProvider + 移除 MyMemory + 默认切 lingva + 迁移纯函数 | 通过 | coding.md / test.md / review.md 齐 |

## 大功能级验收项对照
| 验收项 | 断言 | 结果 | 证据 |
|---|---|---|---|
| TV1-F1-A01 | 注册表含 lingva(免key)、不含 mymemory；build_provider 行为正确 | **pass** | test `providers_registry_has_lingva_no_mymemory ... ok` |
| TV1-F1-A02 | Lingva build_request URL + parse_response(`translation`) 正确 | **pass** | test `lingva_build_request_url_and_parse_translation ... ok` |
| TV1-F1-A03 | 默认 lingva；非法/未知 selected_provider（含 mymemory）迁移回退 lingva，不过度回退有效源 | **pass** | test `selected_provider_migrates_unknown_to_lingva ... ok` + 边界探测 baidu 保持 |
| TV1-F1-M01 | 真网 Lingva 返回正确译文、开箱可用 | **待采证**（manual_confirm/real_device，headless 不打真网；已 curl 预证 冰川→glacier） | pending-manual |

## 门禁
- tester 动态证伪：通过（3 验收命中 + 变异 A/B/C 全红 + 迁移无过度回退 + cargo 连跑 3×163 绿 + 前端 462 绿 + 工作区干净）。
- code-reviewer：APPROVE（无 Critical；2 Important 清洁项已修）。
- 工程质量：cargo clippy `-D warnings` exit 0、tsc 无错。
- 许可：未复制 pot 代码，注释标 Lingva 公开协议来源。

## 结论：**通过**（objective 项全 pass；M01 待真机采证，不阻塞 done）。
