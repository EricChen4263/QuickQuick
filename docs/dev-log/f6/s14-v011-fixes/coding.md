---
id: F6-S14-coding
type: coding
level: 小功能
parent: F6
status: 已实现
commit: e63897c
author: coder
---

# v0.1.1 四项 bug 修复 · 编码留痕

本 slice 实现 v0.1.1 的 4 项已诊断 bug 修复：A 图片空白（CSP）、B 标题栏与红绿灯垂直不齐、
C 关窗/Esc 还焦给上一个 app、D 设置↔翻译页默认翻译源双向同步。

---

## A · 图片空白（CSP）

- **bug**：release 包内剪贴板图片条目渲染为空白；dev（Vite）正常。
- **根因**：图片用 `<img src="data:image/...;base64,...">` 渲染，但 `tauri.conf.json` 的 CSP
  `default-src 'self'; style-src 'self' 'unsafe-inline'` 无 `img-src`，`data:` 被 `'self'` 拒；
  仅 release（tauri:// 注入 CSP）触发，dev 不注入 CSP 故无碍。
- **改法**：`src-tauri/tauri.conf.json` 的 `app.security.csp` 改为
  `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'`。
  **只新增 `img-src 'self' data:`，未放松其它指令、未引入 `unsafe-eval`/通配**（红线遵守）。
- **验证**：`cargo build` 通过（schema 校验过）；release 实测由主流程出包核对。

## B · 标题栏文字与红绿灯垂直不齐

- **bug**：主窗自绘标题栏文字（`.qq-titlebar` 高 38px、文字中线 ~19px）与 macOS Overlay
  系统红绿灯按钮中线不重合，视觉错位。
- **根因**：红绿灯用系统默认 y，未对齐到 38px 自绘栏。
- **改法**：`src-tauri/tauri.conf.json` 的 main window 配置加
  `"trafficLightPosition": { "x": 18, "y": 13 }`。
  - schema 支持确认：tauri-utils-2.9.2 `config.rs:2065` `traffic_light_position: Option<LogicalPosition>`，
    serde 默认 camelCase 即 JSON 键 `trafficLightPosition`；要求 `titleBarStyle: Overlay` +
    `decorations: true`（本配置满足），故无需走 lib.rs 的 Rust 窗口 API。
  - **取值依据**：`trafficLightPosition` 为按钮组左上角坐标（LogicalPosition）。栏高 38px → 中线 y=19；
    红绿灯按钮直径约 12px → 顶边 ≈ 19 − 6 = 13，故 `y=13` 使按钮中线 ≈ 19 与文字中线重合。
    `x=18` 接近系统默认横向缩进，且远小于标题文字 76px 左缩进，红绿灯与文字不重叠。
  - **视觉项，最终以本地 release 出包实测为准**（主流程核对，可微调 ±1~2px）。

## C · 关主窗/Esc 关 popover 时焦点还给上一个 app

- **bug**：popover 按 Esc 关闭、主窗点关闭按钮关闭时，都只 `hide()`，焦点留在 QuickQuick 进程，
  未还给触发前的上一个 app；仅「粘贴到前台」路径走了 s12 的显式还焦。
- **根因**：Esc（`src/clip-popover/main.tsx`、`ClipPopoverApp.tsx`）与主窗 `CloseRequested`
  （`src-tauri/src/lib.rs setup_main_window_behavior`）都只 `hide()`，未走 `app.hide()` +
  按 `LastExternalApp` 记录 pid 显式激活的还焦逻辑。
- **改法**：复用 `system.rs` 既有还焦设施（`LastExternalApp`/`activate_target_app`/`hide_app`），
  抽出共享前半段 `hide_window_and_activate_target`（hide 窗口 + app.hide() + 按 pid 激活，**无等待**），
  原 `hide_and_restore_focus` 改为调它再追加 350ms 等待（仅粘贴路径需等待后 send_paste）。
  - 新增 Tauri 命令 `ipc::system::hide_and_return_focus`（读 `State<Arc<LastExternalApp>>`，
    调 `hide_window_and_activate_target`），在 `lib.rs` invoke_handler 注册。
  - 前端 popover 两处 Esc 处理改为 invoke 新命令：`ipc-client.ts` 加 `hideAndReturnFocus()` 包装，
    `main.tsx`（document keydown）与 `ClipPopoverApp.tsx`（onKeyDown）的 Esc 分支调它替代裸 hide。
  - 主窗接线：新增 `pub fn return_focus_after_main_hide(app)`（窗口已由 `win.hide()` 隐藏，故只补
    app.hide() + 激活；经 `app.try_state::<Arc<LastExternalApp>>()` 取托管状态）。
    `CloseRequested` 分支在 `win.hide()` 后调 `return_focus_after_main_hide(win.app_handle())`。
    **保留 stay_in_tray 语义**（仅 `stay_in_tray` 为真时 prevent_close + hide，进程驻留托盘不退出）。
  - **降级**：拿不到 pid/状态时 `activation_decision` 返回 `FallbackHide`，退化为纯 `app.hide()` 隐式
    还焦，不 panic。**非 macOS**：`activate_target_app`/`hide_app` 为既有 no-op，cfg 对称。
- **抽纯函数/测试**：C 的还焦决策已由既有纯函数 `activation_decision` + `should_record_pid` 覆盖
  （`tests/frontmost_logic_test.rs`），新命令仅 OS 边界 glue 复用之。新增 2 个复合契约测试到
  `frontmost_logic_test.rs`：`hide_and_return_focus_falls_back_when_no_pid_recorded`（无记录→FallbackHide，
  对应降级）、`hide_and_return_focus_activates_recorded_pid`（已记录→ActivatePid，对应显式激活），
  复现命令体内 `state.get() → activation_decision` 取值链。
- **验证**：新增 2 测试通过；`make verify` 五步全绿（见下）。

## D · 设置↔翻译页 默认翻译源不双向同步

- **bug**：在设置页改默认翻译源后，翻译页（或反向）不感知，需重 mount 才更新。
- **根因**：`set_selected_provider` 写完不发事件；两页各自 mount 时 `getSelectedProvider()` 读一次，
  互不感知对方修改。
- **改法**（照搬 `provider-config-changed` 事件范式）：
  - 后端 `settings.rs`：新增常量 `SELECTED_PROVIDER_CHANGED_EVENT: &str = "selected-provider-changed"`
    （注释标明与前端 events.ts 一致）；`set_selected_provider` 命令在 `set_selected_provider_impl`
    成功后 `app.emit(SELECTED_PROVIDER_CHANGED_EVENT, ())`，emit 失败仅记日志。
  - 前端 `src/ipc/events.ts`：加 `export const SELECTED_PROVIDER_CHANGED_EVENT = "selected-provider-changed" as const;`。
  - 翻译页 `TranslatePage.tsx`：新增 `listen(SELECTED_PROVIDER_CHANGED_EVENT, ...)` effect，回调
    `getSelectedProvider()` 刷新 `selectedProviderId`，用 cancelled+unlisten 范式防泄漏。
  - 设置页 `TranslateSourcePanel.tsx`：同样监听该事件，回调 `getSelectedProvider()` 刷新 `selectedId`。
  - 自发自收幂等（值相同），无需去抖。
- **测试**：`set_selected_provider_impl` 已有测试覆盖写入逻辑；emit 为 runtime 边界（需 AppHandle），
  与既有 `provider-config-changed` 一致不加 headless 单测，靠 `make verify` 保障。

---

## make verify 五步结果

见返回正文（tsc / cargo fmt --check / clippy -D warnings / vitest / cargo test 逐项），原始日志存
`artifacts/`。
