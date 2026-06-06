---
id: RT1-F1-S01-review
type: review
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A01]
evidence: []
author: code-reviewer
---

# 审查结论 · 存储层支持 html（RT1-F1-S01）

## 审查范围

| 文件 | 改动说明 |
|---|---|
| `src-tauri/src/db.rs` | 核心：加列、迁移守卫、ingest 补写分支、结构体字段、SELECT 补列 |
| `src-tauri/src/ipc/clipboard.rs` | 内联建表 SQL 同步加 `html_content TEXT` |
| `src-tauri/src/ipc/system.rs` | 内联建表 SQL 同步加 `html_content TEXT` |
| `src-tauri/tests/capture_image.rs` | 内联建表 SQL 同步加 `html_content TEXT` |
| `src-tauri/tests/richtext_storage.rs`（新增） | 真实加密库 roundtrip + 纯文本去重集成测试 |

参照标准：项目规范 + code-standards skill（§1 通用原则 / §3 函数 / §5 注释 / §6 类型 / §8 测试 / §10 安全）+ 项目既有约定（ORDER BY 兜底、注释写"为什么"、禁装饰性分隔注释）。

---

## 审查维度逐项核对

### 1. 正确性：迁移幂等守卫（通过）

`has_column` 以 `PRAGMA table_info(clip_items)` 遍历列名，找到即返回 `true`；`migrate_html_column` 在列存在时 early return 跳过 `ALTER`，多次调用安全。

并发安全性：`open_or_create` → `ensure_schema` → `migrate_html_column` 在单连接初始化路径上串行执行，应用只在 `pipeline.rs:setup_app_db` 单点开库，无多线程并发调用迁移的场景。TOCTOU 理论存在（两连接并发调用 PRAGMA+ALTER 窗口），但架构保证不会发生，注释已说明。**通过**。

### 2. 正确性：`has_column` PRAGMA 注入分析（通过）

`format!("PRAGMA table_info({table})")` 做字符串插值而非参数化。`has_column` 为私有函数（`fn`，非 `pub`），仅被 `migrate_html_column` 以硬编码字符串 `"clip_items"` 调用，无外部输入路径。注入无实际攻击向量；注释已说明"table 名来自内部常量，非用户输入"。**通过**。

### 3. 正确性：ingest 补写分支逻辑与决策4一致性（通过）

条件为 `existing_html.is_none() && item.html.is_some()`，精确对应决策4语义：

| 情形 | 行为 | 期望 |
|---|---|---|
| 旧行无 html，新来带 html | UPDATE html_content + kind='richtext'，再 bump | 补写升级 |
| 旧行已有 html | 仅 bump，不覆盖 | 保留旧 html |
| 新来无 html（纯文本重复） | 仅 bump | 去重不降级 |

三条路径逻辑均与决策4一致。`text_hash` 计算沿用既有 `text_hash(&item.text)`，纯文本相同即命中，不变。**通过**。

### 4. 正确性：UPDATE 与 bump_to_top 原子性（通过）

补写分支：`conn.execute(UPDATE html_content/kind)` + `bump_to_top(conn, &id)`，两步均属同一 `Connection`，运行于调用方 `capture_and_ingest` 开启的 `SAVEPOINT capture_ingest` 范围内。任一步失败则整批回滚，不会出现"html 写入、时间戳未刷"或"html 写入、bump 失败"的半完成态。**通过**。

### 5. 正确性：SELECT 列号对应（通过）

- `list_items_full`：SELECT 第 5 列（0-based）= `html_content`，`row.get(5)` 正确。
- `list_items_with_images`：SELECT 第 7 列（0-based）= `ci.html_content`，`row.get(7)` 正确。

**通过**。

### 6. ORDER BY 确定性兜底（通过）

两个查询均含 `ORDER BY ci.is_favorite DESC, ci.last_modified_utc DESC, ci.rowid DESC`（或等价形式），`rowid DESC` 作为同毫秒兜底，符合项目约定（hints.md 和 code-general.md 均明确要求）。**通过**。

### 7. SQL 安全（通过）

所有用户/外部数据（hash、html、id）均通过 `rusqlite::params![]` 参数化传入，无字符串拼接。SQL 注入风险已排除。**通过**。

### 8. 代码规范（通过）

