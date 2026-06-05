---
id: V6-F1-S03-review
type: review_record
level: 小功能
parent: V6-F1
created: 2026-06-05T08:00:00Z
status: 通过
commit: 0db9178
acceptance_ids: [V6-F1-A05, V6-F1-A07]
author: code-reviewer
---

# 审查结论 · restart-command（S03）

## 审查范围

| 文件 | S03 新增/改动内容 |
|---|---|
| `src-tauri/src/ipc/update.rs` | 新增 `restart_app` 命令（L164-167）；订正模块级 doc + `check_for_updates` doc（去"占位 endpoint"说法） |
| `src-tauri/src/lib.rs` | `invoke_handler![]` 注册 `restart_app`（L178）；订正 `spawn_update_watcher` doc（去"留给 S02"说法） |
| `src-tauri/tests/update_watcher.rs` | 新增 `restart_app_command_exists_with_apphandle_signature` 编译期签名存在性测试（L18-25） |

S01/S02 已审过不重复。

---

## 逐维度核查

| 维度 | 核查结果 |
|---|---|
| 函数规模 | `restart_app` 函数体 1 行（L166）；doc 注释 10 行 ≤50 行 ✓ |
| 命名规范 | `restart_app` = 动词 + 名词，符合项目约定 ✓ |
| 注释风格 | doc 注释明确说明"为何用 core API 而非 plugin-process"、"为何签名 `()` 而非 `!`"、"正常路径根本走不到返回"；写"为什么"，无装饰性分隔 ✓ |
| 错误处理 | `restart()` 是 Tauri 核心 API，替换当前进程，无可失败路径，无需 Result 包装；符合设计 §四#2 ✓ |
| 无 plugin-process 引入 | 实现仅调用 `AppHandle::restart()`，未引入 `tauri-plugin-process`、未在 capabilities 添加 `process:*` 权限；`capabilities/default.json` 的 `core:default` 已足够（coder 实测 clippy exit 0 佐证）✓ |
| 签名 `()` 取舍 | doc 注释 L161-163 明确说明：`#[tauri::command]` 宏需要可序列化的具体回执类型；`-> !` 会触发 E0282；`restart()` 的 `!` 强转为 `()`，满足宏、doc 说明"正常路径根本走不到返回"，取舍合理 ✓ |
| 测试写法合规性 | `assert_ne!(cmd as usize, 0)`：函数指针地址确实近恒真，但核心价值在编译期签名绑定（`let cmd: fn(tauri::AppHandle) = restart_app;`）；tester 报告已说明其编译期守卫本质，且 `assert_ne!` 作用是强制 `cmd` 被"使用"以避免 dead code 警告，这是 Rust 生态已知惯用法；clippy exit 0 确认无 `unused_variables`/`unused_assignments` 告警 ✓ |
| 无死代码 / TODO / FIXME | grep 核验无残留 ✓ |

---

## A05 验收核对（restart_app 命令注册 + 函数存在）

**设计 §四#2 要求**：新增命令 `restart_app`，调用 `app.restart()`。

核对：

- `src-tauri/src/lib.rs` L178 已注册 `ipc::update::restart_app,`（紧接 `download_and_install_update`）。
- `src-tauri/src/ipc/update.rs` L164-167：`#[tauri::command] pub fn restart_app(app: tauri::AppHandle) { app.restart() }`，与设计 §四#2 "调用核心重启 API" 完全一致。
- `tests/update_watcher.rs` 签名存在性测试：绑定为 `fn(tauri::AppHandle)` 函数指针，tester 变异②（改函数名）触发编译失败，判别力已确认。

**A05 通过。**

---

## A07 安全核对（F1 级再核）

执行检查：

```bash
grep -q '"pubkey"' src-tauri/tauri.conf.json && \
! grep -rni 'dangerous' src-tauri/src/ipc/update.rs && \
echo "A07_PASS" || echo "A07_FAIL"
```

