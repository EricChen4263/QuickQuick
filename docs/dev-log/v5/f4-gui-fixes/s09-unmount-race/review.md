---
id: s09-unmount-race-review
type: review
level: 小功能
parent: v5-f4-gui-fixes
created: 2026-06-02T00:00:00Z
status: 通过
commit: c208198
acceptance_ids: [I-02]
author: code-reviewer
---

# s09-unmount-race 代码审查报告（ClipboardPage 卸载竞态修复 I-02）

## 审查范围

- `src/panels/clipboard/ClipboardPage.tsx`（cancelledRef + lifecycle useEffect + handler 改动）
- `src/panels/clipboard/clipboard-page.test.tsx`（新增两个正向 reload 测试）

依据：code-standards + 项目规范（禁 any / 函数≤50行 / 命名 / 注释写「为什么」/ 禁死代码 / 禁装饰分隔符）。

---

## 高优先级问题（必修才放行）

**无。**

---

## 中/低优先级问题（不阻塞放行）

以下两条均为预存在设计取舍，本次 diff 未引入，置信度低于 80，仅作记录。

**[NOTE-01] loadItems 失败路径与 handler catch 行为不对称（置信度 60，预存在）**

位置：`ClipboardPage.tsx` 第 167-174 行 / 第 176-183 行。

`loadItems` 内部 try/catch 自行 setLoadError 不向外抛，外层 handler catch 只能捕获 IPC 操作（toggleFavoriteClip / deleteClipItem）的失败，两条错误路径分别设置 loadError（全页替换）和 opError（banner）。语义上轻微不对称，但属设计选择，非 bug。

**[NOTE-02] 并发 handler 多次 loadItems in-flight 可能交错（置信度 45，预存在）**

位置：`ClipboardPage.tsx` handler 层。cancelledRef guard 只防「卸载后 setState」，不防「多个 handler 并发触发时 setItems 交错写入」。React 批处理通常不影响最终一致性，实际影响极低，且为预存在设计。

---

## 两套 cancelled 机制并存评估

**评估结论：两套机制并存是正确设计，不应统一，无需改动。**

挂载 useEffect（第 99-105 行）和 listen useEffect（第 109-129 行）各持独立局部 `cancelled`，其 cleanup 在 effect 被清除时触发——包括依赖变化引起的 re-run。cancelledRef 的 lifecycle useEffect cleanup 只在整个组件卸载时触发。

若将 handler 的 guard 与 effect 的局部 cancelled 合并，当 `loadItems` 依赖变化导致 effect 重跑时，effect cleanup 会提前将共享 ref 置 `true`，误拦组件仍在挂载中的 handler loadItems 调用。两套机制 scope 不同（effect 生命周期 vs 组件生命周期），合并会引入新竞态，应保持现状。

---

## 规范符合性结论

| 项目 | 结论 |
|------|------|
| cancelledRef lifecycle useEffect 正确性 | StrictMode 复位（setup 置 false）+ 卸载 cleanup（置 true），deps=[]，逻辑正确 |
| 共享 ref 多处 async 读写 | handler 只传 ref 引用，loadItems 内只读，无竞态问题 |
| 函数长度 | 各 handler ≤ 8 行，符合规范 |
| 命名 | cancelledRef / handleXxx 符合描述性命名 |
| 注释 | 第 69-71 行解释「为什么」，符合规范 |
| 禁 any | 无 any，类型齐全 |
| 死代码 | 无 |
| 装饰分隔符 | 无 |
| 测试注释 | 两新测试均有「为什么」说明（含 React 18 guard 不可黑盒证伪的边界声明）|

---

## 放行结论

**通过。**

cancelledRef lifecycle useEffect 逻辑正确（StrictMode 复位 + 卸载 cleanup），两套 cancelled 机制并存是有意设计（scope 不同，不能合并），共享 ref 多处 async 读写无竞态隐患，规范各项符合，无高置信度问题。

guard 竞态不可黑盒证伪属 React 18 已知约束（tester 已实证确认），由代码审查 + 全绿套件守门，符合既定判定原则，不因此降级。
