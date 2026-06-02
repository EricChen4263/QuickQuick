---
id: s02-clip-translate-jump-test
title: 一键翻译跳转 动态证伪
status: 测试通过
commit: 52e10a4
date: 2026-06-02
---

# 动态证伪报告：一键翻译跳转

## 命中校验

全量运行 3 次（抗 flaky）：

| 次数 | 结果 |
|------|------|
| 第1次 | Test Files 40 passed, Tests 340 passed |
| 第2次 | Test Files 40 passed, Tests 340 passed |
| 第3次 | Test Files 40 passed, Tests 340 passed |

新增用例命中确认（verbose 模式）：

- `translate-page.test.tsx`（18 tests）：
  - ✓ translate-page: seed prop 传入文本后自动填入输入框并调用 translateText
  - ✓ translate-page: seed nonce 自增时相同文本再次触发 translateText
  - ✓ translate-page: seed 为 null 时不调用 translateText
  - ✓ translate-page: seed.text 为空字符串时不调用 translateText
- `clipboard-page.test.tsx`（17 tests）：
  - ✓ clipboard-page: onTranslateItem prop 在点击一键翻译后以条目 content 被调用

共 5 个新增用例全部命中并通过，无假绿。

## 变异 Sanity

### 变异 A — 注释掉 `void handleTranslate(current.text)`

- 目标文件：`src/panels/translate/TranslatePage.tsx` 第 110 行
- 操作：sed 注释 `void handleTranslate(current.text);`
- 结果：**如期变红**
  - FAIL `seed prop 传入文本后自动填入输入框并调用 translateText`
  - FAIL `seed nonce 自增时相同文本再次触发 translateText`
- 还原：从备份复原，`git status` 干净

### 变异 B — useEffect 依赖 `[seed?.nonce]` 改为 `[]`

- 目标文件：`src/panels/translate/TranslatePage.tsx` 第 113 行
- 操作：`}, [seed?.nonce]);` → `}, [];`
- 结果：**如期变红**
  - FAIL `seed nonce 自增时相同文本再次触发 translateText`（第一次 seed 仍通过，重复触发失败）
- 还原：从备份复原，`git status` 干净

### 变异 C — ClipboardPage 传 `""` 而非 `item.content`

- 目标文件：`src/panels/clipboard/ClipboardPage.tsx` 第 245 行
- 操作：`onTranslateItem?.(item.content)` → `onTranslateItem?.("")`
- 结果：**如期变红**
  - FAIL `onTranslateItem prop 在点击一键翻译后以条目 content 被调用`
- 还原：从备份复原，`git status` 干净
- 注：首次变异因 JSX 单行注释语法错误导致编译失败，已改为无注释直接替换参数后重做，结论有效

### 变异 D — `handleTranslate` 守卫恒用 `inputText`（忽略 textOverride）

- 目标文件：`src/panels/translate/TranslatePage.tsx` 第 85 行
- 操作：`const text = typeof textOverride === "string" ? textOverride : inputText;` → `const text = inputText;`
- 结果：**如期变红**
  - FAIL `seed prop 传入文本后自动填入输入框并调用 translateText`
  - FAIL `seed nonce 自增时相同文本再次触发 translateText`
  - 原因：useEffect 调用 `handleTranslate(current.text)` 时，textOverride 被忽略，改用空 inputText state，`text.trim().length === 0` 提前返回，翻译未触发
- 还原：从备份复原，`git status` 干净

## 边界探测

### 边界 1：seed nonce 不变但组件 rerender（父组件其它 state 变）是否误重译

分析：useEffect 依赖数组为 `[seed?.nonce]`，仅当 nonce 值变化时才触发。父组件其它 state 导致 rerender 时，nonce 值不变，useEffect 不执行，不会误重译。

已有用例「seed 为 null 时不调用 translateText」和「seed.text 为空字符串时不调用 translateText」覆盖 seed 守卫逻辑，边界安全。

### 边界 2：seed.text 相同、nonce 自增 → 应重译

已有专用测试用例覆盖：「seed nonce 自增时相同文本再次触发 translateText」验证 `mockTranslateText.toHaveBeenCalledTimes(2)`，行为一致。

## git 干净证明

变异 A-D 全部还原后：

```
git status --porcelain → (空输出，工作树干净)
全量测试：Test Files 40 passed, Tests 340 passed
```

开工快照与结尾快照逐行一致（均为空，工作树无未提交改动）。
