---
id: V4-F1-report
type: feature_report
level: 大功能
parent: V4
children: [V4-F1-S01, V4-F1-S02, V4-F1-S03, V4-F1-S04, V4-F1-S05]
created: 2026-06-01T00:45:00Z
status: 已闭合
author: orchestrator
---

# 大功能报告 · V4-F1 IPC 桥接 + 启动数据管道

## 目标
把 V0–V3 各自建好的「后端业务逻辑」与「前端纯逻辑」两座孤岛，经 IPC 命令层 + 启动数据管道 + 前端封装层接通，使 F2 三页 UI 可向后端取数/管理。

## 小功能闭合清单（均三联留痕齐 + tester 动态证伪过 + reviewer 无未决高危）

| 小功能 | 内容 | objective 验收 | 结果 |
|---|---|---|---|
| S01 clip-cmd | 剪贴板 IPC 命令（list/delete/toggle_favorite）+ AppDb 状态 + 可单测 impl 模式 | V4-F1-A01 / A14(剪贴板部分) | ✅ ipc_clipboard 6 + ipc_input_validation 6 |
| S02 translate-cmd | 翻译 IPC（translate_text/list_translate_history）+ HttpExecutor 注入抽象 + UreqExecutor + 方向编排 | V4-F1-A02 | ✅ ipc_translate 6（FakeExecutor 隔离网络）|
| S03 settings-cmd | 设置 IPC（热键/排除名单/翻译源 持久化往返，7 命令）+ AppSettings 文件持久化 | V4-F1-A03 | ✅ ipc_settings 7 |
| S04 boot-pipeline | 启动开库(keyprovider)+轮询(arboard)→ingest 装配 + 注册全部 12 命令 + register_hotkeys 读持久化 | V4-F1-A04 / A14 | ✅ boot_pipeline 4；整体编译 exit0 |
| S04-R1 | DB 不可用守卫：AppDb→Option，with_db helper，命令优雅 Err 不 panic（修 reviewer I-1/I-2）| —（A04 健壮化）| ✅ ipc_clipboard 8（含 2 守卫）|
| S05 ipc-client | 前端 src/ipc 类型化 invoke 封装层（12 函数 + DTO 类型，命令名/参数名/camelCase 100% 对齐 Rust）| V4-F1-A05 | ✅ ipc-client 20 |

## 关键架构决策
- **命令薄包装 + 可单测 impl(conn)**：所有 IPC 命令 = `#[tauri::command]` 薄壳 + 纯函数 impl；单测只测 impl（传临时库/路径/fake），不依赖 Tauri 运行时。
- **依赖注入隔离不可测边界**：翻译用 `HttpExecutor`（FakeExecutor 测/UreqExecutor 生产）、开库用 `KeyProvider` trait、捕获用 `ClipboardBackend` trait——真实网络/keychain/GUI 全部隔离到生产实现，objective 测试零外部依赖。
- **优雅降级永不崩**（设计§六）：开库失败 manage(None)，命令经 with_db 返回"数据库不可用"而非 dispatch panic——避免重蹈 V3 后 autostart 启动崩的覆辙（复盘 P1）。

## 已 objective 闭合
A01/A02/A03/A04/A05/A14 全部 objective 项独立 tester 动态证伪通过（命中校验真命中 + 共 ≥11 处变异如期变红 + 边界含 SQL 注入/非法 JSON/DB 不可用 + git 快照逐行一致）。

## 归 manual（并入 pending-manual.yaml，不阻塞）
- V4-F1-A02-H01：翻译真实 provider 网络往返
- V4-F1-A04-H01：真实 keychain 开库 + arboard 捕获 + 轮询线程 + 12 命令真实 invoke 往返 + 热键持久化重启生效

## 对 F2 的交付
前端 `src/ipc/ipc-client.ts` 暴露 12 个类型化函数供三页调用：剪贴板页用 listClipItems/deleteClipItem/toggleFavoriteClip；翻译页用 translateText/listTranslateHistory；设置页用 getHotkeys/setHotkey/getExcludeList/setExcludeList/getTranslateProviders/get·setSelectedProvider。
