---
document_kind: plan
render_html: true
---

# QuickQuick 对标 Maccy 全面升级方案

- 状态：待确认
- 日期：2026-10-24
- 目标版本：v0.4.x → v0.5.x
- 依据：REA 对 Maccy 2.6.1 发布态的逆向取证 + Maccy 2.6.1 源码对照 + QuickQuick 全域只读代码审计

---

## 0. 摘要：五个必须先接受的判断

在给出方案前，有五个判断会直接改变方案形状。它们不是修辞，是本轮调研推翻或确认的事实。

**判断一：Maccy 的功能面比「标杆」印象窄得多。**
实测它的功能边界是：6 类 pasteboard 类型、200 条上限、4 种搜索模式、6 个设置页、**0 个右键菜单、0 个多选、0 个粘贴栈、0 个导入导出、0 个多词 AND 搜索**。它没有 OCR 之外的内容理解，没有翻译，存储不加密。

所以「全面看齐 Maccy」如果按字面理解，是一个**比想象中小**的目标。真正值得看齐的是它把一条窄路径打磨到极致的部分：交互模型、捕获完整性、规则系统、搜索手感、本地化工程。

**判断二：粘贴栈不存在，不要为它立项。**
调研初期我曾把它列为 Maccy 的核心能力（这也是社区常见误解）。全量枚举 76 个 Swift 文件后确认：**Maccy 2.6.1 没有任何 Paste Stack / 队列 / 多选实现**。它用两套机制覆盖「连续粘贴」需求：

- **字母 pin**：`pin` 字段存单个字母（`b`–`y`，排除 `a/q/v/w/z`），20 个键位持久化在库里，重启不变。按 `b`/`c` 即取该条粘贴。
- **数字直达**：前 10 条 unpinned 自动绑定 `1`–`0`。

体验上比粘贴栈**少一步**（不需要先组队列），但覆盖不了「临时挑 5 条按顺序贴进一个表单」。这是 QuickQuick 可以**超越**的地方，不是「看齐」项。

**判断三：交互重构的技术路径已被同栈项目验证，风险从「成败未定」降为「适配工作量」。**
Maccy 交互体验的根基是 `NSPanel(.nonactivatingPanel)`——**点开面板不激活 Maccy**。这一个决策消灭了整类 bug：不需要记录/恢复前台应用，不需要 `NSApp.activate`，粘贴必然落到正确目标，用户原应用的光标与选区完全不变。

初版方案把「Tauri webview 能否在非激活面板拿到键盘焦点」列为单点成败风险。**这个判断过重了，已被实证推翻**：

- 存在成熟库 `tauri-nspanel`（crates.io 2.1.1，12,593 下载），定位即「subclassing Tauri's NSWindow to NSPanel」，机制是 `object_setClass` 把 `NSWindow` 改成动态生成的 panel 子类。
- 存在同技术栈、同品类的成功先例：**EcoPaste**（Tauri v2 剪贴板管理器，v1.1.0）已在 `src-tauri/src/window/macos.rs` 落地了与 Maccy 同构的方案：

```rust
tauri_panel! {
    panel!(MainPanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,      // 能成为 key window，接收键盘
            can_become_main_window: false
        }
    })
}
panel.set_style_mask(StyleMask::empty().resizable().nonactivating_panel().into());
panel.set_collection_behavior(
    CollectionBehavior::new().stationary().move_to_active_space().full_screen_auxiliary().into()
);
panel.show_and_make_key();   // ← 等价 Maccy 的 makeKey()：拿键盘焦点但不激活 App
```

- 它甚至已踩过并解决了最隐蔽的坑，注释写得很明确：**面板处于 key window 时，合成出来的 `CGEvent ⌘V` 会被面板自己吞掉**。解法是粘贴前 `resign_key_window()`，粘贴后 `make_key_window()` 把焦点拿回来（`macos.rs:180-204`）。

所以 Phase 0 的性质从「可行性验证」变为「**接入适配 + 关键坑复现**」：不是问「能不能做」，而是问「在 QuickQuick 现有 `popover.rs` 的焦点编排（`NSApplication.activate()` + `set_focus`）之上，如何替换成 panel 语义，并保留已有的辅助功能降级路径」。

参考实现的价值不止于此——EcoPaste 还解决了全屏 Space 下的显示策略（`move_to_active_space` 与 `can_join_all_spaces` 的切换时机）、失焦自动隐藏（`window_did_resign_key`）、`to_panel` 的逆操作与生命周期还原（`lifecycle/mod.rs:650`）。这些都是 QuickQuick 会遇到的同类问题。

