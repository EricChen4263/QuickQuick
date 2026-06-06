---
id: TV2-F3-report
type: feature_report
level: 大功能
parent: TV2
children: [TV2-F3-S01-code, TV2-F3-S01-test, TV2-F3-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F3-A01]
---

# TV2-F3 大功能验收报告：腾讯 TC3 + 阿里 HMAC（keyed）

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV2-F3-S01 | TencentProvider(TC3-HMAC-SHA256 三层派生) + AlibabaProvider(HMAC-SHA1+Base64) + 自实现 UTC 公历换算 + schema/映射 | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV2-F3-A01 | **pass** | tencent/alibaba 签名确定性（锚定具体 hex/Base64，独立 Python 按官方文档手算交叉核对）+ build/parse 命中 + 变异 A-D 红 |
| TV2-F5-A01（贡献 tencent/alibaba schema） | 部分（待 F4 补齐后整体裁决） | schema 测试命中 + 变异 E 红 + 缺字段不泄密 |

## 门禁
tester 动态证伪通过（10 命中 + 变异 A-E 全红 + 签名锚定具体值 + parse 安全 + 无泄密 + debug/release 双绿 + clippy 0）、code-reviewer APPROVE（Python 独立重算 TC3/阿里签名一致、UTC 算法闰年边界核对、无 Critical/Important）、未抄 pot（按腾讯云/阿里云官方文档）、既有源未改。新增 hmac/sha1 依赖。

## 结论：**通过**（A01 objective pass；真译 manual 待用户密钥采证）。
