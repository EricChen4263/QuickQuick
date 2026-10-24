---
status: done
category: implementation
subject: Maccy 对标 · Phase 4 预计算 title 落库 + 分批幂等回填（P4-5a 后端）
summary: clip_items 通过迁移框架 v1→v2 增加 title 列（迁移只做无条件 ADD COLUMN），入库四条路径写定 title（正文空白规范化后按字符截断 1000，图片留空串），开库后由后台线程按 200 行一批幂等回填（只补 title IS NULL，可中断可续跑，批间让锁）。列表 DTO 按裁决**不改**：content 仍全量下发，2000 条 payload 改前/改后逐字节一致。
evidence:
  commits:
    - 6eabaf0 feat(db): clip_items 增加预计算 title 列与分批幂等回填（P4-5a）
  commands:
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "rustfmt --check src-tauri/src/db.rs src-tauri/src/ipc/clipboard.rs src-tauri/src/ipc/system.rs src-tauri/tests/search_title.rs"
    - "cd /tmp/qq_head_check/src-tauri && CARGO_TARGET_DIR=... cargo check --all-targets"
    - "cd src-tauri && cargo test --test search_title -- --nocapture"
  urls: []
---

# AI 活动记录 · 预计算 title 落库 + 分批幂等回填（P4-5a 后端）

## 摘要

- **schema**：`CURRENT_SCHEMA_VERSION` 1 → **2**；新增 `Migration { to_version: 2 }`，步骤内只有一句无条件
  `ALTER TABLE clip_items ADD COLUMN title TEXT;`。既有行升级后 `title` 为 **NULL**，表示「尚未回填」。
- **title 规则**（`db::compute_title(kind, content)`，`TITLE_MAX_CHARS = 1000`）：正文做空白规范化
  （去首尾、连续空白折叠为单个半角空格）后**按字符**截断到 1000；图片条目写**空串**。
- **入库四条路径写定 title**：`ingest`（文本/富文本）、`insert_image_clip`（图片）、
  `insert_backup_text_item` / `insert_backup_image_item`（备份导入）。写入与去重键同一个 INSERT。
- **回填**：`db::backfill_titles_batch(conn, 200)` + `lib.rs::spawn_title_backfill`——开库后起的后台线程，
  一次一批、每批一个短事务、**批间释放 `AppDb` 锁并 `sleep(20ms)`**；只挑 `WHERE title IS NULL`，
  所以「跑一半被杀掉」后重跑只补剩下的行。
- **列表 DTO 不变（裁决 C）**：`ClipItemDto` 没有 title 字段，`content` 仍全量下发。2000 条实测
  payload 改前/改后**逐字节一致**。

## 三个必须写下来的判断

**1. 图片条目的 title 是空串，不是 NULL——这不是风格问题，是回填能不能收敛的问题。**
回填的候选集是 `WHERE title IS NULL`。若图片行留 NULL，它永远留在候选集里：每一轮回填都会挑到它、
写回同样的 NULL，循环要么永不结束、要么每批都在白扫图片行。所以把「NULL = 未回填」与
「空串 = 已定值且按约定为空」严格分开，图片入库直接写 `''`。这条有测试锚定
（`image_item_title_is_empty_string_not_null`，变异成写 NULL 立刻红）。

**2. 列表 DTO 不做 title（裁决 C），因为 `content` 还在全量传。**
按 roadmap 字面「预计算 title + 内存过滤」看起来应该让 DTO 带 title，但实测代价是：
2000 条数据下 **title 无条件下发让列表 JSON +82.8%（混合形态）/ +62.2%（全多行形态）**；
即使只在与 content 不同时下发，最坏形态仍 +62.2%（用户历史以多行文本为主时，
空白规范化必然让 title ≠ content，于是同一段文本被传两遍）。
所以本项只做**底座**：列、规则、回填、可读路径都就位，DTO 切换留到 P4-2 两段式加载
（列表只传 title、正文按需取）时一起做——那时 title 才有「替代 content」的意义。

