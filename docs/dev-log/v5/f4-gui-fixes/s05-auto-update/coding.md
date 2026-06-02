---
id: f4-gui-fixes-s05-client
title: 自动更新客户端实装
status: 实现完成
commit: 34a8a85
date: 2026-06-02
---

# 自动更新客户端实装

## 实现内容

### 后端 `check_for_updates` 命令（新建 `src-tauri/src/ipc/update.rs`）
通过 `tauri_plugin_updater::UpdaterExt` 的 `app.updater()?.check().await` 发起更新检查：有更新返回 `{ available: true, version, currentVersion }`；无更新返回 `available: false`；任何错误映射为中文友好字符串。注册进 `ipc/mod.rs` 与 `lib.rs` invoke_handler。

### 前端 ipc-client 扩展（`src/ipc/ipc-client.ts`）
新增 `CheckUpdateResult` 接口与 `checkForUpdates()` 函数，调用 `invoke("check_for_updates")`，错误通过 `toError` 统一转 Error。

### GeneralPanel 独立文件（新建 `src/panels/settings/GeneralPanel.tsx`）
从 SettingsPage.tsx 内联的 GeneralPanel 提取为独立组件，加入「立即检查更新」行：`isChecking` 禁用按钮 + `checkMsg`（成功，muted 色）/ `checkError`（失败，danger 色 + role=alert）反馈，复用 StoragePanel 反馈样式风格。

## 为何不在 startup 自动检查
`tauri.conf.json` 的 `endpoints` 仍为占位 URL（`placeholder.example.com`），自动检查会在每次启动时产生网络错误噪音。仅提供手动触发命令，待真实 infra 就绪后再接入自动流程。

## 服务端 infra 仍外部阻塞
endpoint（真实更新服务器）、minisign 签名密钥对、CI 发布流水线均未就绪，属仓库外基础设施，**本次不改 tauri.conf 占位配置**，留待后续单独处理。

## 测试范围与不可测部分
- **可测（已覆盖）**：`checkForUpdates()` 命令名/返回值/错误映射（4 例）；GeneralPanel 按钮交互全路径（available=true/false/reject/loading，5 例）。
- **不可测**：真实 `updater.check()` 依赖网络与 endpoint，后端命令编译通过即算覆盖，不造无意义 mock 后端测试。

## 验收结果
- `cargo check`：exit 0，无错误
- `pnpm test --run`：43 files / 355 tests，全绿
- `pnpm exec tsc --noEmit`：exit 0，无类型错误
