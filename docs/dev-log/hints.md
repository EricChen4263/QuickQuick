# 项目速查池（hints）

> 跨版本滚动累积的「踩坑 / 可复用模式 / 项目本地约定」。小功能开工前 coder/tester 先过一遍，版本内即时避坑。
> 提炼自 retrospective-post-v3 / retrospective-v4 + 项目现状探测。

## Procedure（怎么跑·怎么验）

- **Rust 测试**：`cd src-tauri && cargo test`；提交前必跑 `cargo clippy --all-targets -- -D warnings`（`cargo test`/`check` 不报 dead_code，clippy 才拦）。
- **前端测试**：`pnpm test`（= `vitest run`）；组件测试用 `@testing-library/react` + `@testing-library/jest-dom`，样例见 `src/components/Select.test.tsx`、`src/shell/TitleBar.test.tsx`。
- **启动守卫**：配置/启动类改动后留意 `src-tauri/tests/boot_smoke.rs`（配置反序列化守卫）；配置类 bug 编译不报、单测难抓，发布前需真跑一次应用。
- **dev 数据隔离**：debug 构建用文件密钥库（绕钥匙串）、配置/DB 落 dev 子目录；切换密钥来源后旧 dev DB 无法解密需删库重建。

## Hint（坑·模式·本地约定）

- **排序查询必须确定性**：`ORDER BY` 主键之外必带稳定兜底（`, rowid DESC`）。同毫秒并列在并发负载下退化为 flaky 假绿（V4 两个独立阻塞同源）。
- **tester 抗 flaky 多跑**：并发/排序/时间戳/共享资源类，单跑一次的绿不可信，全量套件连跑 ≥3 次（理想 5），每次全绿才过。
- **tester 变异还原禁用 git**：在带未提交改动的文件上做变异，还原只能"改前 `cp` 备份 → 改后从备份复原"，**禁 `git checkout`/`git restore`**（会冲掉未提交修复）；开工/结束 `git status` 快照逐行比对自证干净。
- **coder 交付序**：多交付项任务，测试一过立即按序落地 ① 完成接入/wiring ② 写 coding.md 骨架 ③ 才润色。接入与留痕都是交付物，不是收尾——那正是被截断丢失的位置。
- **回合预算有限**：单次任务切到预算内、尽早落痕（coding.md/test.md），撞顶也留可续状态；超大任务交付最小片段。

### v6 自动更新本地约定

- **前端事件监听**：用 `import { listen } from "@tauri-apps/api/event"`，惯例见 `src/App.tsx`、`TranslateSourcePanel.tsx:204`。事件名常量集中定义（如 `TRANSLATE_HISTORY_CHANGED_EVENT`），update 事件用 `update://ready`。
- **自定义命令注册**：Rust 命令写在 `src-tauri/src/ipc/<域>.rs`，在 `lib.rs` 的 `invoke_handler![]` 注册（updater 域已有 `ipc::update::check_for_updates @ lib.rs:163`）；前端在 `src/ipc/ipc-client.ts` 加 `invoke` 封装。
- **重启权限**：`app.restart()` 是 Rust 端核心 API，自定义 `restart_app` 命令前端可直接 invoke，`capabilities/default.json` 已含 `core:default`，**预期无需引入 plugin-process**——S03 实测确认即可。
- **updater 现状**：`check_for_updates` 已在 `ipc/update.rs`（仅查不装）；endpoint 为**真实地址**（`update.rs:6-8` "占位 endpoint"注释已过时，需修正）；`auto_update` 开关 get/set 在 `ipc/settings.rs:587`、字段在 `settings.rs:67`、前端开关已接线但后端无消费者。
- **下载可测性**：`tauri-plugin-updater` 的 `Update` 不易在单测构造——把"是否应检查"抽成纯函数 `should_check(enabled, already_ready) -> bool`、把就绪事件 payload 构造抽成纯函数单测；真实下载/重启隔离到薄封装层，归真机 manual_confirm。
