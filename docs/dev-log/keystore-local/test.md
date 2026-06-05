# 密钥库改造 测试报告（动态证伪）

> feature-dev Phase 6 · tester 动态证伪（含一次 maxTurns 续跑）。tester 无 Write，本报告由编排器据其结构化结论落盘。

## 一、命中校验（杀假绿）
9 个安全关键测试 debug+release 各连跑 3× 均 `... ok`（N≥1）：
`different_machine_id_fails_to_decrypt` / `local_provider_different_machine_fails_decrypt`、`fallback_machine_id_still_opens_keystore`、`corrupt_file_reports_error_not_panic`、`master_key_file_has_0600_permissions`、`first_call_creates_master_key_file`、`new_instance_reads_same_key_with_same_machine_id`、`db_cred_store_set_get_roundtrip`、`save_then_display_reports_secret_present_without_plaintext`、`ensure_schema_drops_retired_secret_presence_table`。

## 二、变异 sanity（安全关键，cp 备份还原，禁 git checkout）
| 变异 | 改坏点 | 对应测试 | 结果 |
|---|---|---|---|
| A 机器绑定失效 | derive_kek 忽略 machine_id（固定字节） | different_machine_id_fails_to_decrypt（两个） | 如期红 |
| B 降级回退破坏 | machine_id() 取不到改 panic | fallback_machine_id_still_opens_keystore | **未红（覆盖缺口）**——测试用 with_machine_id 注入绕过真实 machine_id()，macOS ioreg 总成功，panic 分支不可达 |
| C 明文泄露 | display 回 secret 明文 | save_then_display_..._without_plaintext（集成+内联） | 如期红 |
| D 路由错 | DbCredStore 存进不存在的表 | db_cred_store_set_get_roundtrip | 如期红 |
| E 0600 权限 | 建文件权限改 0644 | master_key_file_has_0600_permissions | 如期红 |

A/C/D/E 四个安全点全如期红、有判别力；**B 覆盖缺口**：真实平台 machine_id 取失败→回退路径未被直接测试覆盖（降级逻辑代码本身正确 `unwrap_or_else(FALLBACK)`，但缺判别力测试）。**待补**：把平台读取函数也设计成可注入，加一条「强制 machine_id() 返回 None → 用 FALLBACK → 仍能开库」的判别力测试。

## 三、边界探测（安全）
- 密钥不入日志：keyprovider.rs/credential.rs grep eprintln/println/log/dbg **零匹配**。
- 生产路径无裸 panic：read_platform_machine_id 全 `.ok()?` + Option 链；唯一解包是安全回退 unwrap_or_else(FALLBACK)。
- 损坏文件：corrupt_file 测试断言 Err(Format|Decrypt)，走 Result 不 panic（debug+release 均 ok）。
- 全仓残留：keyring/KeychainKeyProvider/FileCredStore/apply_dev_subdir src+tests 零残留；secret_presence 仅 db.rs 退役迁移 + provider_secret.rs「验证已退役」测试刻意保留。

## 四、debug + release 双绿
debug `cargo test` 连跑 3× 均 lib 182 passed/0 failed + 各集成套件 0 failed；release `cargo test --release` 连跑 3× 全绿（证去 cfg gate 痛点消除）；clippy `-D warnings` exit 0。

## 五、工作区一致性
开工/结束 `git status --porcelain` 逐行一致，5 变异经 cp 还原（MD5 核验一致），未用 git checkout/restore。

## 门禁结论：**通过（放行）**，附 1 项非阻塞覆盖缺口（变异 B 真实回退路径）待补判别力测试。
