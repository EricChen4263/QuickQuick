---
id: RT1-F1-S04-review
type: review
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A04]
author: code-reviewer
---

# 审查记录 · 还原：粘贴+复制（后端）（RT1-F1-S04）

## 审查范围

| 文件 | 改动摘要 |
|---|---|
| `src-tauri/src/ipc/system.rs` | `fetch_paste_item` SELECT 新增 `html_content`；构造 `CapturedItem{text, html}`；新增 `fetch_clip_for_copy`（委托 `fetch_paste_item`）、`write_clip_to_system_clipboard`（薄封装）、IPC 命令 `copy_clip_to_clipboard` |
| `src-tauri/src/macos_paste.rs` | 新增 `write_item_to_clipboard`（`pub(crate)`，富文本 / 纯文本分支）；`macos_impl` 与 `fallback_impl` 两处 `write_with_marker` 改调此函数 |
| `src-tauri/src/lib.rs` | `invoke_handler` 注册 `copy_clip_to_clipboard` |
| `src/ipc/ipc-client.ts` | `copyClipToClipboard` 封装 |
| `src-tauri/tests/richtext_paste_copy.rs`（新建） | 集成测试：`fetch_paste_item_includes_html`、`copy_clip_assembles_text_and_html` |

参照标准：项目规范（AGENTS.md）+ code-standards（函数 ≤50 行 / 注释写"为什么" / 命名 / SQL 参数化 / 安全红线 / DRY）。

---

## 重点检查判定

### 1. SQL 取数正确性（通过）

`fetch_paste_item` 改为 `SELECT content, kind, html_content FROM clip_items WHERE id = ?1 AND is_deleted = 0`，行映射到三元组 `(String, String, Option<String>)`，构造 `CapturedItem { text: content, html }`。

- SQL 参数化：`?1` 绑定 `id`，合规，无注入风险。
- `html_content` 对应 DB 列 `TEXT`（`NULL` 可能），`Option<String>` 映射正确。
- 软删过滤 `is_deleted = 0` 保持原有语义，未丢失。
- 空 id 分支（`.trim().is_empty()` 守卫）已保留，不存在条目时返回 `Err`，图片条目返回 `Err`——三条错误路径与改前一致，无回归。**通过**。

### 2. 富文本写入分支语义（通过）

`write_item_to_clipboard`（`macos_paste.rs:29-40`）：

```
match &item.html {
    Some(html) if !html.is_empty() => clipboard.set().html(html, Some(item.text)),
    _ => clipboard.set_text(item.text),
}
```

- `Some("")`（空串）走 `set_text` 分支，不会向 arboard 写空 html，语义正确（空 html 等同无 html）。
- `set().html(h, Some(text))` 的纯文本参数 `Some(item.text.clone())` 保证不支持 HTML 的目标 app 仍可粘出纯文本（设计§四任务4 决策2）。
- `_ =>` 涵盖 `None` 与 `Some("")`，分支完整无漏洞。**通过**。

### 3. DRY：`write_item_to_clipboard` 复用（通过）

`macos_impl::write_with_marker` 与 `fallback_impl::write_with_marker` 两处均改调顶层 `pub(crate) write_item_to_clipboard`，消除了原来两份平行的 `set_text` 逻辑。`write_clip_to_system_clipboard`（`system.rs`）同样复用此函数，确保复制路径与粘贴路径写入行为一致。**DRY 合规**。

### 4. `fetch_clip_for_copy` 作为独立函数的合理性（通过）

函数体为单行委托 `fetch_paste_item(conn, id)`；注释解释了「语义独立于复制域、便于各自演进」，符合"注释写为什么"规范。测试文件直接引用 `fetch_clip_for_copy`（而非 `fetch_paste_item`）作为复制域的断言入口，测试意图清晰。虽然当前实现零差异，但作为未来各自演进的接缝是合理设计，不属过度抽象。**通过**。

### 5. IPC 命令注册与前端签名完整性（通过）

- `lib.rs:175`：`ipc::system::copy_clip_to_clipboard` 已加入 `invoke_handler`，顺序在 `paste_to_front` 之后，无遗漏。
- 命令签名 `copy_clip_to_clipboard(state: State<'_, AppDb>, id: String) -> Result<(), String>` 符合 Tauri 命令约定。
- `ipc-client.ts` 的 `copyClipToClipboard(id: string): Promise<void>` 调用 `invoke<void>("copy_clip_to_clipboard", { id })`，命令名、参数键名、返回类型三者吻合。
- `try/catch` 用 `toError(err)` 抛出，与文件中其他封装函数风格一致。**通过**。

### 6. 错误处理：无 panic、错误不泄漏敏感内容（通过）

