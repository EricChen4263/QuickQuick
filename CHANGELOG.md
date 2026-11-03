# 更新日志 / Changelog

本项目每个发布版本的更新内容。排版约定：每版先一整段**中文**，分隔线后一整段 **English**（分语言成段，不行内混排）。
发版时 `release.yml` 会自动抽取对应 `## v<版本>` 段拼到 GitHub Release 顶部；`release.sh` 预检要求新版在此有对应段，否则阻断发版。

## v0.5.1

### 🖥️ 多显示器
- **唤出时跟随光标所在屏**：在分辨率与缩放比例不同的多显示器上（如 Retina 主屏配 1x 外接屏），快捷键与托盘唤出的窗口会落在光标所在的显示器，不再回退到主屏。
- **主窗口唤出时重新定位**：⌘⇧M 以及托盘的「显示」「打开设置」等入口，唤出前会按光标所在屏居中靠上定位，不再停在上次的位置。
- **再次启动 app 唤出**同样按光标所在屏定位。
- Windows 上的判屏逻辑保持不变；主窗口同样会按光标所在屏定位。

---

### 🖥️ Multi-display
- **Shows on the cursor's display**: on multi-monitor setups with different resolutions or scale factors (for example a Retina main display plus a 1x external one), the window summoned by a hotkey or the tray now appears on the display under the cursor, instead of falling back to the main display.
- **Main window repositions on summon**: ⌘⇧M and tray items such as Show and Open Settings now center the window near the top of the cursor's display before showing it, instead of reappearing where it was last.
- **Launching the app again** also places the window on the cursor's display.
- On Windows, display detection is unchanged; the main window is likewise placed on the cursor's display.

---

## v0.5.0

### 🖼️ 图片预览
- **右侧预览显示大图**：图片条目的预览不再是列表缩略图的放大版，而是原图按最长边 1600px 生成的预览图，放大后依然清晰。浮窗同步放大到 880×560，列表固定 320pt，预览区拿到剩余宽度。
- **点击预览图片弹出大图**：点击右侧预览的图片，在浮窗或主窗口内弹出覆盖层显示大图。点击列表里的缩略图只负责选中，不再弹出大图。
- **大图可缩放**：覆盖层工具栏提供缩小、放大、适应窗口、原比例，并显示当前比例；键盘 `-` `+` `0` `1` 同样可用。放大超出窗口后可以滚动查看局部。
- **关闭方式**：点击空白处或按 Esc 关闭，按 Esc 只关闭大图，不会关掉整个浮窗。点击图片本身和工具栏不关闭。

---

### 🖼️ Image preview
- **Large image in the right-hand preview**: the preview of an image entry is no longer an enlarged list thumbnail. It shows a preview generated from the original at up to 1600 px on its longest edge, so it stays sharp when enlarged. The quick-paste popover grows to 880×560, with the list fixed at 320 pt and the preview taking the remaining width.
- **Click the preview image to open it large**: clicking the image in the right-hand preview opens an overlay with the full-size view, in either the popover or the main window. Clicking a thumbnail in the list now only selects the row and no longer opens the large view.
- **Zoomable large view**: the overlay toolbar offers zoom out, zoom in, fit to window and actual size (100%), and shows the current scale. The keys `-` `+` `0` `1` work too. When the image overflows the window, you can scroll to see parts of it.
- **Closing**: click an empty area or press Esc to close. Esc closes only the large view, not the whole popover. Clicking the image itself or the toolbar does not close it.

## v0.4.0

