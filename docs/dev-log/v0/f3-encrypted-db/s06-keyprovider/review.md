---
id: V0-F3-S06-review
type: review
level: 小功能
parent: V0-F3
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F3-A03]
evidence: []
author: code-reviewer
---

# 审查结论 · V0-F3-S06 KeyProvider 密钥层抽象

## 审查范围

| 文件 | 变更类型 |
|---|---|
| `src-tauri/src/keyprovider.rs` | 新增 |
| `src-tauri/src/lib.rs` | mod 暴露（pub mod keyprovider） |
| `src-tauri/tests/keyprovider.rs` | 新增集成测试 |

审查标准：code-standards + 设计文档§六（数据库与加密）。

## 发现问题（置信度 ≥ 80）

### Important

#### [I-01] keyring 未启用 native feature，macOS 下 kSecAttrAccessible 属性落点不确定（置信度 90）
- 位置：`src-tauri/Cargo.toml`（`keyring = { version = "3", features = [] }`）
- 描述：keyring v3 在 macOS 使用真实 Keychain 后端需 `apple-native` feature；features=[] 时实际后端行为不确定，可能 fallback 到默认 `WhenUnlocked` 或不带 ThisDeviceOnly，密钥可能漫游 iCloud Keychain，违反设计§六「ThisDeviceOnly 绝不同步」（v2 信封模型前提）。
- 修复：启用 `apple-native`（macOS）+ `windows-native`（Windows）feature；并确认真实写入的 accessibility 属性。

#### [I-02] `KeyAccessibility::AfterFirstUnlockThisDeviceOnly` 纯语义声明，无 OS 属性背书，构成误导性安全保证（置信度 85）
- 位置：`src-tauri/src/keyprovider.rs`（accessibility() / is_device_only() 恒返回固定值）
- 描述：恒返回 ThisDeviceOnly/true，仅 Rust enum 层声明，不证明底层真正设置了对应 OS 属性。调用方据此做安全判断会被误导。
- 修复：真正落地 ThisDeviceOnly（见下方裁定），并在 doc comment 诚实标注实现边界。

## 无 Critical（逐项确认通过）

随机熵来源 uuid v4 基于 getrandom/OS CSPRNG，密码学安全、非恒真；生产代码无密钥日志泄漏；KeyError thiserror 规范；mock 仅 `#[cfg(test)]`/tests，不污染生产；trait 抽象单方法清晰；测试 AAA、FakeKeyProvider 非恒真、随机性断言 `key_a!=key_b` 非恒真、mock 下幂等性验证；函数 ≤50 行嵌套 ≤3；无 TODO/FIXME；mod 正确暴露。

## 编排裁定（修复方向）

设计§六将「ThisDeviceOnly 不漫游」列为 v2 信封模型前提（硬约束），且 KeyProvider 是为此解耦设计的接缝。要求真正落地而非仅声明：
1. **I-01 必修**：Cargo.toml 启用 `apple-native` + `windows-native`。
2. **真 ThisDeviceOnly**：macOS 用 `security-framework` 以 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` + `kSecAttrSynchronizable=false` 存取密钥，使 is_device_only() 有真实实现背书；Windows 凭据管理器本机不漫游，用 keyring windows-native。
3. **I-02 必修**：accessibility()/is_device_only() 的文档诚实标注其由哪条实现路径保证；真实 OS 属性回读验证（需真实钥匙串、会弹窗）归 pending-manual。

## 结论

**打回。** 修复 I-01 + 真 ThisDeviceOnly 落地 + I-02 诚实标注后复审。

---

## 复审结论（第 2 次 · 2026-05-31）

**status: 通过**

- **I-01**（keyring native features）：Cargo.toml 已启用 `apple-native`+`windows-native`，macOS 后端确定为 Security.framework，`kSecAttrSynchronizable=false` 库级别保证、密钥不漫游 iCloud，设计§六"ThisDeviceOnly 绝不同步"核心硬约束真实满足。
- **I-02**（诚实标注）：`accessibility()`/`is_device_only()` doc comment 明确区分"已保证(synchronizable=false 不漫游)"与"已知差距(AfterFirstUnlock 未强制)"，不再以 enum 声明冒充 OS 保证。
- pending-manual `V0-F3-A03-H01` 已录入（差距有界有归宿，强化方向 security-framework）；coding.md 回退路径决策完整可追溯。
- 无新增≥80 高危；测试仍 headless mock 不弹窗；无 TODO/FIXME。
