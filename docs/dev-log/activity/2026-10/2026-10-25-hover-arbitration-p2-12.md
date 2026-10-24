---
status: done
category: implementation
subject: Maccy 对标 · Phase 2（P2-12 悬停与键盘仲裁：鼠标真实位移才交还主导权）
summary: popover 列表的悬停改选中项此前无条件生效，键盘翻页导致列表滚动时，静止的鼠标会被浏览器补发 mouseenter/mousemove，把键盘刚选中的行抢走。修法是把「谁主导选中项」显式建模为 InputOwnership 仲裁器：键盘导航立即收回主导权；只有坐标真实变化的 pointer 移动才交给鼠标；悬停改选中必须先通过 isPointerOwner 判定。
evidence:
  files:
    - "src/clip-popover/keyboard-nav.ts（createInputOwnership 仲裁器）"
    - "src/clip-popover/PopoverList.tsx（handleHoverSelect / handlePointerMoveCapture）"
    - "src/clip-popover/ClipPopoverApp.tsx（键盘/数字直达/搜索输入/面板重开处 handToKeyboard）"
    - "src/clip-popover/hover-arbitration.test.tsx（组件级 3 条）"
  commands:
    - "rtk pnpm exec vitest run src/clip-popover  # 14 files / 164 passed"
    - "rtk pnpm exec tsc -b  # exit 0"
    - "rtk pnpm test  # 75 files / 804 passed"
    - "变异：注释掉 PopoverList.tsx 的 handleHoverSelect 守卫 → vitest 红 4 条（PopoverList 2 条 + hover-arbitration 2 条），还原后 164 绿"
  urls: []
---

# AI 活动记录 · P2-12 悬停与键盘仲裁

## 问题

popover 列表行绑定了 `onMouseEnter` 直接改选中项。键盘 ↑↓ 翻页后，列表滚动把另一行送到静止指针下，浏览器补发 `mouseenter` / 同坐标 `mousemove`，选中项被鼠标抢走——键盘刚选中的行瞬间失效。

## 决策

- **建模而非打补丁**：引入 `InputOwnership`，把「选中项当前由谁驱动」显式化，而不是在每个键盘分支里零散判断。
- **判据用坐标而非事件时序**：`observePointerMove(x, y)` 只在坐标与上一次记录不同时才返回 true 并交出主导权。滚动补发的同坐标移动因此不算真实位移。
- **`handToKeyboard()` 保留 `lastPosition`**：若清空坐标，「滚动补发 → 坐标被视为新位置 → 交还鼠标」的抢选中会重新出现。这一点由 `keyboard-nav.test.ts` 的「交还键盘后，同坐标移动仍不算真实位移」锁定。
- **进入与移动两个事件都接**：`mouseenter` 负责本行选中，`mousemove` 负责确认位移；移动走列表区的捕获阶段先裁决，再交给行。
- **键盘入口全部收回主导权**：方向键、`⌘↑↓` 首尾、数字直达、搜索框输入、面板重新打开时的首项选中。

## 取舍与边界

- 被否决的方案：仅靠 `mouseenter` 的时序去重。原因：浏览器补发与真实进入在事件序列上不可区分，只能靠坐标。
- 已知边界：若用户在键盘导航后恰好把鼠标移动到另一行并停住，选中会跟随鼠标（这是预期：真实位移即交还）。
- 本项只改 webview 层逻辑，**不依赖面板语义**（与 `phase2-gate-verification.md` 的依赖切分一致），面板 NSPanel 化后无需改写。

## 验收（实跑）

- 前端 clip-popover 子集 14 文件 / 164 测试通过；全量 `pnpm test` 75 文件 / 804 测试通过；`tsc -b` 退出 0。
- 变异可失败性：注释掉悬停守卫后 4 条仲裁测试变红，还原后全绿。
- 未验证：真机滚动时的手感（静止鼠标在长列表上键盘翻页的实际表现）；Rust 侧无改动，未跑 cargo。

## 审查

主线程自检 + 变异自证，**未做独立 reviewer 审查**（改动限于 clip-popover 单模块，未触碰公共契约之外的接口；`PopoverList` 新增的 `ownership` 为必填 props，调用点仅 `ClipPopoverApp` 一处）。
