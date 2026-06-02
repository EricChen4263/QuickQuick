---
id: s10-event-const
title: "动态证伪报告：抽取事件名常量（I-01，纯重构）"
status: passed
commit: PENDING
date: 2026-06-02
---

# 动态证伪报告：s10 抽取事件名常量

## 开工状态快照

```
 M src-tauri/src/lib.rs
 M src/panels/clipboard/ClipboardPage.tsx
 M src/panels/clipboard/clipboard-page.test.tsx
?? docs/dev-log/v5/f4-gui-fixes/s10-event-const/
?? src/ipc/events.ts
```

---

## 1. 命中校验（回归安全网）

### clipboard-page 全套测试

命令：`./node_modules/.bin/vitest run --reporter=verbose src/panels/clipboard/clipboard-page.test.tsx`

结果：**20 passed (20)，0 failed**

关键测试命中确认：
- `✓ 收到 clipboard-changed 事件后触发 listClipItems 重新加载`（s08 事件驱动）
- 全部 20 个测试均为 `✓`，非空跑（Tests 20 passed (20)）

### TypeScript 类型检查

命令：`./node_modules/.bin/tsc --noEmit`

结果：**EXIT:0，无任何错误或警告**

### Cargo check（后端编译）

命令：`cd src-tauri && $HOME/.cargo/bin/cargo check`

结果：`Finished 'dev' profile [unoptimized + debuginfo] target(s) in 0.34s`，**EXIT:0，无 warning/error**

---

## 2. 变异 Sanity（判别力验证）

### 前端变异：listen 偏离常量

操作：
1. `cp ClipboardPage.tsx /tmp/ClipboardPage.tsx.bak`（备份）
2. 将 `listen(CLIPBOARD_CHANGED_EVENT, ...)` 改为 `listen("wrong-event-name", ...)`
3. 重跑 clipboard-page 测试

变异后结果：**1 failed | 19 passed (20)**

失败用例：`收到 clipboard-changed 事件后触发 listClipItems 重新加载`

错误信息：
```
AssertionError: expected "spy" to be called with arguments: [ 'clipboard-changed', Any<Function> ]
-   "clipboard-changed",
```

结论：**测试如期变红，判别力确认有效，非恒真/非旁路。**

还原：`cp /tmp/ClipboardPage.tsx.bak ClipboardPage.tsx`，确认 `listen(CLIPBOARD_CHANGED_EVENT, ...)` 已还原。

### 后端变异：emit 使用字面量

操作：
1. `cp lib.rs /tmp/lib.rs.bak`（备份）
2. 将 `handle.emit(CLIPBOARD_CHANGED_EVENT, ())` 改为 `handle.emit("xxx", ())`
3. 重跑 `cargo check`

变异后结果：`Finished 'dev' profile in 1.18s`，**EXIT:0，仍编译**

结论：符合预期——后端无针对 emit 参数的单测（运行时行为，Rust 编译器不验证字符串值）；主要目的是确认重构后后端无编译回归，已达成。

还原：`cp /tmp/lib.rs.bak lib.rs`，确认 `handle.emit(CLIPBOARD_CHANGED_EVENT, ())` 已还原。

---

## 3. 跨语言一致性核对

| 端 | 文件 | 常量声明 | 值 |
|---|---|---|---|
| 前端 | `src/ipc/events.ts:4` | `export const CLIPBOARD_CHANGED_EVENT = "clipboard-changed" as const;` | `"clipboard-changed"` |
| 后端 | `src-tauri/src/lib.rs:44` | `const CLIPBOARD_CHANGED_EVENT: &str = "clipboard-changed";` | `"clipboard-changed"` |

**值一致，通过。**

### 注释互指评估

- **前端** (`events.ts:2-3`)：
  ```
  // 注意：与后端 src-tauri/src/lib.rs 的 CLIPBOARD_CHANGED_EVENT 常量必须保持一致。
  // Tauri 事件名跨语言无法编译期共享，改动需两端同步。
  ```
- **后端** (`lib.rs:42-43`)：
  ```
  /// 剪贴板变化事件名。与前端 src/ipc/events.ts 的 CLIPBOARD_CHANGED_EVENT 必须一致。
  /// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
  ```

评估：注释**到位**——两端均明确指向对方文件路径、说明了 Tauri 固有限制（无编译期共享）、强调了改动需两端同步。这是此类跨语言常量能做到的最佳文档化方式。

---

## 4. 结束状态快照

```
 M src-tauri/src/lib.rs
 M src/panels/clipboard/ClipboardPage.tsx
 M src/panels/clipboard/clipboard-page.test.tsx
?? docs/dev-log/v5/f4-gui-fixes/s10-event-const/
?? src/ipc/events.ts
```

与开工快照**逐行一致**，无任何残留变异。

---

## 5. 门禁结论

**放行。**

- 命中校验：20/20 通过，s08 事件驱动测试真命中，非空跑
- 变异 sanity：前端偏离常量后测试如期变红（判别力有效），已还原
- 后端变异后仍编译（无回归），已还原
- 跨语言一致性：两端值均为 `"clipboard-changed"`，注释互指完整到位
- tsc --noEmit：无错误；cargo check：无 warning/error
- 工作树与开工快照一致，无残留

此重构属纯行为无变化重构，测试安全网完整有效。
