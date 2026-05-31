---
id: V4-F1-S02-review
type: review
level: 小功能
parent: V4-F1
children: []
created: 2026-05-31T08:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V4-F1-A02]
author: code-reviewer
---

# 审查记录 · 翻译 IPC 命令层（V4-F1-S02）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src-tauri/src/ipc/translate.rs` | 新建 | `HttpExecutor` trait、`UreqExecutor`、`FakeExecutor`、`translate_text_impl`、`list_translate_history_impl`、两个 `#[tauri::command]`、两个 DTO |
| `src-tauri/src/ipc/mod.rs` | 一行 diff | 新增 `pub mod translate;` |
| `src-tauri/src/translate/history.rs` | diff 部分 | 新增 `list_translate_history` 函数与 `TranslateHistoryRow` 结构体 |
| `src-tauri/Cargo.toml` / `Cargo.lock` | 依赖新增 | `ureq = { version = "2", features = ["tls"] }`，锁定版本 2.12.1 |
| `src-tauri/tests/ipc_translate.rs` | 新建 | impl 层集成测试（6 个函数，覆盖 V4-F1-A02） |

参照：V4-F1-A02、项目规范、code-standards、tester 动态证伪报告（6/6 通过）。

---

## 问题清单

### Critical

无。

### Important

无（所有潜在关注点经逐项静态分析后置信度均低于 80%，详见下方逐维度核查）。

---

## 逐维度核查

### 1. 超时设置

`UreqExecutor.execute()` 在每次调用时构造 `AgentBuilder::new().timeout(Duration::from_secs(10)).build()`。

`ureq::AgentBuilder::timeout()` 覆盖整个请求生命周期（DNS 解析 + 连接 + 重定向 + 响应体读取），10 秒上限对翻译 API 合理。**通过。**

### 2. 错误映射与 API Key 泄露分析

#### 2a. 当前路径（MyMemory，无 Key）

`translate_text_impl` 中固定使用 `MyMemoryProvider::new(None)`，URL 结构为 `https://api.mymemory.translated.net/get?q=<percent_encoded_text>&langpair=...`，不含任何 API Key 或邮箱参数。

#### 2b. ureq::Error::Status 与 URL-in-error 问题

ureq 2.x 中，`Error::Status(code, response)` 的 `Display` 实现为 `"{response.get_url()}: status code {code}"`。若 MyMemory 返回真实 HTTP 4xx/5xx（网络层），该 URL（含 percent-encoded 源文本）将出现在错误字符串中并随 `Result<_, String>` 传回前端。

分析结论：

- 传回的是用户自己输入的翻译文本，非任何凭据或密钥，不满足"安全红线：日志不打印敏感信息（密钥/salt/nonce）"的触发条件。
- MyMemory API 惯例：应用层错误（配额超限 403、频率超限 429）通过 HTTP 200 + 响应体 `responseStatus` 字段表达；ureq `Error::Status` 仅在服务器真正返回 HTTP 错误状态码时触发，属极低概率路径。
- 置信度：30%，低于 80% 阈值，不阻断。

#### 2c. HTTP 4xx 语义归类

当 ureq `Error::Status` 触发时，`UreqExecutor` 将其映射为 `TranslateError::Network`，而非 `Auth`/`ServerError`。`TranslateError::Network` 的语义定义为"网络层错误"，HTTP 应用层状态码归入此变体存在语义偏差。但对于当前 MyMemory 路径（实践中不触发），以及该错误字符串仅用于前端展示不影响功能正确性，置信度 20%，不阻断。

### 3. 错误路径无 panic

`UreqExecutor.execute()` 无裸 `unwrap`/`expect`；方法匹配使用 early return + `Err`，不支持的 HTTP 方法返回 `TranslateError::Network` 而非 panic。`translate_text_impl` 和 `list_translate_history_impl` 全程 `?` 传播，无 panic 路径。**通过。**

### 4. Mutex 锁中毒处理

两个命令函数均使用 `state.0.lock().map_err(|e| format!("锁获取失败: {e}"))`。`PoisonError<MutexGuard<Connection>>` 的 Display 输出为固定字符串，不含数据库内容或连接状态，无敏感泄露。与 S01 模式一致。**通过。**

### 5. SQL 参数化查询

新增的 `list_translate_history` 函数使用静态 SQL 字符串，无用户输入，`query_map([], ...)` 空参数绑定，无注入面。`add_translate_history`（pre-existing）使用 `rusqlite::params![]` 参数化，全链路安全。**通过。**

### 6. DTO camelCase 与前端契约

`TranslateResultDto`（`#[serde(rename_all = "camelCase")]`）：
- `translated` → `translated`
- `source_lang` → `sourceLang`
- `target_lang` → `targetLang`

`TranslateHistoryDto`（`#[serde(rename_all = "camelCase")]`）：
- `id` → `id`、`source_text` → `sourceText`、`translated_text` → `translatedText`
- `source_lang` → `sourceLang`、`target_lang` → `targetLang`
- `provider_id` → `providerId`、`created_utc` → `createdUtc`

字段映射完整，与预期 TypeScript 接口对齐。**通过。**

### 7. resolve_direction 参数传递正确性

`translate_text_impl` 签名中 `configured_target: Option<&str>`；调用 `configured_target.map(Lang::new)` 利用 `&str: Into<String>` 正确构造 `Option<Lang>`，传入 `resolve_direction(text, target_lang)`，与函数签名 `Option<Lang>` 完全匹配。Tauri 命令层用 `target.as_deref()` 将 `Option<String>` 转为 `Option<&str>`，链路正确。**通过。**

### 8. 历史写入失败即整体失败语义

