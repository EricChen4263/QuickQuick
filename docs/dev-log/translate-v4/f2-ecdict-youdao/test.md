---
id: TV4-F2-S01-test
type: test_report
level: 小功能
parent: TV4-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F2-A01]
---

# TV4-F2 测试报告（动态证伪）· ECDICT + 有道词典模式（JSON 词条→Dict）

> tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，编排器据其结论落盘。变异经 cp 备份还原（禁 git checkout）。

## 一、命中校验（RTK 完整路径取原始输出，防假绿）
3 冻结测试真命中（各 1 passed，503 filtered）：`ecdict_build_and_parse_dict`、`youdao_dict_parses_basic_to_dict`、`youdao_dict_falls_back_to_plain_when_not_word`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | ECDICT parse phonetic 取错字段 | ecdict_build_and_parse_dict | 如期红（音标断言 Some("ˈɡleɪʃər") 失败）|
| B | youdao_dict isWord 判断反转 | youdao_dict_parses_basic_to_dict | 如期红（应返 Dict 实返 Plain）|
| C | is_word 强制 true | youdao_dict_falls_back_to_plain_when_not_word | 初次**仍绿**——tester 分析根因：回退真正判别力来自「有无 basic 字段」（`is_word && basic.is_object()`），非词响应无 basic 故路径未变；精修为 C2 |
| C2 | 回退路径输出值改坏 | youdao_dict_falls_back_to_plain_when_not_word | 如期红（"MUTATED"≠"你好世界这是一个句子"，证锚定具体译文）|
| D | ECDICT exchange 词形取 `:` 前值（应取后值） | inflections 断言 | 如期红（词形应含 glaciers）|
| E（锚定） | 读断言 | — | 确认 ecdict 断言具体音标/词形、youdao 回退断言具体译文，非弱断言 |

变异 A/B/C2/D 全红，判别力充分。**C 初版未命中经 tester 路径分析定位真实判别条件后由 C2 覆盖**——真证伪，非假绿。

## 三、边界/安全
- panic 安全：ECDICT 非法 JSON / 空词条 `{"word":"","translation":""}` → ParseError 不 panic。
- 错误分类：youdao_dict errorCode "108" → TranslateError::Auth。
- 密钥不泄露（hints TV2-RETRO-1）：sentinel SENTINEL_DEADBEEF 断言 `!body.contains`（同验签名正确复用 youdao_sign）；grep providers.rs `eprintln|println|log::|dbg!` 零匹配。

## 四、debug + release 双绿 + 抗 flaky
`cargo test` debug 连跑 3× 均 `505 passed; 0 failed`（32 套件，无 flaky）+ `cargo test --release` `505 passed`；`cargo clippy --all-targets -- -D warnings` exit 0 No issues。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致（M credential.rs/providers.rs/tests/translate.rs + 无关未跟踪），变异 A/B/C/C2/D 经 cp 全还原，无 git checkout。

## 门禁结论：**通过（放行）**
