---
id: f4-gui-fixes-s05-client-review
title: 自动更新客户端 规范审查
type: review
level: small
parent: f4-gui-fixes-s05-client
created: 2026-06-02T00:00:00Z
status: 审查通过
commit: 34a8a85
acceptance_ids:
  - f4-gui-fixes-s05-client
author: code-reviewer
---

# 审查结论：审查通过（无未决高危）

## 逐文件发现

### src-tauri/src/ipc/update.rs

- 三路径完整：`Ok(Some)` / `Ok(None)` / `Err` 均有处理，无漏臂。
- 错误均通过 `map_err` / `Err(format!(...))` 映射为 `String`，无 unwrap / panic 点。
- `CheckUpdateResult` 带 `#[serde(rename_all = "camelCase")]`，`current_version` 序列化为 `currentVersion`，与前端 interface 字段名一致。
- `Ok(None)` 路径的 `current_version` 取自 `app.package_info().version`，`Ok(Some)` 路径取自 `update.current_version`；两者来源不同但在 tauri 中一致，无功能问题。
- **低 / 规范建议**：文件含 module 文档注释共 56 行，超出"函数 ≤ 50 行"规范 6 行（实际函数体本身仅 22 行，超出由文档头注释贡献）。置信度 25——规范针对函数体而非文件整体，不视为违规。

### src-tauri/src/lib.rs

- `check_for_updates` 仅注册进 `invoke_handler`（第 125 行），setup 闭包（第 127–140 行）内无任何自动调用。
- 设计意图（占位 endpoint 不自动触发）与代码行为完全一致，属有意设计，确认通过。

### src/ipc/ipc-client.ts

- `CheckUpdateResult` interface 三字段：`available: boolean`、`version: string`、`currentVersion: string`，与 Rust struct camelCase 序列化完全对齐。
- `checkForUpdates` 通过 `toError` 将 invoke reject（Rust `Err(String)` 或网络异常）统一转为 `Error` 对象，无 `any`，无裸 promise。
- `toError` 实现（第 56–61 行）已有完整 `instanceof Error` 判断，覆盖字符串和 Error 两种 reject 形式。

### src/panels/settings/SettingsPage.tsx

- 删去 39 行内联 `GeneralPanel` 实现，改 import 独立模块，等价重构。
- `launchOnLogin` / `stayInTray` / `autoUpdate` 三个 toggle 全部保留，无回归。

### src/panels/settings/GeneralPanel.tsx

- `isChecking` / `checkMsg` / `checkError` 三状态管理正确：点击前重置、finally 恢复、反馈分别写入对应 state。
- `handleCheckUpdate` 用 `void` 处理 onClick 中的 async 函数，无未捕获 promise。
- `runCheckForUpdates` 内部已 catch 所有异常并返回 `UpdateCheckFeedback`，不再抛出；`handleCheckUpdate` 外层 try/finally 仍有必要（防 `runCheckForUpdates` 意外 throw 导致 `isChecking` 卡住）。
- 错误展示 `<div role="alert" ...>`，通过 ARIA 标注，可访问性满足项目惯例（与 HotkeyPanel、StoragePanel 等一致）。
- `button` 带 `type="button"` + `disabled={isChecking}`，交互态完整。
- 无 `any`，无装饰性分隔注释，无死代码，命名描述性。
- **低 / 说明**：`runCheckForUpdates` 是模块级 async 函数（非 hook），并非 React 组件直接调用——这使它可在测试中独立覆盖，属合理设计。

### src/ipc/check-update.test.ts

- 覆盖四个路径：有更新返回 / 无更新返回 / 字符串 reject / Error reject，断言均为真实值检查，非空跑。
- `mockInvoke.mockReset()` 在 `beforeEach` 确保测试隔离。
- `checkForUpdates().catch(e => e)` 模式正确捕获 reject 并断言类型。

### src/panels/settings/check-update-button.test.tsx

- 五态覆盖：初始渲染 / available=true / available=false / reject / loading 期间 disabled。
- reject 路径：`checkForUpdates` reject → `runCheckForUpdates` 内 catch → `checkError` 设为固定文案 "检查更新失败，可能更新服务尚未配置" → 断言 `/检查更新失败/` 匹配，逻辑一致。
- loading 态用 `mockReturnValueOnce(new Promise(...))` 控制时序，真实测试异步 disabled 状态，非恒真断言。
- `vi.clearAllMocks()` + `beforeEach` 重设 getLaunchOnLogin / getStayInTray / getAutoUpdate，避免跨 case 状态泄漏。

## 高危 / 中危

无。

## 低危 / 建议

1. **[低] update.rs 文件 56 行略超规范**（置信度 25）：规范"函数 ≤ 50 行"的语义对象为函数体，此处超出由模块文档注释贡献，实际 `check_for_updates` 函数体 22 行，不影响可读性，可维持现状。

2. **[建议] `Ok(None)` 路径 current_version 来源注释**（置信度 20）：`app.package_info().version` 与 `update.current_version` 来源不同，现有注释已说明设计意图，如后续 tauri 版本行为有变化此处需留意，可加一行 `// package_info().version 与 update.current_version 在正常情况下应一致` 的简短说明，供未来维护者参考。

3. **[建议] `runCheckForUpdates` 丢弃原始 Error 消息**（置信度 20）：catch 返回固定文案"检查更新失败，可能更新服务尚未配置"，不含 Rust 返回的具体错误字符串。在占位 endpoint 阶段这是有意为之（避免把内部 URL 等技术信息暴露给用户），属合理权衡；若将来接入真实 infra，可考虑在开发环境 console.warn 原始错误以便调试。