结果：**A07_PASS（exit 0）**。

- `tauri.conf.json` 含 `"pubkey"` 字段，签名校验已配置。
- `src-tauri/src/ipc/update.rs` 全文无 `dangerous` 旁路字样。
- `capabilities/default.json` 未引入 `process:*`；S03 新增命令仅依赖 `core:default`，无额外权限扩张。

**A07 通过。**

---

## 两处注释订正核对

### 1. `update.rs` — 去"占位 endpoint"说法

**旧注释**（diff 确认）：
- 模块 doc：`不在 setup 阶段自动调用，原因：当前 tauri.conf.json 使用占位 endpoint，自动检查会在每次启动时产生网络错误噪音`
- `check_for_updates` doc：`endpoint 为占位地址时会返回网络/解析错误，前端应以友好文案展示`

**新注释**（L6-11、L32-34）：
- 模块 doc 改为：`endpoint 为真实地址（github.com/EricChen4263/QuickQuick/…，CI 已产签名 latest.json），…后台任务定期自动检查…`
- `check_for_updates` doc 改为：`endpoint 已是真实地址，见模块顶部说明。网络不可达或版本清单解析失败时返回 Err，前端以友好文案展示`

验证无残留：`grep -rni '占位'` 在 `update.rs` 无任何匹配（exit 1）。**订正到位 ✓**

### 2. `lib.rs` — 去"留给 S02"说法

**旧注释**（diff 确认）：`spawn_update_watcher` doc 原文包含"真实下载/update://ready emit 留给 S02"。

**新注释**（L348-349）：改为"判定为应检查时调用 `run_one_update_check`：经 S02 已实现真实下载安装，并在就绪后 emit `update://ready` 通知前端"。

验证无残留：`grep -rni '留给 S0[123]'` 在 `update.rs` 和 `lib.rs` 无任何匹配（exit 1）。

注：`lib.rs:L348` 含 `S02` 字样，但语义为"回顾已完成的 S02 工作"，非前瞻性占位表述，不属过时说法。**订正到位 ✓**

---

## 发现问题（置信度 ≥ 80 才报）

### Critical 级

无。

### Important 级

无。

---

以下观察置信度低于 80，不阻塞，仅供参考：

- `lib.rs:L348` 注释说"经 S02 已实现"——此为回顾性叙述，意义清晰但略显"流水账"，可在后续清理中改为聚焦函数当下职责的描述（置信度约 30，纯风格）。
- `tests/update_watcher.rs:L24` 的 `assert_ne!(cmd as usize, 0)` 技术上近恒真，tester 已如实说明其编译期守卫本质；如项目日后统一 lint 规则要求运行期断言必须有真实语义，可改为 `let _ = cmd;` + 注释，但当前 clippy 不警告此惯用法，不构成问题（置信度约 30）。

---

## 是否合规

**符合。**

- **项目规范**：函数规模、嵌套、命名、注释、错误处理全部合规；无死代码、无 TODO/FIXME。
- **code-standards**：无 panic 泄漏、无不安全 API 滥用、注释写"为什么"、无装饰性分隔、无密钥/凭证硬编码。
- **设计文档 §四#2**：`restart_app` 调用 `AppHandle::restart()`，与设计指定的"核心重启 API"完全一致；doc 已说明"为何不用 plugin-process"与"签名 `()` 取舍"，合理且充分。
- **设计文档 §七重启权限**：实测 `core:default` 足够，无需引入 `tauri-plugin-process` 或 `process:*` 权限，开放点已落地验证。
- **验收项 V6-F1-A05**：命令注册 + 函数存在，代码核对通过；tester 命中校验 + 变异 sanity（两处变异均如期变红）确认判别力。
- **验收项 V6-F1-A07**：exit 0，pubkey 存在，无 dangerous 旁路，无权限扩张。

## 结论

**通过。无 Critical/Important 级问题，S03 可闭合。**

APPROVE
