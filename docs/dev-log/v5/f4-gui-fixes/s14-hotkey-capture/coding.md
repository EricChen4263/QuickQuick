---
id: s14-hotkey-capture
title: 热键改键改为按键捕获 UX
status: done
commit: 950cfdc
date: 2026-06-03
---

## 来由

设置页热键改键原本是**手打加速键字符串**（纯文本框输入 `CmdOrCtrl+Shift+V` 再点保存），反人类、需用户知道格式。改为**按键捕获**：选中某条热键 → 点「修改」进入录制 → 直接按下组合键即捕获显示 → 点保存持久化。后端 `hotkey.rs` / `setHotkey` IPC / `validateRebind` 均不改。

## 交互模型（用户确认：点「修改」进入录制）

每行：当前键的 kbd 芯片 + 「修改」按钮。
1. 点「修改」→ 录制模式：显示「录制中…请按下快捷键」，出现「保存」「取消」。
2. 录制中 `onKeyDown` 捕获组合键、`preventDefault()` 防误触发本应用；有效则以 kbd 芯片显示。
3. **Esc**（或「取消」）→ 还原当前值、退出录制。
4. 「保存」→ 走原有 `validateRebind`（与另一条热键冲突校验）+ `setHotkey`；系统占用由后端 `AlreadyInUse` 兜底。captured 为空时保存禁用。

## 改动 / 新增文件（纯前端）

### `src/main-window/settings/key-capture.ts`（新建·TDD 重点）
导出纯函数 `keyEventToAccelerator(e: KeyEventLike): string | null`：
- 修饰键固定顺序拼接：`metaKey→CmdOrCtrl`、`ctrlKey→Ctrl`、`altKey→Alt`、`shiftKey→Shift`。
- 主键由 `e.code` 归一：`KeyA-Z→A-Z`、`Digit0-9→0-9`、`F1-F12` 原样、`Space/Enter/Tab/Minus(-)/Equal(=)/Arrow*(Up/Down/Left/Right)`。
- null 条件三路：纯修饰码（MODIFIER_CODES 8 种 L/R）、无有效主键（不支持的 code）、无修饰键（`modifiers.length===0`）。
- 用 `e.code` 而非 `e.key` 归一，避开 Shift 改字符（Shift+1 在 key 是 "!"）的问题。

### `src/panels/settings/HotkeyPanel.tsx`（HotkeyRow 改造）
- 抽 `KbdCombo` 子组件（按 `+` 分段渲染 kbd）。
- HotkeyRow 录制状态机：`isRecording` / `captured`；`enterRecording`（focus 捕获按钮，setTimeout(0)）/ `cancelRecording` / `handleCaptureKeyDown`（preventDefault → Esc 优先拦截 → `keyEventToAccelerator` → 非 null 存 captured）/ `handleSave`（captured 守卫 + validateRebind + setHotkey + 退出录制）。
- 移除旧 `<input type="text">` 手打改键；保留 label/desc、kbd 当前值、错误 alert。

### `src/panels/settings/settings-page.test.tsx`（测试改写）
- 旧「手打 input 改键」测试改写为捕获交互（点「修改」→ `fireEvent.keyDown` 传 `{code, metaKey, shiftKey,…}` → 断言显示捕获值 → 点「保存」→ 断言 `setHotkey` 以捕获到的 accelerator 被调用）。
- 新增：Esc 取消还原、纯修饰键时保存禁用、冲突键显示「已被占用」且不调 setHotkey。

## TDD 红绿
- 纯函数：RED（模块不存在 import 失败）→ GREEN（32 测试通过）。
- 组件捕获交互：RED（6 个热键测试失败）→ GREEN（28 测试通过）。

## 实跑输出摘要
```
# 前端全量
Test Files  44 passed (44)
      Tests  400 passed (400)
# key-capture 专项 32 passed；settings-page 28 passed
# TypeScript
pnpm tsc --noEmit: No errors
```

## 已知项 / 后续
- **[低·非阻塞] handleSaved 的 cancelled 无 cleanup**（reviewer 置信度 70）：理论上保存后立即卸载组件有 setState warning 风险；HotkeyPanel 不动态卸载，影响可控。属改造前既有写法，建议日后统一为 useRef 守卫范式。
- **`.capture-area` CSS**：录制按钮带 `btn capture-area` 类，若 settings.css 无 `.capture-area` 专门样式则沿用 `.btn` 外观，功能不受影响，后续可加录制态视觉强调。
- 系统级占用的组合键（如 Cmd+W）部分可能在 webview 层被拦截、keydown 不必达——属按键捕获固有限制，preventDefault 已尽量兜底。