**3. 批大小 200，是按「持锁时长」而不是「总吞吐」选的。**
回填挂在启动路径附近，而每条 IPC 命令都要抢同一把 `AppDb` 锁，所以单批的判据是
「一次持锁多长」。实测口径必须写全：**debug 构建、单机、1 万行真文件 SQLCipher 库、批大小 200**——
50 批，**首批 1.68 ms、最大 2.58 ms、平均 1.89 ms**，整轮 94 ms。
这组数字只用于「批大小 200 的单批持锁落在毫秒级」这一判断，**不推广为生产性能结论**
（真机大库的页 I/O、SQLCipher 页大小、OS 缓存都没有覆盖）。
批再小会把「每批固定开销（事务提交 + 一次 rowid 顺序扫描）」摊到太少行、总时长反而上升；
批再大则单次持锁跨过 10 ms 量级，热键唤起的列表刷新会等它。

## 幂等条件只剩一处（删掉了一个谁都覆盖不到的分支）

初版 `UPDATE ... WHERE id = ?1 AND title IS NULL` 的守卫是**多余的**：候选集已经被
`SELECT ... WHERE title IS NULL` 筛过一遍，而 `compute_title` 是从 `content` 确定性算的，
两个写者补同一行也会算出同一个值。变异验证证实了这一点——**去掉这个守卫，全部测试 0 红**。
一个进不去、也没人覆盖的条件留着只会误导后来人，故删除，并把幂等契约显式写在
`PENDING_TITLE_SQL` 的文档里：**`WHERE title IS NULL` 才是幂等与可续跑的唯一落点**。

## 实跑证据

- **迁移 v1→v2（真实文件库 + SQLCipher + 真密钥 + 既有数据）**：先用生产迁移链只升到版本 1
  （`apply_migrations(&v1, 1, &MIGRATIONS[..1])`）写入 2 条，再用当前代码 `open_or_create` 打开 →
  版本 2、`title` 列存在、2 条数据完好、两条的 `title IS NULL`（迁移未做全表 UPDATE）；
  二次打开成功（重放 `ADD COLUMN` 会报 `duplicate column name`，能打开即证明没重放）；
  随后回填一批补齐两条。`db::tests::ensure_schema_upgrades_v1_file_db_to_v2_keeping_rows`。
- **回填中断可续跑 + 已补行不被重算**：500 条待回填行，先只跑一批（= 进程在第二批前被杀），
  审计触发器记录每次 `UPDATE OF title` 的目标行；续跑后断言「剩余 300 行被补」「审计行总数
  恰为 500、DISTINCT 也是 500」——已补行被重算的话审计会 >500。
  `tests/search_title.rs::backfill_resumes_after_interruption_without_recomputing_filled_rows`。
- **payload（2000 条，LCG 确定性造数，serde_json 序列化整个列表）**：
  | 形态 | 改前 | 改后 |
  |---|---|---|
  | 混合（55% 单行短 / 30% 多行短 / 15% 长文本） | **2,105,877 B** | **2,105,877 B** |
  | 全多行短文本（最坏） | **923,648 B** | **923,648 B** |
  两条都逐字节相同（不只比长度：拿「改前 DTO 结构」序列化后整串比对，
  一旦 DTO 多出任何字段立刻红）。对照：若 title 无条件下发，同样是这两批数据是 +82.8% / +62.2%。
- **测试与静态检查**：`cargo test` **786 passed / 0 failed**（lib 491 + 集成 295，含新增 6 条
  `tests/search_title.rs` 与 2 条 db 单测）；`cargo clippy --all-targets -- -D warnings` 干净；
  我的 5 个文件逐个 `rustfmt --check` 干净；`HEAD` 在隔离 worktree 里 `cargo check --all-targets`
  **exit 0**（证明「提交后 HEAD 可编译」，不是只说我的分支能编）。

## 变异验证（7 处，全部打红；第 8 处暴露了死条件）

