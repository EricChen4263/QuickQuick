# 项目速查池（hints）

> 跨版本滚动累积的「踩坑 / 可复用模式 / 项目本地约定」。小功能开工前 coder/tester 先过一遍，版本内即时避坑。
> 提炼自 retrospective-post-v3 / retrospective-v4 + 项目现状探测。

## Procedure（怎么跑·怎么验）

- **Rust 测试**：`cd src-tauri && cargo test`；提交前必跑 `cargo clippy --all-targets -- -D warnings`（`cargo test`/`check` 不报 dead_code，clippy 才拦）。
- **命中校验取原始输出须绕 RTK 代理**（V6 复盘 V6-RETRO-FRICTION-1）：tester 命中校验 / producer 版本裁决需要原始 `test <名> ... ok` / `Tests N passed`（N≥1）逐行证据防假绿时，本机 cargo/pnpm 经 RTK hook 改写后输出被压成摘要、看不到逐测试名命中——须用 `rtk proxy cargo test …` / `rtk proxy pnpm test …`（或前端加 `--reporter=verbose`）绕过代理取原始证据。
- **前端测试**：`pnpm test`（= `vitest run`）；组件测试用 `@testing-library/react` + `@testing-library/jest-dom`，样例见 `src/components/Select.test.tsx`、`src/shell/TitleBar.test.tsx`。
- **启动守卫**：配置/启动类改动后留意 `src-tauri/tests/boot_smoke.rs`（配置反序列化守卫）；配置类 bug 编译不报、单测难抓，发布前需真跑一次应用。
- **dev 数据隔离**：debug 构建用文件密钥库（绕钥匙串）、配置/DB 落 dev 子目录；切换密钥来源后旧 dev DB 无法解密需删库重建。

## Hint（坑·模式·本地约定）

- **排序查询必须确定性**：`ORDER BY` 主键之外必带稳定兜底（`, rowid DESC`）。同毫秒并列在并发负载下退化为 flaky 假绿（V4 两个独立阻塞同源）。
- **tester 抗 flaky 多跑**：并发/排序/时间戳/共享资源类，单跑一次的绿不可信，全量套件连跑 ≥3 次（理想 5），每次全绿才过。
- **tester 变异还原禁用 git**：在带未提交改动的文件上做变异，还原只能"改前 `cp` 备份 → 改后从备份复原"，**禁 `git checkout`/`git restore`**（会冲掉未提交修复）；开工/结束 `git status` 快照逐行比对自证干净。
- **coder 交付序**：多交付项任务，测试一过立即按序落地 ① 完成接入/wiring ② 写 coding.md 骨架 ③ 才润色。接入与留痕都是交付物，不是收尾——那正是被截断丢失的位置。
- **coder 声明完成前必须实跑全量测试，cargo check 不够**（TV3 复盘 TV3-RETRO-1）：最后一次编辑之后必须实跑全量 `cargo test`（前端 `pnpm test`），**不能以 `cargo check`/单测子集/记忆代替**；自报的 `N passed` 必须来自本次实跑、贴真实 `test result: ok. N passed` 行。TV3-F2 coder polish 后只 cargo check 就自报 487 passed，实树 GeminiProvider URL `format!` 漏占位符编译失败（exit 101），被 tester 命中校验首步抓到打回。〔已全局落地 2026-06-06：`~/.claude/agents/coder.md` 提交前自检清单首条〕
- **回合预算有限**：单次任务切到预算内、尽早落痕（coding.md/test.md），撞顶也留可续状态；超大任务交付最小片段。
- **续跑撞顶无进展 ≥2 次改派 fresh agent**（TV4 复盘 TV4-RETRO-1）：同一 subagent 续跑（SendMessage resume）若连续 ≥2 次撞 maxTurns 且无实质进展（工具调用数≈0 或无文件变更），判定其累积上下文已饱和——resume 只续任务不缩上下文，越续越死。此时**改派全新 subagent**，带『已落地状态摘要 + 精确剩余步骤（可含批量 sed/perl 指令）』接力，而非继续 resume 老 agent。TV4-F1 枚举大重构实测：老 coder 反复 0 进展，改派 fresh coder 后两轮收口完成。
- **移除/重命名 provider/源/实体时全仓清旧引用**（TV1 复盘 TV1-RETRO-1）：删或改名一个翻译源/实体后，全仓 `grep` 其旧名（id/display/字段），**同批清理注释、测试 fixture、doc 中的过时引用**，纳入交付清单逐条核销——否则会留下与实现矛盾的死注释/旧值断言（TV1 移除 MyMemory 后 ipc_settings.rs 头注释、translate.rs fixture、mod.rs:65 doc 三处复发，被 reviewer 逐个抓）。翻译源持续新增/演进期此坑高频。
- **证否「密钥不泄露」必须用非空 sentinel 脏值**（TV2 复盘 TV2-RETRO-1）：断言密钥/token/apikey 不出现在请求/输出/日志/错误消息时，凭据字段要填入**可识别的非空脏值**（如 `"SENTINEL_DEADBEEF"`）再断言 `!contains(sentinel)`；用空值/空串占位是**恒真假绿**（空值本就不会出现在任何输出里）。F2 缺字段测试曾用空凭据被 reviewer 抓。TV3 LLM apiKey、TV4 词典 key 同理。〔已全局落地 2026-06-06：`code-standards` SKILL.md §8 测试〕
- **复杂签名确定性测试须独立复算锚定**（TV2 [仅观察]）：TC3/HMAC-SHA1/SigV4 等签名源，别只「本实现算一次断言等于自己」（循环论证，写错也绿）——用独立工具（Python 按厂商官方文档手算）算出参照向量，断言锚定**具体 hex/Base64 常量**。TV2 三源已照此做并经 codex 异构裁判复核。

### v6 自动更新本地约定

- **前端事件监听**：用 `import { listen } from "@tauri-apps/api/event"`，惯例见 `src/App.tsx`、`TranslateSourcePanel.tsx:204`。事件名常量集中定义（如 `TRANSLATE_HISTORY_CHANGED_EVENT`），update 事件用 `update://ready`。
- **自定义命令注册**：Rust 命令写在 `src-tauri/src/ipc/<域>.rs`，在 `lib.rs` 的 `invoke_handler![]` 注册（updater 域已有 `ipc::update::check_for_updates @ lib.rs:163`）；前端在 `src/ipc/ipc-client.ts` 加 `invoke` 封装。
- **重启权限**：`app.restart()` 是 Rust 端核心 API，自定义 `restart_app` 命令前端可直接 invoke，`capabilities/default.json` 已含 `core:default`，**预期无需引入 plugin-process**——S03 实测确认即可。
- **updater 现状**：`check_for_updates` 已在 `ipc/update.rs`（仅查不装）；endpoint 为**真实地址**（`update.rs:6-8` "占位 endpoint"注释已过时，需修正）；`auto_update` 开关 get/set 在 `ipc/settings.rs:587`、字段在 `settings.rs:67`、前端开关已接线但后端无消费者。
- **下载可测性**：`tauri-plugin-updater` 的 `Update` 不易在单测构造——把"是否应检查"抽成纯函数 `should_check(enabled, already_ready) -> bool`、把就绪事件 payload 构造抽成纯函数单测；真实下载/重启隔离到薄封装层，归真机 manual_confirm。
