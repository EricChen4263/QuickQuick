# clip-popover 非激活面板（NSPanel）改造方案

> 日期：2026-10-24
> 范围：macOS 的 `clip-popover` 从普通 `NSWindow` 改为 `NSPanel` + `nonactivatingPanel`；`trans-popover` 本期不动。
> 定位：v0.4 交互重构的 Phase 0（接入适配 spike）。Phase 2 的交互改造以此为前提。
> 状态：代码完成；自动化验证结果见 §6.1；夺焦 / 输入法 / 粘贴落点 / 全屏 四项需人工验证（步骤见 §6.2），**未声称已验证**。

---

## 一、要解决的问题

改造前的唤起链条是：全局热键 → `trigger_popover` → `window.show()` → `NSApplication.activate()` → `window.set_focus()`。

普通窗口要拿到 key 状态（收键盘），前提是它所属进程是 macOS 的活跃应用。于是这条链有一个结构性代价：

1. **弹出面板 = 把用户的原应用踢下前台**。光标位置、文本选区、输入法候选状态全部丢失。
2. **粘贴时必须把前台还回去**：hide 窗口 → `app.hide()` → 按记录的 pid 显式激活目标 app → 固定等待 350ms 让窗口动画与焦点转移完成 → 才注入 `⌘V`。

第 2 步的每一环都能单独出错——目标 app 记录陈旧、等待不足、动画未完成、注入早于还焦——而且失败表现为「静默无效果」，排查成本极高。这不是实现 bug，是普通窗口语义决定的必然代价。

Maccy 的做法是 `NSPanel` + `.nonactivatingPanel`：面板能成为 key window 接收键盘，但不激活 app。前台应用自始至终没被换走，上面第 1 步的副作用和第 2 步的整段编排都不需要存在。

## 二、决策

### 2.1 只面板化 `clip-popover`

| 窗口 | 本期 | 理由 |
| --- | --- | --- |
| `clip-popover` | 面板化 | 唯一「由全局热键唤起、直接服务当前输入」的窗口。它的存在意义就是在用户正在编辑的地方插入内容，最不能受夺焦影响。 |
| `trans-popover` | 保持普通窗口 | 同样由热键唤起，但本期范围外。面板化的机械改动相同、风险相同，但收益路径不同（翻译面板的粘贴回填与剪贴板面板不是同一条链），同期改造会扩大风险面而不带来叠加收益。 |
| `main` | 保持普通窗口 | 主窗口是用户显式打开的应用界面，本来就该激活 app。 |

`trans-popover` 后续是否面板化，取决于一个判断：它是否也需要「不打断用户原输入」。若 Phase 2 把「就地取词 → 面板显示译文 → 回车替换原文」做起来，答案就是需要，届时照搬本文的机制即可（面板类型、通知观察、同步派发都是可复用的）。

### 2.2 技术路径：`tauri-nspanel` 2.1.1（crates.io）

选它而不是手写 objc2 的理由：

- 它用 `object_setClass` 把**已创建**的 `NSWindow` 就地换成动态生成的 panel 子类。Tauri 的窗口构建、webview 挂载、事件系统全部不感知，改造面最小。
- 手写等价物要自己实现 `define_class` + `MainThreadOnly` 约束 + `NSWindowDelegate` 协议对象 + `canBecomeKeyWindow` 覆写，收益为零。
- 版本兼容已核实：crate 声明 `tauri ^2.8.5`、`objc2 ^0.6.1`、`objc2-app-kit ^0.3.1`，与本项目 `tauri 2.11.2` / `objc2 0.6` / `objc2-app-kit 0.3` 一致。

参考实现（EcoPaste）用的是同一个 crate 的 git `branch = "v2.1"`；此处用 crates.io 的 2.1.1 发布版，避免 git 依赖（可复现性更好）。两者 API 有一处差异已适配：2.1.1 的 `set_style_mask` 返回 `Result<(), StyleMaskError>`，并提供推荐的 `add_style_mask`。

### 2.3 不替换 NSWindow delegate，改用通知中心观察 resign key ⭐

这是本方案与参考实现最重要的分歧，也是最容易踩的坑。

