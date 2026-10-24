---
status: done
category: implementation
subject: Maccy 对标 · Phase 2 部分解冻（P2-2/3/6/7 前端键盘子集）+ P1-9 存储页口径
summary: 把 Phase 2 从「整阶段 blocked」拆成「不依赖面板语义 / 依赖面板语义」两半，推进前者：主窗 Enter 与 ⌘1-9 真正粘贴、输入法 composition 防御、数字直达、⌃N/P/J/K 等键盘模型；同时收掉 P1-4 复审挂号的 P1-9（存储页占用口径）。一次 BLOCK 打回（数字直达 50ms 待取用定时器未随其它按键取消，会双发/误粘）修复后复审；另修掉一个既有 flaky，并证伪了审查者给它的错误根因。
evidence:
  commits:
    - f76fc51 fix(clipboard): 主窗 Enter 与 ⌘1-9 真正执行粘贴（P2-7）
    - 4a69788 feat(popover): 输入法 composition 期间不劫持方向键（P2-6）
    - f86d714 feat(popover): 数字直达 1–0 取前 10 条（P2-3）
    - 944e03b feat(popover): 键盘模型升级 ⌃N/P/J/K、⌘↑↓、⌃W/U/H（P2-2 子集）
    - 2577580 fix(popover): ⌥↑↓ 保持原有列表移动行为（P2-2 回归修复）
    - c11f85f fix(popover): 非数字键取消数字直达待取用，消除双发与误粘（P2-3 复审修复）
    - 4b44e37 feat(clipboard): 主窗 ⌥Enter 只复制，与 popover 语义对齐（P2-7 复审修复）
    - 4e3ce3d test(popover): 补双发/preventDefault/连续数字/光标中段位的守护断言
    - 981d410 test(popover): 锁住待取用取消位置与复制路径无降级横幅两个设计点（复审补强）
    - c6391ad fix(storage): 存储页占用口径对齐，进度条改取图片占用（P1-9）
    - 6848d6f test(settings): 修 StoragePanel 阈值用例的竞态 flaky + 收紧上限文案
  commands:
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "pnpm exec tsc -b"
    - "pnpm test"
  urls: []
---

# AI 活动记录 · Phase 2 部分解冻 + P1-9

## 摘要

上一轮结束时 Phase 2 被整阶段标为 blocked（前置是 ADR §6.2 的 8 项人工验证）。本轮先做了一件更该先做的事：**把「等人工」从「整阶段冻结」里拆出来**，依据写进 `docs/design/phase2-gate-verification.md`：

- 面板语义是否成立（不激活 app 也能收键盘、输入法正常、全屏可见）→ 只能真人真机判定，**继续等**；
- webview 层的键盘/交互逻辑 → 与「窗口是不是 NSPanel」无关，即使面板语义被证伪、回退成普通窗口路径也不需要改写，**可以现在做**。

据此推进了 P2-7（主窗键盘流，bug 级）、P2-6（输入法防御）、P2-3（数字直达）、P2-2（键盘模型子集），并把 Phase 1 复审挂号的 P1-9 一并收掉。

## 一个真 BLOCK：数字直达的待取用定时器

数字直达的实现是「按 1–0 → 先高亮目标 → 50ms 后执行取用」。审查者用 fake timers 探针实证：这 50ms 的待取用**只被「另一个数字 / Esc / 卸载」取消**，Enter、⌥Enter、打字、方向键都会留下它继续触发——

```
按 2 后 50ms 内按 Enter  → 同一条目 pasteToFront 两次
按 2 后 50ms 内按 ⌥Enter → 粘贴 + 复制双发
按 2 后立刻打字          → 用户已在搜新词，d2 仍被粘
```

在剪贴板管理器里，这类缺陷的后果是「往用户原应用多贴一次」，会覆盖用户文档里的内容。而且它与代码自己写的不变量注释（「同一时刻只允许一个待取用」）直接矛盾。修法是非数字键在 `handleKeyDown` 顶层统一取消待取用。

执行者做了一个**有依据的偏离**：取消逻辑没有按建议放在 IME guard 之后，而是放在 guard **之前**——理由是「组词期间的按键同样意味着用户在打字，放在 guard 之后这些键会 return 掉、定时器不被取消，等于复现同一个 bug」。取消只清定时器、不 `preventDefault`，不干扰输入法放行。

复审时审查者**主动撤回自己上一轮的建议**，并用探针证明这个位置差异是真实的：

```
PROBE-NOCANCEL → paste: ["d2"]   # 无 cancel：误粘（BLOCK-1 原型）
PROBE-Y         → paste: ["d2"]   # cancel 在 guard 之后：仍然误粘（guard 提前 return 了）
```

