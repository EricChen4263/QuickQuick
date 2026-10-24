---
status: done
category: implementation
subject: P2 门禁探针化 · ADR §6.2 #1 面板不夺焦 / #6 面板失焦自动隐藏
summary: 新增 src-tauri/examples/panel_probe.rs（自建 NSPanel + NSApplication.run() 事件循环，不依赖 Tauri 启动流程），把 §6.2 的 #1/#6 做成机器可读 PASS/FAIL 探针。正常路径 3 项判据全绿（exit 0），连续复跑 3 次结论一致；三组对照组实测全红（只去 nonactivatingPanel 样式位 → 面板拿不到 key；面板化前旧路径 → 前台被夺走；不挂 resign-key 钩子 → 面板仍可见，exit 1）。关键实测发现：非激活面板拿到 key 后，程序化激活另一个 app 会让前台换人但**不产生** NSWindowDidResignKeyNotification，因此 #6 的「另一个 app 抢 key」这一跳在本机不可机械复现，探针退到显式 resignKeyWindow 驱动并在证据里标注 drive=explicit_resign_key。
evidence:
  commits:
    - "（本卡对应提交）test(gate): 新增面板焦点语义探针（#1 不夺焦 / #6 失焦隐藏）"
  commands:
    - "cd src-tauri && cargo run --example panel_probe"
    - "cd src-tauri && cargo run --example panel_probe -- control-no-nonactivating"
    - "cd src-tauri && cargo run --example panel_probe -- control-legacy-activate"
    - "cd src-tauri && cargo run --example panel_probe -- control-no-resign-hide"
    - "cd src-tauri && rustfmt --check --edition 2021 examples/panel_probe.rs"
    - "cd src-tauri && cargo clippy --example panel_probe"
  urls: []
---

# AI 活动记录 · 面板焦点语义探针（门禁 #1 / #6 探针化）

## 摘要

`docs/design/phase2-gate-verification.md` §二把 ADR §6.2 的八项人工验证重新分类，其中 #1「面板弹出不夺焦」与 #6「面板失焦自动隐藏」被标为**可探针化（待做）**。本卡把它做出来并实跑。

- 产物：`src-tauri/examples/panel_probe.rs`（新增，单文件；`examples` 不进默认 `cargo test`，未改 `Cargo.toml`）。
- 正常路径：3 项判据 `#1` / `#1-key` / `#6` 全 PASS，退出码 `0`，连续复跑 3 次结论一致。
- 对照组：三组全红（退出码 `1`），其中「面板化前旧路径」把 #1 打红（前台从 `com.deepseek.dsh#10954` 变成探针进程 `#no-bundle#2265`）。
- **未改动任何生产代码**：`src-tauri/src/**` 与 `docs/design/**` 一行没动。

## 一、第一步：本机 GUI 会话可用性（结论：可用）

探针在动任何判据前先做前置探测，全部通过：

```
PROBE_ENV window_created=true panel_visible_after_show=true panel_key_after_show=true \
           can_become_key_window=true activation_policy=0
PROBE_ENV gui_session=ok frontmost_before=com.deepseek.dsh#10954 probe_app_is_active=false
```

逐项对应任务要求的第一条：

| 能力 | 结论 | 证据 |
|---|---|---|
| 能创建 NSApplication | 可用 | `nsapp_ready=true`，`activation_policy=0`（Regular） |
| 能创建并上屏窗口 | 可用 | `window_created=true`、`panel_visible_after_show=true` |
| 能读 `NSWorkspace.frontmostApplication` 的 bundleIdentifier 与 pid | 可用 | `frontmost_before=com.deepseek.dsh#10954`（多次复跑里出现过 `com.apple.finder#1360`，说明读到的是真实前台而非常量） |
| 能建 `NSPanel` 并让它成为 key window | 可用 | `panel_key_after_show=true` |
| 能让另一个 app 成为前台 | **部分可用** | `activateWithOptions` 返回 true 且前台确实换到 `com.apple.finder#1360`；但换台**不会**让面板 resign key（见 §四） |

前置探测失败时探针打印 `gui_session=unavailable reason=...` 并以退出码 `3` 退出，不会伪造判据结果。

## 二、探针复刻的生产语义（设计与依据）

探针不调用 `tauri-nspanel`，也不依赖 Tauri 启动流程；它按 `src-tauri/src/popover.rs` 的生产行为**自建**等价物：

