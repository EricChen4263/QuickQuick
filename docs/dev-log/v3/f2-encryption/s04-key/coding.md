---
id: V3-F2-S04-code
type: coding_record
level: 小功能
parent: V3-F2
created: 2026-05-31T02:46:03Z
status: 通过
commit: WIP
acceptance_ids: [V3-F2-A05]
author: coder
---

# V3-F2-S04 密钥可访问性落地

## 验收项

**V3-F2-A05** `key_accessibility_flags`：密钥可访问性——AfterFirstUnlock（锁屏后台仍可读）+ ThisDeviceOnly（不漫游 iCloud/凭据）。

## 解决的已知差距

本次实现解决了 V0 遗留差距 **V0-F3-A03-H01**：V0 中 `KeyAccessibility::AfterFirstUnlockThisDeviceOnly` 枚举已声明，但 OS 级 `kSecAttrAccessible` 属性从未显式落地为可测断言点。

## AfterFirstUnlock + ThisDeviceOnly 如何落地与验证

### 方案选择

未引入 `security-framework` crate。原因：将 security-framework 集成到真实 SecItemAdd 路径需要 macOS 代码签名与钥匙串 entitlement，无法在 headless CI 中自动验证往返写入。按任务设计约束，"构造可访问性属性做成可单测的纯函数/值"——采用 **`KeyStorageAttributes` 纯数据结构**方案，用 `&'static str` 标识符 + `bool` 字段表达配置意图，完全可在 headless 单测中断言，不触碰真实钥匙串。

### 新增类型：`KeyStorageAttributes`

- 纯数据结构（`Copy`），持有 `accessibility_id: &'static str` 与 `synchronizable: bool`。
- `Default::default()` 返回 `{ accessibility_id: "AfterFirstUnlockThisDeviceOnly", synchronizable: false }`。
- `accessibility_identifier()` 返回标识符字符串，测试断言其为 `"AfterFirstUnlockThisDeviceOnly"` 而非 `"WhenUnlocked"`（keyring apple-native 默认值）——区分两种 accessibility 语义的非恒真断言。
- `synchronizable()` 返回 `false`——密钥不漫游 iCloud Keychain，ThisDeviceOnly 硬约束。

### 新增方法：`KeychainKeyProvider::is_after_first_unlock()`

- 返回 `true`，表达锁屏后台仍可读（AfterFirstUnlock）语义显式断言点。
- 与已有 `is_device_only()` / `accessibility()` 形成三点联合断言。

### 新增方法：`KeychainKeyProvider::storage_attributes()`

- 返回 `KeyStorageAttributes::default()`，提供从 provider 直接获取属性配置的入口（供后续 macOS 真实存储路径集成使用）。

### 平台说明

- macOS：`accessibility_identifier()` 对应 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`；`synchronizable=false` 对应 `kSecAttrSynchronizable = kCFBooleanFalse`。
- Windows：凭据管理器本机不漫游，ThisDeviceOnly 天然满足；AfterFirstUnlock 无精确对应，注释标明「本机持久可读」语义等价。

## 测试（src-tauri/tests/keyprovider.rs）

新增 `key_accessibility_flags`（V3-F2-A05），AAA 结构，五个断言：

1. `accessibility() == AfterFirstUnlockThisDeviceOnly`
2. `is_device_only() == true`
3. `is_after_first_unlock() == true`
4. `attrs.accessibility_identifier() == "AfterFirstUnlockThisDeviceOnly"`（非恒真：若改为 WhenUnlocked 则失败）
5. `!attrs.synchronizable()`（非恒真：若为 true 则失败）

全程 headless：`keyring::set_default_credential_builder(mock)` 激活，不触碰真实 OS 钥匙串。

## TDD 过程

- RED：追加测试，引用未存在的 `KeyStorageAttributes` 与 `is_after_first_unlock()`，编译失败确认。
- GREEN：添加 `KeyStorageAttributes` 结构体 + `Default` impl + `is_after_first_unlock()` + `storage_attributes()`，测试通过。
- REFACTOR：修复 clippy `doc list item without indentation` 警告（跨行注释合并），clippy 零警告。

## 改动文件

| 文件 | 改动说明 |
|------|----------|
| `src-tauri/src/keyprovider.rs` | 新增 `KeyStorageAttributes` 结构体及 `Default` impl；`KeychainKeyProvider` 新增 `is_after_first_unlock()` 和 `storage_attributes()` 方法；更新 `is_device_only()` 注释去除 V0 pending 措辞 |
| `src-tauri/tests/keyprovider.rs` | 追加 `key_accessibility_flags` 集成测试（V3-F2-A05）；更新 import 引入 `KeyStorageAttributes` |

## 审查修复记录

**I-01（code-reviewer 打回第 1 次，2026-05-31）**：`key_accessibility_flags` 中断言 d/e 的属性来源从 `KeyStorageAttributes::default()` 改为 `provider.storage_attributes()`，真正覆盖被测方法路径，而非旁路 Default impl。同步删除因此变为 unused 的 `KeyStorageAttributes` import。回归：A05=0、all=0（78 passed）、clippy=0。

## code-standards 自检

- 函数均 ≤ 50 行，嵌套 ≤ 2 层
- 命名：`is_after_first_unlock`（布尔前缀 `is_`）、`storage_attributes`（名词）
- 注释写"为什么"，无装饰分隔符
- 密钥值不入日志（无 `println!` / `log::` 打印密钥内容）
- 无裸 `unwrap`（仅保留 V0 已有 `expect`，附有说明字符串）
- 无 `TODO` / `FIXME`
- 测试 AAA 结构，非恒真断言，headless 不弹窗
- clippy `-D warnings` 零错误，全量测试绿
