# QuickQuick 自动更新实现方案（v6）

> 日期：2026-06-05
> 范围：把现有"仅手动检查"的更新脚手架，补成端到端自动更新（后台检查 → 静默下载安装 → 提示重启）。
> 定位：本文是 v6 版本的**冻结设计源**，`docs/dev-log/v6/acceptance.yaml` 的验收标准从本文机械派生。
> 状态：待用户确认冻结。

---

## 一、背景与现状差距

自动更新所需的脚手架已齐全，但"自动"那一环从未实现。逐项核对：

| 环节 | 现状 | 位置 |
|---|---|---|
| updater 插件配置 + 公钥 + endpoint | ✅ 已就绪，endpoint 为真实地址 | `src-tauri/tauri.conf.json:46-51` |
| 插件注册 | ✅ 已注册 | `src-tauri/src/lib.rs:92` |
| 手动检查命令 `check_for_updates` | ✅ 仅查询有无新版，**不下载不安装** | `src-tauri/src/ipc/update.rs` |
| 前端"立即检查更新"按钮 + IPC | ✅ 只显示有无新版文案 | `GeneralPanel.tsx`、`ipc-client.ts:503` |
| `auto_update` 开关（读/写/持久化） | ✅ 后端 get/set 齐、前端已接线 | `settings.rs:67`、`ipc/settings.rs:587` |
| CI 签名 + 生成 `latest.json` | ✅ tag 触发，已产签名清单 | `.github/workflows/release.yml` |
| **后台自动检查** | ❌ setup 阶段无任何调用 | — |
| **下载 + 安装** | ❌ 无 `download_and_install` | — |
| **提示重启 / relaunch** | ❌ 无 process 能力、无重启入口 | — |
| **`auto_update` 开关生效** | ❌ 无代码读它触发检查（摆设） | — |

> ⚠️ `update.rs:6-8` 的设计注释称"endpoint 为占位地址、故不自动检查"——该前提**已过时**：endpoint 现为 `https://github.com/EricChen4263/QuickQuick/releases/latest/download/latest.json`，与 git remote 一致，CI 已产出签名 `latest.json`。本方案落地时需同步修正该注释。

**结论**：基础设施完整，缺的是把"检查—下载—安装—重启"串成自动闭环，并让 `auto_update` 开关真正控制它。

---

## 二、产品决策（已确认冻结）

| 决策项 | 选定 | 含义 |
|---|---|---|
| 更新交互模式 | **静默下载 + 提示重启** | 后台静默完成下载安装，完成后弹非打扰提示「新版本已就绪，点击重启更新」，用户点了才 `relaunch`。 |
| 检查时机 | **启动 + 定时轮询** | 启动后延迟首检，运行期每隔固定间隔再查；全程受 `auto_update` 开关控制。 |
| 版本归属 | **新建 v6** | 按 goal-dev-workflow 走完整版本生命周期（acceptance 冻结 → 大功能 → producer 裁决）。 |

固定参数（实现时写入常量，可在本文 change_log 调整）：

- **首检延迟**：应用启动后 `8s`——避开启动高峰，不与剪贴板轮询/托盘初始化抢资源。
- **轮询间隔**：`6 小时`（21600s）——常驻效率工具，无需更频繁。
- **开关语义**：`auto_update = false` 时，**后台检查/下载/提示全部不发生**；但设置页"立即检查更新"手动入口**始终保留**（关开关只关"自动"，不关"手动"）。

---

## 三、架构方案

### 选型：Rust 后台驱动（而非前端驱动）

理由：① 现有 `check_for_updates` 已在 Rust，保持一致；② 静默后台轮询天然属于 Rust 长驻任务（setup 起 tokio task）；③ 下载安装在 Rust 侧用 `tauri-plugin-updater` 的 `download_and_install`，避免前端长时间持有下载状态。前端只负责"收到就绪事件 → 提示 → 点击重启"。

### 数据/控制流

```
[启动] setup
   └─ 起后台 tokio 任务 update_watcher
        ├─ sleep(首检延迟 8s)
        └─ loop:
             ├─ 读 auto_update 开关 → false 则 sleep(间隔) 后 continue
             ├─ updater.check()
             │     ├─ None  → sleep(间隔), continue
             │     └─ Some(update):
             │           ├─ update.download_and_install(on_chunk, on_finish)  # 静默
             │           │     └─ 可选 emit "update://progress" {downloaded,total}
             │           ├─ emit "update://ready" {version}                   # 完成
             │           └─ 停止本轮循环（已就绪，等用户重启；避免重复下载）
             └─ sleep(间隔)

[前端] App 顶层
   └─ listen("update://ready", ({version}) => 显示非打扰提示条 +「重启更新」按钮)
        └─ 点击 → invoke("restart_app") → Rust app.restart()
```

### 关键 API（tauri v2 / tauri-plugin-updater 2.x）

- `app.updater()? .check().await? -> Option<Update>`（已在用）
- `update.download_and_install(on_chunk, on_download_finish).await?` —— 下载并就地安装；`on_chunk(chunk_len, content_len)` 供进度累加。
- 重启：优先用 Tauri 核心 `app.restart()`（不返回，进程替换重启）；若 capabilities 需要，则评估引入 `tauri-plugin-process` + 前端 `relaunch()`。**落地前需核对 `capabilities/default.json` 是否已含 `core:app` 重启权限**——这是一个实现期需验证的开放点。

### 并发/状态约束

