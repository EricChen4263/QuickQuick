---
id: TV4-F2-report
type: feature_report
level: 大功能
parent: TV4
children: [TV4-F2-S01-code, TV4-F2-S01-test, TV4-F2-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F2-A01]
---

# TV4-F2 大功能验收报告：ECDICT + 有道词典模式（JSON 词条→Dict）

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV4-F2-S01 | EcdictProvider（pot-app/api/dict 免key，parse→Dict）+ YoudaoDictProvider（复用有道 SHA256 v3 签名，isWord 分流 Dict/Plain）+ 解析纯函数（parse_youdao_basic/group_definitions_by_pos/parse_ecdict_exchange 等）| 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV4-F2-A01 | **pass** | ECDICT build+parse→Dict（音标/词性分组释义/词形）+ 有道 isWord→Dict、非词→Plain 回退 + 错误分类 + 变异 A/B/C2/D 红 |

## 门禁
tester 动态证伪通过（3 冻结命中 + 变异 A/B/C2/D 全红[C 初版未命中经路径分析定位真实判别条件 basic 字段后由 C2 覆盖，真证伪]+ 边界 panic 安全 + errorCode→Auth + sentinel 不泄密 + debug×3/release 505 passed + clippy 0）、code-reviewer APPROVE（解析正确性逐项核实、有道签名完全复用不另造、isWord&&basic 回退健壮、未抄 pot、key is_secret 加密；I-1 非阻塞）。

## 关键决策
- ECDICT：phonetic→音标；translation 按 \n + 词性前缀分组→definitions；exchange 取 `:` 后值→inflections。is_unofficial=true（pot 自建）。
- 有道词典：isWord===true && basic.is_object() → Dict（音标优先级 us→phonetic→uk、explains 词性分组、wfs 词形）；否则回退 Plain（translation 拼接）。复用既有 youdao_sign。
- youdao_dict 与既有 youdao 共用凭据 schema（app_key/app_secret），不破坏既有翻译源。

## 遗留（非阻塞，TV4-F3 顺修）
- I-1（reviewer，confidence 82）：providers.rs:14–24 build_provider doc 注释未补 ecdict（免key）/ youdao_dict（app_key/app_secret）两条（bing 为预存遗漏）。TV4-F3 在同文件工作时顺手补。

## 结论：**通过**（A01 objective pass；真网词条 manual 待 TV4-M01 采证）。
