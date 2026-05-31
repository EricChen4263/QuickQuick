---
id: V4-F2-report
type: feature_report
level: 大功能
parent: V4
children: [V4-F2-S06, V4-F2-S07, V4-F2-S08, V4-F2-S09]
created: 2026-06-01T00:50:00Z
status: 已闭合
author: orchestrator
---

# 大功能报告 · V4-F2 主窗口三页 React UI

## 目标
把 V0–V3 的前端纯逻辑接进 React 组件，建出设计§9.3 主窗口：左侧栏一级三入口（剪贴板/翻译/设置）+ 三页真实 UI，接 F1 的 IPC 数据。这是用户报告"主界面空白"问题的正解——渲染层从此真实存在。

## 小功能闭合清单（三联齐 + tester 动态证伪过 + reviewer 放行）

| 小功能 | 内容 | objective 验收 | 结果 |
|---|---|---|---|
| S06 app-shell | 引入 jsdom/@testing-library 渲染测试框架；重建 App.tsx 为侧栏外壳（三入口+切换+route事件集成+Esc+theme）；窗口形态 400×600弹窗→960×640主窗口 | V4-F2-A06 | ✅ app-shell 6；2 变异如期变红 |
| S07 clipboard-page | 列表+预览双栏，listClipItems 取数，搜索/筛选/键盘流复用 search/filter/keyboard，收藏/删除 IPC | V4-F2-A07 | ✅ clipboard-page 10（R1 修错误静默+cleanup）；3 变异如期变红 |
| S08 translate-page | 工作区+翻译历史右栏三栏，translateText+listTranslateHistory，copy/speak/操作集，历史回填 | V4-F2-A08 | ✅ translate-page 10（R1 修 copy/speak 静默+历史日志）；3+1 变异如期变红 |
| S09 settings-page | 六子项纵向栏；改键 validateRebind 冲突拒绝；隐私排除名单 add/removeExcludedApp；翻译源选择 | V4-F2-A09 | ✅ settings-page 10（R1 修翻译源错误态分离）；3+1 变异如期变红 |

## 关键架构决策
- **渲染测试框架**（S06 引入）：jsdom + @testing-library/react + jest-dom，把"UI 真挂载、交互真生效"变成可机检 objective 门禁——堵住 V0–V3"渲染全归 manual"导致主窗口长期未建的根因（CL-V4-001）。全量前端测试从 106 增至 142，无回归。
- **错误处理纪律统一**：三页所有 async handler 一律 try/catch（失败 role=alert 不静默、成功清旧错误）、取数 useEffect 用 cancelled flag + cleanup（防卸载后 setState）。S07/S08 经 reviewer R1 补齐，S09 从一开始即遵循——同类问题逐页收敛。
- **逻辑复用零重造**：search.filterBySearch/filter.filterByType/keyboard.* / translate-actions.* / rebind.validateRebind / sections.* / nav.* 全部复用，组件只做渲染+交互编排+IPC 接线。
- **窗口形态变更**（S06）：400×600 无边框弹窗 → 960×640 可调整带边框主窗口（保留 visible:false 托盘唤起），以容纳侧栏三页布局（§9.3"类 macOS 系统设置"主窗口）；§8 选中即译浮窗属另一独立窗口/未来项，本版不处理。

## 对应资源/契约
- 三页通过 data-testid `page-clipboard`/`page-translate`/`page-settings` 挂载于 App 外壳。
- 全部经 F1 的 `src/ipc/ipc-client.ts` 12 函数取数/管理。
- 复用 `src/theme/theme.css` 的 `--qq-*` 设计 token。

## 归 manual（并入 pending-manual.yaml）
- V4-F2-A10：主窗口三页视觉还原（列表+预览/三栏/子项栏布局，峡湾青蓝、随系统明暗）——结构已 objective 测（渲染+交互），纯视觉审美需运行确认。
- 真实数据/持久化/网络往返见 V4-F1-A02-H01 / A04-H01。

## follow-up（非阻断，已记录）
- ClipboardPage 成功后清 opError；cancelled guard 在 lazy-mount 后补专测；deleteClipItem 变异测试。display:none 常驻挂载下当前无实际影响。
