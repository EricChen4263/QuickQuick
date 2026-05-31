# QuickQuick

A cross-platform (macOS + Windows) tray utility: **clipboard history** + **translation**, opened by global hotkeys.
跨平台（macOS + Windows）托盘小工具：**剪贴板历史** + **翻译**，全局热键唤起。

- Tray-resident; global hotkey pops a panel; the main window only holds settings and full history.
  托盘常驻；全局热键弹出面板；主窗口只管设置与全量历史。
- Default hotkeys / 默认热键：History `Cmd/Ctrl+Shift+V` · Translate `Cmd/Ctrl+Shift+T`（rebindable in Settings / 可在设置里改键）.

---

## Install / 安装

Pre-built installers are attached to each [GitHub Release](../../releases). Builds are **unsigned**, so the OS warns on first launch — clear it once with the steps below.
预编译安装包见每个 [GitHub Release](../../releases)。构建**未签名**，首次打开会被系统拦，按下面一次性放行即可。

### macOS (`.dmg`)
1. Open the `.dmg`, drag **QuickQuick** to Applications.
   打开 `.dmg`，把 **QuickQuick** 拖进"应用程序"。
2. First launch: **right-click** the app → **Open** → **Open** (don't double-click).
   首次打开：**右键**应用 →"打开"→"打开"（别直接双击）。
3. Still blocked? Run in Terminal: `xattr -cr /Applications/QuickQuick.app`
   仍被拦？终端跑：`xattr -cr /Applications/QuickQuick.app`
4. Grant **Accessibility** when asked (needed for auto-paste): System Settings › Privacy & Security › Accessibility.
   按提示授予**辅助功能**权限（自动粘贴需要）：系统设置›隐私与安全性›辅助功能。

### Windows (`.exe` / `.msi`)
1. Run the installer. SmartScreen may show "Windows protected your PC".
   运行安装包，可能弹 SmartScreen"已保护你的电脑"。
2. Click **More info** → **Run anyway**.
   点"更多信息"→"仍要运行"。

> Updates / 更新：no auto-update yet — grab the newest installer from [Releases](../../releases).
> 暂无自动更新——到 [Releases](../../releases) 下最新安装包即可。

---

## Develop & Build / 开发与构建

Prerequisites / 前置：Node ≥ 20, [pnpm](https://pnpm.io), Rust (stable) + Cargo. Run `make doctor` to check.
前置：Node ≥ 20、pnpm、Rust(stable) + Cargo。跑 `make doctor` 自检。

```bash
pnpm install        # install deps / 装依赖
make doctor         # check toolchain + rust targets / 体检工具链与 target
make dev            # run with hot reload / 热重载跑应用
make fmt            # auto-format Rust code / 自动格式化 Rust
make check          # fast type + compile check / 极速类型 + 编译体检
make test           # vitest + cargo test in parallel / 前后端测试并行
make verify         # full gate: types + fmt + clippy + tests / 提交门禁
make build          # local Universal macOS .dmg/.app / 本地出 Universal mac 包
```

Run `make` (no args) for the full target list. / 直接 `make` 看全部目标。

### Releasing / 发布

Releases are built by GitHub Actions on tag push — never built by hand.
发布由 GitHub Actions 在打 tag 时构建——不手工出包。

```bash
make bump VERSION=0.1.0                     # sync version in 3 files / 同步三处版本号
git commit -am "chore: bump v0.1.0"
git tag v0.1.0 && git push --tags           # triggers the release workflow / 触发发布工作流
```

CI builds macOS (Universal) + Windows, then creates a **draft** Release with installers attached — review it on GitHub, then publish.
CI 构建 macOS(Universal) + Windows，生成挂好安装包的**草稿** Release——在 GitHub 上过目后再点发布。

---

## Tech stack / 技术栈

Tauri 2 (Rust backend) + React + TypeScript + Vite. Local history encrypted at rest (SQLCipher + AES-256-GCM / Argon2id).
Tauri 2（Rust 后端）+ React + TypeScript + Vite。本地历史落库加密（SQLCipher + AES-256-GCM / Argon2id）。