**判断四：QuickQuick 当前最大的问题不是缺功能，是「承诺与实现脱节」。**
审计发现一批**代码写了但运行期不可达**的能力：

| 能力 | 实现状态 | 实际效果 |
|---|---|---|
| 跳过敏感内容（concealed/transient） | `privacy.rs:97-117` 逻辑完整 | `is_concealed` 硬编码 `false`，**永不生效** |
| 按来源应用排除 | 名单管理链齐全 | `source_app` 硬编码 `None`，**永不生效** |
| 防自污染（私有 UTI） | trait 文档承诺 | `has_self_marker` 恒 `false`，**未实现** |
| 翻译缓存（四元组 + LRU） | `cache.rs` 196 行 + 建表 | **零生产调用**，每次翻译都走网络 |
| 翻译重试 / 取消 / 错误归一 | `retry.rs`/`cancel.rs`/`error.rs` | **全部零生产调用** |
| 可移植备份（口令加密包） | `portable.rs` 格式加密完整 | **只被测试调用**，无 IPC、无 UI |
| 分级容量清理（500MB） | `image.rs:413` 有实现有测试 | **零调用**，而存储页 UI 已向用户承诺此行为 |

这类问题的严重性高于「功能缺失」：**设置页在向用户撒谎**。用户配了排除名单、开了敏感跳过，实际密码管理器内容照样入库。

**判断五：翻译、加密、跨平台、富文本是 Maccy 结构上不可能有的资产，重构不能伤到它们。**
QuickQuick 有 21 个翻译源、SQLCipher 整库加密 + 机器绑定密钥、Windows 双端、HTML 全链路保真、内置签名自动更新、离线 ECDICT 词典。这是差异化护城河。任何交互重构都必须先确认不破坏这些链路。

---

## 1. 调研方法与证据边界

### 1.1 用了什么

| 手段 | 对象 | 产出 |
|---|---|---|
| REA `inspect_macho` / `inspect_plist` / `inspect_signature` / `inspect_artifact` | Maccy 2.6.1 发布态 | 依赖（仅 Sparkle 2.6.4）、31 语言、`LSUIElement`、沙盒与 XPC 更新架构 |
| `strings` + `__cstring` / `__swift5_reflstr` | Maccy 发布态 | 默认忽略类型 5 项、数据库路径、UI 文案、类名 |
| Maccy 2.6.1 源码（MIT，tag 对齐） | 实现逻辑 | 能力矩阵、设计决策、键位表、淘汰策略、本地化工程 |
| 只读代码审计 | QuickQuick 全域 | 能力矩阵、死代码清单、性能链条 |

### 1.2 反编译器：已装但未接入，且判定为**非关键路径**

本轮已安装 OpenJDK 21 + Ghidra 12.1.4（`~/tools/ghidra_12.1.4_PUBLIC`），REA `doctor` 对 install/version/headless/java 四项均报 healthy，但卡在最后一步：Ghidra 官方发布包**只含 `linux_x86_64` 与 `win_x86_64` 的 native decompiler**，macOS arm64 需本地编译，而 Makefile 硬编码 x86_64、depend 规则不透传 BFD include，两次编译均失败。

**判定放弃的理由**（不是「懒得装」）：

1. Maccy 是 MIT 开源，源码可读，比伪代码更快更准；
2. 发布版与源码的一致性已由 `sha256` 绑定 + 类名/字符串逐条交叉印证（2.6.1 / build 60 与 appcast pubDate 一致）；
3. REA 的 Ghidra provider 对 universal Mach-O 报 `architecture_unsupported`，还需先 thin slice，多一层麻烦；
4. Maccy 二进制已 strip（exports 仅 1 个），Swift 类型名在元数据 section，反编译产出是 `FUN_xxx` + 指针运算，边际价值低。

**保留状态**：Ghidra 已就位，未来若需要验证**闭源**目标（无源码可读时），只差编译 native decompiler 这一步。

### 1.3 拿不到的（明确记录）

- Maccy 的实际 SQLite 物理 schema（本机容器为空，从未使用过该 app；物理 schema 由 SwiftData 运行时生成）
- SwiftUI 视图树的精确层级与 modifier 顺序（SwiftUI 无 nib，IB 解析不到 Maccy 自身文档）
- Glass / VisualEffect 的最终视觉参数（系统渲染决定）
- **运行时观察 = 0 项**：Maccy 的 `FloatingPanel` 在 `resignKey()` 里无条件 `close()`，窗口存活约 2s，跨不过一次 MCP 调用；REA 只接受 onscreen 窗口，而该 panel 的 `kCGWindowIsOnscreen` 为空

