# impl.md — f6/s11 自定义 Select 下拉组件替换全部原生 select + 移除译文区冗余 UI

**日期**：2026-06-04
**类型**：前端组件改造 + 死代码清理

---

## 1. 背景（为什么）

主窗口 `titleBarStyle: "Overlay"`（macOS 覆盖式标题栏）下，原生 `<select>` 的弹出菜单在
WKWebView 里坐标系错位、CSS 无法纠正。解法：自定义下拉组件（JS 自定位 + portal 到
`document.body` 的 `position: fixed` 浮层）替换全部原生 select，绕开原生弹窗与
overflow 容器（`.tx-scroll`）的裁剪。同时移除译文区两块冗余 UI。

## 2. 任务 A：自定义 Select 组件 + 替换 5 处原生 select

### 新增组件
- `src/components/Select.tsx`：类型化 props（`value` / `onChange` / `options` / `ariaLabel` / `className`）。
  - 浮层 `createPortal` 到 `document.body`，`position: fixed`，按 trigger 的
    `getBoundingClientRect()` 计算坐标；下方空间不足时向上翻（`openUpward`）。
  - 行为：点击/Enter 展开，点选项或 Enter 选中，点外部 / Esc / blur 关闭；↑↓ 移动高亮
    并跳过禁用项；禁用项不可选、视觉置灰。窗口 resize/scroll 时关闭避免错位。
  - 无障碍：trigger `role="button"` + `aria-haspopup="listbox"` + `aria-expanded`；
    菜单 `role="listbox"`、选项 `role="option"` + `aria-selected` + `aria-disabled`。
- `src/components/Select.css`：复用设计 token（`--surface` / `--border` / `--accent` /
  `--muted` / `--r-sm` / `--shadow-pop` / `--hover`），保持与原 select 一致外观（含右侧 chevron）。

### 替换的 5 处
1. `DirBar.tsx` 源语（SOURCE_LANGUAGES）
2. `DirBar.tsx` 目标语（TARGET_LANGUAGES）
3. `DirBar.tsx` 翻译源（providers，**禁用逻辑 `p.needsKey && !configuredIds.has(p.id)` 保留**，
   映射到 `SelectOption.disabled`）
4. `ClipSearchBar.tsx` 类型筛选
5. `StoragePanel.tsx` 单张图片阈值（原 `<label htmlFor>` 改为 Select 的 `ariaLabel`）

各处原生 `<select>` + chevron svg 已删除，统一由 Select 内部渲染 chevron。

## 3. 任务 B：移除译文区两块冗余 UI

- `translate-actions.ts`：`TranslateAction` 去掉 `switch_target` / `switch_source_retranslate`，
  `availableActions()` 改为 `["copy", "speak", "save_history"]`。
- `TranslateWorkspace.tsx`：`ACTION_LABELS` 删掉这两个 label；删除整块 `dict-slot` div。
- `TranslatePage.tsx`：`handleAction` 删除已不可达的 `switch_target || switch_source_retranslate`
  分支（`resolveTranslateAction` 对它们已返回 null），同步更新方法 docstring。
- `translate.css`：删除 `.dict-slot` / `.dict-slot svg` 规则（无他处引用）。

grep 确认无 trans-popover 等别处依赖这两个 action。

## 4. TDD（红→绿）

- 新增 `Select.test.tsx`：展开/收起、选中触发 onChange、点外部关闭、Esc 关闭、键盘 ↑↓ Enter、
  跳过禁用项、禁用项不可选、aria-selected 标记。**测行为不测像素坐标**（jsdom 的
  getBoundingClientRect 返回 0）。
  - RED：`Failed to resolve import "./Select"`（组件未实现）。
  - GREEN：10 passed。
- 更新受影响测试：`DirBar.test.tsx`（combobox→button trigger + 展开点选项；
  option.disabled→aria-disabled；新增"点禁用项不触发 onChange"）、`translate-page.test.tsx`
  （源/目标语切换改点击展开+点选项；provider disabled 断言改展开下拉 + aria-disabled；
  `name: /翻译/` 改 `name: "翻译"` 避免误匹配"翻译源" trigger）、`clipboard-page.test.tsx`
  （筛选改展开+点选项）、`StoragePanel.test.tsx`（combobox/toHaveValue→button trigger
  textContent + 展开点选项）、`translate-actions.test.ts`（断言移除的两 action 返回 null、
  availableActions 仅 3 项）。

## 5. 验证结果

`export PATH="$HOME/.cargo/bin:$PATH" && make verify` 五步全绿（分步重跑取证）：

| 步骤 | 命令 | 结果 |
|---|---|---|
| 1/5 tsc | `pnpm exec tsc -b` | EXIT=0，0 行输出（无类型错） |
| 2/5 fmt | `cargo fmt --all --check` | EXIT=0，0 行 diff |
| 3/5 clippy | `cargo clippy --all-targets -- -D warnings` | EXIT=0，`Finished`，无 warning/error |
| 4/5 vitest | `pnpm test` | EXIT=0，**Test Files 49 passed / Tests 446 passed** |
| 5/5 cargo test | `cargo test` | EXIT=0，全部 `test result: ok`（151 / 67 / 32 passed 等，0 failed） |

原始日志：`artifacts/vitest.log`、`artifacts/make-verify.log`（make 整体 EXIT=0）。

## 5.1 Phase 6 review 打回修复（二轮）

tester + reviewer 一致打回三项，已修（仍 TDD 红→绿）：

- **[Critical] scroll capture 致菜单内部滚动自关闭**：原 `scroll` capture 监听被菜单
  `<ul>` 自身滚动触发，选项超过 max-height(260px) 时滚菜单即关。修法：`closeOnExternalScroll`
  判断 `event.target` 是否在 listbox 内，**菜单内部滚动忽略、仅外部滚动才关**。
  补测试：内部滚动（target 在 listbox 内）菜单不关、外部滚动（target=document）菜单关。
- **[Important] data-open 死 CSS**：JSX 从未设 `data-open`，CSS `[data-open="true"]` 规则永不命中。
  修法：root div 加 `data-open={isOpen}`。补测试断言展开/收起时 `data-open` 切换。
- **[Important] Select 函数 171 行超规范**：拆为 `useSelectState`（状态/定位/关闭副作用编排）、
  `useMenuRect`、`useCloseOnOutside`、`useCloseOnScroll`、`useSelectInteractions`（键盘/失焦/提交）
  + 子组件 `SelectTrigger` / `SelectMenu` / `SelectOptionItem` / `SelectChevron`。
  拆后所有函数 ≤50 行、嵌套 ≤3 层。
- **[可选补测]** 空 options、value 不在 options 内、全禁用项 Enter 不触发 onChange——均已补测通过。

修复后全量 vitest **452 passed**（446 + 新增 6），make verify 五步仍全绿。
原始日志：`artifacts/vitest-review-fix.log`、`artifacts/cargo-test-review-fix.log`。

## 6. 偏离方案处

- dev-log 文件名沿用项目既有约定 `impl.md`（非任务文案里的 `coding.md`）。
- 无法在 jsdom 测到、留给 GUI 实测的点：①浮层向上翻转（openUpward，依赖真实
  innerHeight/getBoundingClientRect）②portal fixed 定位坐标正确性 ③Overlay 标题栏下
  弹窗不再错位（本任务的根本目的）。jsdom 下 getBoundingClientRect 恒返回 0，只能测行为不测坐标。
