---
status: done
category: implementation
subject: Maccy 对标 · Phase 3 规则接线（P3-5/P3-6）+ Phase 4 索引（P4-1）
summary: 补上托盘暂停语义（⌥ 切换暂停 / ⌥⇧ 仅忽略下一次）与忽略规则增强（内容正则 + 应用名单黑/白模式），并给 db 建上第一批索引（此前全仓零 CREATE INDEX）。两次 BLOCK 都是「测试没锁住真实契约」——一次是优先级阶梯漏了相邻层，一次是查询计划测试里有一句恒真断言。
evidence:
  commits:
    - 2e67530 feat(tray): ⌥ 点击切换暂停、⌥⇧ 仅忽略下一次（P3-6）
    - 67dac67 feat(privacy): 内容正则忽略与应用名单黑白模式（P3-5 后端）
    - 74faf56 feat(settings): 忽略规则面板接线（P3-5 前端）
    - a9345c4 test(privacy): 锁住 Concealed 与 ContentRegex 的优先级并补白名单来源未知提示
    - af474c1 perf(db): 为列表/去重/清理查询建索引（P4-1）
    - 153f066 test(db): 修计划测试里的空断言并让 hash 索引列序理由可证伪
  commands:
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "pnpm exec tsc -b && pnpm test"
  urls: []
---

# AI 活动记录 · Phase 3 规则接线 + Phase 4 索引

## 摘要

本轮做的都是「不等面板语义、不等 arboard 替换」的条目：

- **P3-6 托盘暂停语义**：`⌥` 点托盘切换暂停、`⌥⇧` 点只忽略下一次；
- **P3-5 忽略规则增强**：内容正则 + 应用名单黑/白两模式（类型集合忽略依赖 `types()` 遍历，登记 blocked）；
- **P4-1 建索引**：仓库此前**零 `CREATE INDEX`**，列表/去重/清理全在全表扫。

两次 BLOCK 都不是「功能写错了」，而是**测试没能锁住真实契约**——这恰好是这套改造最该防的东西。

## 两个「测试假绿」被打出来

**1. 优先级阶梯漏了相邻层**
忽略规则的判定顺序被文档钉成契约（`Paused → SkipOnce → Concealed → ContentRegex → 名单规则 → SelfMark`）。阶梯测试号称逐层叠加，但第 3 层用的是无正则的规则集、第 4 层又显式 `is_concealed = false`——**这两层从未同时命中**，所以把 `ContentRegex` 提到 `Concealed` 之前，测试**全绿**。

这跟我记忆里的一条老教训是同一类：阶梯测试只有在相邻两层同时成立时才有意义；每步只让当前层成立，互换顺序不会红。修法是把阶梯改成「每步同时命中所有更弱层、只关掉一颗条件」，并补一条最小对照 + 反向对照的相邻对专测。

**2. 查询计划测试里有一句恒真断言**
`!after.contains("SCAN clip_items")` —— 那条 SQL 用了别名 `FROM clip_items ci`，计划串里写的是 `SCAN ci`。所以这句话**永远为真**，留着只会让后来人以为它在守什么。同时「`text_hash` 必须排在 `is_deleted` 前面」这条承重理由也没有任何测试钉住（变异成 `(is_deleted, text_hash)` 是 0 红）。

修正后断言升级为带约束串（`(text_hash=? AND is_deleted=?)`）并加了「不得退化成另一条索引」的反向断言，现在列序互换会红。

## P4-1 的三处「按实测而非按字面」

路线图给的索引是字面的 `(is_deleted, last_modified_utc)` / `(text_hash)` / `(image_hash)`。实测之后三处都得改：

1. **列表索引必须含 `is_favorite`**：`ORDER BY is_favorite DESC, last_modified_utc DESC`，只按字面建，计划里仍留 `USE TEMP B-TREE FOR ORDER BY`——省掉的只是「取回来再排序」。
2. **hash 索引必须带第二列 `is_deleted`**：只建单列时，在生产那些**没有 `sqlite_stat1` 统计**的库上，规划器会把两条路径估算成等价代价，改走 `is_deleted` 前缀索引 —— 等于把全部未软删条目扫一遍，等于没建。
3. **`clip_images(clip_item_id)` 不是凑数**：外键级联删除实测 10k 父行 × 2k 子行 **0.47s → 0.01s**，同时消除列表 LEFT JOIN 的自动索引。

如实修正一处可能的过度宣称：**列表查询墙钟基本持平**（35.7ms → 25.3ms），大头是 Rust 侧全量行映射；真正的收益是计划形态（去掉临时排序）而不是可测的墙钟。干净收益在去重查询：1.6ms → 53µs（~30×）。

## skip-once 的消费位置（本项最易做错处）

「仅忽略下一次」看起来简单，实际有两个错误做法：
- 用「本轮入库结果为空」判断 → 那时内容**已经进库了**；
- 让空轮也消费 → 用户按完 `⌥⇧` 后往往几轮内什么都没复制，一次 `swap(false)` 就把机会白吃了，之后真正复制敏感内容照常入库。

正确位置是**轮询循环层**，判据与 `poll_once_with_policy` 同口径（`change_count() > last_seen`）。已知边界写进了注释：若置位后第一次变化恰好是本工具自写回（自产标记），这次机会会被用掉——探针只看到「剪贴板变了」，与用户「这一次别记」的心智一致。

## 一条被约束改变的建模

`CapturePolicy` 在 `src-tauri/tests/**` 有 30+ 处字面量构造，给它加字段会让刚落地的 P3-0 契约护栏整体 E0063。所以新规则没有加字段，而是挂到 `IgnoreRules`（`ExcludeList` 的语义化别名，应用名单与正则是两个独立字段，`should_skip*` 签名不变）。代价是类型名与语义有偏差，已在文档里改成实情并加了别名。

## 失败方向的两个决定

- **非法正则 → 不跳过**：写错规则的代价应该是「这条没生效」，而不是「剪贴板被一条写错的正则全吞」（那是数据丢失）。设置侧原文照存，用户才能在面板里看到并修正半成品规则。
- **白名单空名单 → 视同未启用**：一开白名单就全吞，用户是先丢数据再看懂界面，方向反了。非空但来源身份未知 → 不记录（白名单是「证明它在名单内才记」），并在设置页补了提示文案——否则用户会困惑「为什么这个 app 什么都没记」。

## 验证（Lead 侧实跑）

```
cargo check / clippy -D warnings / fmt --check   → 全绿
cargo test                                       → 694 passed / 0 failed
pnpm exec tsc -b                                 → 无错误
pnpm test                                        → 67 files / 666 tests 全绿
```

## 未闭环

- **类型集合忽略 = blocked**（依赖 P3-1 的 `types()`）。
- **只能真机验证**：`⌥`/`⌥⇧` 点托盘的修饰键读取（事件经 tao event-loop proxy 转发，用户若在 handler 执行前松键会退化成「显示窗口」，风险中）、tooltip 三态与菜单文案、白名单下「来源未知不记」的实际频率、面板 chip 交互。
- **Windows 全路径未编译验证**（`GetKeyState` 符号与 feature 已按 `windows-sys 0.61.2` 核对）。
- **P4-1 的真机收益未验**：既有大库首次升级的建索引代价、5 条索引对 ingest 尾延迟与开库成本的影响。
- **P3-5 前端「白名单来源未知」提示已补，但未做二次独立审查**（复审只要求补文案 + 断言，改动小）。
