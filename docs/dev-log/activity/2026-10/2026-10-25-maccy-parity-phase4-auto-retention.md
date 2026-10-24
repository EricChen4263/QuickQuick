---
status: done
category: implementation
subject: Maccy 对标 · Phase 4 自动淘汰 + 可配历史上限（P4-4）
summary: 把「手动清理才生效、保留条数硬编码 500」改成「入库成功即自动淘汰 + 设置页可配 1–999（默认 500）+ 收藏项既不计数也不删除」；淘汰挂在本轮确有新增行之后，常态轮次零额外 SQL，上限调小立即收敛并回报淘汰条数。
evidence:
  commits:
    - 7df0e60 feat(storage): 入库自动淘汰 + 可配历史上限 1–999（P4-4）
  commands:
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "cd src-tauri && cargo fmt --all --check"
    - "pnpm exec tsc -b"
    - "pnpm test"
  urls: []
---

# AI 活动记录 · 自动淘汰 + 可配历史上限（P4-4）

## 摘要

- **设置**：新增 `AppSettings.max_history_items`（1–999，默认 **500**），有设置页 UI（存储面板数字输入框）、有后端持久化、真的决定淘汰行为。
- **自动淘汰**：挂在 `capture_and_ingest` 写库事务 RELEASE 之后，判据是「本轮有任意类型的 `Inserted`」；超过上限时淘汰最旧的**非收藏**条目（收藏既不计数也不删）。
- **手动清理同一口径**：删掉 `KEEP_RECENT_COUNT`（硬编码 500），改由命令层从设置读上限，手动与自动不再各持一个数字。

## 三个必须写下来的判断

**1. 默认值选 500，不是 Maccy 的 200。**
本项之前「立即清理非收藏历史」就是按 500 条裁剪，沿用默认值让存量用户升级后历史规模不变（改成 200 会在升级那一刻凭空丢掉 300 条历史，属于静默数据损失）。设置页此前也没有对条目数做过具体数字承诺——页面上那个 `500 MB` 是**图片占用**上限（P1-9 已把图片占用 / 库文件体积 / 条数拆成三个口径），和条数不是同一个量。要对齐 Maccy 只改 `DEFAULT_MAX_HISTORY_ITEMS` 一个常量，已写进注释与 work-items。

**2. 「pin 豁免」在本项目里只能是 `is_favorite` 豁免，而且这是个近似。**
字母 pin 是 P2-4，仍 blocked（需要 `pin` 列 + 迁移）。当前的收藏语义是「置顶 + 豁免清理」，正好覆盖「豁免」这一半，所以淘汰判据用 `is_favorite = 0`。但两者不是同一件事：P2-4 落地时若要求「字母 pin 与收藏分别豁免 / 分别计数」，必须新增独立列并让它进同一条豁免条件，**不能把 pin 塞进 `is_favorite`**。这条已作为风险登记在 work-items 的 P4-4 条目里，避免后来人顺手复用。

**3. 参数收进结构体，顺手删掉一条只有测试走的路径。**
`capture_and_ingest` 原本 5 个位置参数，最后一个是 `max_image_bytes: u64`。再加一个 `max_history_items: usize` 就变成 6 个位置参数、且两个参数都是数字但单位不同（字节 / 条数），是最典型的「传反了也编译通过」的形状。收进 `IngestLimits` 之后，测试要注入的 `CleanupPolicy` 也进了同一个结构体，于是原来那个只为测试存在的私有变体 `capture_and_ingest_with_cleanup` 就没必要存在了——删掉它，测试改成直接跑生产函数。

## 触发判据为什么是「本轮有新增行」而不是「每轮都查」

条数只能被 `Inserted` 推高：`Bumped`（同内容重复复制）只刷新 `last_modified_utc`，条数不变；空轮次在 `clips.is_empty()` 就已经返回。所以常态（未超限）的代价被压到「每次真的复制新内容才付一次 `COUNT(*)`」，而不是每 500ms 一次。这一跳是真的被测试钉住的：

- 去掉触发 → 3 条红；
- 判据退化成「本轮非空」（把去重轮次也算进来）→ 2 条红（其中一条就是「去重命中不触发淘汰」）。

