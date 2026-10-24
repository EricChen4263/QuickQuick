---
status: done
category: implementation
subject: Maccy 对标 · Phase 5（P5-1 托盘菜单补齐：接手未提交改动 + 清空未固定二次确认）
summary: 接手工作区里约 728 行无主托盘改动，先在隔离 worktree 里做只读审计（编译/测试/验收覆盖/半成品/与 P3-6 冲突），确认可用后单独提交；再补上唯一缺口「清空未固定」。缺口的实现取舍是「托盘只发请求、前端 window.confirm 确认、后端复用 cleanup_history_impl(conn, 0)」，不新写删除 SQL——收藏项由既有 `is_favorite = 0` 条件豁免，与容量裁剪/自动淘汰共用同一口径。提交时发现 lib.rs 已混入他人 52 行未提交的 title 回填改动，改用临时索引 + commit-tree 只提交自己那一行，工作区文件未被覆盖。
evidence:
  commits:
    - "d057ae2 feat(tray): 托盘菜单补齐暂停/打开设置/检查更新/版本号（P5-1）"
    - "e92c87d feat(tray): 托盘「清空未固定…」二次确认后清空非收藏历史（P5-1）"
  commands:
    - "rtk proxy git diff -- <托盘文件>  # 生成可用补丁（rtk git diff 会改写成人类可读摘要，git apply 不能用）"
    - "rtk git worktree add /tmp/qq-audit/wt HEAD"
    - "cargo check --all-targets / cargo clippy --all-targets -- -D warnings"
    - "cargo test  # 提交态 778 passed / 0 failed"
    - "npx vitest run  # 提交态 74 files / 771 tests passed"
    - "npx tsc -b"
  urls: []
---

# AI 活动记录 · P5-1 托盘菜单补齐（接手 + 补齐）

## 摘要

- **接手**：工作区里 6 个托盘相关文件有 728 行无主改动（tray.rs +712 等），作者与完整性未知。先在隔离 worktree 里只读审计，结论是「编译/测试全绿、无半成品、与 P3-6 无冲突，但 P5-1 缺 1 项」，再单独提交（`d057ae2`）。
- **补齐**：缺的是「清空未固定」。新增菜单项 + 二次确认链路（`e92c87d`）。
- 审计阶段实跑：`cargo check` / `clippy -D warnings` / `cargo test`（769）/ `vitest`（768）/ `tsc` 全绿；补齐后提交态为 Rust 778、前端 771。

## 审计发现（接手部分）

被接手改动本身是完整的，不是半成品：新增 24 个函数/类型全部有生产调用点，无 `TODO/FIXME`，无「写了但没接线」的命令。P5-1 六项验收中五项已具备（暂停捕获 / 打开设置 / 检查更新 / 版本号 + tooltip 三态），**只缺「清空未固定」**，另外「图标显示最近内容」是 roadmap 标注的可选项、未做。

两处值得记的判断：

1. **「检查更新」确实复用了既有链路**：托盘调的是 `ipc::update::check_for_updates`（lib.rs 已注册的 `#[tauri::command]`），只在其上加了 `UpdateStatus` 状态机；没有第二套 endpoint。
2. **与 P3-6 无冲突**：`2e67530` 已是 HEAD 祖先，`handle_tray_icon_event` 仍走 `privacy::tray_click_action`（⌥/⌥⇧/无修饰），`refresh_tray_indicator` 签名未变，故 lib.rs 三处调用点不需要动。

## 「清空未固定」为什么是二次确认

用户裁决走确认框。托盘原生菜单承载不了确认框，所以链路拆成两段：托盘**只发请求**（`tray-clear-unpinned-request`），删不删由前端 `window.confirm` 决定。这样「取消」是一个真实存在的分支，而不是靠文案威慑；取消路径有测试锚定，实现里取消时一行都不删。

确认文案写的是「清空未固定历史？收藏项会保留，此操作不可撤销。」——破坏性确认必须给出**代价边界**，只说「不可撤销」等于让人盲选。

## 删除逻辑为什么一行 SQL 都没写

`clear_unpinned_impl` 就是 `cleanup_history_impl(conn, 0)`：`keep_count = 0` 的语义恰好等于「非收藏项一条不留」，而底层候选查询带 `AND is_favorite = 0`，收藏项天然豁免。

