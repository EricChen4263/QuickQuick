---
status: done
category: implementation
subject: Maccy 对标 · Phase 6 工程加固（P6-2 覆盖率基线）
summary: 为前端与后端建立覆盖率基线，不设任何阈值。前端用 vitest 的 v8 provider（@vitest/coverage-v8 钉在与 vitest 同版本的 2.1.9），后端用 cargo-llvm-cov 编译期插桩；Makefile 新增 coverage / coverage-js / coverage-rs 三个目标，CI 侧新增独立且仅 workflow_dispatch 触发的 coverage.yml（报告进 artifact）。基线取在 trunk d057ae2：前端 91.32% lines / 90.72% branches / 87.83% functions，后端 85.44% lines / 84.98% regions / 79.40% functions。
evidence:
  commits:
    - "04a9d0e ci: 建立前后端覆盖率基线（P6-2）"
  baseline_commit: d057ae2
  commands:
    - "make coverage-js"
    - "make coverage-rs"
    - "pnpm test:coverage"
    - "cargo llvm-cov --html --output-dir target/coverage"
    - "cargo llvm-cov report"
    - "cargo llvm-cov report --lcov --output-path target/coverage/lcov.info"
    - "actionlint .github/workflows/*.yml"
    - "pnpm exec knip --include files,unlisted"
  urls: []
---

# AI 活动记录 · P6-2 覆盖率基线

## 摘要

- **前端**：`@vitest/coverage-v8` **2.1.9**（= lockfile 里 vitest 的解析版本，peer 绑定 `@vitest/coverage-v8@2.1.9(vitest@2.1.9)`），新增 `pnpm test:coverage`；覆盖率配置写进 `vite.config.ts` 的 `test.coverage`，只统计 `src/**/*.{ts,tsx}` 源码，排除测试文件、`test-setup.ts`、`*.d.ts`。
- **后端**：`cargo-llvm-cov`（编译期 `-C instrument-coverage`），输出 HTML + lcov + 控制台总表。
- **Makefile**：新增 `coverage`（= `coverage-js coverage-rs`）、`coverage-js`、`coverage-rs` 三个目标；**`verify` / `dist` / `check` / `test` / `test-rs` 的既有接线一个字没动**。
- **CI**：新增 `.github/workflows/coverage.yml`，仅 `workflow_dispatch`，报告上传 artifact。**`verify.yml` 零改动**。
- **不设阈值**：全线没有任何 `thresholds` / `--fail-under-*`。路线图原话「先建立基线再谈阈值」，本 item 只负责把数字变可见。
- 基线数字见下「实跑基线」，取数于**干净 worktree**（`git worktree add /tmp/qq-head <commit>`），只覆盖本 item 改的配置文件，源码与 `Cargo.lock` 一律是被测 commit 的原样，避免把别人未提交的改动算进基线。

## 为什么版本要钉死

`@vitest/coverage-v8` 与 `vitest` 是 peer 强绑定关系：主次版本不一致时 vitest 会报 `Cannot find module`，或插桩结果与真实不符。本仓 `package.json` 的 vitest 声明是 `^2.0.5`（范围），lockfile 实际解析到 **2.1.9**，所以 coverage-v8 直接写**精确版本 `2.1.9`**（pnpm 写入的形态就是精确版本），并在 Makefile 注释里写明「升级要成对改」。用范围（`^2.1.9`）反而会让两者在未来的某次 `pnpm update` 里漂开。

## reportOnFailure 为什么打开

vitest 的 `coverage.reportOnFailure` 默认 **false**：测试一挂，报告一份都不留。本次第一版配置就是这个默认值，结果在工作树上跑到一条失败用例时，`coverage/` 目录**根本没有生成**——覆盖率是诊断信号而不是门禁，失败时恰恰更需要知道「哪块没被测到」，所以显式打开。

这个默认值的坑值得记一笔：它不是报错，是静默什么都不产出，容易被误判成「provider 没装上」。

## CI 为什么是独立 workflow + 手动触发

后端覆盖率要在**编译期插桩**（`RUSTFLAGS=-C instrument-coverage`），依赖要全量重编一遍（本次干净树上 `target/llvm-cov-target` 涨到 ~715MB），比 `make verify` 慢一个量级。而覆盖率此刻**不具备拦截判据**（没有阈值、基线也刚建立），挂到每个 PR 上只会白烧 CI 分钟。

