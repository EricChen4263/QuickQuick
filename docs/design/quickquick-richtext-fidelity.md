# QuickQuick 剪贴板富文本保真 · 实现方案

> 版本：v1（待批准） · 日期：2026-06-07 · 分支建议：`feat/clipboard-richtext-fidelity`
> 流程：feature-dev 七阶段 · 当前处于 Phase 4（架构方案），**待用户批准后进入 Phase 5 实现**

---

## 一、目标与范围

让 QuickQuick 的剪贴板历史**保真富文本**：复制带格式内容（加粗/颜色/列表/表格/链接等）时，捕获并保存其 HTML，预览区可视化呈现，粘贴 / 复制回系统时还原格式。

### 已冻结的范围决策（用户拍板）

| # | 决策点 | 选定 |
|---|--------|------|
| 1 | 格式范围 | **只 HTML**（不做 RTF）。覆盖浏览器 / Office / 多数富文本源；纯 RTF 源退纯文本 |
| 2 | 还原范围 | **「粘贴到前台」与「复制」按钮都带富文本** |
| 3 | 预览渲染 | **预览区渲染真富文本 + DOMPurify 清洗**；列表行仍纯文本摘要 |
| 4 | 去重语义 | **纯文本相同即去重**（维持现有 `text_hash` 语义）；命中旧行但新来的带 html 而旧行没有，则**补写 html 并升为 richtext** |

### 明确不做（YAGNI）

- ❌ RTF 捕获/还原（arboard 不支持，需各平台原生 + cfg 分支，成本高）
- ❌ HTML 内联远程图片的下载/内嵌、完整 CSS 保真（取决于目标 app 渲染能力）
- ❌ 列表行的富文本缩略渲染（保持纯文本摘要，性能与可读性更好）

---

## 二、关键发现（决定方案复杂度）

**arboard 3.6.1 三平台均已原生实现 HTML 的「读」和「写」**，无需任何平台原生代码、无需新增依赖、无需 `#[cfg(target_os)]` 分支：

| 平台 | 读 HTML | 写 HTML | 实现位置（arboard 源码） |
|------|---------|---------|--------------------------|
| macOS | `string_from_type(NSPasteboardTypeHTML)` | `setString_forType(.., NSPasteboardTypeHTML)` | `platform/osx.rs:212 / 301` |
| Windows | `clipboard_win::raw::get_html("HTML Format")` | `wrap_html` + set（**自动处理 CF_HTML 偏移头**） | `platform/windows.rs:611 / 688` |
| Linux | X11 `text/html` atom / Wayland `text/html` mime | 同 | `platform/linux/{x11,wayland}.rs` |

调用方式：
- 读：`clipboard.get().html().ok()`
- 写：`clipboard.set().html(html, Some(plain_text_alt))`（同时写 HTML 与纯文本兜底）

> 这推翻了"跨平台需手撸 NSPasteboard/CF_HTML/Wayland"的预设——**跨平台 HTML 几乎免费**。

### 现状根因（富文本为何丢失）

数据模型早已预留 `html` 字段，但三处把它截断：

1. **捕获**：`pipeline.rs:155` `ArboardBackend::read()` 中 `html: None` 硬编码（arboard 的便捷方法 `get_text()` 只读纯文本）。
2. **存储**：`db.rs` 的 `clip_items` 表**无 html 列**；`ingest()`（`db.rs:471`）INSERT 只写 6 字段、`kind` 写死 `'text'`。
3. **还原**：`ipc/system.rs:141` `fetch_paste_item` 构造 `CapturedItem{html:None}`；`macos_paste.rs:91` `write_with_marker` 只 `set_text`。

而前端**早已为 richtext 预留**：`kind: "text" | "richtext" | "image"` 贯穿搜索/过滤/历史（`src/panels/history/{search,filter}.ts`），连"富文本"筛选 tab 都在——本方案是接通这条已铺好的轨道。

---

## 三、端到端架构（数据流）