| # | 变异 | 结果 |
|---|---|---|
| 1 | `PENDING_TITLE_SQL` 的 `WHERE title IS NULL` 反转成 `IS NOT NULL` | lib 1 红 + search_title 2 红 |
| 2 | 回填体短路成 `return Ok(0)`（等价「移除回填」） | lib 1 红 + search_title 2 红 |
| 3 | 入库路径 title 写 NULL（不写预计算值） | search_title 3 红（含 payload 对账） |
| 4 | `compute_title` 不做空白规范化 | lib 1 红 + search_title 3 红 |
| 5 | `TITLE_MAX_CHARS` 1000 → 100 | search_title 1 红（截断长度用字面量 1000 断言，不用常量自证） |
| 6 | 图片条目 title 写 NULL 而非空串 | search_title 1 红 |
| 7 | 迁移步骤里塞 `UPDATE ... SET title = content`（全表回填） | lib 2 红（含 v1→v2 用例断言「既有行 title 仍为 NULL」） |
| 8 | 去掉回填 `UPDATE` 的 `AND title IS NULL` 守卫 | **0 红** → 判定为不可达的死条件，已删除（见上节） |

## 已知缺口（本项不做，P4-2 必须解决）

**缺口 1：列表 DTO 不带 title（裁决 C）。** 列表 payload 里没有任何 title 字段，
前端搜索仍跑在 `content` 上；`title` 列目前是「只写不读」的底座（写入 + 回填 + 规则就位，
消费方要等两段式加载）。这不是漏做，是刻意的顺序：`content` 还全量传的时候，
DTO 带 title 只会让 2000 条列表 JSON +82.8%／+62.2%（见上表），而命中集合不变。

**缺口 2：title 截断到 1000 字符后，超过 1000 字符的正文尾部在 title 上不可搜。**
现在因为列表仍下发完整 `content`，前端搜索跑在 `content` 上，所以**用户当前不会踩到这个坑**；
但 P4-2 一旦把列表改成「只传 title、正文按需取」，长文本尾部的命中就会真的丢掉。
届时必须同时给出方案（候选：后端 `LIKE` / FTS5 对正文建检索面，或对命中后按需补齐全文再二次过滤），
**不能靠「反正列表还带正文」蒙混过去**——那句前提在 P4-2 里正好被推翻。
两个缺口是绑在一起的：缺口 1 一解决（列表只传 title），缺口 2 就立刻变成用户可见的功能回归，
所以 P4-2 必须在同一次改动里同时给出「长正文尾部可搜」的答案。

## 未验证 / 已知盲区

- **真机启动期回填未实跑**：`spawn_title_backfill` 是 Tauri 启动路径里的后台线程，headless 测试
  覆盖不到（无 `AppHandle`）。已验证的是它调用的 `backfill_titles_batch` 与回填语义；
  「开库后线程真的被拉起、真的按批让锁」这一层只有代码与日志，**没有真机日志为证**。
- **加密大库（数万条）首启回填总耗时**：只在内存/临时文件库上测到 1 万行 94 ms；
  真机 SQLCipher 大库 + 页 I/O 的墙钟未测。
- **回填与剪贴板轮询的锁竞争**：仅从设计上让锁（批间 sleep 20ms），未做真实并发压测。
- **Windows**：本机只有 `aarch64-apple-darwin` target，本项未在 Windows 编译验证。
- **前端**：按裁决 C 前端零改动；`pnpm exec vitest run src/clip-popover/clip-search.test.ts`
  35 passed，但同批 `src/panels/clipboard` 的 33 条红是他人进行中的 OnboardCard 改动所致
  （`Cannot read properties of undefined (reading 'step')`），与本项无关，未在本项处理。

## 与文档口径的对齐（供 lead 写入 work-items）

P4-5 的验收项「列表 DTO 带 title / 移除 content 回退」在本项**未做**，理由：payload 红线
（最坏形态 +62.2%）与 `content` 仍全量下发二者同时成立时，DTO 切换只有成本没有收益；
它与「去 content」必须配套，故并入 **P4-2 两段式加载**。
