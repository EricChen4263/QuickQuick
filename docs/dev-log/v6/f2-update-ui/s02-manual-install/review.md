---
id: V6-F2-S02-review
type: review_record
level: 小功能
parent: V6-F2
status: 通过
commit: PENDING
acceptance_ids: [V6-F2-A09]
author: code-reviewer
created: 2026-06-05T00:00:00Z
---

# 审查结论 · 手动检查后下载安装入口（S02）

## 审查范围

| 文件 | 性质 |
|---|---|
| `src/panels/settings/GeneralPanel.tsx` | 新增 `UpdateInstallAction` 子组件、`UpdateCheckOutcome` 接口、outcome 状态、条件渲染 |
| `src/ipc/ipc-client.ts` | 新增 `downloadAndInstallUpdate` + `restartApp`；订正 `checkForUpdates` doc 注释 |
| `src/panels/settings/GeneralPanel.test.tsx` | 新建，覆盖 A09 三条用例 |
| `src/panels/settings/check-update-button.test.tsx` | vi.mock 补 `downloadAndInstallUpdate` 防既有用例报 undefined |

## 注释订正核对

`grep -n "占位" src/ipc/ipc-client.ts` → **0 匹配**（无残留"占位"过时说法）。

`ipc-client.ts:501` 现文：`endpoint 已是真实地址；网络/清单异常时会 reject，调用方应以友好文案展示错误。`

Rust 侧 `update.rs:6-7` 现文：`endpoint 为真实地址（github.com/EricChen4263/QuickQuick/…，CI 已产签名 latest.json）`，前端注释与后端口径一致。**订正到位。**

## 分级发现

### Critical（置信度 ≥ 80，阻塞）

无。

### Important（置信度 ≥ 80，建议改）

**I01 · UpdateInstallAction 函数体超 50 行 · 置信度 85**

- `GeneralPanel.tsx:46-96`，函数体共 51 行（第 46 行 `function UpdateInstallAction` 至第 96 行 `}`）。
- 项目规范（`code-general.md`）硬规则：函数 ≤ 50 行。超出 1 行，属边界违规。
- 逻辑本身正确，唯函数规模与硬规则有 1 行越界。
- 建议：将 JSX 中 `{doneMsg !== null && ...}` / `{error !== null && ...}` 两个小块提取为独立的局部元素变量（`const doneMsgEl` / `const errorEl`），或把 `handleInstall` 内联 `inner` 提出，可将主函数体压到 ≤50 行。

### Low（置信度 < 80，不计入阻塞）

以下问题置信度均低于报告阈值，仅供参考：

- **L01 · `outcome.version` 非空访问（置信度 70）**：`GeneralPanel.tsx:172` 在 `{hasUpdate ? <UpdateInstallAction version={outcome.version} />` 处，`outcome` 类型为 `UpdateCheckOutcome | null`，TypeScript 无法通过派生 `const hasUpdate` 布尔变量做 narrowing，理论上应报 TS2531（对象可能为 null）。实测 `pnpm exec tsc --noEmit` 无错误，说明当前 TypeScript 版本的 control-flow analysis 在此场景接受了推断（strict:true 已开启）。**运行时不存在崩溃风险**（`hasUpdate=true` 时逻辑上 `outcome` 必非 null），但建议显式断言 `outcome!.version` 或将分支改为 `outcome !== null && outcome.available ? <UpdateInstallAction ...>` 以消除隐式假设，防未来 TS 版本行为变更。置信度 70（tsc 无报错故不确认为真问题），不计入阻塞。

- **L02 · `GeneralPanel` 函数体 92 行（置信度 60）**：`GeneralPanel.tsx:99-190`，函数体 92 行，超出 50 行规则。但其中约 60 行为纯 JSX 返回语句（含 SettingGroup 四项），属于 React 组件 JSX 片段惯用体量，实际逻辑行（state + handler）约 28 行。项目其他组件（如 `TranslateSourcePanel`）亦有同等体量 JSX，无历史阻塞记录，故置信度降至 60。不计入本次阻塞。

## 规范合规性检查

| 维度 | 结论 |
|---|---|
| 禁 `any` | 无 any 使用 ✓ |
| 函数式 setState | 独立 `setState` 调用，无 mutate ✓（I01 为行数，非逻辑问题） |
| 类型化 props | `{ version: string }` 明确类型 ✓ |
| 子组件职责单一 | `UpdateInstallAction` 仅管理安装态，不读检查态 ✓ |
| 条件渲染 | `available=false` 不渲染操作区 ✓ |
| 下载中 disabled | `disabled={isInstalling}` 防重复点击 ✓ |
| 失败 role=alert | `role="alert"` 覆盖下载失败和检查失败两处 ✓ |
| ipc-client 模式 | `try/catch + toError` 同既有模式 ✓ |
| 无 TODO/FIXME | grep 确认 0 匹配 ✓ |
| 注释写"为什么" | JSDoc 均写行为与约束，无装饰性分隔注释 ✓ |
| 命名规范 | `handleInstall`（动词+名词）、`isInstalling`/`hasUpdate`（布尔前缀）✓ |
| mock 补齐 | `check-update-button.test.tsx` vi.mock 补 `downloadAndInstallUpdate`，既有 5 条用例全绿 ✓ |
| Rust 命令注册 | `download_and_install_update` + `restart_app` 均在 `lib.rs:177-178` 注册 ✓ |
| 全量测试 | artifacts/vitest-a09.log: 460 passed (52 files) ✓ |

## A09 验收符合度

设计要求（`§四前端#6`）：手动检查发现新版后出现「下载并安装」入口，点击触发 `downloadAndInstallUpdate`。

实现：`available=true` 时渲染 `UpdateInstallAction`，内含「下载并安装」按钮，`onClick` 调用 `downloadAndInstallUpdate()`；`available=false` 时仅显示「已是最新版本」，无操作按钮。与设计完全一致。

tester 已确认 A09 通过（命中校验有判别力）。

## 结论

**通过**。唯一置信度 ≥ 80 的问题为 I01（`UpdateInstallAction` 超出函数行数限制 1 行），属 Important 级别，不阻塞闭合，建议下一个小功能合并前顺手修正（1 行 JSX 提取即可）。无 Critical 级问题。

APPROVE
