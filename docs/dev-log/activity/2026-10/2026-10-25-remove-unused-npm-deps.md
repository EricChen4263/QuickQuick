---
status: done
category: implementation
subject: P6-3 残留 · 删除两个未使用的 npm 依赖（@tauri-apps/plugin-autostart / plugin-updater）
summary: 移除 src/ 下零 import 的两个 Tauri 插件 npm 包，锁文件只减 20 行（3 个 hunk、零无关改动）；knip 未使用依赖告警从 2 项归零，files/unlisted 门禁仍为 0，vitest 74 文件 771 测试与改前逐位一致，产物文件名与改前完全相同（说明对运行时零影响）。Rust 侧的插件 crate 注册与 capabilities 权限一律未动。
evidence:
  commits:
    - "（本卡对应提交）chore(deps): 移除两个未使用的 Tauri 插件 npm 依赖"
  commands:
    - "pnpm remove @tauri-apps/plugin-autostart @tauri-apps/plugin-updater"
    - "pnpm install --frozen-lockfile"
    - "pnpm exec knip"
    - "pnpm exec knip --include files,unlisted"
    - "pnpm test"
    - "pnpm exec tsc -b"
    - "pnpm run build"
  urls: []
---

# AI 活动记录 · 删除两个未使用的 Tauri 插件 npm 依赖

## 摘要

P6-3 的第一批 knip 告警里，有两条是「真的没人用」而不是「多余 export」：

| 依赖 | 前端引用 | 能力实际由谁承载 |
|---|---|---|
| `@tauri-apps/plugin-autostart` | 0 处（`grep -rn` 全 `src/` exit 1） | `src-tauri/src/ipc/settings.rs` 的 `apply_autostart_preference` 自写 IPC + Rust crate |
| `@tauri-apps/plugin-updater` | 0 处 | `src-tauri/src/ipc/update.rs` 自写 `check_for_updates` 命令 + Rust crate |

lead 裁定删除，本卡记录执行与验证。

## 为什么删 npm 包是安全的：能力由 Rust 侧承载

这三层是**互相独立**的，删掉最上面那层不改变任何行为：

1. **Rust crate**（`src-tauri/Cargo.toml` 的 `tauri-plugin-autostart` / `tauri-plugin-updater`）：提供插件本体与 `*.init()` 注册；
2. **capabilities 权限**（`src-tauri/capabilities/default.json` 的 `updater:default`、`autostart:allow-enable/disable/is-enabled`）：放行的是 Rust 侧命令的 ACL，权限定义来自 crate 而非 npm 包；
3. **npm 包**（`@tauri-apps/plugin-*`）：只提供「前端 JS 直接调插件命令」的糖。本项目没走这条路径——前端一律调自写 IPC 命令。

所以第 3 层是零引用的死依赖，1、2 层必须原样保留。**本次未触碰**：`src-tauri/Cargo.toml`（第 24/28 行仍在）、`src-tauri/src/lib.rs`（第 114 行 `tauri_plugin_autostart::init` 仍在）、`src-tauri/capabilities/default.json`（`updater:default` 与三条 `autostart:allow-*` 仍在）。

## lockfile：`pnpm remove` 够干净，没走 `--lockfile-only` 兜底

```
$ pnpm remove @tauri-apps/plugin-autostart @tauri-apps/plugin-updater
Packages: -2
dependencies:
- @tauri-apps/plugin-autostart 2.5.1
- @tauri-apps/plugin-updater 2.10.1
Done in 4s

$ git diff --stat pnpm-lock.yaml
 pnpm-lock.yaml | 20 --------------------
 1 file changed, 20 deletions(-)
```

**20 行删除、0 行新增、3 个 hunk**，全部指向这两个包：

- `importers.dependencies`：两段 specifier/version（6 行）
- `packages`：两条 `resolution` 条目（6 行）
- `snapshots`：两条快照（8 行）

没有版本重解析、没有无关依赖被顺带升级/降级——所以不需要改用 `pnpm install --lockfile-only`。这也说明本仓的 lockfile 此前是干净的（否则 `pnpm remove` 会把历史漂移一起带出来）。

`pnpm install --frozen-lockfile` → `Already up to date, Done in 192ms`（exit 0），证明锁文件与 package.json 自洽。

## knip 改前 / 改后

**改前**（11 项，exit 1）：

```
Unused dependencies (2)
@tauri-apps/plugin-autostart  package.json:16:6
@tauri-apps/plugin-updater    package.json:18:6
Unused exports (1)
SECTION_LABELS  src/panels/settings/SectionNav.tsx:5:14
Unused exported types (8)
Matcher / ActionBarVariant / ActionBarAlign / PosDefinition / PasteOutcome
ClipboardSub / TranslateSub / SettingsSub
```

**改后**（9 项，exit 1）：

```
Unused exports (1)
SECTION_LABELS  src/panels/settings/SectionNav.tsx:5:14
Unused exported types (8)
Matcher           clipSearchModule  type       src/clip-popover/clip-search.ts:32:13
ActionBarVariant                    type       src/components/ActionBar.tsx:5:13
ActionBarAlign                      type       src/components/ActionBar.tsx:8:13
PosDefinition     ipcClient         interface  src/ipc/ipc-client.ts:41:18
PasteOutcome      ipcClient         type       src/ipc/ipc-client.ts:547:13
ClipboardSub                        type       src/main-window/nav.ts:10:13
TranslateSub                        type       src/main-window/nav.ts:11:13
SettingsSub                         type       src/main-window/nav.ts:12:13
```

`Unused dependencies` 整段消失，剩余 9 项全部是「导出但只在本文件内使用」的多余 `export` 关键字——按 P6-3 的既定处置，它们不进阻塞门禁、也不在本项删除（删 `export` 属于改前端源码，超出本项写入范围）。

**门禁**（`knip --include files,unlisted`）：改前 0、改后 0（exit 0），说明删除没有制造新的未接线文件或未声明依赖。

## 验证（全部实跑）

```
pnpm install --frozen-lockfile  → Already up to date, 192ms（exit 0）
pnpm exec knip                  → 9 项（2 个未使用依赖已消失）
pnpm exec knip --include files,unlisted → 0 告警（exit 0）
pnpm test                       → 74 files / 771 tests passed（改前 74 / 771，逐位一致）
pnpm exec tsc -b                → 无错误（exit 0）
pnpm run build                  → ✓ built in 431ms（exit 0）
```

**产物零差异**是最有力的一条：改后 `dist/assets/main-1iEuy2Fd.js`、`main-C2hHwCTl.css`、`browser-api-Bc7jY-_U.js` 等文件名与改前一天的重建结果**完全相同**（vite 文件名含内容哈希），说明删掉这两个包对打包产物没有任何影响——符合「它们本来就没被 import」的判断。

## 未闭环 / 说明

- **未在真机验证**：自启开关与「检查更新」按钮的端到端行为。但本次只删 npm 包、不动 Rust 侧与 IPC，且产物字节级一致，风险集中在「是否有隐藏的动态 import」——已用 `grep -rn` 全 `src/` 复核（exit 1，无匹配），`vite build` 成功也排除了构建期解析失败。
- **`package.json` 的 `dependencies` 定位**：这两个包放在 `dependencies` 而非 `devDependencies`，本来就不对（前端构建期依赖已全在 devDependencies）；删除顺带修正了这一处分类问题，但其余依赖未做重新归类（超出本项写入范围）。
