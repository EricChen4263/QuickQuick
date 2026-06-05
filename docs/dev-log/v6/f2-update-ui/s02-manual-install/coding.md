---
id: V6-F2-S02-code
type: coding_record
level: 小功能
parent: V6-F2
children: []
status: 通过
commit: PENDING
acceptance_ids: [V6-F2-A09]
evidence: [src/panels/settings/GeneralPanel.tsx, src/panels/settings/GeneralPanel.test.tsx, src/ipc/ipc-client.ts, src/panels/settings/check-update-button.test.tsx]
author: coder
---

# V6-F2-S02 manual-install 编码记录

## 做了什么
手动检查发现新版后，把原先的纯文案「发现新版本 X，可前往下载」升级为可操作入口：渲染「发现新版本 X」+「下载并安装」按钮，点击调用 `downloadAndInstallUpdate()`，下载期间按钮 disabled 且文案变「下载中…」，成功反馈「已下载，待重启生效」，失败以 `role=alert` 中文文案展示。重启走 F2/S01 的就绪提示条/restartApp，本处不重复实现。

## 关键决策
- **抽子组件 `UpdateInstallAction`**：把「发现新版后的操作区」（下载安装按钮 + 进度/成功/失败状态）独立成子组件，自持 isInstalling/doneMsg/error 三态。好处：GeneralPanel 主体与各 handler 保持 ≤50 行、职责单一；安装态不污染检查态。
- **检查结果建模为 `UpdateCheckOutcome`**：用 `outcome.available` 决定渲染操作区还是纯文案，available=false 时仅显示「已是最新版本」、不出现下载按钮。
- **错误处理走友好文案**：checkForUpdates / downloadAndInstallUpdate 的 reject 均 catch 后转中文 alert，不向用户暴露原始 error。
- **函数式/不可变**：状态更新均为独立 setState，反馈对象用字面量构造不原地 mutate。

## 改动文件
- `src/ipc/ipc-client.ts`：① 新增 `downloadAndInstallUpdate(): Promise<void>`，内部 `invoke<void>("download_and_install_update")`，沿用既有 try/catch + `toError` 模式。② 订正 `checkForUpdates` 过时 doc 注释：「endpoint 为占位地址时会 reject」→「endpoint 已是真实地址；网络/清单异常时会 reject，调用方应以友好文案展示错误」，与后端 update.rs 订正口径一致。
- `src/panels/settings/GeneralPanel.tsx`：发现新版后渲染 `UpdateInstallAction`；新增 outcome 状态；import downloadAndInstallUpdate。
- `src/panels/settings/GeneralPanel.test.tsx`（新建）：A09 测试 `general_panel_offers_install_after_update_found` + available=false 不出现按钮 + 下载失败 alert 两条增强用例。
- `src/panels/settings/check-update-button.test.tsx`：vi.mock 工厂补 `downloadAndInstallUpdate: vi.fn().mockResolvedValue(undefined)`，保持既有 5 条用例全绿。

## 自测结论
- **红**：先写 GeneralPanel.test.tsx，跑得 2 failed（按钮「下载并安装」未渲染 / downloadAndInstall 未调用），失败因功能缺失非环境错。/tmp/s02-red.log。
- **绿**：实现后 GeneralPanel.test.tsx 3 passed + check-update-button.test.tsx 5 passed = 8 passed。artifacts/vitest-a09.log。
- **重构**：操作区抽 UpdateInstallAction 子组件后测试保持全绿。
- **A09 命中**：`general_panel_offers_install_after_update_found ... ✓`，Tests 3 passed（该文件）。
- **既有不回归**：check-update-button.test.tsx 5 tests 全绿；全量套件见 artifacts/vitest-a09.log。
- **tsc**：`pnpm exec tsc --noEmit` 无新增错误，见 artifacts/tsc.log。
- **注释订正**：已完成（checkForUpdates doc）。

## 合规重构（I01）
- **背景**：review 指出 `GeneralPanel.tsx` 的 `UpdateInstallAction` 函数体越「函数 ≤50 行」硬规则 1 行。
- **修法**：把组件末尾两处条件渲染（`doneMsg` 成功提示、`error` 失败告警）提取为模块级子组件 `InstallFeedback`（props: `doneMsg: string | null` / `error: string | null`，内部按状态择一渲染）。`UpdateInstallAction` return 内改为 `<InstallFeedback doneMsg={doneMsg} error={error} />`。
  - 说明：最初按建议「提为局部 element 变量」，但纯局部变量提取不减少函数总行数（反而增加），无法满足 ≤50 行；遂改为提取独立子组件，真正缩短函数体，且零可见行为变化。
- **行数结果**：`UpdateInstallAction` 重构后函数总 39 行（含签名行+闭括号）、函数体 37 行，满足 ≤50 行。
- **验证**（artifacts/refactor-verify.log）：GeneralPanel.test.tsx 3 passed（A09 等不变）；全量 `pnpm test` 460 passed（52 文件）无回归；`pnpm exec tsc --noEmit` exit 0；无装饰性分隔注释。未改任何测试与可见行为。
