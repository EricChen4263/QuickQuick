---
id: f4-gui-fixes-s02
title: 一键翻译跳转接真
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 改动文件

- `src/App.tsx`：新增 `translateSeed` state 和 `handleTranslateFromClip`，给 ClipboardPage 传 `onTranslateItem`，给 TranslatePage 传 `seed`。
- `src/panels/clipboard/ClipboardPage.tsx`：新增 `ClipboardPageProps`（`onTranslateItem?`），`onTranslate` 占位改为调 `onTranslateItem?.(item.content)`。
- `src/panels/translate/TranslatePage.tsx`：新增 `TranslatePageProps`（`seed?`），`handleTranslate` 改为 `useCallback` 并接受 `textOverride?: string`，新增 seed `useEffect` 监听 `seed?.nonce`。
- `src/panels/translate/translate-page.test.tsx`：新增 4 条 seed prop 测试。
- `src/panels/clipboard/clipboard-page.test.tsx`：新增 1 条 `onTranslateItem` 回调透传测试。

## 关键设计说明

- **`seed { text, nonce }` 用 nonce**：TranslatePage 常驻 DOM（display 切显），inputText 可能与上次相同；nonce 自增确保 useEffect 依赖变化，同一条目重复点击也能重新触发翻译。
- **`handleTranslate` textOverride typeof 守卫**：TranslateWorkspace 翻译按钮用 `onClick={onTranslate}`（非箭头包裹），合成事件对象会作为第一个参数传入；`typeof textOverride === "string"` 守卫过滤事件对象，仅显式字符串时才覆盖 inputText。
- **seed useEffect 依赖只放 `seed?.nonce`**：刻意不依赖 `handleTranslate`（useCallback 内含 inputText 依赖，放入会在每次输入时误触发）；通过 `seedRef` 在 effect 内安全读取最新 seed 值。

## 验证结果

- `pnpm test`：Tests 340 passed (340)（5 条新增测试全绿）
- `pnpm exec tsc --noEmit`：exit 0，无类型错误
- `pnpm build`：exit 0，built in 369ms
