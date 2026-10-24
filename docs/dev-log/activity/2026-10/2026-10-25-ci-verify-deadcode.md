---
status: done
category: implementation
subject: Maccy 对标 · Phase 6 工程加固（P6-1 PR/push 验证 workflow + P6-3 死代码检测）
summary: 新增独立于发布流水线的 verify workflow（PR/push 跑 make verify，只上 macOS 腿）+ knip 与后端人工清单两类死代码检测；门禁只拦「未接线文件/未声明依赖」，11 项前端告警与 18 项后端零生产引用候选全部只报告不删除。干净树上实跑才暴露真坑：tauri::generate_context! 要求 dist/ 存在，否则干净克隆的 clippy/cargo test 直接 panic——已在 Makefile 加 dist 前置目标修掉。
evidence:
  commits:
    - "42f2cf3 ci: 新增 PR/push 验证 workflow 与死代码检测（P6-1/P6-3）"
  commands:
    - "make verify"
    - "make deadcode"
    - "make deadcode-gate"
    - "node scripts/rust-dead-code-inventory.mjs --all"
    - "actionlint .github/workflows/verify.yml .github/workflows/release.yml"
  urls: []
---

# AI 活动记录 · Phase 6 工程加固（CI 验证 + 死代码检测）

## 摘要

- **P6-1**：新增 `.github/workflows/verify.yml`，`push` 到 `main` / `pull_request` / 手动触发，job `verify` 就是一条 `make verify`。**`release.yml` 零改动**。
- **P6-3**：前端 `knip`（`knip.json` 钉住三个 HTML 入口）+ 后端 `scripts/rust-dead-code-inventory.mjs`（人工清单口径）。门禁只拦「写完没接线的文件」「import 了没声明的依赖」，其余全量报告不拦截、不删码。
- 现状：前端 11 项告警、后端 190 项候选（真孤儿 **1** 项：`map_deepl_http_status`）。全部留档交裁决，本次删除 0 行业务代码。

## 一个必须先纠正的事实

`Makefile` 里 `verify` 上方原写着「与 CI release 工作流复用同一条标准」。**这句是错的**：`release.yml` 只在 `v*` tag 上做版本守卫 → 生成 ECDICT → `tauri-action` 打包发布，一次都没跑过 `make verify`。所以「CI 会替我验证」这个心理保障此前并不存在——这正是 P6-1 要补的洞。

顺手把这句注释改成指向新的 `verify.yml`，并在 workflow 头注释里写明两者分工（发布 vs 验证），避免下次改其中一个时误以为另一个在联动。

## 干净克隆上 clippy 与 cargo test 编译不过（本次最大的发现）

第一版 workflow 写好后，我没有只在脏工作树上跑 `make verify`，而是把 `git archive HEAD` 解到临时目录、只覆盖本 item 的改动，跑了一遍「接近 CI」的验证。结果第 3 步就炸：

```
error: proc macro panicked
   --> src/lib.rs:242:14
    |
242 |         .run(tauri::generate_context!())
    = help: message: The `frontendDist` configuration is set to `"../dist"` but this path doesn't exist
```

`tauri::generate_context!` 是**编译期宏**，它要求 `tauri.conf.json` 的 `frontendDist`（`"../dist"`）目录存在，否则直接 panic，clippy 与 cargo test 一起退出 101。而 `dist/` 是 `vite build` 的产物、已 gitignore，干净克隆里根本没有。

**本机为什么一直没暴露**：任何一个跑过 `pnpm build` 或 `release.sh` 的工作树都留着 `dist/`，于是本地 `make verify` 一直是绿的。这正是「本地绿 → CI 红」最典型的来源，而且是**在任何人的改动之前就存在**的坑，不是本 item 引入的。

修法放在 Makefile（不是业务代码）：新增 `dist` 前置目标（`pnpm exec vite build`，**仅在 `dist/` 不存在时触发**），挂到 `check` / `test` / `test-rs` / `verify` 上。对本地开发无感（dist 存在就跳过），对干净树则是「原来编译都过不去」变成「自动构建前端产物后继续」。

顺带确认了另一件常被想当然的事：**ECDICT 词库不是 verify 的前提**。干净树上 `src-tauri/resources/` 是空目录，`cargo test` 照样编过并全绿——`bundle.resources` 的 glob 取不到文件只是得到空映射。所以没有复制 `release.yml` 里那段拉 CSV 生成词库的步骤。

