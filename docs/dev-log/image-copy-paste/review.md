---
id: image-copy-paste-review
type: review
level: 小功能
parent: image-copy-paste
children: []
created: 2026-06-11T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 图片复制 + 一键粘贴到前台

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/clipboard.rs` | 新增 `ClipboardPayload` 枚举 + `png_to_rgba` 纯函数 |
| `src-tauri/src/paste.rs` | `PasteBackend` trait 新增 `write_image` 方法 |
| `src-tauri/src/macos_paste.rs` | `write_image_to_clipboard` 助手 + 两后端实现 `write_image` |
| `src-tauri/src/ipc/system.rs` | `fetch_paste_item`/`fetch_clip_for_copy` 改返 `ClipboardPayload`；image 取数、写回、粘贴编排分流 |
| `src/clip-popover/ClipPopoverApp.tsx` | 移除图片 Alt+Enter 守卫，图片走 `copyClipToClipboard` |
| `src-tauri/tests/capture_image.rs` | 新增 `png_to_rgba` 往返 + 非法字节测试 |
| `src-tauri/tests/paste.rs` | `FakePasteBackend` 补 `write_image` no-op |
| `src-tauri/tests/onboarding.rs` | 同上 |
| `src-tauri/tests/richtext_paste_copy.rs` | 适配 `ClipboardPayload`，新增 `expect_text` 辅助 |
| `src/clip-popover/clip-popover-actions.test.tsx` | 图片 Alt+Enter 测试改为正向断言 |

参照标准：项目规范（paste.rs `write_and_confirm` 写入确认契约、code-general.md 排序/取数确定性规则）+ code-standards skill（函数 ≤50行/嵌套 ≤3层/参数 ≤5）。

---

## Critical 级问题

### C1 · `paste_image_with_backend`：write_image 失败后仍无条件 send_paste

**severity: Critical · confidence: 92**
**文件：`src-tauri/src/ipc/system.rs:589-596`**

```rust
fn paste_image_with_backend(...) -> String {
    backend.write_image(width, height, rgba);   // ← 失败仅 eprintln，返回 void
    if probe.is_trusted() {
        hide_and_restore_focus(app, target_pid);
        backend.send_paste();                   // ← 写入失败时仍被调用
        "full_paste".to_string()
    } else {
        "write_back_only".to_string()
    }
}
```

`PasteBackend::write_image` 的 trait 设计为 `fn write_image(&mut self, ...) `（无返回值），失败时实现层只做 `eprintln!` 降级、不 panic——调用方无法感知写入是否成功。当 arboard `set_image` 失败（如剪贴板被其他进程占用、系统资源耗尽），图片实际未写入剪贴板，但代码判断 `probe.is_trusted()` 后仍然执行 `hide_and_restore_focus` + `send_paste`，向前台 App 发出 Cmd+V。此时剪贴板里是旧内容（上一条历史或空），用户触发粘贴实际粘贴的是错误内容，且操作无任何错误反馈。

对比文本路径：文本走 `write_and_confirm`（轮询 changeCount 确认写入成功），失败（Timeout）则直接返回 `"write_back_only"` 不 send_paste——两条路径在"写入确认"这一关键环节的处理完全不对称。

**建议修复（两选一）：**

方案 A——`write_image` 改为返回 `bool`，调用方据此决定是否 send_paste：
```rust
// trait 改签名
fn write_image(&mut self, width: usize, height: usize, rgba: &[u8]) -> bool;

// 实现层
fn write_image(&mut self, width: usize, height: usize, rgba: &[u8]) -> bool {
    if let Err(e) = write_image_to_clipboard(&mut self.clipboard, width, height, rgba) {
        eprintln!("[MacOsPasteBackend] write_image 失败: {e}");
        false
    } else {
        true
    }
}

