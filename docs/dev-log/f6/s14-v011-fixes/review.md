---
id: F6-S14-review
type: review
level: 小功能
parent: F6
children: []
created: 2026-06-04T00:00:00Z
status: 通过
commit: e63897c
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · v0.1.1 四项 bug 修复（F6-S14）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/tauri.conf.json` | A: CSP 加 `img-src 'self' data:`；B: main window 加 `trafficLightPosition` |
| `src-tauri/src/ipc/system.rs` | C: 新增 `hide_window_and_activate_target` / `hide_and_return_focus` / `return_focus_after_main_hide` |
| `src-tauri/src/lib.rs` | C: 注册 `hide_and_return_focus` 命令；CloseRequested 接线 `return_focus_after_main_hide` |
| `src-tauri/src/ipc/settings.rs` | D: 新增 `SELECTED_PROVIDER_CHANGED_EVENT` 常量；`set_selected_provider` 写入后 emit |
| `src-tauri/tests/frontmost_logic_test.rs` | C: 新增 2 条复合契约单测 |
| `src/ipc/events.ts` | D: 新增 `SELECTED_PROVIDER_CHANGED_EVENT` 常量 |
| `src/ipc/ipc-client.ts` | C: 新增 `hideAndReturnFocus()` 包装 |
| `src/clip-popover/main.tsx` | C: document Esc 改调 `hideAndReturnFocus` |
| `src/clip-popover/ClipPopoverApp.tsx` | C: onKeyDown Esc 改调 `hideAndReturnFocus` |
| `src/panels/settings/TranslateSourcePanel.tsx` | D: 新增 `selected-provider-changed` 监听 effect |
| `src/panels/translate/TranslatePage.tsx` | D: 新增 `selected-provider-changed` 监听 effect |
| `src/clip-popover/clip-popover-actions.test.tsx` | C: 更新 Esc 测试断言 |

参照标准：项目规范 + code-standards skill（函数≤50行 / 注释写"为什么" / cfg 对称 / 命名描述性 / 禁 any / setState 函数式）。

---

## A · 图片 CSP 改动审查

### CSP 红线核查（通过）

改前：`default-src 'self'; style-src 'self' 'unsafe-inline'`
改后：`default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'`

逐指令核查：

- **新增的 `img-src 'self' data:`**：`data:` URI scheme 仅对图片资源生效，不影响 `script-src` / `connect-src` / `style-src`；`default-src 'self'` 仍约束其它指令的默认值；无 `unsafe-eval`，无通配符，无对 `data:` 放松到 script/style 层。修复逻辑精准，覆盖 base64 图片的 release CSP 阻断，红线全部遵守。**通过**。
- **其它指令未变**：`default-src` / `style-src` / 隐含的 `script-src` 均与原值等价，无新增宽松。**通过**。

---

## B · trafficLightPosition 配置审查

### schema 合法性（通过）

查阅 tauri-utils-2.9.2 config.rs：`traffic_light_position: Option<LogicalPosition>`，serde camelCase 序列化为 `trafficLightPosition`，JSON 键名与 schema 匹配。`$schema: https://schema.tauri.app/config/2` 已引用，`cargo build` 通过（schema 校验过）。

前提条件满足：`titleBarStyle: Overlay` 已设、`decorations: true` 已设，`trafficLightPosition` 仅在两者同时满足时生效（否则字段被 tauri 忽略），配置一致。

### 取值合理性（通过，视觉项）

栏高 38px → 中线 y=19；红绿灯按钮直径约 12px → 顶边 ≈ 19−6=13，使按钮中线与文字中线对齐。`x=18` 在标题文字 76px 左缩进内，红绿灯不与文字重叠。视觉项，需 release 出包实测微调，设计依据充分。**通过**。

---

## C · 关窗/Esc 还焦审查

### s12 既有粘贴路径完整性（通过）

`hide_and_restore_focus(app, target_pid)` 被重构为调用 `hide_window_and_activate_target`（共享前半段：hide 窗口 + app.hide() + 按 pid 激活）后追加 `std::thread::sleep(350ms)`。粘贴路径仍完整执行三步，350ms 等待保留，`send_paste` 在 sleep 后调用。s12 功能未受破坏。**通过**。

