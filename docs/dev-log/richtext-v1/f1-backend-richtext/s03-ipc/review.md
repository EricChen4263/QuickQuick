---
id: RT1-F1-S03-review
type: review
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 9ee6e7a
acceptance_ids: [RT1-F1-A03]
author: code-reviewer
---

# 审查记录 · IPC 取数透出 html（RT1-F1-S03）

## 审查范围

| 文件 | 改动摘要 |
|---|---|
| `src-tauri/src/ipc/clipboard.rs` | `ClipItemDto` 加 `pub html_content: Option<String>`；`list_clip_items_impl` 映射闭包加 `html_content: r.html_content` |
| `src/ipc/ipc-client.ts` | `ClipItem` 接口加 `htmlContent?: string` |
| `src-tauri/tests/ipc_clipboard.rs` | 新增 `list_clip_items_exposes_html_content_for_richtext`、`list_clip_items_html_null_for_plaintext` 两个测试 |

参照标准：项目规范（AGENTS.md）+ code-standards（函数≤50行 / 注释写"为什么" / 命名 / 安全红线）。

---

## 重点检查判定

### 1. DTO 字段 camelCase 映射正确性（通过）

`ClipItemDto` 已标注 `#[serde(rename_all = "camelCase")]`，`html_content` 序列化为 `htmlContent`；TypeScript `ClipItem` 接口对应字段 `htmlContent?: string`，映射完全吻合。`Option<String>` 在 JSON 序列化为 `null` / 缺失，TypeScript 侧 `?: string` 语义一致，Tauri invoke 透传无歧义。**通过**。

### 2. 映射链路完整性——无遗漏（通过）

链路：`clip_items.html_content`（SQLite TEXT）→ `ClipItemRowWithImage.html_content`（db 层，`row.get(7)`）→ `ClipItemDto.html_content`（IPC 层，`html_content: r.html_content`）→ 前端 `ClipItem.htmlContent`。

`list_items_with_images` 的 SELECT 语句第 8 列（index 7）已正确读取 `ci.html_content`，IPC 层映射闭包已加对应字段，既有字段（`id/content/kind/is_favorite/last_modified_utc/thumbnail_data_url/image_id`）全部保留。链路完整，无遗漏，无回归。**通过**。

### 3. Option / 可选语义正确性（通过）

- Rust 层：`Option<String>` 语义明确——富文本条目持有 html 串，纯文本/图片条目为 `None`，与 db 层 `html_content TEXT` 列语义（NULL=无 html）一致。
- TypeScript 层：`htmlContent?: string` 与 Rust `Option<String>` 对称，消费端只需判断字段是否存在/非空即可，无歧义。**通过**。

### 4. 测试断言非恒真（通过）

**`list_clip_items_exposes_html_content_for_richtext`**：
- 构造具体 html 串 `"<b>hello</b>"`，写入带 html 的 `CapturedItem`，ingest 后从 DTO 列表中 `.find(kind == "richtext").expect(...)` 找到富文本条目（找不到直接 panic，不会假绿），再 `assert_eq!(rich.html_content, Some(html), ...)` 断言具体值。非恒真，杀假阳。**通过**。

**`list_clip_items_html_null_for_plaintext`**：
- `insert_text` 插入纯文本条目（`html: None`），从 DTO 列表中 `.find(kind == "text").expect(...)` 找到后 `assert!(text_dto.html_content.is_none(), ...)`。测试隔离（临时库、独立 `_dir`），kind 过滤确保目标条目正确，非恒真。**通过**。

两个用例覆盖 RT1-F1-A03 的两个等价类（有 html / 无 html），验收标准完整覆盖。

### 5. 注释规范（通过）

- Rust 字段注释：`/// 透出供前端富文本预览与粘贴还原格式。` — 写了"为什么"（预览 + 还原格式），符合"注释写为什么"规范。
- TypeScript 接口注释：`/** 富文本项的 HTML 串；纯文本项与图片项无此字段。 */` — 准确描述字段含义及缺席场景，与已有同组字段注释风格一致。
- 测试函数 doc 注释均标注验收 ID `RT1-F1-A03` 及语义说明，格式与既有测试一致。
- 无 TODO/FIXME，无死代码，无装饰性分隔注释。**通过**。

### 6. 函数长度与代码规范（通过）

`list_clip_items_impl` 原本 ≤ 30 行，新增一行映射后仍远低于 50 行上限。嵌套层级未增加。命名延续既有 snake_case 规范，TS 侧 camelCase，无违规。**通过**。

### 7. 安全考量（通过）

HTML 字符串在 IPC 层原样透传，属于正确的职责划分——IPC 层的职责是取数/序列化，富文本内容的 XSS 防护（sanitize）属于前端渲染层（RT1-F1-S04 或展示组件）的职责。本层不需要也不应该修改 html 内容。**通过**。

---

## 问题列表

**无置信度 ≥80 的 Critical 或 Important 问题。**

以下为置信度 <80 的观察，仅供参考，不阻塞：

| 置信度 | 位置 | 描述 |
|---|---|---|
| 40 | `src-tauri/src/ipc/clipboard.rs:7-10`（模块文档注释） | 模块头部命令清单注释和 DTO 注释（第 23-26 行）未提及新增的 `htmlContent` 字段，文档与实现存在微小偏差。非阻塞，改动量极小。 |

---

## 无其他置信度 ≥80 问题

- camelCase 序列化映射经 serde 宏保证，无手工拼写风险。
- db 层 SELECT 列顺序与 `row.get(index)` 对齐（index 7 = `ci.html_content`），无错位。
- 两个测试均使用临时文件库（`open_tmp_db`），测试间隔离，无共享状态。
- 既有 7 个 `ipc_clipboard_*` 测试用例接口未被修改，无回归面。
- 无魔法字符串、无 `any` 类型、无硬编码安全值。

---

## 审查结论

**通过（APPROVE）。**

三处改动均最小化、有逻辑：Rust DTO 加一字段 + 映射一行，TypeScript 接口加一字段，测试两个用例验证正反两路。链路完整、映射正确、类型语义一致、注释规范、测试断言具体值（非恒真）。符合 code-standards 与项目规范所有强制要求。

---

**VERDICT: APPROVE**

无置信度 ≥80 的 Critical 或 Important 问题。