影响评估：**不影响**能力矩阵与设计决策；**轻度影响**像素级 UI 复刻（需真机开一次 Maccy 校准行高/留白/字体）。

---

## 2. 差距分析：四层 + 一层债

把差距按「用户可感知的先后顺序」分层，而不是按模块分。

### L0 — 诚信债（应最先还）

见判断四。这批问题的共同特征是：**UI 已承诺、代码未接线**。还债不需要新设计，只需要接线或诚实下线。

### L1 — 交互模型（用户明确说最痛）

| 维度 | Maccy | QuickQuick 现状 | 差距性质 |
|---|---|---|---|
| 面板激活语义 | `NSPanel(.nonactivatingPanel)`，不夺焦 | 普通窗口 + `NSApplication.activate()`（`popover.rs:122-152`） | **架构级** |
| 唤起位置 | 5 种可选（光标/图标/窗口/中心/上次），多屏可选 | 固定公式：水平居中 + 距顶 15%（`window_pos.rs:17-55`） | 能力级 |
| 高度自适应 | 三段 `GeometryReader` 上报 + 0.2s 动画，顶边固定 | 固定 720×480 | 能力级 |
| 键盘模型 | 30+ 键位（vim `⌃N/P/J/K`、`⌘↑↓` 跳首尾、`⌃W` 删词、`⌥⌫` 删除、`⌥P` pin、`⌘,` 设置…） | 4 个：`↑↓` / `Enter` / `⌥Enter` / `Esc` | **数量级** |
| 数字直达 | `1`–`0` 取前 10 条 | 无 | 能力级 |
| 字母 pin | `b`–`y` 持久化键位 | 只有 `is_favorite` 布尔，无键位 | 能力级 |
| cycle 模式 | 按住 `⇧⌘C` 连按 `C` 翻页、松手即粘贴 | 无 | **体验杀手锏** |
| 输入法防御 | `hasMarkedText` 拦截（中文拼音期间不响应方向键） | 无（webview 有 composition events，但未用） | 必须自研 |
| 行内预览 | `NSPopover` + 分级延迟（首 1500ms→后续 200ms） | 有预览面板，无分级延迟 | 打磨级 |
| 快捷键徽标随修饰键变 | 每条目 3 变体，按当前修饰键切换 | 静态 footer 4 个提示 | 打磨级 |
| 主窗键盘流 | — | **主窗 Enter 无动作**（`ClipboardPage.tsx:156-159` 丢弃返回值） | **bug 级** |
| popover 管理动作 | pin / 删除 / 清空 | 无（只能 Enter 粘、⌥Enter 复制） | 能力级 |
| 悬停与键盘仲裁 | 鼠标移动即交还主导权 | 无 | 打磨级 |

### L2 — 捕获与规则（基本功）

| 维度 | Maccy | QuickQuick | 差距 |
|---|---|---|---|
| 轮询机制 | `changeCount` 只比整数 | **内容哈希伪计数**：每 500ms 全量读 text+html+image 再构造合并 Vec 哈希（`pipeline.rs:86-134`） | **性能根因** |
| 捕获类型 | 硬白名单 6 类，含**文件**（`.fileURL`） | 文本 / HTML / 图片，**无文件** | 能力级 |
| 任意 UTI 遍历 | `pasteboard.types()` 全量 | arboard 抽象，拿不到任意 UTI | 架构级 |
| 多 pasteboard item 合并 | 支持（BBEdit/Edge 双 item） | 无 | 能力级 |
| 去重 | `supersedes()` 集合包含判定 + `numberOfCopies` 累加 | 单 hash 精确匹配（`text_hash`/`image_hash`） | 精度级 |
| 忽略规则 | **三层**：类型集合 / 来源应用（黑或白名单）/ 内容正则 | 三层都有代码，**全部空接线**（见 L0） | 接线级 |
| 暂停 | `⌥` 点图标切换；`⌥⇧` 点仅忽略下一次 | 仅设置页开关，托盘无 | 能力级 |
| 自产标记 | 写回时置 `org.p0deje.Maccy` 类型 | 未实现 → **粘贴后自己再捕获一次、污染历史顺序** | 接线级 |
| 动态类型剔除 | 丢弃 `dyn.*`、OLE、Word 书签三件套 | 无 | 边缘 |
| OCR | 图片异步跑 Vision，回填 `title` → 图片可被文字搜到 | 无（图片 `content` 是 `[图片] W×H`） | 能力级 |
| Universal Clipboard 识别 | 显示 "iCloud" 标记 | 无 | 边缘 |

