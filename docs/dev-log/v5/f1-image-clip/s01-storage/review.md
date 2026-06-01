---
id: V5-F1-S01-review
type: review
level: 小功能
parent: V5-F1
children: []
created: 2026-06-01T00:00:00Z
status: 未过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 图片剪贴板存储层（V5-F1-S01）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/db.rs` | diff | 新增 `ingest_image_as_clip`（公开）+ `insert_image_clip` + `try_insert_image_clip`（私有）+ 辅助 `make_test_png` / `make_test_conn` + T1/T2/T3 |

参照：Rust 规范（函数≤50行/单一职责/SQL参数化/Result别panic/持久化哈希显式稳定算法/ORDER BY确定性/注释写为什么/禁装饰注释/禁TODO_FIXME/错误处理完整）、code-standards、项目规范。

---

## 问题清单

### Critical

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| **Critical** | UPDATE clip_images SET clip_item_id 在 RELEASE SAVEPOINT **之后**执行，脱离事务保护。若 UPDATE 失败（如磁盘满 / I/O 错误），clip_items + clip_images 两行已提交但外键 clip_item_id 仍为 NULL，产生半关联孤立数据，与注释"任一步失败则 ROLLBACK SAVEPOINT，不留孤立行"的设计意图矛盾。 | `src-tauri/src/db.rs:563-568` | 规范：事务原子性——任一步失败整体回滚。**修复**：将 `UPDATE clip_images SET clip_item_id` 移到 `RELEASE SAVEPOINT` **之前**（即仍在 SAVEPOINT 保护内）。SAVEPOINT 内执行顺序调整为：INSERT clip_items → ingest_image_with_policy → UPDATE clip_images → RELEASE。此时若 UPDATE 失败走 ROLLBACK SAVEPOINT，三步全回滚，无孤立数据。外键约束亦无问题：同一事务内的写操作在 COMMIT 时整体检查，clip_items 行在 SAVEPOINT 内可见。 |

**置信度：95**

分析：SQLite autocommit 模式下（无外层显式 BEGIN），`RELEASE SAVEPOINT` 等同于 `COMMIT`，之后的 `conn.execute(UPDATE)` 是独立的单语句 autocommit 事务。coder 在 coding.md §4.3 中解释"UPDATE 在 RELEASE 之后是因为外键约束已满足"，但该解释只说明 UPDATE 为何不会因外键失败，并不消除 UPDATE 本身因 I/O 错误失败后无法回滚的风险。T1/T2/T3 的通过不能证明原子性——T3 只测了 SAVEPOINT Err 分支（ingest_image_with_policy 失败），没有测试"RELEASE 之后 UPDATE 失败"的路径，该场景在测试中无覆盖。

---

### Important

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| **Important** | 孤立行场景（clip_item_id=NULL）走新建路径时，`try_insert_image_clip` 内调用 `ingest_image_with_policy`，后者查到同 hash 的孤立行会走 `Bumped` 分支（仅 UPDATE last_modified_utc，**不**新建 clip_images 行）。外层 `insert_image_clip` 随后用 `RELEASE SAVEPOINT` 提交新 clip_items 行，再 UPDATE 孤立 clip_images.clip_item_id。结果：旧孤立行被关联到新 clip_items，**逻辑正确**；但 `ingest_image_with_policy` 返回 `Bumped(old_img_id)` 在 `try_insert_image_clip` 内被静默处理为与 `Inserted` 相同（均取 id），**这条分支无测试覆盖**。若未来 `ingest_image_with_policy` 的 Bumped 语义发生变化（如 Bumped 不再 UPDATE 或不再返回原 id），此处静默处理会引发外键漏写但无错误。 | `src-tauri/src/db.rs:598-600` | 规范：错误处理完整；测试充分性。**建议**：① 为孤立行场景（首次入库后手动将 clip_item_id 置 NULL 再次入库）增加 T4 测试，验证孤立行被正确关联而非新建；② 在 `Bumped` 分支加注释说明为何与 `Inserted` 处理相同（"孤立行命中时 Bumped 同样返回旧 image_id，UPDATE 将其关联到新 item_id，语义符合预期"）。 |

**置信度：82**

---

## 低于阈值的观察项（不阻断，备忘）

**ROLLBACK 失败被 `let _ =` 静默**（置信度约 70%）

`Err` 分支：`let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT ...")` 失败被静默。SQLite ROLLBACK 极少失败（唯一场景是数据库连接已断开），且失败时外层调用方收到原始 `Err(e)` 从语义上已是"操作失败"。当前 db 层无日志基础设施，统一不打日志（其他函数亦如此）。置信度不足 80%，不阻断。

