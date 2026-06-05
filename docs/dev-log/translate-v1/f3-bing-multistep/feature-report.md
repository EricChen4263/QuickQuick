---
id: TV1-F3-report
type: feature_report
level: 大功能
parent: TV1
children: [TV1-F3-S01-code, TV1-F3-S01-test, TV1-F3-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F3-A01, TV1-F3-M01]
---

# TV1-F3 大功能验收报告：Bing 免key源 + 多步请求架构扩展

## 范围
扩展 provider 抽象以支持多步请求（为 Bing 两步 token 流程、及后续需签名/握手的源铺路），并实现 Bing 免 key 源。

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV1-F3-S01 | HttpExecutor 上移框架层 + TranslateProvider::translate 默认实现 + Bing 两步 override | 通过 | coding.md / test.md / review.md |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV1-F3-A01（多步抽象 + Bing 两步正确解析） | **pass** | bing_two_step/parse 等 8 测试命中 + 变异 A/B/C/D 全红（test.md） |
| TV1-F3-M01（真网 Bing 返回正确译文） | 待采证（manual；已 curl 预证 glacier→冰川/zh-Hans/zh-Hant） | pending-manual |

## 架构成果
- `HttpExecutor` 上移到 `translate::mod` 框架层；`TranslateProvider` 新增带默认实现的 `translate(req, executor)`（默认=单步），**既有 7 源零改动自动适配**。
- 多步源（Bing）override `translate` 编排两步 HTTP。
- **零回归经动态证伪坐实**：变异 C 改坏默认 translate → 既有 lingva 源测试如期变红，证明既有源确经新路径且语义正确。

## 累计免 key 源（含 F1/F2）
lingva（默认）、google_free、yandex、transmart、bing —— **5 个免 key 机翻源**。

## 携带项（非阻塞，转 F4 修）
- mod.rs:65 `needs_key` 字段注释仍引用已移除的 MyMemory（reviewer Important conf 80）→ F4 coder 顺手订正。

## 门禁
tester 动态证伪通过、code-reviewer APPROVE（1 非阻塞 Important 转 F4）、clippy `-D warnings` exit 0、未抄 pot 代码。

## 结论：**通过**（objective 全 pass；M01 待真机采证）。
