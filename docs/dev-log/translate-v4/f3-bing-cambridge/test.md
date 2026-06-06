---
id: TV4-F3-S01-test
type: test_report
level: 小功能
parent: TV4-F3
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV4-F3-A01]
---

# TV4-F3 测试报告（动态证伪）· Bing词典 JSON + 剑桥 HTML scraper + 非词回退

> tester 动态证伪（含一次 maxTurns 续跑）。tester 据其结论落盘（编排器复核补齐 frontmatter）。变异经 cp 备份还原（MD5 校验，禁 git checkout）。

## 一、命中校验（RTK 完整路径取原始输出，防假绿）
3 冻结测试真命中（各 1 passed，512 filtered）：`bing_dict_parses_json_to_dict`、`cambridge_parses_html_to_dict`、`dict_source_falls_back_or_hints_on_non_word`。

## 二、变异 sanity（cp 备份还原，MD5 校验，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | Bing parse pronunciations 取错字段 | bing_dict_parses_json_to_dict | 如期红（音标 None≠Some("ˈɡleɪʃər")）|
| B | Cambridge `.def-block` 选择器改不存在 | cambridge_parses_html_to_dict | 如期红（未找到释义 ParseError）|
| C | Cambridge 音频 src 绝对补全去掉 | cambridge_parses_html_to_dict | 如期红（音频 URL 相对≠绝对）|
| D | 非词回退两道防线同时改坏 | dict_source_falls_back_or_hints_on_non_word | 如期红（应 ParseError 实 Ok(空 Dict)）|
| E（锚定） | 读断言 | — | 三冻结均断言具体值（Bing 具体音标/中文释义/glaciers；Cambridge 具体音标 ˈɡlæs.i.ər/绝对音频 URL/英汉释义；非词 ParseError(非空 msg)），非弱断言 |

变异 A–D 全红。**D 说明**：单独绕过第一道 ok_or_else 仍绿（第二道 `definitions.is_empty()&&phonetic.is_none()` 兜底仍 Err，属深防御），两道同时改坏才红——证测试有真实判别力。

## 三、边界/安全
- **panic 安全**：Bing 非法 JSON / 空数组 / 空对象、Cambridge 畸形 HTML / 无结果 / 空串 → 均 ParseError 不 panic（scraper/html5ever 遵 HTML5 错误恢复天然容错）。
- **无 JS 执行**：`cargo tree -p scraper` 依赖链 cssparser+html5ever+selectors+ego-tree，无 v8/boa/quickjs/deno，解析只取文本。
- **无打印密钥**：grep providers.rs `eprintln|println|log::|dbg!` 零匹配；`BING_DICT_APPID` 为硬编码公开客户端标识（非用户密钥/签名密钥）。

## 四、debug + release 双绿 + 抗 flaky
`cargo test` debug 连跑 5× 均 `513 passed; 0 failed`（无 flaky）+ `cargo test --release` `513 passed`；`cargo clippy --all-targets -- -D warnings` exit 0 No issues。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致（M Cargo.lock/Cargo.toml/providers.rs/tests/translate.rs + 无关未跟踪；Cargo.lock 因新增 scraper 变更属预期），变异 A–D 经 cp 还原 MD5 一致，无 git checkout。

## 门禁结论：**通过（放行）**