### L3 — 存储与性能（规模可持续性）

| 维度 | Maccy | QuickQuick | 差距 |
|---|---|---|---|
| 索引 | SwiftData 自动 | **零 `CREATE INDEX`**（`db.rs:745-812`） | 硬伤 |
| 列表加载 | 内存常驻 + 预计算 `title` | **无 LIMIT 全表** + 全量排序（`db.rs:272-280`） | 硬伤 |
| 图片 IPC | 原生视图直接持有 `NSImage` | **每条内联 base64 缩略图**（`ipc/clipboard.rs:52-66`），每次刷新全量重传 | 硬伤 |
| 自动淘汰 | `add` 内自动 `limitHistorySize`，pin 无限豁免 | `tiered_cleanup` 死代码；只剩手动按钮（500 条硬编码） | 硬伤 |
| 搜索 | 预计算 title + 内存过滤 + 4 模式 + 200ms 防抖 + 命中高亮 | 纯前端 `includes` 子串，无模糊/无高亮/无多词 AND | 能力级 |
| 容量上限 | 1–999 可配，默认 200 | 硬编码 500，不可配 | 配置级 |
| 备份迁移 | 无（Maccy 也没有） | `portable.rs` 完整实现但**未接线** | 接线级 |
| 迁移框架 | 布尔开关 `migrations[...]` | 1 个 ad-hoc 函数，`PRAGMA user_version` 只做解密探测 | 工程级 |

### L4 — 产品化

| 维度 | Maccy | QuickQuick | 差距 |
|---|---|---|---|
| 托盘菜单 | 图标可显示最近内容；`⌥`/`⌥⇧` 点击语义 | **只有「显示」「退出」2 项** | 能力级 |
| 系统通知 | `UNUserNotificationCenter`，body = 条目标题 | **零系统通知**（全仓无 notification 插件） | 能力级 |
| 音效 | `Knock.caf`（取用）/ `Write.caf`（捕获） | 无 | 能力级 |
| 首次引导 | 有设置向导 | 只有一张辅助功能卡片，且授权后仍会再出现一次 | 打磨级 |
| 辅助功能引导 | `AXIsProcessTrusted` **只检查不提示**（Maccy 的缺陷） | 检查 + 降级 + 深链横幅（**比 Maccy 好**） | QQ 领先 |
| 设置项 | 6 页 / 约 40 项，每项带 tooltip 写默认值 | 6 区 / 8 项 | 数量级 |
| i18n | **31 语言**（BartyCrouch + DeepL 管线，`knownRegions` 作出货闸门） | **纯中文硬编码**，无 i18n 框架 | 数量级 |
| 排序偏好 | 3 种（最近/最早/复制次数） | 固定按收藏 + 最近 | 能力级 |
| CI | GitHub Actions | **只发布不验证**（唯一 workflow 是 `release.yml`，不跑测试/tsc/clippy） | 工程级 |

---

## 3. 需要你裁决的五个取舍

方案的形状取决于这五个问题的答案。我给出推荐，但这是你的产品判断。

### Q1｜交互架构路线：已定，走非激活面板（附适配 spike）

**结论：走 Maccy/EcoPaste 路线，不再考虑「激活 + 前台应用恢复」的降级方案。**

理由见判断三：`tauri-nspanel` + EcoPaste 已证明 Tauri v2 webview 能在非激活面板里拿到键盘焦点，且关键坑（粘贴时面板吞掉 `CGEvent ⌘V`）已有明确解法。

仍保留一个**短 spike（约 1 天）**，但性质是「适配验证」而非「可行性验证」，只确认三件事：

1. QuickQuick 现有 `popover.rs:122-152` 的 `NSApplication.activate()` + `set_focus()` 编排，替换为 `show_and_make_key()` 后，原前台应用是否仍保持 active
2. webview 内 `<input autofocus>` 能收到字符（中文输入法 composition 事件是否正常）
3. 「`resign_key_window` → 注入 ⌘V → `make_key_window`」这套编排，与 QuickQuick 现有「hide 窗口 + hide app + 等 350ms + 注入」（`ipc/system.rs:69-78`）如何合并——**注意现有代码是「隐藏整个 app」，panel 语义下应改为「仅 resign key」**，这是最容易改错的一处

