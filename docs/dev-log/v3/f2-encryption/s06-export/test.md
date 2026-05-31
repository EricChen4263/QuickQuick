---
id: V3-F2-S06-test
type: test_report
level: 小功能
parent: V3-F2
created: 2026-05-31T03:16:36Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A07]
author: tester
---

# V3-F2-S06 测试报告：导出/导入便携文件（口令保护）

## 执行命令

```bash
# 1. portable 集成测试（含 I-1/I-2 修复验证）
cargo test --manifest-path src-tauri/Cargo.toml --test portable

# 2. 全量单元+集成测试
cargo test --manifest-path src-tauri/Cargo.toml

# 3. Clippy 静态检查
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 测试结果

### portable 测试（5/5）

| 用例名 | 结果 | 覆盖场景 |
|---|---|---|
| `export_import_passphrase_roundtrip` | ok | A07：正常往返加密/解密一致 |
| `export_import_passphrase_wrong_passphrase_returns_err` | ok | A07：错口令返回 Err |
| `export_import_passphrase_ciphertext_does_not_contain_plaintext` | ok | A07：密文中不含明文（隔离验证） |
| `export_import_passphrase_truncated_blob_returns_format_err` | ok | A07：截断 blob 返回 FormatError（I-1 修复验证） |
| `export_produces_distinct_blobs_each_call` | ok | A07：同输入多次导出产出不同 blob（I-2 修复验证） |

```
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.40s
```

### 全量测试

```
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok.  1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
```

总计：78 passed，0 failed，0 ignored。

### Clippy

退出码 0，零警告，零错误。

## 覆盖缺口

无。A07 所有验收场景均有对应用例并全部通过：

- 往返正确性（roundtrip）
- 错口令拒绝（wrong_passphrase）
- 密文隔离（ciphertext_does_not_contain_plaintext）
- 截断格式校验（truncated_blob，覆盖 I-1）
- 随机化 blob 唯一性（distinct_blobs，覆盖 I-2）

全量 78 个用例，无跳过。

## 结论

**通过。允许进入下一任务。**

- portable 5/5 ok（含 I-1、I-2 修复验证）
- 全量：78 passed，0 failed
- Clippy：退出码 0，零警告