- **函数长度**：`has_column`（12 行）、`migrate_html_column`（5 行）、ingest 新增逻辑（约 6 行增量）均 ≤ 50 行。
- **注释**：`has_column`、`migrate_html_column` 均有"为什么用 PRAGMA table_info 守卫"说明，ingest 分支注释写"旧行无 html 才补写升级"决策理由，符合"注释写为什么"规范。
- **无 TODO/FIXME/装饰性分隔注释**：全文无发现。
- **无 SELECT \***：所有查询按需取列。
- **无魔术值新增**：`kind='richtext'` 是与既有 `'text'`/`'image'` 同风格的延伸，非本次引入新的违规；整库无 Kind 枚举是既有技术债，不属本次改动范围。

**通过**。

### 9. DRY：四处建表 SQL 重复（观察，不阻塞）

`ensure_schema`（db.rs）、`make_test_conn`（db.rs tests）、clipboard.rs/system.rs/capture_image.rs 各有内联 `CREATE TABLE clip_items` SQL，共 5 处。本次改动同步更新了所有 5 处（均加上 `html_content TEXT`），保持一致，没有漏更新导致运行期 `no such column` 的风险。

长期来看，可以将建表 SQL 抽取为模块级常量（如 `pub(crate) const CREATE_CLIP_ITEMS_SQL: &str = ...`）以消除重复维护成本，但这属于跨多处文件的重构，超出本小功能范围，不要求本次解决。**观察，不阻塞**。

### 10. 测试质量（通过）

内联单测（db.rs）：

| 测试名 | 验证内容 | 断言质量 |
|---|---|---|
| `ingest_richtext_roundtrip_persists_html_and_kind` | roundtrip 读回 html_content 与原值一致、kind='richtext' | 具体值断言，非恒真 |
| `html_column_migration_idempotent_on_existing_db` | 迁移前无列→迁移后有列→可写读→二次迁移不报错 | 全链路逐步验证 |
| `dedup_by_plaintext_unchanged` | 纯文本二次 ingest 仍 Bumped 同一行，count=1 | 行为化命名，具体值断言 |
| `ingest_backfills_html_and_upgrades_kind_on_hit` | 旧行无 html → 补写后 html_content 和 kind 均正确 | 具体值断言 |

集成测试（richtext_storage.rs）：

- `fresh_db_persists_richtext_roundtrip_through_encrypted_store`：走真实 SQLCipher 文件库，防内存库通过/加密库不通过的假绿，目标明确。
- `plaintext_dedup_unchanged_on_encrypted_store`：加密库上验证去重语义。
- `TempDir` 生命周期正确（`_dir` 持有至测试函数结束），不会提前 drop 导致路径消失。

AAA 结构清晰，测试名描述行为，断言验具体值而非恒真。**通过**。

---

## 发现问题（置信度 ≥ 80 才报）

**无置信度 ≥80 的问题。**

以下为置信度 <80 的观察，供参考，不阻塞：

| 置信度 | 严重度 | 位置 | 描述 |
|---|---|---|---|
| 55 | 观察 | `src-tauri/src/db.rs`（多处）| 建表 SQL 在 5 个文件/位置重复，每次加列需逐一同步。本次已正确同步全部 5 处，无当前 bug；建议长期重构为常量，但不要求本次解决。 |
| 50 | 观察 | `src-tauri/src/db.rs:836` | `PRAGMA table_info({table})` 使用字符串格式化而非参数化；当前仅被私有调用链以硬编码参数调用，无注入风险；若未来函数可见性提升或调用方变化需重新评估。 |
| 45 | 观察 | `src-tauri/src/db.rs`（测试） | 缺少"旧行已有 html 时二次 ingest 不覆盖旧值"的专门测试（决策4第二条路径）。代码逻辑简单（`is_none()` 为 false 跳过），分支被 `ingest_backfills_html` 测试间接验证，当前置信度不达报告阈值。 |

---

## 是否合规

符合项目规范与 code-standards 的全部核查维度：
- 迁移幂等守卫逻辑正确，并发场景架构层面安全
- ingest 补写分支与决策4完全一致，不会覆盖已有 html
- SQL 全程参数化，无注入
- ORDER BY 含 `rowid DESC` 兜底
- 函数 ≤ 50 行，注释写"为什么"，无死代码/TODO/装饰性注释
- 测试断言验具体值，集成测试走真实加密库防假绿
- `richtext_storage.rs` 未追踪（新文件），是本次改动的组成部分

---

## 结论

**通过（APPROVE）**

核心改动正确，无置信度 ≥80 的 Critical 或 Important 问题。三条低置信度观察均不阻塞。

---

**VERDICT: APPROVE**
