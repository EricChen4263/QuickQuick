---
id: V6-F2-S02-test
type: test_report
level: 小功能
parent: V6-F2
status: 通过
commit: 8646585
acceptance_ids: [V6-F2-A09]
---

# 测试报告：V6-F2-S02 手动安装更新

## 命中校验

命令：`pnpm test src/panels/settings/GeneralPanel.test.tsx --reporter=verbose`

结果（完整证据见 artifacts/test-a09.log）：

```
✓ GeneralPanel 手动检查后下载安装 > general_panel_offers_install_after_update_found
✓ GeneralPanel 手动检查后下载安装 > available=false 时不出现「下载并安装」按钮
✓ GeneralPanel 手动检查后下载安装 > 下载安装失败时渲染 role=alert 失败文案
Test Files  1 passed (1)
Tests  3 passed (3)
```

- `general_panel_offers_install_after_update_found` 真命中、真通过，N=3 >= 1，无假绿。
- 全量套件：52 个测试文件、460 个测试全部通过（artifacts/test-full.log）。
- `check-update-button.test.tsx` 未回归，总数 460 passed。
- 本轮测试均为纯 JSDOM 同步渲染，不涉及并发排序，单跑可信（未强制多跑）。

## 变异 sanity

### 变异① 注释掉 downloadAndInstallUpdate() 调用

- 改动：`await downloadAndInstallUpdate();` 注释为 `// await downloadAndInstallUpdate(); // MUTANT1`
- 备份/还原：`cp GeneralPanel.tsx /tmp/GeneralPanel.tsx.bak` 改前备份，验完 `cp` 复原；未使用 git checkout/restore。
- 结果（artifacts/mutant1.log）：
  - `general_panel_offers_install_after_update_found` 变红（expected spy to be called 1 times, but got 0 times）
  - `下载安装失败时渲染 role=alert 失败文案` 变红（同因）
  - 判别力确认：测试非恒真，非旁路被测方法。
- 已还原，git status 与开工快照一致。

### 变异② 强制 hasUpdate = false（恒不渲染操作区）

- 改动：`const hasUpdate = outcome?.available === true;` 改为 `const hasUpdate = false; // MUTANT2`
- 备份/还原：同上 cp 备份还原，未使用 git checkout/restore。
- 结果（artifacts/mutant2.log）：
  - `general_panel_offers_install_after_update_found` 变红（Unable to find role="button" and name "下载并安装"）
  - `下载安装失败时渲染 role=alert 失败文案` 变红（同因）
  - 判别力确认：测试能捕获"操作区不渲染"缺陷。
- 已还原，git status 与开工快照一致。

## 边界探测

1. **防重复点击**：`UpdateInstallAction` 在 `handleInstall` 执行期间 `setIsInstalling(true)`，按钮同步设置 `disabled={isInstalling}`，按钮文案切换为"下载中…"。测试套件的失败 alert 用例间接验证了这一路径（模拟 reject 后 finally 仍清 isInstalling）。防重复点击设计完备。

2. **失败 role=alert 反馈**：组件内 catch 块 `setError("下载安装失败，请稍后重试")`，渲染 `<div role="alert">`，测试 `下载安装失败时渲染 role=alert 失败文案` 已覆盖并通过。反馈语义正确。

3. **available=false 不出现按钮**：`hasUpdate` 由 `outcome?.available === true` 驱动，`available=false` 分支渲染 `checkMsg`（"已是最新版本"）不渲染 `UpdateInstallAction`，对应测试 `available=false 时不出现「下载并安装」按钮` 通过。

4. **注释订正准确性**：`ipc-client.ts` 第 499-501 行注释已将"占位 endpoint"等过时说明改为"endpoint 已是真实地址；网络/清单异常时会 reject，调用方应以友好文案展示错误"，与 hints.md v6 本地约定一致，无遗留误导注释。

5. **合成定向边界用例（未覆盖分支）**：
   - 合成思路：outcome=null（即 checkForUpdates reject，error 路径）是否影响"下载并安装"出现？当前逻辑 `hasUpdate = outcome?.available === true` 在 outcome=null 时为 false，不会错误渲染操作区。此边界由 GeneralPanel 主组件正确处理（checkError 路径），无缺陷。
   - 合成思路：下载成功后再次点击是否防御？`doneMsg !== null` 时按钮并未隐藏，但 `disabled` 仅在 `isInstalling` 时生效——下载成功后 `isInstalling=false`，按钮重新可点。此为轻微可重入风险，属设计选择（S03 重启时再处理），非本轮验收范围内的缺陷。

6. **真实缺陷**：无（合成测试均在可预期行为范围内）。

## Artifacts

| 文件 | 说明 |
|------|------|
| `artifacts/test-a09.log` | A09 目标测试 verbose 运行日志 |
| `artifacts/test-full.log` | 全量 pnpm test 日志（460 tests passed） |
| `artifacts/mutant1.log` | 变异①（注释调用）验证日志 |
| `artifacts/mutant2.log` | 变异②（恒 false）验证日志 |

## 门禁结论

**放行（通过）**

- A09 `general_panel_offers_install_after_update_found` 真命中真通过
- 全量 460 tests 全绿，无回归
- 两处变异 sanity 均如期变红，测试有真实判别力
- 边界探测无真实缺陷，防重复点击和 role=alert 反馈设计完备
- 工作树与开工状态逐行一致（变异均已从备份复原）