选择复用而非另写删除语句，理由是**口径唯一**：容量裁剪、自动淘汰、清空未固定三处若各写一条 `UPDATE ... WHERE`，迟早漂成三套语义（本次审计正好看到 P4-4 把「上限」从硬编码 500 收敛成单一口径，是同一个道理）。抽成 `clear_unpinned_impl` 也是为了让单测调用**生产路径本身**，而不是在测试里自己再传一次 `0`——否则改坏生产参数测试不会红。

## 测试与变异（本轮最该记住的部分）

真库锚定（`tempfile::tempdir()` + `db::open_or_create`，走生产开库入口而非手工建表）：入库 2 条文本、收藏其中 1 条 → 清空 → 断言收藏项仍 live、非收藏项被软删、`soft_deleted=1`、`purged=1`。另有一条「只剩收藏项时重复清空是幂等空操作」。

三次变异全部实跑，都成功变红：

| 变异 | 结果 |
|---|---|
| `db.rs` 候选 SQL 去掉 `AND is_favorite = 0` | 2 条红：`只应软删那一条非收藏项` left=2 right=1；`重复清空不得动收藏项` left=0 right=1 |
| `App.tsx` 删掉 `if (!window.confirm(...)) return;` 守卫 | 「取消时不调用后端命令」红 |
| `App.tsx` 回执改用 `result.purged` 冒充非收藏项条数 | 期望「已清空 3 条」实得「已清空 5 条」红 |

第三条变异是**变异测试自己发现的问题**：最初 mock 里 `softDeleted: 3, purged: 3` 同值，取错字段根本抓不到。改成 `purged: 5` 后才真正钉住「回执说的是非收藏项条数」。

## 一个必须记下来的工程坑：lib.rs 已被他人改动

准备提交第二个 commit 时发现 `src-tauri/src/lib.rs` 里有 **52 行别人未提交的 title 回填改动**（`spawn_title_backfill` / `db::backfill_titles_batch`），我只需要加一行 `tray::clear_unpinned_history`。直接 `git add lib.rs` 会把别人的半成品卷进我的提交。

做法是**不碰工作区文件**：临时索引（`GIT_INDEX_FILE`）→ `read-tree HEAD` → 用 `git show HEAD:lib.rs` 生成「HEAD + 我那一行」的干净内容并以 `hash-object -w` 写入 → `write-tree` → `commit-tree` → `update-ref`（带旧值 CAS）。最终 `git show --stat` 里 lib.rs 只有 `+1`，别人的 52 行仍在工作区未提交。

顺带一个副作用教训：提交后索引仍停在提交前的状态（我改文件前 `git add` 过），`git status` 一度显示 `MM`。为避免把别人的 lib.rs 重新暂存，用 `git update-index --cacheinfo <HEAD blob>` 把索引项指回 HEAD 版本——只动索引，不动工作区文件。

## 环境坑（复现审计时需要）

- **`rtk git diff` 会改写 diff 格式**（人类可读摘要），`git apply` 直接失败；生成补丁必须 `rtk proxy git diff`。
- worktree 里 `tauri::generate_context!()` 要求 `frontendDist`（`../dist`）存在，否则 proc macro panic；前端测试需要把主工作区 `node_modules` 软链进来。
- 某个中间时点的基线 commit 是**不自洽的**（lib.rs 引用了只存在于他人工作区的 `db::backfill_titles_batch`），从该 commit 拉 worktree 编译不过——这不是本 item 的问题，但排查时会误导。

## 未验证

- **真机托盘行为**：菜单项实际显示顺序、省略号视觉、「清空未固定…」在 macOS 菜单里的分组观感，均未在打包 GUI 上点过。
- **`window.confirm` 在打包 GUI 中的表现**：WKWebView 的 `confirm` 是原生模态；但本应用是 Accessory 策略 + 窗口可能刚被 `show()` 唤起，是否会出现「模态框挂在未激活窗口后面」未验证。已在 `request_clear_unpinned` 里选择「先 show 再 emit」，就是为了降低这个风险，但需要真机确认。
- **Windows**：`current_modifiers` 的 `GetKeyState` 分支未编译（本机只有 aarch64-apple-darwin target）。
- 清空后列表刷新依赖 `clipboard-changed` 事件，只在单测层面确认了发射条件，未做真机联调。
