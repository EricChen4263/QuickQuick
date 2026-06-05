---
id: V6-F1-S02-review
type: review_record
level: 小功能
parent: V6-F1
status: 通过
commit: 0db9178
acceptance_ids: [V6-F1-A03, V6-F1-A07]
author: code-reviewer
created: 2026-06-05T08:00:00Z
---

# 审查结论 · 下载安装薄封装 + 就绪事件 + 命令注册（S02）

## 审查范围

`git diff HEAD` 中以下文件的 S02 部分：

- `src-tauri/src/ipc/update.rs`：新增 `UPDATE_READY_EVENT`、`UpdateReadyPayload`、`build_ready_payload`、`download_install_and_notify`（私有薄封装）、`download_install_for_watcher`（后台入口）、`download_and_install_update`（`#[tauri::command]`），以及对应单测 `update_ready_payload_carries_version`。
- `src-tauri/src/lib.rs`：`run_one_update_check` 的 `Ok(Some)` 分支改调薄封装；`invoke_handler![]` 注册 `download_and_install_update`；过渡性注释更新。

S03（`restart_app`）不在本轮范围，不报缺失问题。

---

## 审查维度核对

| 维度 | 结论 |
|---|---|
| 函数规模（≤50 行） | `download_install_and_notify`（17行）、`download_install_for_watcher`（4行）、`download_and_install_update`（14行）全部合规 ✓ |
| 嵌套深度（≤3层） | 最深 2 层（match → arm），合规 ✓ |
| 命名规范 | `build_ready_payload`（动词+名词）、`download_install_and_notify`/`download_install_for_watcher`（描述性）、常量 UPPER_SNAKE；合规 ✓ |
| DRY | 后台与手动路径共用同一薄封装 `download_install_and_notify`，下载逻辑仅一份 ✓ |
| 注释风格 | 新增注释均写"为什么"（emit 失败不回滚就绪的原因、手动独立标志的理由、薄封装隔离 I/O 的原因）；无装饰性分隔注释 ✓ |
| 错误处理 | `download_and_install`/`emit`/`check` 均经 `Result` 处理；下载失败 eprintln + return Err，不 panic、不置位；后台路径 `let _ = ...` 静默忽略，手动路径回传 Err 给前端，语义分明 ✓ |
| Ordering::Relaxed 正确性 | `already_ready` 单一写者（download_install_and_notify）+ 单一读者（watcher loop），无跨变量 happens-before 要求，Relaxed 语义充分 ✓ |
| 验签未旁路 | 无 `dangerous` 旁路调用，`tauri.conf.json` 含 `pubkey` 字段，签名校验由 updater 插件内部执行 ✓ |
| 测试覆盖 | `update_ready_payload_carries_version` 精确命中（N=1），两次变异均如期变红，判别力已证（tester test.md 已记录） ✓ |
| 命令注册 | `download_and_install_update` 已在 `invoke_handler![]` 注册（lib.rs:177） ✓ |

---

## 发现问题

### Important 级（非阻塞，建议修正）

**[I-01]** · `src-tauri/src/ipc/update.rs` · 第 34 行 · 过时注释未修正 · 置信度 85

```
/// endpoint 为占位地址时会返回网络/解析错误，前端应以友好文案展示。
```

该行属于 `check_for_updates` 的 doc comment，仍沿用旧版"占位地址"措辞。设计文档 `docs/design/auto-update.md` 第 27 行明确指出该前提已过时，并要求 S02 落地时同步修正注释。文件顶部模块级注释（第 7 行）已正确描述"endpoint 为真实地址"，两者矛盾。

**建议修正**：
```rust
// 改为：
/// endpoint 为真实地址（CI 已产签名 latest.json），前端应以友好文案展示错误。
```

**[I-02]** · `src-tauri/src/lib.rs` · 第 347 行 · S01 过渡说明未随 S02 接入更新 · 置信度 82

```
/// 本小功能（S01）只判定 + 记录 + 置位；真实下载/`update://ready` emit 留给 S02。
```

该行是 `spawn_update_watcher` doc comment 的末句，描述的是 S01 的中间状态。S02 已将 `run_one_update_check` 的 `Ok(Some)` 分支改为真实下载安装，该说明已过期（S02 diff 修改了 `run_one_update_check` 的同类注释，但 `spawn_update_watcher` 自身的 doc comment 漏更）。读者查看 `spawn_update_watcher` 的 doc 时会得到错误的现状描述。

**建议修正**：
```rust
// 删除末句，或改为：
/// 检测到可用更新后，内部调用 `ipc::update::download_install_for_watcher` 执行静默下载安装，
/// 完成后 emit `update://ready` 并置位去重，不再重复检查。
```

---

## A07 安全核对（V6-F1-A07）

执行命令：
```bash
grep -q '"pubkey"' src-tauri/tauri.conf.json && ! grep -rni 'dangerous' src-tauri/src/ipc/update.rs
```

**exit code：0（PASS）**

- `tauri.conf.json` 含 `pubkey` 字段（minisign 公钥已配置），updater 插件签名校验正常生效。
- `src-tauri/src/ipc/update.rs` 中无 `dangerous` 关键字，签名校验未被旁路。
- A07 验收通过。

---

## A03 验收核对（V6-F1-A03）

tester test.md 已记录：

- `build_ready_payload("1.2.3").version == "1.2.3"` 精确命中（N=1，397 filtered out）
- 变异①（`String::new()`）和变异②（`"9.9.9"`）均如期让测试变红，判别力已证
- A03 验收通过。

---

## 综合评估

两处 Important 级问题均为**过时注释未清理**，不影响运行时行为与正确性，无 Critical 问题。

| 维度 | 结果 |
|---|---|
| 逻辑正确性 | 无 bug，emit 失败后仍置位 already_ready 属有意设计（注释已说明），符合设计约定 |
| 安全性（A07） | 通过，签名校验未旁路 |
| DRY / 代码质量 | 薄封装层级清晰，后台/手动路径真正共用，无重复逻辑 |
| 注释规范 | 新增注释均写"为什么"，但两处预存注释未随 S02 接入同步更新（I-01/I-02） |
| 错误处理 | 后台路径静默、手动路径回传，语义明确，无 panic 泄漏 |
| 测试覆盖 | 纯函数部分有单测且具真实判别力；不可测的真实下载路径由设计隔离到薄封装层，归真机 manual_confirm |

---

## 结论

**通过。**

两处 Important 问题（过时注释）不阻塞合并，建议在 S02 提交前或下一个小功能开工前顺手修正，保持文档与实现同步。无 Critical 问题。

WARNING