// paste_image_with_backend
fn paste_image_with_backend(...) -> String {
    let ok = backend.write_image(width, height, rgba);
    if ok && probe.is_trusted() {
        hide_and_restore_focus(app, target_pid);
        backend.send_paste();
        "full_paste".to_string()
    } else {
        "write_back_only".to_string()
    }
}
```

方案 B——与文本路径对齐，用 changeCount 轮询确认（成本更高但语义统一）：在 write_image 后轮询 `change_count` 是否递增，不增则不 send_paste。

---

## Important 级问题

### I1 · `fetch_image_payload` SQL 无 LIMIT 1，多行时取非确定性第一行

**severity: Important · confidence: 82**
**文件：`src-tauri/src/ipc/system.rs:169-176`**

```rust
conn.query_row(
    "SELECT original FROM clip_images WHERE clip_item_id = ?1 AND is_deleted = 0",
    rusqlite::params![clip_item_id],
    |row| row.get::<_, Option<Vec<u8>>>(0),
)
```

`clip_images.clip_item_id` 列在 schema 上**无 UNIQUE 约束**（`src-tauri/src/db.rs:763`），当前入库逻辑（`ingest_image_as_clip`）保证了 1:1 关系，但 SQL 层面没有任何防护。

项目规范（`code-general.md`）明确要求"排序/取数查询 ORDER BY 必须确定性"，此处无 `ORDER BY` 且无 `LIMIT 1`，多行场景返回的是数据库随机选取的第一行（rusqlite `query_row` 底层只调一次 `rows.next()`，无多行报错）。若未来 bug 或迁移导致一个 `clip_item_id` 关联多行 `clip_images`，取到的原图可能不是最新那张，且无任何报错。

**建议修复：** 加 `ORDER BY last_modified_utc DESC LIMIT 1`，明确"取最近修改的原图行"：
```sql
SELECT original FROM clip_images
WHERE clip_item_id = ?1 AND is_deleted = 0
ORDER BY last_modified_utc DESC LIMIT 1
```

### I2 · `write_image_to_clipboard` 中 `Cow::Owned(rgba.to_vec())` 造成不必要克隆

**severity: Important · confidence: 80**
**文件：`src-tauri/src/macos_paste.rs:60`**

```rust
bytes: Cow::Owned(rgba.to_vec()),
```

入参 `rgba: &[u8]`，`arboard::ImageData.bytes` 类型为 `Cow<'_, [u8]>`。此处本可用 `Cow::Borrowed(rgba)` 直接借用，避免在大图场景（每 4×W×H 字节）做一次额外的堆拷贝。注释也说"用 `Cow::Owned` 转移所有权避免额外约束"，但 `Cow::Borrowed` 同样满足生命周期要求（`set_image` 调用结束后 `ImageData` 即丢弃，借用在调用期间保持有效）。

**建议修复：**
```rust
bytes: Cow::Borrowed(rgba),
```

---

## 通过核查项

- **`png_to_rgba` 正确性**：`image::load_from_memory` 解码 + `.to_rgba8()` 转换，格式为 RGBA 行优先——与 arboard `ImageData` 要求（RGBA packed bytes）一致；宽高从 `rgba8` 对象取，无手动计算错位风险；失败返回 `Err(String)` 不 panic。测试 `png_to_rgba_roundtrips_known_pixels`（含半透明 alpha 128）和 `png_to_rgba_invalid_bytes_returns_err` 充分覆盖，断言锚定具体值，无捷径模式。

- **`fetch_image_payload` 空/无原图路径**：`.optional().map_err(DbError::Sqlite)?.flatten()` + `.filter(|bytes| !bytes.is_empty())` 正确处理"无关联行"（`optional` 返回 `None`）和"空 BLOB 降级"两种情况，统一转 `Err("图片原图已不可用…")`，不会把空图写入剪贴板。测试 T5b/T5c 正向 + 负向覆盖，断言具体（宽高值、错误信息子串），质量合格。