**为什么不能跳过这个 spike**：不是怀疑可行性，而是 QuickQuick 现在的粘贴编排是围绕「隐藏 app」设计的。panel 化之后这个假设失效，改错会导致「粘贴到错误应用」或「面板消失后无法继续操作」。

### Q2｜「全面看齐」的范围：一次到底，还是分两版？

**推荐：分两版。v0.4 还债 + 交互地基；v0.5 捕获/规则/存储/产品化。**

理由：L0 诚信债 + L1 交互是用户当前体感最痛的两块，且它们互相独立于 L2/L3。L2 的「任意 UTI 遍历」需要换掉 arboard、直连 `NSPasteboard`，是架构级改动，风险独立，不该和交互重构挤在同一版。

### Q3｜i18n：做 31 语言，还是先中英双语？

**推荐：v0.5 只做「i18n 框架 + 中英双语」，语言数后续按需求扩。**

Maccy 的 31 语言靠 BartyCrouch + DeepL 机翻管线自动铺开，但它的**出货闸门是 `knownRegions` 白名单**——机翻铺满 41 个目录，只有被信任的 31 个进包。这个工程思路值得抄（框架 + 白名单），但一次性上 31 语言对当前阶段是负担，且 QuickQuick 的文案量（6 区设置 + 3 个窗口）远大于 Maccy 的窄界面。

### Q4｜死代码：接线还是删除？

**推荐：逐个判定，不搞一刀切。**

| 模块 | 建议 | 理由 |
|---|---|---|
| 隐私三件套（concealed/source_app/self_marker） | **接线** | 用户已能看到配置项，且这是隐私承诺 |
| `portable.rs` 备份 | **接线** | 解决「换机即失去全部历史」的真实痛点，格式已完整 |
| `tiered_cleanup` | **接线** | 存储页已承诺此行为 |
| 翻译 `cache.rs` | **接线** | 直接减少网络请求与免费源限流 |
| 翻译 `retry.rs` / `cancel.rs` / `error.rs` | **接线** | 免费源失败率高的根因 |
| `AppStoreReview` 类死代码（Maccy 侧） | 不适用 | — |

### Q5｜翻译域是否同期修？

**推荐：v0.4 顺带接线（缓存/重试/取消），不新增翻译功能。**

理由：翻译是差异化资产，但当前「21 个源下面没有缓存/重试/取消」的状态会持续损耗它的价值。接线是低成本高回报，而新增源/流式输出/划词即译应等剪贴板域稳定后再说。

---

## 4. 分阶段方案

### Phase 0｜接入适配 spike（约 1 天）

**性质**：不是可行性验证（已有先例），是**接入方式验证**。

| 任务 | 产出 | 通过标准 |
|---|---|---|
| 引入 `tauri-nspanel`，把 `clip-popover` 窗口 `to_panel` 成 `NSPanel`，置 `can_become_key_window: true` + `nonactivating_panel` styleMask | 可运行 PoC | 面板打开时 `NSWorkspace.frontmostApplication` 不变 |
| 替换现有焦点编排：`NSApplication.activate()` + `set_focus()` → `show_and_make_key()` | 改动点清单 | webview 搜索框能输入；中文输入法 composition 正常 |
| 复现并解决「面板 key 状态下吞掉 `CGEvent ⌘V`」 | 粘贴编排方案 | 「resign key → 注入 ⌘V → make key」链路可用 |
| 全屏 Space 显示策略验证 | 策略结论 | 全屏应用下唤起面板可见 |
| 决策记录 | ADR：panel 化改造范围 | 明确哪些窗口 panel 化、哪些保持常规窗口 |

**Gate**：这一阶段不通过则 L1 方案退回降级路线；通过则 L1 全部可做。

**参考实现**：EcoPaste `src-tauri/src/window/macos.rs`（完整 panel 生命周期）、`window/lifecycle/mod.rs:650`（`to_panel` 逆操作与还原）。

### Phase 1｜还诚信债（约 1 周）

