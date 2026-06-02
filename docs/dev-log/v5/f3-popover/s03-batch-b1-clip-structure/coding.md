---
id: f3-popover-s03-batch-b1
title: "里程碑4 popover · Batch B1：clip-popover 结构与数据"
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 新建/改动文件

| 文件 | 说明 |
|---|---|
| `src/components/KindIcon.tsx` | 从 ClipItemRow 提取的独立图标组件，供列表行与 popover 共用 |
| `src/panels/clipboard/ClipItemRow.tsx` | 改为从 KindIcon 组件导入，移除内联 SVG 重复代码 |
| `src/clip-popover/grouping.ts` | 三纯函数：`isToday` / `filterClipBySearch` / `groupClipItems` |
| `src/clip-popover/grouping.test.ts` | TDD 覆盖全三函数（13 个测试） |
| `src/clip-popover/ClipPopoverApp.tsx` | 根组件：IPC 加载 + 搜索框 + 分组列表 + 预览 + footer |
| `src/clip-popover/PopoverList.tsx` | 左侧列表区，三分组渲染 + 鼠标点击选中 |
| `src/clip-popover/PopoverPreview.tsx` | 右侧预览区，展示类型/时间 kicker + 内容 |
| `src/clip-popover/PopoverFooter.tsx` | 底部快捷键提示静态条 |
| `src/clip-popover/popover.css` | 720 布局：搜索行/左列表/右预览/footer，毛玻璃样式 |

## TDD 过程

grouping.ts 先写测试（RED）：`isToday` 同日/昨天/明天、`filterClipBySearch` 空 query / 大小写 / 无命中、`groupClipItems` 收藏独占 / 今天 / 更早 / 混合 / 空数组。测试失败后写最小实现（GREEN），三函数逻辑简单无需重构。

## 验证结果

```
Tests: 283 passed (283) — grouping.test.ts 13 tests ✓，全套无失败
tsc --noEmit: No errors found
build exit 0，三入口产出：
  dist/index.html
  dist/src/clip-popover/index.html
  dist/src/trans-popover/index.html
```

## B2 衔接说明

**扁平顺序来源**：`ClipPopoverApp` 中的 `buildFlatList(groups)` 将 favorites → today → earlier 拼成 `visibleFlatList`，键盘 ↑↓ 直接在此数组上做 index 加减。

**selectedId 位置**：`ClipPopoverApp` 的 `useState<string | null>` 持有 `selectedId` / `setSelectedId`，已经传入 `PopoverList`；B2 只需把 `setSelectedId` 也传给键盘 handler（或提升到 `main.tsx` 的 ref）。

**粘贴动作（Enter）**：
```ts
import { pasteToFront } from "../ipc/ipc-client";
// 调用：pasteToFront(selectedId)
```

**复制动作（Alt+Enter）**：
```ts
import { writeToClipboard } from "../panels/translate/browser-api";
// 调用：writeToClipboard(selectedItem.content)
// 注意：图片条目可按 imageId 另走 getClipImageOriginal，B2 可视需要处理
```

**Esc 关闭**：已在 `src/clip-popover/main.tsx` 监听 `keydown`，调 `getCurrentWindow().hide()`，B2 无需重复接入。
