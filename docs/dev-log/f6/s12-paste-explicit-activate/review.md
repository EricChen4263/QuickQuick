---
id: F6-S12-review
type: review
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 通过
commit: 7f08550
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 主窗口粘贴显式激活目标 App（方案 B）（F6-S12）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/frontmost.rs`（新） | `LastExternalApp(Mutex<Option<i32>>)` 托管状态 + 纯逻辑 `should_record_pid` / `activation_decision` |
| `src-tauri/src/lib.rs` | NSWorkspace 观察者注册（block2 RcBlock + addObserverForName）、`extract_activated_pid`、cfg 对称 no-op |
| `src-tauri/src/ipc/system.rs` | `paste_to_front` 读 pid；`hide_and_restore_focus` 插入 `activate_target_app`；macOS/非 macOS cfg 分支 |
| `src-tauri/Cargo.toml` | `objc2-app-kit` features 增 NSWorkspace/NSRunningApplication；新增 `objc2-foundation`/`block2` |
| `src-tauri/tests/frontmost_logic_test.rs`（新） | 纯逻辑集成测试 10 例 |

参照标准：项目规范 + code-standards skill（函数≤50行 / 注释写"为什么" / unsafe 最小化 + 安全性说明 / cfg 对称）。

---

## 重点检查判定

### 1. block 捕获的 `Arc<LastExternalApp>` + `self_pid` Sendable/生命周期（通过）

`RcBlock::new(move |notification: NonNull<NSNotification>| { ... })` 捕获：
- `shared: Arc<LastExternalApp>`：`Arc<T>` 在 `T: Send + Sync` 时是 `Send`；`LastExternalApp` 内含 `Mutex<Option<i32>>`，`Mutex<T>` 在 `T: Send` 时满足 Send + Sync，i32 是 Send，故整体满足 sendable 要求。
- `self_pid: i32`：Copy 类型，无生命周期问题。
- block 由 `addObserverForName:object:queue:usingBlock:` 持有，queue=None 意味着在通知中心默认线程（对 NSWorkspace 通知即主线程）回调；block 生命周期由 forget 的 observer token 间接保证（token 存活则 block 存活）。**通过**。

### 2. `notification.as_ref()` 安全性（通过）

`NonNull<NSNotification>` 由 AppKit 在回调时传入，框架保证回调执行期间指针有效，转为 `&NSNotification` 安全。objc2-foundation 的 `addObserverForName_object_queue_usingBlock` 签名（block 参数类型为 `dyn Fn(NonNull<NSNotification>)`）与调用处一致。**通过**。

### 3. `extract_activated_pid` 的 nil/类型安全（通过）

- `notification.userInfo()` 返回 `Option`，`?` 处理 nil。
- `user_info.objectForKey(unsafe { NSWorkspaceApplicationKey })` 返回 `Option<Retained<NSObject>>`，`?` 处理 None。
- `.downcast::<NSRunningApplication>()` 是 objc2 运行时类型检查（isKindOfClass: 语义），失败返回 Err；`.ok()?` 正确处理类型不符情形，不会 panic/UB。
- `processIdentifier()` 是安全 API（生成代码 `#[unsafe(method)]` 属性是 objc2 宏标记，方法调用本身在取得有效引用后安全）。异常 userInfo 的所有情形均安全回退。**通过**。

### 4. `std::mem::forget` 观察者 token 重复注册风险（通过）

`setup_frontmost_tracking` 仅在 Tauri `setup` 闭包中调用一次（lib.rs:173），`setup` 由 Tauri 框架保证单次执行，无重复注册风险。forget 一个常驻 token 是此场景的正确做法，代码注释已说明理由（"与进程同生命周期，不在任何时点反注册"）。**通过**。

### 5. 线程模型与死锁分析（通过）

- **观察者回调**（queue=None → 主线程）→ `LastExternalApp::set`（Mutex 写，短暂持锁）。
- **paste_to_front 命令线程**→ `LastExternalApp::get`（Mutex 读，短暂持锁后释放）→ 投递消息（hide/activate）到主线程事件队列 → sleep(350ms)。
- 主线程处理 hide 和 activate 任务时不需要任何锁；命令线程调用 `get` 时早已释放锁再进入 sleep。两条线程操作 Mutex 的时间窗口不重叠，**无死锁**。
- lock poison：`set` 静默跳过，`get` 返回 None 触发降级路径，均不 panic。**通过**。

### 6. `run_on_main_thread` 时序正确性（通过）

`hide_and_restore_focus` 内三次消息投递顺序：
1. `hide_popover_window` → 投入事件队列（窗口 hide）
2. `hide_app` → 投入事件队列（Application hide）
3. `activate_target_app` → 投入事件队列（激活目标）
4. `thread::sleep(350ms)` — 此时命令线程等待，主线程按 FIFO 顺序依次执行三条任务

消息队列 FIFO 保证 hide 在 activate 之前执行，350ms 给予主线程充足时间完成全部三步（通常 <5ms）。时序正确。**通过**。

### 7. `activateWithOptions(empty)` 有效性评估（观察，非阻塞）

