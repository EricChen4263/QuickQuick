---
id: s13-settings-cleanup
title: 设置页占位清理（移除回车粘贴占位 + 关于页版本号读真实值）
status: done
commit: d6c9ae8
date: 2026-06-03
---

## 来由：设置页端到端审计

对设置页 6 个分区（常规/热键/翻译源/隐私/存储/关于）做了端到端只读审计，逐项追踪 UI→handler→IPC→后端→持久化/运行时生效。结论：

- **完全可用（代码级）**：开机自启、托盘常驻、暂停捕获、跳过敏感、排除名单、翻译源选择、图片阈值、库体积/条目数显示、清理历史、热键改键（历史/翻译）。多项用 `Arc<AtomicBool>`/`Arc<RwLock>` 运行时即时生效。
- **外部阻塞**：自动检查更新 / 立即检查更新——代码全实现，但 updater endpoint 占位、无真实服务器（dev-log s05 已记录，非代码缺陷，本次不动）。
- **需处理**（本 s13 收口）：
  1. 热键页「回车粘贴」开关——纯本地 `useState`、无 IPC、刷新即丢、不影响实际粘贴行为（注释自述"里程碑3接入前的本地占位"）。
  2. 关于页版本号硬编码 `v1.0.0`，与实际构建 v0.0.1 不符。

用户决策：「回车粘贴」**移除占位**（不补全功能，待真正需要时再正式做）；版本号**改读真实值**。

## 改动文件（3 个，纯前端）

### `src/panels/settings/HotkeyPanel.tsx`（移除占位）
- 删除 `enterToPaste` 本地 state 及其注释。
- 删除渲染它的整个 `<SettingGroup><SettingToggle label="回车粘贴" …/></SettingGroup>` 块及注释。
- 移除本文件不再使用的 `SettingToggle` import。
- **保留** `SettingGroup` import（热键行仍用）；**未动** `SettingToggle.tsx` 组件本体（隐私/常规页仍在用）。

### `src/panels/settings/SettingsPage.tsx`（版本号读真实值）
- `AboutPanel` 新增 `version` state + `useEffect` 调 `getVersion()`（`@tauri-apps/api/app`，读 tauri.conf.json 真实版本），cancelled 守卫防卸载后 setState，catch 仅 `console.error` 不崩溃。
- 显示：`v{version} · Tauri 2.0`；加载完成前占位 `v… · Tauri 2.0`。
- `@tauri-apps/api` 已是 package.json 直接依赖（`^2.0.0`），此前未用过 `app` 子模块。

### `src/panels/settings/settings-page.test.tsx`（测试）
- mock `@tauri-apps/api/app` 的 `getVersion` 返回 `"0.0.1"`，`beforeEach` 设默认 mock。
- 既有「关于面板含版本号」测试断言从 `v1.0.0` 改为 `waitFor` + `v0.0.1`。
- 新增 2 条：「热键面板不渲染『回车粘贴』占位开关」（`queryByText` 断言不存在）、「关于面板版本号从 getVersion 读取（非硬编码 v1.0.0）」。

## TDD 红绿

**RED**：版本号测试期望 v0.0.1 但实际硬编码 v1.0.0（红）；占位移除测试因「回车粘贴」仍在 DOM（红）。共 3 红。

**GREEN**：移除占位 + 接 getVersion 后全绿。

## 实跑输出摘要

```
# 前端全量
Test Files  43 passed (43)
      Tests  366 passed (366)

# TypeScript
pnpm tsc --noEmit: No errors
```

## 已知边界 / 未动项
- **关于页 "· Tauri 2.0" 文案**仍为静态描述（非读 Tauri 运行时版本），只有应用版本号改为动态——应用版本是这次"诚实性"的关注点，框架版本标签保留。
- **自动更新外部阻塞**未动，留待真实 infra 就绪（dev-log s05）。
