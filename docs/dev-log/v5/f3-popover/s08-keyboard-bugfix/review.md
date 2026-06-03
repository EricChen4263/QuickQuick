---
id: V5-F3-S08-review
type: review
level: 小功能
parent: V5-F3-popover
children: []
created: 2026-06-03T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: [clip-popover-keyboard-full-fix]
evidence: []
author: code-reviewer
---

# 审查结论 · clip-popover 键盘全失效 bug 修复

## 审查范围

| 文件 | 改动性质 |
|---|---|
| `src-tauri/src/popover.rs` | 新增 `activate_app_macos`（NSApplication.activate 显式激活） |
| `src-tauri/src/ipc/system.rs` | `hide_and_restore_focus` 拆分 + 新增 `hide_app`（app.hide 还焦） |
| `src-tauri/src/lib.rs` | `setup_main_window_behavior` match 重构 + `exclude: &*` clippy 修复 |
| `src-tauri/Cargo.toml` | objc2-app-kit features 新增 NSApplication/NSResponder |
| `src-tauri/capabilities/default.json` | windows 加入 clip-popover/trans-popover + 授 hide/show/set-focus |
| `src/clip-popover/ClipPopoverApp.tsx` | inputRef + onFocusChanged 聚焦重置 + Esc 分支 + type=text |
| `src/clip-popover/clip-popover-actions.test.tsx` | 新增 onFocusChanged 3 用例 + Esc 用例 |

**忽略（纯 rustfmt 格式重排，无逻辑）**：`db.rs`、`ipc/settings.rs`、`ipc/translate.rs`、`macos_paste.rs`、`translate/lang.rs`、`tests/*`。

## 审查维度

对照 `code-standards` §1–12 + 项目规范逐条核查：

### §1 通用原则

- `activate_app_macos`：单一职责，仅激活 app；错误路径优雅降级（eprintln + return），无 panic。合规。
- `hide_and_restore_focus` → `hide_popover_window` + `hide_app` 拆分：职责清晰，各函数 < 25 行，符合 ≤50 行规则。合规。
- `setup_main_window_behavior` 的 match 重构：语义与旧 if-let 链等价（见分析），是纯重构无行为变化。合规。

### §2 格式

- Rust 文件：`cargo fmt --all` 已整理格式，改动符合 rustfmt 标准。合规。
- TypeScript 文件：2 空格缩进，分号一致，行宽合规。合规。

### §3 函数长度 / 嵌套

- `hide_popover_window`（20 行）、`hide_app`（5 行）、`activate_app_macos`（10 行）均 < 50 行，嵌套 ≤ 2 层。合规。
- `handleKeyDown`（44 行）< 50 行，嵌套 ≤ 2 层。合规。

### §4 命名

- `activate_app_macos`、`hide_popover_window`、`hide_app`、`hide_and_restore_focus`：均为「动词+名词」，描述性强。合规。
- `inputRef`、`visibleFlatListRef`：明确描述用途。合规。

### §5 注释质量

- `activate_app_macos` 的 doc 注释完整解释「为什么」：tao/tauri `set_focus` 底层用废弃的 `activateIgnoringOtherApps:YES`，macOS 14+ no-op，需显式 `NSApplication.activate()`。注释质量高。
- `hide_and_restore_focus` 注释解释 `app.hide()` 替代原 `activateWithOptions(empty)` 的理由，准确。
- `onFocusChanged` useEffect 的内联注释说明 stale closure 规避思路（ref 模式）。合规。
- 无注释掉的死代码，无装饰性分隔符。合规。

### §6 类型

- `inputRef: useRef<HTMLInputElement>(null)`，`visibleFlatListRef: useRef<ClipItem[]>([])`：显式类型，无 `any`。合规。
- `unlisten: (() => void) | null`：精确类型，无逃逸。合规。

### §7 性能

- `visibleFlatListRef.current = visibleFlatList` 写在 render 函数体（非 useEffect），为渲染期同步更新 ref，符合 React ref 惯用法，无性能问题。合规。

### §8 测试

- 新增 Esc 测试（`按 Esc 调 hide 关闭窗口`）：Arrange/Act/Assert 结构清晰，命名描述行为。合规。
- 新增 onFocusChanged 三用例（输入框聚焦、query 重置、selectedId 重置）：均通过 `capturedFocusCallback!({ payload: true })` 直接触发，测试新行为核心路径。合规。
- 测试以 `mockUnlisten` mock 返回值，可验证 unlisten 被调用。合规。

### §9 提交规范

- 改动尚未 commit（WIP），现有提交命名符合 Conventional Commits 格式。

### §10 安全

- 无密钥硬编码。
- `NSApplication.activate()` 不涉及敏感数据。合规。

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| — | — | — | 未发现置信度 ≥ 80 的问题 |

## 重点核查细节（有深度分析但低于阈值）

以下问题经仔细分析后置信度均低于 80，列出作为参考，不要求修复：

