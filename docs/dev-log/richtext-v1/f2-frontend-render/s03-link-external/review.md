---
id: RT1-F2-S03-review
type: review
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: 800ee36
acceptance_ids: [RT1-F2-S03]
author: code-reviewer
---

# 审查报告：RT1-F2-S03 富文本链接点击走外部浏览器

## 审查范围

- `src-tauri/Cargo.toml`（新增 `tauri-plugin-opener = "2"`）
- `src-tauri/src/lib.rs`（`.plugin(tauri_plugin_opener::init())`）
- `src-tauri/capabilities/default.json`（新增 `opener:allow-open-url`）
- `src/panels/translate/browser-api.ts`（新增 `openExternalUrl`）
- `src/panels/clipboard/rich-link.ts`（新文件：`resolveRichLinkClick` + `handleRichLinkClick`）
- `src/panels/clipboard/ClipPreview.tsx`（PreviewContent 加 `onClick={handleRichLinkClick}`）
- `src/clip-popover/PopoverPreview.tsx`（TextContent 加 `onClick={handleRichLinkClick}`）
- `src/panels/clipboard/rich-link.test.ts`（纯函数测试）
- `src/panels/clipboard/rich-link-click.test.tsx`（集成测试）

## Critical 级问题

无。

## Important 级问题（非阻塞建议）

### I-1：`javascript:` 和 `data:` 协议未在测试中显式覆盖（Important · 置信度 82）

- **文件**：`src/panels/clipboard/rich-link.test.ts`
- **证据**：`resolve_rich_link_filters_non_http_schemes` 测试用例仅覆盖 `file://` 协议被拒。`javascript:` 和 `data:` 未出现在测试中。虽然这两类协议在 DOMPurify 清洗时就已被移除（双重防御），但 `resolveRichLinkClick` 函数本身是独立可测的白名单守卫，其规格注释明确"javascript: 已被 DOMPurify 剥离，file:// 等不放行"——有说明却无对应断言，使纯函数级别的测试规格不完整。
- **影响**：不影响运行时安全（DOMPurify 已前置剥离），但降低测试可信度——若将来 sanitizer 被替换，`resolveRichLinkClick` 的测试无法单独守住此防线。
- **建议修复**：在 `resolve_rich_link_filters_non_http_schemes` 中补充：
  ```ts
  expect(resolveRichLinkClick(buildAnchor("javascript:alert(1)"))).toBeNull();
  expect(resolveRichLinkClick(buildAnchor("data:text/html,<b>x</b>"))).toBeNull();
  ```

## 合规验证结论

### 安全（主要审查重点）

| 检查点 | 结论 |
|--------|------|
| scheme 白名单（http / https / mailto 放行，其余拒绝） | 通过——`EXTERNAL_LINK_SCHEMES = new Set(["http:", "https:", "mailto:"])` 明确，其余协议走 `parsed.protocol` 匹配，不命中返回 `null` |
| `javascript:` 被拒 | 通过——`new URL("javascript:alert(1)", base).protocol` 为 `"javascript:"`，不在白名单，返回 `null`；且 DOMPurify 已在入 DOM 前剥除，双重防御 |
| `file://` 被拒 | 通过——测试已验证 `file:///etc/passwd` 返回 `null` |
| `data:` 被拒 | 通过——`new URL("data:...", base).protocol` 为 `"data:"`，不在白名单；DOMPurify 默认亦剥除 `data:` 图片以外的危险 data URI |
| 相对路径 href（如 `/etc/passwd`、`foo/bar`） | 审查通过——经 `new URL(rawHref, window.location.href)` 解析后 protocol 变为 `https:`（Tauri webview 的 `window.location` 为 `https://tauri.localhost/` 或 `tauri://localhost/`），通过白名单。最终 `openUrl` 调用系统外部浏览器打开 `tauri.localhost/etc/passwd`，**不会读取本机文件**（外部浏览器无 Tauri IPC 权限），不构成本地文件泄露；在本修复的威胁模型范围内可接受 |
| `preventDefault` 阻止 webview 导航 | 通过——`handleRichLinkClick` 在 `url !== null` 时先 `event.preventDefault()` 再调用 `openExternalUrl`；集成测试 `rich_link_click_opens_external_and_prevents_default` 断言 `clickEvent.defaultPrevented === true` |
| `getAttribute('href')` 避免 jsdom base url 误判 | 通过——用 `getAttribute` 而非 `anchor.href`，避免裸 `<a>`（无 href）被 `anchor.href` 解析为 jsdom base url；注释解释了原因 |
| 非链接点击不拦截 | 通过——`closest('a')` 为 `null` / `undefined` 时提前返回，集成测试 `rich_link_click_ignores_non_link_target` 验证 |
| 链接内子元素点击（如 `<a><b>text</b></a>` 点 `<b>`） | 通过——`closest('a')` 向上遍历，单测 `点击链接内的子元素（如 <b>）向上找到最近的 <a>` 覆盖 |

### capability 定性（`opener:allow-open-url` + `opener:allow-default-urls`）

