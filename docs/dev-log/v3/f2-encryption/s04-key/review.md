---
id: V3-F2-S04-review
type: review
level: 小功能
parent: V3-F2
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A05]
evidence: []
author: code-reviewer
---

# 审查结论 · V3-F2-S04 密钥可访问性 AfterFirstUnlock + ThisDeviceOnly

## 审查范围
- `src-tauri/src/keyprovider.rs`（新增 KeyStorageAttributes + is_after_first_unlock + storage_attributes；accessibility/is_device_only V0 既有）+ `tests/keyprovider.rs`（key_accessibility_flags）
依据：code-standards（密钥不入日志）+ 设计§六#2。

## 问题清单
### Critical
无。
### Important
**[I-01] 测试断言绕过被测方法（置信度 85）**
- 位置：`tests/keyprovider.rs` 第 109 行用 `KeyStorageAttributes::default()` 而非 `provider.storage_attributes()`，实际验证 Default impl 而非 provider 配置意图接口；若 storage_attributes() 实现被改动测试不感知。
- 修复：改 `let attrs = provider.storage_attributes();`（单行）。

## AfterFirstUnlock OS 落地判断
- **ThisDeviceOnly 不漫游**：真实 OS 落地（keyring apple-native synchronizable=false 写入 Security.framework，设计§六核心硬约束满足）。
- **AfterFirstUnlock 精确 accessibility 类**：未真正 OS 落地（keyring 默认 WhenUnlocked），KeyStorageAttributes 为配置意图非 OS 回读。**处置可接受**：accessibility() doc comment 已诚实区分"已保证/已知差距"，差距归 pending-manual V0-F3-A03-H01，强化路径（security-framework）已标注；headless CI 无法验真实 SecItemAdd 是客观约束，意图声明+诚实标注为合理降级，非误导性安全保证。当前阶段无需 security-framework 真实落地。

## code-standards 符合性
密钥不入日志（Debug 仅含 accessibility_id/synchronizable 非密钥；KeychainKeyProvider 无 Debug）✓；无裸 unwrap（expect 附说明）✓；禁装饰注释 ✓；函数 ≤15 行 ✓；命名规范 ✓；无 TODO ✓；测试 headless mock 不弹窗、断言值有判别力（非 WhenUnlocked、synchronizable=false）✓（I-01 修复后）。

## 结论
**未过（打回，单行修复）。** 修 I-01（测试改用 provider.storage_attributes()）后通过。AfterFirstUnlock OS 落地差距已诚实标注+pending，可接受。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-01 已解决**：`tests/keyprovider.rs` 第 108 行改为 `let attrs = provider.storage_attributes();`（覆盖被测方法路径，非旁路 Default），断言内容（accessibility_id=="AfterFirstUnlockThisDeviceOnly"、synchronizable==false）不变，key_accessibility_flags 真命中 pass。
- AfterFirstUnlock OS 落地差距按上轮判定可接受（ThisDeviceOnly 不漫游真实满足；AfterFirstUnlock 诚实标注 + pending V0-F3-A03-H01；headless 无法验真实 SecItemAdd 为客观约束）。
（注：单行修复为 reviewer 上轮明确指定的确切改法，编排器只读核对改动正确 + 测试真命中后确认。）