### stay_in_tray 语义保留（通过）

`CloseRequested` 分支仍在 `stay_in_tray.load(Ordering::Relaxed) == true` 时才 `api.prevent_close() + win.hide()`，进程不退出、驻留托盘。`return_focus_after_main_hide` 仅补 `app.hide()` + 按 pid 激活，不改变窗口隐藏或进程生命周期。**通过**。

### 降级路径（通过）

`activation_decision(None)` → `FallbackHide`，`activate_target_app` 中 `let ActivationDecision::ActivatePid(pid) = ... else { return; }` 直接 return，不 panic。`hide_and_return_focus` 命令在 `last_external.get()` 返回 None 时走此降级，`return_focus_after_main_hide` 中 `app.try_state::<Arc<LastExternalApp>>().and_then(|state| state.get())` 也可安全返回 None，两条路径均不 panic。**通过**。

### try_state 失败处理（通过）

`return_focus_after_main_hide` 用 `app.try_state::<Arc<LastExternalApp>>()` 而非 `app.state()`（state 缺失时后者会 panic）。`and_then(|state| state.get())` 把 `None<State>` 和 `state.get()==None` 均映射为 `None`，统一走降级。注释说明「该状态在 setup 阶段必被托管，try_state 正常恒为 Some；缺失时降级跳过激活，不 panic」，逻辑可信。**通过**。

### cfg 对称（通过）

`activate_target_app` / `hide_app` 均有 `#[cfg(target_os = "macos")]` + `#[cfg(not(target_os = "macos"))]` 对称实现；非 macOS 为 no-op，编译无误。**通过**。

### Focused(false) 分支不还焦（通过）

`Focused(false)` 触发时 QuickQuick 已不是前台 app，焦点已由 OS 自然转移，无需显式激活目标——此时调用 `return_focus_after_main_hide` 反而可能把刚激活的其它 app 重新"拉回"目标（pid 可能已过时）。设计正确：只 hide 窗口，不主动还焦。**通过**。

### 函数长度与规范（通过）

- `hide_window_and_activate_target`：5 行函数体。
- `hide_and_return_focus`：4 行函数体。
- `return_focus_after_main_hide`：6 行函数体。
- 注释均解释"为什么"（与 `hide_and_restore_focus` 的差异、无等待的原因、降级语义），无装饰注释。命名为动词+名词。**通过**。

### 单测覆盖（通过）

新增 2 条契约测试 `hide_and_return_focus_falls_back_when_no_pid_recorded` / `hide_and_return_focus_activates_recorded_pid`，复现命令体内 `state.get() → activation_decision` 取值链，覆盖 None→FallbackHide 和 pid→ActivatePid 两分支。测试通过（make-verify.log 确认 cargo test 全绿）。**通过**。

---

## D · 翻译源双向同步审查

### 事件名两端一致（通过）

后端 `SELECTED_PROVIDER_CHANGED_EVENT: &str = "selected-provider-changed"`（settings.rs:74）与前端 `SELECTED_PROVIDER_CHANGED_EVENT = "selected-provider-changed" as const`（events.ts:19）字符串逐字符一致。注释两端均标注「改动需两端同步」，沿袭既有 `provider-config-changed` 范式。**通过**。

### cancelled + unlisten 防泄漏（通过）

两页（TranslatePage.tsx:174–201、TranslateSourcePanel.tsx:201–228）均使用完全一致的 `cancelled` ref + `unlisten` 变量范式：
- `listen(...)` 成功时若已取消则立即调 `fn()`；否则赋值 `unlisten`。
- cleanup 函数先设 `cancelled.current = true` 再 `unlisten?.()`，防卸载后 setState。
- 与同组件内其它事件监听（`TRANSLATE_HISTORY_CHANGED_EVENT` / `PROVIDER_CONFIG_CHANGED_EVENT`）实现模式完全对称。**通过**。

### 自发自收无循环（通过）

