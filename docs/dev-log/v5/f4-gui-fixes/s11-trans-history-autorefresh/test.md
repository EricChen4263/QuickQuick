---
id: s11-trans-history-autorefresh
title: 翻译历史栏自动刷新（事件驱动）测试留痕
status: passed
commit: 25297c7
date: 2026-06-03
---

# 测试留痕：翻译历史栏自动刷新（s11）· 动态证伪

## 开工 git status 快照

```
 M src-tauri/src/ipc/translate.rs
 M src/ipc/events.ts
 M src/panels/translate/TranslatePage.tsx
 M src/panels/translate/translate-page.test.tsx
```

---

## 一、命中校验（杀假绿）

### 前端新增测试命中行

```
✓ src/panels/translate/translate-page.test.tsx > translate-page > translate-page: 收到 translate-history-changed 事件后触发 listTranslateHistory 重新加载
```

### 全量

```
# 前端
Test Files  43 passed (43)
      Tests  364 passed (364)

# 后端
cargo test -p quickquick: test result: ok. 67 passed; 0 failed + doc-tests 1 passed
```

结论：新增测试命中，非空匹配假绿。

---

## 二、变异 sanity（杀恒真/旁路）

### 变异A — 事件名订阅校验

变异方式：把 `TranslatePage.tsx` 订阅处 `listen(TRANSLATE_HISTORY_CHANGED_EVENT, ...)` 改为硬编码错误字符串 `listen("translate-history-WRONG", ...)`。

> 注：不改 `events.ts` 常量定义——测试导入同一常量做断言对比，改常量定义两端同步变化测试不会红（这是测试设计正确：用常量对比常量）。变异须改**实现侧的使用处**，才能验证测试真在校验订阅的事件名。

结果如期变红：
```
Tests  1 failed | 19 passed (20)
```
失败点：`expect(mockListen).toHaveBeenCalledWith(TRANSLATE_HISTORY_CHANGED_EVENT, ...)` — 期望 `"translate-history-changed"`，实际 `"translate-history-WRONG"`。

还原后复绿：20 passed。

### 变异B — 回调效果校验

变异方式：把监听回调里 `void fetchHistory(cancelled);` 改为 `void 0;`（空操作）。

结果如期变红：
```
Tests  1 failed | 19 passed (20)
```
失败点：`expect(mockListTranslateHistory).toHaveBeenCalledTimes(2)` — waitFor 超时，事件触发后未再调用（只剩挂载时 1 次）。证明测试真校验了「收到事件 → 真的触发重新加载」，非只测注册不测效果。

还原后复绿：20 passed。

变异 sanity 结论：两个变异均如期变红，测试有真实判别力，非恒真/旁路。

---

## 三、边界探测

### 1. 后端 emit 时机——只在成功时发

`translate.rs` emit 段：
```rust
if result.is_ok() {
    if let Err(e) = app.emit(TRANSLATE_HISTORY_CHANGED_EVENT, ()) {
        eprintln!("[QuickQuick] 发送 {TRANSLATE_HISTORY_CHANGED_EVENT} 事件失败: {e}");
    }
}
```
翻译失败 `result.is_err()` 时不进入 if 块、不 emit，直接返回 Err。语义正确——失败路径不会导致历史栏空刷。

### 2. fetchHistory 引用稳定性——安全，无抖动（重点）

`TranslatePage.tsx`：
```tsx
const fetchHistory = useCallback(async (cancelled: { current: boolean }) => { ... }, []);
```
deps 为 `[]` 空数组，`fetchHistory` 组件生命周期内引用恒定。监听 useEffect 的 deps=`[fetchHistory]` 因引用永不变，只在挂载/卸载各运行一次，**不存在「每次渲染 unlisten+重新 listen」的抖动或泄漏**。

### 3. 事件名两端一致性核对

- 后端 `translate.rs:30`：`const TRANSLATE_HISTORY_CHANGED_EVENT: &str = "translate-history-changed";`
- 前端 `events.ts:9`：`export const TRANSLATE_HISTORY_CHANGED_EVENT = "translate-history-changed" as const;`

字面量逐字符完全一致：`"translate-history-changed"`。核对通过。

### 4. cancelled+unlisten 卸载竞态

早取消守卫（`cancelled.current` 为真时立即 `fn()`）正确处理「listen 注册完成前组件已卸载」；cleanup 置 `cancelled.current = true` 并调 `unlisten?.()`。双路防护完整，无竞态漏洞。

### 边界探测结论

未发现真实缺陷。

---

## 四、抗 flaky 校验

翻译测试不涉及排序/时间戳/共享资源竞争，为单组件渲染测试（完全 mock 隔离），无 flaky 风险。

---

## 五、最终门禁结论

**PASS（放行）**

- 命中校验：前端新增测试真命中；364 前端 + 68 后端全绿
- 变异 sanity：变异A（错误事件名）、变异B（移除回调）均如期变红再复绿，测试有判别力
- fetchHistory 稳定性：`useCallback(fn, [])` 稳定引用，安全无抖动
- emit 仅成功时发，语义正确；卸载竞态守卫完整
- 事件名两端逐字符一致
- 工作树还原干净，4 个 M 文件与开工快照逐行一致，无残留业务代码改动

## 结束时 git status --short（业务代码部分）

```
 M src-tauri/src/ipc/translate.rs
 M src/ipc/events.ts
 M src/panels/translate/TranslatePage.tsx
 M src/panels/translate/translate-page.test.tsx
```

与开工快照逐行一致，无残留业务代码改动。