所以分成两条独立链路：

| 工作流 | 触发 | 职责 |
|---|---|---|
| `verify.yml` | push main / pull_request / workflow_dispatch | 门禁：`make verify` + 死代码 |
| `coverage.yml` | **仅 workflow_dispatch** | 出报告：`make coverage-js` + `make coverage-rs`，报告进 artifact |

两者互不引用：改门禁不动报告，改报告不动门禁。CI 里复用 Makefile 的同一批目标，避免「CI 命令」与「本地命令」各写一套再漂开。

`cargo-llvm-cov` 用 `taiki-e/install-action@v2` 装 prebuilt 二进制（省掉现场编译那 1 分多钟），`llvm-tools-preview` 组件由 `dtolnay/rust-toolchain` 的 `components:` 装上——缺它 cargo-llvm-cov 会直接报错退出。

## 本机为什么得绕一圈（如实记录）

本机 `rustup component add llvm-tools-preview` 与 `cargo install cargo-llvm-cov` **首次都失败**，原因不是网络，而是本会话沙箱只允许写工作区：

1. `~/.rustup/downloads` 不可写 → rustup 下完 llvm-tools 后回滚，报 `cleaning up cached downloads: No such file or directory`。
2. `~/.cargo` 不可写（`cargo install` 要往 `~/.cargo/bin` 落二进制）。
3. `static.crates.io` 的 TLS 握手在本机被拦（`SSL_ERROR_SYSCALL`），但 `index.crates.io` 与国内镜像可达；本机 `~/.cargo/config.toml` 本来就把源换成了 `rsproxy`。

解法（**全部在 `/tmp`，仓库里没有硬编码任何路径**）：

```bash
# ① 工具链搬到可写目录，再补组件（rustup 只认 RUSTUP_HOME 下的 downloads）
cp -R ~/.rustup/toolchains/stable-aarch64-apple-darwin /tmp/qq-rustup/toolchains/
#    + 写 /tmp/qq-rustup/settings.toml（default_toolchain）
RUSTUP_HOME=/tmp/qq-rustup rustup component add llvm-tools-preview

# ② cargo-llvm-cov 用镜像源装到 /tmp（CARGO_HOME 重定向 + 复制本机镜像配置）
cp ~/.cargo/config.toml /tmp/qq-cargo-home/config.toml
CARGO_HOME=/tmp/qq-cargo-home cargo install cargo-llvm-cov --root /tmp/qq-tools --locked
```

顺带一个反直觉点：**系统自带的 `/Library/Developer/CommandLineTools/usr/bin/llvm-profdata` 用不了**。Apple LLVM 21.0.0 只认 raw profile **v10**，而 rustc 1.99.1 产出 **v11**，合并直接失败：

```
raw profile version mismatch: Profile uses raw profile format version = 11; expected version = 10
```

必须用 rustup 的 `llvm-tools-preview`（其 `llvm-profdata` 来自 `LLVM 23.1.1-rust-1.99.1-stable`，与 rustc 配套）。也就是说「macOS 自带 llvm-cov 就能做 Rust 覆盖率」这个直觉是错的。

## 实跑基线

取数 commit：**d057ae2**（`feat(tray): 托盘菜单补齐…（P5-1）`，即本 item 提交时的 trunk HEAD）。取数方式：`git worktree add /tmp/qq-head d057ae2` + 只覆盖本 item 改的 `vite.config.ts` / `Makefile`。

### 前端（vitest v8，74 个测试文件 / 768 条测试全绿）

统计口径 `src/**/*.{ts,tsx}`（排除测试文件、`test-setup.ts`、`*.d.ts`）。

| 指标 | 覆盖 |
|---|---|
| Statements | **91.32%**（4908/5374） |
| Branches | **90.72%**（1301/1434） |
| Functions | **87.83%**（325/370） |
| Lines | **91.32%**（4908/5374） |

主要目录（Stmts / Branch / Funcs / Lines）：