`set_selected_provider` 命令 emit `selected-provider-changed` → 两页监听回调调 `getSelectedProvider()` → 仅执行 `setSelectedId(currentId)` / `setSelectedProviderId(currentId)`（直接赋值，不再调 `setSelectedProvider`），不触发新的 emit，无循环。同一页自己触发事件后收到通知，值与当前一致，赋值幂等。**通过**。

### 回调刷新选中态（通过）

- `TranslatePage`：`setSelectedProviderId(currentId)`，被 `TranslateWorkspace` 消费渲染 provider 选择器。
- `TranslateSourcePanel`：`setSelectedId(currentId)`，被 `ProviderCard.isSelected` 条件消费渲染徽标。
- 两处均从后端重读最新值（`getSelectedProvider()`）而非直接用事件载荷（payload 为 `()`），保证与持久层同步，不依赖乐观更新。**通过**。

### 测试 stderr 噪音（观察，不阻塞）

`TranslateSourcePanel.test.tsx` 未 mock `@tauri-apps/api/event` 的 `listen`，导致每条测试的 jsdom 环境下新增 `selected-provider-changed 监听注册失败` stderr 输出。`.catch` 已吸收错误，10 个测试全部通过，不影响正确性判断。

此为漏添 mock 导致的测试完整性缺口，但属同类既存问题（`provider-config-changed` 监听也无 mock），且 `translate-page.test.tsx` 已正确 mock 了 `listen`。置信度 70，不阻塞。可后续统一补全 `TranslateSourcePanel.test.tsx` 的 `@tauri-apps/api/event` mock 以消除 stderr 噪音（参照 `translate-page.test.tsx` 第 7-8 行范式）。

---

## 规范合规性（全部通过）

| 检查项 | 结论 |
|---|---|
| 函数 ≤ 50 行 | 新增 4 个 Rust 函数均 ≤ 10 行；前端 effect 代码块 ≤ 30 行 |
| 禁 `any` | 无任何 `any` 类型使用 |
| setState 函数式 | `setSelectedId(currentId)` 直接赋新值（非基于旧值计算），符合规范；不涉及 prev 依赖 |
| 注释写"为什么" | 关键函数的重构理由、与 s12 差异、降级语义均有注释 |
| 无装饰注释 | 未发现横线分隔符等装饰性注释 |
| 无死代码/TODO/FIXME | 全局 grep 确认无遗留 |
| 命名描述性 | `hide_window_and_activate_target` / `hide_and_return_focus` / `return_focus_after_main_hide` / `hideAndReturnFocus` 均为动词+名词 |
| cfg 对称 | macOS/非 macOS 分支均编译，no-op 正确 |

---

## 问题列表

**无置信度 ≥80 的 Critical 或 Important 问题。**

以下为置信度 <80 的观察，供参考，不阻塞：

| 置信度 | Severity | 位置 | 描述 | 建议 |
|---|---|---|---|---|
| 70 | Important | `src/panels/settings/TranslateSourcePanel.test.tsx` | 新增 `selected-provider-changed` listen effect 未在测试中 mock `@tauri-apps/api/event`，每次渲染产生 stderr 噪音，但错误被 `.catch` 吸收，不影响测试通过 | 参照 `translate-page.test.tsx:7–8` 在文件顶部添加 `vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }))` |

---

## 审查结论

**通过（APPROVE）。**

4 项修复的关键路径均已核实：A CSP 只新增 img-src、红线全过；B schema 合法、取值依据充分；C 粘贴路径完整保留（s12 未破坏）、stay_in_tray 语义保留、降级路径不 panic、cfg 对称；D 事件名两端一致、cancelled+unlisten 范式防泄漏、无循环无抖动。make verify EXIT=0，所有测试通过。一条低置信度观察（测试 mock 缺失）不阻塞合并。

---

**VERDICT: APPROVE**

无置信度 ≥80 的 Critical 或 Important 问题。一条置信度 70 的 Important 观察（测试 mock 漏添导致 stderr 噪音）供后续跟进，不阻塞。

---

## B 二次修正复审（NSWindow 重定位）