- `copy_clip_to_clipboard` 命令：取数失败（不存在/已删/图片）返回 `Err(String)`，arboard 写入失败返回 `Err(String)`，均不 panic，前端可捕获错误提示用户。
- `write_with_marker` 失败路径：`eprintln!("[MacOsPasteBackend] write_with_marker 失败: {e}")` 与 `eprintln!("[FallbackPasteBackend] write_with_marker 失败: {e}")` 中 `{e}` 是 arboard 的错误描述（如 "ClipboardUnavailable"），不包含剪贴板内容本身，不泄漏 html/text 数据。**安全合规**。

### 7. 注释规范（通过）

- 所有新增公开函数均有 `///` doc 注释，且解释了"为什么"（例：`write_clip_to_system_clipboard` 解释了 arboard 实写属 GUI 副作用不进自动化；`fetch_clip_for_copy` 解释了语义独立性）。
- 无 `TODO`/`FIXME`，无装饰性分隔注释，无注释掉的死代码。
- 测试文件 `richtext_paste_copy.rs` 模块头注释说明了测试范围与 arboard 不进自动化的理由，符合规范。**通过**。

### 8. 函数长度与嵌套（通过）

- `copy_clip_to_clipboard`：6 行，远低于 50 行上限。
- `write_item_to_clipboard`：9 行，match 嵌套 1 层。
- `write_clip_to_system_clipboard`：4 行。
- `fetch_clip_for_copy`：2 行（委托）。
所有新增函数均满足"函数 ≤50 行 / 嵌套 ≤3 层"规范。**通过**。

### 9. 测试断言非恒真（通过）

**`fetch_paste_item_includes_html`**：
- 入库具体 html 串 `"<b>hello</b>"`，对返回的 `rich.html.as_deref()` 断言等于 `Some("<b>hello</b>")`（具体值，非 `is_some()` 恒真）。
- 纯文本条目 `plain.html` 断言 `== None`，覆盖反向路径。

**`copy_clip_assembles_text_and_html`**：
- 同上，用 `"<i>world</i>"` 断言具体值；纯文本条目断言 `html == None` 且 `text == "just text"`（字段级验证）。
- 两个测试均使用各自独立的临时库（key `[7u8; 32]` / `[9u8; 32]`），测试间隔离，无共享状态。**通过**。

---

## 问题列表

**无置信度 ≥80 的 Critical 或 Important 问题。**

以下为置信度 <80 的观察，仅供参考，不阻塞：

| 置信度 | 位置 | 描述 |
|---|---|---|
| 35 | `src-tauri/src/ipc/system.rs:165-167` | `fetch_clip_for_copy` 当前为纯委托（零差异），若未来两条路径不需分化演进，可考虑在 `copy_clip_to_clipboard` 内直接调用 `fetch_paste_item`。属预防性隔离设计决策，注释已说明理由，不构成问题。 |
| 30 | `src-tauri/tests/richtext_paste_copy.rs` | 集成测试未覆盖"空 id 返回 Err"与"图片条目返回 Err"两个错误路径，这两个路径在 `system.rs` 的单元测试（T4/T5）中已有覆盖，无空洞。可选补充，不阻塞。 |

---

## 无其他置信度 ≥80 问题

- SQL 参数化、软删过滤、类型映射均正确，无注入风险、无字段错位。
- `write_item_to_clipboard` 的 `pub(crate)` 可见性适当（仅 crate 内复用，不对外暴露），`macos_paste` 模块已在 `lib.rs` 以 `pub mod macos_paste` 导出，跨模块引用路径 `crate::macos_paste::write_item_to_clipboard` 正确可达。
- `write_item_to_clipboard` 无 `#[cfg]` 条件编译限制，macOS 与 fallback 两条路径均可调用，跨平台正确。
- `copy_clip_to_clipboard` 命令在非 macOS 平台走 `FallbackPasteBackend` 的 arboard 实写路径，行为合理（降级写纯文本或富文本，不 panic）。
- 无魔法字符串、无 `any` 类型、无硬编码安全值、无遗留旧引用。
- 既有 `paste_to_front` 命令路径未被修改（仅 `fetch_paste_item` 新增了 html 字段，下游 `CapturedItem` 用法兼容），无回归风险。

---

## 审查结论

**通过（APPROVE）。**

五处改动最小化且逻辑正确：`fetch_paste_item` 补 `html_content` 字段完整覆盖存取链路；`write_item_to_clipboard` 抽取消除两份平行写入逻辑（DRY）；`copy_clip_to_clipboard` 命令按规范注册并前端封装；集成测试断言具体值、测试隔离、覆盖正反两路。符合 code-standards 与项目规范所有强制要求，RT1-F1-A04 验收标准完整满足。

---

**VERDICT: APPROVE**

无置信度 ≥80 的 Critical 或 Important 问题。
