---
id: TV4-F3-report
type: feature_report
level: 大功能
parent: TV4
children: [TV4-F3-S01-code, TV4-F3-S01-test, TV4-F3-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F3-A01]
---

# TV4-F3 大功能验收报告：Bing词典 JSON + 剑桥 HTML scraper + 非词回退

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV4-F3-S01 | BingDictProvider（bing.com/api/v6 硬编码appid 免key，JSON→Dict）+ CambridgeProvider（dictionary.cambridge.org，scraper 0.27 HTML 解析→Dict，音标/音频/释义）+ 非词回退（ParseError 明确提示）+ I-1 doc 修复 | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV4-F3-A01 | **pass** | Bing JSON→Dict（音标/词性分组释义/变形）+ 剑桥 HTML→Dict（音标/音频/释义）+ 非词输入回退/提示不 panic + 变异 A–D 红 |

## 门禁
tester 动态证伪通过（3 冻结命中 + 变异 A–D 全红[D 深防御双防线同改才红，证判别力]+ 畸形 HTML panic 安全 + scraper 无 JS 执行路径 + 无打印密钥 + debug×5/release 513 passed 无 flaky + clippy 0）、code-reviewer APPROVE（无 Critical/Important；字段/选择器映射符合设计§二.2.4、scraper 仅剑桥用、音频绝对补全健壮、BING_DICT_APPID 公开标识非密钥、未抄 pot、I-1 doc 修复完整、registry 21→23 同步）。

## 关键决策
- Bing JSON：音标 pronunciations.transcriptions、释义 meaningGroups 按 partsOfSpeech 分组、变形 inflections.displayText。is_unofficial=true。
- 剑桥 HTML（scraper 0.27，html5ever 无 JS 执行）：音标 .ipa、音频 source[type=audio/mpeg].src 补全为绝对 URL、释义 .def-block（.pos/.def/.trans）。仅英文输入。is_unofficial=true。
- 非词回退：空/无结果→明确 ParseError 提示，深防御双防线（ok_or_else + definitions/phonetic 皆空兜底），不 panic 不返垃圾。「回退普通翻译」由上层兜底链负责。
- BING_DICT_APPID 公开客户端标识提为具名常量。

## 里程碑
**registry 达 23 源 = pot 全集**（6 免key机翻 + 10 需key机翻 + 4 LLM + ...词典，方案A 21 源目标按 provider 计已全部落地，含 baidu/deepl/google 等既有）。

## 结论：**通过**（A01 objective pass；真网词条/剑桥音频 manual 待 TV4-M01 采证）。