| 任务 | 涉及文件 | 验收 |
|---|---|---|
| 敏感内容过滤接线（`concealed`/`transient` 真实读取） | `pipeline.rs:187`、`macos_paste.rs`、`clipboard.rs` | 从 1Password/Bitwarden 复制密码不入库 |
| 来源应用排除接线（`source_app` 真实读取 + bundle id 匹配） | 同上 + `privacy.rs` | 名单内 app 的复制不入库 |
| 私有 UTI 自产标记（写回时附加，读取时跳过） | `macos_paste.rs:146-152`、`paste.rs:49` | 从历史粘贴后不产生新条目、不打乱顺序 |
| `tiered_cleanup` 接线（自动触发） | `image.rs:413`、`lib.rs` | 超过 500MB 自动两级清理 |
| `portable.rs` 暴露 IPC + UI（导出/导入） | `portable.rs`、`ipc/*`、设置页 | 能导出加密备份包并在新机导入 |
| 翻译 cache/retry/cancel/error 接线 | `ipc/translate.rs`（`translate_text_impl` 编排段）、`translate/{cache,retry,cancel,error}.rs` | 同文本二次翻译零网络；瞬时失败自动重试；连点丢弃过期结果 |

**落地记录**：Phase 1 七个 item 已完成并各自独立 commit（`8ee670a`…`b2245da`），逐项结论与未闭环的人工验证见 [v04-work-items.md](v04-work-items.md)。其中「敏感内容过滤接线」在独立审查中被判 BLOCK（`NSPasteboard` 跨线程访问必崩），修复后复审闭环；由此暴露的「串行化全部 `NSPasteboard` 访问」作为 **P1-8** 挂号，与 Phase 3 的 `changeCount` 改造一起做。

**为什么排第一**：这批是「UI 已承诺、代码未做到」，修复成本低、信任收益高，且不依赖任何架构决策。

### Phase 2｜交互地基重构（2–3 周，依赖 Phase 0 结论）

| 任务 | 说明 |
|---|---|
| 面板语义改造 | 引入 `tauri-nspanel`，`clip-popover` 转 `NSPanel`（`can_become_key_window: true` + `nonactivating_panel`）；替换现有 `NSApplication.activate()` 编排为 `show_and_make_key()`；高度自适应（前端测高 → IPC → `set_size` 同步 `set_position` 保顶边） |
| 键盘模型升级 | 照抄 Maccy 键位表的可用子集：`⌃N/P/J/K` 上下、`⌘↑↓` 首尾、`⌃W` 删词、`⌃U` 清搜索、`⌃H` 删字符、`⌥⌫` 删除、`⌥P` pin、`⌘,` 设置 |
| 数字直达 | `1`–`0` 取前 10 条（含 50ms 高亮反馈） |
| 字母 pin | `pin` 字段改存字母，20 键位持久化；设置页加 Pins 管理 |
| cycle 模式 | 按住热键连按翻页、松手即粘贴（`NSEvent` 全局 `flagsChanged` 监听） |
| 输入法防御 | webview `compositionstart/update/end` 期间禁用方向键导航 |
| 主窗键盘流修复 | Enter 粘贴、`⌘1-9` 直接粘贴（当前只移动高亮） |
| popover 管理动作 | pin / 删除 / 清空，配 `⌥⌫` / `⌥P` |
| 唤起位置可选 | 光标 / 托盘图标 / 屏幕中心 / 上次位置（多屏） |
| 行内预览分级延迟 | 首次 1500ms，后续 200ms |
| 快捷键徽标随修饰键变 | 前端跟踪 modifier，`opacity` 切换 |
| 悬停与键盘仲裁 | 鼠标移动即交还主导权 |

### Phase 3｜捕获与规则（约 2 周）

| 任务 | 说明 |
|---|---|
| **换掉 arboard，直连 `NSPasteboard`** | `types()` 全量遍历 + `data(forType:)`；这是支持任意 UTI、文件、多 item 合并的前提 |
| `changeCount` 替代内容哈希轮询 | 消除「每 500ms 全量读取 + 哈希」的性能根因 |
| 类型白名单 | 对齐 Maccy 6 类（`fileURL`/`html`/`png`/`rtf`/`string`/`tiff`），可配 |
| 文件类型条目 | 捕获 + 图标 + 复制回写 |
| 忽略规则三层接线增强 | 应用黑/白名单 + 类型集合 + 内容正则（提示 Rust `regex` 与 `NSRegularExpression` 的语法差异） |
| 暂停语义补齐 | 托盘 `⌥` 点击切换、`⌥⇧` 仅忽略下一次 |
| 多 pasteboard item 合并 | 与 Maccy 同构 |
| 去重精度提升 | 集合包含判定 + 复制次数累加 |
| OCR（图片可搜） | **建议 Swift helper + IPC**（进程隔离，OCR 是低频路径），不硬啃 `objc2-vision` |
| 动态类型/OLE/Word 书签剔除 | 边缘规则，一并补 |

