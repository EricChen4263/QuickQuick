---
id: V6-F2-S01-test
type: test_report
level: 小功能
parent: V6-F2
status: 通过
commit: PENDING
acceptance_ids: [V6-F2-A08]
---

# 测试报告：V6-F2-S01 UpdateBanner 就绪提示条

## 命中校验

**验证命令**：`pnpm test src/components/UpdateBanner.test.tsx --reporter=verbose`

目标测试名精确命中：
```
✓ src/components/UpdateBanner.test.tsx > UpdateBanner > update_banner_shows_on_ready_and_restart_invokes_command
✓ src/components/UpdateBanner.test.tsx > UpdateBanner > 点「稍后」隐藏提示条

Test Files  1 passed (1)
     Tests  2 passed (2)
```

**抗 flaky 多跑 3 次**：每次均 `Tests 2 passed (2)`，无 flaky。

**结论：A08 精确命中，非空匹配，通过。**

---

## 变异 sanity

### 变异① — 移除 restartApp 调用

变异点：`handleRestart()` 函数体改为空函数（`// mutant: restartApp call removed`）。

期望：`update_banner_shows_on_ready_and_restart_invokes_command` 中 `expect(restartApp).toHaveBeenCalledTimes(1)` 应变红。

实际结果（`/artifacts/mutant1.log`）：
```
FAIL  src/components/UpdateBanner.test.tsx > UpdateBanner > update_banner_shows_on_ready_and_restart_invokes_command
AssertionError: expected "spy" to be called 1 times, but got 0 times
```
如期变红。已从备份 `/tmp/UpdateBanner.tsx.bak` 复原（未使用 git checkout）。

### 变异② — 移除 setReadyVersion 调用

变异点：`listen` 回调中注释掉 `setReadyVersion` 调用，组件状态永远为 null。

期望：`update_banner_shows_on_ready_and_restart_invokes_command` 中 `getByText(/1\.2\.3/)` 应找不到元素变红。

实际结果（`/artifacts/mutant2.log`）：
```
FAIL  src/components/UpdateBanner.test.tsx > UpdateBanner > update_banner_shows_on_ready_and_restart_invokes_command
TestingLibraryElementError: Unable to find an element with the text: /1\.2\.3/
```
如期变红（两个测试均 FAIL）。已从备份复原。

**两次变异均证明测试有真实判别力，非恒真/旁路。**

工作树自证：变异后 `git status --porcelain` 与开工快照逐行一致（仅含原有未提交改动，无新增/丢失行）。

---

## 边界探测

1. **cancelled flag + unlisten 清理**：实现正确，沿用 App.tsx 惯例。cancelled flag 防卸载后 Promise.resolve 泄漏，cleanup 函数调 `unlisten?.()` 释放监听。无泄漏风险。

2. **「稍后」后能否再次响应新就绪事件**：`setReadyVersion(null)` 仅隐藏，listen 监听器在组件生命周期内持续活跃（`useEffect([], [])`），再次收到事件会重新显示。行为符合预期。现有测试未覆盖此路径（覆盖缺口，非缺陷）。

3. **版本号 camelCase 对齐**：`interface UpdateReadyPayload { version: string }` 与 Tauri serde camelCase 序列化一致，无对齐问题。

4. **多次就绪事件覆盖**：`setReadyVersion((_prev) => event.payload.version)` 每次覆盖，始终展示最新版本。行为合理。

5. **空版本号边界**（轻微缺口）：若后端 emit `{ version: "" }`，Banner 渲染为 `新版本  已就绪`（空串），前端无防御校验。后端正常路径不产生空版本号，不影响核心功能，不作为门禁阻塞项。

**未发现影响门禁的真实缺陷。**

---

## Artifacts

```
docs/dev-log/v6/f2-update-ui/s01-ready-banner/artifacts/
├── test-a08.log     # 命中校验 verbose 输出（含测试名 + 2 passed）
├── mutant1.log      # 变异①：移除 restartApp 调用 → 变红证明
└── mutant2.log      # 变异②：移除 setReadyVersion 调用 → 变红证明
```

---

## 门禁结论

**通过（放行）**

| 检查项 | 结论 |
|--------|------|
| A08 命中校验（测试名 + N passed） | 通过（`update_banner_shows_on_ready_and_restart_invokes_command` ok，Tests 2 passed） |
| 抗 flaky 多跑 3 次 | 通过（每次全绿） |
| 变异① restartApp 旁路检测 | 通过（如期变红） |
| 变异② setReadyVersion 旁路检测 | 通过（如期变红） |
| 工作树还原自证 | 通过（开工/结束快照逐行一致） |
| 边界探测 | 通过（无阻塞级缺陷） |