crate 的 `set_event_handler` 内部执行 `setDelegate:`——它会**替换掉 tao 的窗口代理**。后果是 tao 从此收不到任何窗口回调，包括：

- `windowDidBecomeKey` / `windowDidResignKey` → Tauri 的 `WindowEvent::Focused` 事件消失；
- 而前端 `src/clip-popover/ClipPopoverApp.tsx:97` 正是靠 `onFocusChanged` 在**每次获焦时**聚焦搜索框并清空上一次的搜索词。

也就是说，照着参考实现用 `set_event_handler`，面板能正常弹出、能打字，但**「每次唤起都是干净的搜索框」这个交互会静默失效**，而且不报错。这类退化在人工验收里极易漏过。

所以这里改成观察 `NSWindowDidResignKeyNotification`：这正是 `windowDidResignKey:` 委托方法的信号源，只是经由通知中心广播，**不占用 delegate**。tao 的事件链完好，前端交互一行不改。

代价是需要在 `dealloc` 前反注册观察者。本场景窗口只创建一次、与进程同生命周期，故 `mem::forget` 观察者 token（与本项目 `register_frontmost_observer` 的处理一致）。

### 2.4 粘贴编排：让面板交出 key，而不是隐藏 app

面板路径的完整时序：

```
resign_key_window()  →  hide()（order out）  →  注入 ⌘V
```

为什么必须让出 key：面板还处于 key 状态时，合成出来的 `CGEvent ⌘V` 会先被面板自己的 webview 消费掉，前台应用什么也收不到——参考实现踩过这个坑，注释写得很直接。

**为什么不用固定等待（参考实现用 50ms）**：`run_on_main_thread` 是**异步投递**，投递完立刻注入必然存在竞态。参考实现只能靠「等一拍」掩盖。这里改为**同步派发**——投递任务后阻塞等待主线程发回完成信号，用「确定已完成」替代「猜一个时延」：

```rust
let (tx, rx) = std::sync::mpsc::channel::<bool>();
// 派发失败与等待超时都降级为 false，不用 `?`：本模块约定可失败操作只记录、不 panic、
// 不向上抛错；而且「焦点没交出去」是粘贴路径需要拿来决策的信息，不能当异常丢掉。
if app.run_on_main_thread(move || { let done = f(); let _ = tx.send(done); }).is_err() {
    eprintln!("[QuickQuick] 派发主线程任务失败");
    return false;
}
match rx.recv_timeout(MAIN_THREAD_WAIT_TIMEOUT) {
    Ok(done) => done,
    Err(_) => {
        eprintln!("[QuickQuick] 等待主线程任务超时");
        false
    }
}
```

命令线程阻塞等主线程不会死锁：粘贴命令跑在 Tauri 的 worker 线程上（`AppHandle::hide()` 需要内部派发正说明这一点），主线程并没有在等这条命令。同时保留了「若已在主线程则就地执行」的自检，防止未来把命令改成主线程执行时自己等自己。

**超时与派发失败不算成功**：同步派发能等到结果，也就必然能等到"等不到"。`on_main_thread_blocking` 返回是否确认完成，`hide_clip_popover` 把结果带回给粘贴路径，粘贴路径据此决定要不要注入 `⌘V`：

| 焦点交出结果 | 面板路径 | 主窗口路径 |
| --- | --- | --- |
| 确认完成 | 注入 `⌘V` → `full_paste` | 注入 `⌘V` → `full_paste` |
| 超时 / 派发失败 / 取不到面板 | **不注入** → `write_back_only` | 不变（仍注入） |

面板路径为什么必须放弃注入：面板若仍持有 key，注入的 `⌘V` 会被它自己的 webview 吞掉，而注入方拿到的是"按键已发出"——最终表现成界面报成功、用户什么都没粘。宁可让用户手动按一次 `⌘V`，也不兑现一个假的成功。判据抽成纯函数 `should_inject_paste`，与 `paste_focus_plan` 一样可在 headless 环境单测。

主窗口路径不参与该判据：它靠 hide + 显式激活目标 app + 固定等待完成还焦，结果无法在命令线程回读（见 §2.5 与 `FOCUS_YIELD_WAIT_MS` 的注释），硬造一个失败信号只会让它无故退化成只写回。

