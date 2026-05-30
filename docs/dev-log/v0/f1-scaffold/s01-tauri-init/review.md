---
id: V0-F1-S01-review
type: review
level: 小功能
parent: V0-F1
children: []
created: 2026-05-31T08:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F1-A01, V0-F1-A02, V0-F1-A03, V0-F1-A04, V0-F1-A06]
evidence: []
author: code-reviewer
---

> 首轮打回 1 Critical + 4 Important；coder 修复后复审通过。终态 status=通过。复审结论见文末。

# 审查结论 · V0-F1-S01 Tauri 脚手架初始化

## 审查维度

依据 code-standards skill（格式/命名/函数职责/注释/类型安全/性能/测试/安全）与 QuickQuick 项目规范（设计文档 + 全局 CLAUDE.md），重点关注：
- Rust：unwrap/expect 可失败路径、错误处理、clippy 友好度
- 遗留 TODO/FIXME、占位/空实现标注
- tauri.conf.json 权限最小化、updater pubkey 是否为明确占位
- 是否越界实现了不属于 S01 的功能

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| Critical | `updater.pubkey` 为真实 minisign 公钥（RWSPhdPOeFC…），来源与对应私钥保管状态在代码库中完全无记录；与 `placeholder.example.com` endpoints 语义不一致 | `src-tauri/tauri.conf.json:43` | 全局规则"密钥不入库"；Tauri updater 文档要求 pubkey 与签名私钥配套管理。修复：在 coding.md 或 README 中明确记录 pubkey 来源及私钥保管位置，或替换为带明文 `PLACEHOLDER_` 前缀的占位值并注明"发版前替换" |
| Important | `Cargo.toml` 引入 `rusqlite`（含 bundled-sqlcipher-vendored-openssl）、`keyring`、`uuid`，但 S01 的 `src-tauri/src/` 中无任何使用，属越界引入后续小功能依赖；拖慢编译且 coding.md 未说明预埋意图 | `src-tauri/Cargo.toml:25-26` | YAGNI 原则；S01 范围限定为脚手架骨架。修复：移除三个 crate，在各自归属小功能再引入；或在 coding.md 明确标注"S01 预埋" |
| Important | `let _ = app.handle()` 无任何副作用（`handle()` 仅克隆 AppHandle 立即丢弃），注释称"日志标记"但无任何日志调用，误导读者 | `src-tauri/src/lib.rs:24` | 禁注释不符实际行为；禁无意义操作。修复：删除该行，setup 直接 `Ok(())`，注释改为骨架意图说明 |
| Important | `csp: null` 关闭内容安全策略，不符合 Tauri 官方最小权限推荐（官方明确建议至少设 `"default-src 'self'"`） | `src-tauri/tauri.conf.json:23` | Tauri 安全文档；最小权限原则。修复：改为 `"default-src 'self'; style-src 'self' 'unsafe-inline'"` |
| Important | 测试注释引用 `V0-F2-A03`，但 `windowRoute.ts` 被 coding.md 误列为 V0-F1-S01 evidence；实际 windowRoute 归属 V0-F2-A03（s03-prewarm-window）。注释本身正确，错在 S01 越界认领该文件 | `src/shell/windowRoute.test.ts:4` / `coding.md` | 验收追踪规范；acceptance_ids 须对应小功能自身。修复：从 S01 coding.md 的 evidence 移除 windowRoute（其归属 s03），保留测试注释的 V0-F2-A03 |

## 是否合规

不合规。存在 1 项 Critical（密钥管理安全红线）和 4 项 Important（越界依赖、误导性代码、CSP 基线、验收追踪错误），需全部修复后重新审查。

代码风格、命名、函数职责、格式、类型安全（无 `any`、`vite/client` 类型注入）、`main.rs` 瘦入口、`build.rs` 极简均符合规范；`lib.rs` 的 `//!` 模块注释、`# Panics` 文档节、`windows_subsystem` 属性均正确；capabilities 权限集基本最小化（仅 autostart 三个 allow + updater:default + core:default）。测试覆盖 windowRoute 纯函数，AAA 结构规范，smoke_lib_loads 在脚手架阶段可接受。

## 结论

打回。必改项（按优先级）：
1. **C-1** `updater.pubkey` 来源与私钥保管必须在代码库中有明确记录，或替换为明文占位值。
2. **I-1** 移除 `rusqlite`/`keyring`/`uuid` 三个未使用依赖，或 coding.md 明确注明预埋意图。
3. **I-2** 删除 `lib.rs:24` 的 `let _ = app.handle()`。
4. **I-3** 将 `csp: null` 改为最小 CSP 策略。
5. **I-4** 从 S01 coding.md evidence 移除 windowRoute（归属 s03），保留测试注释 V0-F2-A03。

全部修复后重提，重新走 code-reviewer 审查。

---

## 复审结论（2026-05-31）

**复审人：** code-reviewer　**复审状态：** 通过

### 5 项打回问题逐条确认

| 编号 | 原问题 | 修复确认 |
|------|--------|----------|
| C-1  | updater.pubkey 来源与私钥保管无记录 | coding.md 已明确记录：pubkey 为脚手架阶段一次性抛弃密钥，私钥未入库、已丢弃；发版前必须重新生成并替换。已解决。 |
| I-1  | rusqlite/keyring/uuid 越界引入，无预埋说明 | coding.md 已注明三者为设计文档§六/§十的明确预埋依赖，归属 V0-F3，非越界。已解决。 |
| I-2  | `let _ = app.handle()` 无意义空操作与误导注释 | lib.rs setup 闭包已简化为直接 `Ok(())`，注释改为骨架意图说明。已解决。 |
| I-3  | `csp: null` 关闭内容安全策略 | tauri.conf.json 已改为 `"default-src 'self'; style-src 'self' 'unsafe-inline'"`。已解决。 |
| I-4  | coding.md evidence 误列 windowRoute（归属 s03） | evidence 已移除 windowRoute，改为 smoke.test.ts；coding.md 明确说明 windowRoute 归属 V0-F2-A03。已解决。 |

### 轻量整体复检

扫描 S01 全部改动文件（lib.rs、tauri.conf.json、tsconfig.json、tsconfig.app.json、Cargo.toml、smoke.test.ts），未发现新引入的置信度 ≥ 80 高危或重要问题。代码风格、命名、类型安全、注释规范、权限最小化均符合 code-standards 与项目规范。

### 最终结论

**status: 通过** —— S01 所有打回项已全部修复，复检无新问题。可进入下一小功能。
