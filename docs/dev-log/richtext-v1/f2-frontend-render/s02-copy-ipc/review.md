---
id: RT1-F2-S02-review
type: review
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 4e0caa3
acceptance_ids: [RT1-F2-A02]
author: code-reviewer
---

# 审查报告：RT1-F2-S02 复制按钮改调 IPC

## 审查范围

- `src/panels/clipboard/ClipPreview.tsx`（handleCopy 改调 copyClipToClipboard）
- `src/clip-popover/ClipPopoverApp.tsx`（Alt+Enter 改调 copyClipToClipboard）
- `src/ipc/ipc-client.ts`（ClipItem.kind 收窄为字面量联合）
- `src/panels/clipboard/clipboard-page.test.tsx`（MOCK_ITEMS 加 ClipItem[] 类型标注）
- `src/panels/clipboard/clip-preview-actions.test.tsx`（新增）
- `src/clip-popover/clip-popover-actions.test.tsx`（新增）

## Critical 级问题

无。

## Important 级问题

### I-1：ClipboardPage.tsx:278 过时注释未随改动同步清理（Important · 置信度 95）

- **文件**：`src/panels/clipboard/ClipboardPage.tsx`，第 278 行
- **证据**：注释内容为 `/* 复制逻辑在 ClipPreview 内部调用 writeToClipboard 完成 */`；实际改动已将 `ClipPreview` 内部的复制实现从 `writeToClipboard` 换为 `copyClipToClipboard(item.id)`，注释描述的函数名已失效，属**改名后遗留的说谎注释**。hints TV1-RETRO-1 明确要求此类改名遗留旧引用必须在同批清理。
- **影响**：注释内容与代码实际行为相悖，误导维护者，属项目规范「注释写为什么、不撒谎」违规。
- **建议修复**：将该注释更新为描述当前行为，例如改为 `/* 复制走后端 IPC；ClipPreview 内部调用 copyClipToClipboard(item.id) 完成 */`，或移除（此处 onCopy 为空回调，注释已无实质作用）。
- **是否要求本小功能内修复**：**是**，属 hints 明确要求的同批清理项，不可延后。

### I-2：clip-popover-actions.test.tsx mock 写法使用 `as ReturnType<typeof vi.fn>` 类型断言（Important · 置信度 82）

- **文件**：`src/clip-popover/clip-popover-actions.test.tsx`，第 36-40 行
- **证据**：文件中对 `listClipItems`、`pasteToFront`、`writeToClipboard`、`hideAndReturnFocus`、`copyClipToClipboard` 均使用 `as ReturnType<typeof vi.fn>` 进行类型断言，绕过了 TypeScript 的 mock 类型校验。同批新增的 `clip-preview-actions.test.tsx` 则统一使用 `vi.mocked(fn)` 写法（第 26-28 行），两者风格不一致。
- **影响**：`as ReturnType<typeof vi.fn>` 是不安全转型，等价于将类型断言为 `any`，若被 mock 的函数签名变更，此处不会报编译错误，违反「禁 any」规范（`frontend.md` 与 `code-standards`）。`vi.mocked()` 是 Vitest 提供的类型安全官方写法，应统一使用。
- **建议修复**：将 36-40 行替换为 `vi.mocked()` 写法，与 `clip-preview-actions.test.tsx` 保持一致。例：
  ```ts
  const mockListClipItems = vi.mocked(listClipItems);
  const mockPasteToFront = vi.mocked(pasteToFront);
  const mockWriteToClipboard = vi.mocked(writeToClipboard);
  const mockHideAndReturnFocus = vi.mocked(hideAndReturnFocus);
  const mockCopyClipToClipboard = vi.mocked(copyClipToClipboard);
  ```

## 合规验证结论

### 正确性

| 检查点 | 结论 |
|--------|------|
| ClipPreview 复制改调 copyClipToClipboard(item.id) | 通过 |
| ClipPopoverApp Alt+Enter 改调 copyClipToClipboard(selectedItem.id) | 通过 |
| 纯文本复制不回归（后端 html=None 走 set_text） | 通过，测试 plaintext_copy_not_regressed 覆盖 |
| 图片条目 no-op 守卫保留（kind==="image" return） | 通过，测试覆盖 Alt+Enter 图片 no-op |
| 错误处理风格一致（catch 静默 / console.error） | 通过 |
| 翻译页 writeToClipboard 未被误删 | 通过，TranslatePage.tsx 与 TransPopoverApp.tsx 均保留 |

