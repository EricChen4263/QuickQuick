# QuickQuick 设计系统

本文件是界面的规范基准。实现层唯一真源是 `src/theme/tokens.css`、`base.css`、`components.css`；本文件解释**为什么**是这些值，以及哪些地方是刻意豁免的。

规范依据：[`elayadesign/ai-design-skills`](https://github.com/elayadesign/ai-design-skills) 的 `redesign-existing-projects`（诊断与替换规则）+ `landing-page-design` Part B（视觉系统硬约束）。Part A（落地页结构 / CTA / SEO）不适用桌面工具类应用。

---

## 一、改造前的问题（诊断基线）

| # | 问题 | 证据 |
|---|---|---|
| 1 | 字体是系统默认栈 | `--font: -apple-system, SF Pro Text, PingFang SC, Segoe UI…` |
| 2 | 字号有 15 个任意值 | 10 / 10.5 / 11 / 11.5 / 12 / 12.5 / 13 / 13.5 / 14 / 15 / 16 / 18 / 19 / 21px |
| 3 | 圆角有 9 种任意值 | 3/4/5/6/8/10/12/14/15px |
| 4 | 间距大量脱轨 | 18/11/13/9/7/3/5px，且 `padding: 18px 18px 16px 0` 上下不等 |
| 5 | 动效全是默认缓动且过短 | 全部 `ease` + 90/110/120/140ms |
| 6 | 112px 常驻侧栏吃掉横向宽度 | skill 点名「Dashboard 永远有左栏」是最泛的 AI 布局 |
| 7 | 暗色底不在批准色板 | `oklch(17.5%)` / `oklch(21.5%)` |
| 8 | 五处「边框 + 圆角 + 白底」卡片并排 | `.clip-list-col` / `.clip-preview` / `.tx-work` / `.tx-history` / `.set-body` |
| 9 | 背景用了渐变 | `.clip-thumb` 的 `linear-gradient` |
| 10 | 图标混用 Lucide 语汇 + Segoe MDL2 字体 | 描边 1.6 / 1.7 混杂 |
| 11 | 胶囊滥用 | 徽章 / 计数 / 芯片 / 进度条全部 999px |
| 12 | 主题切换是太阳 / 月亮三档图标 | skill 点名反模式 |
| 13 | 死代码 | `src/theme/design-tokens.ts`（`#3A7CA5` 品牌色契约）除自身测试外零引用，已删除 |

---

## 二、落地的设计决策

### 1. 字体

`Geist` + `Geist Mono`（本地 woff2，`src/assets/fonts/`）。禁用 Inter / Roboto / Arial / Open Sans / Helvetica；无斜体；字重上限 700。

Geist 无 CJK 字形，因此按 `unicode-range` 只让它接管拉丁与通用标点，中文回落苹方 / 微软雅黑。这是「一种主字体」在中文产品上的正确落地，不是二字体搭配。

### 2. 字号

只允许 12 / 14 / 16 / 18 / 20 / 24 / 30（Tailwind 默认阶梯）。非阶梯值一律**向下吸附**并沿用该档行高。文字必须使用 `base.css` 的命名样式类：

`heading-5/6` · `title-1..5` · `body-1..6` · `button-lg/md/sm/xsm` · `caption-1..5`

### 3. 间距

只允许 0 / 2 / 4 / 8 / 12 / 16 / 24 / 32 / 40 / 48 / 64 / 80 / 96（`--sp-*`）。主按钮内边距固定纵向 8 · 横向 12。

### 4. 圆角

| token | 值 | 用途 |
|---|---|---|
| `--r-4` | 4px | 徽章、键帽、小标签、指示竖条、芯片 |
| `--r-6` | 6px | 小按钮、图标按钮 |
| `--r-8` | 8px | 按钮、输入框、列表行、分段控件 |
| `--r-12` | 12px | 面板、空态图标容器 |
| `--r-16` | 16px | 浮窗 |
| `--r-full` | 999px | **只给开关滑块**（全站唯一一处） |

**真圆形不另开 token。** 4px 圆点用 `--r-4`、16px 单选圆用 `--r-8`、滚动条滑块用 `--r-4`——圆角大于元素半径时浏览器会自行钳成圆，渲染与 `50%` 完全一致。曾短暂加过 `--r-circle: 50%`，被审查指出「给 50% 换个名字等于绕过规则」，已删除：能从既有阶梯推导出来的形状，就不该新增 token。

嵌套按 `inner = outer − gap`（外 12 + 内边距 4 → 内层 8）。

### 5. 色板

- **浅色**：冷中性灰单一家族（不混暖灰）。**深色**：只用 `#181818` / `#1F1F1F` / `#272727` / `#313131`。
- **强调色只有一个**：`#2F6E8F`（浅）/ `#7FB4CE`（深），饱和度 < 80%。只用于主按钮、选中指示、焦点环、当前导航。
- **辅助色已按 WCAG AA 校正**（浅色模式原来不达标）：

| token | 原值 | 对 `--bg` | 对 `--surface-2` | 现值 | 对 `--bg` | 对 `--surface-2` |
|---|---|---|---|---|---|---|
| `--muted` | `#6e7479` | 4.34 ✗ | 4.07 ✗ | `#62686d` | 5.17 ✓ | 4.85 ✓ |
| `--ok`（14% 染色底） | `#3f7f5a` | — | 4.00 ✗ | `#2f6a48` | — | 5.23 ✓ |
| `--warn`（16% 染色底） | `#9a6b1f` | — | 3.81 ✗ | `#7d5314` | — | 5.29 ✓ |

- 字重只允许 `--fw-regular`(400) / `--fw-medium`(500) / `--fw-semibold`(600) / `--fw-bold`(700)。`--fw-bold` 只用于「搜索命中的加粗档」这类必须与正文强区分的场合，skill 的字重上限就是 bold。
- 禁止背景渐变。阴影向背景色相偏移（`--shadow-pop` / `--shadow-panel` / `--shadow-knob`），不用纯黑平铺。
- 一层 2.8% 细颗粒（`feTurbulence`）解决「纯平表面没质感」，而不是用渐变糊过去。

### 6. 层次策略（最核心的一条）

**一个页面只让一个容器抬升。**

| 页面 | 抬升的容器 | 其余部分 |
|---|---|---|
| 剪贴板 | 预览区（承载「阅读」） | 列表栏透明，行只在 hover / 选中时浮起 + 2px accent 竖条 |
| 翻译 | 工作区 | 历史栏透明，选中行才抬到 `--surface` |
| 设置 | 无（全平铺） | 分组靠留白 + 1px 分隔线 |
| 浮窗 | 窗口本身（毛玻璃，见豁免） | 内部列表选中用 `accent-soft` + 2px 竖条 |

改造前是五处同样观感的卡片并排，等于没有层次。

### 7. 动效

曲线统一 `cubic-bezier(0.22, 1, 0.36, 1)`（`--ease`）。位移 500ms，颜色 / 透明度 300ms，**任何过渡不短于 200ms**。只动 `transform` 与 `opacity`：选中指示条用 `scaleY` 长出来，开关滑块用 `translateX`。`prefers-reduced-motion` 下把位移换成纯透明度变化。

### 8. 结构

一级导航从 112px 常驻侧栏并入 48px 顶部栏。剪贴板与翻译都是「列表 + 预览」双栏，横向空间是这类工具最稀缺的资源；原侧栏把它长期占掉了。

主题入口从顶部栏移到「设置 › 外观」——主题是设一次就不再动的偏好，且太阳 / 月亮图标开关是被点名的泛化模式。当前实现为纯文字三档分段。

### 9. 图标

手写内联 SVG，统一 `stroke-width: 1.5`、24 网格、圆头圆角，描边语汇向 Phosphor 靠。替换掉原来 1.6 / 1.7 混杂的 Lucide 风格描边，以及 Windows 标题栏的 Segoe MDL2 图标字体（改内联 SVG）。

设置入口用「三滑杆」而非齿轮：齿轮在这个体量下必然滑向 Feather / Lucide 的同一条路径骨架，滑杆在 16px 下辨识度更高，也不与主题的太阳图标混淆。

### 10. 状态

- **加载**：骨架屏，形状照抄真实行（图标位 + 两行文字，长度错开），不转圈；带 `aria-busy` / `role="status"` 播报。
- **空态**：有内容的「下一步」引导视图，不是空白面板。
- **错误**：具体到「什么失败 + 下一步做什么」，例如「无法写入 hotkeys.json，已回退到备份 `/Users/…/hotkeys.json.bak`。本次键位修改未生效。」
- **焦点**：`:focus-visible` 2px 强调色；列表行内联操作在 `:focus-within` 时同样浮出，否则键盘用户看不见自己聚焦了什么。
- **按下**：`translateY(1px)`。

---

## 三、豁免清单（明确在规范之外，不是遗漏）

| 项 | 现状 | 理由 |
|---|---|---|
| 浮窗同时有玻璃底 + 1px 边框 + 阴影 | `.popover-shell` 的窗口外框 | 浮窗是 OS 级窗口，边框与阴影是窗口与桌面之间的边界，不是「卡片观感」。真正的层级问题在窗口**内部**，那里没有卡片 |
| `font-style: italic` | 设置 › 外观 › 命中片段样式的色板样本 | 它是产品真实存在的功能选项（搜索命中斜体档）的预览，不是排版上的斜体滥用 |
| 动效 1ms | `base.css` 的 `prefers-reduced-motion` 块 | 减速动效的标准写法：把时长压到近零来关闭动画 |
| `.popover-row-thumb` 的 `40px` fallback | `src/clip-popover/popover.css` | `popover-style.test.ts` 锁定的契约：fallback 必须等于 `DEFAULT_IMAGE_HEIGHT_PX` |
| 结构性 px（1px 描边 / 2px 竖条 / `translateY(1px)` / 列宽 `minmax`） | 各页 CSS | 不属于字号 / 行高 / 间距 / 圆角 / 缓动 / 颜色六类 |
| 品牌词双色上色（`.brand-accent` 只给「Quick」上 accent） | 顶部栏、设置 › 关于 | 既有品牌资产，不是装饰性排版。全站仅此一处把强调色用在文字上 |
| 弹层同时有边框 + 阴影 + 底色 | `Select.css` 的下拉浮层、`UpdateBanner.css` | 它们浮在页面之上、需要与页面分离，边框与阴影是「脱离文档流」的语义，不是卡片式的层级表达 |
| 引导卡 `.onboard` 有全周边框 + accent 淡底 | 剪贴板首次引导 | 它是「提示」不是抬升面板（无阴影），全周边框用来把提示与列表分开；同页唯一的抬升面仍是预览区 |

---

## 四、实现中与规范冲突时的取舍（已决策）

| 冲突 | 决策 | 理由 |
|---|---|---|
| 类型筛选用分段控件 vs 用 `Select` | **保留 `Select`** | `clipboard-page.test` 用「点 `aria-label=类型筛选` → 点 `role=option`」驱动筛选，换成分段控件会打断行为断言。已把 `Select` 外观按新系统重做 |
| 主题开关放顶栏 vs 放设置 | **放设置 › 外观** | 与 skill 一致；顶栏只留三个一级入口 |
| 浮窗译文用 18/28 还是 16/24 | **16/24（`body-2`）** | 真机窗口 320×200，正文可视高 93px。18/28 只放得下 3.32 行，第 4 行必然露出半个字；16/24 是 3.875 行，第 4 行完整、第 5 行完全在视口外 |
| 浮窗数字直达提示写 `1–9` 还是 `1–0` | **`1–0`** | 浮窗走 `directPickIndex`（`panels/history/keyboard.ts`），其中 `0` 明确映射到第 10 条。注意别和主窗的 `quickSelectIndex` 混——那条路径只接受 ⌘1–9 |
| 结果区角标要不要显示耗时 | **显示**，由 `TranslatePage` 通过可选 prop `elapsedMs` 透传 | 在 workspace 内部按 `isLoading` 计时会被 `MIN_LOADING_VISIBLE_MS`（400ms 地板）污染成假数字 |
| 「存为片段」按钮 | **不做** | `availableActions()` 里的 `save_history` 语义是「后端在 translate_text 时已自动写历史」，没有对应 IPC，做成按钮就是死按钮 |

---

## 五、被改动的测试（逐条说明，都是「断言了旧结构 / 旧文案」）

| 文件 | 改动 | 原因 |
|---|---|---|
| `src/theme/design-tokens.test.ts` | **删除**（连同 `design-tokens.ts`） | 断言 `#3A7CA5` / `10px` 的旧里程碑契约，且模块本身零引用 |
| `src/app-shell.test.tsx` | `qq-sidebar` → `qq-topnav`；「主题切换区渲染三个 aria-pressed 按钮」→「主导航只容纳三个一级入口」 | 侧栏删除、导航并入顶栏、主题开关移入设置 |
| `src/panels/settings/settings-style.test.ts` | 子项导航宽度断言 `128px` → `160px` | 子导航按新间距阶梯加宽 |
| `src/clip-popover/footer-hints.ts` | 加注释说明 `1–0` 的语义 | 防止后人把它误改成 `1–9`（曾误改并已回滚，测试断言未变） |
| `src/clip-popover/popover-appearance.test.tsx` | `rowsByGroup` 改为按 `role="group"` 读组名与行 | 浮窗分组从「裸 div 标题 + 相邻行」改成 `role="group"` 包裹（listbox 直接子项只能是 option / group）。断言语义「哪一行落在哪一组」完全没变 |

除此之外**没有放宽任何断言**；其余 900+ 条测试原样通过。

---

## 六、成品截图

`docs/design/screenshots/` 下的图全部来自真实渲染（Vite dev server + 注入 Tauri IPC 桩，窗口尺寸取 `tauri.conf.json` 的默认 960×640，浮窗取 Rust 侧常量 720×480 / 320×200）：

| 文件 | 内容 |
|---|---|
| `clipboard-light.png` / `clipboard-dark.png` | 剪贴板页深浅两色：列表扁平、预览是唯一抬升面 |
| `translate-light.png` | 翻译页：工作区抬升、历史栏透明 |
| `settings-light.png` | 设置页：零卡片、160px 竖条子导航 |
| `popover-light.png` | 剪贴板热键浮窗（720×480） |
| `trans-popover-light.png` | 翻译热键浮窗（320×200，正文按真实可视高定档） |

## 七、验收命令

```bash
npx tsc -b                 # 类型
npx vitest run             # 全量测试（83 文件 / 926 用例）
npx vite build             # 生产构建
```

样式合规自检（应全部无输出）：

```bash
grep -rnE "font-size:[ ]*[0-9]" src --include=*.css
grep -rnE "(^|[ ,:])(ease|linear|ease-in-out)([ ;,)]|$)" src --include=*.css
grep -rnE "gradient" src --include=*.css
grep -rnE "(999px|border-radius:[ ]*(1|3|5|7|9|10|11|13|14|15|20)px)" src --include=*.css
grep -rnE "Segoe MDL2|Fluent Icons" src
```