1. **`NSPanel` 子类把 `canBecomeKeyWindow` 覆盖为 true**。这一步是必须的，不是可选装饰：无标题栏窗口的 `canBecomeKeyWindow` 默认返回 **false**，`makeKeyWindow` 会静默失败。生产在 `tauri_panel! { panel!(ClipPopoverPanel { config: { can_become_key_window: true } }) }`（`popover.rs:96-104`）里给的是同一个开关，所以探针必须同样覆盖，否则「面板能收键盘」这个前提在探针里根本不成立（实测见 §四对照组 A1）。
2. **样式位追加 `NSWindowStyleMaskNonactivatingPanel`**（生产 `add_style_mask`，`popover.rs:414`）。这是「不激活 app 也能成为 key window」的全部机制。
3. **show 顺序与生产 `show_and_make_key()` 一致**：`makeFirstResponder(contentView)` → `orderFrontRegardless` → `makeKeyWindow`（先 order front 再 make key）。探针未使用 `makeKeyAndOrderFront`，因为生产的 crate 实现就是「order front + make key」两步。
4. **失焦隐藏走 `NSWindowDidResignKeyNotification` 观察者 + `orderOut`**（生产 `observe_resign_key_hide`，`popover.rs:466`）。不使用 `set_event_handler`——生产注释说明它会 `setDelegate:` 替换 tao 的窗口代理，掐断前端依赖的 focus 事件；探针同样不占用 delegate。
5. **主线程约束**：`NSPanel` 是 `MainThreadOnly` 类型，`ProbePanel::alloc(mtm)` 在编译期就要求主线程标记；判据在 `NSApplication.run()` 的 `NSTimer` 回调里执行，探针不开任何子线程。

判据口径：

| check | gate | 断言 |
|---|---|---|
| `keeps_frontmost` | `#1` | show 前后 `frontmostApplication` 的 (bundleIdentifier, pid) 完全相同 |
| `panel_holds_key` | `#1-key` | 同一轮 show 之后 `panel.isKeyWindow() == true`（否则 #1 是空判据） |
| `hides_on_resign_key` | `#6` | 面板持有 key 后一次 key 转移 → `panel.isVisible() == false` |

`#1-key` 是本次新增的前提判据：如果面板压根拿不到 key，「不夺焦」会变成恒真废话。

## 三、正常路径实跑（`cargo run --example panel_probe`，退出码 0）

```
PROBE_ENV gui_session=probing variant=baseline nsapp_ready=true
PROBE_ENV window_created=true panel_visible_after_show=true panel_key_after_show=true can_become_key_window=true activation_policy=0
PROBE_ENV gui_session=ok frontmost_before=com.deepseek.dsh#10954 probe_app_is_active=false
CHECK check=keeps_frontmost gate=#1 verdict=PASS
EVIDENCE check=keeps_frontmost before=com.deepseek.dsh#10954 after=com.deepseek.dsh#10954 panel_key=true panel_visible=true probe_app_is_active=false
CHECK check=panel_holds_key gate=#1-key verdict=PASS
EVIDENCE check=panel_holds_key panel_key=true show_mode=order_front_then_make_key nonactivating_style=true
CHECK check=hides_on_resign_key gate=#6 verdict=PASS
EVIDENCE check=hides_on_resign_key panel_key_before=true panel_visible_before=true target=com.apple.finder#1360 activate_returned=true frontmost_after_activation=com.apple.finder#1360 hidden_by_activation=false resign_notifications_after_activation=0 drive=explicit_resign_key panel_visible_after=false panel_key_after=false nsapp_is_active_after=false resign_notifications=1 hook=true timeout_ms=1500
PANEL_PROBE_SUMMARY variant=baseline gates=3 pass=3 fail=0 exit=0
```

要点：

- `before == after == com.deepseek.dsh#10954`，且探针进程的**外部视角** `probe_app_is_active=false`——不是「内部 `NSApplication.isActive` 撒谎」导致的自证清白。
- 同一次 show 里 `panel_key=true`：面板确实拿到了 key，所以 #1 不是空判据。
- #6 的证据里 `drive=explicit_resign_key`、`resign_notifications=1`：见 §四。

## 四、三组对照组实跑（全部红，退出码 1）

### A1 `control-no-nonactivating`：只去掉 nonactivatingPanel 样式位

```
CHECK check=keeps_frontmost gate=#1 verdict=PASS
EVIDENCE check=keeps_frontmost before=com.deepseek.dsh#10954 after=com.deepseek.dsh#10954 panel_key=false panel_visible=true probe_app_is_active=false
CHECK check=panel_holds_key gate=#1-key verdict=FAIL
EVIDENCE check=panel_holds_key panel_key=false show_mode=order_front_then_make_key nonactivating_style=false
PANEL_PROBE_SUMMARY variant=control-no-nonactivating gates=2 pass=1 fail=1 exit=1
```

