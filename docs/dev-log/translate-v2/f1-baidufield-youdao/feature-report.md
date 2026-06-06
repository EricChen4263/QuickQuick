---
id: TV2-F1-report
type: feature_report
level: 大功能
parent: TV2
children: [TV2-F1-S01-code, TV2-F1-S01-test, TV2-F1-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV2-F1-A01]
---

# TV2-F1 大功能验收报告：百度专业 + 有道（keyed）

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV2-F1-S01 | BaiduFieldProvider(MD5+field) + YoudaoProvider(SHA256 v3+truncate) + 凭据 schema + 语言映射 | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV2-F1-A01 | **pass** | baidu_field/youdao 各 sign+build+parse 测试命中 + 变异 A/B/C 红；签名手算三方交叉验证 |
| TV2-F5-A01（贡献 baidu_field/youdao schema 部分） | 部分（待 F2-F4 补齐 7 源后整体裁决） | credential_schema_for_v2_keyed_sources 命中 + 变异 D/E 红 |

## 门禁
tester 动态证伪通过（8/8 命中 + 变异 A-E 全红 + 边界安全 + debug/release 各 3× 绿 + clippy 0）、code-reviewer APPROVE（签名手算交叉验证、无 Critical/Important）、未抄 pot（签名按百度/有道官方文档）、未动既有源（baidu_sign 仍 4 参）。

## 结论：**通过**（A01 objective pass；真译 manual 待用户密钥采证）。