**为什么没有 `make_key_window`**：参考实现把 key 拿回来，是因为它有「固定窗口」模式——粘贴后面板仍然可见，用户要继续用键盘。QuickQuick 的 popover 粘贴后即关闭（后端 order out + 前端 `getCurrentWindow().hide()`），此时把 key 还给一个已隐藏的面板，只会让焦点落空。等将来真做「固定窗口」再补这一步。

### 2.5 必须保留的旧路径

| 场景 | 路径 | 判据 |
| --- | --- | --- |
| 主窗口内粘贴 | hide 窗口 → `app.hide()` → 显式激活目标 app → 350ms → 注入 | 面板未持有 key |
| 辅助功能未授权 | `write_back_only`：只写回剪贴板，不注入、不动焦点 | `AXIsProcessTrusted() == false` |
| 面板转制失败 / 非 macOS | 普通窗口 + `WindowEvent::Focused(false)` 失焦隐藏（= 改造前行为） | `to_panel` 失败 |

第一条的判据不是「面板是否存在」而是「面板是否持有 key」：`clip-popover` 是懒建的，用户可能先唤起过面板、再打开主窗口粘贴，此时面板存在但不在焦点上。用「是否持有 key」判定，两种场景都能走对路径。

## 三、改动范围

| 文件 | 改动 |
| --- | --- |
| `src-tauri/Cargo.toml` | 新增 `tauri-nspanel = "2.1"`（`target.'cfg(target_os = "macos")'` 下） |
| `src-tauri/Cargo.lock` | 机械产物：随依赖新增 `tauri-nspanel` 及其传递依赖 `pastey` 的锁定记录 |
| `src-tauri/src/lib.rs` | builder 阶段 `.plugin(tauri_nspanel::init())`；拆开原链式调用 |
| `src-tauri/src/popover.rs` | 面板类型声明、`convert_clip_popover_to_panel`、`show_clip_panel`/`hide_clip_panel_now`、resign key 通知观察、`on_main_thread_blocking`、`paste_focus_plan`、`clip_popover_holds_key` |
| `src-tauri/src/ipc/system.rs` | `yield_focus_before_paste` 分叉点；`paste_text_with_backend` / `paste_image_with_backend` 改调它；`hide_and_return_focus` 增面板分支 |

**未改动**：前端全部（`src/**`）、`trans-popover`、翻译域、加密链路（`keyprovider.rs` / `portable.rs`）、DB schema。

## 四、关键实现细节

- **层级 `PanelLevel::Dock`（20）**：足以盖住普通应用窗口与浮动窗口，又不会压过系统菜单栏与系统弹窗。更高会变成「盖住 Spotlight / 输入法候选窗」的体验灾难。
- **样式掩码用 `add_style_mask` 而非 `set_style_mask`**：只追加 `nonactivatingPanel`，保留 tao 建窗时写入的结构标志。整体替换会丢掉既有位；`add_style_mask` 底层仍调 `set_style_mask`，同时同步 AppKit 侧「阻止应用被激活」的开关——这是不抢前台的关键。
- **跨 Space 行为成对切换**：隐藏态 `stationary + move_to_active_space + full_screen_auxiliary`，显示态换成 `can_join_all_spaces + full_screen_auxiliary`。前者保证每次显示落在用户此刻的 Space，后者保证显示期间跟随用户视野（含多屏切换）。`full_screen_auxiliary` 是能出现在全屏应用上方的必要条件。
- **key 状态用 `AtomicBool` 而非读 `NSWindow.keyWindow`**：粘贴命令在 worker 线程，跨线程读 AppKit 状态不安全。该状态只在唤起与 resign 两个已知时机变化，无锁原子量足够。乐观置位（面板理论上总能成为 key）不会造成错误：最坏只是多做一次「让出 key + 隐藏」，结果仍正确。
- **`panel_registered` 前置探测**：crate 的 `to_panel` / `get_webview_panel` 内部直接 `state::<WebviewPanelManager>()`，插件未注册时会 panic。所有面板入口先探测 `try_state`，把「崩溃」降级成「保持普通窗口」。

## 五、被否决的方案