`translate_text_impl` 中 `add_translate_history().map_err(|e| e.to_string())?`：历史写入失败时翻译整体返回 `Err`，译文不返回给前端。

coding.md 已标注"历史写入失败不中断译文返回的语义待后续评估；此处选择失败即报错，保持一致性"。本次审查维持此决策：写入失败即意味着系统状态异常（数据库错误），向前端报错是正确的防御姿态；如后续产品方向需要"即使历史失败也展示译文"，届时在 impl 层将 `?` 改为 `if let Err(e) = ... { log } OK(...)` 即可，改动最小。**通过（标记为已知 pending 决策，不阻断）。**

### 9. ureq 依赖 features 与版本锁定

`ureq = { version = "2", features = ["tls"] }`：`tls` feature 启用 rustls（纯 Rust TLS 实现），与项目既有 `rusqlite` 的 `bundled-sqlcipher-vendored-openssl` 并存，属不同层级。rustls 是 ureq 2.x 的标准 TLS 后端，无额外 OpenSSL 依赖冲突。Cargo.lock 锁定 2.12.1，供应链可审计。version = "2" 允许小版本更新（semver 兼容），可接受。**通过。**

### 10. FakeExecutor 可见性

`FakeExecutor`、`FakeExecutor::new`、`call_count` 均为 `pub`（非 `#[cfg(test)]`）。集成测试位于 `tests/` 目录，无法访问 `cfg(test)` 限定的项，`pub` 是唯一可行方案。`FakeExecutor` 仅依赖 `AtomicU32` 与 `String`，无生产副作用。**通过。**

### 11. 代码规范符合度

| 检查项 | 结论 |
|---|---|
| 函数 ≤ 50 行、嵌套 ≤ 3 层 | 最长函数 `translate_text_impl` 约 35 行；`execute` 约 30 行；嵌套最深 2 层 |
| 参数 ≤ 5 个 | 最多 4 个（translate_text_impl），符合 |
| 命名描述性 | `translate_text_impl`、`list_translate_history_impl`、`db_list_translate_history` 均为动词+名词 |
| 注释写「为什么」，无装饰性分隔符 | 公共接口有 `///` 文档，注释说明选型理由（ureq 同步、AtomicU32）；无 `═══/───` 横线 |
| 无死代码注释、无 TODO/FIXME | 通过 |
| 无裸 `unwrap`（生产代码） | 通过；测试中 `expect("...")` 含描述符合惯例 |

**通过。**

### 12. 测试质量

6 个集成测试覆盖：中文→en 方向、英文→zh 方向、历史写入 +1、空文本 Err（executor call=0）、全空白 Err（executor call=0）、list 倒序。使用真实加密临时库（`open_or_create + tempdir`）+ `FakeExecutor` 隔离网络，无恒真断言。tester 动态证伪报告显示 2 变异如期变红、边界探测 5 场景无 panic。**通过。**

---

## 低于阈值的观察项（不阻断，后续参考）

**per-call AgentBuilder 构造**（置信度约 45%）

`UreqExecutor.execute()` 每次调用都新建 `Agent`，无法复用 TLS 连接池。翻译为低频操作，当前阶段不构成性能瓶颈（YAGNI）。若后续高频场景出现，可将 `Agent` 提升为 `UreqExecutor` 的字段（单行改动）。

**ureq HTTP 4xx/5xx 语义分类**（置信度约 20%）

`Error::Status` 映射为 `TranslateError::Network` 存在语义偏差；实践中 MyMemory 不产生此路径，Baidu/Google 为 POST，不在 S02 范围。若未来接入更多 provider 且需精确错误分类，可在 UreqExecutor 中对 `Error::Status` 单独处理。

---

## 对 S04 / 前端的注意事项

1. **S04 注册**：`invoke_handler` 需列入 `translate_text`、`list_translate_history` 两个命令（snake_case 即 Tauri 命令名，与 JS `invoke("translate_text", ...)` 对应）；两个命令均依赖 `AppDb` 状态，`app.manage(AppDb(...))` 需在注册前完成（S01 已有此约定，S02 共享同一 State）。

2. **前端 TypeScript 接口**：
   - `translate_text` 返回 `{ translated: string; sourceLang: string; targetLang: string }`
   - `list_translate_history` 返回 `Array<{ id: string; sourceText: string; translatedText: string; sourceLang: string; targetLang: string; providerId: string; createdUtc: number }>`
   - `target` 参数为可选 `string | null`（`Option<String>` 对应），不传则由本地检测决定方向

3. **错误处理**：两个命令均返回 `Result<_, String>`，Tauri 映射为 JS reject。前端 IPC 封装（S05）应区分"空文本"错误（用户提示）与"网络/数据库"错误（系统提示）；错误消息当前为中文字符串，可直接展示。

4. **历史接口无分页**：`list_translate_history` 返回全量历史，当前为 YAGNI 设计。S05 前端 IPC 封装层如有分页需求，须等待 Rust 侧添加 `limit/offset` 参数支持，不可在前端侧截断（数据库层未限制，全量数据已传输）。

---

## 总结论

**无未决高危，放行。**

全部置信度 ≥ 80% 的静态检查项均通过。代码延续 S01 的"命令薄包装 + 可单测 impl"模式，`HttpExecutor` 抽象隔离网络依赖设计清晰；超时 10 秒覆盖全链路；Mutex 锁中毒处理正确一致；SQL 全静态无注入面；DTO camelCase 与前端契约对齐；`resolve_direction` 参数链路正确；`ureq` 版本锁定、TLS 后端无冲突；测试覆盖主路径与边界，tester 变异证伪通过。

V4-F1-A02 审查维度通过。可进入 S03（settings-cmd）。
