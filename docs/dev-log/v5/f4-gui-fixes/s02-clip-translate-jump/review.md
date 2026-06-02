---
id: f4-gui-fixes-s02-review
title: 一键翻译跳转 规范审查
type: review
level: small-feature
parent: f4-gui-fixes
created: 2026-06-02T00:00:00Z
status: 审查通过
commit: 0d5ab46
acceptance_ids:
  - f4-gui-fixes-s02
author: code-reviewer
---

# 审查结论：审查通过（无未决高危）

审查范围：`git diff 0d5ab46~1 0d5ab46` 涉及的三个文件：
- `src/App.tsx`
- `src/panels/clipboard/ClipboardPage.tsx`
- `src/panels/translate/TranslatePage.tsx`

---

## 高危问题

无。

---

## 中等问题

无。

---

## 低优先级 / 建议（均不阻断通过）

### L1：`TranslateWorkspace.onTranslate` 类型与 `handleTranslate` 签名存在轻微宽松性

- **文件:行**：`src/panels/translate/TranslateWorkspace.tsx:23` + `TranslatePage.tsx:207`
- **问题**：`TranslateWorkspaceProps.onTranslate` 声明为 `() => void`，而实际传入的 `handleTranslate` 签名是 `(textOverride?: string) => Promise<void>`。TypeScript 允许此赋值（函数参数双变 + void 接受 Promise），不会产生类型错误。但声明层与实现层签名不一致，若未来维护者在 TranslateWorkspace 内部直接传参调用 `onTranslate("xxx")` 将静默丢失——属于"类型说谎"。
- **建议**：将 `TranslateWorkspaceProps.onTranslate` 改为 `(textOverride?: string) => void`，或保持 `() => void` 同时在 `TranslatePage` 侧用箭头包裹 `() => handleTranslate()` 使签名匹配。两者均可，前者更诚实。
- **置信度**：82（确为不一致，但实际运行无 bug，TypeScript 不报错）

### L2：seed useEffect 中 `handleTranslate` 闭包读取时机

- **文件:行**：`TranslatePage.tsx:106-113`
- **问题**：`seedRef.current = seed`（第 105 行）在渲染期同步写入，但 `handleTranslate` 通过 useCallback 捕获 `inputText` 的快照——seed effect 触发时读取的 `handleTranslate` 是渲染时的最新版本（通过 eslint-disable 明确排除了其依赖），逻辑上由 `textOverride` 传参绕过了对 `inputText` 的依赖，因此无实际 bug。
- **说明**：这是已知的刻意设计（coding.md 有说明），eslint-disable 注释有对应说明行（第 102-103 行注释），符合规范要求的正当注释说明。无需更改，仅记录理解为已审查。
- **置信度**：N/A（已确认无问题，仅留文档痕迹）

---

## 各重点逐项核查结论

| 审查点 | 结论 |
|--------|------|
| `setTranslateSeed` 使用函数式更新 | 通过（`(prev) => ({ text: content, nonce: (prev?.nonce ?? 0) + 1 })`）|
| `setActiveTop` 使用函数式更新 | 通过（`(_prev) => "translate"`，虽然不依赖 prev，但形式合规）|
| `handleTranslate` typeof 守卫防合成事件 | 通过；TranslateWorkspace 第 83 行 `onClick={onTranslate}` 直接绑定，按钮点击会将 MouseEvent 作为第一参数注入，守卫正确拦截 |
| nonce 自增保证同文本重复点击能重新触发 | 通过；逻辑自洽 |
| seed useEffect seedRef 模式防闭包陈值 | 通过；textOverride 显式传参彻底规避了对 inputText state 的闭包依赖 |
| eslint-disable 有正当注释说明 | 通过（第 102-103 行注释说明了刻意设计原因）|
| 图片项不显示一键翻译按钮 | 通过；ClipPreview.tsx:179 `{item.kind !== "image" && (...)}` 守卫 |
| 空文本守卫 | 通过；`handleTranslate` 第 86 行 `if (text.trim().length === 0) return`；seed effect 第 108 行有同等守卫 |
| React 异步 setState 陷阱 | 通过；seed effect 通过 `textOverride` 传入 current.text，不依赖 `setInputText` 后读 state |
| 禁 any | 通过；三个文件无 any |
| catch 正确处理 promise | 通过；所有 async 调用均有 try/catch |
| 函数 ≤ 50 行 | 通过；`handleTranslate`（约17行）、`handleAction`（约27行）、`handleSwap`（约18行）均在限制内 |
| 死代码 / 装饰性分隔注释 | 通过；无此问题 |
| `onTranslateItem` 可选 prop 不破坏既有无 props 调用 | 通过；`ClipboardPageProps.onTranslateItem?` 可选，无 props 调用兼容 |
