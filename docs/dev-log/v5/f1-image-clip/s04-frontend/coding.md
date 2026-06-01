# S04 前端渲染层 — 编码留痕

## 阶段信息

- 版本：V5 / F1（图片剪贴板·能看）/ S04（前端渲染层）
- 执行时间：2026-06-01
- 执行者：coder agent（claude-sonnet-4-6）

## 改动文件清单

| 文件 | 改动内容 |
|------|----------|
| `src/ipc/ipc-client.ts` | ClipItem 接口加 `thumbnailDataUrl?: string` + `imageId?: string`；新增 `getClipImageOriginal(imageId)` 函数 |
| `src/panels/history/search.ts` | HistoryItem.kind 由 `"text" \| "richtext"` 扩为 `"text" \| "richtext" \| "image"` |
| `src/panels/history/filter.ts` | HistoryFilter 加 `"image"` 联合成员 |
| `src/panels/clipboard/ClipSearchBar.tsx` | FILTER_LABELS 加 `image: "图片"` |
| `src/panels/clipboard/ClipboardPage.tsx` | toHistoryItem 加 image 分支（`clip.kind === "image"` → kind: "image"） |
| `src/panels/clipboard/ClipItemRow.tsx` | 加 ImageContent 子组件：有 thumbnailDataUrl 渲染 `<img>`，否则显示 "[图片]" 占位；文本项保持原有 truncateSummary |
| `src/panels/clipboard/ClipPreview.tsx` | 加 ImagePreview 子组件（useEffect + cancelled ref 防 stale，异步加载原图，失败/null 回退缩略图）；图片项走 ImagePreview，文本项保持 `<p>` |
| `src/panels/clipboard/clip-image.test.tsx` | 新增测试文件（见下） |

## 新增测试

| 测试名 | 覆盖点 |
|--------|--------|
| filterByType image > filter=image 只返回 kind=image 的条目 | filter.ts image 类型筛选 |
| filterByType image > filter=text 不含 image 条目 | text 筛选不含 image |
| filterByType image > filter=all 包含 image 条目 | all 筛选包含 image |
| ClipItemRow image rendering > 图片项有 thumbnailDataUrl 时渲染 `<img>` | 缩略图渲染 |
| ClipItemRow image rendering > 图片项无 thumbnailDataUrl 时渲染 "[图片]" 占位 | 无缩略图占位 |
| ClipItemRow image rendering > 文本项不渲染 img | 文本项回归不变 |
| ClipPreview ImagePreview > 加载中显示缩略图，成功后显示原图 img | 原图加载流程 |
| ClipPreview ImagePreview > getClipImageOriginal 返回 null 时显示缩略图回退 | null 降级回退 |
| ClipPreview ImagePreview > 文本项显示 `<p>` 不渲染 img | 文本预览回归不变 |

## TDD 流程记录

**RED 阶段**：先写 `clip-image.test.tsx`，运行确认 4 个测试因功能未实现而失败：
- ClipItemRow 图片项 img 渲染（2 个）
- ClipPreview ImagePreview 原图/null 回退（2 个）

filter image 测试在 RED 时意外通过——因为 filterByType 实现是 `item.kind === filter`，只要类型定义扩展即可通过。

**GREEN 阶段**：按序实现 7 个文件，最终 152 tests passed（18 test files）。

**REFACTOR**：无需重构，实现已足够简洁。

## 关键实现决策

1. **cancelled ref 模式**：ImagePreview 内用局部 `cancelled` 对象（非 `useRef` 持久引用），与 ClipboardPage.loadItems 保持一致的写法——每次 effect 创建新的 `{ current: false }` 对象，cleanup 置 true，异步回调前 guard。

2. **originalDataUrl === null 与缩略图回退**：用 `originalDataUrl ?? thumbnailDataUrl ?? null` 合并状态，初始态（未加载完）和 API 返回 null 都回退到缩略图显示，逻辑集中。

3. **ImageContent 子组件**：将图片内容区（有缩略图 vs 无缩略图）抽为独立子组件，保持 ClipItemRow 主体结构清晰，单一职责。

4. **filterByType 无需改逻辑**：原实现 `items.filter(item => item.kind === filter)` 天然支持任意 kind 值，只需扩展类型联合。

## Reviewer 打回修复（2026-06-01）

### 修复清单

**I-1 删 cancelledRef 死代码**
- `ClipPreview.tsx` 删除 `cancelledRef = useRef({ current: false })` 声明及 `cancelledRef.current = cancelled` 赋值，去掉 `useRef` import。
- 防 stale 由 effect 内局部 `const cancelled = { current: false }` 闭包保证，逻辑不变。
- 组件注释同步改为「用 effect 内局部 cancelled 闭包防止 stale 更新」。

**I-2 filter.ts JSDoc 同步**
- `filterByType` JSDoc 补充 `"image" 仅返回图片项`，与 `HistoryFilter` 的 `"image"` 成员一致。

**I-3 测试名与断言一致**
- `clip-image.test.tsx`「加载中先显示缩略图，成功后显示原图 img」测试，在 render 后、`waitFor` 前新增同步断言：
  `expect(screen.getByRole("img")).toHaveAttribute("src", imageItem.thumbnailDataUrl)`
- 使测试名声称的「加载中先显示缩略图」确实被断言覆盖。

**补 stale 防护测试（守护 I-1）**
- 新增测试「快速切换 imageId 时旧请求迟到 resolve 不覆盖新结果（stale 防护）」。
- 使用两个可手动控制 resolve 的 deferred Promise，按 imageId 区分。
- 流程：render item1 → rerender item2 → 先 resolve item2 → 再 resolve item1（模拟迟到）。
- 断言最终 img.src 为 item2 的原图，不被 item1 迟到 resolve 覆盖。
- 若删除 `cancelled` guard，item1 的迟到 resolve 会写入 state，测试因断言 `src !== originalUrl1` 而失败——确认能抓到 stale；保留 guard 时通过。

### 测试数量变化

修复前：152 tests passed（18 test files）
修复后：153 tests passed（18 test files，新增 1 个 stale 防护测试）

## 验证结果

- 全量测试：`pnpm test` → 153 passed, 0 failed（18 test files）
- TypeScript：`pnpm exec tsc --noEmit` → 0 errors
