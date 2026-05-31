# V4-F1-S05 IPC 封装层 动态证伪测试报告

- 日期：2026-05-31
- 被测文件：`src/ipc/ipc-client.ts`
- 测试文件：`src/ipc/ipc-client.test.ts`（20 个测试）
- 验收项：V4-F1-A05，verify: `pnpm test ipc-client`

---

## Phase 1：命中校验（杀假绿）

**命令：** `pnpm test ipc-client`（重定向 /tmp/test-ipc-client.log，grep 结论）

**结果：**
```
 ✓ src/ipc/ipc-client.test.ts (20 tests) 4ms
 Test Files  1 passed (1)
      Tests  20 passed (20)
```

- Test Files：1 passed (1) — 真命中，非 0 passed 假绿
- Tests：20 passed (20) — 与预期 N=20 完全吻合
- 结论：**命中校验通过**

---

## Phase 2：变异 sanity（杀恒真/旁路）

开工 git 快照：`?? docs/dev-log/v4/f1-ipc/s05-ipc-client/`、`?? src/ipc/`（两条未追踪，其余干净）

改前备份：`cp src/ipc/ipc-client.ts /tmp/ipc-client.ts.bak`

### 变异 1：改坏命令名 `toggle_favorite_clip` → `toggle_favorite_XXX`

- 改动：第 83 行命令名字符串替换
- 跑结果：`Tests 2 failed | 18 passed (20)`，失败用例：
  - `toggleFavoriteClip > 传入正确命令名、id、favorite 参数（true）`
  - `toggleFavoriteClip > 传入 favorite=false 时参数正确`
  - AssertionError：expected `toggle_favorite_clip`，got `toggle_favorite_XXX`
- 结论：**如期变红，测试真实校验命令名**
- 还原：`cp /tmp/ipc-client.ts.bak src/ipc/ipc-client.ts`

### 变异 2：改坏参数映射 — `invoke("toggle_favorite_clip", { id, favorite })` → `invoke("toggle_favorite_clip", { id })` 漏传 `favorite`

- 改动：第 83 行去掉 `favorite` 参数
- 跑结果：`Tests 2 failed | 18 passed (20)`，失败用例：
  - `toggleFavoriteClip > 传入正确命令名、id、favorite 参数（true）`
  - `toggleFavoriteClip > 传入 favorite=false 时参数正确`
  - AssertionError：参数对象缺少 `favorite` 键
- 结论：**如期变红，测试真实校验参数完整性**
- 还原：`cp /tmp/ipc-client.ts.bak src/ipc/ipc-client.ts`

### 变异 3：改坏错误重抛 — `toError` 直接 `return cause as Error`，不包装成 Error 实例

- 改动：将 `toError` 函数体替换为 `return cause as Error;`（raw string 直接返回）
- 跑结果：`Tests 5 failed | 15 passed (20)`，失败用例：
  - `listClipItems > invoke reject 字符串时重抛为 Error 且含原始消息`
  - `toggleFavoriteClip > invoke reject 字符串时重抛为 Error`
  - `translateText > invoke reject 字符串时重抛为含原始消息的 Error`
  - `setHotkey > invoke reject 字符串时重抛为含原始消息的 Error`
  - `setSelectedProvider > invoke reject 字符串时重抛为含原始消息的 Error`
  - 均为 `toBeInstanceOf(Error)` 失败
- 结论：**如期变红，错误路径测试真实校验 Error 包装**
- 还原：`cp /tmp/ipc-client.ts.bak src/ipc/ipc-client.ts`

**结束 git 快照：** `?? docs/dev-log/v4/f1-ipc/s05-ipc-client/`、`?? src/ipc/`  
与开工逐行一致，工作树干净，无未还原改动。

---

## Phase 3：边界探测

**方法：** 临时写入 `src/ipc/boundary-probe.test.ts`（4 个边界用例），跑完后删除。

| 边界场景 | 预期 | 实际 | 结果 |
|---|---|---|---|
| `translateText("你好")` 不传 target，invoke 参数中 `target` 键是否存在 | `Object.hasOwnProperty("target") === true`，值为 `undefined`（显式传 undefined，非省略键） | 通过 | 优雅，Rust 侧可区分"未传"语义 |
| `listClipItems()` 返回空数组 `[]` | 透传 `[]` | 通过 | 空数组透传正常 |
| `invoke` reject 数字 `42` 时重抛类型 | `instanceof Error`，`message === "42"` | 通过 | `String(42)` 正确 |
| `invoke` reject `null` 时重抛类型 | `instanceof Error`，`message === "null"` | 通过 | `String(null)` 正确 |

**结论：** 4 个边界全部优雅处理，无 panic、无静默错误。  
实现中 `translateText` 显式传 `{ text, target }` 哪怕 target 为 undefined——此为有意设计，使 Rust 侧可区分"调用方未传 target"与"调用方传了某值"。

临时文件已删除，git 快照确认干净。

---

## 门禁结论

**放行。**

- 命中校验：20/20 passed，Test Files 1/1 passed，无假绿
- 变异 sanity：3 处改坏均如期变红（命令名校验 / 参数完整性校验 / 错误包装校验），证明测试有真实判别力，非恒真/旁路
- 边界探测：4 个边界全部优雅处理
- 工作树还原：git 快照开工/结束逐行一致，无残留改动