### Phase 4｜存储与性能（约 2 周）

| 任务 | 说明 |
|---|---|
| 建索引 | `clip_items(is_deleted, last_modified_utc)`、`text_hash`、`clip_images(image_hash)` |
| 分页 + 增量加载 | `list_items_with_images` 加 LIMIT/OFFSET；前端虚拟滚动（`@tanstack/react-virtual`，固定行高） |
| 缩略图移出 IPC payload | 改为按需拉取或自定义协议 URL，消除 base64 全量重传 |
| 自动淘汰 | 接 `tiered_cleanup` + 可配上限（对齐 Maccy 的 1–999 语义，pin 豁免） |
| 搜索升级 | 预计算 `title` 落库（截断 1000 字符）→ 内存过滤 + 4 模式（exact/fuzzy/regex/mixed）+ 200ms 防抖 + 命中高亮；**加多词 AND（Maccy 没有，是我们的超越点）** |
| 翻译历史分页 | 加 LIMIT + 清理 |
| 迁移框架版本化 | 用 `PRAGMA user_version` 承担 schema 版本，替代当前 ad-hoc 函数 |

### Phase 5｜产品化（2–3 周）

| 任务 | 说明 |
|---|---|
| 托盘菜单补齐 | 暂停捕获 / 清空未固定 / 打开设置 / 检查更新 / 版本号；图标可显示最近内容（可选） |
| 系统通知 | `tauri-plugin-notification`，用于「更新就绪」「清理完成」等低频事件 |
| 音效 | 捕获 / 取用两个音效；**与通知开关解耦**（Maccy 把音效挂在通知权限上，是缺陷，别抄） |
| 首次引导向导 | 多步：欢迎 → 热键介绍 → 辅助功能授权 → 自启确认；授权后自动恢复（当前 OnboardCard 授权后仍会再出现一次） |
| 设置项扩充 | 排序方式、图片高度、预览延迟、命中高亮样式、容量上限、弹出位置 |
| 每项设置加 tooltip 写明默认值 | 抄 Maccy 的文案约定 |
| i18n 框架 + 中英双语 | `i18next` + namespace 按区域分（对齐 Maccy 8 张表的思路）；CI 维护 `shipped_locales` 白名单作出货闸门 |
| 设置页加搜索 | 项数增长后必要 |

### Phase 6｜工程加固（贯穿，约 1 周）

| 任务 | 说明 |
|---|---|
| CI 加验证 | 现有 `release.yml` 只发布不验证；新增 PR/push workflow 跑 `make verify`（tsc + fmt + clippy + 双端测试） |
| 覆盖率基线 | 当前无 coverage 配置；先建立基线再谈阈值 |
| 死代码检测 | 引入 `knip`（前端）/ `cargo-udeps` 或人工清单，防止再次出现「写完未接线」 |

---

## 5. 不做的事（明确边界）

| 项 | 理由 |
|---|---|
| **粘贴栈 / 多选队列** | Maccy 没有；它用字母 pin + 数字直达覆盖了 80% 场景。若要做，是**超越项**，应单独立项，不在「看齐」范围内 |
| **右键菜单** | Maccy 没有（全量 grep 0 命中）。QuickQuick 现有 `⌥⌫`/`⌥P` 键位已能覆盖，右键菜单收益低于成本 |
| **AppIntents / Shortcuts** | Tauri 无法生成 `Metadata.appintents`，这是硬边界 |
| **沙盒** | Maccy 因沙盒才需要 `Downloader.xpc`/`Installer.xpc` 那套复杂架构。QuickQuick 不上沙盒，`tauri-plugin-updater` 直接够用，**不要复刻** |
| **Core Data / SwiftData** | 无意义，继续用 `rusqlite` |
| **像素级 1:1 复刻 Maccy 视觉** | 行高 22pt、padding 5/10 等常量可参考，但 QuickQuick 有自己的 OKLch token 体系，应保持视觉自主 |
| **Ghidra native decompiler 编译** | 见 §1.2，非关键路径；已保留 Ghidra 待未来验证闭源目标 |

---

## 6. 风险登记

