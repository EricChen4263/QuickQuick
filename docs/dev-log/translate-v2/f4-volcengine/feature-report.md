---
id: TV2-F4-report
type: feature_report
level: 大功能
parent: TV2
children: [TV2-F4-S01-code, TV2-F4-S01-test, TV2-F4-S01-review]
created: 2026-06-06T00:00:00Z
status: 通过
commit: 29bae0c
acceptance_ids: [TV2-F4-A01, TV2-F5-A01]
---

# TV2-F4 大功能验收报告：火山 SigV4（keyed）

## 小功能
| 小功能 | 内容 | 状态 | 三联 |
|---|---|---|---|
| TV2-F4-S01 | VolcengineProvider（AWS SigV4 四层 HMAC-SHA256 派生）+ schema/映射，复用 F3 工具 | 通过 | coding/test/review 齐 |

## 验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| TV2-F4-A01 | **pass** | SigV4 签名确定性（锚定具体 hex，Python 独立按官方文档复算一致）+ build/parse 命中 + 变异 A/B 红 |
| TV2-F5-A01（火山 schema，至此 7 源 schema 全齐） | **pass** | credential_schema_for_v2_keyed_sources 覆盖全 7 源 + 变异 C 红 |

## 门禁
tester 通过（7 命中 + 变异 A-D 全红 + 签名锚定具体值 + parse 安全 + 无泄密 + debug/release 226 passed + clippy 0）、code-reviewer APPROVE（Python 复算 SigV4 一致、CanonicalHeaders 双换行经核为 AWS SigV4 正确写法、无 Critical/Important）、未抄 pot、既有源未改、复用 F3 工具 DRY。

## TV2-F5（凭据 schema 横切）说明
TV2-F5-A01 为跨源验收项，各源 credential_schema 在 F1–F4 各自添加，credential_schema_for_v2_keyed_sources 测试累积覆盖全部 7 源（baidu_field/youdao/caiyun/niutrans/tencent/alibaba/volcengine），至 F4 闭合即整体满足，无独立 F5 feature 目录。

## 结论：**通过**（A01/F5-A01 objective pass；真译 manual 待用户密钥采证）。