**`now_ms` 时间戳仅捕获一次，clip_items 与 clip_images 的 created_utc 可能毫秒级不一致**（置信度约 60%）

`insert_image_clip` 在函数入口捕获 `now_ms` 传给 `try_insert_image_clip` 写 clip_items；而 `ingest_image_with_policy` 内部独立调用 `current_utc_ms()` 写 clip_images。两者时间戳可能差 0-1ms。业务无精确时间戳一致性要求，不影响排序与查重。置信度不足 80%，不阻断。

---

## 逐维度核查

### 1. 事务原子性（见 Critical 项）

UPDATE 脱离 SAVEPOINT，存在原子性缺口，需修复。

### 2. SQL 参数化

全部 SQL 使用 `rusqlite::params![]` 参数化绑定，无字符串拼接。**通过。**

### 3. image_hash 算法稳定性

复用 `img_mod::image_hash`（FNV-1a 64-bit，显式常量 `FNV_OFFSET`/`FNV_PRIME`，输出 16 位 hex），不另造哈希，不依赖语言默认 hash。**通过。**

### 4. 函数行数与单一职责

三函数行数：`ingest_image_as_clip` 30 行 / `insert_image_clip` 32 行 / `try_insert_image_clip` 23 行，均 ≤ 50 行。职责分明：查重+路由 / SAVEPOINT 生命周期 / 实际写操作。**通过。**

### 5. 注释质量

注释均描述"为什么"（`// 查是否已有同 hash 的未软删图片行`，`// clip_item_id 为 NULL（历史孤立行）：视为未命中，走新建路径`，`// 补写 clip_images.clip_item_id（V3-F1-A04 缺口...）`，`// 整体回滚，不留孤立 clip_items 行`）。无装饰性横线分隔符，无 TODO/FIXME，无注释掉的死代码。**通过。**

### 6. 错误处理

生产代码无 `unwrap()`/`panic!`，全部使用 `?` 传播；测试辅助函数（`make_test_png`/`make_test_conn`）用 `.expect("...")`，测试辅助允许。**通过。**

### 7. ORDER BY 确定性

本次新增代码未引入新 SELECT 查询（`ingest_image_as_clip` 中的 `SELECT id, clip_item_id FROM clip_images WHERE ... LIMIT 1` 是单行查取，无多行排序要求）。**通过。**

### 8. 测试质量（T1/T2/T3）

T1：验证首次入库 → Inserted，clip_items 恰好1行 kind='image'，clip_images.clip_item_id 非NULL，行数断言，非恒真。**有效。**

T2：验证去重 → Bumped，行数不变，last_modified_utc >= 首次值（断言语义正确，允许相等）。sleep(5ms) 以区分时间戳，注释说明相等也属正常。**有效。**

T3：空字节触发 make_thumbnail 失败，SAVEPOINT ROLLBACK，clip_items/clip_images 均0行。**有效。**

缺失覆盖：① "RELEASE 之后 UPDATE 失败"路径（对应 Critical 问题）；② 孤立行（clip_item_id=NULL）走新建路径的端到端验证（见 Important 问题）。

---

## 结论

**打回（必改 1 项，建议改 1 项）**

### 必改项

**M1（对应 Critical）**：将 `UPDATE clip_images SET clip_item_id` 移到 `RELEASE SAVEPOINT` 之前，确保三步写操作（INSERT clip_items + ingest_image_with_policy + UPDATE clip_images）全部处于 SAVEPOINT 事务保护内。同步更新 `ingest_image_as_clip` doc-comment 中行为描述的顺序（c 与 d 对调，先 UPDATE 后 RELEASE）。

### 建议改项（不强制阻断，但建议本阶段一并处理）

**S1（对应 Important）**：为孤立行场景增加 T4 测试；在 `Bumped` 分支匹配处补写解释注释。
