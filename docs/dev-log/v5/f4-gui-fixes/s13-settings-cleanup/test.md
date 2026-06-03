---
id: s13-settings-cleanup
title: 设置页占位清理 测试留痕
status: passed
commit: pending
date: 2026-06-03
---

# 测试留痕：设置页占位清理（s13）· 动态证伪

## 开工 git status 快照

```
 M src/panels/settings/HotkeyPanel.tsx
 M src/panels/settings/SettingsPage.tsx
 M src/panels/settings/settings-page.test.tsx
```

---

## 一、命中校验（杀假绿）

全量前端：`Test Files 43 passed (43)` / `Tests 366 passed (366)`。

两条目标测试真实命中（非空匹配）：

| 测试 | 结果 |
|---|---|
| `热键面板不渲染「回车粘贴」占位开关（已移除本地占位）` | ✓ passed |
| `关于面板版本号从 getVersion 读取（非硬编码 v1.0.0）` | ✓ passed |

---

## 二、变异 sanity（杀恒真/旁路）

### 变异A — 占位移除测试有判别力

把「回车粘贴」SettingToggle 块（含 import + state）**加回** HotkeyPanel.tsx。

结果：「热键面板不渲染『回车粘贴』占位开关」如期变红（`expect(element).not.toBeInTheDocument()` 1 failed）。还原后复绿 26 passed。

### 变异B — 版本号测试有判别力

把 SettingsPage.tsx 的 `versionText` 改回硬编码 `"v1.0.0 · Tauri 2.0"`（绕过 getVersion）。

结果：「关于面板版本号从 getVersion 读取」变红，且「关于面板含 .logo-mark 与版本号」也变红（同样校验非硬编码版本），2 failed。还原后复绿 26 passed。

两变异均证明测试有真实判别力，非恒真/旁路。备份还原后 3 个 M 文件与开工快照逐行一致。

---

## 三、边界探测

- **getVersion 失败路径**：catch 仅 `console.error`，`version` 保持 null → 走占位 `v… · Tauri 2.0`，不崩溃。降级正确（catch 不 setState）。未崩溃路径无测试覆盖，属可接受降级。
- **cancelled 守卫**：cleanup 置 `cancelled.current=true`，`.then` 内 `if (!cancelled.current) setVersion(v)`，范式与既有代码一致，无卸载后 setState。
- **SettingToggle 本体未误伤**：`SettingToggle.test.tsx` 9 测试全绿；PrivacyPanel/GeneralPanel 仍正常引用；settings 目录全量 8 files / 67 tests 全绿。

---

## 四、最终门禁结论

**PASS（放行）**

- 2 目标测试命中，366 全量全绿
- 变异A/B 均如期变红再复绿，测试有判别力
- getVersion 失败降级、cancelled 守卫、SettingToggle 本体完好均确认
- 工作树还原干净，无残留业务代码改动

## 结束 git status --short（业务代码部分）

```
 M src/panels/settings/HotkeyPanel.tsx
 M src/panels/settings/SettingsPage.tsx
 M src/panels/settings/settings-page.test.tsx
```

与开工快照一致，无残留。
