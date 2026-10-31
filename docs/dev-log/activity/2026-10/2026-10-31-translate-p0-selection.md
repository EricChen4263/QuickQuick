---
status: done
category: implementation
subject: 翻译交互 P0 · 热键即译取当前选中文字（macOS AX），失败降级剪贴板并标来源
summary: Cmd+Shift+T 唤起 trans-popover 时改为「选中文字优先、失败降级剪贴板」，浮窗显式标注来源（来源：选中 / 来源：剪贴板）并在取词通道不可用时说明原因。新增 selection.rs（SelectionOutcome + classify 纯判定 + macOS AXUIElement 取词，0.3s 消息超时，不模拟 ⌘C、不碰剪贴板）；trans-source 事件 payload 由 Option<String> 改为结构体 TransSource{text, source, fallbackReason}；冷启动改为经新 IPC 命令 get_trans_popover_source 补读 Rust 侧来源快照（前端不再自读剪贴板，否则来源标签必错）。实跑：cargo test 909 passed / 0 failed（37 suites）、cargo clippy -D warnings 干净、cargo fmt --check 干净、vitest 84 files / 929 passed、tsc -b 退出 0。真机手测（AX 能否读到真实应用选区 / 授权撤销 / 多屏）未做，已写入 pending-manual.yaml（TP0-M01..M03）。
evidence:
  commits: []
  files:
    - "src-tauri/src/selection.rs（新增：SelectionOutcome / classify / macOS AXSelectedText 取词）"
    - "src-tauri/src/ipc/trans_popover.rs（新增：TransSource payload / build_trans_source / TransSourceStore / get_trans_popover_source）"
    - "src-tauri/src/popover.rs（show 前取词、struct payload、写来源快照）"
    - "src-tauri/src/lib.rs + src-tauri/src/ipc/mod.rs（模块声明、状态注册、命令注册）"
    - "src-tauri/Cargo.toml + Cargo.lock（core-foundation 0.10 直接依赖，lock 仅加一行）"
    - "src/ipc/ipc-client.ts + src/ipc/events.ts（TransSourcePayload 类型 + getTransPopoverSource）"
    - "src/trans-popover/TransPopoverApp.tsx + MiniTranslate.tsx + trans-source-payload.ts + popover.css（来源标签 / 降级提示 / payload 收敛）"
    - "测试：trans-popover.test.tsx / MiniTranslate.test.tsx / trans-source-payload.test.ts / ipc-client.test.ts"
    - "docs/dev-log/pending-manual.yaml（TP0-M01..M03 真机手测项）"
  commands:
    - "cd src-tauri && cargo test  # 37 suites / 908 passed / 0 failed"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings  # 干净，退出 0"
    - "cd src-tauri && cargo fmt --check  # 退出 0"
    - "npx tsc -b  # 退出 0"
    - "npx vitest run  # 84 files / 929 passed"
    - "npx vitest run src/trans-popover src/ipc  # 8 files / 104 passed"
  urls:
    - "https://developer.apple.com/documentation/applicationservices/1462085-axuielementcopyattributevalue"
    - "https://developer.apple.com/documentation/applicationservices/1462070-axuielementsetmessagingtimeout"
---

# AI 活动记录 · 翻译交互 P0（热键即译取词）

## 摘要

- 取词走 macOS Accessibility：`AXUIElementCreateSystemWide` → `AXFocusedUIElement` → `AXSelectedText`，
  仅对 CFString 类型的值做转换，所有 CF 对象用 create rule 包装、由类型 Drop 释放。
- **取词发生在任何 show / 激活之前**：本进程一旦成为前台应用，源应用的选区就没了（设计 §5 实现约束）。
- 取词失败按四种结局收敛，并**把原因带到浮窗**：未授权辅助功能 / 无选中 / 平台不支持；只有后两种
  中的「未授权 / 平台不支持」会在浮窗里出提示，无选中是常态、不打扰。
- `trans-source` payload 从 `Option<String>` 变成 `TransSource{ text, source, fallbackReason }`（serde camelCase），
  与前端 `TransSourcePayload` 逐字对齐，并用一条 Rust 序列化断言把 camelCase 线格式钉住。
