---
id: TV1-F1-S01-test
type: test_report
level: 小功能
parent: TV1-F1
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F1-A01, TV1-F1-A02, TV1-F1-A03]
evidence:
  - src-tauri/src/translate/providers.rs
  - src-tauri/src/ipc/translate.rs
  - docs/dev-log/translate-v1/f1-lingva-default/artifacts/cargo-test.log
---

# TV1-F1-S01 测试报告（动态证伪）

> tester 动态证伪，非橡皮图章复跑。tester 无 Write，本报告由编排器据其返回的结构化结论落盘。

## 一、命中校验（杀假绿）

三个冻结验收测试全部真实命中（N=1，非空匹配假绿、非 skip），原始行（`rtk proxy cargo test` 绕 RTK 代理取）：

| 验收项 | 测试名原始行 | 结果 |
|---|---|---|
| TV1-F1-A01 | `test translate::providers::tests::providers_registry_has_lingva_no_mymemory ... ok` | 1 passed |
| TV1-F1-A02 | `test translate::providers::tests::lingva_build_request_url_and_parse_translation ... ok` | 1 passed |
| TV1-F1-A03 | `test ipc::translate::tests::selected_provider_migrates_unknown_to_lingva ... ok` | 1 passed |

`test result: ok. 163 passed; 0 failed`（lib）。

## 二、变异 sanity（杀恒真/旁路，cp 备份还原，禁 git checkout）

| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A | `parse_response` 取 `v["text"]` 替代 `v["translation"]` | A02 | 如期红 `0 passed; 1 failed`，还原后无残留 |
| B | `capability.id` 改 `"mymemory"`（registry 含 mymemory 缺 lingva） | A01 | 如期红，还原后无残留 |
| C | `resolve_provider_or_fallback` 改为原样返回不回退 | A03 | 如期红，还原后无残留 |

三处全部如期变红，测试有真实判别力，非恒真/旁路。

## 三、边界探测

- **迁移不过度回退**：`resolve_provider_or_fallback("baidu")` 保持 `"baidu"`（A03 第 320 行已断言）；只有不在 registry 的 id（mymemory/garbage/空串）才回退 lingva。无误改写有效源。
- **URL 编码**：空格→`%20`、`&`→`%26`、`/`→`%2F`、`?`→`%3F`（大写十六进制）、中文 UTF-8 正确百分号编码、ASCII unreserved 不编码；`provider_lingva_build_request_url_path_encoding` 集成测试通过。

## 四、抗 flaky（全量连跑 3 次）

lib `cargo test` 连跑 3 次均 `163 passed; 0 failed`；前端 `pnpm test` 51 files / 462 passed / 0 failed。

## 五、工作区一致性

开工/结束 `git status --porcelain` 逐行一致，变异全经 cp 还原，无残留，未用 git checkout/restore。

## 门禁结论：**通过（放行）**

三验收测试真实命中、变异全红、迁移无过度回退、URL 编码正确、连跑无 flaky、工作区干净。