| 风险 | 等级 | 应对 |
|---|---|---|
| **panel 化与现有粘贴编排冲突**：现有实现是「hide 窗口 + hide app + 等 350ms + 注入」（`ipc/system.rs:69-78`），panel 语义下正确做法是「仅 `resign_key_window`」，改错会粘贴到错误应用 | 中 | Phase 0 明确改动点清单；粘贴链路加回归测试；保留现有降级路径作为兜底 |
| `tauri-nspanel` 上游稳定性：EcoPaste 用的是 git `branch = "v2.1"` 而非 crates.io 版本，说明可能需要跟主干 | 低 | 锁定 commit（`a3122e8`）；若上游停滞，参考实现足够自写 |
| **换掉 arboard 直连 NSPasteboard** 破坏现有捕获/粘贴链路 | 高 | Phase 3 单独成版；先补集成测试覆盖现有捕获路径再动手 |
| **cycle 模式需全局 `flagsChanged` 监听** | 中 | 用 `NSEvent.addGlobalMonitorForEvents`（比 `CGEventTap` 轻），依赖已有辅助功能权限 |
| **i18n 改造触碰全部前端文案** | 中 | 只在中英双语阶段做；用 `i18next-parser` 抽取，避免手改遗漏 |
| **交互重构伤到翻译/加密链路** | 中 | Phase 2 前后各跑一次全量 `make verify`；翻译域加回归测试 |
| **索引 + 分页改动引发数据兼容问题** | 中 | 借 Phase 4 的迁移框架版本化一起做，带幂等守卫 |
| 工作量超预期（全案约 10–13 周） | 中 | 按 Q2 分两版交付，每版可独立发布 |

---

## 7. 验证方式

每阶段完成时必须实跑，不接受「应该可以」：

| 阶段 | 验证 |
|---|---|
| Phase 0 | PoC 三条断言实跑；录屏或日志为证 |
| Phase 1 | 逐项手工验证：1Password 复制不入库、名单 app 不入库、粘贴不产生新条目、导出包能在干净环境导入、同文本二次翻译零网络（抓包或日志） |
| Phase 2 | 键位表逐条手测；`cycle` 模式松手粘贴落到正确应用；中文输入法拼音期间方向键不劫持 |
| Phase 3 | 文件复制可入库；多 item 合并；忽略三层各命中一次；500ms 轮询 CPU 占用前后对比 |
| Phase 4 | 造 2000 条含图数据，测量列表首屏耗时与 IPC payload 体积（前后对比）；搜索 4 模式 + 多词 AND |
| Phase 5 | 31 语言白名单机制验证（先中英）；托盘每项可用；引导全流程 |
| Phase 6 | CI 在 PR 上真实跑绿 |

日常用 `make verify`；CI、发布、版本裁决时才跑全量。

---

## 8. 一页纸结论

Maccy 不是「功能更多的剪贴板管理器」，而是「把窄路径打磨到极致的交互范本」。它的杀手锏只有三个：**非激活面板**（消灭前台应用恢复类 bug）、**cycle 模式**（把选+粘压成一次肌肉动作）、**预计算 title + 内存过滤**（200 条列表秒开）。它的空缺（无引导、无 AND 搜索、无导入导出、音效搭通知权限）可以直接超越。

QuickQuick 要「全面看齐」，真正的工程量集中在**十个原生桥接模块**。交互架构那条路已经通了——`tauri-nspanel` + EcoPaste 证明了 Tauri v2 webview 能在非激活面板里拿到键盘焦点，连「面板吞掉合成 ⌘V」这个最隐蔽的坑都有现成解法。剩下的是适配与工作量，不是赌运气。

在此之前，有一批更该先还的债：**设置页正在向用户承诺它并未实现的能力**。这件事的优先级高于任何新功能。

---

## 附：裁决结果（2026-10-24 确认）

| 编号 | 问题 | 结论 |
|---|---|---|
| Q1 | 交互架构 | 走非激活面板（`tauri-nspanel`），保留 1 天适配 spike，不做降级路线 |
| Q2 | 交付节奏 | **分两版**：v0.4 = 还债 + 交互；v0.5 = 捕获 + 存储 + 产品化 |
| Q3 | i18n | **框架 + 中英双语**，语言数后续按需扩，CI 维护出货白名单 |
| Q4 | 死代码 | **逐项接线**（隐私三件套 / portable / tiered_cleanup / 翻译四件套） |
| Q5 | 翻译域 | **v0.4 顺带接线，不新增功能** |