- 浮窗显式标注「来源：选中 / 来源：剪贴板」（D4），避免把剪贴板旧内容误当选中文字。
- 不改翻译缓存/重试/取消链路，不引入新翻译源，不碰 STRATEGY.md。

## 关键取舍

### 1. 冷启动为什么改成「Rust 存快照 + 新 IPC 命令补读」，而不是前端自读剪贴板

窗口首次创建时 webview 还没挂载，Rust 在 `show` 之后 emit 的那一份事件会落在没人监听的时候丢掉。
旧实现靠前端挂载后自己 `listClipItems()` 补位，这在「只译剪贴板」时是对的——但取词之后就成了错的：
用户明明选中了文字，冷启动却会去查剪贴板，并把来源标成「剪贴板」。

因此：Rust 在取词时把同一份 `TransSource` 写进 `TransSourceStore`，前端挂载时经
`get_trans_popover_source` 补读。两条路径拿到的是**同一份数据**，来源标签不可能自相矛盾；
只有在快照为空（非热键路径直接打开该窗口）时才回退 `listClipItems()` 并如实标「剪贴板」。

### 2. 来源标签与降级说明放在哪一层

`MiniTranslate` 拥有文案（`sourceLabel` / `fallbackHint` 两个纯函数），`TransPopoverApp` 复用来渲染
「无可译内容」态——那里也需要说明「为什么没取到字」。把文案集中在组件内、再导出纯函数，避免两处
各写一份中文后漂移。

### 3. AX 消息超时 0.3s，设在 system-wide 与焦点元素两条连接上

热键回调跑在主线程；目标应用无响应时 AX 默认会阻塞数秒，把热键和整个 UI 拖住。超时按 AX 连接生效，
system-wide 与目标应用是两条不同连接，只设前者挡不住对目标应用的阻塞（真正容易卡住的就是后者），
故两条都设。设不上超时只记日志、继续读——降级成系统默认超时，总比让热键失灵好。

### 4. 与设计文档 §2 验收的一处口径差异（按 task-1 执行）

设计 §2 写「选中为空时，浮窗给出『请先选中文字』提示，不误译剪贴板」；task-1 与 D4 的组合口径是
「无选中 → 回退剪贴板并标『剪贴板』，两者都空才提示」。本次按 **task-1** 实现：无选中时回退剪贴板，
但因为浮窗显式标了来源，不会产生「误认为译了选中文字」的认知错误；真正空态（选中与剪贴板都无内容）
才给「请先选中或复制文字再按 ⌘⇧T」。若后续要严格按设计 §2 的字面语义（无选中时不回退），
改动点只有 `build_trans_source` 一个纯函数。

## 验证实跑（本机，2026-10-31）

| 项 | 命令 | 结果 |
|---|---|---|
| Rust 全量测试 | `cd src-tauri && cargo test` | 37 suites / **909 passed / 0 failed** |
| 新增模块单测 | `cargo test --lib selection`、`cargo test --lib trans_popover` | 9 passed（classify 6 + build 5 + store 1 + 序列化 1，含既有 popover 用例） |
| Rust 静态检查 | `cargo clippy --all-targets -- -D warnings` | 干净，退出 0 |
| Rust 格式 | `cargo fmt --check` | 退出 0 |
| 前端类型 | `npx tsc -b` | 退出 0 |
| 前端全量测试 | `npx vitest run` | 84 files / **929 passed** |
| 本次相关前端子集 | `npx vitest run src/trans-popover src/ipc` | 8 files / **104 passed** |

## 未验证项（如实说明）

- **真机 AX 取词未验**：headless 环境无法运行 `npx tauri dev`，也无法触发全局热键。Safari / Notes /
  TextEdit / Terminal / Chrome 网页 / VS Code / Electron 里「选中后按热键译的是选中文字」只有真机能证，
  已登记 `TP0-M01`。
- **撤销辅助功能授权后的降级未验**：分支逻辑已单测锚定（Untrusted → fallbackReason → 文案），
  但真实 TCC 授权撤销的进程内行为需真机，已登记 `TP0-M02`。
