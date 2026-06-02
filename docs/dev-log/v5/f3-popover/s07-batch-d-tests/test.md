---
id: s07-batch-d-tests-tester
title: Batch D 新测试判别力验证
status: 测试通过
commit: 4b2fe93
date: 2026-06-02
---

## 命中校验

pnpm test --run 全套跑 40 个文件，335 tests passed。

Batch D 3 个目标文件全部命中运行：

| 文件 | 用例数 | 结果 |
|---|---|---|
| src/clip-popover/PopoverList.test.tsx | 5 | 通过 |
| src/clip-popover/PopoverPreview.test.tsx | 6 | 通过 |
| src/trans-popover/MiniTranslate.test.tsx | 5 | 通过 |

无假绿（每个文件均有明确的 `N tests` 通过记录，非空匹配）。

## 变异 sanity

### 变异 A — PopoverList.tsx：「收藏」标题改为「星标」

- 改动：`sed -i '' 's/label="收藏"/label="星标"/'`
- 预期：分组标题断言变红
- 实际：2 个用例 FAIL（「三组均有条目时渲染收藏/今天/更早三个标题」和「只有收藏组时不渲染今天/更早标题」）
- 结论：如期变红，测试有判别力
- 还原：`cp /tmp/PopoverList.tsx.bak` 复原，`git status --porcelain` 干净

### 变异 B — PopoverPreview.tsx：「已收藏」badge 文案改为「收藏中」

- 改动：`sed -i '' 's/已收藏<\/span>/收藏中<\/span>/'`
- 预期：「收藏条目渲染「已收藏」badge」变红
- 实际：1 个用例 FAIL（「收藏条目渲染「已收藏」badge」—— getByText("已收藏") 找不到元素）
- 结论：如期变红，测试有判别力
- 还原：`cp /tmp/PopoverPreview.tsx.bak` 复原，`git status --porcelain` 干净

### 变异 C — MiniTranslate.tsx：复制按钮 onClick 从 onCopy 改为 onSpeak

- 改动：`sed -i '' 's/aria-label="复制" onClick={onCopy}/aria-label="复制" onClick={onSpeak}/'`
- 预期：「点复制按钮调 onCopy 回调」变红
- 实际：1 个用例 FAIL（expected "spy" to be called 1 times, but got 0 times）
- 结论：如期变红，测试有判别力
- 还原：`cp /tmp/MiniTranslate.tsx.bak` 复原，`git status --porcelain` 干净

## 边界探测

本轮 Batch D 测试覆盖了以下边界：

- PopoverList：全空组 → 占位文案；单组有条目另两组为空 → 其余标题不渲染
- PopoverPreview：item=null → 空态；图片无 thumbnailDataUrl → [图片] 占位；非收藏不渲染 badge
- MiniTranslate：三个回调按钮各自独立测试，防止串扰

以上边界均在测试用例中覆盖，运行通过，无 panic / 无静默错。

## git 干净证明

开工快照：干净（无任何修改）
结束快照：`git status --porcelain` 输出为空（干净）
全套复跑：335 passed / 40 files

## 最终判定

测试通过。Batch D 3 个新测试文件（16 个用例）均有真实判别力，非恒真/旁路。变异 A-C 全部如期变红，还原后全套绿色。
