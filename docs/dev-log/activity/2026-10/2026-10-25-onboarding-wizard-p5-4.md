---
status: done
category: implementation
subject: P5-4 首次引导向导（四步 + 授权后自动收起 + 此后不再出现）
summary: OnboardCard 从单张授权卡片改成「欢迎 → 热键介绍 → 辅助功能授权 → 自启确认」四步向导，状态机在 Rust 与 TS 两侧各以纯函数实现并各有一套单测；新增只读命令 get_onboarding_status 把 onboarding_action / paste_capability 接进生产路径（两者从 dead-code 清单的「仅测试引用」消失）。修复 roadmap 原文的缺陷「授权后 OnboardCard 仍会再出现一次」：授权观测由未授权翻成已授权时授权步骤立即收起，向导走完/关闭后落盘 qq-onboard-finished 此后不再出现（兼容旧的 qq-onboard-dismissed）。变异验证：删「授权后收起」分支 → Rust 红 1、TS 红 2。新增的 4 个 Rust 状态机函数落在 dead-code 清单的「仅测试引用」，已按 P6-3 口径登记。
evidence:
  commits:
    - "ca2668 feat(onboarding): 首次引导改为四步向导，授权后自动收起（P5-4）"
  commands:
    - "pnpm exec vitest run"
    - "pnpm exec vitest run src/panels/clipboard/OnboardCard.test.tsx"
    - "pnpm exec tsc -b"
    - "cargo test --test onboarding"
    - "cargo clippy --all-targets -- -D warnings"
    - "cargo check --lib"
    - "rustfmt --edition 2021 --check src/onboarding.rs tests/onboarding.rs"
    - "node scripts/rust-dead-code-inventory.mjs"
  urls: []
---

# AI 活动记录 · P5-4 首次引导向导

## 背景

roadmap Phase 5 的 P5-4 原话：「多步：欢迎 → 热键介绍 → 辅助功能授权 → 自启确认；授权后自动恢复
（当前 OnboardCard 授权后仍会再出现一次）」。改动前 `src/panels/clipboard/OnboardCard.tsx` 只有
一张辅助功能卡片，显隐完全由 localStorage 的 `qq-onboard-dismissed` 决定——**它从不读授权状态**，
所以用户去系统设置点完「允许」回到面板，卡片照样在那；点掉之后也只是「关掉一次」，而不是
「授权已满足，不用再问」。

`src-tauri/src/onboarding.rs` 里 `onboarding_action` / `paste_capability` 当时被
`scripts/rust-dead-code-inventory.mjs` 登记为「仅测试引用」（`perform_paste_or_degrade` 实际上
已被 `ipc/system.rs::paste_orchestrate` 调用，不在清单里），本项负责把前者接进生产路径。

## 设计决策（两处取舍，都可一行改回）

1. **「授权后自动收起」收起的是授权步骤，不是整个向导。**
   纯函数 `observe_trust` 在「未授权 → 已授权 且正停在授权步骤」时把步骤从
   `accessibility` 推到 `autostart`，而不是推到 `done`。理由：按字面「引导自动收起」会让
   **任何当场授权的用户永远看不到自启确认这一步**，而自启确认正是四步里的最后一步
   （`AutostartConfig::default()` 是「开」，仍需要用户确认一次）。若裁决要字面口径，
   把 Rust/TS 两侧各 1 行（`AutostartConfirm` → `Done`）改掉即可，两侧测试已按「收起授权步骤」
   命名，改口径时同步改名。
2. **向导进度仍存在 localStorage，不新建 onboarding.json。**
   与既有 `qq-onboard-dismissed` 同源，减少一条 IPC 与一个配置文件；代价是后端不知道进度，
   详见下方「登记的风险」。

## 改动

### 后端 `src-tauri/src/onboarding.rs`（+213）