```
复制带格式内容
  │
  ▼  [捕获] pipeline.rs ArboardBackend
     change_count(): compute_composite_hash 纳入 html（使"同文本但新增了格式"能被检测）
     read(): html = clipboard.get().html().ok()
  │
  ▼  snapshot_to_clips（clipboard.rs，已正确透传 html，无需改）
  │
  ▼  [存储] db.rs ingest
     新增列 html_content TEXT；kind = html ? 'richtext' : 'text'
     去重：text_hash 仍只对纯文本（决策4）
       命中且旧行无 html、新行有 html → UPDATE 补写 html + 升 kind，再 bump
  │
  ▼  [取数] ipc/clipboard.rs ClipItemDto + db 查询
     DTO 加 html_content（serde camelCase → 前端 htmlContent）
  │
  ├──▼ [预览] ClipPreview.tsx / PopoverPreview.tsx
  │     kind==='richtext' && htmlContent → DOMPurify.sanitize → dangerouslySetInnerHTML
  │     列表行：仍渲染纯文本 content 摘要
  │
  ├──▼ [粘贴到前台] ipc/system.rs fetch_paste_item 取 html
  │     → macos_paste.rs / fallback write_with_marker
  │       item.html ? clipboard.set().html(html, Some(text)) : set_text
  │
  └──▼ [复制按钮] 新增 IPC 命令 copy_clip_to_clipboard(id)
        取 text+html → arboard set().html（替代前端 navigator.clipboard.writeText）
```

---

## 四、改动清单（按 TDD 任务顺序）

> 每个任务红-绿-重构：先写失败测试 → 实现 → 重构。后端 Rust 用 `coder`(Opus)，测试 `tester`(Sonnet) 动态证伪，规范审查 `code-reviewer`(Sonnet)。

### 任务 1 · 存储层：html 列 + ingest + 查询

- **`db.rs ensure_schema`**：建表 SQL 加 `html_content TEXT`；对存量库做幂等迁移——`PRAGMA table_info(clip_items)` 检测无 `html_content` 时执行 `ALTER TABLE clip_items ADD COLUMN html_content TEXT`（SQLite 无 `ADD COLUMN IF NOT EXISTS`，用 table_info 守卫；新列默认 NULL，整库 SQLCipher 加密自动覆盖）。
- **`db.rs ingest`**：
  - 入参 `CapturedItem` 已含 `html`。
  - INSERT 增加 `html_content`，`kind` 改为 `if item.html.is_some() { "richtext" } else { "text" }`。
  - 去重命中分支：若 `existing` 行 `html_content IS NULL` 且 `item.html.is_some()`，先 `UPDATE clip_items SET html_content=?, kind='richtext' WHERE id=?` 再 `bump_to_top`（决策4 的"补写"）。
  - `text_hash` 不变（只对 `item.text`）。
- **`db.rs` 查询**：`ClipItemRow` / `ClipItemRowWithImage` 加 `html_content: Option<String>`；`list_items_with_images` / `list_items_full` 等 SELECT 补 `html_content` 列。
- **测试**：① 富文本 ingest→查询 roundtrip；② 存量库（无列）迁移后可写读；③ 同文本先纯文本后带 html → 同一行被补写 html + 升 richtext（不新增行）；④ 纯文本去重语义不变。

### 任务 2 · 捕获层：读 HTML + 变化检测

- **`pipeline.rs ArboardBackend::read()`**：`html: None` → `html: cb.get().html().ok()`（注意 `get()` 是一次性 builder，与 `get_text()` 分别调用）。
- **`pipeline.rs compute_composite_hash`**：在 text、image 之外，把 `cb.get().html().ok()` 的字节也拼入哈希（加独立分隔符），使"同纯文本但新增/变更了 html"能触发 `change_count` 递增 → 进而走 ingest 补写。保持顺序固定（确定性）。
- **测试**：纯逻辑可测的 `snapshot_to_clips`/哈希组合；arboard GUI 读取沿用项目惯例不写联网/GUI 自动化测试，但对 hash 函数做"html 不同 → hash 不同"的单测。

### 任务 3 · IPC 取数：DTO + 前端类型

- **`ipc/clipboard.rs ClipItemDto`**：加 `pub html_content: Option<String>`（已是 `rename_all=camelCase`，前端见 `htmlContent`）；`list_clip_items_impl` 映射 `r.html_content`。
- **`src/ipc/ipc-client.ts ClipItem`**：加 `htmlContent?: string`。
- **测试**：`list_clip_items_impl` 对富文本行返回 html_content；纯文本行为 `null`。

### 任务 4 · 还原（粘贴 + 复制）

- **`ipc/system.rs fetch_paste_item`**：SELECT 增加 `html_content`，构造 `CapturedItem{ text, html }`。
- **`macos_paste.rs write_with_marker`**（`macos_impl` 与 `fallback_impl` 两处）：
  ```
  match &item.html {
    Some(h) => clipboard.set().html(h.clone(), Some(item.text.clone())),
    None    => clipboard.set_text(item.text.clone()),
  }
  ```
  （`paste.rs` 的 `PasteBackend` trait 签名已收 `&CapturedItem`，**无需改接口**。）
