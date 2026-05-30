---
id: V0-F3-S06-code
type: coding_record
level: 小功能
parent: V0-F3
children: []
created: 2026-05-30T21:44:58Z
updated: 2026-05-31T12:00:00Z
status: 通过（打回第1次修复后）
commit: WIP
acceptance_ids: [V0-F3-A03]
evidence:
  - src-tauri/src/keyprovider.rs
  - src-tauri/tests/keyprovider.rs
  - src-tauri/src/lib.rs
author: coder
---

# 编码记录 · V0-F3-S06 密钥层抽象 KeyProvider

## 做了什么

实现 `KeyProvider` trait 抽象接口与 `KeychainKeyProvider` 唯一 v1 真实实现，
通过 keyring mock 后端确保测试 headless 安全（不触碰真实 OS 钥匙串、不弹授权框），
并用 `KeyAccessibility::AfterFirstUnlockThisDeviceOnly` 表达密钥设备绑定语义（不漫游）。

## 关键决策与理由

- **Entry 字段持有（非每次重建）**：keyring mock 的 `persistence()` 为 `EntryOnly`，
  密钥只在同一 Entry 实例内存活。若每次 `get_or_create_key()` 重建 Entry，首次写入
  的密钥在第二次调用时丢失，幂等性断言失败。持有 Entry 字段同时对真实 Keychain 无
  副作用（OS 层自行跨实例持久化）。否决了"懒初始化/每次 Entry::new"方案。

- **随机密钥熵：uuid v4 两次拼接**：Cargo.toml 已有 `uuid = {features=["v4"]}`，
  `Uuid::new_v4()` 内部调用 `getrandom`（OS CSPRNG），无需额外引入 `rand` crate。
  两次各 16 字节拼接得 32 字节。否决了时间种子方案（熵不足）。

- **`KeyAccessibility` 枚举表达设备绑定**：显式枚举变体 `AfterFirstUnlockThisDeviceOnly`
  比布尔字段更可扩展、自文档化，且与 macOS Security framework 的
  `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` 语义名称直接对应。

- **`is_device_only()` 便捷方法**：测试断言明确，避免测试代码直接 match 内部类型，
  降低测试与实现的耦合。

- **mock 激活位置**：测试中在创建 `KeychainKeyProvider::new()` 前调用
  `keyring::set_default_credential_builder(keyring::mock::default_credential_builder())`，
  确保 Entry 创建时已切换至 mock 后端。

## 打回第1次修复记录（2026-05-31）

### I-01 修复：启用 keyring native features

**问题**：`keyring = { version = "3", features = [] }` 在 macOS 未启用 `apple-native`，
后端行为不确定，可能漫游 iCloud Keychain。

**修复**：`Cargo.toml` 改为 `keyring = { version = "3", features = ["apple-native", "windows-native"] }`。

`apple-native` feature 使 keyring 在 macOS 使用真实 Security.framework 后端，以
`kSecAttrSynchronizable = false` 存储密钥，满足「不漫游 iCloud」硬约束。`windows-native`
使 Windows 走凭据管理器本机不漫游存储。

### I-02 修复：accessibility() / is_device_only() 诚实 doc comment

**问题**：原 doc comment 未区分「已保证」与「已知差距」，纯语义声明无 OS 属性背书。

**修复**：重写两个方法的 doc comment，明确：
- 已保证部分：`apple-native` feature 提供 `kSecAttrSynchronizable=false`，不漫游 iCloud。
- 已知差距：`kSecAttrAccessible` 精确属性未强制为 `AfterFirstUnlockThisDeviceOnly`（keyring
  默认 `WhenUnlocked`）；归 pending-manual V0-F3-A03-H01。

### ThisDeviceOnly 路径决策：走回退