- **多屏 / 全屏 Space 未验**：取词与显示器无关，浮窗落点与来源标签观感需真机，已登记 `TP0-M03`。
- 前端测试跑动时 `TransPopoverApp` 会打印若干 `not wrapped in act(...)` 警告：来自 blur 监听在测试里
  同步 setState 的既有测试写法，断言全绿，本次未改动该测试基础设施。

## 独立审查与修复（2026-10-31）

独立审查者（`translate-p0-review`，非实现者）给出 **BLOCK**，唯一的阻断项是：全局热键插件对按下与松开各调用一次回调，原实现两次都跑取词。第二次取词发生在本进程已激活之后，焦点不在源应用，读到空选区并回退剪贴板，覆盖掉正确的「选中」快照。

**已核实**：`tauri-plugin-global-shortcut-2.3.2` 的分发代码对每个 `GlobalHotKeyEvent` 都调用 handler；`global-hotkey-0.8.0` 的事件带 `state: HotKeyState`。

**修复**：
- `src-tauri/src/lib.rs` `register_action_shortcut`：三个动作的回调都只在 `event.state == ShortcutState::Pressed` 时执行。
- 审查第 3 条：`selection.rs` 每个 unsafe 块补 `// SAFETY:`。
- 审查第 5 条：`copy_attribute` 在 `AXUIElementCopyAttributeValue` 失败时记录错误码（只含属性名与错误码，不含文本）。授权失效与超时在用户侧都表现为「没有选中」，日志是唯一区分手段。
- 审查第 4 条：超时注释改为说明它是进程全局副作用。

**未在本次完成（已记入设计文档待办）**：
- 审查第 2 条：QuickQuick 自身为前台时取词会等待本进程主线程，最坏约 0.9 秒。计划：前台是自己时跳过取词。
- 审查第 6 条：`TransSourceStore` 保留最近一次选中文字，无清理时机。计划：补读成功或窗口隐藏后清掉。

**设计偏差处理**：审查第 6 条指出设计文档 §2 与 D3/D4 互相矛盾（「选中为空不误译剪贴板」vs「回退剪贴板并标来源」）。判定为非 BLOCK，按回退并标注来源执行，已在 `docs/design/translate-interaction-alignment.md` 第 5 节澄清。

**修复后实跑**：
- `cargo fmt`（作用于整个 crate）后 `cargo fmt --check` 干净。`git status` 显示文件集合没有新增无关文件；但 fmt 可能改写了其他已有改动文件的格式，未逐文件核对，提交前需 review `git diff` 确认。
- `cargo clippy --all-targets -- -D warnings`：退出 0。
- `cargo test --lib`：574 passed / 0 failed。
- 前端未改动，沿用上一轮 `vitest` 104 passed 与 `tsc -b` 退出 0 的结果。

**仍未验证**：真机热键行为（按下/松开只触发一次）需要真机确认，已在 pending-manual 的 TP0-M01 覆盖选中场景，建议补一条「按住热键观察是否只弹一次」的手测项。

## 收尾：两项待办落地（2026-10-31）

- **焦点应用是本进程时跳过取词**（`selection.rs`）：取词改为先经 `AXFocusedApplication` 拿焦点应用，用 `AXUIElementGetPid` 比对本进程 pid，相同则放弃，不向本进程发 AX 请求。原因：热键回调在主线程上，对本进程发 AX 会等主线程自己，同步阻塞到 0.3 秒超时。`AXUIElementGetPid` 只读元素自带的 pid，不发消息，本身不阻塞。
- **失焦隐藏时清空来源快照**（`popover.rs` `register_focus_lost_hide` + `ipc/trans_popover.rs` `TransSourceStore::clear`）：选中的文字不在窗口关闭后继续留在内存。挂载补读发生在用户看到窗口之前，不受清空影响。新增单测 `store_clear_drops_snapshot`。

**实跑**：`cargo fmt --check` 退出 0；`cargo clippy --all-targets -- -D warnings` 退出 0；`cargo test --lib` 575 passed / 0 failed。

**未验证**：焦点应用分支与 Pressed 过滤依赖真实 AX 与全局热键，无法在单测中模拟，已写入 pending-manual 的 TP0-M04（真机）。非 macOS 的编译未在本机验证（本次改动中 popover.rs 的清空逻辑与平台无关，但未交叉编译确认）。