**复审时间**：2026-06-05
**改动范围**：
- `src-tauri/src/lib.rs`：新增 `traffic_light_logical_position()` + `reposition_traffic_lights()`（macOS）+ no-op（非 macOS）；`setup_main_window_behavior` 接线调用
- `src-tauri/Cargo.toml`：objc2-app-kit 加 NSWindow/NSView/NSControl/NSButton feature；objc2-foundation 加 NSGeometry feature
- `src-tauri/tauri.conf.json`：`trafficLightPosition` y 13→12（与 NSWindow 重定位值对齐，作 fallback）
- `src/theme/components.css`：`.qq-titlebar` padding-left 76→96px；font-size 13→15px

### unsafe 安全性（通过）

`ns_window()` 返回 `Result<*mut c_void, _>`，`let Ok(ns_window_ptr)` 排除 Err。tauri 的 `ns_window()` 在 OK 路径保证指针非 null（窗口存活期内），转为 `&NSWindow` 不会 null deref。内联注释 `// ns_window() 返回的指针在窗口存活期间有效` 已标注生命周期前提，调用点在 setup（窗口刚创建、立即存活），生命周期假设成立。`close.superview()` 已包裹在 `unsafe {}` 块内，返回 `Option` 用 `let Some(...)` 处理，不 panic。**通过**。

### 坐标换算正确性（通过）

NSView 使用左下原点（非翻转坐标系）；`new_y = container_height - y - frame.size.height` 将"距顶 y"正确转换为左下原点的 origin.y。`base_x = close.frame().origin.x` 取最左按钮基准，`new_x = x + (frame.origin.x - base_x)` 对 close 按钮 delta=0，对 miniaturize/zoom 保持原相对间距整体平移，不改变三颗按钮间距。container 是 close 的 superview（NSTitlebarContainerView），三颗按钮共享同一 superview，height 取值合理。数学正确。**通过**。

### 健壮性（通过）

全部失败路径均 early return 不 panic：ns_window 失败 → eprintln + return；close 按钮 None → return；superview None → return；循环内单颗按钮 None → continue。`setFrameOrigin` 无返回值，正确忽略。**通过**。

### cfg 对称（通过）

`#[cfg(target_os = "macos")]` 实现 + `#[cfg(not(target_os = "macos"))]` no-op，函数签名完全一致；调用点用 `#[cfg(target_os = "macos")]` 守卫，非 macOS 编译路径干净。Cargo.toml 新增 feature 为 NSWindow/NSView/NSControl/NSButton（按钮类型链）+ NSGeometry（NSPoint/NSRect），最小必要。**通过**。

### 重定位时机（通过，已知前提）

在 `setup_main_window_behavior`（窗口 visible:false 阶段）调用一次。用户本地 release 实测已确认按钮位置正确、AppKit 在 show 时未重排覆盖——此为已验证的可接受前提，注释已说明 config 失效原因及 NSWindow 直接重定位的理由。**通过（依赖实测确认）**。

### 函数长度与规范（通过）

- `traffic_light_logical_position`：1 行体，纯函数便于单测。
- `reposition_traffic_lights`（macOS）：函数体约 37 行，远 ≤ 50 行。
- 注释均写"为什么"（tafficLightPosition 在隐藏窗口失效的根因、坐标系说明、base_x 间距保持语义）；无装饰注释；命名为动词+名词。**通过**。

### CSS 改动（通过）

- `padding-left 76→96px`：注释说明原因（76px 时绿灯紧贴"QuickQuick"）。合理。
- `font-size 13→15px`：纯视觉调整，无逻辑影响。**通过**。

### 问题列表

**无置信度 ≥80 的 Critical 或 Important 问题。**

---

**B 二次修正复审结论：通过（APPROVE）。**

NSWindow 直接重定位方案各核查项全部通过：unsafe 生命周期前提有注释且成立、坐标翻转公式正确、全路径健壮无 panic、cfg 对称、函数规范。CSS 调整合理。无置信度 ≥80 的阻塞问题。

**VERDICT: APPROVE**
