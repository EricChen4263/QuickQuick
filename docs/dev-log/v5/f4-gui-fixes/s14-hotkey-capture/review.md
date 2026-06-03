---
id: V5-F4-S14-review
type: review
level: 小功能
parent: V5-F4
created: 2026-06-03T00:00:00Z
status: 通过
commit: 950cfdc
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 热键按键捕获改键（V5-F4-S14）

## 审查范围

- `src/main-window/settings/key-capture.ts`（新增）：纯函数 `keyEventToAccelerator` + `resolveMainKey` + `KeyEventLike` interface
- `src/main-window/settings/key-capture.test.ts`（新增）：32 条单元测试
- `src/panels/settings/HotkeyPanel.tsx`（改写）：录制状态机 + `KbdCombo` 组件 + `handleSave` 接线
- `src/panels/settings/settings-page.test.tsx`（改写+新增）：新增 5 条热键捕获集成测试

参照：项目规范、code-standards（code-general + frontend）。

---

## 问题清单

### Critical（高危，阻断放行）

无。

---

### Important（中优先级）

无达到报告门槛（置信度 ≥ 80）的问题。

---

### Low（低优先级）

无达到报告门槛（置信度 ≥ 80）的问题。

---

## 逐维度核查

### 1. 纯函数正确性

**结论：正确，5 项全部合规。**

**修饰键顺序**：`keyEventToAccelerator` 的推入顺序为 `metaKey(CmdOrCtrl) → ctrlKey(Ctrl) → altKey(Alt) → shiftKey(Shift)`，与规范"CmdOrCtrl→Ctrl→Alt→Shift"完全一致。测试 `"CmdOrCtrl+Ctrl+Alt+Shift+Z"` 四键全开时的全排列顺序已覆盖（key-capture.test.ts:49-54）。

**主键 e.code 映射**：
- `KeyA`..`KeyZ`：`/^Key([A-Z])$/` 正则 + `.slice(3)` → 大写字母，与 Tauri 加速键格式一致。
- `Digit0`..`Digit9`：`/^Digit(\d)$/` + `.slice(5)` → 数字字符，正确。
- `F1`..`F12`：`/^F([1-9]|1[0-2])$/` 完整覆盖 F1-F12，返回原串（如 `"F12"`），符合 Tauri 格式。
- 具名键（Space/Enter/Tab/Minus/Equal/Arrow*）：静态映射表，映射值符合 Tauri 命名（`Up`/`Down`/`Left`/`Right` 而非 `ArrowUp` 等）。

**null 条件完备**：三条路径均处理：① `MODIFIER_CODES.has(e.code)` 纯修饰键直接 null；② `resolveMainKey` 返回 null（不支持键如 Backspace/BracketLeft/Escape）；③ `modifiers.length === 0` 无修饰裸键。三路均有测试覆盖。

**MODIFIER_CODES 8 种码**：`ShiftLeft`/`ShiftRight`/`MetaLeft`/`MetaRight`/`ControlLeft`/`ControlRight`/`AltLeft`/`AltRight` 共 8 枚，L/R 均入集合，测试逐一验证（key-capture.test.ts:102-131）。

**Escape 不被当主键**：`handleCaptureKeyDown` 中 Esc 判断（`e.code === "Escape"` → `cancelRecording()`）先于 `keyEventToAccelerator` 调用（HotkeyPanel.tsx:68-72），不会落入主键转换路径。`resolveMainKey` 对 `"Escape"` 也返回 null（未在 named 表中），双重保险。

---

### 2. 录制状态机

**结论：流转正确，useEffect 竞态分析通过，setTimeout(0) 已知可接受。**

**状态流转**：

| 动作 | 前置状态 | 后置状态 |
|------|---------|---------|
| 点击「修改」(`enterRecording`) | 非录制 | `isRecording=true`, `captured=null`, 错误清零 |
| 捕获有效键 (`handleCaptureKeyDown`) | 录制中 | `captured=accelerator` |
| 捕获纯修饰/不支持键 | 录制中 | `captured` 不变（null 或保留上次） |
| Esc (`cancelRecording`) | 录制中 | `isRecording=false`, `captured=null`, 错误清零 |
| 点击「取消」(`cancelRecording`) | 录制中 | 同 Esc |
| 保存成功 (`handleSave`) | 录制中+captured≠null | `isRecording=false`, `captured=null`, 调 `onSaved` |
| 保存冲突 | 录制中 | `conflictError` 非 null，`isRecording` 保持录制中 |
| 保存后端失败 | 录制中 | `saveError` 非 null，`isRecording` 保持录制中 |

**`useEffect([currentValue])` 竞态分析**：`currentValue` 来自父组件 `hotkeys.history`/`hotkeys.translate`，只在 `onSaved` 触发 `fetchHotkeys` + 后端返回新值后才变更。保存成功路径：`handleSave` 在 `setHotkey` resolve 后先 `setIsRecording(false)` + `setCaptured(null)` 再调 `onSaved`，UI 已退出录制态；随后 `fetchHotkeys` 返回、`setHotkeys` 触发 `currentValue` 变更，`useEffect` 再次重置状态（此时 `isRecording` 已是 false，无副作用）。不会产生"保存中途 effect 覆盖中间态"的竞态。