**如实记录**：只去掉样式位，#1 仍然是绿的。原因不是判据不敏感，而是物理事实——不激活 app 就不会夺焦。去掉样式位的真实后果是**面板拿不到 key**（`panel_key=false`），也就是「面板能收键盘」这个前提没了。所以「样式位」的作用是 key 可达性，不是前台归属。

### A2 `control-legacy-activate`：面板化之前的旧路径（必须先激活 app）

```
CHECK check=keeps_frontmost gate=#1 verdict=FAIL
EVIDENCE check=keeps_frontmost before=com.deepseek.dsh#10954 after=<no-bundle>#2265 panel_key=false panel_visible=true probe_app_is_active=true
CHECK check=panel_holds_key gate=#1-key verdict=FAIL
EVIDENCE check=panel_holds_key panel_key=false show_mode=activate_then_make_key_and_order_front nonactivating_style=false
PANEL_PROBE_SUMMARY variant=control-legacy-activate gates=2 pass=0 fail=2 exit=1
```

这是**夺焦可达的证明**：前台从 `com.deepseek.dsh#10954` 变成探针进程 `#<no-bundle>#2265`，`probe_app_is_active=true`。#1 判据对此变红，说明它的确是能失败的判据，不是恒真断言。

### B `control-no-resign-hide`：不挂失焦隐藏

```
CHECK check=hides_on_resign_key gate=#6 verdict=FAIL
EVIDENCE check=hides_on_resign_key panel_key_before=true panel_visible_before=true target=com.apple.finder#1360 activate_returned=true frontmost_after_activation=com.apple.finder#1360 hidden_by_activation=false resign_notifications_after_activation=0 drive=explicit_resign_key panel_visible_after=true panel_key_after=false nsapp_is_active_after=false resign_notifications=0 hook=false timeout_ms=1500
PANEL_PROBE_SUMMARY variant=control-no-resign-hide gates=1 pass=0 fail=1 exit=1
```

面板先拿到 key（`panel_key_before=true`）、key 也确实让出了（`panel_key_after=false`），但**因为没挂钩子，面板仍然可见**（`panel_visible_after=true`）→ #6 变红。说明 #6 判的是「resign key 后是否隐藏」，不是「key 是否让出」。

## 五、#6 的关键实测发现：程序化激活另一个 app 不会让面板 resign key

正常路径里 `activateWithOptions` 返回 true 且前台确实换到了 Finder（`frontmost_after_activation=com.apple.finder#1360`），但：

- `resign_notifications_after_activation=0`
- `hidden_by_activation=false`
- 面板在被显式 `resignKeyWindow` 之前一直 `isKeyWindow()==true`

即：**非激活面板在 app 从未活跃的情况下拿到的 key，不会因为「另一个 app 成为前台」而失去**——没有「app 失活」这一跳，也就没有 `NSWindowDidResignKeyNotification`。这是本机实测结果（多次复现），也是探针把 #6 的驱动器做成两级的原因：

1. 驱动器 A（最贴近真实场景）：激活另一个 app；若面板隐藏 → `drive=foreign_app_activation`。
2. 驱动器 B（确定性兜底）：显式 `resignKeyWindow()`；若面板隐藏 → `drive=explicit_resign_key`（证据里同时保留了驱动器 A 的实测数据）。

探针不做「挑选通过的那一条」，两条驱动器的观测值都原样打印，由证据说话。

## 六、3 次连续复跑（稳定性）

主工作区（`src-tauri/`）连续 3 次，全部 `gates=3 pass=3 fail=0 exit=0`，且三次之间前台基线 app 变过（DSH / Finder），#1 均成立：

```
--- main-tree run #1 (exit=0) ---
PROBE_ENV gui_session=ok frontmost_before=com.deepseek.dsh#10954 probe_app_is_active=false
CHECK check=keeps_frontmost gate=#1 verdict=PASS
CHECK check=panel_holds_key gate=#1-key verdict=PASS
CHECK check=hides_on_resign_key gate=#6 verdict=PASS
PANEL_PROBE_SUMMARY variant=baseline gates=3 pass=3 fail=0 exit=0
--- main-tree run #2 (exit=0) ---
PROBE_ENV gui_session=ok frontmost_before=com.apple.finder#1360 probe_app_is_active=false
CHECK check=keeps_frontmost gate=#1 verdict=PASS
CHECK check=panel_holds_key gate=#1-key verdict=PASS
CHECK check=hides_on_resign_key gate=#6 verdict=PASS
PANEL_PROBE_SUMMARY variant=baseline gates=3 pass=3 fail=0 exit=0
--- main-tree run #3 (exit=0) ---
PROBE_ENV gui_session=ok frontmost_before=com.deepseek.dsh#10954 probe_app_is_active=false
CHECK check=keeps_frontmost gate=#1 verdict=PASS
CHECK check=panel_holds_key gate=#1-key verdict=PASS
CHECK check=hides_on_resign_key gate=#6 verdict=PASS
PANEL_PROBE_SUMMARY variant=baseline gates=3 pass=3 fail=0 exit=0
```