但它的变异 R-M2（把 cancel 挪到 guard 之后）当时**测试全绿**——说明这个差异没有测试守护。于是补了最后两条测试：一条锁「组词期间的按键也必须取消待取用」（R-M2 会打红它），一条锁「复制路径不出现 `write_back_only` 降级横幅」（R-M8 变异点）。至此这个位置判词由测试固定，不再依赖任何人的记忆。

## 被明确保留的一个设计争议：组词期间 Esc 关窗

审查者专门为「Esc 是否该在 composition 期间让给输入法」出了判词：**保留现状（Esc 仍关窗）**。论证是——被丢的只是 QuickQuick 自己搜索框里未上屏的拼音，代价极低；而反向改法有死锁失败模式：本 webview 本就会吞掉 Esc 的默认行为，「让 Esc 穿透」并不保证输入法会消费它，结果可能是 Esc 什么都不做、面板关不掉。对瞬时面板而言，「Esc 永远能关窗」是更该保住的不变量。这条决策已被测试锁定，但**仍需真机定性**（见下）。

## 一个被证伪的根因

审查者报告 `StoragePanel.test.tsx` 存在既有 flaky（偶发 `expected 10 MB, received 20 MB`），根因判为「该 describe 的 `beforeEach` 没给 `mockGetImageThreshold` 设默认值」。执行者没有照方抓药，而是先做确定性探针：把 `getImageThreshold` 改成延迟 30ms 返回，用现行断言模式复刻出**逐字相同**的失败；再改成"DOM 断言进 `waitFor`"就稳定变绿。

真实根因是：用例只 `await` 了「mock 被调用就同步断言 DOM」，而 `getImageThreshold()` 是 promise，`await` 之后才 `setState`；并发负载下微任务可能在断言之后才跑完，于是读到组件初始值。补 mock 默认值只是卫生，单独修不好。commit message 因此据实写成「竞态 flaky」，而不是照抄一个错根因进 `git log`。这条与 Engram 里已有的教训（「RTL 偶发收到组件初始值」）是同一类。

## P1-9：占用口径与阈值判据必须同源

P1-4 复审挂号的问题收口：存储页原先拿**库文件体积**去比「图片总量 500 MB」上限，分子分母不是同一个量；SQLite 的 `DELETE` 只把页标记为可复用、不收缩文件，所以图片清理跑完进度条不会下降。

现在拆成两个量：`imageBytes`（图片逻辑字节，直接复用 `image::total_image_bytes`——与自动清理阈值**同一个函数**）画进度条并挂「图片占用上限 500 MB」；`fileSizeBytes`（磁盘占用）单列，并如实写明「VACUUM 前不会缩小」。审查者用 4 组变异确认测试锚定的是行为（改回文件体积 → 2 红；改用内联 SQL 去掉 `is_deleted=0` → 370 vs 870；降级改成传播 → 红；清理后不重拉 → 2 红）。

## 验证（Lead 侧在最终 HEAD 实跑）

```
cargo check --all-targets                → Finished（0 warning）
cargo clippy --all-targets -- -D warnings → Finished（0 warning）
cargo fmt --all --check                  → clean
cargo test                               → 639 passed / 0 failed
pnpm exec tsc -b                         → 无错误
pnpm test × 3 连跑                        → 每次 62 files / 616 tests 全绿
```

## 未闭环

**必须真机手测**（本批新增，按优先级）：

1. **拼音组词中按 Esc**：是「输入法取消组词、面板不关」还是「面板直接关」。这是本批唯一无法静态定性的语义分歧。
2. **组词中按方向键 / Enter / 数字**：确认 `keyCode === 229` 分支在真实 WKWebView 下真的会派发（jsdom 只能验逻辑）。
3. **数字直达 50ms 反馈**是否看得见；条目不足时按数字当前零反馈是否可感知。
4. **`⌃W`/`⌃H`/`⌃U` 后光标位置**：`useLayoutEffect` + `setSelectionRange` 在真实 WKWebView 的时序（jsdom 与真实布局时序不同）。
5. **Enter / ⌘1-9 的真实粘贴落点**与 `write_back_only` 降级横幅。

**仍 blocked 的 Phase 2 条目**：P2-1（面板高度自适应）、P2-4（字母 pin，需迁移框架）、P2-5（cycle 模式）、P2-8（popover 管理动作）、P2-9（唤起位置）——全部依赖面板语义人工验证或迁移框架。

**挂号未立项**：主窗 `⌘0` 取不到第 10 条；`⌃W` 中文无标点长段整段删除；`⇧↑↓` 搜索框内不能选文字（既有问题）；新键位零 UI 提示（归 P2-11 一起做）。
