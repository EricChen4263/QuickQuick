# f6-s12 主窗口"粘贴到前台"显式激活目标 app（方案 B）

## 要解决的 bug
从主窗口点"粘贴到前台"：内容已写进剪贴板、窗口也隐了，但自动 Cmd+V 没落进目标 app
（popover 路径正常）。根因：`paste_to_front` 的 trusted 分支只靠 `app.hide()` 隐式让
macOS 把焦点还给"上一个 app" + 固定等 350ms；主窗口下 QuickQuick 长时间是前台，macOS 的
"上一个 app"已陈旧/错误 → Cmd+V 落空。

## 方案 B：显式记录目标 app 并主动激活
事件驱动持续追踪"最近一个非 QuickQuick 前台 app 的 pid"，粘贴时按 pid 主动激活该 app。

## 改动文件清单
- `src-tauri/src/frontmost.rs`（新增）：托管状态 `LastExternalApp(Mutex<Option<i32>>)` +
  纯决策逻辑 `should_record_pid` / `activation_decision`（`ActivationDecision` 枚举）。存 pid（i32）
  而非 ObjC 对象，规避跨线程 Send。
- `src-tauri/src/lib.rs`：注册模块；setup 装 `setup_frontmost_tracking`（manage `Arc<LastExternalApp>`
  + 装 NSWorkspace 观察者）；新增 `register_frontmost_observer` / `extract_activated_pid`
  （macOS）与 non-macOS no-op。
- `src-tauri/src/ipc/system.rs`：`paste_to_front` 读 `State<Arc<LastExternalApp>>` 取 pid 传入；
  `run_paste_with_backend` 增 `target_pid` 形参；`hide_and_restore_focus` 在 hide 后、等待前插入
  `activate_target_app`；新增 `activate_target_app` / `activate_running_app_by_pid`（macOS）与
  non-macOS no-op。
- `src-tauri/Cargo.toml`：`objc2-app-kit` features 加 `NSWorkspace`/`NSRunningApplication`；
  新增 `objc2-foundation`（NSNotification/NSDictionary/NSString/NSValue）、`block2`（均已在 lockfile，
  作 objc2-app-kit 传递依赖，不引入功能重复的新生态）。
- `src-tauri/tests/frontmost_logic_test.rs`（新增）：纯逻辑集成测试（10 例）。

## 关键实现决策
- **观察者注册方式 = block2 RcBlock + `addObserverForName:object:queue:usingBlock:`**（非
  `define_class!`）。理由：无需声明新 NSObject 子类/管理 selector，最简且能编译；block 仅捕获
  `Arc<LastExternalApp>` 与 `self_pid`(i32) 满足 sendable 要求。
- **激活 API = `activateWithOptions(NSApplicationActivationOptions::empty())`**。偏离点：方案原文
  说用无参 `activate()`，但 objc2-app-kit 0.3.2 **未生成**无参 `activate()`，只暴露
  `activateWithOptions`（macOS14+ 标弃用但仍可用）。empty 选项语义等价（激活但不强制前置全部窗口），
  配合先前 hide 已让出前台，足以让目标成为 key app。
- **存 pid 而非对象**：NSRunningApplication 非 Send，观察者主线程 → 命令线程无法传对象；pid 跨线程安全，
  激活时再用 `runningApplicationWithProcessIdentifier(pid)` 取回对象。

## 线程模型
- 观察者注册（setup）+ 回调（投递到 NSWorkspace 默认 center，主线程）+ `LastExternalApp::set`：**主线程**。
- `paste_to_front` 命令体 + `LastExternalApp::get` + `hide_and_restore_focus`：**命令线程**。
- 真实激活 `activate_running_app_by_pid`：经 `app.run_on_main_thread(...)` 派发回**主线程**执行
  （activateWithOptions 必须主线程）。

## 降级路径
- pid 为 None（观察者还没记录到）或非正：`activation_decision` 返回 `FallbackHide` → 跳过显式激活，
  回退原有 `app.hide()` 隐式还焦路径，不破坏 popover 流程。
- `runningApplicationWithProcessIdentifier` 返回 nil（目标 app 已退出）：静默跳过，不 panic。
- `run_on_main_thread` 派发失败 / lock poison：eprintln 降级，不 panic。
- 观察者 token `std::mem::forget`：需与进程同生命周期，从不反注册（非疏漏）。

## 测试清单（可单测部分 · 全绿）
`src-tauri/tests/frontmost_logic_test.rs`，10 例：
- should_record_pid：排除自身 pid / 接受外部 pid / 排除 0 / 排除负数（4 例）
- LastExternalApp：new 为 None / set→get / 后写覆盖前写（3 例）
- activation_decision：有 pid→ActivatePid / None→FallbackHide / 非正→FallbackHide（3 例）

证据见 `artifacts/`：`frontmost-test.log`、`cargo-check.log`、`make-verify.log`。

## 只能 GUI 实测（不计入"通过"，留用户实跑）
1. 真实 NSWorkspace `DidActivateApplication` 通知触发、`extract_activated_pid` 取到真实 pid。
2. 真实 `activateWithOptions` 把目标 app 拉回前台。
3. 从**主窗口**点"粘贴到前台"，Cmd+V 真落进目标 app（本 bug 的最终验收）。
4. popover 路径回归未被破坏。

## make verify（全绿 · VERIFY_EXIT=0）
五步全过：[1/5] tsc、[2/5] cargo fmt --check、[3/5] cargo clippy -D warnings、
[4/5] vitest、[5/5] cargo test。机器证据：
- `artifacts/make-verify.log`：VERIFY_EXIT=0。
- `artifacts/cargo-test-full.log`：`cargo test: 387 passed (28 suites)`，0 失败。
- `artifacts/frontmost-test.log`：`running 10 tests` / `test result: ok. 10 passed; 0 failed`。
- `artifacts/cargo-check.log`：`cargo check` 0 error 0 warning（含 macOS cfg 代码）。

## 偏离记录
1. 激活 API 从方案原文的 `activate()` 改为 `activateWithOptions(empty)`：objc2-app-kit 0.3.2
   未生成无参 `activate()`，仅 `activateWithOptions` 可用，语义等价（见上）。
2. 未改 `capabilities/default.json`：NSWorkspace/NSRunningApplication 是进程内 ObjC FFI，
   不经 Tauri JS API ACL，无需新权限。
3. 未改 `paste.rs`：`FocusStep::RecordFrontmost`/`ActivateOriginalApp` 是顺序契约枚举，
   本改动让其落地为真实行为，契约本身无需变更。