- 新增向导状态机（纯函数）：`OnboardingStep`（`welcome/hotkey/accessibility/autostart/done`）、
  `OnboardingState`、`advance`、`skip_step`、`observe_trust`、`finish`、`is_visible`。
  `advance` 在**前进到授权步骤的瞬间**才判断是否跳过（已授权则直接到 `autostart`），
  而不是进向导时一次算好——用户可能在向导开着的时候才去授权。
- 新增 DTO 与命令：`OnboardingStatusDto { action, capability }`、`action_code`、`capability_code`、
  `onboarding_status(probe)`（纯函数，可 headless 测）、`get_onboarding_status()`（只读命令，
  经 `platform_probe()` 选 `MacOsAccessibilityProbe` / `FallbackAccessibilityProbe`）。
- `perform_paste_or_degrade` 改为按 `paste_capability(probe)` 分支：同一个「能不能粘贴」问题
  只留一处判据，`paste_capability` 因此也进了生产路径。
- `src-tauri/src/lib.rs` 只加 1 行注册（`invoke_handler` 里 `onboarding::get_onboarding_status,`）。

### 前端

- `src/ipc/ipc-client.ts`：`OnboardingStatus` 接口 + `getOnboardingStatus()` wrapper，取值字符串
  与 Rust DTO 逐字对齐；两侧各有一条契约测试锚定（Rust `onboarding_status_dto_codes_are_frontend_contract`、
  TS `引导状态 IPC 契约` 两条）。
- `src/panels/clipboard/OnboardCard.tsx`：导出与 Rust 同名同义的纯状态机（`createGuideState` /
  `advanceGuide` / `skipGuideStep` / `observeAccessibilityTrust` / `finishGuide` / `isGuideVisible`），
  组件按步骤渲染标题、说明、进度点（`第 N / 4 步`）与操作按钮；`done` 时 `return null`。
  另加 `formatAccelerator`（`CmdOrCtrl+Shift+C` → `⌘⇧C`），热键步骤显示用户真实绑定。
- **欢迎页也放了「前往系统设置」按钮**：`clipboard-page.test.tsx`（不在本项写入范围）有一条既有
  行为契约——首次渲染就能一键跳系统设置。该按钮同时是老用户「跳过介绍直接授权」的捷径，
  不是为了测试硬凑：三个步骤里只有欢迎页与授权页会渲染它，授权页才是主入口。
- `src/panels/clipboard/ClipboardPage.tsx`：挂载时读一次 `getOnboardingStatus`，
  并在 `window` 的 `focus` 事件上再读一次（用户从系统设置切回面板即触发，这是「即时收起」的实现基础）；
  读失败按未授权处理。自启确认读 `getLaunchOnLogin`（失败保持后端默认「开」）、切换走
  `setLaunchOnLogin`（失败如实提示 `opError`，不假装生效）。向导进入终态即落盘
  `qq-onboard-finished=true`；`qq-onboard-dismissed` 作为旧标记继续被认作「已完成」，
  已关过旧卡片的用户不会重新看一遍。
- `src/panels/clipboard/clipboard.css`：进度点、`kbd`、复选框三组样式。

## 实跑验证（证据先于断言）

| 命令 | 结果 |
|---|---|
| `pnpm exec vitest run` | **74 files / 788 tests passed** |
| `pnpm exec vitest run src/panels/clipboard/OnboardCard.test.tsx` | **22 passed** |
| `pnpm exec tsc -b` | 无输出（干净） |
| `cargo test --test onboarding`（隔离 worktree） | **14 passed** |
| `cargo check --lib`（隔离 worktree） | OK（21.56s，真编译） |
| `cargo clippy --all-targets -- -D warnings`（隔离 worktree） | `No issues found` |
| `rustfmt --check src/onboarding.rs tests/onboarding.rs` | 干净 |

**为什么后端验证跑在隔离 worktree**：共享工作区当时同时有 pasteboard-execute 未完成的
`MainThreadMetaReader` 重构，`cargo test` 直接编不过（`src/lib.rs:438` 报
`cannot find MainThreadMetaReader in pasteboard_meta`），与本项无关。按纪律用
`git worktree add /tmp/qq-onb-verify HEAD` + 只覆盖本项补丁复现，全部绿；共享工作区一行未动。