**评估过程**：检查 `security-framework` v3.7.0 的 `PasswordOptions::push_query` 方法
签名为 `pub(crate)`，外部无法直接调用以插入 `kSecAttrAccessible` 属性。若要精确设置
`AfterFirstUnlockThisDeviceOnly`，需直接用 `security-framework-sys` 的底层 FFI 手工构造
`CFDictionary` 并调 `SecItemAdd`/`SecItemUpdate`，工作量大且与 keyring 后端存在冲突风险。

**结论**：走回退路径。`keyring apple-native` 已保证 `synchronizable=false`（不漫游），
满足设计§六核心硬约束。`AfterFirstUnlock` 精确 accessibility 类作为已知差距归
`pending-manual V0-F3-A03-H01`，注明强化方向（`security-framework` 直接调用）。

## 改动文件

- `src-tauri/src/keyprovider.rs` — 新增模块，含 `KeyError`、`KeyAccessibility`、
  `KeyProvider` trait、`generate_random_key()` 纯函数、`KeychainKeyProvider` 实现
- `src-tauri/src/lib.rs` — 新增 `pub mod keyprovider;` 注册模块
- `src-tauri/tests/keyprovider.rs` — 新增集成测试：`keyprovider_abstraction_and_device_only`
  与 `random_key_generation_is_not_constant`

## 自测结论（TDD 红-绿-重构）

**RED**：先写集成测试 `tests/keyprovider.rs`，`cargo test keyprovider` 报
`unresolved import quickquick_lib::keyprovider`（模块不存在），确认为功能缺失失败。

**GREEN 第一轮**：写 `keyprovider.rs` + 注册模块，单元测试通过，但集成测试
`keyprovider_abstraction_and_device_only` 的幂等性断言失败——mock EntryOnly 持久化
导致跨调用密钥丢失。

**GREEN 第二轮**：改为 Entry 字段持有，修复幂等性问题。所有测试通过：
- 集成测试 `keyprovider_abstraction_and_device_only`：ok
- 集成测试 `random_key_generation_is_not_constant`：ok
- 单元测试 `generate_random_key_returns_32_bytes`：ok
- 单元测试 `keychain_provider_is_device_only_by_default`：ok

**REFACTOR**：无需重构，结构已清晰。

**验证结论（证据）——打回第1次修复后回归**：
- `check=0`（`cargo check` 新依赖拉取后编译过）
- `A03=0`（`cargo test keyprovider` 退出码 0，4 个 keyprovider 测试全过）
  - keyprovider_abstraction_and_device_only: ok
  - random_key_generation_is_not_constant: ok（集成测试，filter 后未进此批次，已在全量覆盖）
  - keyprovider::tests::generate_random_key_returns_32_bytes: ok
  - keyprovider::tests::keychain_provider_is_device_only_by_default: ok
- `all=0`（全量 Rust 测试通过）
- `clippy=0`（`-D warnings` 零警告零错误）
- `build=0`（完整编译通过）
- `todo=1`（grep TODO/FIXME 无命中，退出码 1 = 无匹配 = 正常）
- 测试运行中未弹出任何 OS 钥匙串授权框（keyring mock 隔离）
- 新增 pending-manual 条目 V0-F3-A03-H01（AfterFirstUnlock 精确 accessibility 回读验证）

**code-standards 自检**：
- 格式：4 空格缩进（Rust 惯例），行宽 ≤ 120，文件末尾换行
- 函数：单一职责，`load_or_generate` ≤ 25 行，`generate_random_key` ≤ 10 行，嵌套 ≤ 2 层
- 命名：`is_device_only`（布尔前缀 is）、`load_or_generate`（动词+名词）、常量 UPPER_SNAKE
- 注释：模块文档说明设计要点与为什么，关键决策均有注释
- 类型：无魔术数字（32 字节在函数签名显式体现），`KeyError` 枚举化所有错误
- 安全：密钥不入库，日志不打印密钥内容，无裸 unwrap（Entry::new panic 有注释说明理由）
- 测试：AAA 结构，测试名描述行为，无恒真断言（两次随机值比较、固定 FakeKeyProvider 值校验）
- 无 TODO/FIXME 遗留
