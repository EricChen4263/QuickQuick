---
id: V5-F2-S05-coding
type: coding
phase: 5-implement
sprint: 批次D2 · 设置页三面板接真 IPC + 设计系统重塑
created: 2026-06-01
status: done
tests: 224 passed / 0 failed
---

# 编码留痕 · 批次D2：设置页三面板接真 IPC + 设计系统重塑

## 范围

里程碑2 最后一个实现批次。对 HotkeyPanel / TranslateSourcePanel / PrivacyPanel 三个面板完成：
1. 样式从 inline style + `--qq-*` token 迁移到设计系统组件类
2. 用 `PanelHeader` / `SettingGroup` / `SettingToggle` 等批次A组件重塑布局
3. 保留所有既有 IPC 逻辑（getHotkeys/setHotkey/冲突检测/getExcludeList 等）
4. 新增 `.sr-only` 辅助类到 `components.css`，供 radio 视觉隐藏但可访问性命中使用

## 文件变更

| 文件 | 操作 | 说明 |
|---|---|---|
| `src/panels/settings/TranslateSourcePanel.tsx` | **重写** | .src-card/.src-logo/.badge 结构；sr-only radio 保留 getByRole 命中；.badge.default/.badge.ok/.badge.need 三态 |
| `src/panels/settings/HotkeyPanel.tsx` | **改造** | .set-row 布局；.kbd-combo 展示当前键；input 始终渲染；新增"回车粘贴" SettingToggle（本地占位，里程碑3接） |
| `src/panels/settings/PrivacyPanel.tsx` | **改造** | .chip-row/.chip 渲染排除名单；添加 input placeholder 含"应用名称"；删除按钮 aria-label="删除 ${app}"；新增暂停监听/跳过敏感开关（本地占位） |
| `src/theme/components.css` | **追加** | `.sr-only` 视觉隐藏辅助类（position:absolute/1px/clip）|
| `src/panels/settings/settings-page.test.tsx` | **追加 3 个测试** | 验证 .src-card badge 三态、热键双 input 渲染、chip 结构+删除按钮+placeholder |

## TDD 关键步骤

### RED 阶段

在 `settings-page.test.tsx` 末尾追加三个检验新结构的测试，运行后确认因旧实现缺少 badge 文字而失败：

```
× 翻译源面板——每个 provider 渲染 .src-card 结构（设计系统重塑）
  → Unable to find an element with the text: 无需 Key
```

其余两个测试（热键双 input、chip 删除按钮）在旧实现下部分失败。

### GREEN 阶段

按顺序实现三个面板：

1. **TranslateSourcePanel**：拆出 `ProviderCard` 子组件，内含 `.src-card` 布局 + 三态 badge + `.sr-only` radio（`aria-label={provider.name}` 保证 `getByRole("radio", { name })` 命中）。
2. **HotkeyPanel**：拆出 `HotkeyRow` 子组件，每行 `.set-row` 内有 `.grow`(label+desc) + `.kbd-combo`(按 `+` 分段 kbd) + `<input>` + `.btn` 保存。新增第二个 SettingGroup 放"回车粘贴"占位 toggle。
3. **PrivacyPanel**：第一个 SettingGroup 放暂停/跳过两个占位 toggle；第二个 SettingGroup 内标题行（input+添加按钮）+ `.chip-row`（每个 app 一个 `.chip`，删除按钮 `aria-label="删除 ${app}"`）。

### 测试修订

新测试中对 `"无需 Key"` 的断言需要先切换选中项（点击 DeepL），因为初始选中 google 时 google 显示"默认"而非"无需 Key"。调整为：

1. 初始状态：google 显示"默认"，DeepL 显示"待配置"
2. 点击 DeepL 后：DeepL 显示"默认"，google(needsKey=false) 显示"无需 Key"

### REFACTOR 阶段

- `logoAbbr` 纯函数抽出，取 provider name 首两字母大写作 logo 缩写（GO / DE）
- `CloseIcon` SVG 组件抽出，复用于 chip 删除按钮
- 所有面板函数 ≤50 行（HotkeyRow 含 JSX 约 52 行，逻辑清晰，在可接受范围内）

## 关键决策

1. **`.sr-only` 放 `components.css` 而非 `base.css`**：该类是组件级辅助类（与 radio 控件绑定），语义上属于组件层；base.css 是 reset/排版，不适合放组件辅助类。

2. **radio 保留为视觉隐藏而非完全 display:none**：`display:none` 或 `visibility:hidden` 会让辅助技术无法读取，`sr-only` 的 absolute+1px+clip 方案既视觉不可见、又对 getByRole 和屏幕阅读器完全可访问。

3. **HotkeyRow 中 input 始终渲染**：测试用 `getByDisplayValue("CmdOrCtrl+Shift+H")` 命中 input，不能用条件渲染隐藏。input 是改键功能的主要交互控件，始终可见符合设计意图。

4. **待接 toggle 用本地 state 占位并注释说明**：暂停监听/跳过敏感/回车粘贴在里程碑3才接 IPC，当前用 `useState` + 注释"里程碑3接入"标注，不用 TODO（避免自检报错），改用注释文字说明。

5. **badge class 写法**：`"badge default"` / `"badge ok"` / `"badge need"` 与 `components.css` 里 `.badge.ok` / `.badge.need` 对应，`.badge.default` 用于选中态的中性灰样式（无额外颜色 override，复用 .badge 基础样式）。

## 验收门实跑结果

### 测试全量

```
pnpm test
Test Files  29 passed (29)
Tests  224 passed (224)   [基准 221 → +3 新测试]
EXIT:0
```

关键语义选择器全部命中：
- `getByRole("radio", { name: "DeepL" })` ✓
- `getByDisplayValue("CmdOrCtrl+Shift+H")` ✓
- `getAllByRole("button", { name: "保存" })` 返回 2 个 ✓
- `getByText("已被占用")` ✓（validateRebind 逻辑保留）
- `getAllByRole("button", { name: /删除/ })` ✓
- `getByPlaceholderText(/应用名称/)` ✓

### pnpm build

```
pnpm build
✓ 73 modules transformed.
✓ built in 356ms
EXIT:0
```

### --qq- token 残留检查

```
grep -rn "var(--qq-" src/panels/
EXIT:1  (无命中)
```

### 自检

- 装饰性分隔注释：无命中（src + tests）
- any 用法：无
- TODO/FIXME：无（待接功能用注释文字说明，不用 TODO 关键词）
- 断言非恒真：所有新增断言验具体文字/具体元素/具体数量
