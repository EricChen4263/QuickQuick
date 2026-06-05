---
id: TV1-F2-S01-test
type: test_report
level: 小功能
parent: TV1-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F2-A01]
evidence:
  - src-tauri/src/translate/providers.rs
  - docs/dev-log/translate-v1/f2-free-sources/artifacts/cargo-test-full-run1.log
---

# TV1-F2-S01 测试报告（动态证伪）· Google 免费源

> tester 动态证伪 + 一次打回-修复闭环。tester 无 Write，本报告由编排器据其返回结论落盘。

## 一、命中校验（杀假绿）
5 个新测试真实命中（`test result: ok. 5 passed`，N=5，非空匹配/skip）：
`google_free_build_request_url`、`google_free_parse_concatenates_segments`、`google_free_is_keyless_and_built_without_credentials`、`registry_contains_google_free_keyless`、`map_lang_for_provider_google_free_uses_google_style_codes` 均 `... ok`。

## 二、变异 sanity（cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | parse 取错层级（segment[0]→[1]） | parse_concatenates_segments | 如期红，还原干净 |
| B | build_request 去掉 client=gtx | build_request_url | 如期红，还原干净 |
| C | google_free needs_key false→true | is_keyless / registry_contains_keyless | 两测试均如期红，还原干净 |

三处全红，特征实现的测试有真实判别力。

## 三、打回-修复闭环（tester 抓到的真实问题）
tester 全量 cargo test 发现 **2 个 TV1-F1 旧测试因新增 google_free 而失败**（注册表 4→5 家；"非 lingva 即 needs_key" 假设对免key源不成立）——coder 漏同步。**打回 coder**。
修复（仅改 tests/translate.rs 2 处，未动 google_free 实现）：
- `static_registry_lists_four_providers`→`..._five_providers`，断言 len=5。
- `static_registry_keyed_providers_need_key`：改为维护免key id 集合 `["lingva","google_free"]`，断言集合内 needs_key=false、集合外=true（后续新增免key源往此集合补）。

## 四、边界探测
- 多分句拼接 `[["Hello",..],["World",..]]`→"HelloWorld"（与 Google 实测一致，不补分隔符）。
- 异常响应：空 result[0]/顶层非数组/非法 JSON/分句缺译文/数值非字符串 → 全部 ParseError，无 panic。
- 既有官方 `google`(needs_key=true) 未被破坏（registry 仍含且行为不变）。
- providers.rs 无 eprintln，不泄露待译文本/译文（TV1-A-SEC）。

## 五、抗 flaky
修复后全量 `cargo test` 连跑 3 次均 `168 passed; 0 failed`；clippy `-D warnings` exit 0。

## 门禁结论：**通过（放行）**
特征实现正确、5 新测试有判别力、变异全红、边界安全、打回的旧测试已修、全量连跑 3× 绿、工作区干净。
