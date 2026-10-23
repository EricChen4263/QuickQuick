---
status: done
category: implementation
subject: Phase 0 · clip-popover 改造为非激活 NSPanel
summary: 把 macOS 的 clip-popover 从普通 NSWindow 换成 NSPanel + nonactivatingPanel，弹出不再激活 app；粘贴编排按「面板是否持 key」分叉，主窗口与未授权降级路径保持原样。用通知中心观察 resign key 而非替换 NSWindow delegate，避免掐断前端依赖的 focus 事件。
evidence:
  commits: []
  commands:
    - "cd src-tauri && cargo check --all-targets"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo build"
    - "pnpm exec tsc -b && pnpm test"
    - "git worktree add /tmp/qq-head HEAD --detach && cargo fmt --all --check（验证 fmt 差异在 HEAD 已存在）"
  urls:
    - "https://crates.io/crates/tauri-nspanel"
    - "https://github.com/ahkohd/tauri-nspanel"
    - "https://github.com/ayangweb/EcoPaste"
---

# AI 活动记录 · clip-popover 非激活面板（Phase 0）

## 摘要

目标是把 `clip-popover` 改成 Maccy 那种「弹出不夺焦」的面板。改造前它是普通窗口：弹出必须 `NSApplication.activate()`，于是粘贴前要把整个 app 藏起来、再等 350ms 把焦点还给原应用——失败即「粘贴静默无效果」。

落地方案（详见 `docs/design/clip-popover-nspanel.md`）：

1. **引入 `tauri-nspanel` 2.1.1（crates.io）**，`object_setClass` 把已建好的窗口就地换成动态 panel 子类，Tauri 的建窗与 webview 链完全不动。插件在 builder 阶段注册（`to_panel` 内部取插件状态，未注册会 panic）。
2. **粘贴编排分叉**：新增 `paste_focus_plan(面板是否持 key)` 纯决策——面板持 key 时只让面板 `resign_key_window()` + `order out`；其余情况（主窗口粘贴、面板降级）沿用隐藏 app + 显式激活目标 app + 350ms。`hide_and_return_focus`（Esc 还焦）同样加面板分支。
3. **一处与参考实现的关键分歧**：EcoPaste 用 crate 的 `set_event_handler` 监听 `window_did_resign_key`，但那内部会 `setDelegate:` 替换掉 tao 的窗口代理，导致 tao 不再发 `Focused` 事件——而 `ClipPopoverApp.tsx:97` 正靠 `onFocusChanged` 在每次唤起时聚焦搜索框、清空搜索词。照抄会让这个交互**静默失效**。改为观察 `NSWindowDidResignKeyNotification`（同一个信号源，不占用 delegate）。
4. **用同步派发替代魔法时延**：`run_on_main_thread` 是异步投递，参考实现靠「等 50ms」掩盖竞态。这里改成投递后阻塞等主线程回发完成信号（`mpsc::recv_timeout`），以「确定已完成」取代「猜时长」。未加 `make_key_window`——本项目的 popover 粘贴后即关闭，把 key 还给隐藏面板会让焦点落空。

`trans-popover` 未改造，保持普通窗口语义。

## 过程中踩到 / 确认的坑

- **前端 `onFocusChanged` 依赖**：见上第 3 点。这是「照抄参考实现就会引入静默退化」的典型，靠读 `ClipPopoverApp.tsx` 才发现。
- **crate API 与 git 版不一致**：2.1.1 的 `set_style_mask` 返回 `Result`，且提供推荐的 `add_style_mask`（只追加 nonactivatingPanel，保留 tao 已写的结构样式位）。按 2.1.1 写。
- **`WebviewPanelManager` 带泛型参数**：`try_state::<WebviewPanelManager<Wry>>()`，写裸名编不过。
- **`AppHandle::plugin()` 在 setup 里注册也可行**，但放在 builder 阶段更稳（早于 setup 与懒建窗口），且能避开 `MockRuntime` 测试路径。

## 后续修复 · 主线程超时不再误报 full_paste

独立审查指出一个非阻塞但真实的缺陷：`on_main_thread_blocking` 超时后只 `eprintln!` 就返回，粘贴两臂仍注入 ⌘V 并返回 `full_paste`——面板若没让出 key，⌘V 会被它自己吞掉，界面报成功而实际什么都没粘。

修法：让「焦点是否真的交出去」成为可决策信息，而不是日志里的叹息。

1. `on_main_thread_blocking` / `hide_clip_popover` / `hide_clip_panel_now` 全部返回 `bool`；派发失败与超时都 `eprintln` + `false`。
2. 新增纯函数 `should_inject_paste(plan, focus_yielded)`：面板路径返回 `focus_yielded`，主窗口路径恒 `true`（旧路径的还焦结果无法在命令线程回读，硬造失败信号只会让它无故退化成只写回）。
3. 粘贴两臂在 `false` 时 `return "write_back_only"`，`send_paste` 在代码路径上不可达。
4. 六个 `bool` 返回函数加 `#[must_use]`——本次修的正是「返回值被忽略」，让这类退化在编译期暴露。

审查复核：**非 BLOCK**，6 点核实全部成立（判据正确、主窗口路径逐语句一致、返回值在所有调用点被消费、单测断言具体值、超时后 `send_paste` 不可达、无死代码）。顺手修掉审查提的两条文档问题：ADR 里同步派发的示意代码原来用 `?` 向上抛错（与文件级「不 panic、不向上抛错」约定矛盾），改成与实现一致的降级形式；改动范围表补上 `Cargo.lock`。

## 环境侧（本机缺口，非代码问题）

- 本机**没有 Rust 工具链**（`cargo`/`rustc`/`~/.cargo` 全缺失），验收要求的 `cargo check/clippy/test` 无法直接跑。补装 `rustup stable 1.99.1`（`--no-modify-path`，未改 shell profile）。
- `index.crates.io` TLS 握手不稳（`SSL_ERROR_SYSCALL`），`cargo fetch` 失败。cargo 源临时指向镜像，配置写在 `~/.cargo/config.toml`（**不进仓库**）。
- `cargo test`/`check` 需要 `dist/`（`generate_context!` 会检查 `frontendDist`），故先 `pnpm install && pnpm build`。

## 证据

- 方案与人工验证清单：`docs/design/clip-popover-nspanel.md`
- 改动：`src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`src-tauri/src/popover.rs`、`src-tauri/src/ipc/system.rs`
- 自动验证：check / clippy(-D warnings) / test（35 个二进制、0 failed）/ build / tsc / vitest（519 用例）全绿；`cargo fmt --all --check` 在 HEAD 上即不干净（同 5 个文件，rustfmt 版本错位），未顺手全局格式化
- **未验证**：面板是否真的不夺焦、中文输入法 composition、Enter 粘贴落点、Esc 后焦点、全屏 Space 可见性、辅助功能未授权降级——均为真机 GUI 项，步骤见方案 §6.2