- **复制按钮（决策2）**：新增 IPC 命令 `copy_clip_to_clipboard(id)`——后端取 text+html，用 arboard `set().html` 写系统剪贴板；前端 `ClipPreview` / `ClipPopoverApp` 的"复制"改调此命令（替代 `browser-api.ts` 的 `navigator.clipboard.writeText`，因 Tauri WebView 对 `ClipboardItem` 多 MIME 写入支持不稳，走 IPC 更可靠且与粘贴路径同源）。
- **测试**：`fetch_paste_item` 取出 html；`copy_clip_to_clipboard` 纯函数部分（取数+组装）可测，arboard 写入沿用惯例不做 GUI 自动化。

### 任务 5 · 前端渲染 + 安全清洗（决策3）

- **依赖**：新增 `dompurify`（+ `@types/dompurify`）。
- **`ClipPreview.tsx`**：`kind==='richtext' && htmlContent` 时 `dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(htmlContent) }}`；否则维持 `{item.content}` 纯文本。
- **`clip-popover/PopoverPreview.tsx`**：同样的富文本分支。
- **列表行 `ClipItemRow.tsx`**：不变（继续显示纯文本 `content` 摘要）。
- **安全**：DOMPurify 默认配置即移除 `<script>`、`on*` 事件属性、`javascript:` URL 等；剪贴板内容来自任意外部 app，**必须清洗后才入 DOM**。同时确认 Tauri 的 CSP 不会因内联样式破坏渲染（必要时为预览容器放宽 style 白名单，但不放开 script）。
- **测试**：`<script>` / `onerror` 注入被 DOMPurify 剥离；正常富文本（加粗/表格）保留；纯文本条目不受影响。

---

## 五、安全（XSS 红线）

引入 `dangerouslySetInnerHTML` 是本方案唯一显著风险面。硬约束：

1. **任何 html 入 DOM 前必须 `DOMPurify.sanitize`**，无例外、无"信任来源"豁免（剪贴板来源不可信）。
2. 清洗白名单只放安全的展示标签/属性，**剥离** `<script>`、事件处理器、`javascript:`/`data:`(非图片) URL、`<iframe>`/`<object>` 等。
3. 后端**原样保存**未清洗 html（保真），清洗只发生在**渲染层**——保证"粘贴/复制回去"拿到的是用户原始格式，而非被前端清洗过的残缺版。
4. CSP 复核：预览渲染富文本不得放开 `script-src`。

---

## 六、测试策略（动态证伪，硬门禁）

- Rust 单测：迁移幂等、ingest/补写/去重、fetch_paste_item、hash 区分 html。
- 前端单测：DOMPurify 剥离恶意标签、保留正常格式、纯文本回退。
- `tester` 子 agent 做命中校验 + 变异 sanity（如把 sanitize 调用注释掉应导致安全测试失败；把 html 列读取改成常量应导致 roundtrip 失败）。
- 真机视觉验证：受屏幕录制权限限制，**交用户截图确认**（见记忆 `quickquick-gui-verify-env`）。
- 全量校验脚本（若有 `verify_all.sh`）须通过；lint / `tsc` / `cargo check` 通过、无遗留 TODO。

---

## 七、风险与回退

| 风险 | 应对 |
|------|------|
| 存量加密库加列失败 | `PRAGMA table_info` 守卫 + `ADD COLUMN`（NULL 默认）是 SQLite 低风险操作；迁移失败仅记录日志、退纯文本，不阻断启动 |
| 某些源的 HTML 含大量内联样式/碎片（如 Word 的 CF_HTML 片段） | 保真保存；预览靠 DOMPurify 容错渲染；不追求像素级还原 |
| 粘贴到不支持 HTML 的目标 app | arboard `set().html(h, Some(text))` 已带纯文本兜底，目标 app 自动取纯文本 |
| 前端 XSS | 见第五节硬约束 |
| 与并发后端工作冲突 | 已确认对侧 ECDICT 重构收尾、`html` 链路与其不重叠；开工前 `git pull`/rebase 同步 |

---

## 八、落地顺序与交付物

1. 任务 1（存储）→ 2（捕获）→ 3（IPC）→ 4（还原）→ 5（前端渲染）逐个 TDD 闭环，每个任务测试通过方进下一个。
2. 完成后更新本方案文档（如实现与设计有偏差）。
3. 双份文档保持一致（本 `.md` 为权威，`.html` 为可视化）。

> **本方案待批准。** 批准后我按 feature-dev Phase 5 让 `coder` 开始任务 1。
