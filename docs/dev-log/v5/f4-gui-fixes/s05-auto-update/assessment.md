---
id: f4-gui-fixes-s05-auto-update
title: 自动检查更新真实 endpoint · 可行性评估（外部阻塞）
status: 外部阻塞（无法在本仓内完成）
commit: 001ccd8
date: 2026-06-02
---

# 自动检查更新 endpoint · 评估结论

## 现状
- `src-tauri/tauri.conf.json` `bundle.updater`：`endpoints` 指向 **`https://placeholder.example.com/quickquick/{{target}}/{{arch}}/{{current_version}}`**（占位）；`pubkey` 为一个示例 minisign 公钥。
- `src-tauri/src/lib.rs`：已注册 `tauri_plugin_updater::Builder::new().build()`，但**全仓没有任何 `app.updater().check()` 调用**——即从未真正发起更新检查。
- 设置侧：`auto_update` 开关已可持久化（`get_auto_update`/`set_auto_update`），但该开关**未被任何运行时逻辑读取**去驱动更新检查。

## 为什么无法在本仓内"做完"
Tauri 自动更新是 **客户端 + 服务端** 的完整链路，缺一不可：
1. **真实更新服务器**：需托管符合 Tauri updater 协议的版本清单 JSON（按 `{{target}}/{{arch}}/{{current_version}}` 响应是否有新版 + 下载 URL + 签名）。仓库内不存在、也无法在此创建并长期托管。
2. **minisign 私钥**：发布产物必须用与 `pubkey` 配对的**私钥**签名（`tauri signer`）。私钥是机密，不在仓库、不应入库，本环境无法获取/生成可信发布签名。
3. **发布/CI 流水线**：需要构建并把签名产物上传到更新服务器的发布流程。

以上三者均属**仓库外基础设施**，非代码改动可达成。强行把 `check()` 接到 `placeholder.example.com` 只会在启动时产生网络错误噪音，且无法验证，属"假完成"，故**不做**。

## 真正落地所需（交接清单，待具备基础设施后再做）
1. 部署更新服务器（自建静态清单 / GitHub Releases + `tauri-plugin-updater` 的 endpoints 格式，或 CrabNebula 等）。
2. `tauri signer generate` 生成密钥对：公钥写入 `tauri.conf.json` `bundle.updater.pubkey`（替换示例值），私钥存 CI secret。
3. 将 `endpoints` 改为真实地址。
4. 客户端接线（**这一步是纯代码、届时可在本仓做**）：启动后（或菜单"检查更新"）若 `get_auto_update()` 为真，调用 `app.updater().check()`，有更新则提示/下载/安装；无更新或失败优雅降级。可加 `AppSettings.auto_update` 门控 + 一个 `check_update` IPC 命令供 UI 手动触发。
5. 发布流程接 CI：构建 → `tauri signer sign` → 上传产物与清单。

## 本次处置
- **不改代码**（避免接死 endpoint 的假完成 / 启动噪音）。
- 如实标记为**外部阻塞**，保留为里程碑3 遗留技术债，待更新基础设施就绪后按上方第 4 步做客户端接线（纯代码、可测）。