**`handleSaved` 的 cancelled 守卫**：`handleSaved`（HotkeyPanel.tsx:193-196）每次调用都新建 `cancelled = { current: false }` 然后调 `fetchHotkeys(cancelled)`，但 cleanup 回调只在 `useEffect` 返回时绑定（首次 mount 的 `cancelled`），`handleSaved` 创建的 `cancelled` 对象**永远不会被置为 true**——存在理论上的卸载后 setState 风险。实践影响：仅在用户保存后立刻卸载组件的极端情况触发，当前版本设置页不会动态卸载 HotkeyPanel，影响可控。置信度 70（低于报告门槛），不作为必改项，但建议日后统一为 useRef+useCallback 守卫模式。

**`setTimeout(0)` focus**：将 focus 推迟一个事件循环，绕过 React 批量 state 更新后 DOM 还未渲染的时序问题，是前端常见的可接受写法。更稳妥方案（`useEffect` + `isRecording` 依赖）存在，但当前写法在主流浏览器和 Tauri WebView 下可靠，不作为必改项。

---

### 3. 接线复用

**结论：接线完整，未绕过任何校验。**

- `validateRebind(captured, occupiedValues)` 在 `handleSave`（HotkeyPanel.tsx:89）中正确调用，`!result.ok` 时写 `conflictError` 并提前 return，不调 `setHotkey`。
- `setHotkey(action, result.accelerator)` 接收的是 `validateRebind` 返回的 `result.accelerator`（类型收窄后的 string），不是原始 `captured`——设计正确，未绕过。
- 保存成功：`setSaveError(null)` → `setIsRecording(false)` → `setCaptured(null)` → `onSaved()`，序列合理。
- 冲突/后端失败均通过 `conflictError ?? saveError` 合并后在 `role="alert"` div 展示（HotkeyPanel.tsx:106,138-141）。

---

### 4. 可用性与健壮性

**结论：4 项全部合规。**

- `onKeyDown` 中 `e.preventDefault()` 在最顶层调用（HotkeyPanel.tsx:66），所有按键均阻止默认浏览器行为（含 Tab 切焦、Esc 关弹窗等），正确。
- `captured === null` 时「保存」按钮 `disabled`（HotkeyPanel.tsx:129），UI 层硬守卫；`handleSave` 第一行也有 `if (captured === null) return`（HotkeyPanel.tsx:87），双重防护。
- Esc 判断先于 `keyEventToAccelerator`（见第 1 节），不会把 Esc 误转为加速键。
- `err: unknown` + `instanceof Error` 类型收窄（HotkeyPanel.tsx:101-102），无裸 `any`。

---

### 5. 规范符合性

| 检查项 | 结论 |
|--------|------|
| 禁 `any` | 合规（`err: unknown`，`KeyEventLike` interface，无裸 any） |
| 函数 ≤ 50 行 | 合规（`handleSave` 18 行，`resolveMainKey` 20 行，`HotkeyRow` JSX 拆为两段各 ≤ 40 行） |
| 嵌套 ≤ 3 层 | 合规 |
| 无魔术值 | 合规（`MODIFIER_CODES` 常量集合，named 映射表，无散落字符串） |
| 注释写「为什么」 | 合规（模块头注释说明"纯函数无副作用"目的，JSDoc 说明修饰键顺序约定，组件注释说明职责） |
| 无残留旧 input | 合规（grep 无 `type="text"` 热键 input 遗留） |
| 测试 AAA 结构 | 合规（settings-page.test.tsx 所有热键用例均有 Arrange/Act/Assert 注释） |
| 测试行为化命名 | 合规（"捕获与另一动作相同的键显示已被占用且不调用 setHotkey" 等描述性名称） |
| 断言非弱 | 合规（`toHaveBeenCalledWith("history", "CmdOrCtrl+Shift+Y")` 精确参数断言，非仅 `toHaveBeenCalled`） |
| 无装饰性分隔注释 | 合规 |

---

## 总结论

**通过（放行）。**

本次改动三个部分目标明确，实现质量高：

1. **纯函数 `keyEventToAccelerator`**：修饰键顺序与 Tauri 加速键格式完全一致，主键 e.code 映射覆盖字母/数字/F1-F12/具名键，null 条件三路完备，MODIFIER_CODES 8 种码齐全。32 条单元测试逐一验证所有分支，测试质量达标。

2. **录制状态机**：进入/捕获/Esc 取消/保存成功/保存失败各路流转正确；Esc 优先判断不被误转；`useEffect([currentValue])` 保存后重置无竞态；冲突/错误均经 `role="alert"` 展示。`handleSaved` 的 `cancelled` 守卫存在理论漏洞但实践影响可控（置信度 70，低于门槛，不阻断）。

3. **接线复用**：保存路径完整走 `validateRebind` + `setHotkey`，未绕过；保存按钮双重禁用守卫；规范符合性全项通过。

无高危、无中优问题，可直接提交。
