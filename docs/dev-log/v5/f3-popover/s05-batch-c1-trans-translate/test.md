---
id: batch-c-trans-popover-test
title: Batch C 动态证伪
status: 测试通过
commit: 36f4d93
date: 2026-06-02
---

# Batch C 动态证伪报告

## 命中校验

运行命令：`pnpm test --run`（全量，连跑1次，非并发敏感场景）

| 测试文件 | 用例数 | 结果 |
|---|---|---|
| src/trans-popover/source-text.test.ts | 5 | 全通过 |
| src/trans-popover/retranslate.test.ts | 4 | 全通过 |
| src/trans-popover/trans-popover.test.tsx | 6 | 全通过 |
| 全套 (37 文件) | 316 | 全通过 |

三目标测试文件均有实际运行（grep 到测试名 + 数量），无假绿。

## 变异 sanity

每次：cp 备份 → 改坏 → 跑测试确认变红 → cp 还原（严禁 git checkout，防抹未提交改动）。

| 变异 | 改动内容 | 预期 | 实际 | 结论 |
|---|---|---|---|---|
| A | source-text.ts：去掉 `content.length > 0 ? content : null` 守卫，空串直接返回 | 空串/空白/图片3个用例变红 | 3 failed | 如期变红 |
| B | source-text.ts：取 `items[1]` 而非 `items[0]` | "正常取最新"用例变红 | 1 failed，Expected "hello world" Received "older" | 如期变红 |
| C | retranslate.ts：恒返回 true（注释掉 null 守卫，改 return true） | null不重译 + 相同文本不重译 共2个用例变红 | 2 failed | 如期变红 |
| D | TransPopoverApp.tsx：`translateText(text!)` 改成 `translateText("")` | 渲染译文用例捉住参数不对 | 1 failed，toHaveBeenCalledWith("Hello world") 断言失败 | 如期变红 |
| E | TransPopoverApp.tsx：`emit("route","translate")` payload 改成 "clipboard" | 展开测试断言 payload 变红 | 1 failed，Expected "translate" Received "clipboard" | 如期变红 |

**所有5处变异均如期变红，测试有真实判别力，非恒真/旁路。**

变异全部还原后 `pnpm test --run` 回全绿（316 passed），git 工作树被测文件无残留改动。

## 边界探测

1. **pickLatestText - 前后空白但中间有字**：`"  hello  "` → trim 后返回 `"hello"`（非空，符合「trim后非空则返回」设计意图）。行为合理。

2. **shouldRetranslate - firstTime（lastText 为 null，newText 有值）**：返回 `true`——首次有文本触发翻译，符合「首次有文本要译」预期。

3. **shouldRetranslate - 两者都 null**：返回 `false`——无内容不译，符合预期。

4. **translateText 抛错路径**：TransPopoverApp.tsx 有顶层 `catch` 统一处理，设 `status="error"` 渲染"翻译失败"文案，不崩溃。已有测试覆盖（`translateText.mockRejectedValue`）。

5. **listClipItems 抛错路径**：同一 `try/catch` 块覆盖，IPC 调用失败也会走 error 态，功能上安全。但测试中未覆盖此路径（无 `listClipItems.mockRejectedValue` 用例）——属轻微覆盖缺口，不影响核心验收。

## 构建 + tsc sanity

- `pnpm build`：exit 0，三入口产出（dist/index.html / dist/src/trans-popover/index.html / dist/src/clip-popover/index.html）。
- `pnpm exec tsc --noEmit`：`TypeScript: No errors found`，exit 0。

## git 干净证明

开工快照：`git status --porcelain` → 空（工作树干净）

结束快照：`git status --porcelain` → 仅有 `?? docs/dev-log/v5/f3-popover/s06-batch-c2-trans-expand/review.md`（系 code-reviewer agent 留下的未追踪文件，非本次变异产生）。

被变异的三个实现文件（source-text.ts / retranslate.ts / TransPopoverApp.tsx）已通过备份还原，与开工时逐字一致。

## 门禁结论

**测试通过。放行。**

所有变异均如期变红（A/B/C/D/E 全部）；命中校验无假绿；构建和类型检查干净；边界逻辑符合设计意图；唯一缺口（listClipItems 抛错无独立测试）不影响核心验收功能，实现层面有 catch 兜底。