- **函数规模与嵌套**：`fetch_image_payload`（21行/嵌套2）、`write_image_to_clipboard`（13行）、`paste_text_with_backend`（14行）、`paste_image_with_backend`（14行）——各函数均 ≤50行、嵌套 ≤3层，符合规范。`#[allow(clippy::too_many_arguments)]` 作用于 `paste_image_with_backend`（7参）确实超出5参上限，但7个参数均是由上层 match 解构后逐一传入（无法在类型系统层面直接传 `ClipboardPayload::Image` 变体）的必要参数，整合为结构体需新增仅在此函数存在的单次 use struct，属正当豁免，注释理由充分。

- **去重 / 重复捕获**：图片粘贴不套 marker 回读确认（macOS 写图后系统重编码为 TIFF，marker 机制对图片无意义），去重依赖 `image_hash`。`ingest_image_as_clip` 已通过 FNV-1a 字节哈希判重（`image_hash` 字段），paste 后的自捕由哈希命中 bump 处理，语义正确。

- **安全面**：图片字节通过 arboard 写入系统剪贴板（OS 管理），无额外落盘或日志打印；`eprintln!` 仅打印 arboard 错误字符串，不含像素数据。无新的注入面。

- **前端改动**：移除 `if (selectedItem.kind === "image") return;` 守卫后，图片条目 Alt+Enter 走 `copyClipToClipboard`，测试由"验证某事不发生"改为正向断言 `toHaveBeenCalledWith("id-img")` + `toHaveBeenCalled()`，测试语义与行为变更一致，断言质量合格，无捷径模式。

---

## 测试质量评估

Rust 侧新测试（`capture_image.rs` 中 `png_to_rgba_roundtrips_known_pixels`/`png_to_rgba_invalid_bytes_returns_err`，`system.rs` 中 T5b/T5c）断言全部锚定具体值（宽高 2/1、逐字节 RGBA 相等、错误信息子串），无 `toBeDefined` / `is_some()` 等弱判别，无硬编码循环论证问题。

C1 问题（write_image 失败后 send_paste）当前测试未覆盖：`FakeBackend` 的 `write_image` 是 no-op，测试无法检测到"写入失败但 send_paste 仍被调"的场景——这是 Critical 问题在测试层面的覆盖缺口，应随 C1 修复一并补测（在 write_image 失败时 `send_paste_called` 应为 false）。

---

## 必改项（打回清单）

1. **C1（必改）**：`paste_image_with_backend` 在 `write_image` 失败时仍 send_paste，修复方式参见上文 C1 修复方案 A（推荐）。随修复一并补测 write_image 失败 → send_paste 不被调用的用例。

---

## 审查结论

存在 1 个 Critical 问题（C1：图片写剪贴板失败时仍发 Cmd+V，导致错误粘贴）。

**VERDICT: BLOCK**

`severity(Critical) · confidence(92) · src-tauri/src/ipc/system.rs:589-596 · write_image 失败仅 eprintln 无返回值，paste_image_with_backend 无法感知失败，trusted=true 时仍执行 send_paste，导致剪贴板为旧内容时向前台 App 注入错误粘贴；对比文本路径 write_and_confirm 有 changeCount 轮询保护完全不对称 · 将 write_image 改为返回 bool，失败时 paste_image_with_backend 跳过 send_paste 直接返回 write_back_only`

---

## 复审记录 · 2026-06-11

### 复审范围

核查 coder 对三项打回问题（C1 Critical + I1/I2 Important）的修复，并扫描是否引入新缺陷。

### C1 复审结论：**已解决**

**核查证据：**

1. `paste.rs:68` — `PasteBackend` trait 签名已改为 `fn write_image(&mut self, width: usize, height: usize, rgba: &[u8]) -> Result<(), String>`，trait 文档明确注明"返回 Result，不内部吞错；写入失败时调用方必须跳过 send_paste"。

2. `system.rs:620-622`（`paste_image_with_backend`，macOS 实现臂）：
   ```rust
   if backend.write_image(width, height, rgba).is_err() {
       // C1：写图失败时剪贴板未更新，跳过 send_paste 避免粘出前台旧内容。
       return "write_back_only".to_string();
   }
   ```
   失败路径提前 return，不进入 `probe.is_trusted()` 分支，send_paste 绝对不会被调用。