| 目录 | Stmts | Branch | Funcs | Lines |
|---|---|---|---|---|
| `src/translate` | 100 | 100 | 100 | 100 |
| `src/main-window`（含 settings） | 100 | 100 | 100 | 100 |
| `src/panels/history` | 100 | 98.65 | 100 | 100 |
| `src/shell` | 100 | 100 | 72.73 | 100 |
| `src/components` | 98.47 | 91.35 | 100 | 98.47 |
| `src/panels/clipboard` | 98.11 | 93.28 | 96.30 | 98.11 |
| `src/panels/settings` | 94.61 | 85.12 | 97.87 | 94.61 |
| `src/clip-popover` | 94.41 | 96.68 | 94.34 | 94.41 |
| `src/panels/translate` | 93.09 | 84.91 | 76.47 | 93.09 |
| `src/theme` | 91.67 | 85.71 | 92.86 | 91.67 |
| `src/trans-popover` | 81.19 | 77.36 | 90.00 | 81.19 |
| `src`（根：App.tsx / main.tsx） | 75.29 | 80.95 | 50.00 | 75.29 |
| `src/ipc` | **48.48** | 85.96 | 50.00 | **48.48** |

洼地成因很集中，不是「测试少」而是「形态决定的」：

- `src/ipc` 48.48%：`ipc-client.ts` 46.43%（176/379 行）。它是**全量 Tauri 命令的薄封装**，几乎一个命令一个函数，测试只覆盖 UI 流程真正走到的那几个；剩下的是「有函数、有类型、无调用」的静态面。
- `src`（根）75.29%：`App.tsx` 83.11%、`main.tsx` 0%（0/8 行）。三处 `main.tsx`（根 / clip-popover / trans-popover）全是 0%，它们只做 `createRoot(...).render(...)`，测试从不 import webview 入口。
- `browser-api.ts` 18.18%（2/11 行）、函数覆盖 0%：浏览器翻译 API 兜底路径，jsdom 里无法真实触发。

### 后端（cargo-llvm-cov，35 个测试二进制 / 771 条测试全绿）

统计口径为 `src-tauri/src/**`；插桩运行，不含 doctest（cargo-llvm-cov 默认不跑 doctest）。

| 指标 | 覆盖 |
|---|---|
| Lines | **85.44%**（12958/15166，missed 2208） |
| Regions | **84.98%**（23865，missed 3584） |
| Functions | **79.40%**（1434/1806，missed 372） |
| Branches | 0/0 —— 本配置下 llvm-cov 不产出 Rust 分支覆盖 |

主要模块（行覆盖 / 函数覆盖）：

| 模块 | Lines | Functions |
|---|---|---|
| `settings.rs` | 100.00 | 100.00 |
| `privacy.rs` | 99.84 | 100.00 |
| `translate/lang.rs` | 98.87 | 100.00 |
| `translate/credential.rs` | 98.78 | 96.23 |
| `portable.rs` | 98.48 | 100.00 |
| `hotkey.rs` | 98.28 | 100.00 |
| `db.rs` | 96.65 | 97.44 |
| `image.rs` | 95.46 | 96.49 |
| `translate/providers.rs` | 93.51 | 88.04 |
| `translate/error.rs` | 93.39 | 93.33 |
| `ipc/translate.rs` | 90.50 | 83.33 |
| `ipc/portable.rs` | 88.70 | 72.00 |
| `ipc/clipboard.rs` | 87.14 | 65.91 |
| `pipeline.rs` | 84.68 | 60.71 |
| `pasteboard_meta.rs` | 82.48 | 80.70 |
| `ipc/settings.rs` | 73.82 | 61.54 |
| `ipc/system.rs` | 66.87 | 56.79 |
| `autostart.rs` | 61.54 | 80.00 |
| `tray.rs` | 58.19 | 64.86 |
| `window_pos.rs` | 57.32 | 60.00 |
| `macos_paste.rs` | 48.91 | 55.56 |
| `ipc/update.rs` | 23.86 | 28.57 |
| `lib.rs` | 19.84 | 28.07 |
| `popover.rs` | 14.29 | 23.53 |

洼地的共性是**必须有真实窗口 / 系统事件才能跑到的路径**：