## 平台矩阵：为什么只有 macOS 一条腿

路线图原话是「macOS 与 Windows 双端」，但这次**只上 macOS**，理由是事实而不是省事：

- 后端的 Windows 分支**从未编译验证过**。P1-1 / P1-2 / P1-3 / P1-5 / P1-7 五项都各自登记了「Windows 全路径未编译验证（本机无 windows target）」。
- 把 `windows-latest` 挂进矩阵，第一次跑就会红，而修它必须动 `src-tauri/src/**`——本 item 明令禁止改业务代码，等于提交一条永远红的 check。
- 本机也只有 `aarch64-apple-darwin` 一个 target，连「先本地预演 Windows 会红在哪」都做不到。

所以 Windows 的开启条件写进了 work items：等 Windows 分支能编过、`cargo test` 能过之后再扩矩阵；届时还要注意 GitHub 的 windows 镜像**不自带 `make`**，得把 `make verify` 换成等价的直跑命令。

## 死代码门禁为什么只拦两类

`knip` 一把梭的默认结果是 11 项告警，其中**没有一项是死文件**：

- 2 个未使用依赖（`@tauri-apps/plugin-autostart` / `plugin-updater`）：前端 0 import，但它们是 Tauri 插件的 npm 包，删之前得确认不打算用插件 JS API。
- 9 项「多余 `export`」：全部只在本文件内使用（逐个 grep 核实过）。

如果直接让 `knip` 拦 PR，这 11 项会永久常红，最后的走向一定是有人加 `ignore` 把它们藏起来——那比不接还糟。所以拆成两层：

- **阻塞**（`make deadcode-gate` = `knip --include files,unlisted`）：只留「写完没接线的文件」「import 了没声明的依赖」。这两类修复方向唯一，且当前是 0，能真拦住事故。
- **报告**（`make deadcode`，CI 里 `continue-on-error`）：未使用依赖、多余导出、后端候选清单，给人看。

`ignoreExportsUsedInFile` 这个开关**故意不开**：开了等于把那 9 项永久藏起来，而不开就能每次看到它们。

入口钉死也是实测出来的必要性：把 `src/trans-popover/index.html` 挪走，`src/trans-popover/main.tsx` 立刻被判成未使用文件——第二个 webview 的接线只靠 HTML 入口这一根线。

## 后端为什么不用 cargo-udeps

`cargo-udeps` 要 nightly，而且它查的是「Cargo.toml 里没被使用的 crate」。本项目最典型的事故形态是**模块写好了、`pub fn` 也在、生产路径零调用点**（`portable.rs`、`tiered_cleanup`、翻译 `cache/retry/cancel` 全是这个形态，且都是靠人肉审计才发现的）。`rustc` 的 `dead_code` 对这类同样失明：`pub` 条目在 lib crate 里天然对外可见。

于是写了 `scripts/rust-dead-code-inventory.mjs`，三个关键设计：

1. **先剔除注释与字符串字面量再计数**。否则「注释里提到过某个名字」会把孤儿伪装成已接线。剔除器处理了 `//`、可嵌套的 `/* */`、`r#"…"#` 原始字符串。
2. **排除 `#[tauri::command]`**。这些函数由前端按名字字符串经 IPC 调用，代码里没有任何标识符引用，不排除就会全员误报成孤儿。判断范围只取「上一个条目结束（`}`/`;`）之后到本声明之间」的属性尾部，避免上一个函数的属性误伤当前项。
3. **分类三个引用面**：其它生产文件（真正算接线）/ 同文件内 / 集成测试。只有第一类是「活着」。

### 用两条已登记的 ground truth 校准脚本

自证不算证据，所以拿 work items 里已有的两条独立结论对账：

| 已知登记 | 脚本输出 |
|---|---|
| P4-7：`db::open_or_recover` 生产 0 调用、4 处测试调用 | 仅测试引用 / 4 ✅ |
| P4-1 子项：`db::top_id` 生产无调用点、仅 `tests/clipboard.rs` 用 | 仅测试引用 / 4 ✅ |

两条都对得上，才敢把这个启发式当基线用。

## 本次扫出的真东西