### 🎨 界面（重做）
- **整体界面重做**：按统一的视觉规范重建了主窗三个页面与两个热键浮窗——字体、字号、间距、圆角、动效、图标全部收敛到一套阶梯上。此前界面里散着 15 种字号、9 种圆角、5 套「边框 + 白底」卡片，现在是 7 档字号、5 档圆角、每页只有一个抬升面。
- **侧边栏改为顶部栏**：原来常驻的 112px 左侧导航占掉了横向宽度。现在标题栏与一级导航合并成一条 48px 顶栏，剪贴板与翻译的「列表 + 预览」双栏拿到全宽。
- **字体换成 Geist**：拉丁与数字用 Geist / Geist Mono，中文回落到苹方 / 微软雅黑，并启用等宽数字让时间、计数、耗时对齐。
- **层次改为「一页一个抬升面」**：剪贴板页只有预览区是抬升面板、翻译页只有工作区、设置页整页不用卡片，其余靠背景色差与留白分层。
- **主题入口移到「设置 › 外观」**：顶栏不再放太阳 / 月亮式的三档图标开关，改为设置页里的纯文字三档分段。
- **状态补齐**：列表加载改为骨架屏（不再是空白或转圈），空态给下一步动作，错误提示统一写成「什么失败 + 下一步怎么做」；键盘 Tab 进列表行时行内操作按钮会浮出，不再出现焦点看不见的问题。
- **对比度修正**：浅色模式下次要文字与成功 / 警告徽章的对比度原本低于无障碍 AA 4.5:1（最低 3.4），现已全部提升到 4.85:1 以上。
- **两个热键浮窗跟随同一套视觉**：毛玻璃窗口内部的选中态、键帽、徽章、译文排版与主窗一致；翻译浮窗的正文字号按 320×200 的真实窗口尺寸调小，长译文不再被切在字中间。
- 修复翻译页语言方向箭头被渲染成实心黑三角、翻译历史显示 `deepl-free` 这类内部标识、剪贴板历史为空时右侧仍提示「选中左侧任意一条记录」等几处细节。

---

### 🎨 Interface (redesign)
- **Full interface rebuild**: the three main-window pages and both hotkey popovers were rebuilt on one visual spec — type, spacing, corner radii, motion and iconography now all resolve to a single scale. The old UI carried 15 font sizes, 9 radius values and five variants of the same "border + white card" look; it is now 7 font sizes, 5 radii, and exactly one raised surface per page.
- **Sidebar became a top bar**: the permanent 112px left navigation was eating horizontal space. The title bar and top-level navigation are now merged into a single 48px bar, so the list-plus-preview layouts on the clipboard and translate pages get the full width.
- **New typeface, Geist**: Latin text and numerals use Geist / Geist Mono, with Chinese falling back to PingFang SC / Microsoft YaHei. Tabular numerals keep timestamps, counts and elapsed times aligned.
- **One raised surface per page**: on the clipboard page only the preview is elevated, on the translate page only the workspace, and the settings page uses no cards at all — everything else layers through background tone and whitespace.
- **Theme moved to Settings › Appearance**: the top bar no longer carries a sun / moon icon toggle; it is now a plain three-way text segment in settings.
- **States completed**: list loading is a skeleton shaped like the real rows, empty states offer a next step, and error messages are written as "what failed plus what to do next". Tabbing into a list row now reveals its inline actions, so focus is never invisible.
- **Contrast corrected**: secondary text and the success / warning badges fell below the WCAG AA 4.5:1 threshold in light mode (as low as 3.4). All now sit at 4.85:1 or above.
- **Both hotkey popovers follow the same system**: selection states, keycaps, badges and translation typography match the main window, and the translate popover's body size was tuned to its real 320×200 window so long translations are no longer cut mid-glyph.
- Fixed the translate page's direction arrow rendering as a solid black triangle, the translation history showing internal provider ids such as `deepl-free`, and the clipboard preview still prompting "select an item on the left" when the history is empty.

## v0.3.8

### 🪟 Windows（修复）
- 修复自绘标题栏的最小化 / 最大化 / 关闭按钮点击无反应：根因是应用 capabilities 未授予对应窗口权限，按钮的窗口控制调用被 Tauri ACL 静默拒绝（按钮本身没问题）。现补齐 minimize / toggle-maximize / close 权限，三个按钮恢复可用。

---

### 🪟 Windows (fixes)
- Fixed the minimize / maximize / close buttons on the custom title bar doing nothing when clicked: the app's capabilities did not grant the matching window permissions, so the buttons' window-control calls were silently rejected by Tauri's ACL (the buttons themselves were fine). The minimize / toggle-maximize / close permissions are now granted and all three buttons work again.

## v0.3.7

### 🪟 Windows（修复）
- 修复主界面出现两个标题栏：此前 Windows 上系统原生标题栏与应用自绘标题栏同时显示。现 Windows 关闭原生标题栏，统一为单条自绘标题栏，并补上最小化 / 最大化 / 关闭按钮。
- 修复点击主界面窗口直接消失：此前在窗口内部点击会被误判为失焦而隐藏到托盘。现仅在真正切换到别的应用时才隐藏，窗口内点击、拖动标题栏移窗均正常。
- 修复「粘贴到前台」在 Windows 上无效：此前只把内容写入剪贴板、不会真正粘贴。现 Windows 也会在唤起时记录前台窗口，粘贴时把焦点交回该窗口并模拟 Ctrl+V，直接粘进前台应用；还焦失败时退化为仅写回剪贴板、需手动粘贴。

