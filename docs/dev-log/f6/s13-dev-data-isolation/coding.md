---
id: F6-S13-coding
type: coding
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 已实现
commit: 9751cca
acceptance_ids: []
author: coder
---

## 实现记录（coder · 2026-06-04）

按方案落地，两处改动完成：

- **改动 1** `src-tauri/src/ipc/settings.rs`
  - `resolve_config_dir`（约 234 行）：取 `app_config_dir()` 得 `base` 后，调抽出的纯函数 `apply_dev_subdir(&base, cfg!(debug_assertions))` 决定最终目录，再 `create_dir_all`。
  - 新增纯函数 `pub fn apply_dev_subdir(base, is_debug)`：debug 追加 `dev` 子目录，release 返回 `base` 本身。抽成纯函数以可单测（避免 cfg 在函数体内分叉、保证两分支都编译）。
- **改动 2** `src-tauri/src/lib.rs` 5 处直调 `app.path().app_config_dir()` 全部改走 `ipc::settings::resolve_config_dir(...)`：
  - `setup_app_db`（用 `app.handle()`）、捕获轮询（`&handle`）、autostart 同步（`app.handle()`）、`register_hotkeys`（`handle`）、`init_capture_state`（`app.handle()`）。
  - 各处原 eprintln 降级语义原样保留（`Err` → eprintln + None/skip/return，无 unwrap/panic）；原手动 `create_dir_all` 删除（resolve_config_dir 已内含），消除重复。

**TDD**：抽出纯函数 `apply_dev_subdir` 并加集成测试 `tests/config_dir_isolation_test.rs`（debug 加 `dev` 后缀 / release 不加，两条 AAA 断言验具体路径值）。红（函数缺失 → 编译失败）→ 绿（2 passed）。

**验证**（实跑）：`cargo fmt`(0) / `cargo clippy --all-targets -D warnings`(0 警告) / `cargo build`(debug 过) / `cargo build --release`(release 过，两 cfg 分支均编译) / `cargo test`(389 passed, 29 suites)。机器证据见 `artifacts/`。

# f6-s13 dev/release 数据目录隔离（debug 落 dev 子目录）

## 要解决的 bug

正式版（release，adhoc 未签名）装机后，剪贴板页报「加载失败，请稍后重试」。

实测复现与定位（已坐实，非推断）：

- 从终端带日志启动 release 二进制，stderr：
  `数据库打开失败：数据库操作失败：file is not a database`
- `file is not a database` 是 **SQLCipher 密钥不匹配**时的典型表现：头部解密不出来，遂报「不是数据库」。
- 数据目录 `~/Library/Application Support/com.quickquick.app/` 同时存在 `dev-master-key`、`dev-credentials.json`（debug 文件密钥库产物）与 `quickquick.db`——该 DB 是先前 **dev 构建用文件密钥**加密的。
- release 构建走 **keychain 密钥**路径（`#[cfg(not(debug_assertions))] KeychainKeyProvider`），用 keychain 派生的密钥去开 dev 文件密钥加密的库 → 密钥不匹配 → 开库失败。
- 验证：把数据目录整体挪开后，release 从零建库（217KB 全新 `quickquick.db`，无 dev 文件，无报错），剪贴板正常。证明 **release 对全新用户无问题，发布有效**；问题仅在「同机 dev/release 共用数据目录」时出现。

## 根因

debug 与 release 构建共用同一 identifier `com.quickquick.app` → 同一数据目录 `app_config_dir()`。两者密钥体系不同：

- debug：`FileKeyProvider`（文件 `dev-master-key`）
- release：`KeychainKeyProvider`（macOS 钥匙串）

同机上 dev 与 release 来回跑，后跑的一方用自己的密钥去开对方加密的 `quickquick.db`，必然 `file is not a database`。用户只装 release 永不触发；开发者每次切构建就踩。

## 方案（单点收口，DRY）

所有持久化文件路径都源自 `app.path().app_config_dir()`。中心 helper `resolve_config_dir()`（`src-tauri/src/ipc/settings.rs:234`）已服务 ipc 层，但 **lib.rs 有 5 处直接调 `app.path().app_config_dir()` 绕过了它**。

