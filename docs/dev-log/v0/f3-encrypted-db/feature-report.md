---
id: V0-F3-report
type: feature_report
level: 大功能
parent: V0
children: [V0-F3-S05-code, V0-F3-S05-test, V0-F3-S05-review, V0-F3-S06-code, V0-F3-S06-test, V0-F3-S06-review]
created: 2026-05-31T12:30:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F3-A01, V0-F3-A02, V0-F3-A03, V0-F3-A04, V0-F3-A05, V0-F3-A06]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · V0-F3 加密数据库与 schema 预埋

## 引用的小功能（children）

| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| V0-F3-S06 KeyProvider 密钥层抽象 | [code](s06-keyprovider/coding.md) | [test](s06-keyprovider/test.md) | [review](s06-keyprovider/review.md) | 通过（reviewer 打回 2I 安全→复审通过；AfterFirstUnlock 差距入 pending-manual） |
| V0-F3-S05 加密DB+schema+软删GC+恢复 | [code](s05-db-init/coding.md) | [test](s05-db-init/test.md) | [review](s05-db-init/review.md) | 通过（reviewer 通过+3I 改进，polish 全修复核通过） |

> 实现顺序：S06（KeyProvider 接口）先行，S05 开库依赖其抽象（测试用固定 key 的 FakeKeyProvider，headless 不碰钥匙串）。

## 大功能级验收项对照

| 验收项 | 结果 | 证据 |
|---|---|---|
| V0-F3-A01 首次启动自动创建单文件 quickquick.db | pass | s05/test.md（db_create_auto_creates_file + 幂等） |
| V0-F3-A02 SQLCipher 加密（错密钥无法打开、密文落盘） | pass | s05/test.md（db_encrypt_wrong_key_returns_error + ciphertext_on_disk 头部非明文魔数） |
| V0-F3-A03 KeyProvider 抽象/v1 仅 Keychain/密钥 ThisDeviceOnly 不漫游 | pass | s06/test.md（keyprovider_abstraction_and_device_only）；不漫游 via keyring apple-native synchronizable=false 真实满足；AfterFirstUnlock 精确属性差距入 pending-manual V0-F3-A03-H01 |
| V0-F3-A04 schema 预埋（UUID+UTC created/last_modified+墓碑；图片表拆缩略图/原图） | pass | s05/test.md（schema_preembed_columns_clip_items/clip_images，PRAGMA table_info 逐列断言） |
| V0-F3-A05 软删+墓碑（非物理删）+ 本地物理清理 GC | pass | s05/test.md（soft_delete_and_gc 全生命周期 + 不误伤 live 行） |
| V0-F3-A06 库损坏改名备份保留、绝不静默删、显式确认才重建空库 | pass | s05/test.md（db_recovery 备份字节级校验 + allow_rebuild 门控） |

## 状态汇总

V0-F3 两个小功能（S05、S06）均 done。大功能级 6 个 objective 验收项全部 pass。schema 预埋满足设计§十铁律（UUID/UTC/墓碑/图片双 BLOB），并 polish 补齐外键约束+级联（§十预埋从严）。安全核心（SQLCipher 加密、密钥不漫游、永不静默删库）均真实落地。无熔断阻塞。manual 衍生项：V0-F3-A03-H01（AfterFirstUnlock 精确 OS 属性回读，需真实钥匙串人工验证，已入 pending-manual，不参与 done 判定）。大功能 **通过**。