**`cargo fmt --all --check` 仍红，但不是本项**：唯一 diff 在 `src-tauri/src/tray.rs` 三处
（tray-execute 已提交但未格式化的代码）。本项两个 `.rs` 文件 `rustfmt --check` 干净。

## 可失败性（变异实跑）

| 变异 | 结果 |
|---|---|
| Rust：删掉 `observe_trust` 里「已授权且在授权步骤 → 前进到 autostart」分支 | **1 红**：`onboarding_guide_observe_trust_collapses_accessibility_step`（`left: AccessibilityAuth, right: AutostartConfirm`） |
| TS：删掉 `observeAccessibilityTrust` 里同一条分支 | **2 红**：`OnboardCard 状态机 > 授权从未授权变为已授权后，授权步骤自动收起并前进到自启确认`、`OnboardCard 渲染 > 辅助功能从未授权变为已授权后，授权请求不再渲染` |

「授权后不再出现」由三处锚定：`is_visible(done) === false`、`advance(done)` 保持终态、
以及渲染层 `container.querySelector(".onboard") === null`。

## dead-code 闭环与新增登记

- 闭环：`onboarding_action`、`paste_capability` 从清单的「仅测试引用」消失（现由同文件内的
  `onboarding_status` / `perform_paste_or_degrade` 引用，落进「仅本文件引用」桶，生产可达）。
- **登记的风险（P6-3 口径）**：本项新增的 `is_visible` / `skip_step` / `observe_trust` / `finish`
  落在「仅测试引用」（`advance` 因被 `skip_step` 引用而不在清单）。原因是验收要求双端各一套状态机
  （Rust 一份兼作参考实现与契约锚点、TS 一份驱动实际 UI），而向导每次点击都走 IPC 往返不值得。
  真正闭环的方案：把向导进度从 localStorage 迁到 `onboarding.json`，由后端持有步骤并在
  `get_onboarding_status` 里做 `observe_trust`，这样 4 个函数全部进生产路径——代价是多一条
  `set_onboarding_step` 命令 + 每次点击一次 IPC。是否值得，留给后续裁决。

## 待人工（真机项，本项**未验证**，不得据本文声称已验证）

1. **系统辅助功能授权弹窗的真实行为**：从深链跳到「隐私与安全性 › 辅助功能」、勾选 QuickQuick 后
   macOS 是否弹 TCC 提示、是否需要重启应用才生效。
2. **授权后的即时恢复手感**：从系统设置切回面板时 `window` 的 `focus` 事件是否触发、
   `AXIsProcessTrusted` 是否立刻翻真、授权步骤收起是否即时（headless 只验到「状态机与渲染会收起」，
   没有验到真实焦点事件链）。
3. **打包 GUI 中的呈现**：进度点、`kbd`、复选框、按钮组在真实 webview 里的排版与对比度。
4. **授权身份按可执行文件判定**：`AXIsProcessTrusted` 认的是当前进程的可执行文件，
   **开发构建与打包构建是两个不同的可执行文件，TCC 授权互不复用**——两处必须分别确认，
   否则会出现「开发机上已授权、打包后仍降级」的误判。
5. 自启真实生效：`set_launch_on_login` 写 LaunchAgent 后重启机器是否真的自启（本项只复用既有 IPC，
   该路径原本就未在真机验过）。

## 未验证

- Windows：`platform_probe()` 走 `FallbackAccessibilityProbe` 恒未授权，本机无 Windows target，
  未编译验证。
- 授权状态的**持续轮询**：只在挂载与窗口获焦时读取，没有做定时轮询；若某个 macOS 版本不派发
  `focus` 事件，用户在面板开着的情况下授权后需要重开面板才会收起（真机确认项第 2 条覆盖）。