`NSApplicationActivationOptions::empty()`（值 0）对应旧版 `NSApplicationActivateAllWindows` 未置位，即"激活但不强制前置全部窗口"。在 `app.hide()` 已让出前台后调用，目标 app 应成为 key app 接收键盘事件，语义足够。

风险点：`activateWithOptions` 在 macOS 14+ 已标弃用（Apple 建议换用 `activate()`），但 objc2-app-kit 0.3.2 未生成无参 `activate()`（coding.md 偏离记录 #1 已说明），此 API 在当前 macOS 版本仍可用。`activateWithOptions` 返回 `bool` 表示请求发送成功，当前代码静默丢弃该值，若目标 app 处于特殊状态（正在启动/无窗口）可能静默失败，降级回 app.hide() 隐式路径。**已知偏离，可接受，不阻塞**。

真实激活效果（Cmd+V 是否落进目标 app）属于 GUI 行为，只能实测验证，在 coding.md 的"只能 GUI 实测"节已明确列出。

### 8. 降级路径完整性（通过）

| 情形 | 处理路径 |
|---|---|
| pid 为 None（尚未记录）| `activation_decision` 返回 FallbackHide，跳过激活，隐式路径 |
| pid = 0 或负数 | 同上 |
| 目标 app 已退出（runningApplication 返回 nil）| `activate_running_app_by_pid` 静默 return，隐式路径 |
| `run_on_main_thread` 派发失败 | eprintln + 隐式路径 |
| Mutex lock poison | set 跳过 / get 返回 None → FallbackHide |

所有降级路径均不 panic、不破坏 popover 流程。**通过**。

### 9. cfg 对称性（通过）

| 函数 | macOS | 非 macOS |
|---|---|---|
| `register_frontmost_observer` | 完整 NSWorkspace 实现 | no-op（_shared 前缀标未使用参数）|
| `hide_app` | `app.hide()` | 空实现 |
| `activate_target_app` | `run_on_main_thread` 激活 | 空实现 |
| `activate_running_app_by_pid` | NSRunningApplication 激活 | 不存在（仅 macOS 调用） |

`State<Arc<LastExternalApp>>` 在非 macOS 下仍注册（仅 setup_frontmost_tracking 调用，no-op 观察者），paste_to_front 可安全访问。`cargo check` 已验证 0 error 0 warning（artifacts/cargo-check.log）。**通过**。

### 10. 规范合规性（通过）

- **函数长度**：所有新增函数均 ≤ 50 行（最长为 `register_frontmost_observer` 约 36 行）。
- **注释**：均写"为什么"（mem::forget 原因、Mutex poison 静默策略、activateWithOptions(empty) 选择原因、固定等待替代轮询的说明）。
- **无死代码**：`_shared` 前缀 non-macOS no-op 参数有意，非死代码。
- **unsafe 最小化**：两处 unsafe（`notification.as_ref()` 和 `NSWorkspaceApplicationKey` 解引用）均有注释说明安全理由，块尽量小。
- **命名**：`should_record_pid` / `activation_decision` / `LastExternalApp` 均描述性，布尔逻辑函数有"should"前缀。

---

## 问题列表

**无置信度 ≥80 的问题。**

以下为置信度 <80 的观察，供参考，不阻塞：

| 置信度 | 位置 | 描述 |
|---|---|---|
| 65 | `src-tauri/src/ipc/system.rs:295` | `activateWithOptions` 返回 bool 被静默丢弃；目标 app 特殊状态下激活失败无日志，可考虑 `if !running_app.activateWithOptions(...) { eprintln!(...) }` 增加可调试性。非阻塞。 |
| 60 | `src-tauri/src/ipc/system.rs:279-280` | `activateWithOptions` 在 macOS 14+ 弃用，未来可能移除；当 objc2-app-kit 升级生成 `activate()` 后应跟进替换。已在 coding.md 偏离记录 #1 中标注，当前可接受。 |

---

## 无其他置信度 ≥80 问题

- block 捕获的 Arc 满足 Sendable，无跨线程 Send 风险。
- userInfo downcast 路径有完整 nil/类型安全处理。
- Mutex lock poison 处理方式与"尽力而为的优化"语义一致。
- `std::mem::forget` token 无重复注册，进程生命周期常驻可接受。
- 三个消息按 FIFO 时序正确执行，无竞争条件。
- cfg 非 macOS 编译路径对称，`State<Arc<LastExternalApp>>` 始终可取。
- 10 例纯逻辑测试全绿（artifacts/frontmost-test.log），make verify VERIFY_EXIT=0。

---

## 审查结论

**通过（APPROVE）。**

改动逻辑清晰、降级路径完整、unsafe 使用合理且有注释、cfg 对称编译通过、测试覆盖 headless 可测部分。真实激活效果（Cmd+V 是否落进目标 app）属 GUI 行为，在 coding.md 中已明确列为"只能 GUI 实测"项，非本次静态审查范围。

---

**VERDICT: APPROVE**

无置信度 ≥80 的 Critical 或 Important 问题。两条低置信度观察已列入问题表，不阻塞。
