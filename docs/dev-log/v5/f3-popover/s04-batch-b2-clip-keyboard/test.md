---
id: s04-batch-b2-clip-keyboard-test
title: Batch B 动态证伪（clip-popover 完整 UI + 键盘流）
status: 测试通过
commit: e9ef51c
date: 2026-06-02
---

# Batch B 动态证伪测试报告

## 1. 命中校验

运行命令：`pnpm test`

| 测试文件 | 测试数 | 状态 |
|---|---|---|
| `src/clip-popover/grouping.test.ts` | 13 tests | passed |
| `src/clip-popover/keyboard-nav.test.ts` | 12 tests | passed |
| `src/clip-popover/clip-popover-actions.test.tsx` | 5 tests | passed |
| 全量（34 files） | 300 tests | passed |

无假绿（N=300，非空匹配，exit 0 有实际用例跑到）。

## 2. 变异 Sanity（共 5 处，全部如期变红）

### 变异 A — groupClipItems 收藏项短路逻辑

改动：把 `if/else if/else` 改成两个独立 `if`，让收藏项同时落入 today/earlier。

变红：`收藏项进 favorites，不落入 today/earlier` + `混合场景` 共 2 tests FAILED。

还原：`cp /tmp/grouping.ts.bak` 复原，`git status` 干净。

### 变异 B — filterClipBySearch 过滤逻辑恒 true

改动：`.includes(lower)` 改为恒 `true`（不过滤）。

变红：`大小写不敏感命中`、`无命中返回空数组`、`精确子串命中` 共 3 tests FAILED。

还原：从备份复原，`git status` 干净。

### 变异 C — isToday 恒 true

改动：年月日比较整体改为 `return true`。

变红：`昨天返回 false`、`明天返回 false`、`更早的非收藏项进 earlier`、`混合场景` 共 4 tests FAILED。

还原：从备份复原，`git status` 干净。

### 变异 D — advanceSelection ArrowDown 不移动

改动：ArrowDown 分支直接返回 `flatIds[currentIndex]`（不调用 moveHighlight，不前进）。

变红：`列表中段 ArrowDown 前进一步` FAILED（expected 'c', got 'b'）。

还原：`cp /tmp/keyboard-nav.ts.bak` 复原，`git status` 干净。

### 变异 E — ClipPopoverApp pasteToFront 参数改为空字符串

改动：`pasteToFront(selectedId)` 改为 `pasteToFront("")`。

变红：`按 Enter 调 pasteToFront(selectedId) 并 hide`、`按 ArrowDown 再 Enter 粘贴第二项`、`pasteToFront 失败时不调 hide` 共 3 tests FAILED（断言 `toHaveBeenCalledWith("id1"/"id2")` 精确校验参数）。

还原：`cp /tmp/ClipPopoverApp.tsx.bak` 复原，`git status` 干净。

## 3. 边界探测

- **filterClipBySearch 纯空白 query**：走 `trimmed === ""` 分支返回全部副本，合理，有用例覆盖。
- **filterClipBySearch 大写 query**：`toLowerCase()` 双向转换，大小写不敏感正确，有用例覆盖。
- **filterClipBySearch content 为空串**：非空 query 时 `"".includes("x")` 返回 false，被过滤——合理，无专项用例但逻辑无误。
- **groupClipItems 空数组**：循环不执行，三组均空——有测试覆盖。
- **groupClipItems 全是收藏**：均进 favorites，today/earlier 为空——逻辑正确（混合场景测试覆盖了 fav1+fav2 同时出现的情况）。
- **跨天边界（昨天 23:59 vs 今天 00:00）**：`getDate()` 本地时间比较正确区分，测试用正午时间避免时区误判，行为正确。
- **advanceSelection currentId 不在 flatIds**：`indexOf` 返回 -1，走 `return flatIds[0]`，不崩溃，有专项用例覆盖（`"z"` 不在列表）。

无未覆盖的真实缺陷。

## 4. 构建与类型检查

- `pnpm build`：exit 0，89 modules transformed，三入口均产出：
  - `dist/index.html`
  - `dist/src/clip-popover/index.html`
  - `dist/src/trans-popover/index.html`
- `pnpm exec tsc --noEmit`：`TypeScript: No errors found`，exit 0。

## 5. 最终 git status（干净证明）

开工快照：`(clean)`（无输出）

收工快照：`(clean)`（无输出）

两次快照逐行一致，所有变异已还原，无新增或丢失文件。

## 6. 门禁结论

**测试通过** — 放行进入下一阶段。

- 命中校验：三个目标测试文件全部有真实用例跑到，无假绿。
- 变异 Sanity：A-E 5 处变异全部如期变红，测试有判别力，无恒真/旁路问题。
- 边界探测：无新发缺陷，边界行为合理。
- 构建 + tsc：全部通过，无类型错误。
