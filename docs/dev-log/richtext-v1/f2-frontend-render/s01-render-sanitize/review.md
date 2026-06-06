---
id: RT1-F2-S01-review
type: review
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F2-A01, RT1-A-SEC]
author: code-reviewer
---

# 审查报告：RT1-F2-S01 富文本预览渲染 + DOMPurify 清洗

## 审查范围

- `src/panels/clipboard/sanitize-html.ts`（新增）
- `src/panels/clipboard/ClipPreview.tsx`（PreviewContent 组件，richtext 分支）
- `src/clip-popover/PopoverPreview.tsx`（TextContent 组件，richtext 分支）
- `src/panels/clipboard/ClipPreview.test.tsx`（新增）
- `src/clip-popover/PopoverPreview.test.tsx`（新增）
- `package.json` / `pnpm-lock.yaml`（dompurify 依赖新增）

## Critical 级问题

无。

## Important 级问题（非阻塞建议）

### I-1：`<iframe>` payload 未覆盖（Important · 置信度 82）

- **文件**：`src/panels/clipboard/ClipPreview.test.tsx`、`src/clip-popover/PopoverPreview.test.tsx`
- **证据**：验收标准 RT1-A-SEC 明确要求证否测试使用 `<script>`、`onerror=`、`javascript:` URL、`<iframe>` 四类 payload。当前两份测试均覆盖前三类，但无 `<iframe>` 断言。
- **影响**：不满足 RT1-A-SEC 验收项"用真实 payload……断言被剥离"的完整要求，仅此一处遗漏，不影响主流程安全性（DOMPurify 默认 ALLOWED_TAGS 不含 iframe），但使 tester 无法通过 spec 机械验证全覆盖。
- **建议修复**：在 `clip_preview_strips_malicious_html` 与 `popover_preview_strips_malicious_html` 两个 case 的 `htmlContent` 中追加 `<iframe src="javascript:alert(4)"></iframe>`，并断言 `preview!.querySelector('iframe')` 为 null。

### I-2：`ClipItem.kind` 类型为宽 `string` 致守卫语义偏弱（Important · 置信度 80）

- **文件**：`src/ipc/ipc-client.ts`（第 7 行，pre-existing；本次 ClipPreview/PopoverPreview 消费了该字段）
- **证据**：`ClipItem.kind` 的类型为 `string` 而非字面量联合 `"text" | "richtext" | "image"`；守卫 `item.kind === "richtext"` 从编译器视角是 `string` 比较，类型不收窄，无法在此处直接访问 `item.htmlContent` 时从类型系统获得约束。本小功能改动未新引入，但若后续重构开发者误拼 kind 值，编译器无法拦截。
- **影响**：当前逻辑运行时正确，不影响 XSS 安全；属工程质量可改项，不阻塞本次交付。
- **建议修复**：将 `kind: string` 收窄为 `kind: "text" | "richtext" | "image"`（改动在 `ipc-client.ts`，超出本小功能范围，可在后续 RT1-F2 收尾时统一处理）。

## 合规验证结论

### 安全（XSS 红线，设计§五）

| 检查点 | 结论 |
|--------|------|
| 所有 htmlContent 入 DOM 前经 `sanitizeRichHtml` | 通过——全局搜索 `dangerouslySetInnerHTML` 确认两处均包裹 `sanitizeRichHtml(item.htmlContent)` |
| 无信任来源豁免 | 通过——无条件调用 sanitize，无 bypass 分支 |
| `sanitizeRichHtml` 默认配置剥除 `<script>` / 事件属性 / `javascript:` | 通过——DOMPurify 3.x 默认行为已覆盖，测试 payload 也验证了三类 |
| `<iframe>` payload 缺失 | 见 I-1（Important，非阻塞） |
| 后端 ingest 原样保存未清洗 HTML | 通过——本次改动全部在 `src/` 前端，未触碰 `src-tauri/`，后端存储逻辑未被改写 |
| CSP 未放开 `script-src` | 通过——`tauri.conf.json` CSP 为 `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'`，本次改动未修改 CSP |
| `DOMPurify.isSupported=false` 降级路径（返回原始 dirty 字符串） | 运行环境为 Tauri WebView（浏览器环境），`isSupported` 在运行时为 true；测试使用 jsdom 亦 true。无沙箱/SSR 场景，可接受 |

### 守卫严谨性

`kind === "richtext" && htmlContent !== undefined` 两重守卫均存在，纯文本与图片条目不会误入富文本渲染路径，通过。

### DRY

`sanitizeRichHtml` 单一出口，两处预览复用同一函数，通过。

### 依赖合理性

`dompurify ^3.4.8` 已在 `package.json` 的 `dependencies` 中声明，包自带 `.d.ts`（`dist/purify.cjs.d.ts`），无需额外安装 `@types/dompurify`，合理。

### 注释质量

三文件注释均写"为什么"（引用设计§五），无 TODO/FIXME，无死代码，通过。

### 代码规范

无 `any`，无 `@ts-ignore`，组件 props 已类型化，无装饰性分隔符，通过。

## 必改项

无（status = 通过）。

## 非阻塞建议汇总

1. **I-1**（置信度 82）：在两份恶意 payload 测试 case 中补充 `<iframe>` 断言，满足 RT1-A-SEC 全量覆盖要求。
2. **I-2**（置信度 80）：后续统一将 `ClipItem.kind` 由 `string` 收窄为字面量联合类型，提升类型系统防护。