隔离工作树（`git worktree add /tmp/qq-panel-probe-wt HEAD`）另跑 3 次，结论相同（当时主工作区因他人未提交改动在编译期报 `pasteboard_meta::MainThreadMetaReader` 缺失），说明结论不依赖主工作区的未提交状态。

## 七、对门禁 #1 / #6 的结论建议（供 lead 写入门禁文档）

> 以下只是执行者建议；`docs/design/*` 未改动。

1. **#1 可以降级为机械证据**（面板弹出不夺焦）：`frontmost`(bundle+pid) 前后不变，且探针进程外部视角 `is_active=false`，同时 `panel.isKeyWindow()==true`。建议门禁文案要求**同时**看 `#1` 与 `#1-key`，只报 #1 会漏掉「面板压根拿不到 key」这种组合。
2. **#6 只能部分降级**：`resign key → orderOut` 这条接线可以机械复跑（对照组 B 证明它可失败）；但「**用户点击另一个 app 会不会让面板 resign key**」这一跳在本机程序化路径下**不成立**（实测前台换了、通知没来、面板仍持有 key）。建议门禁保留一条真人项，并把它写成更具体的问句：「唤起面板 → 点击别的 app 窗口 → 面板是否消失」，同时把本节结论作为已知风险记录在案。
3. **建议把 §6.2 #2/#3/#4/#5/#8 维持原分类**，本探针不覆盖它们。
4. 若后续把 #1/#6 探针纳入常规门禁，建议在 CI 之外跑（需要真实 GUI 会话与「另一个 app」可被激活），并接受它会把前台焦点短暂切到 Finder（探针结束会激活回原 app）。

## 八、未验证项与已知局限

- **真实鼠标点击是否触发 resign key 未验证**：需要真人点一下别的 app 窗口，或需要辅助功能授权后用 `CGEvent` 合成点击（本机未授权，探针未尝试，避免干扰用户操作）。这是 #6 结论里唯一不能机械闭合的一环。
- **`activateIgnoringOtherApps` 是已废弃 API**：对照组 A2 用它复现「面板化前旧路径夺焦」，在本机（macOS 27.0.1 / 26A434 / arm64，无 bundle 的探针进程）仍生效；生产 `activate_app_macos()` 用的是现代 `activate()`，两者不完全等价，A2 只用来证明判据可失败。
- **探针进程无 Info.plist/bundle**：启动时 `NSApplicationActivationPolicy` 默认是 Prohibited，探针显式设为 Regular（与生产 app 一致）。把同一个二进制放进真实 `.app` bundle 里直接运行，观测结果与 `cargo run` 一致；`open -n` 拉起 bundle 时无法可靠捕获 stdout（本次尝试没拿到输出），故未作为主要验证路径。文档里的复跑命令仍是 `cargo run --example panel_probe`。
- **未覆盖生产的面板层级（`PanelLevel::Dock`）与跨 Space 行为**（`collectionBehavior`）：焦点语义与它们无关，探针刻意不加，避免把两件事混在一个判据里。
- **探针会短暂切前台**：#6 的驱动器 A 会激活 Finder（跑完探针会把原前台 app 激活回来），对照组 A2 会让探针进程成为前台。不写文件、不写库。

## 九、改动范围

| 文件 | 改动 |
|---|---|
| `src-tauri/examples/panel_probe.rs` | 新增（约 700 行，含注释） |
| `docs/dev-log/activity/2026-10/2026-10-25-panel-gate-probes.md` | 本卡 |

**未改动**：`src-tauri/src/**`（含 `popover.rs`）、`src-tauri/Cargo.toml`、`docs/design/**`、前端全部。

## 十、复跑命令

```bash
cd src-tauri
cargo run --example panel_probe                               # 期望 exit 0，三项 PASS
cargo run --example panel_probe -- control-no-nonactivating    # 期望 exit 1（#1-key FAIL）
cargo run --example panel_probe -- control-legacy-activate     # 期望 exit 1（#1 FAIL）
cargo run --example panel_probe -- control-no-resign-hide      # 期望 exit 1（#6 FAIL）
```

最后一行固定为 `PANEL_PROBE_SUMMARY variant=... gates=... pass=... fail=... exit=...`，便于比对。