---

### 🪟 Windows (fixes)
- Fixed the main window showing two title bars: previously the native Windows title bar and the app's custom title bar were both displayed. Windows now hides the native title bar and uses a single custom one, with minimize / maximize / close buttons added.
- Fixed the main window vanishing on click: clicking inside the window was previously misread as losing focus and hidden to the tray. It now hides only when you actually switch to another app; clicking inside and dragging the title bar to move the window both work correctly.
- Fixed "Paste to foreground" doing nothing on Windows: previously it only wrote to the clipboard without actually pasting. Windows now records the foreground window on activation, returns focus to it and simulates Ctrl+V on paste, pasting straight into the frontmost app; if focus restore fails it falls back to writing the clipboard only (paste manually).

## v0.3.6

### 🎨 界面（修复）
- 修复暗色模式下滚动条显示为白色：在「系统设置›外观›显示滚动条=始终」或外接鼠标的 Mac 上，列表与面板的滚动条此前一律渲染成刺眼的浅色、不跟随主题。现滚动条会跟随亮/暗主题正确着色。
- 翻译页去掉两处多余的深色背景块：原文输入框下方的「翻译」按钮行、以及译文卡片内的「复制/朗读」按钮行，此前各多出一块与面板不一致的深色矩形，现已透明融入。

---

### 🎨 Interface (fixes)
- Fixed scrollbars showing up white in dark mode: on Macs with "System Settings › Appearance › Show scroll bars = Always" (or a mouse plugged in), list and panel scrollbars were previously rendered in a glaring light color that ignored the theme. Scrollbars now follow the light/dark theme correctly.
- Removed two stray dark background blocks on the translate page: the "Translate" button row below the source input and the "Copy/Read aloud" button row inside the result card each had an extra dark rectangle that clashed with the panel; they now blend in transparently.

## v0.3.5

### 📋 剪贴板
- 图片条目现在可以「复制」和「一键粘贴到前台」了——此前图片项点这两个按钮无反应（仅文本/富文本可用）。复制会把原图写入系统剪贴板，已授辅助功能权限时「粘贴到前台」会直接把图片粘进前台应用，未授权则退化为仅写回剪贴板、需手动 `Cmd/Ctrl+V`。
- 快捷弹窗用方向键选择条目时，列表会跟随高亮自动滚动，选中项不再滚出可视区；同时自定义了弹窗滚动条配色，与整体风格统一。

### 🎨 界面
- 统一了全应用底部操作条的样式：快捷弹窗、迷你翻译、翻译页、剪贴板预览等处的底部按钮条现在共用同一套视觉规范（高度、间距、分割线、对齐一致）。
- 关于页改版：换上应用图标、名称与标题栏对齐，移除了 Tauri 默认标记。

---

### 📋 Clipboard
- Image entries can now be **Copied** and **Pasted to the foreground** — previously these two buttons did nothing for images (text/rich-text only). Copy writes the original image to the system clipboard; with Accessibility permission granted, "Paste to foreground" pastes the image straight into the frontmost app, otherwise it falls back to writing the clipboard only (paste manually with `Cmd/Ctrl+V`).
- When navigating the quick popover with arrow keys, the list now auto-scrolls to follow the highlight so the selected item never scrolls out of view; the popover scrollbar was also restyled to match the overall look.

### 🎨 Interface
- Unified the styling of bottom action bars across the app: the quick popover, mini translate, translate page, and clipboard preview now share one visual spec (consistent height, spacing, dividers, and alignment).
- Redesigned the About page: now uses the app icon, aligns the name with the title bar, and removes the default Tauri branding.

## v0.3.4

### ⌨️ 热键
- 新增「应用主界面」全局热键，默认 `Cmd/Ctrl+Shift+M`，可在设置页与剪贴板历史、翻译热键一起修改。
- 改键流程更稳健：运行时注册新热键成功后才保存配置并注销旧热键；如果新热键被系统或其他应用占用，旧配置和旧热键都会保留。

---

### ⌨️ Hotkeys
- Added a global hotkey for the main app window, defaulting to `Cmd/Ctrl+Shift+M`, configurable alongside clipboard history and translation hotkeys in Settings.
- Made rebinding safer: QuickQuick now registers the new hotkey before saving config and unregistering the old one, so conflicts keep the existing shortcut intact.

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