- **唯一真孤儿**：`translate/providers.rs` 的 `map_deepl_http_status`。它的文档注释写着「供框架层（s03）在拿到 HTTP 状态码后调用」，实际全仓无人调用——DeepL 的 456 配额错误仍然只能从响应体文案去猜。这是典型的「写完没接线」，而且注释还在替它作证。
- **测试养死的路径**：`ipc/settings.rs` 的 `set_hotkey_impl` 被 7 条测试调用，但 `set_hotkey` 命令实际走 `set_hotkey_runtime_impl`。测试全绿，测的却是一条生产不走的路径。
- **注释与事实漂移**：`clipboard.rs` 模块头写「实际运行时由后台线程循环调用 `poll_once`」，而生产只调用 `poll_once_with_policy`，`poll_once` 只剩测试引用。
- `db.rs` 里 6 个恢复策略函数整体落在「仅测试引用」——和 P4-7 登记的「`open_or_recover` 与 `reset_and_reopen` 判据分裂」是同一件事，应该随启动恢复路径那次改动一起处理。

## 处置：一行都没删

路线的原话是「逐项接线，不搞一刀切」，而这一批条目的共同特征是「语义仍然要保留，只是缺接线」，直接删会顺手删掉唯一的测试覆盖（比如 `poll_once` 那条「无策略版必须保持同样基线推进语义」的契约测试）。

所以本次只做「发现 + 归因 + 留档」，逐项的处置建议和需要裁决的点都写进了 work items 的 P6-3：`map_deepl_http_status` 该接线还是删除、`set_hotkey_impl` 与运行时版二选一、`db::*` 六项随哪次改动收口。

## 验证

```
# ① 干净树（git archive HEAD + 只覆盖本 item 改动；无 dist/、无 ecdict.db）
pnpm install --frozen-lockfile        → OK
make verify                           → VERIFY_EXIT=0
  dist（前置目标自动触发）            → vite v5.4.21 built in 398ms
  [1/5] tsc -b                        → 无错误
  [2/5] cargo fmt --all --check       → 干净
  [3/5] cargo clippy --all-targets -D warnings → 干净
  [4/5] pnpm test                     → 74 files / 751 tests passed
  [5/5] cargo test                    → 718 passed / 0 failed

# ② 工作树（当时与别人未提交的改动并存）
make verify                           → 全过
  [4/5] pnpm test                     → 74 files / 753 tests passed
  [5/5] cargo test                    → 739 passed / 0 failed

# ③ 死代码
make deadcode-gate                    → 通过（files/unlisted 0 告警）
make deadcode                         → 前端 11 项 + 后端 190 项候选（报告模式，退出 0）
node scripts/rust-dead-code-inventory.mjs → 零引用 1 / 仅测试引用 17 / 仅本文件引用 172

# ④ workflow 静态校验
actionlint 1.7.12（+ shellcheck 0.11.0）
  .github/workflows/verify.yml        → 0 报错
  .github/workflows/release.yml       → 0 报错
```

> 干净树与工作树的测试数不同（751/718 vs 753/739），是因为工作树里还有别人未提交的新代码与新测试。**结论要引用干净树那组**——那是 HEAD + 本 item 改动，也是 CI 会看到的形态。

## 未闭环

- **两个 job 都没有在真实 GitHub Actions 上跑过**。actionlint 只能保证语法/事件/runner 标签/表达式/shell 层面成立，镜像差异（rust-cache 键、pnpm 11 的 store 行为、镜像里的 make 版本）只能首次真跑才知道。第一条 PR 请当「验证 workflow 本身」的复盘对象。
- **Windows 腿缺席**，理由与开启条件见上。
- **knip 不覆盖 CSS**（`project` 只含 ts/tsx），未引用的 `.css` 不会被报。
- **后端清单是词法级启发式**：宏生成的调用点、`Box<dyn Trait>` 动态派发、trait 默认方法体内的调用可能漏计；同名条目跨文件重复声明时计数会互相加成。它的定位是「给出候选 + 让人去看语义」，不能当删除依据直接用。
- **`make verify` 在脏工作树上会红**：本卡收尾时别人正在改 `db.rs` / `lib.rs`（P4-4 自动淘汰），`cargo fmt --check` 因他们未格式化的改动而失败——与本 item 无关，但说明「拿脏工作树的一次绿当结论」不可靠，所以上面引用的结论都来自干净树。
- 顺带发现 `pnpm 11` 在本仓（有 `pnpm-workspace.yaml`）会把 store 建在项目内的 `.pnpm-store/`，已加进 `.gitignore`，避免脏工作树。
