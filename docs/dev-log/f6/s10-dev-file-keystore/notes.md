---
id: F6-S10-notes
type: notes
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 已解决
commit: WIP
acceptance_ids: []
---

# macOS 钥匙串反复弹密码：根因、修复与踩坑记录

## 现象
打开 app 时、以及打开「翻译源设置」时，macOS 反复弹「quickquick 想要使用你储存在钥匙串的
"io.quickquick.app" 中的机密信息」密码框，点「始终允许」也存不住。

## 根因
1. **dev 二进制是 ad-hoc 签名**（`codesign -dv` 显示 `Signature=adhoc`、`TeamIdentifier=not set`）。
   macOS 钥匙串 ACL 按代码签名的 designated requirement 授权 app；ad-hoc 签名的 DR 是
   `cdhash`，每次 `cargo build` 重编都变 → 系统认作「新 app」→「始终允许」存不住、反复弹。
2. **弹「多次」**= 每个独立钥匙串条目各弹一次：启动读 `sqlcipher_master_key`
   （keyprovider.rs），打开翻译源旧逻辑**逐个** secret 字段读 `cred.<pid>.<key>`
   （credential.rs，service 均为 `io.quickquick.app`）。

> 注意：终端用户装固定版本不重编时 cdhash 稳定，并不会像开发期这样狂弹；这是 dev 特有问题。

## 修复一：设置页不读钥匙串（release 用户同样受益）
- 新增 `secret_presence(provider_id, field_key)` 表（db.rs `ensure_schema`），只记某 secret
  是否已写入钥匙串、**绝不存值**。
- credential.rs 新增 `load_credentials_for_display`（签名**无 store 参数**，编译期保证不碰钥匙串）
  + `write/delete/secret_marker_exists`；`save/delete_credentials` 的 secret 分支同步写/删标记。
  - delete 顺序刻意为「先删标记、再删 keychain」：半途失败时 `is_set` 偏向保守的 false（未配置），
    而非谎报「已配置」且无自愈。
- `get_provider_credentials_impl` 去掉 store 参数、改走 DB 标记表。**翻译路径 `load_credentials`
  仍读真密钥，一字未动**。
- 迁移=不回填：本改动前已存入钥匙串的 secret 无标记行，设置页显示「待配置」，需重存一次注册标记
  （预发布期、单用户，刻意为之）。

## 修复二：dev 文件密钥库（取代自签方案）

### 先走了自签证书路，连栽三次后放弃
目标是给 dev 二进制一个**稳定签名身份**让「始终允许」存得住。试了「固定自签证书 + cargo runner」，
但 macOS 自签证书在本机不可靠，三个真实踩坑（**留作前车之鉴**）：

1. **bash 把 `$VAR` 后紧贴的中文首字节吞进变量名**：脚本里 `echo "...「$CERT_NAME」..."`，
   macOS 自带 bash 3.2 / 某些 locale 下，`$CERT_NAME` 后紧跟多字节中文 `」`(0xe3..) 时把首字节
   误并入变量名 → `set -u` 报 `CERT_NAME�: unbound variable`。**修法：变量贴非 ASCII 字符时一律用
   `${VAR}` 花括号显式终止。**
2. **OpenSSL 3 的 p12 Apple `security` 读不了**：`MAC verification failed during PKCS12 import
   (wrong password?)`。OpenSSL 3 默认 PBE/MAC 算法 + 空导出口令，Apple Security framework 无法校验。
   **修法：`openssl pkcs12 -export -legacy`（退回旧算法）+ 用非空临时口令**（LibreSSL/旧版无 `-legacy`，
   需降级重试）。
3. **CLI 导入的自签证书未受信任，codesign 拒用**：`codesign -s` 报 `no identity found`，
   `security find-identity -p codesigning -v` 列「0 valid identities」。自签证书要被 codesign 当作
   可用身份，须建立 code signing 信任（`security add-trusted-cert`，需交互式授权/admin）。GUI「钥匙串
   访问 > 证书助理」建的自签根证书因创建者隐式信任而能直接用，CLI 非交互建则不行。

结论：自签信任链折腾的性价比 << 收益，**放弃**。

### 改为：debug 文件密钥库（确定、无平台坑）
- `#[cfg(debug_assertions)]` 门控的 `FileKeyProvider`（keyprovider.rs，存 `dev-master-key`，32 字节）
  和 `FileCredStore`（credential.rs，存 `dev-credentials.json`），均 `0600`（`#[cfg(unix)]`）。
- `default_cred_store(config_dir)` cfg 工厂：debug→File，release→Keyring，收敛 store 选择单点。
- 接线：lib.rs `setup_app_db`、settings.rs 两命令、translate.rs。
- 效果：**debug 构建三条路径（启动主密钥 / 点翻译读 secret / 设置页判断已配置）全绕开钥匙串 → dev 零弹窗**；
  **release（`debug_assertions` off）一字不变，仍走真钥匙串**，分发版安全性不受影响。

### 又一个坑（Phase 6 审查抓出）
4. **debug-only 测试模块的 cfg 门控**：`file_provider_tests` / `file_cred_store_tests` 若只写
   `#[cfg(test)]`，会引用 `#[cfg(debug_assertions)]` 门控的类型 → `cargo test --release` 编译失败
   （14 个 E0433；`cargo check --release` 查不出，因它不编译测试模块）。**修法：`#[cfg(all(test,
   debug_assertions))]`。**

## 其它踩坑
5. **路径混淆**：app_config_dir 用 bundle identifier `com.quickquick.app`（tauri.conf.json），
   **不是**钥匙串 service 名 `io.quickquick.app`。旧 dev DB 在
   `~/Library/Application Support/com.quickquick.app/quickquick.db`。
6. **HEAD 既存 tsc 红**：`src/shell/capabilities.test.ts`（上游 4e9992b）import `node:fs`/`node:path`
   但仓库未加 `@types/node` 依赖 → `make verify` 第 1 步本就过不了。顺带修复（package.json + tsconfig）。

## 留给用户的一次性手动步骤
1. 删旧 dev DB：`rm ~/Library/Application\ Support/com.quickquick.app/quickquick.db`
   （换了密钥来源，旧库解不开，需重建）。
2. dev 翻译密钥重填一次（旧钥匙串值不迁移到新文件）。
3. 之后 `make dev` 不再需要任何证书 / codesign / 点「始终允许」。

## 质量门
`make verify` 五步全绿 · `cargo test --release` 363 passed · `cargo check --release` 通过 ·
Phase 6 tester 动态证伪（5 变异全杀假绿）+ code-reviewer（1 Critical + 2 Important 已修复）。
