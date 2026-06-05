---
id: V6-F2-S01-code
type: coding_record
level: 小功能
parent: V6-F2
children: []
status: 通过
commit: PENDING
acceptance_ids: [V6-F2-A08]
evidence: [src/components/UpdateBanner.tsx, src/components/UpdateBanner.test.tsx, src/components/UpdateBanner.css, src/ipc/ipc-client.ts, src/ipc/restart-app.test.ts, src/App.tsx, docs/dev-log/v6/f2-update-ui/s01-ready-banner/artifacts/vitest-a08.log, docs/dev-log/v6/f2-update-ui/s01-ready-banner/artifacts/vitest-a08-names.log, docs/dev-log/v6/f2-update-ui/s01-ready-banner/artifacts/tsc.log]
author: coder
---

# V6-F2-S01 更新就绪提示条（ready-banner）编码留痕

## 做了什么

实现 A08：前端收到后端 `update://ready` 事件时渲染非打扰更新提示条（含版本号），
点「重启更新」调用 `restart_app` 命令，点「稍后」隐藏提示条。

交付物（按交付序）：

1. **IPC 封装** `src/ipc/ipc-client.ts`：新增 `restartApp(): Promise<void>`，
   内部 `invoke<void>("restart_app")`，复用文件既有 `try/catch + toError` 模式（参照 `checkForUpdates`）。
2. **组件** `src/components/UpdateBanner.tsx` + `UpdateBanner.css`：自包含监听 `update://ready`，
   就绪后渲染右下角浮层提示条（版本号文案 +「重启更新」+「稍后」）。
3. **挂载** `src/App.tsx`：在 `AppShell` 内顶层渲染 `<UpdateBanner />`。
4. **额外健壮性单测** `src/ipc/restart-app.test.ts`：mock invoke，断言 `invoke("restart_app")` 被调用及 reject 重抛为 Error。

## 关键决策

- **为什么自包含 listen**：更新提示是跨页全局关注点，挂在 AppShell 顶层一处即可，
  无需父组件传事件或提升状态，故组件内部自行 `listen` 并用 `useState` 管理就绪版本号，职责单一、可独立测试。
- **为什么沿用 cancelled-flag**：严格复用 `App.tsx:40-60` 的 listen 注册惯例——
  `listen(...).then(fn => cancelled ? fn() : unlisten=fn)` + cleanup 置 `cancelled=true` 并 `unlisten?.()`，
  防止组件卸载后 Promise 才 resolve 造成监听器泄漏。
- **版本号 camelCase 对齐**：payload 类型 `{ version: string }`（camelCase），
  对齐 Rust `UpdateReadyPayload`（`#[serde(rename_all = "camelCase")]`，见 `update.rs:71-76`）；
  事件名常量 `UPDATE_READY_EVENT = "update://ready"` 与后端 `update.rs:66` 完全一致。
- **视觉复用既有 token**：提示条用 `.btn`/`.btn-primary`/`.btn-ghost` 与 `var(--surface-2)`/`var(--border)`/
  `var(--shadow-pop)`/`var(--r)`，不引第三方 UI 库、不新造设计系统。
- **role=status**：提示条用 `role="status"`（礼貌区，非打扰），区别于错误用的 `role="alert"`。
- **重启失败兜底**：`restartApp()` 正常路径下进程随即被替换重启、Promise 不 resolve；
  失败时 `.catch` 仅记日志，不阻塞 UI。

## 改动文件

- `src/ipc/ipc-client.ts`（改）：新增 `restartApp` 封装。
- `src/components/UpdateBanner.tsx`（新）：提示条组件。
- `src/components/UpdateBanner.css`（新）：提示条样式（复用 token）。
- `src/components/UpdateBanner.test.tsx`（新）：A08 组件测试 + 「稍后」隐藏测试。
- `src/ipc/restart-app.test.ts`（新）：restartApp IPC 单测。
- `src/App.tsx`（改）：import + 顶层挂载 `<UpdateBanner />`。

## 自测结论（红-绿-重构）

- **RED**：先写 `UpdateBanner.test.tsx`，运行 `pnpm test src/components/UpdateBanner.test.tsx`
  因 `./UpdateBanner` 无法解析（组件未实现）失败（EXIT=1，"no tests"）——非语法/环境错。
- **GREEN**：新增 `restartApp` + 实现 `UpdateBanner.tsx`/`.css`，再跑该测试通过（EXIT=0，2 tests passed）。
- **REFACTOR**：实现已最小且单一职责（函数均 ≤50 行、嵌套 ≤3 层、函数式 setState），无可去重项，保持全绿。

证据：

- A08 命中（JSON reporter 机器证据，`artifacts/vitest-a08-names.log`）：
  `update_banner_shows_on_ready_and_restart_invokes_command => passed`。
- 全量套件：`Test Files 51 passed (51) / Tests 457 passed (457)`（`artifacts/vitest-a08.log`）。
- 类型检查：`pnpm exec tsc --noEmit` EXIT=0、0 error（`artifacts/tsc.log`）。
- 自检：无装饰性分隔注释、无 TODO/FIXME、无 `any`。
- 注：项目无 lint 脚本（package.json scripts 仅 dev/build/preview/test/test:watch/tauri），以 tsc 严格类型检查为质量门。