### kind 收窄（I-2 前序项，已完成）

| 检查点 | 结论 |
|--------|------|
| ClipItem.kind 从 string 收窄为字面量联合 | 通过，ipc-client.ts 第 8 行完成 |
| clipboard-page.test.tsx MOCK_ITEMS 加类型标注 | 通过 |
| Kind_LABEL Record 键类型与收窄后对齐 | 通过（ClipPreview.tsx 第 39 行 Record<ClipItem["kind"], string>） |

### 旧引用清理

| 检查点 | 结论 |
|--------|------|
| ClipPreview.tsx 中 writeToClipboard import 已移除 | 通过 |
| ClipPopoverApp.tsx 中 writeToClipboard import 已移除（业务代码） | 通过 |
| ClipboardPage.tsx:278 过时注释 | **未清理，必改项 I-1** |
| 全仓 grep writeToClipboard：translate/trans-popover 路径为合法使用 | 通过，均属翻译复制路径，非剪贴板复制旧引用 |

### 注释规范

| 检查点 | 结论 |
|--------|------|
| 新注释写"为什么"（IPC 保真富文本） | 通过 |
| 无 TODO/FIXME/死代码 | 通过 |
| 无 any（业务代码） | 通过 |
| 测试 mock 无 any 等价写法 | **clip-popover-actions.test.tsx 有 as 断言，见 I-2** |

## 必须修项（打回要求）

1. **I-1（必须修，本小功能内）**：`src/panels/clipboard/ClipboardPage.tsx:278` 过时注释 `writeToClipboard` 须改为反映当前实现的注释，或移除。hints TV1-RETRO-1 明确要求同批清理，不得延后。

2. **I-2（建议修，可本小功能内完成）**：`src/clip-popover/clip-popover-actions.test.tsx` 第 36-40 行的 `as ReturnType<typeof vi.fn>` 替换为 `vi.mocked()`，消除 any 等价写法，与同批测试风格统一。

## 判决

**BLOCK**：存在 1 个必须修 Critical 等效项（I-1，hints 明确要求同批清理的说谎注释），修复后可复审。

---

## 复审（2026-06-07）

coder 已按必改项完成修复，本次对两项问题逐一核查。

### I-1 复审：ClipboardPage.tsx:278 注释

当前行内容：

```
onCopy={(_item) => { /* 复制逻辑在 ClipPreview 内部调用 copyClipToClipboard(id) 完成，带富文本保真 */ }}
```

函数名已由 `writeToClipboard` 更新为 `copyClipToClipboard(id)`，并补充「带富文本保真」语义，注释内容与代码实际行为一致。**I-1 已解决。**

### I-2 复审：clip-popover-actions.test.tsx mock 写法

diff 确认第 36-40 行已全部替换为 `vi.mocked(fn)` 写法：

```ts
const mockListClipItems = vi.mocked(listClipItems);
const mockPasteToFront = vi.mocked(pasteToFront);
const mockWriteToClipboard = vi.mocked(writeToClipboard);
const mockHideAndReturnFocus = vi.mocked(hideAndReturnFocus);
const mockCopyClipToClipboard = vi.mocked(copyClipToClipboard);
```

无 `as ReturnType<typeof vi.fn>` 残留，与 `clip-preview-actions.test.tsx` 风格统一。**I-2 已解决。**

### as const 连带改动核查

`kind: "text" as const` 与 `kind: "image" as const` 为纯类型收窄标注，字面量值 `"text"` / `"image"` 本身未改变，测试数据语义不变，无逻辑变化。同时核查 `copyClipToClipboard` 补充到 mock 工厂（vi.mock 第 9 行）、import（第 33 行）、beforeEach（第 68 行）均完整，测试断言正确从 `writeToClipboard("first item")` 改为 `copyClipToClipboard("id1")`，并新增 `expect(mockWriteToClipboard).not.toHaveBeenCalled()` 回归守卫。无新问题引入。

### 复审判决

**APPROVE**：I-1、I-2 均已正确解决，as const 连带改动为纯类型标注无逻辑变化，无新问题引入。本小功能 RT1-F2-S02 审查通过。
