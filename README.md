# QuickQuick

跨平台（macOS + Windows）托盘小工具：**剪贴板历史** + **翻译**，全局热键唤起。
A cross-platform (macOS + Windows) tray utility: **clipboard history** + **translation**, opened by global hotkeys.

**[中文](#中文) ·  [English](#english)**

---

## 中文

托盘常驻，全局热键弹出面板；主窗口管设置与全量历史。

### 功能
- **剪贴板历史**：自动记录文本 / 图片；**富文本（HTML）保真**——捕获、预览、粘贴与复制都保留格式（加粗 / 颜色 / 列表 / 表格 / 链接），粘到纯文本编辑器自动退纯文本。本地历史落库加密（SQLCipher + AES-256-GCM / Argon2id）。
- **翻译**：多源——免 key（Lingva / Google）+ 需 key 机翻（百度 / 腾讯 / 阿里 / 火山等）+ LLM 对话翻译（OpenAI / Ollama / ChatGLM / Gemini）+ 内置离线词典（ECDICT）/ 有道词典（音标 / 释义）。
- **默认热键**：剪贴板 `Cmd/Ctrl+Shift+C` · 翻译 `Cmd/Ctrl+Shift+T`（可在设置里改键）。

### 安装
预编译安装包见每个 [GitHub Release](../../releases)。构建**未签名**，首次打开会被系统拦，按下面一次性放行即可。

**macOS（`.dmg`）**
1. 打开 `.dmg`，把 **QuickQuick** 拖进"应用程序"。
2. 首次打开：**右键**应用 →"打开"→"打开"（别直接双击）。
3. 仍被拦？终端跑：`xattr -cr /Applications/QuickQuick.app`
4. 按提示授予**辅助功能**权限（自动粘贴需要）：系统设置›隐私与安全性›辅助功能。

**Windows（`.exe` / `.msi`）**
1. 运行安装包，可能弹 SmartScreen"已保护你的电脑"。
2. 点"更多信息"→"仍要运行"。

**更新**：已支持应用内自动更新——发新版后客户端可自动检测并升级；也可随时到 [Releases](../../releases) 手动下最新安装包。

### 开发与构建
前置：Node ≥ 20、[pnpm](https://pnpm.io)、Rust(stable) + Cargo。跑 `make doctor` 自检。

```bash
pnpm install        # 装依赖
make doctor         # 体检工具链与 target
make dev            # 热重载跑应用
make fmt            # 自动格式化 Rust
make check          # 极速类型 + 编译体检
make test           # 前后端测试并行（vitest + cargo test）
make verify         # 提交门禁：类型 + fmt + clippy + 测试
make build          # 本地出 Universal mac .dmg/.app
```
直接 `make` 看全部目标。

### 发布
发布由 GitHub Actions 在打 tag 时构建——不手工出包。**发版前先在 `CHANGELOG.md` 写本版更新内容**（`release.sh` 预检会校验，CI 自动把对应版本段拼进 Release 正文）。

```bash
# 在 CHANGELOG.md 写好 ## v0.3.0 段后：
scripts/release.sh 0.3.0            # bump 版本号 → 提交推 main → 打 tag 触发 CI
scripts/release.sh 0.3.0 --dry-run # 只演练，不提交/不推送/不打 tag
```
CI 构建 macOS(Universal) + Windows、签名 updater、生成 `latest.json`，发到 GitHub Releases **草稿**——过目后点 Publish 正式放出（草稿不会自动发布）。

### 技术栈
Tauri 2（Rust 后端）+ React + TypeScript + Vite。

---

## English

Tray-resident; a global hotkey pops a panel; the main window holds settings and full history.

### Features
- **Clipboard history**: auto-captures text / images; **rich-text (HTML) fidelity** — capture, preview, paste and copy all preserve formatting (bold / color / lists / tables / links), and fall back to plain text in plain-text editors. Local history is encrypted at rest (SQLCipher + AES-256-GCM / Argon2id).
- **Translation**: multi-source — free (Lingva / Google) + key-based MT (Baidu / Tencent / Alibaba / Volcengine…) + LLM chat (OpenAI / Ollama / ChatGLM / Gemini) + a built-in offline dictionary (ECDICT) / 有道 (phonetics / definitions).
- **Default hotkeys**: clipboard `Cmd/Ctrl+Shift+C` · translate `Cmd/Ctrl+Shift+T` (rebindable in Settings).

### Install
Pre-built installers are attached to each [GitHub Release](../../releases). Builds are **unsigned**, so the OS warns on first launch — clear it once with the steps below.

**macOS (`.dmg`)**
1. Open the `.dmg`, drag **QuickQuick** to Applications.
2. First launch: **right-click** the app → **Open** → **Open** (don't double-click).
3. Still blocked? Run in Terminal: `xattr -cr /Applications/QuickQuick.app`
4. Grant **Accessibility** when asked (needed for auto-paste): System Settings › Privacy & Security › Accessibility.

**Windows (`.exe` / `.msi`)**
1. Run the installer. SmartScreen may show "Windows protected your PC".
2. Click **More info** → **Run anyway**.

**Updates**: in-app auto-update is supported — clients detect and upgrade to new releases automatically; you can also grab the newest installer from [Releases](../../releases) anytime.

### Develop & Build
Prerequisites: Node ≥ 20, [pnpm](https://pnpm.io), Rust (stable) + Cargo. Run `make doctor` to check.

```bash
pnpm install        # install deps
make doctor         # check toolchain + rust targets
make dev            # run with hot reload
make fmt            # auto-format Rust code
make check          # fast type + compile check
make test           # vitest + cargo test in parallel
make verify         # full gate: types + fmt + clippy + tests
make build          # local Universal macOS .dmg/.app
```
Run `make` (no args) for the full target list.

### Releasing
Releases are built by GitHub Actions on tag push — never built by hand. **Write the version's notes in `CHANGELOG.md` first** (`release.sh` preflight enforces it; CI splices that version's section into the Release body).

```bash
# After writing the ## v0.3.0 section in CHANGELOG.md:
scripts/release.sh 0.3.0            # bump versions → commit & push main → tag → trigger CI
scripts/release.sh 0.3.0 --dry-run # rehearse only; no commit/push/tag
```
CI builds macOS (Universal) + Windows, signs the updater, generates `latest.json`, and creates a **draft** Release — review it on GitHub, then Publish (drafts are never auto-published).

### Tech stack
Tauri 2 (Rust backend) + React + TypeScript + Vite.
