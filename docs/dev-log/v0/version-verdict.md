---
id: V0-verdict
type: version_verdict
level: 版本
parent: null
children: [V0-F1-report, V0-F2-report, V0-F3-report]
created: 2026-05-31T13:00:00Z
status: 条件性通过
commit: 00371e3
acceptance_ids: []
evidence: []
author: producer
---

# 版本裁决报告 · V0（Phase 0 骨架）

> 由独立制作人 agent（只读 + 可执行验证，无 Write/Edit）产出。逐项独立重跑、有据可查。

## 逐项对照表（核心）

### V0-F1 项目脚手架与构建
| 验收项 | 结果 | 证据出处 | 备注 |
|---|---|---|---|
| V0-F1-A01 | pass | `cargo build` 退出码 0 | 重跑构建成功 |
| V0-F1-A02 | pass | `pnpm build` 退出码 0 | vite 打包成功 |
| V0-F1-A03 | pass | clippy 0 + tsc 0 + grep TODO/FIXME 退出 1（0 匹配） | 工程质量基线 |
| V0-F1-A04 | pass | `cargo test` 全量 ok + `pnpm test` 5 例（smoke 1 + windowRoute 4） | 后端/前端各≥1 实质单测 |
| V0-F1-A05 | pass | `cargo test autostart` 3 例（含 autostart_default_on，默认 enabled=true） | |
| V0-F1-A06 | pass | `grep -q updater tauri.conf.json` 退出 0 | updater 段存在 |

### V0-F2 托盘 + 全局热键 + 预热窗口
| 验收项 | 结果 | 证据出处 | 备注 |
|---|---|---|---|
| V0-F2-A01 | pass | `cargo test hotkey` → hotkey_defaults_and_rebind | 默认键+改键持久化 |
| V0-F2-A02 | pass | hotkey_conflict_rejected | 冲突拒绝保存、不崩溃 |
| V0-F2-A03 | pass | `pnpm test windowRoute`（CL-V0-001 校正后 runner）4 例 | V/T 路由单测 |
| V0-F2-A04 | 未决(manual) | pending-manual.yaml | GUI 体感项，功能已实现，headless 无法取证，不阻塞 done |
| V0-F2-A05 | pass | `ls icons/*.png *.ico *.icns` 退出 0（17 文件） | 图标资源齐 |

### V0-F3 加密数据库与 schema 预埋
| 验收项 | 结果 | 证据出处 | 备注 |
|---|---|---|---|
| V0-F3-A01 | pass | `cargo test db_create` 2 例（auto_creates_file + 幂等） | 首次自动创建单文件库 |
| V0-F3-A02 | pass | `cargo test db_encrypt` 2 例（错key报错 + 密文落盘头部非魔数） | SQLCipher 加密 |
| V0-F3-A03 | pass | `cargo test keyprovider` → keyprovider_abstraction_and_device_only | KeyProvider 抽象 + 不漫游(synchronizable=false)；AfterFirstUnlock 精确属性→pending V0-F3-A03-H01 |
| V0-F3-A04 | pass | `cargo test schema` → schema_preembed_columns_clip_items/clip_images | UUID/UTC/墓碑/图片双BLOB 逐列断言 |
| V0-F3-A05 | pass | `cargo test soft_delete` 2 例 | 软删墓碑非物理 + GC 物理清理 |
| V0-F3-A06 | pass | `cargo test db_recovery` 2 例 | 损坏改名备份不静默删 + allow_rebuild 门控 |

### 留痕产出（版本级）
| 验收项 | 结果 | 证据出处 | 备注 |
|---|---|---|---|
| V0-A-LOG | pass | 6 小功能各 coding/test/review 齐 + f1/f2/f3 各 feature-report.md | 三联留痕 + 大功能报告齐全 |

## 覆盖检查
| 类别 | 状态 |
|---|---|
| 功能正确性 | covered（有匹配项） |
| 测试充分性 | covered（V0-F1-A04） |
| 工程质量 | covered（V0-F1-A03） |
| 性能 | N/A（骨架阶段无量化阈值；瞬开归 V0-F2-A04） |
| UI还原度 | N/A（Phase 0 无正式 UI 面板） |
| 资源规范 | covered（V0-F2-A05） |
| 安全 | covered（V0-F3-A02/A03/A06） |
| 留痕产出 | covered（V0-A-LOG） |
| 人工确认点 | covered（V0-F2-A04） |

所有 covered 类别均有 ≥1 个 category 匹配条目，无空洞声明；N/A 类别有明确理由。

## 未决审美/人工项（并入全局 pending-manual.yaml，不阻塞）
- V0-F2-A04 — 托盘常驻/热键唤起活动屏上中/预热瞬开体感（GUI）
- V0-F2-A04-H01 — 托盘点击与失焦隐藏时序（运行期）
- V0-F2-A04-H02 — 高 DPI/Retina 定位偏移（运行期）
- V0-F3-A03-H01 — kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly 精确属性回读（真实钥匙串）

## 打回 / 熔断记录
| 小功能 | 打回次数 | 是否熔断阻塞 |
|---|---|---|
| V0-F1-S01 | 1（1C+4I→复审通过） | 否 |
| V0-F2-S02 | 1（1I→复审通过） | 否 |
| V0-F2-S03 | 1（4I→复审通过） | 否 |
| V0-F1-S04 | 1（2I→复审通过） | 否 |
| V0-F3-S05 | 0（3I 非阻塞 polish→复核通过） | 否 |
| V0-F3-S06 | 1（2I 安全→复审通过） | 否 |

全部 ≤1 次，均未达熔断上限 3，无阻塞。

## 总裁决
**条件性通过（= 版本完成 / done）**
- 阻塞项：无
- 全部 16 个 objective 验收项独立重跑均 pass；覆盖 9 类完整；打回均 ≤1 无熔断；git 前后逐行一致（裁决期间未下场改动）；4 个 manual_confirm 项并入全局未决清单滚动追踪，不改变 done 状态。

## 裁决锚（防对着旧标准/旧代码裁决）
- commit: `00371e3`（00371e36f9e38116512eb5db48b7b80427d0fe08）
- criteria_freeze: `V0-criteria@2026-05-31`（含 change_log CL-V0-001）

## 制作人"没下场"证据
- 裁决前后 git HEAD 与 `git status --porcelain` 逐行一致（before/after HEAD 均 00371e36…，状态行数均 0，diff 空）——证明裁决期间未引入任何改动（target/dist/node_modules 已 .gitignore，不计入）。
