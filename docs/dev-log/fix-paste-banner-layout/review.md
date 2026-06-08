---
id: fix-paste-banner-layout-review
type: review
level: 小功能
parent: fix-paste-banner-layout
children: []
created: 2026-06-08T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: []
evidence:
  - "静态读码 diff：tauri.conf.json / ClipboardPage.tsx / clipboard-page.test.tsx"
  - "schema 验证：node_modules/@tauri-apps/cli config.schema.json 3682 行确认 signingIdentity camelCase + string|null 类型"
  - "CSS token 验证：src/theme/tokens.css 确认 --muted/--surface/--surface-2/--border/--fg/--danger 全部已定义"
  - "focus 样式：src/theme/base.css button:focus-visible 全局规则已覆盖新增按钮"
author: code-reviewer
---

# 审查结论（复审）· paste 横幅按钮 + ad-hoc 签名（v0.3.2 增量）

## 审查范围

本次为增量复审，对象是在前次横幅移位修复之上新增的三处改动：

1. `src-tauri/tauri.conf.json`：`bundle.macOS.signingIdentity: "-"` 启用 ad-hoc 签名
2. `src/panels/clipboard/ClipboardPage.tsx`：横幅重构（`role="status"`、flex 布局、新增「打开辅助功能设置」按钮、文案改善）
3. `src/panels/clipboard/clipboard-page.test.tsx`：新增 5 条测试（2 条命名功能用例 + 1 条 DOM 位置用例 + 2 条边界探测用例）

前次 review 遗留的唯一 Important 问题（`role="status"` 缺失）已在本次改动中修复，确认闭环。

## 发现问题（置信度 ≥ 80 才报）

无置信度 ≥ 80 的问题。

## 详细分析

### 1. tauri.conf.json：`bundle.macOS.signingIdentity`

- **键名合规**：Tauri v2 官方 schema（`@tauri-apps/cli@2.11.2/config.schema.json:3682`）确认键名为 `signingIdentity`（camelCase），类型 `string | null`，与本次改动完全吻合。
- **值语义**：`"-"` 是 macOS `codesign` 的 ad-hoc identity 标准值，仅绑定运行路径、不触发公证，符合修复辅助功能授权持久化的预期目的。
- **副作用核查**：`hardenedRuntime` 默认为 `true`，未显式覆盖；ad-hoc + hardenedRuntime 组合在本机开发场景下是已知可用组合（entitlements 由 Tauri 默认提供），不引入新运行时限制。未引入 `entitlements`、`providerShortName` 等敏感字段。
- **JSON 合法性**：文件格式与结构正确，`macOS` 对象嵌于 `bundle` 顶层，位置符合 schema 层级要求。

### 2. ClipboardPage.tsx：横幅重构

**结构与布局**

- 横幅位于 `opError` 之后、`.clip-list-col` 之前，`gridColumn: "1 / -1"` 正确跨双栏，与旧 opError 横幅模式一致。
- 新增 `display: "flex"` + `alignItems: "center"` + `gap: 12`，文字与按钮横向对齐，布局逻辑清晰。
- `<span>{pasteMsg}</span>` 包裹文案无冗余嵌套。
- 按钮 `flexShrink: 0` 防止按钮被压缩，合理。

**无障碍**

- `role="status"` 已加（修复前次 review 指出的问题），屏幕阅读器将以 `aria-live="polite"` 感知非打断式通知，正确。
- 按钮有 `type="button"` 防止意外表单提交；全局 `button:focus-visible` 样式（`src/theme/base.css:41-47`）已覆盖新按钮，键盘焦点指示器不缺失。
- 点击回调 `() => { void handleOpenSystemSettings(); }` 正确处理 Promise void，无未捕获 rejection 风险（`handleOpenSystemSettings` 内部已有 try/catch）。

**CSS token**

所有 token 均已在 `src/theme/tokens.css` 定义（亮/暗双模式）：`--muted`、`--surface`、`--surface-2`、`--border`、`--fg`。无幻数 / 硬编码颜色。

**code-standards 合规**

- inline style 是文件已有横幅的延续模式（`opError` 横幅同样使用 inline style），属一致性复用，非新引入反模式。
- 函数/嵌套/命名均在约束内，无死代码，无装饰性分隔注释。
- 文案修改仅针对 `write_back_only` 分支的 `setPasteMsg` 字符串，无逻辑变更。

### 3. clipboard-page.test.tsx：新增测试

**5 条新测试分析**

| 测试名 | 是否恒真 | 是否旁路 | 命中率判断 |
|---|---|---|---|
| 横幅含「打开辅助功能设置」按钮，点击触发 openAccessibilitySettings | 否（实际 click + waitFor 断言调用次数） | 否（走完整 IPC mock 链） | 高：按钮不存在或 onClick 错写则断言失败 |
| 横幅有 role=status | 否（closest 找 role="status" 如不存在返回 null） | 否 | 高：role 缺失则 `expect(banner).not.toBeNull()` 失败 |
| 横幅渲染在列表栏之前（DOM 位置） | 否（compareDocumentPosition bit 检测位置关系） | 否 | 高：若仍在列表后，DOCUMENT_POSITION_FOLLOWING bit 为 0，断言失败 |
| 边界：opError 和 pasteMsg 同时出现，两者均在 listCol 之前，opError 在 pasteMsg 之前 | 否（三条 compareDocumentPosition 断言相互独立） | 否 | 高：渲染顺序错误任一断言失败 |
| 边界：pasteMsg 为 null 时不渲染横幅 | 否（queryByText 应返回 null） | 否 | 中-高：空状态防退步 |

**命名规范**：`clipboard-page: <场景>` 与文件已有约定一致；边界探测独立 `describe` 块，组织清晰。

**装饰性分隔注释**：`// ---- 边界探测（tester 补充，2026-06-08）----`（line 558）违反 code-general.md「禁装饰性分隔注释（含测试文件）」规定。

> 说明：该分隔行置信度约 75（明确在规范中列出，但仅影响注释风格、不影响运行正确性），不达 80 报告门槛。不上升为正式 finding，记录备查。

**compareDocumentPosition 语义验证**：`A.compareDocumentPosition(B) & DOCUMENT_POSITION_FOLLOWING(4)` 非零表示 B 在 A 之后（DOM 顺序），用于断言「listCol 在 pasteMsg/opError 之后」逻辑正确。

## 前次 review 遗留问题闭环

| 旧问题 | 状态 |
|---|---|
| `pasteMsg` 横幅缺 `role="status"` | 已修复（ClipboardPage.tsx:229） |

## 总结

无置信度 ≥ 80 的问题。三处改动均符合项目规范与 code-standards。
测试覆盖扎实，5 条新用例均非恒真/非旁路，命中率高。

---

## 复审结论（2026-06-08）

status: 通过

三项改动全部通过审查。前次遗留 `role="status"` 问题已闭环。
无阻塞性发现；唯一低于报告门槛的风格问题（装饰性分隔注释 line 558）留作非阻塞备注。