**1. `onFocusChanged` unlisten 竞态（置信度 72）**

`src/clip-popover/ClipPopoverApp.tsx:58-75`：useEffect cleanup 中 `unlisten` 可能为 `null`（Promise 尚未 resolve 时组件已卸载，如 React StrictMode dev 模式双调用）。项目内 `ClipboardPage.tsx` 和 `App.tsx` 均使用 `cancelled` 双重保险 pattern，此处未遵循。

**实际危害极低**：clip-popover 作为懒建常驻浮层，生命周期等于应用生命周期，在正常运行中几乎不卸载；StrictMode 泄漏仅在 dev 环境且 React batching 会合并重复 state 更新，UI 无感知。

如有余力可对齐项目 pattern：
```typescript
const cancelled = { current: false };
win.onFocusChanged(...).then((fn) => {
  if (cancelled.current) { fn(); } else { unlisten = fn; }
});
return () => { cancelled.current = true; unlisten?.(); };
```

**2. Esc 分支无 `return` 语句（置信度 42）**

`src/clip-popover/ClipPopoverApp.tsx:137-147`：`handleKeyDown` 的 ArrowDown/ArrowUp、Enter、Alt+Enter 分支均有显式 `return`，Esc 分支作为最后一个 `if` 没有 `return`。语义正确（函数自然结束），但风格不一致。不要求修改。

**3. `focused=false` 无负向测试（置信度 60）**

`src/clip-popover/clip-popover-actions.test.tsx` 中 onFocusChanged 三个测试均只触发 `payload: true`，未验证 `payload: false` 时 focus/reset 不被调用。该分支仅一行 `if (!focused) return;`，逻辑极简，风险极低。

## 逻辑正确性确认

| 改动点 | 分析结论 |
|---|---|
| `activate_app_macos` 线程安全 | 全局热键回调在 macOS 主线程（CGEventTap 在主 runloop），`MainThreadMarker::new()` 返回 Some 是正确假设；拿不到时优雅降级不 panic。正确。 |
| `app.hide()` 非主线程 | `paste_to_front` 是 Tauri 命令，跑在 runtime 线程池（非主线程）。`AppHandle::hide()` 内部有线程派发，非主线程调用安全。正确。 |
| `hide_and_restore_focus` sleep(100ms) 阻塞 | sleep 在 Tauri 命令线程，不阻塞主线程/UI。正确。 |
| `setup_main_window_behavior` match 重构语义 | 旧 `CloseRequested` 分支：`if stay_in_tray { prevent+hide }`（否则空，放行默认关闭）。新 guard 写法：`CloseRequested { .. } if stay_in_tray => { prevent+hide }` + `_ => {}`。两者完全等价。正确。 |
| `exclude: &exclude_guard` clippy 修复 | `RwLockReadGuard<ExcludeList>` 实现 `Deref<Target=ExcludeList>`，`&exclude_guard` 自动 coerce 到 `&ExcludeList`，与原 `&*exclude_guard` 语义完全相同，消除 `explicit_auto_deref` lint。正确。 |
| capabilities 授权最小必要性 | `allow-hide`：前端 `getCurrentWindow().hide()` 使用；`allow-show`：`TransPopoverApp` 的 `main?.show()`；`allow-set-focus`：`TransPopoverApp` 的 `main?.setFocus()`。三项均有对应用途，无通配符，最小必要。正确。 |
| objc2-app-kit NSResponder feature | NSApplication 继承 NSResponder，代码编译通过且 GUI 实测有效。合理。 |
| WKWebView 吞 Esc 不冒泡 + 双层处理 | 注释解释了 `type=search` 时 WKWebView 原生拦截 Esc 导致不冒泡；改 `type=text` 后 onKeyDown 分支是主路径，`main.tsx` document 监听作为无 input 焦点时的兜底，两层互补不 double-hide（WKWebView 不让 Esc 冒泡到 document）。架构合理。 |

## 是否合规

改动符合 `code-standards` §1–12 的所有强制项：

- 格式：符合 rustfmt + 2 空格 TypeScript 规范。
- 命名：描述性动词+名词命名，无魔术字符串（`eprintln` 前缀字符串用于调试定位，可接受）。
- 函数长度与嵌套：均达标。
- 注释：高质量「为什么」注释，无死代码残留。
- 类型安全：无 `any`，公共接口有类型声明。
- 错误处理：所有可失败操作均 graceful 降级（eprintln + return/continue），无裸 panic。
- 测试：新行为均有测试覆盖（Esc 关窗、获焦重置三场景），AAA 结构，行为化命名。
- 安全：无密钥，无用户输入直接执行，无日志泄密。

## 结论

**通过**。

四个 bug（#1 键盘进不了 webview / #2 输入框无焦点 / #3a Esc 不关窗 / #3b 粘贴后焦点未还原）的修复方案准确对症，代码质量符合规范，关键路径逻辑验证正确，未发现置信度 ≥ 80 的强制修复问题。