### 改动 1：`resolve_config_dir()` 加 debug 子目录

`src-tauri/src/ipc/settings.rs` 的 `resolve_config_dir()`：取得 `app_config_dir()` 后，`#[cfg(debug_assertions)]` 追加 `dev` 子目录；release 不追加。`create_dir_all` 对最终目录执行。

```rust
pub(crate) fn resolve_config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录：{e}"))?;

    // debug 构建落 dev 子目录，与 release（钥匙串密钥）的数据/密钥彻底隔离，
    // 消除同机 dev↔release 切换时 SQLCipher 密钥不匹配（file is not a database）。
    #[cfg(debug_assertions)]
    let dir = base.join("dev");
    #[cfg(not(debug_assertions))]
    let dir = base;

    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录：{e}"))?;
    Ok(dir)
}
```

### 改动 2：lib.rs 的 5 处直接调用全部改走 `resolve_config_dir()`

逐处（行号以当前 HEAD 为准，实现时以实际为准）：

| 位置 | 用途 | 现状 | 改为 |
|---|---|---|---|
| `lib.rs:192` `setup_app_db` | `quickquick.db` 开库目录 | `app.path().app_config_dir()` + 手动 `create_dir_all` | `crate::ipc::settings::resolve_config_dir(app.handle())` |
| `lib.rs:282` 捕获轮询 | 读 `settings.json` 取 `max_image_bytes` | 直接 `app_config_dir()` | 同上 |
| `lib.rs:319` autostart 同步 | `autostart.json` | 直接 `app_config_dir()` | 同上 |
| `lib.rs:458` 热键读取 | `hotkey.json` | 直接 `app_config_dir()` | 同上 |
| `lib.rs:491` 设置布尔字段读取 | `settings.json` | 直接 `app_config_dir()` | 同上 |

注意：

- `setup_app_db(app: &mut tauri::App)` 取 handle 用 `app.handle()`（`&AppHandle`），传给 `resolve_config_dir`。
- 各处原有「拿不到目录则 eprintln 不 panic / 跳过」的降级语义必须保留；`resolve_config_dir` 返回 `Err` 时走原降级路径，不得改成 panic。
- 改后 lib.rs 不再直接 `use` `app_config_dir`（若有未用 import 一并清理）。

## 不做什么（边界）

- 不改 `tauri.conf.json` 的 identifier（构建期静态值，debug/release 同一份 conf 无法分流；路径层加子目录是最小且正确的做法）。
- 不动 release 路径：release 仍落 `app_config_dir()` 根，**已装真实用户零迁移、零影响**。
- 不做 dev 旧库迁移：dev 是测试数据，落新 `dev/` 子目录后旧根目录的 dev 文件自然弃用，无需搬迁。
- 不改 keyprovider/credential 的单测：它们用 tempdir 显式传目录，与本改动正交。

## 验收标准

1. `make verify` 五步全绿（tsc / cargo fmt --check / clippy -D warnings / vitest / cargo test）。
2. debug 构建运行后，所有文件（`quickquick.db` / `dev-master-key` / `dev-credentials.json` / `settings.json` / `hotkey.json`）落在 `<app_config_dir>/dev/` 子目录；根目录不再新增这些文件。
3. release 构建路径不变：文件仍落 `<app_config_dir>/` 根（用 `cargo build --release` 跑一次或代码审查 cfg 分支确认）。
4. 同机先跑 release（根目录建库）再跑 debug（dev 子目录建库），两者互不报 `file is not a database`。
5. lib.rs 5 处降级语义（拿不到目录不 panic）保持不变。

## TDD 说明

目录解析是既有注释明示的「不可单测的胶水层」（依赖 Tauri AppHandle 运行时）。可单测的纯逻辑极少；本 slice 以**改动小、cfg 对称、make verify + 手动落盘验证**为主要保障。若可低成本抽出「给定 base 返回带/不带 dev 后缀」的纯函数则加单测，但不为凑测试强行过度抽象。
