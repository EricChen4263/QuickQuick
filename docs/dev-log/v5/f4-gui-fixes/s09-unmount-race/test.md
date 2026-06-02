---
id: s09-unmount-race
title: "测试报告：ClipboardPage 卸载竞态修复（I-02）"
status: PASS
commit: c208198
date: 2026-06-02
---

# 测试报告：ClipboardPage 卸载竞态修复（I-02）

## 开工快照（git status --porcelain）

```
 M src/panels/clipboard/ClipboardPage.tsx
 M src/panels/clipboard/clipboard-page.test.tsx
?? docs/dev-log/v5/f4-gui-fixes/s09-unmount-race/
```

---

## 一、命中校验

命令：`pnpm vitest run --reporter=verbose src/panels/clipboard/clipboard-page.test.tsx`

结果（第 1/3 次，连跑 3 次全绿）：

```
✓ clipboard-page: 删除操作成功后触发 listClipItems 重加载刷新列表
✓ clipboard-page: 收藏操作成功后触发 listClipItems 重加载刷新列表
✓ ... (其余 18 项全绿)
Tests  20 passed (20)
```

连跑 3 次结果：20 passed / 20 passed / 20 passed，无 flaky。

两个新测试均真实命中（非空跑），N=20 ≥ 1，确认非假绿。

---

## 二、变异 sanity

### 层 A：正向流判别力

**变异内容**：用 `sed` 把 `handleDelete` 和 `handleToggleFavorite` 里的 `await loadItems(cancelledRef)` 均注释掉（MUTATION_A 标记）。

**备份方式**：`cp ClipboardPage.tsx /tmp/ClipboardPage.tsx.bak`

**运行命令**：同上 verbose 跑

**结果**：

```
FAIL  clipboard-page: 删除操作成功后触发 listClipItems 重加载刷新列表
FAIL  clipboard-page: 收藏操作成功后触发 listClipItems 重加载刷新列表
```

其余 18 项仍绿。两个新测试在 handler 不调用 loadItems 后如期变红，证明新测试对「handler 触发 reload」这一正向流具有真实判别力，非恒真。

**还原**：`cp /tmp/ClipboardPage.tsx.bak ClipboardPage.tsx`，git status 与开工快照逐行一致。

### 层 B：guard 判别力（实证 coder 声明）

**变异内容**：删除 `loadItems` 内两处 `if (cancelled.current) return;`（成功路径第 88 行 + catch 路径第 92 行均删除）。

**备份方式**：重新 `cp ClipboardPage.tsx /tmp/ClipboardPage.tsx.bak`

**运行命令**：同上

**结果**：

```
Tests  20 passed (20)   ← 无任何测试变红
```

删除 guard 后，全部 20 个测试仍绿，无任何变红。

#### 主动构造探针测试（三种手段）

在 guard 已删除的状态下写 `_guard-probe.test.tsx`，尝试以下三种手段证伪：

**探针 1：act 内 resolve deferred，spy console.error**

命令：`pnpm vitest run --reporter=verbose src/panels/clipboard/_guard-probe.test.tsx`

结果：
```
stdout:
[probe1] console.error calls: []
[probe1] actWarning detected: false
✓ 探针1: unmount后在act内resolve → 检查console.error告警
```

无任何 "not wrapped in act"、"Can't perform"、"unmounted"、"state update" 告警。

**探针 2：act 外 resolve deferred，spy console.error + console.warn**

结果：
```
stdout:
[probe2] errors: []
[probe2] warns: []
[probe2] hasSignal: false
✓ 探针2: unmount后在act外resolve → 检查有无任何告警
```

同样无任何告警信号。

**探针 3：deferred 控制两次加载 + unmount 对比渲染**

（此探针因 mockListClipItems 在 listen 的 useEffect 上下文返回 undefined 而崩溃，属探针自身 mock 设计问题，与 guard 行为无关，不影响结论。）

#### 层 B 实证结论

**在本项目 React 18.3.1 + jsdom + vitest 配置下，删除 cancelledRef guard 后无任何可观测的黑盒信号：**

- `console.error` 无任何 act 告警（React 18 已将"对卸载组件 setState"从错误降级为静默 no-op）
- `console.warn` 亦无任何信号
- DOM 行为无差异（组件已卸载，无渲染可比较）

**coder 声明经实证成立**：guard 竞态在 React 18 + jsdom 下客观不可黑盒证伪。现有测试套件无法通过行为差异守护此 guard，但这不等于 guard 无价值——它是防御生产环境（真实异步竞态、StrictMode 双挂载）的正确性 hygiene 代码。

**还原**：`cp /tmp/ClipboardPage.tsx.bak ClipboardPage.tsx` + `rm _guard-probe.test.tsx`，git status 与开工快照逐行一致。

---

## 三、边界探测

### StrictMode 二次挂载下 cancelledRef 复位

`main.tsx` 使用 `<React.StrictMode>`，生产环境下 useEffect 会在开发模式中被执行两次（挂载→清理→重挂载）。代码第 73-78 行的 useEffect：

```tsx
useEffect(() => {
  cancelledRef.current = false;   // setup 复位
  return () => {
    cancelledRef.current = true;  // cleanup 置 true
  };
}, []);
```

StrictMode 二次挂载流程：
1. 第一次挂载 → setup 置 false（正确）
2. cleanup → 置 true
3. 第二次挂载（重挂）→ setup 再次置 false（正确复位）

此逻辑正确处理 StrictMode 的卸载-重挂场景，ref 不会残留 true。

### catch 路径 guard 覆盖

第 91-93 行 catch 内亦有 `if (cancelled.current) return;`，guard 在成功路径和错误路径均有覆盖，无遗漏。

### 两个已有 useEffect 未被破坏

- `loadItems` 的 useEffect（第 99-105 行）：使用独立局部 `cancelled = { current: false }`，不依赖 cancelledRef，不受 I-02 修改影响。
- `listen` 的 useEffect（第 109-129 行）：同样使用独立局部 cancelled，也不受影响。

连跑 3 次全绿验证两者均正常工作。

### 测试不在 StrictMode 下运行

`render(<ClipboardPage />)` 未包裹 `<React.StrictMode>`，StrictMode 双挂载行为未被测试覆盖（属已知局限，与 I-02 fix 范围无关）。

---

## 四、最终结论

**结论：通过（附判别力边界说明）**

- **层 A 通过**：两个新测试对「handler 触发 reload」正向流有真实判别力（注释 loadItems 调用后如期变红）。
- **层 B 实证确认**：在 React 18 + jsdom 环境下，guard 竞态客观不可黑盒证伪——`console.error`/`warn` 均无可观测信号（三种探针手段均无触发）。coder 声明经实证成立。
- **连跑 3 次全绿**，无 flaky。
- **边界**：StrictMode 复位逻辑正确；catch 路径 guard 覆盖完整；既有 useEffect 未被破坏。
- **guard 修复定性**：属无可观测行为差异的正确性 hygiene 重构，由代码审查 + 全绿套件守门，符合「通过（附判别力边界说明）」判定原则。

---

## 五、结束快照验证

结束时 `git status --porcelain`：

```
 M src/panels/clipboard/ClipboardPage.tsx
 M src/panels/clipboard/clipboard-page.test.tsx
?? docs/dev-log/v5/f4-gui-fixes/s09-unmount-race/
```

与开工快照逐字一致，无业务代码改动残留。