- `update_watcher` 检测到一次就绪后**停止重复下载**（置一个 `AtomicBool already_ready`），避免每轮重复下载同一版本、反复弹提示。
- `download_and_install` 失败（网络中断/校验失败）按本轮失败处理：记日志、不弹提示、等下一轮重试；连续失败不打扰用户（静默策略）。
- 手动"立即检查更新"与后台 watcher 共用底层逻辑，但手动路径**允许向用户展示错误**（区别于后台静默）。

---

## 四、改造点（文件级）

### 后端 Rust

1. **`src-tauri/src/ipc/update.rs`**
   - 修正过时注释（去掉"占位 endpoint"说法）。
   - 抽出可复用的"检查 + 下载安装"内部函数（手动/后台共用）。
   - 新增命令 `download_and_install_update`：供前端手动"发现新版后下载安装"调用，完成后 emit `update://ready`。
   - 新增命令 `restart_app`：调用 `app.restart()`。

2. **`src-tauri/src/lib.rs`**
   - `setup` 内起 `update_watcher` 后台任务（首检延迟 + 定时轮询 + 读 `auto_update` 开关 + 检测就绪后 emit）。
   - `invoke_handler` 注册新命令 `download_and_install_update`、`restart_app`。

3. **`src-tauri/capabilities/default.json`**
   - 核对/补齐重启所需权限（`core:app` 或 process 插件权限）。

### 前端

4. **`src/ipc/ipc-client.ts`**
   - 新增 `downloadAndInstallUpdate()`、`restartApp()` 封装。

5. **App 顶层（全局更新提示）**
   - `listen("update://ready")` → 渲染非打扰提示条（含版本号 + 「重启更新」按钮 + 「稍后」关闭）。
   - 提示条点击「重启更新」→ `restartApp()`。

6. **`src/panels/settings/GeneralPanel.tsx`**
   - 手动检查发现新版后，把当前的纯文案升级为可操作：出现「下载并安装」按钮 → `downloadAndInstallUpdate()`。
   - `auto_update` 开关文案/语义与"后台轮询受控"对齐（开关已接线，无需改 IPC）。

### 不改动

- `tauri.conf.json` updater 配置、CI `release.yml`、`settings.rs` 的 `auto_update` 字段（仅消费它）。

---

## 五、大功能拆分（v6 验收骨架）

> 仅拆到大功能层；小功能在大功能启动时滚动细拆（遵循 goal-dev-workflow）。

- **V6-F1 后端自动更新引擎**
  检查+下载安装内部函数、`download_and_install_update`/`restart_app` 命令、`update_watcher` 后台任务（延迟+间隔+开关受控+就绪 emit+去重）、注释修正、capabilities 权限核对。
- **V6-F2 前端更新提示与手动入口**
  全局 `update://ready` 监听 + 非打扰提示条组件 + 重启交互；GeneralPanel 手动"下载并安装"入口；IPC 封装。

---

## 六、验收要点（供机械派生 acceptance）

| # | 断言 | category | 验证手段（objective 优先） |
|---|---|---|---|
| 1 | `auto_update=true` 时，watcher 在首检延迟后调用 `updater.check()`（可注入/可单测的检查触发逻辑） | 功能正确性 | Rust 单测：watcher 触发逻辑命中检查 |
| 2 | `auto_update=false` 时，watcher 跳过检查/下载/提示 | 功能正确性 | Rust 单测：开关 false 不触发 |
| 3 | 检测到新版 → 调用 `download_and_install` → 完成后 emit `update://ready{version}` | 功能正确性 | Rust 单测（mock updater）/ 事件断言 |
| 4 | 一次就绪后不重复下载（去重标志生效） | 功能正确性 | Rust 单测：already_ready 抑制二次下载 |
| 5 | `restart_app` 命令调用核心重启 API | 功能正确性 | 代码审查 + 命令注册断言 |
| 6 | 前端收到 `update://ready` 显示提示条，点击触发 `restart_app` | 功能正确性 | 前端组件测试（mock listen/invoke） |
| 7 | 手动"立即检查更新"发现新版后可触发下载安装 | 功能正确性 | 前端组件测试 |
| 8 | 本版改动文件无新增 tsc/eslint/clippy 错误（增量口径） | 工程质量 | `cargo clippy` + `tsc`/`eslint` 增量 |
| 9 | 三联留痕齐、commit 回填真 hash | 留痕产出 | dev-log 核对 |
| — | 静默下载体感、提示条动效自然 | 人工确认点 | manual_confirm（录屏挂起，不阻塞） |

> 真机端到端（真实从 GitHub 拉 `latest.json` → 下载 → 重启）依赖发布一个高版本 Release，headless CI 难复现，列为 manual_confirm / 待采证。后端逻辑以 mock updater 单测覆盖。

---

## 七、风险与开放点

- **重启权限**：`app.restart()` 是否需在 capabilities 显式授权，或须改用 `tauri-plugin-process`——实现期首要验证项。
- **下载安装的可测性**：`tauri-plugin-updater` 的 `Update` 不易在单测中构造，watcher 的"检查/下载"应抽成可注入依赖的纯逻辑，把不可测的真实下载隔离到薄封装层。
- **真机验收**：端到端需真实发布一个更高版本 tag 才能跑通，归 manual_confirm；本版 done 判定不依赖它。
- **静默失败策略**：后台连续失败不打扰用户，仅日志；需确认这不掩盖"endpoint 配置错误"——手动检查路径保留报错以便排查。

---

*QuickQuick 自动更新实现方案 · 2026-06-05 · 待确认冻结*
