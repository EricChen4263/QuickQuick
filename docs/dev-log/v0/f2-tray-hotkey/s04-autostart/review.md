---
id: V0-F1-S04-review
type: review
level: 小功能
parent: V0-F1
children: []
created: 2026-05-31T10:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F1-A05]
evidence: []
author: code-reviewer
---

# 审查结论 · V0-F1-S04 自启动偏好配置

## 审查维度

依据 code-standards（格式/命名/函数/注释/类型/性能/测试/安全）+ 项目规范 + 设计文档§二。审查文件：`src-tauri/src/autostart.rs`（新增）、`src-tauri/src/lib.rs`（mod 暴露 + setup）、`src-tauri/tests/autostart.rs`（新增）。

## 发现问题（置信度 ≥ 80）

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| Important | `lib.rs` 的 setup 闭包从未读取 `AutostartConfig::load_or_default(...)`、也未据 `enabled` 调用插件 enable/disable。coding.md "关键决策" 声称 "真实注册由 setup 层读取偏好后决定调用时机"，但该逻辑完全缺席：插件无条件注册 LaunchAgent，用户 enabled=false 被静默忽略，开关形同虚设。置信度 90。 | `src-tauri/src/lib.rs`（setup 闭包） | V0-F1-A05 "自启动开关可读写" 隐含偏好须作用于运行时。修复：setup 内 `AutostartConfig::load_or_default(&config_path)`，按 `enabled` 调 `tauri_plugin_autostart::ManagerExt` 的 `app.autostart().enable()/disable()`。 |
| Important | `load_or_default` 是为 setup 首次启动安全调用专门设计（coding.md 明确），但无任何测试直接调用 `load_or_default(不存在路径)` 并断言回退 `enabled==true`。若实现回退值有 bug，现有两测试仍全过、首次启动自启动却为关。与 S02 Translate 分支无测试同性质。置信度 85。 | `src-tauri/tests/autostart.rs`（缺失用例） | 新增 `autostart_load_or_default_when_file_not_exist`：传不存在路径，断言返回 `enabled==true`。约 8 行。 |

## 是否合规

**autostart.rs 实现完全合规**：默认开（`Default` → `enabled:true`，注释引设计文档§二）；`AutostartError` thiserror 枚举（`SerdeError`/`IoError` 均 `#[from]`）；save/load `?` 传播无裸 unwrap/panic；`load_or_default` 的 `unwrap_or_default()` 为有意回退且注释说明；格式/命名/注释（`//!`/`///` 含 `# Errors`）/函数长度（最长 3 行）/嵌套（1 层）全合规；无 TODO/FIXME。
**测试部分合规**：OS 副作用隔离达成（仅数据模型 + tempfile，零触发 LaunchAgent）；AAA 结构、断言有判别力、无恒真伪测试；缺 `load_or_default` 文件不存在路径覆盖（I-2）。
**lib.rs 存在语义缺口**（I-1）：setup 未消费 `AutostartConfig`，偏好与真实行为脱节。

## 结论

**打回。** 须修 I-1（setup 加载偏好并按 enabled 调插件 enable/disable）+ I-2（补 load_or_default 文件不存在测试）。修复后复审。

---

## 复审结论（2026-05-31）

**status: 通过**

- **I-1**（setup 未消费偏好）：`lib.rs` 新增 `apply_autostart_preference`，setup 首行调用，完整实现"读 `AutostartConfig::load_or_default` → 按 `enabled` 调 `app.autolaunch().enable()/disable()`"；配置目录获取/创建失败与插件调用失败均 `eprintln` 不 panic，优雅降级完整。coding.md 与实现对齐。
- **I-2**（`load_or_default` 无测试）：`tests/autostart.rs` 新增 `autostart_load_or_default_when_file_not_exist`，AAA、传不存在路径断言 `enabled==true`、非恒真，覆盖首次启动回退。
新增代码无裸 unwrap/panic，无新引入≥80 高危。
