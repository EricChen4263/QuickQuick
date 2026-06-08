# 更新日志 / Changelog

本项目每个发布版本的更新内容。排版约定：每版先一整段**中文**，分隔线后一整段 **English**（分语言成段，不行内混排）。
发版时 `release.yml` 会自动抽取对应 `## v<版本>` 段拼到 GitHub Release 顶部；`release.sh` 预检要求新版在此有对应段，否则阻断发版。

## v0.3.3

### 📋 剪贴板
- 修复：全局热键唤起的快捷弹窗不显示新捕获的内容——此前打开弹窗后再复制 / 截图，弹窗列表停在打开时的旧快照，新项不出现。现弹窗与主窗口一致订阅剪贴板变化事件，新内容实时刷新进列表（隐藏期捕获的项也会在下次唤起时就位）。

---

### 📋 Clipboard
- Fixed: the quick popover (opened by the global hotkey) didn't show newly captured items — after opening it, anything you copied or screenshotted wouldn't appear because its list was frozen at open time. The popover now subscribes to clipboard-change events just like the main window, so new content refreshes into the list in real time (items captured while it's hidden are ready the next time you open it).

## v0.3.2

### 🖥️ macOS 辅助功能与粘贴（重要修复）
- 修复「粘贴到前台」失效：此前 release 的 App 仅带链接器残缺签名（identifier 随机、资源未密封），macOS 辅助功能授权无法生效，`AXIsProcessTrusted()` 始终返回 false。现已对 App 做正经 ad-hoc 代码签名（identifier=com.quickquick.app、密封资源、绑定 Info.plist），授权辅助功能后即可自动粘贴。
- **已知限制**：ad-hoc 签名无稳定身份，**每次自动更新到新版本后，需到「系统设置›隐私与安全性›辅助功能」重新勾选 QuickQuick 一次**（彻底免重授需 Apple Developer ID 签名，后续评估）。
- 降级提示横幅新增「打开辅助功能设置」按钮，一键直达授权页；并补无障碍 `role="status"`。

### 📋 剪贴板
- 修复：「请手动粘贴」降级提示横幅出现时会把左右两栏布局挤塌成单栏的问题，现固定为顶部整行横幅，不影响列表/详情两栏。

### ⌨️ 热键与设置
- 剪贴板历史默认热键改为 `Cmd/Ctrl+Shift+C`（翻译仍为 `Cmd/Ctrl+Shift+T`，均可在设置里改键）。
- 收窄设置页二级菜单，视觉更紧凑。

---

### 🖥️ macOS Accessibility & Paste (important fix)
- Fixed "paste to foreground" not working: the released app previously carried only the linker's incomplete ad-hoc signature (random identifier, resources unsealed), so macOS Accessibility couldn't take effect and `AXIsProcessTrusted()` always returned false. The app is now properly ad-hoc code-signed (identifier=com.quickquick.app, sealed resources, bound Info.plist), so auto-paste works once Accessibility is granted.
- **Known limitation**: ad-hoc signatures have no stable identity, so **after each auto-update you must re-enable QuickQuick once under System Settings › Privacy & Security › Accessibility** (a fully re-grant-free experience requires an Apple Developer ID signature — under evaluation).
- The fallback notice banner now has an "Open Accessibility Settings" button for one-tap access, plus an accessibility `role="status"`.

### 📋 Clipboard
- Fixed: the "paste manually" fallback banner used to collapse the two-column layout into a single column; it's now a fixed full-width top banner that no longer disturbs the list/detail columns.

### ⌨️ Hotkeys & Settings
- Default clipboard-history hotkey changed to `Cmd/Ctrl+Shift+C` (translation stays `Cmd/Ctrl+Shift+T`; both rebindable in Settings).
- Tightened the settings sub-menu for a more compact look.

## v0.3.1

### 🌐 翻译
- 多源翻译：免 key（Lingva / Google）+ 需 key 机翻（百度 / 腾讯 / 阿里 / 火山等）+ LLM 对话翻译（OpenAI / Ollama / ChatGLM / Gemini）。
- 内置离线词典 ECDICT + 有道词典，带音标 / 释义展示组件。
- 默认翻译源改为 Google free；翻译按钮加入「翻译中」加载态反馈。
- 修复：Ollama 等「本地无 key 但需填模型」的源现在正常显示**配置按钮**（此前因无 key 被误判为免配置，导致填不了必填的 model）。

### 📋 剪贴板
- **富文本（HTML）保真**：捕获、预览（DOMPurify 清洗）、粘贴与复制均保留格式；预览里的链接在系统浏览器中打开。

### 🔒 隐私与体验
- 本地文件密钥库替代钥匙串——不再反复弹密码。
- macOS 后台运行（Accessory 策略，不在 Dock 显示）；窗口视觉整体刷新。

---

### 🌐 Translation
- Multi-source translation: free (Lingva / Google) + key-based MT (Baidu / Tencent / Alibaba / Volcengine…) + LLM chat (OpenAI / Ollama / ChatGLM / Gemini).
- Built-in offline dictionary (ECDICT) + 有道, with a phonetics / definitions view.
- Default source switched to Google free; a "translating…" loading state on the translate button.
- Fix: sources that need config but no API key (e.g. Ollama) now show a **Configure** button (previously hidden because they have no key, so their required `model` field couldn't be set).

### 📋 Clipboard
- **Rich-text (HTML) fidelity**: capture, preview (DOMPurify-sanitized), paste and copy all preserve formatting; links in the preview open in your browser.

### 🔒 Privacy & Polish
- Local file keystore replaces the Keychain — no more repeated password prompts.
- macOS runs as a background (Accessory) app — no Dock icon; refreshed window chrome.
