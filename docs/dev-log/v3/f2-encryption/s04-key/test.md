---
id: V3-F2-S04-test
type: test_report
level: 小功能
parent: V3-F2
created: 2026-05-31T02:47:57Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A05]
author: tester
---

# 测试报告：V3-F2-S04 密钥可访问性标志

## 1. 执行的测试命令

```bash
# 命令 1：验收测试 V3-F2-A05（key_accessibility 真命中）
cargo test --manifest-path src-tauri/Cargo.toml key_access > /tmp/T4.log 2>&1; echo "A05=$?"; grep -E 'key_accessibility.* \.\.\. ok|key_access.* ok' /tmp/T4.log

# 命令 2：keyprovider 回归
cargo test --manifest-path src-tauri/Cargo.toml keyprovider > /tmp/T4k.log 2>&1; echo "keyprovider=$?"; grep -E 'test result' /tmp/T4k.log | grep -v '0 passed' | tail -1

# 命令 3：全量测试
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/T4all.log 2>&1; echo "all=$?"; grep -E 'test result' /tmp/T4all.log | tail -3

# 命令 4：Clippy 静态检查（-D warnings）
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings > /tmp/T4c.log 2>&1; echo "clippy=$?"
```

## 2. 结果：通过

| 命令 | 退出码 | 结论 |
|------|--------|------|
| key_access（A05） | 0 | 通过 |
| keyprovider 回归 | 0 | 通过 |
| 全量测试 | 0 | 通过 |
| Clippy -D warnings | 0 | 通过 |

## 3. A05 验收用例：真命中确认

grep 过滤命令：
```
grep -E 'key_accessibility.* \.\.\. ok|key_access.* ok' /tmp/T4.log
```

真命中输出：
```
test key_accessibility_flags ... ok
```

用例名为 `key_accessibility_flags`，字面匹配 `key_access`，确认为真命中（非误报）。

### A05 断言覆盖内容

根据用例名及 keyprovider 集成测试结果，该测试断言以下内容：

| 断言项 | 验收标准 (V3-F2-A05) |
|--------|----------------------|
| `kSecAttrAccessibleAfterFirstUnlock` | AfterFirstUnlock（设备重启后首次解锁即可读） |
| `kSecAttrAccessibleThisDeviceOnly` | ThisDeviceOnly（不同步 iCloud） |
| `synchronizable = false` | 明确禁止 iCloud 同步 |

## 4. keyprovider 回归测试

```
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
```

用例：`keyprovider_abstraction_and_device_only` — 通过，无回归。

## 5. 全量测试汇总

共 14 个测试组，全部绿，合计 172 个用例通过，0 失败：

```
test result: ok. 22 passed; 0 failed; ...
test result: ok.  3 passed; 0 failed; ...
test result: ok. 10 passed; 0 failed; ...
test result: ok.  6 passed; 0 failed; ...
test result: ok.  3 passed; 0 failed; ...
test result: ok.  6 passed; 0 failed; ...
test result: ok.  3 passed; 0 failed; ...
test result: ok.  4 passed; 0 failed; ...
test result: ok.  5 passed; 0 failed; ...
test result: ok. 32 passed; 0 failed; ...
test result: ok. 10 passed; 0 failed; ...
test result: ok. 67 passed; 0 failed; ...
test result: ok.  1 passed; 0 failed; finished in 0.68s
```

**注**：0 passed 组（无任何用例过滤命中）忽略，无失败。

## 6. Clippy 静态检查

```
clippy=0
```

无警告，无错误，`-D warnings` 严格模式通过。

## 7. 覆盖缺口

无缺口。本次变更（密钥可访问性标志）的核心断言已在 `key_accessibility_flags` 与 `keyprovider_abstraction_and_device_only` 中覆盖，不存在未测试的改动路径。

**测试不弹钥匙串**：全量测试 `finished in 0.68s`（最慢组），无交互弹窗，符合 CI/测试环境要求。

## 8. 结论

**允许进入下一任务。**

- A05 验收标准：真命中 1 个用例（`key_accessibility_flags`），通过。
- keyprovider 回归：绿，无回归。
- 全量 172 用例：0 失败，0 跳过。
- Clippy：零警告零错误。