- `popover.rs` / `lib.rs` / `macos_paste.rs` / `window_pos.rs`：窗口与面板装配、`Builder::setup` 闭包、真 NSWindow 与多显示器偏移——跑起来就要真 `AppHandle` 或真显示器，单测只能到参数校验层（`window_pos.rs` 的纯函数部分已被覆盖，57.32%）。
- `ipc/update.rs`：更新器链路（网络 + 安装），测试里全走桩。
- `tray.rs` 58.19%：P5-1 的托盘菜单测试把它从早先的 19.74% 拉到这个水位——顺带说明这套基线对「补测试」是**敏感**的，不是摆设。
- 反过来 `db.rs` 96.65%、`privacy.rs` 99.84%、`translate/*` 93–99%：纯逻辑 + 可注入依赖，覆盖率高得实在。

### 口径提醒

- 前端 branches 是 v8 的粗粒度分支统计，Rust 侧是 region/line，两者**不可跨端比较**，别拿 90.72% 与 84.98% 相减说「前端比后端好」。
- 后端行数 15166 是 `llvm-cov report` 的 TOTAL 原值；不含 `src-tauri/tests/**`（集成测试自身不作为被测对象计入）。

## 验证

```bash
# ① 前端（工作树，含别人已提交 + 未提交改动；768 条测试全绿）
make coverage-js            → 74 files / 768 tests passed
                              All files | 91.32 | 90.72 | 87.83 | 91.32
                              coverage/index.html + coverage/coverage-summary.json 生成

# ② 后端（干净 d057ae2 worktree；35 个测试二进制 / 771 条测试全绿）
make coverage-rs            → 全绿
                              TOTAL | regions 84.98% | functions 79.40% | lines 85.44%
                              src-tauri/target/coverage/html/index.html
                              src-tauri/target/coverage/lcov.info（约 2 万行 lcov）

# ③ 门禁未被破坏
pnpm exec knip --include files,unlisted  → 0 告警（新 devDep 未被判成 unlisted）

# ④ workflow 静态校验（actionlint 1.7.7 darwin/arm64）
actionlint .github/workflows/*.yml       → 0 报错（含新增 coverage.yml）
```

## 未验证项

- **`coverage.yml` 没有在真实 GitHub Actions 上跑过**。actionlint 只能保证语法 / 事件 / action 输入 / 表达式成立；`taiki-e/install-action` 拉 prebuilt 的成功率、`Swatinem/rust-cache` 面对 `target/llvm-cov-target`（~715MB）的缓存行为，都要首次真跑才知道。建议第一次手动触发时当作「验证 workflow 本身」的复盘对象。
- **Rust 侧没有分支覆盖**：`llvm-cov report` 的 Branches 列是 `0/0`（本配置不产出），所以「后端分支覆盖率」这个指标目前不存在，不能拿前端的分支数去类比。
- **一次未定位的 flake**：脏工作树首次全量跑出现 1 例失败（`clipboard-page.test.tsx` 的「富文本内容示例」断言）；单文件复跑、随后两次全量、以及干净 worktree 全量均绿，既未在干净树上复现也未定位根因（疑似测试并行下的时序或全局态残留），留作观察项。
- **基线是快照不是不变量**：本卡取 d057ae2；trunk 只要继续合并代码，这些数字就会变（尤其 `tray.rs` 这类正在被大量补测试的模块）。要复核请按上面的取数方式重新跑，不要引用旧数字做比较。
- **`make coverage-rs` 的前置是 `dist`**：与 `test-rs` 同理（`tauri::generate_context!` 要求 `frontendDist` 存在）。`dist` 目标的陈旧产物问题由 lead 另派修复，本 item 未动它。
- **本机工具链是 /tmp 里的临时副本**：`RUSTUP_HOME=/tmp/qq-rustup` + `CARGO_HOME=/tmp/qq-cargo-home` + `/tmp/qq-tools/bin`。换机器或清 /tmp 后需按上文重装；仓库侧的唯一硬性前提仍是「`llvm-tools-preview` 组件 + `cargo-llvm-cov`」，已在 Makefile 的缺失提示里写明。

## 未做（明确边界）

- **没有设任何阈值**，也没接 Codecov / 第三方覆盖率服务（基线还不到谈门禁的阶段）。
- **没有把覆盖率挂进 `make verify`**：插桩编译太慢，且此时不具备拦截判据。
- **没有改动任何业务代码**（`src/**` 与 `src-tauri/src/**` 零改动）。
