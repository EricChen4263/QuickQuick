---
status: done
category: implementation
subject: Maccy 对标 · Phase 4 列表与 IPC 体积（P4-3 缩略图移出 payload + P4-6 翻译历史分页）
summary: 把缩略图从列表 IPC payload 里彻底移出去（改后每条固定 230 B，与缩略图体积解耦）、翻译历史加分页上限与保留清理。两次审查都打出了「测试拦不住退化」——一次是计划串相同但 SQL 语义被删，一次是行级触发器在删 0 行时必然假绿。
evidence:
  commits:
    - c661590 perf(ipc): 列表 DTO 去掉内联缩略图 base64，新增按需取图命令（P4-3）
    - 77fc4cb perf(clip): 缩略图改为按需拉取 + 模块级缓存与在途去重（P4-3）
    - 1469e79 test(ipc): 为列表 SQL 的软删/空缩略图条件加字面锚并收敛缓存读取路径
    - 95d3045 perf(translate): 翻译历史加分页上限与保留清理（P4-6）
    - fb5d366 test(translate): 钉住未超限时不执行清理 DELETE 的设计意图
  commands:
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "pnpm exec tsc -b && pnpm test"
  urls: []
---

# AI 活动记录 · Phase 4 列表与 IPC 体积

## 摘要

- **P4-3 缩略图移出 IPC payload**：列表不再内联 base64，改为 `has_thumbnail` 标志 + 按需取图命令 + 前端模块级缓存与在途去重。
- **P4-6 翻译历史分页**：加 `LIMIT 200` 与保留上限 500 的 best-effort 清理。

两项都是路线图 L3 里标「硬伤」的量级问题，且完全不需要人工 GUI 验证。

## 一次「为了不动契约测试」的取舍

`list_items_with_images` 返回的行里带缩略图 BLOB，而 `src-tauri/tests/capture_contract.rs`（上一轮刚落地的 P3-0 契约护栏）有 3 处断言 `row.thumbnail` 非空。直接改这个函数的行结构会让护栏编译不过。

于是新增只读函数 `list_items_with_image_flag`：**缩略图 BLOB 连出库都不做**，投影成 `CASE WHEN thumbnail IS NOT NULL AND length(thumbnail) > 0 THEN 1 ELSE 0`。两个变体共用同一份 SQL 骨架（`macro_rules!`），并用 `assert_eq!(flag_plan, blob_plan)` 钉住计划字符串相同。

这个取舍的代价是「数据库里多了一个近似查询」。可接受，因为 P3-0 护栏的价值高于少一个只读函数；但代价要记住：将来谁想合并这两个函数，必须先改那 3 处契约测试并说明理由。

## 关于 payload 数字：一个不该对外引用的倍数

执行者最初汇报「523,955 B → 2,301 B（228×）」，审查者用同一 baseline 重跑得到 **4,751 B → 2,301 B（2.06×）**，判断「228× 复现不出来」。

追下去发现两边都对，但**测的输入不同**：改前 payload 完全由缩略图体积决定（`改前 = 2,301 + 1.33 × 缩略图字节`），噪点图档（39 KB/条）是 523,955 B、渐变档是 20,423 B、纯色档只有 5,275 B。审查者那次的图接近纯色档。

**结论是「改前的数字本身不是个常量」，所以任何单一倍数都是误导。** 可对外引用的稳定事实只有一条：**改后 10 条含图条目 = 2,301 B（230 B/条），三档输入完全一致，与缩略图体积解耦**。这条写进了 work items，并明确标注「不要说 228×」。

## 两次「测试拦不住退化」

**1. 计划串相同 ≠ 语义相同（P4-3）**
`assert_eq!(flag_plan, blob_plan)` 只比较查询计划。审查者变异发现：**去掉 JOIN 的 `cimg.is_deleted = 0`，断言仍然通过** —— 因为两个变体共用宏，计划串逐字相同，但语义已经变成「软删的图片行也会被 JOIN 进来」。修法是加 SQL 字面锚（`cimg.is_deleted = 0` / `length(...) > 0` / `ci.is_deleted = 0`），删任一条件必红。

顺带发现缓存读取有两处互相兜底（改一处测试仍绿），收敛到单一入口 + 渲染期派生读。

**2. 行级触发器在「删 0 行」时必然假绿（P4-6）**
「未超限时不该执行清理 DELETE」这条设计意图原本没有测试。第一反应是用 `BEFORE DELETE` 触发器记审计行——但这条路**必然假绿**：未超限时那条 `DELETE ... OFFSET 500` 即使被执行也是删 0 行，而行级触发器 0 行匹配就 0 次触发，退化前后的代码在 SQL 层面完全无可观测差异。

改用 `sqlite3_set_authorizer` 在 **prepare 阶段**统计 `SQLITE_DELETE` 动作——删 0 行的 DELETE 同样计数。并配了**正向对照**（超限写入后计数必须从 0 变 1），否则「0 次」可能只是探针没接上。

## 「清理失败不影响主链路」的测试怎么构造

不要用只读连接——那连写入也会失败，测不到「写入成功、清理失败」这个真形态。正确做法是 `BEFORE DELETE` 触发器：先 `INSERT INTO prune_audit` 再 `SELECT RAISE(FAIL, ...)`，让 DELETE 单独失败；审计表的存在还证明「清理真的被尝试过」，避免「清理压根没触发」的假绿。

## 失败方向的取舍

- **缩略图取图失败也缓存为 null、不重试**：取不到通常是该行已被清理或 BLOB 为空，重试只会把列表滚动放大成请求风暴。代价是瞬时故障要重开窗口才自愈——写进注释，不假装没有。
- **翻译历史清理吞错**：清理是维护动作，不能让它把用户的一次翻译变成失败。

## 验证（Lead 侧实跑）

```
cargo check / clippy -D warnings / fmt --check   → 全绿
cargo test                                       → 704 passed / 0 failed
pnpm exec tsc -b                                 → 无错误
pnpm test × 2 连跑                                → 每次 69 files / 676 tests 全绿
```

## 未闭环

- **只能真机验证**：列表滚动时按需拉取的实际 IPC 频率与首屏延迟；失败永久缓存在 SQLCipher 大库上是否可接受；占位 `min-height` 与真实缩略图（最大 360px）的视觉衔接；翻译历史提示文案的实际位置。
- **P4-2（分页 + 虚拟滚动）与 P4-5（搜索升级）没有做**：这两项互相耦合——只给 DB 加分页会让前端搜索只能搜到已加载的部分。正确形态是「轻量索引全量 + 可见行才取正文/缩略图」的两段式加载，需要和虚拟滚动一起设计，留给下一轮。
- **未验证**：Windows 全路径；加密库上 `COUNT(*)`/`DELETE OFFSET` 的耗时。
