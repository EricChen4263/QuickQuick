---
id: TV4-F1-report
type: feature_report
level: 大功能
parent: TV4
children: [TV4-F1-S01-code, TV4-F1-S01-test, TV4-F1-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F1-A01]
---

# TV4-F1 大功能验收报告：TranslateResponse 枚举重构（Plain|Dict + DictEntry）

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV4-F1-S01 | TranslateResponse 重构为 enum（struct variant + serde tag="kind"/lowercase）+ DictEntry/PosDefinition 类型 + plain() 便捷构造；19 源全返 Plain；DTO 扁平+kind+可选 entry；前端 TranslateResult 可判别联合 + DictEntry TS | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV4-F1-A01 | **pass** | Plain roundtrip + 19 源返 Plain 不回归（246/495 后端）+ DictEntry 序列化带 type tag + 前端 465 不回归 + 变异 A–D 红 |

## 门禁
tester 动态证伪通过（3 冻结命中 + 变异 A–D 全红[serde tag/字段名/不回归断言锚定具体值] + 19 源不回归 + 前端 Plain 渲染不回归 + debug×3/release 495 passed + pnpm×3 465 passed + clippy 0 + tsc 0）、code-reviewer APPROVE（enum+serde tag 前后端严格对齐、DTO 扁平方案保方向字段零回归、19 源全返 Plain 无遗漏、DictEntry 字段完整覆盖设计§二.2.4；I-01 非阻塞）。

## 关键决策
- enum 用 struct variant + `#[serde(tag="kind", rename_all="lowercase")]`：Plain→`{kind:"plain",translated}`、Dict→`{kind:"dict",entry}`，前端可判别联合对齐。
- DTO 扁平方案（kind + translated/方向字段 + 可选 entry skip_if_none）：保既有前端方向字段零回归；Dict 经 dict_entry_summary 压摘要写历史。
- 前端消费处本步只访问基类 translated（Plain/Dict 共有），Dict 专属渲染留 TV4-F4。

## 遗留（非阻塞，TV4-F4 顺修）
- I-01（reviewer，confidence 75）：src/trans-popover/trans-popover.test.tsx 的 RESULT_B mock 缺 `kind:"plain"`（vi.fn() 未强类型故 tsc 不报，测试有效性不受影响）。TV4-F4 前端工作时顺手补 `kind:"plain" as const`。

## 结论：**通过**（A01 objective pass，TV4 地基就位，后续词典源/前端组件可接入）。
