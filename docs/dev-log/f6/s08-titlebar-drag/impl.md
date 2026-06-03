# impl.md — f6/s08 主界面标题栏无法拖动移窗修复

**日期**：2026-06-04
**类型**：bug 修复（capabilities ACL 授权缺失）
**改动文件**：`src-tauri/capabilities/default.json`（加权限）、`src/shell/capabilities.test.ts`（新增防回归测试）

---

## 1. 问题现象

主窗口（main）标题栏无法拖动移窗。鼠标按住标题栏拖拽，窗口纹丝不动。

`src/shell/TitleBar.tsx` 已正确加 `data-tauri-drag-region` 属性，CSS 也正常（既有 `TitleBar.test.tsx` 断言该属性存在并通过），从前端代码看不出问题。

## 2. 根因

`data-tauri-drag-region` 底层调用 Tauri 的 `startDragging` JS API，该 API 受 capabilities ACL 管控。

`src-tauri/capabilities/default.json` 的 `permissions` 数组只授了：

- `core:window:allow-hide`
- `core:window:allow-show`
- `core:window:allow-set-focus`

**缺 `core:window:allow-start-dragging`**。`core:default` 不含该权限（项目已显式逐个补窗口权限即是佐证），因此拖动调用被 ACL 静默拒绝——无报错、无日志，整条标题栏拖不动。

这与此前 popover 窗口 JS API 被 ACL 静默拒的坑同源：capabilities 未显式授权时，window JS API 调用被静默吞掉而非抛错，极难从前端排查。

## 3. 修复内容

在 `default.json` 的 `permissions` 数组、紧挨现有 `core:window:*` 权限处加入：

```json
"core:window:allow-start-dragging"
```

`windows` 数组已含 `"main"`，无需改动。未触碰 `TitleBar.tsx` / CSS / 其它代码。

## 4. TDD 测试说明（红→绿）

新增 `src/shell/capabilities.test.ts`，锁住真正的根因（ACL 授权），防止未来有人删权限导致拖动再次静默失效：

- 用 Node `fs` 读取 `src-tauri/capabilities/default.json`（vitest 跑在 node/jsdom 环境，路径从 `process.cwd()` 即项目根解析）。
- 断言 `permissions` 数组 `toContain("core:window:allow-start-dragging")` —— 断言具体权限项存在，非恒真断言（不只判文件存在）。

**RED**（加权限前）：
```
AssertionError: expected [ 'core:default', …(10) ] to include 'core:window:allow-start-dragging'
```
测试因权限缺失而失败，非语法/环境错。

**GREEN**（加权限后）：测试通过，全量套件 436 passed / 0 failed。

既有 `TitleBar.test.tsx`（断言 drag-region 属性存在）保留不动，两层测试互补：前者锁前端属性，本测试锁后端 ACL 授权。

## 5. 验证结果

### 前端 vitest（`npx vitest run`）
- 全量套件：**436 passed，0 failed**（EXIT=0）。
- 原始日志：`artifacts/vitest_red.log`（红）、`artifacts/vitest_green.log`（绿）。

### 后端 cargo build（`src-tauri/`）
- Tauri 编译期校验 capabilities 权限标识符合法性。`core:window:allow-start-dragging` 若拼写/标识符非法会编译失败。
- 结果：**build 通过**（EXIT=0，`Finished dev profile ... in 26.09s`），证明 `core:window:allow-start-dragging` 被 ACL schema 接受、标识符合法。
- 原始日志：`artifacts/cargo_build.log`。

## 6. 偏离方案处

无。严格按指派方案执行：仅改 `default.json` + 新增测试 + dev-log。
