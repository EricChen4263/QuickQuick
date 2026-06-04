---
id: F6-S09-review
type: review
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · secret_presence 展示路径 + codesign runner（F6-S09）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/db.rs` | `ensure_schema` 新增 `secret_presence` 表 DDL |
| `src-tauri/src/translate/credential.rs` | `load_credentials_for_display`、`write_secret_marker`、`delete_secret_marker`、`secret_marker_exists`；save/delete_credentials 同步写/删标记；单测 5 条 |
| `src-tauri/src/ipc/settings.rs` | `get_provider_credentials_impl` 去 store 参数、改走 display 路径；文档注释；单测更新 |
| `src-tauri/tests/secret_presence.rs` | 集成测试 4 条 |
| `scripts/dev-codesign-runner.sh`、`scripts/create-dev-cert.sh` | cargo runner + 一次性证书创建脚本 |
| `src-tauri/.cargo/config.toml` | macOS debug runner 配置 |
| `Makefile`、`tsconfig.json`、`package.json`、`pnpm-lock.yaml` | scope creep 项 |

参照标准：Rust code-standards（安全红线 / 函数≤50行 / 嵌套≤3层 / 注释写"为什么"/ 无死代码）、项目规范（secret 绝不入库 / SQL 参数化）。

---

## 重点检查判定

### 安全：secret 绝不入库（通过）

`secret_presence` 表 DDL 仅含 `provider_id TEXT` + `field_key TEXT`，无任何值列。
`write_secret_marker` SQL：`INSERT OR REPLACE INTO secret_presence (provider_id, field_key) VALUES (?1, ?2)`，参数只传键。
`load_credentials_for_display` 对已标记 secret 返回 `String::new()`（空串），不回明文。
安全红线无违反。

### SQL 安全：参数化（通过）

`write_secret_marker`、`delete_secret_marker`、`secret_marker_exists` 均使用 `rusqlite::params![...]` 占位符，无字符串拼接。PK 冲突用 `INSERT OR REPLACE`（幂等语义正确）。

### 函数长度规范（通过）

`load_credentials_for_display`：18 行；三个辅助函数各 ≤12 行；均满足 ≤50 行约束。

### 翻译路径不受影响（通过）

`translate.rs` 仍通过 `load_credentials`（接受 store 参数）读取真实 keychain 值，未被此改动触碰。只有 `settings.rs::get_provider_credentials_impl` 改走展示路径，路由清晰。

### 测试覆盖（通过）

单测覆盖：写标记/删标记/展示报 present/展示报 absent（无标记迁移场景）/roundtrip 幂等/删除幂等。集成测试（`secret_presence.rs`）覆盖真实 DB 开库路径（`ensure_schema` 预埋验证）。

### scope creep：`@types/node` + tsconfig（通过）

`tsconfig.json` 加 `"node"` type 是修复既存 `tsc` 类型报错（`NodeJS.Timeout` 等），属于合法 bug fix 而非无关改动，背景合理。

---

## 问题列表

### Important · 置信度 82

**`delete_credentials` 部分失败：标记残留导致 is_set 撒谎**

文件：`src-tauri/src/translate/credential.rs`，第 260–262 行

```rust
store.delete_secret(provider_id, field.key)?;          // L260
// 同步清除存在标记，使设置页徽标变回"待配置"
delete_secret_marker(conn, provider_id, field.key)?;   // L262
```

执行顺序：先删 keychain（L260），再删 DB 标记（L262）。若 L260 成功但 L262 因 DB 错误失败（`?` 传播），函数返回错误——但此时 keychain 已清空，DB 中 `secret_presence` 行仍存在。

后果：设置页调 `get_provider_credentials_impl` → `secret_marker_exists` 返回 `true` → `is_set=true`，界面显示"已配置"；实际 keychain 为空，翻译调用会报凭据不存在。用户看到"已配置"但翻译失败，难以诊断。

这是镜像于 save 方向的问题：save 时 `set_secret` 成功但 `write_secret_marker` 失败，结果是 `is_set=false`（保守方向——用户重存即可自愈），危害较轻（置信度 75，低于阈值未报）。delete 方向的失败则是乐观方向（误报已配置），用户无法直觉修复，危害更高。

DB 写失败在正常运行中极罕见，但属于可观测的、会在实践中偶发的错误路径，且无自愈机制。

**建议修复：** 调整 delete 顺序，先删标记再删 keychain；或在标记删除失败时回滚（重新写回标记警告日志）。最简可靠方案是先删标记：

```rust
// 先删标记（保守方向：标记消失但 keychain 仍有 → is_set=false，用户重存可修复）
delete_secret_marker(conn, provider_id, field.key)?;
store.delete_secret(provider_id, field.key)?;
```

若 `delete_secret_marker` 失败，keychain 未动，标记也未动，两侧一致（幂等重试安全）。若 `delete_secret_marker` 成功但 `store.delete_secret` 失败，is_set=false 但 keychain 有值——保守方向，用户重存即可。

---

### Important · 置信度 80

**`get_provider_credentials_impl` 文档注释未同步更新**

文件：`src-tauri/src/ipc/settings.rs`，第 657–665 行

函数签名已移除 `store` 参数，但 doc comment 仍描述旧行为：

- L657：`"从 store/DB 加载已保存值"` → store 已不参与，应改为 `"从 secret_presence 标记表（secret 字段）/ 加密 DB（非密字段）加载"`
- L658：`"is_set 表示是否在 store 中存在"` → 应改为 `"is_set 表示是否在 secret_presence 标记表中有记录"`
- L665：`"store 读取失败或 DB 操作失败时返回错误字符串"` → 应移除 "store 读取失败"（函数不再访问 store）

注释写"为什么"是项目规范要求。此处注释描述"是什么"时与实现不符，误导维护者以为函数仍走 keychain 路径，可能导致错误的 PR 审查判断或回归修改。

**建议修复：** 将上述三处更新为实际实现语义。

---

## 无 Critical 问题

置信度 ≥80 的 Critical（影响正确性/安全的）问题：无。

- secret 值绝不入库：已确认。
- SQL 参数化：已确认。
- 展示路径不回明文：已确认。
- 翻译路径仍走真实 keychain：已确认。
- shell 脚本 set -euo pipefail + 变量引用 + 优雅降级（CI / 无证书机器透传）：已确认，健壮性合格。

---

## 审查结论

**通过（WARNING）。**

核心设计正确：secret_presence 表只存键不存值，安全红线无违反；SQL 全程参数化；展示路径与翻译路径分离清晰；测试覆盖充分；codesign runner 降级逻辑完整。

非阻塞建议：
1. `delete_credentials` 调整操作顺序（先删 DB 标记，再删 keychain），使部分失败时偏向保守方向。
2. `get_provider_credentials_impl` 文档注释同步更新，移除过时的 "store" 描述。

---

**VERDICT: WARNING**

`severity(Important) · confidence(82) · src-tauri/src/translate/credential.rs:260-262 · delete_credentials 先删 keychain 再删 DB 标记，若标记删除失败则 keychain 已空但 is_set=true（设置页误报"已配置"，翻译失败） · 调整顺序：先 delete_secret_marker 再 store.delete_secret`

`severity(Important) · confidence(80) · src-tauri/src/ipc/settings.rs:657-665 · get_provider_credentials_impl 文档注释三处仍描述已移除的 store 参数路径，与实现不符 · 将 "store/DB"、"在 store 中存在"、"store 读取失败" 改为实际的 secret_presence 标记表语义`