| 方案 | 否决理由 |
| --- | --- |
| 用 `set_event_handler` 监听 resign key（参考实现做法） | 会替换 tao 的窗口代理，掐断前端依赖的 focus 事件（详见 §2.3） |
| 固定等待 50ms 后注入 `⌘V`（参考实现做法） | 用固定时延掩盖异步派发竞态；同步派发能给出确定性 |
| 粘贴后 `make_key_window` 把焦点还给面板 | 本项目的 popover 粘贴后即关闭，还给隐藏面板会让焦点落空 |
| 手写 objc2 panel 子类 | 与 crate 能力等价，维护成本更高 |
| 同期面板化 `trans-popover` | 扩大风险面且无叠加收益（详见 §2.1） |
| 在 `register_plugins()` 里注册插件 | 该函数是 `R: Runtime` 泛型且被 `MockRuntime` 测试复用，塞入只对真实窗口有意义的面板插件会污染测试路径 |

## 六、验证

### 6.1 已自动验证

环境说明：本机原本未安装 Rust 工具链，验证前补装 `rustup stable 1.99.1`（`~/.cargo/bin`，未改 shell profile）；`crates.io` 直连不稳定，cargo 源临时指向镜像（写在 `~/.cargo/config.toml`，不进仓库）。前端依赖 `pnpm install` 后由 `pnpm build` 产出 `dist/`（`cargo check` 的 `generate_context!` 需要它存在）。

| 项目 | 命令 | 实测结果 |
| --- | --- | --- |
| Rust 类型/借用检查 | `cd src-tauri && cargo check --all-targets` | `Finished dev profile in 3.00s`，零 warning |
| Rust 静态检查 | `cd src-tauri && cargo clippy --all-targets -- -D warnings` | `Finished dev profile in 1.57s`，零 warning |
| Rust 单测 | `cd src-tauri && cargo test` | 35 个测试目标全部 `ok`（含 284 个 lib 单测），557 passed / 0 failed |
| 焦点交出决策单测 | `cd src-tauri && cargo test --lib popover` | 10 passed（含 2 个 `paste_focus_plan` + 3 个 `should_inject_paste` 用例） |
| 新增文件格式 | `cd src-tauri && rustfmt --check --edition 2021 src/popover.rs` | 通过 |
| debug 链接 | `cd src-tauri && cargo build` | 成功产出 `target/debug/quickquick`（验证新增 objc2 FFI 能链接） |
| 前端类型 | `pnpm exec tsc -b` | 退出码 0，无输出 |
| 前端单测 | `pnpm test` | 60 个测试文件 / 519 个用例全部通过 |

**`cargo fmt --all --check` 未通过，但与本次改动无关**：`db.rs` / `ipc/system.rs` / `lib.rs` / `macos_paste.rs` / `tests/richtext_storage.rs` 在当前 rustfmt 1.99.1 下已不符合格式（多为 `assert_eq!` 换行宽度差异）。在 `c4fd069`（本次改动前的 HEAD）上单独 checkout 到临时 worktree 跑同一命令，报告的是**同样这 5 个文件**，确认是仓库既有状态 + 工具链版本错位。本次新增/重写的 `popover.rs` 不在该列表内，单独 `rustfmt --check` 通过；`ipc/system.rs`、`lib.rs` 的 fmt 差异点也均落在未改动的旧代码上。未顺手全局格式化，以避免把无关文件的改动混进本次 diff。

焦点交出判据落地后复核过同一件事：`rustfmt --check src/ipc/system.rs` 报告的 6 处差异（行 190 / 556 / 617 / 1172 / 1194 / 1206）经逐条核对均不含新增代码（`should_inject_paste`、`yield_focus_before_paste` 的返回判定、两臂的跳过注入分支都不在 diff 内），`popover.rs` 仍单独通过。

（本期无前端改动，前端两项是回归确认。）


### 6.2 需人工验证（**以下未验证**）

无头环境无法验证真实 AppKit 行为，以下全部需要在真机 GUI 上跑：

1. **面板弹出不夺焦**
   终端里先执行，记下前台应用名：
   ```bash
   osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true'
   ```
   然后在任意编辑器里按 `⌘⇧C` 唤起面板，**不点任何东西**，回到终端再执行同一条命令。预期两次结果相同；改造前第二次会得到 `QuickQuick`。
