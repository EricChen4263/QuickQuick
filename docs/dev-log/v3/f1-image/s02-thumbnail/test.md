---
id: V3-F1-S02-test
type: test_report
level: 小功能
parent: V3-F1
created: 2026-05-31T02:26:59Z
status: 通过
commit: WIP
acceptance_ids: [V3-F1-A02, V3-F1-A03]
author: tester
---

# 测试报告：V3-F1-S02 缩略图生成（修复 C-01/I-01/I-02）

## 1. 执行命令与结果

| # | 命令 | exit | 结论 |
|---|------|------|------|
| 1 | `cargo test --manifest-path src-tauri/Cargo.toml thumbnail` | 0 | 通过 |
| 2 | `cargo test --manifest-path src-tauri/Cargo.toml oversize` | 0 | 通过 |
| 3 | `cargo test --manifest-path src-tauri/Cargo.toml --test image` | 0 | 通过 |
| 4 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 0 | 零警告 |

## 2. 验收用例映射表

| 验收 ID | assertion 摘要 | 测试用例 | runner | 结果 |
|---------|---------------|---------|--------|------|
| V3-F1-A02 | WebP 缩略图规格：256 px、格式正确、文件大小在合理范围 | `thumbnail_spec_webp_256_format_and_size` | `cargo test … thumbnail` | **通过** |
| V3-F1-A02 | 损坏字节输入时 make_thumbnail 返回 Err，不 panic | `make_thumbnail_returns_err_on_corrupt_bytes` | `cargo test … thumbnail` / `--test image` | **通过** |
| V3-F1-A03 | oversize 跳过原图策略可配置（超大图跳过原文件存储） | `oversize_skip_original_policy_configurable` | `cargo test … oversize` / `--test image` | **通过** |

2 / 2 验收条目全部通过。

## 3. 测试用例详情（--test image 集成套件）

运行命令：`cargo test --manifest-path src-tauri/Cargo.toml --test image`

共 4 个测试，全部通过，耗时 0.14s。

| 序号 | 用例名 | 说明 | 结果 |
|------|--------|------|------|
| 1 | `make_thumbnail_returns_err_on_corrupt_bytes` | 传入损坏字节序列，断言返回 Err（不 panic） | **ok** |
| 2 | `image_capture_lossless_split_insert_dedup_and_different` | 图像无损捕获、分割、插入、去重及差异检测端到端 | **ok** |
| 3 | `oversize_skip_original_policy_configurable` | 超大图跳过原图策略配置化 | **ok** |
| 4 | `thumbnail_spec_webp_256_format_and_size` | WebP 缩略图规格：256 px 宽高、格式字节头、文件大小合理 | **ok** |

test result: **ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out**

## 4. Clippy 静态检查

命令：`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

exit code 0，**零警告，零错误**。

## 5. 覆盖缺口

无缺口。

- A02 由两条用例覆盖：正常路径（`thumbnail_spec_webp_256_format_and_size`）与错误路径（`make_thumbnail_returns_err_on_corrupt_bytes`，损坏字节返回 Err）。
- A03 由 `oversize_skip_original_policy_configurable` 覆盖可配置策略分支。
- 修复目标 C-01/I-01/I-02 均已通过对应测试验证。

## 6. 结论

**门禁：放行。**

A02 通过（2 用例）、A03 通过（1 用例），共 2/2 验收条目；集成套件 4/4 全绿；clippy 零警告。S02 缩略图生成可进入下一任务。
