# 更新日志 / Changelog

本项目每个发布版本的更新内容。排版约定：每版先一整段**中文**，分隔线后一整段 **English**（分语言成段，不行内混排）。
发版时 `release.yml` 会自动抽取对应 `## v<版本>` 段拼到 GitHub Release 顶部；`release.sh` 预检要求新版在此有对应段，否则阻断发版。

## v0.3.0

### 🌐 翻译
- 多源翻译：免 key（Lingva / Google）+ 需 key 机翻（百度 / 腾讯 / 阿里 / 火山等）+ LLM 对话翻译（OpenAI / Ollama / ChatGLM / Gemini）。
- 内置离线词典 ECDICT + 有道词典，带音标 / 释义展示组件。
- 默认翻译源改为 Google free；翻译按钮加入「翻译中」加载态反馈。

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

### 📋 Clipboard
- **Rich-text (HTML) fidelity**: capture, preview (DOMPurify-sanitized), paste and copy all preserve formatting; links in the preview open in your browser.

### 🔒 Privacy & Polish
- Local file keystore replaces the Keychain — no more repeated password prompts.
- macOS runs as a background (Accessory) app — no Dock icon; refreshed window chrome.
