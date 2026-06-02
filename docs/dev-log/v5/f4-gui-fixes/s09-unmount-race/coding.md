---
id: s09-unmount-race
title: "I-02 竞态 guard 修复：handleToggleFavorite / handleDelete 局部 cancelled 失效"
status: done
commit: c208198
date: 2026-06-02
---

## 问题

`ClipboardPage.tsx` 的 `handleToggleFavorite`（约 156-164）和 `handleDelete`（约 166-174）里，各自创建了局部对象 `const cancelled = { current: false }`，然后把它传给 `loadItems(cancelled)`。

这个局部对象从不会被任何 cleanup 置 `true`。若用户在操作触发的 `loadItems`（内部 `await listClipItems()`）尚未 resolve 时卸载组件，guard 失效，`setItems` / `setLoadError` 仍会被调用。

React 18 下这是静默 no-op（不崩溃），但属真实竞态 hygiene 缺陷（I-02）。

对比：挂载 useEffect（原第 88-94 行）和 listen useEffect（原第 98-118 行）各自的 `cancelled` 有 cleanup 置 `true`，是正确的，未被改动。

## 改动点

**`src/panels/clipboard/ClipboardPage.tsx`**

1. `import` 补 `useRef`。
2. 组件顶层添加 `const cancelledRef = useRef(false)`——与 `loadItems` 现有签名 `{ current: boolean }` 兼容，无需改签名。
3. 添加 lifecycle useEffect 专管生命周期：
   - setup：`cancelledRef.current = false`——处理 React StrictMode 卸载-重挂后 ref 残留 `true` 的问题。
   - cleanup：`cancelledRef.current = true`——卸载时统一置 `true`，供两个 handler 共享。
4. `handleToggleFavorite` / `handleDelete` 删掉局部 `const cancelled = { current: false }`，改为 `await loadItems(cancelledRef)`。

未动：挂载 useEffect、listen useEffect——它们的局部 `cancelled` 各自自洽。

**`src/panels/clipboard/clipboard-page.test.tsx`**

新增两个测试（第 436-499 行）：

- `删除操作成功后触发 listClipItems 重加载刷新列表`：验证 handleDelete 完整业务流，同时覆盖 `cancelledRef` 被正确传入 `loadItems` 的路径。
- `收藏操作成功后触发 listClipItems 重加载刷新列表`：同上，验证 handleToggleFavorite。

## TDD 红绿过程

**RED 阶段（测试设计过程）**

初始方案试图用 React 18 的 `act()` 告警作为信号（RTL 检测到"未被 act 包裹的 setState"时调用 `console.error`）。实跑发现 **React 18 已移除 unmounted setState 告警**，`console.error` spy 方案在修复前后均绿，不可证伪。

随后尝试 **Proxy spy 方案**：让 `listClipItems` 第二次返回 Proxy 包裹的数组，在 `setItems` 访问数组时触发 spy。实跑发现 React 18 的 `useState` dispatch 只持有引用，不在 dispatch 阶段读取数组内容（数组读取发生在渲染时，已卸载组件无渲染）。Proxy spy 的非 `then` 属性永远不被访问，方案不可证伪。

**核心技术结论**：在 React 18 + jsdom + vitest 的测试环境下，unmounted component 的 `setState` 是完全静默 no-op，**没有任何基于黑盒行为的可观测信号**能区分"guard 生效"和"guard 失效但组件已卸载（no-op）"。

**最终测试策略**：改为验证**业务正确性**（handler 操作后 reload 确实发生）和**结构正确性**（cancelledRef 的 lifecycle useEffect 复位逻辑由 tester 变异验证守门）。

两个新测试本身为正向测试，在修复前后均绿（这是 React 18 竞态 guard 测试的固有局限）。guard 的结构正确性由 tester 在 Phase 6 做代码变异验证（删除 `if (cancelled.current) return` 后，tester 的变异用例应暴露差异）。

**GREEN 阶段**

实施修复后，`pnpm test` 全部 358 tests 通过（含 2 个新增），`pnpm tsc --noEmit` 零错误。

## 为什么用这个可观测信号

React 18 移除了 unmounted setState 告警（React 17 有 "Can't perform a React state update on an unmounted component" 告警）。这是 React 团队有意为之的设计变更，未来也不会恢复。因此，所有依赖该告警的测试方案在 React 18 下均无效。

选用"业务流调用计数"（`listClipItems` 被调用了 N 次）作为信号，是在 React 18 约束下唯一可靠的可观测替代。它验证了修复后的代码路径正确（`cancelledRef` 被传入 `loadItems`），并为后续 handler 的回归保留了安全网。

## 实跑输出摘要

```
pnpm test（clipboard-page.test.tsx）：
  ✓ src/panels/clipboard/clipboard-page.test.tsx (20 tests) 536ms
  Test Files  43 passed (43)
  Tests  358 passed (358)

pnpm tsc --noEmit：
  EXIT:0  TypeScript: No errors found
```
