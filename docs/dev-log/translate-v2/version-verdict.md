---
id: TV2-verdict
type: version_verdict
level: 版本
parent: TV2
created: 2026-06-06T00:00:00Z
status: 条件性通过
commit: 29bae0c
anchor:
  criteria_freeze: TV2-criteria@2026-06-06
---

# TV2 版本裁决报告（独立 producer）

> 由独立 producer（只读、无写权）核对冻结 acceptance、独立重跑命中校验、git 前后快照一致后产出；编排器据其结构化结论落盘。含异构裁判（codex）交叉复核。

## 裁决锚
criteria_freeze=`TV2-criteria@2026-06-06`；受验代码 commit=`29bae0c`；git 裁决前后快照逐行一致（producer 未引入改动；codex exec 自动生成的 `src-tauri/AGENTS.md` 副产物已移除以恢复一致，对受验业务/标准/测试文件零改动）。

## change_log 合法性
`change_log: []`——本版未改动冻结标准。两处落地偏差已对齐、非移动球门：① TV2-F5-A01 evidence_ref 指向 `f5-credentials/`，但 F5 是横切验收项已并入 F1–F4（无独立目录，`credential_schema_for_v2_keyed_sources` 累积覆盖全 7 源）；② acceptance 聚合名 `build_provider_missing_required_field_errors` 落地为各源独立函数（`build_provider_<源>_missing_*_returns_err`）。两者均为命名/组织层对齐，未放宽断言。

## 逐项对照表
| acceptance_id | result | 证据 |
|---|---|---|
| TV2-F1-A01 | pass | `baidu_field_sign_and_build ... ok`、`baidu_field_parse ... ok`、`youdao_sign_v3_and_build ... ok`、`youdao_parse ... ok`（各 1 passed） |
| TV2-F2-A01 | pass | `caiyun_build_and_parse ... ok`、`niutrans_build_and_parse ... ok` |
| TV2-F3-A01 | pass | `tencent_tc3_signature_deterministic ... ok`、`tencent_build_and_parse ... ok`、`alibaba_hmac_signature_deterministic ... ok`、`alibaba_build_and_parse ... ok` |
| TV2-F4-A01 | pass | `volcengine_sigv4_signature_deterministic ... ok`、`volcengine_build_and_parse ... ok` |
| TV2-F5-A01 | pass | `credential_schema_for_v2_keyed_sources ... ok` + 7 源 `build_provider_{baidu_field/youdao/caiyun/niutrans/tencent/alibaba/volcengine}_missing_*_returns_err ... ok` 全命中 |
| TV2-A-TEST | pass | debug 连跑 3× 主套 `test result: ok. 226 passed; 0 failed`；release 连跑 3× `226 passed; 0 failed`；无 FAILED 行、无 flaky |
| TV2-A-QUAL | pass | `cargo clippy --all-targets -- -D warnings` exit 0 无 warning；`grep TODO\|FIXME src/translate/` 无输出 |
| TV2-A-SEC | pass | `grep eprintln\|println\|log::\|dbg! src/translate/providers.rs` 无输出；7 源 needs_key=true / is_unofficial=false（已命中测试断言覆盖） |
| TV2-A-LOG | pass | f1–f4 各有 coding/test/review/feature-report，acceptance 在根，commit 已回填 `29bae0c`（非 PENDING） |
| TV2-F1234-M01 | 未决（待采证） | manual_confirm/real_device，真网需用户各源 API key，headless 采不了 → 并入 pending |

命中校验：所有过滤命令均确认目标测试真被跑到（`... ok` 行且 N passed N≥1），无空匹配假绿。

## coverage_check
功能正确性/测试充分性/工程质量/安全/留痕产出/人工确认点 covered（各有匹配 category 条目，无空洞）；性能/UI还原度/资源规范 N/A（合理：仅新增源调用与签名无性能目标、复用既有凭据表单/源选择器、不新增二进制资源）。完整性校验通过。

## rejections
无 fail、无打回。F2-S01 reviewer 提 2 项测试充分性 Important（缺字段测试用空凭据无判别力 / 缺错误类型断言）已补强闭合，非熔断。本版无任一小功能打回到 3 次上限。

## pending_manual
TV2-F1234-M01 → 待采证（并入全局 pending-manual.yaml，不阻塞 done）。

## cross_judge
engine: codex（codex-cli 0.137.0，read-only）。对全部 8 个 objective 项独立复核逐项判 pass，dissent: []。

## verdict：**条件性通过（版本 done）**
blocking: [] 。8 个客观项全 pass、覆盖完整（含完整性校验）、无熔断、git 前后一致、dissent 空。manual 项（TV2-F1234-M01）待采证不阻塞。