**合法，非阻塞，已解决**。已通过 `src-tauri/gen/schemas/acl-manifests.json` 静态核查：
- `opener:allow-open-url` 为合法 permission identifier，已在 ACL manifests 中确认。
- `opener:allow-default-urls` 在 ACL manifests 中定义为：`"This enables opening mailto:, tel:, https:// and http:// urls using their default application."`，精确覆盖白名单协议（http / https / mailto / tel），**未引入 open-path / reveal-item-in-dir**，最小授权原则成立。
- `capabilities/default.json` 已同步包含两条 permission（第 19-20 行），与 `gen/schemas/capabilities.json` 一致。

### 正确性

| 检查点 | 结论 |
|--------|------|
| `closest('a')` 返回值的 null guard | 通过——`if (anchor === null \|\| anchor === undefined)` 两种情况均处理，符合 TypeScript 中 `closest()` 返回 `Element \| null` 的实际类型 |
| `openExternalUrl` 失败处理（修复后） | 通过——由 `void openExternalUrl(url)`（静默吞错）改为 `openExternalUrl(url).catch((err: unknown) => { console.error("[QuickQuick] 打开外部链接失败:", err); })`，err 类型为 `unknown`（无 `any`），显式记录，不静默 |
| 错误日志不暴露敏感信息 | 通过——url 经白名单校验（http/https/mailto），记录可接受；err 为原始 Error 对象，不含 API key 等敏感字段 |
| `new URL` 解析失败 guard | 通过——`try { parsed = new URL(...) } catch { return null }` 覆盖非法 URL 格式 |
| 空白 href guard | 通过——`rawHref.trim() === ""` 兜底 |

### DRY

两处预览（`ClipPreview.tsx` / `PopoverPreview.tsx`）复用同一 `handleRichLinkClick`，`resolveRichLinkClick` 纯函数分离，通过。

### 翻译路径隔离

`openExternalUrl` 新增在 `browser-api.ts`，`TranslatePage.tsx` / `TransPopoverApp.tsx` 均未引入；grep 确认翻译路径未受影响，通过。

### 代码规范

| 检查点 | 结论 |
|--------|------|
| 无 `any` / `@ts-ignore` | 通过——catch 用 `unknown` |
| 无 TODO / FIXME | 通过 |
| 注释写"为什么"（为什么用 `getAttribute`，为什么显式 .catch，为什么白名单） | 通过 |
| 无装饰性分隔符 | 通过 |
| 函数 ≤ 50 行，嵌套 ≤ 3 层 | 通过 |
| 命名描述性（动词+名词） | 通过 |

## 必改项

无。

## 非阻塞建议汇总

1. **I-1**（置信度 82）：`rich-link.test.ts` 补充 `javascript:` 和 `data:` 协议的显式断言，使纯函数级白名单测试规格自完备，无需依赖 DOMPurify 的前置剥离。

## 打回复审（2026-06-07）：opener scope + 错误不静默 已解决

### 复审触发原因

初审将 `openExternalUrl` 失败处理标注为"当前接受（void 形式）"，真机测试暴露了两个运行时缺陷：

1. `opener:allow-open-url` 只放行命令、无 URL scope，运行时所有 URL 被 ACL 拒绝，导致"点链接没反应"。
2. `void openExternalUrl(url)` 静默吞掉 rejection，无任何日志输出，使真机缺陷完全不可观测。

### 修复验证

**修复1：capability scope 补全**

`src-tauri/capabilities/default.json` 新增 `opener:allow-default-urls`。已通过 ACL manifests 静态核查，该 permission 描述为"enables opening mailto:, tel:, https:// and http:// urls using their default application"，精确对齐白名单协议（http / https / mailto / tel），**未引入** `open-path` / `reveal-item-in-dir`，最小授权原则成立。`gen/schemas/capabilities.json` 已同步，无遗漏。

**修复2：错误处理不静默**

`handleRichLinkClick` 中 `void openExternalUrl(url)` 改为：

```ts
openExternalUrl(url).catch((err: unknown) => {
  console.error("[QuickQuick] 打开外部链接失败:", err);
});
```

- `err: unknown`，无 `any`，符合规范
- 错误消息前缀 `[QuickQuick]` 便于过滤，不打印 url（白名单已校验，实际无敏感信息，但此处保守不打，可接受）
- 注释已说明"不可静默吞，否则真机点链接没反应无从诊断"

**新增测试 `rich_link_click_logs_error_when_open_rejects`**

`rich-link-click.test.tsx` 第 66-83 行：
- mock `openExternalUrl` 为 `mockRejectedValueOnce(new Error("ACL denied"))`
- 断言 `console.error` 被调用且第一个参数为 `"[QuickQuick] 打开外部链接失败:"`
- tester 验证：命中校验通过、删 `.catch` 变异 RED（不可变异假绿）

### 残留问题

无 Critical，I-1 非阻塞建议保持原样（不影响本次放行）。

### 复审结论

**通过（APPROVE）**。两项打回根因均已修复并有测试覆盖，opener scope 最小授权合规，错误处理显式可观测，代码规范无新违规。
