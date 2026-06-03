---
id: s14-hotkey-capture
title: 热键按键捕获改键 测试留痕
status: passed
commit: pending
date: 2026-06-03
---

# 测试留痕：热键按键捕获（s14）· 动态证伪

## 开工 git status 快照

```
 M src/panels/settings/HotkeyPanel.tsx
 M src/panels/settings/settings-page.test.tsx
?? src/main-window/settings/key-capture.ts
?? src/main-window/settings/key-capture.test.ts
```

---

## 一、命中校验（杀假绿）

连跑 2 次全量（`pnpm test --run`），两次均 `Test Files 44 passed (44)` / `Tests 400 passed (400)`，无 flaky。

- `key-capture.test.ts` — 32 tests 全 ok
- `settings-page.test.tsx` — 28 tests 全 ok

---

## 二、变异 sanity（杀恒真/旁路）

### 变异A — 修饰键零守卫
`if (modifiers.length === 0)` 改 `if (false)`。「无修饰裸键（字母/数字）→ null」2 个测试如期变红（`expected 'A' to be null`）。还原复绿。

### 变异B — 字母键映射破坏
`code.slice(3)` 改 `code.slice(4)`。10 个含字母键测试如期变红。还原复绿。

### 变异C — 组件捕获→保存接线
`setHotkey(action, result.accelerator)` 改 `setHotkey(action, currentValue)`。「捕获不冲突键后调用 setHotkey 正确参数」测试如期变红（`Received ["history","CmdOrCtrl+Shift+H"]` vs 期望 `"CmdOrCtrl+Shift+Y"`）。证明组件测试真校验「捕获值→落库」，非旁路。还原复绿。

三变异均还原干净，业务代码与开工快照逐字节一致。

---

## 三、真值边界探测

1. **纯修饰键 keydown**：`MODIFIER_CODES` 含全部 8 种 L/R 修饰码；测试用 `{code:"ShiftLeft",shiftKey:true}` 触发并断言保存按钮 disabled。✓
2. **Esc 取消逻辑顺序**：`handleCaptureKeyDown` 先 `preventDefault` 再 `if(code==="Escape"){cancelRecording();return;}`，拦截在 `keyEventToAccelerator` 之前；且 Escape 不在主键映射表（双保险）。✓
3. **无修饰键不可保存**：`handleSave` 前置 `if(captured===null)return;` + UI `disabled={captured===null}`，setHotkey 不被调用，有测试覆盖。✓
4. **preventDefault**：onKeyDown 首行执行，所有键均防默认行为误触发。✓
5. **冲突校验仍在**：保存前 `validateRebind(captured, occupiedValues)`，冲突→「已被占用」且不调 setHotkey，有测试覆盖，未被新交互绕过。✓

无发现真实缺陷。

---

## 四、最终门禁结论

**PASS（放行）**

- 命中校验 400/400，连跑无 flaky
- 3 变异全红再复绿，测试有判别力、非恒真/旁路
- 边界全覆盖，无真实缺陷
- 工作树无残留业务代码改动

## 结束 git status --short（业务代码部分）

```
 M src/panels/settings/HotkeyPanel.tsx
 M src/panels/settings/settings-page.test.tsx
?? src/main-window/settings/key-capture.ts
?? src/main-window/settings/key-capture.test.ts
```

与开工快照一致，无残留。
