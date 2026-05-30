---
id: V0-F1-S01-code
type: coding_record
level: 小功能
parent: V0-F1
children: []
created: 2026-05-30T20:43:46Z
status: 通过
commit: WIP
acceptance_ids: [V0-F1-A01, V0-F1-A02, V0-F1-A03, V0-F1-A04, V0-F1-A06]
evidence:
  - src-tauri/src/lib.rs
  - src-tauri/src/main.rs
  - src-tauri/build.rs
  - src-tauri/tauri.conf.json
  - src-tauri/capabilities/default.json
  - src-tauri/Cargo.toml
  - tsconfig.json
  - tsconfig.app.json
  - src/smoke.test.ts
author: coder
---

# 编码记录 · V0-F1-S01 Tauri 脚手架初始化

## 做了什么

完成 QuickQuick V0 Tauri2+React 骨架工程的最终收尾：修复两处类型识别阻塞（`import.meta.env` 的 `vite/client` 类型注入、`lib.rs` 模块文档注释格式），确保全链路构建、类型检查、lint、测试全部通过，验证 A01-A04、A06 验收项达标。

第一次打回后修复（2026-05-31）：按 code-reviewer 审查意见修复 C-1/I-1/I-2/I-3/I-4 共 5 项。

## 关键决策与理由

- **`tsconfig.json` 而非仅 `tsconfig.app.json` 加 `types: ["vite/client"]`**：`pnpm build` 调用 `tsc -b`，实际以 `tsconfig.json`（含 `include: ["src"]`）作为 src 目录的编译配置；仅在 `tsconfig.app.json` 加类型不能被 `tsc -b` 模式读取，必须在根配置加才有效。
- **`lib.rs` 模块文档改为 `//!`（内部注释）**：原 `///`（外部注释）写在文件顶部，与下方 `run()` 的 `///` 之间有空行，触发 `clippy::empty_line_after_doc_comments`。改为 `//!` 是 Rust 惯用的 crate/模块级文档写法，语义正确且消除 lint 错误。
- **[C-1] updater.pubkey 来源说明**：`tauri.conf.json` 中的 `pubkey`（`dW50cnVzdGVk...` base64 编码的 minisign 公钥）为脚手架阶段执行 `tauri signer generate` 生成的**一次性抛弃密钥**，对应私钥未入库、已丢弃。`endpoints` 保持 `placeholder.example.com` 占位值。**正式发版前必须重新生成密钥对，用新 pubkey 替换此值，并将 endpoints 改为真实更新服务器地址。**
- **[I-1] Cargo.toml 预埋 rusqlite/keyring/uuid**：`Cargo.toml` 预埋 `rusqlite`（含 bundled-sqlcipher-vendored-openssl）、`keyring`、`uuid` 三个依赖，S01 阶段代码中暂无引用。这属于设计文档§六（加密数据库）与§十（密钥 provider、UUID 主键）的**明确预埋要求**，将在 V0-F3 小功能中正式使用，非越界引入。
- **[I-2] setup 闭包简化**：删除无意义的 `let _ = app.handle()`，setup 直接返回 `Ok(())`，注释改为骨架意图说明，消除误导性注释。
- **[I-3] CSP 由 null 改为最小策略**：按 Tauri 官方最小权限推荐，将 `csp: null` 改为 `"default-src 'self'; style-src 'self' 'unsafe-inline'"`，满足安全基线。
- **[I-4] windowRoute 归属说明**：`src/shell/windowRoute.ts` 及其测试 `windowRoute.test.ts` 归属 **V0-F2-A03（s03-prewarm-window）**，不属于 S01 范围。已从本记录的 evidence 列表中移除；`windowRoute.test.ts:4` 的 `V0-F2-A03` 注释保持不变（归属正确）。

## 改动文件（S01 范围内）

- `tsconfig.json` — `compilerOptions` 加 `"types": ["vite/client"]`，使 `import.meta.env` 在 `tsc -b` 模式下被识别
- `tsconfig.app.json` — `compilerOptions` 加 `"types": ["vite/client"]`（对应 `--project tsconfig.app.json` 直接调用场景）
- `src-tauri/src/lib.rs` — 模块顶部文档注释由 `///` 改为 `//!`，修复 clippy `empty_line_after_doc_comments` 错误；删除 `let _ = app.handle()` 无意义空操作（第一次打回修复）
- `src-tauri/tauri.conf.json` — CSP 由 `null` 改为最小策略（第一次打回修复）

## 自测结论（TDD 红-绿-重构）

前序 coder 已按 TDD 完成核心实现（`windowRoute.ts` 有完整单测，归属 V0-F2-A03；`lib.rs` 有 `smoke_lib_loads` 冒烟测试）；本轮为收尾修复，TDD 方向：

- RED：运行 `pnpm build` → `tsc -b` 报 `Property 'env' does not exist on type 'ImportMeta'`；运行 `cargo clippy -D warnings` → `empty_line_after_doc_comments` 错误。
- GREEN：分别在 `tsconfig.json` 加 `types: ["vite/client"]`、将 `lib.rs` 顶部 `///` 改为 `//!`，两处修复后重跑均通过。
- REFACTOR：修改最小化，未引入额外变更。

验收结果：

| 验收项 | 指标 | 结果 |
|--------|------|------|
| A01 | `cargo build` exit=0 | pass |
| A02 | `pnpm build` exit=0，dist 产物生成 | pass |
| A03 | clippy exit=0；tsc --noEmit exit=0；无 TODO/FIXME | pass |
| A04 | be_test: 1 passed；fe_test: smoke.test.ts passed | pass |
| A06 | `tauri.conf.json` 含 `updater` 配置 | pass |

code-standards 自检：
- 格式：Rust `//!` 注释符合惯用法；TS 2 空格缩进保持不变。
- 函数：无新增函数，现有函数均 ≤50 行。
- 命名：无新增标识符。
- 注释：改为内部文档注释，解释"为什么"保留在 coding.md；setup 注释改为骨架意图说明。
- 类型：`vite/client` 补全使 `import.meta.env` 类型安全，无 `any` 逃逸。
- 测试：AAA 结构，`smoke_lib_loads` 命名描述行为。
- 安全：updater pubkey 为抛弃性脚手架密钥，来源已在本文件记录，私钥未入库；CSP 已设最小策略。