3. `system.rs:242-259`（`paste_image_orchestrate`，非 macOS 及测试用纯逻辑编排）：同样以 `.is_err()` 提前返回 `"write_back_only"`，与 macOS 臂对称。

4. `macos_paste.rs:206-208` 和 `:282-284`（两个后端实现）：均直接 `return write_image_to_clipboard(...)` 把 `Result<(), String>` 透传给调用方，不吞错。

5. **测试覆盖（三角验证）**：
   - `system.rs:975` — C1①：`paste_image_orchestrate_write_fails_skips_send_paste`：`FakeBackend::image_write_fails()` 构造写图必败后端（`write_image_fails: true`），断言 `outcome == "write_back_only"` 且 `!backend.send_paste_called`。
   - `system.rs:995` — C1②：写图成功+trusted → send_paste 被调用，返回 full_paste。
   - `system.rs:1009` — C1③：写图成功+untrusted → 不 send_paste，返回 write_back_only。
   - `FakeBackend` 有 `write_image_called` 标志位，三用例均断言 `write_image_called == true`，防止测试绕过被测对象。断言无捷径模式。

   与文本路径语义对齐：文本路径 `paste_text_with_backend` 在 `write_and_confirm` 返回 `Err` 时返回 `"write_back_only"`（system.rs:601），图片路径行为完全对称。

**评定：C1 Critical 完全修复，测试覆盖充分，语义与文本路径对齐。**

### I1 复审结论：**已解决**

**核查证据：**

`system.rs:173-176`（`fetch_image_payload`）SQL 已改为：
```sql
SELECT original FROM clip_images
WHERE clip_item_id = ?1 AND is_deleted = 0
ORDER BY last_modified_utc DESC, rowid DESC
LIMIT 1
```

- `clip_images` 表（`db.rs:769`）确有 `last_modified_utc INTEGER NOT NULL` 列，SQL 运行期不会报错。
- 排序键为 `last_modified_utc DESC, rowid DESC`，双键确定性满足项目 `code-general.md` 规范（同毫秒并列时 rowid DESC 兜底）。注释也明确注明原因（"避免同值并列时取序不定"）。

**评定：I1 Important 完全修复，排序确定性达标，运行期无报错风险。**

### I2 复审结论：**已解决**

**核查证据：**

`macos_paste.rs:61`：
```rust
bytes: Cow::Borrowed(rgba),
```

`rgba` 参数类型为 `&[u8]`，`set_image` 同步消费 `ImageData`（函数调用结束即丢弃），借用在调用期间保持有效，无悬垂风险。注释（macos_paste.rs:45-46）明确说明"用 `Cow::Borrowed` 借用入参字节，避免对大图做无谓全量克隆（set_image 同步消费、不需要更长生命周期）"。

**评定：I2 Important 完全修复，借用生命周期正确，无悬垂。**

### 新问题扫描

对修复涉及的代码路径扫描后，未发现引入新的 Critical 或 Important 问题：

- `paste_image_with_backend` 函数体（14行、嵌套2层）符合规范，早返回结构清晰。
- `paste_image_orchestrate`（公开的纯逻辑函数，无 OS 边界）与 macOS 实现臂逻辑对称，可独立测试。
- 两个后端实现（macOS + Fallback）的 `write_image` 签名、实现、错误透传方式完全一致，无对称性漏洞。

### 复审最终结论

三项打回问题全部已解决，且未引入新缺陷。

**VERDICT: APPROVE**

`C1(Critical) → 已解决 · C1-失败路径提前 return + 三角测试覆盖(失败/成功+trusted/成功+untrusted) · system.rs:620-622, paste.rs:68`
`I1(Important) → 已解决 · ORDER BY last_modified_utc DESC, rowid DESC LIMIT 1，列确实存在 · system.rs:173-176`
`I2(Important) → 已解决 · Cow::Borrowed(rgba)，借用生命周期正确 · macos_paste.rs:61`