2. **中文输入法 composition 正常**
   唤起面板 → 切到拼音输入法 → 输入 `nihao` → 预期出现候选窗且能选词上屏。工具栏 / 候选窗位置异常、首个字母丢失、composition 被打断都算失败。（`tauri-nspanel` 内置了 `install_text_input_client_window_level`，这是面板里 webview 能正常接输入法的前提。）
3. **Enter 粘贴落到原应用且内容正确**
   编辑器里选中一段文字 → 唤起面板 → 选中另一条历史 → Enter。预期：内容出现在编辑器光标处，且编辑器窗口未失焦过。同时确认面板已关闭。
4. **Esc 关闭后原应用焦点完好**
   编辑器里光标在正文 → 唤起面板 → Esc。预期面板消失、编辑器仍是前台且光标位置不变，可以立刻继续打字。
5. **全屏应用下可唤起**
   把 Safari/编辑器切到全屏 Space → 在另一 Space 或同 Space 按热键。预期面板出现在**当前** Space、可见且可输入。
6. **面板失焦自动隐藏**
   唤起面板后点击另一个应用窗口。预期面板自动消失。
7. **主窗口粘贴不回归**
   从托盘打开主窗口 → 在编辑器里复制一段文字 → 回到主窗口点某条历史的粘贴。预期内容落到编辑器（走旧路径，仍有约 350ms 延迟）。
8. **辅助功能未授权时降级**
   系统设置里取消 QuickQuick 的辅助功能授权 → 重启 app → 唤起面板 → Enter。预期：剪贴板被写入，但不会自动粘贴（返回 `write_back_only`），且不崩溃、不夺焦。

**「焦点交出失败 → 只写回」这条路径无法手动触发**：它只在主线程被长任务占住超过 500ms、或 `run_on_main_thread` 派发失败时才出现，正常使用中几乎不可复现。目前该决策只由纯函数单测（`should_inject_paste`）覆盖，端到端表现未经真机验证——包括"面板仍可见时用户看到 `write_back_only`"这个 UX 是否可接受，需要真机确认。

## 七、对 Phase 2 的依赖与回退

- Phase 2 的面板高度自适应、键盘模型升级、数字直达、`cycle` 模式，全部建立在「面板不激活 app 也能收键盘」这一前提上。
- 若人工验证发现面板语义不成立（例如输入法在面板内异常、或全屏 Space 下不可见），回退方式是**只撤掉面板转制**（`bind_popover_lifecycle` 里对 `clip-popover` 的 `convert_clip_popover_to_panel` 调用），`paste_focus_plan` 会因「面板未持 key」自动落回旧的隐藏 app 路径，其余改动无需回滚。

## 八、已知局限

- **主线程派发超时（500ms）时面板路径退化为 `write_back_only`**：超时本身消不掉——主线程被长任务占住时不该无限等待，粘贴命令卡死比粘贴失败更糟。它现在不再是静默误报：`on_main_thread_blocking` 返回是否确认完成，粘贴路径在确认不了时不注入 `⌘V`、返回 `write_back_only`，用户看到的是"已写入剪贴板、未自动粘贴"，而不是一个兑现不了的成功。代价是这条路径上的面板可能仍然可见（`hide` 没做成），需要人工确认该表现是否可接受（见 §6.2）。
- **不做 `to_window` 逆操作**：参考实现需要它，是因为它有轻量模式会在空闲时销毁窗口，而带着面板子类让 tao 析构会抛无法跨 FFI 捕获的 ObjC 异常。QuickQuick 的 popover 只创建一次、从不销毁，故暂不涉及；将来若要加窗口回收，必须先补 `to_window`。
- **全屏 Space 行为未实测**：`full_screen_auxiliary` 与 `move_to_active_space` / `can_join_all_spaces` 的切换照搬了已验证实现的做法，但本项目未独立验证。
- **首次唤起时前端可能收不到 focus 事件**：面板是 `visible(false)` 创建、页面加载完成后才 show，前端订阅 `onFocusChanged` 与首次 `becomeKey` 之间存在理论竞态。该竞态在改造前同样存在（普通窗口路径），本期未处理。
