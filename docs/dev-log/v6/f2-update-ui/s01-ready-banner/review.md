---
id: V6-F2-S01-review
type: review_record
level: 小功能
parent: V6-F2
created: 2026-06-05T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [V6-F2-A08]
author: code-reviewer
---

# 审查结论 · 全局更新就绪提示条（F2-S01）

## 审查范围

| 文件 | 性质 |
|---|---|
| `src/components/UpdateBanner.tsx` | 新增——全局提示条组件 |
| `src/components/UpdateBanner.css` | 新增——样式 |
| `src/components/UpdateBanner.test.tsx` | 新增——组件测试 |
| `src/ipc/ipc-client.ts` | 改动——新增 `restartApp` |
| `src/ipc/restart-app.test.ts` | 新增——IPC 单测 |
| `src/App.tsx` | 改动——挂载 `<UpdateBanner/>` |

审查标准：项目规范（CLAUDE.md / AGENTS.md）+ code-standards 通用硬规则 + 前端 TS/React 规范 + 设计文档 `docs/design/auto-update.md` §四前端#4#5。

---

## 发现问题（置信度 ≥ 80 才报）

无置信度 ≥ 80 的问题。

---

## 维度核查

| 维度 | 核查结果 | 结论 |
|---|---|---|
| 事件名一致性 | 前端 `UPDATE_READY_EVENT = "update://ready"` 与后端 `update.rs:66 pub const UPDATE_READY_EVENT: &str = "update://ready"` 完全一致 | 合规 ✓ |
| Payload 字段对齐 | 前端 `{version: string}` 对应 Rust `UpdateReadyPayload { version: String }` + `#[serde(rename_all = "camelCase")]`，字段名 `version` 单字段无大小写变换，精确对齐 | 合规 ✓ |
| listen 清理惯例 | `cancelled flag + unlisten?.()` 与 App.tsx:40-60 惯例完全一致；`cancelled` 在 `then` 回调中检测，防止组件卸载后 Promise resolve 泄漏监听器 | 合规 ✓ |
| `restartApp` IPC 封装 | `try { await invoke<void>("restart_app") } catch (err) { throw toError(err) }`，完全遵循 ipc-client.ts 既有 try/catch + toError 模式 | 合规 ✓ |
| `restart_app` 命令注册 | `lib.rs:178` 已注册 `ipc::update::restart_app`，`update.rs:165` 实现已核对 | 合规 ✓ |
| 错误处理——重启失败 | `handleRestart` 调用 `.catch((err) => console.error(...))` 消化错误并记录日志；设计文档§三"静默失败策略"明确"后台连续失败不打扰用户，仅日志"；重启失败属极端路径（进程替换 API 层面失败），console.error 日志可追查，不崩渲染树，满足设计语义 | 合规 ✓ |
| 函数式 setState | `setReadyVersion((_prev) => event.payload.version)` 和 `setReadyVersion((_prev) => null)` 均使用函数式更新，符合前端规范；`_prev` 前缀标记有意忽略旧值，符合 TS 下划线约定 | 合规 ✓ |
| 禁 `any` | 全文无 `any`；`err: unknown` 类型化 catch 变量 | 合规 ✓ |
| 函数规模 | `UpdateBanner` 组件含 JSX 78 行，主逻辑 `useEffect` 约 22 行、`handleRestart` 5 行；`restartApp` 函数 6 行 | 均 ≤50 行 ✓ |
| 嵌套深度 | 最深 2 层（then 回调内 if cancelled），无超限 | ≤3 层 ✓ |
| 命名规范 | 组件 PascalCase、函数 camelCase、常量 UPPER_SNAKE；`handleRestart` 遵循动词+名词；接口 `UpdateReadyPayload` 描述性 | 合规 ✓ |
| 注释风格 | 组件 JSDoc 说明"为什么自包含 listen"（跨页全局关注点，无需父组件传递）；`update://ready` 行内注释说明对齐后端常量；无装饰性横线分隔 | 合规 ✓ |
| CSS token 复用 | `--surface-2`、`--border`、`--r`、`--shadow-pop`、`--fg` 全在 `tokens.css` 定义；`.btn`、`.btn-primary`、`.btn-ghost` 在 `components.css` 定义；无新造视觉系统、无引入第三方库 | 合规 ✓ |
| 无死代码/TODO/FIXME | 全文未发现 | 合规 ✓ |
| 测试覆盖与判别力 | `UpdateBanner.test.tsx` 覆盖：初始不渲染（无事件）→ 收到就绪事件后渲染含版本号提示条 → 点「重启更新」调 `restartApp`（命中校验）；点「稍后」隐藏提示条 + `restartApp` 未被调用（边界隔离）。`restart-app.test.ts` 覆盖 invoke 调用正确命令名 + reject 时重抛为 Error 含原始消息。tester 已做判别力验证，测试为真阳性 | 合规 ✓ |
| 设计符合度（§四前端#4） | `ipc-client.ts` 新增 `restartApp()`，封装 `invoke("restart_app")` | 合规 ✓ |
| 设计符合度（§四前端#5） | App 顶层挂载 `<UpdateBanner/>`；listen `update://ready` → 渲染含版本号提示条；「重启更新」调 `restartApp()`；「稍后」关闭提示条 | 合规 ✓ |
| 验收项 A08 | 收到 `update://ready` 渲染含版本号提示条 + 点「重启更新」调 `restart_app`，实现与设计一致 | 命中 ✓ |

---

## 低置信度观察（< 80，不阻塞，仅供参考）

- **`handleRestart` 无用户可见错误反馈**（置信度约 55）：重启失败时仅 `console.error`，无 UI 提示。设计文档§三明确"静默策略"，且重启失败是极端路径，当前实现符合设计语义；若后续产品决策变更为"允许展示错误"，可升级为 toast/状态提示，但当前不构成阻塞。

---

## 结论

通过。无 Critical 级问题，无置信度 ≥ 80 的问题。改动满足：

- 项目规范（CLAUDE.md / AGENTS.md）中的函数规模、嵌套、命名、注释、错误处理要求。
- code-standards 通用硬规则：禁 `any`、函数式 setState、注释写"为什么"、无装饰性分隔、无死代码。
- 设计文档 `docs/design/auto-update.md` §四前端 #4（`restartApp` IPC 封装）和 #5（全局提示条组件）。
- 验收项 V6-F2-A08：实现与设计一致，tester 已验证测试有判别力。

APPROVE
