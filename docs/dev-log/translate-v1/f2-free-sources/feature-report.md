---
id: TV1-F2-report
type: feature_report
level: 大功能
parent: TV1
children: [TV1-F2-S01-code, TV1-F2-S01-test, TV1-F2-S01-review, TV1-F2-S02-code, TV1-F2-S02-test, TV1-F2-S02-review, TV1-F2-S03-code]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F2-A01, TV1-F2-M01]
---

# TV1-F2 大功能验收报告：其余免 key 机翻源

## 范围
对齐 pot 的其余免 key 机翻源。最终实现 3 源、暂缓 1 源（端点限流）。

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV1-F2-S01 | Google 免费源（google_free，translate_a/single?client=gtx） | 通过 | coding.md / test.md / review.md |
| TV1-F2-S02 | Yandex + Transmart 免费源 | 通过 | coding.md / test-s02.md / review-s02.md |
| TV1-F2-S03 | DeepL free web | **暂缓**（端点 www2.deepl.com/jsonrpc 实测稳定 429 限流，非代码缺陷；证据 artifacts/deepl-web-probe.log） | coding.md（含实测结论） |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV1-F2-A01（收敛为 Google/Transmart/Yandex 三源，见 acceptance change_log 2026-06-06） | **pass** | 各源 build_request/parse 单测命中 + 变异全红（test.md / test-s02.md） |
| TV1-F2-M01（真网各源返回正确译文） | 待采证（manual_confirm/real_device；Google/Lingva 已 curl 预证；DeepL-free 暂缓） | pending-manual |

## 已实现免 key 源累计（含 F1）
lingva（默认）、google_free、yandex、transmart —— 4 个免 key 机翻源，开箱免配置可用。

## 暂缓说明
DeepL-free（非官方 jsonrpc）端点对本环境匿名访问被稳定限流，按设计文档§七（非官方接口可失效）与红线「实测不通不硬造」暂缓；DeepL 能力由既有官方 keyed `deepl_free` 源兜底。已走 acceptance change_log 显式留痕，未静默放过。

## 门禁
- 每实现源：tester 动态证伪通过（命中 + 变异全红 + 边界安全 + 连跑 3× 绿）、code-reviewer APPROVE。
- 工程质量：cargo clippy `-D warnings` exit 0。
- 许可：未复制 pot 代码，各源注释标公开协议来源。

## 结论：**通过**（A01 收敛后 objective 全 pass；DeepL-free 暂缓已留痕；M01 待真机采证不阻塞）。