`COUNT(*)` 还顺手配了查询计划测试（必须命中 `idx_clip_items_live_favorite_modified` 前缀、不得 `SCAN clip_items`），因为它是常态轮次唯一会执行的那条 SQL，退化会直接落在捕获热路径上。

## 上限调小必须立刻生效

只等「下一次复制」会让「上限已写 50、列表还有 400 条」同时成立——这正是本系列一直在还的那笔债（UI 承诺、代码没接线）。所以 `set_max_history_items` 在落盘后立刻按新上限裁剪，并把淘汰条数返回给前端显示「已按新上限清理 N 条最旧的非收藏记录」；真的有条目被删时还会发 `clipboard-changed`，让常驻的列表页跟着刷新。

裁剪失败（写锁冲突等）只留痕、不把设置回滚：设置已经落盘，下一次入库还会再收敛一遍；为一次裁剪失败把用户的设置退回原值，代价更大。这条用触发器造出真实失败验证过（`set_max_history_items_keeps_settings_when_trim_fails`）。

## 非法值三道防线

1. 前端：`^\d+$` + 1..999，**不做「悄悄取整」**——`2.5` 直接拒收，而不是 `parseInt` 成 2 让用户以为自己设了 2.5 条；失焦/回车提交，非法时把输入框回滚到已生效值（「输入框显示的值必须等于真正生效的值」）。
2. 命令层：越界拒收且**不落盘**（测试先写 300、再试 0/1000，断言文件里还是 300 ——只断言 `Err` 无法证明没落盘）。
3. 读侧：`effective_max_history_items()` 把手工改坏的 0 / 10000 收敛回 1 / 999。0 收敛到 **1** 而不是「保持 0」：保持 0 等于每次复制都清空历史，那是数据损失而不是「严格」。

## 验证（实跑）

```
cd src-tauri && cargo test                         → 480 passed（lib）+ 全部集成套件，0 failed
cd src-tauri && cargo clippy --all-targets -- -D warnings → No issues found
cd src-tauri && cargo fmt --all --check            → 干净（只 format 本 item 涉及的文件，未批量格式化他人未提交文件）
pnpm exec tsc -b                                   → 无错误
pnpm test                                          → 74 files / 766 tests 全绿
```

### 变异验证（6 处全部打红，未留在最终代码里）

| 变异 | 结果 |
|---|---|
| 摘掉入库路径的淘汰触发（`if false &&`） | 3 红 |
| 候选 SQL 去掉 `is_favorite = 0` | 3 红（含列表索引计划测试） |
| `MAX_HISTORY_ITEMS` 999 → 1000 | 3 红 |
| `set_max_history_items_impl` 跳过立即淘汰 | 1 红 |
| `counts_as_new_row` 退化成「非空即真」 | 2 红 |
| 前端非法值不回滚输入框 | 4 红 |

其中一处变异暴露了「两层判据互相兜底」的事实：只改 `cleanup_keep_recent` 的候选 SQL 时，`evict_over_limit_counts_only_non_favorites` 仍绿——因为 `evict_over_limit` 的 `COUNT(*)` 短路先按 `is_favorite = 0` 数出「没超限」直接返回，压根没走到候选 SQL。真实缺陷不会因此漏网（`evict_over_limit_never_deletes_favorites` 与 pipeline 的收藏用例会红），但**它说明「单条 SQL 的变异只被一条用例捕获」这件事不是偶然**：写这类双层判据时必须让两层在至少一条用例里都被走到。

## 未闭环

- **只能真机验证**：打包 GUI 上改上限的手感、`clipboard-changed` 与列表刷新在真实订阅链上的联动（headless 只覆盖到「命令层会发」与「前端只按事件刷新」两端）、加密大库（数万条）首次收敛耗时（当前 `cleanup_keep_recent` 是逐行 UPDATE，超出很多时是一批写）。
- **P2-4 的耦合**：字母 pin 落地时必须重审「pin 豁免」判据，不要复用 `is_favorite`。
- **Windows** 未编译验证（本机无 windows target；本项无平台分支代码，风险仅来自共用路径）。
