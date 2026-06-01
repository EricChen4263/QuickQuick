---
id: V5-F2-S07-review
type: review
level: 小功能
parent: V5-F2
children: []
created: 2026-06-02T04:30:00Z
status: 未过
commit: WIP
acceptance_ids: []
author: code-reviewer
---

# 审查结论 · 里程碑3 后端 IPC + 设置接真

## 审查范围

| 文件 | 类型 |
|---|---|
| `src-tauri/src/privacy.rs` | Rust 新增 |
| `src-tauri/src/settings.rs` | Rust 新增 |
| `src-tauri/src/lib.rs` | Rust 改造 |
| `src-tauri/src/ipc/settings.rs` | Rust 新增 |
| `src-tauri/src/ipc/system.rs` | Rust 新增 |
| `src-tauri/src/ipc/mod.rs` | Rust 改造 |
| `src-tauri/src/autostart.rs` | Rust 新增 |
| `src/ipc/ipc-client.ts` | TS 扩展 |
| `src/theme/themeStore.ts` | TS 新增 |
| `src/panels/settings/useGeneralSettings.ts` | TS 新增 |
| `src/panels/settings/PrivacyPanel.tsx` | TSX 新增 |
| `src/panels/settings/StoragePanel.tsx` | TSX 新增 |
| `src/panels/settings/SettingsPage.tsx` | TSX 新增 |
| `src/panels/clipboard/ClipboardPage.tsx` | TSX 改造 |

审查标准：项目规范 + code-standards（格式/命名/函数/注释/类型/性能/测试/安全）。

---

## 发现问题

### Critical

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| **Critical** | `ExcludeList` 运行时永不生效：`start_clipboard_poll` 在线程启动时创建 `ExcludeList::default()`（空名单），此后无论 `set_exclude_list` IPC 如何写 settings.json，轮询线程的 `exclude` 对象终生为空，排除名单要重启才生效。用户在 UI 添加 1Password 到排除名单后，当前运行中的 app 仍会继续记录来自该 app 的剪贴板内容，与隐私承诺直接冲突。 | `src-tauri/src/lib.rs:193` | 错误处理完整性 + 功能正确性红线。**修复方案**：将 `ExcludeList` 提升为共享运行时状态，类似 `paused`/`skip_sensitive` 的 `Arc<AtomicBool>` 模式：① 在 `CaptureState` 加一个 `Arc<Mutex<Vec<String>>> excluded_apps`；② `start_clipboard_poll` 从该 Arc 读取，每次循环重建 `ExcludeList`；③ `set_exclude_list` 命令在写文件后同时更新该 Arc；或者用更轻量的 `Arc<RwLock<HashSet<String>>>` 直接持有集合（避免 ExcludeList 重建开销）。**置信度：95** |

### Important

| 严重度 | 问题 | 文件:行 | 规范依据 / 修复建议 |
|---|---|---|---|
| **Important** | `themeStore.ts` 的 `init()` 函数第 87 行对 `typeof window === "undefined"` 做了早返回，第 91 行仍重复 `if (typeof window !== "undefined" && window.matchMedia)` 检查——在 Tauri（始终有 window）生产环境无害，但在 SSR/Node 测试环境第一次检查就已 return，永不到达第 91 行，该条件永远是死代码。且第 91 行条件若删掉 `typeof window !== "undefined"` 部分（即直接 `if (window.matchMedia)`）逻辑更清晰，删掉后依靠第 87 行的早返回已足够。虽不影响运行，但是冗余防御逻辑，读者需二次推理才能确认安全。 | `src/theme/themeStore.ts:91` | code-standards §3 单一职责、可读性优先。**修复**：删除第 91 行中的 `typeof window !== "undefined" &&`，保留 `window.matchMedia` 判断即可。**置信度：82** |

---

## 低于阈值的观察（不阻断，备忘）

**settings.json 无 Mutex 保护的 read-modify-write 竞态**（置信度约 70%）

`set_pause_capture`、`set_skip_sensitive`、`set_stay_in_tray` 等每个命令都走 `load_or_default → 修改单字段 → save` 的三步序列，各命令不共享文件级锁。Tauri 非 async 命令运行在线程池，若两个 set 命令并发执行（如前端同时触发 pause + skip_sensitive），先 load 的那个可能在另一个已写入后再 save，导致另一个写入丢失。实际触发概率极低（用户极少同时点两个开关），不阻断。

**StoragePanel `handleCleanup` 中 cancelled 对象生命周期与组件卸载解耦**（置信度约 65%）

`handleCleanup`（第 46-47 行）在 handler 内部创建 `const cancelled = { current: false }`，该对象不与 useEffect cleanup 挂钩，若用户在点击清理后立即导航走，`fetchStats` resolve 后仍会写 state（React 18 已弱化此警告，不会崩溃）。该模式在 `ClipboardPage` 的事件处理器中同样存在，属于既有设计，不新增于本次。不阻断。

**`ipc-client.ts` 中 `getTheme`/`setTheme` 的返回类型为裸 `string` 而非 `ThemePref` 字面量联合**（置信度约 60%）

`getTheme(): Promise<string>` 而非 `Promise<"auto" | "light" | "dark">`，调用方（themeStore）需手动校验，但 themeStore 已在 `hydrateFromIpc` 中做了三值校验（第 72 行），安全。收窄类型会更好，但属于改进而非 bug。不阻断。

---

## 专项核查结论

### 1. 并发安全：`Arc<AtomicBool>` Ordering 用法

所有 `AtomicBool` 均使用 `Ordering::Relaxed`：轮询线程读（`paused.load`、`skip_sensitive.load`），IPC 命令写（`.store`），失焦事件处理读（`stay_in_tray.load`）。

`Relaxed` 在此场景合理：
- 三个布尔量之间无因果依赖（不需要 acquire/release 来建立 happens-before）。
- 轮询线程每 500ms 读一次，最坏情况是本轮使用旧值，下一轮（500ms 内）自动更新；对于"暂停捕获"这类软状态，延迟 500ms 可接受。
- 失焦处理的 `stay_in_tray.load(Relaxed)` 读到稍旧值的场景：IPC 刚写入、窗口立即失焦，若 Relaxed 读到旧值（true），窗口被隐藏而非退出——结果偏安全，不会意外退出。

**结论：通过。**

Arc clone 模式（`Arc::clone` 在 setup 完成后、`manage` 之前，lib.rs 第 120-124 行）正确：轮询线程和失焦处理各自持有 clone，与 `CaptureState`（managed 后仅通过 `state()` 访问）不共享同一 Arc 引用，符合 Tauri 状态生命周期要求。**通过。**

### 2. serde 向前兼容

`AppSettings` 所有新增字段（`pause_capture`、`skip_sensitive`、`stay_in_tray`、`auto_update`、`theme`）均标注 `#[serde(default)]` 或 `#[serde(default = "fn")]`，旧 settings.json 仅含 `excluded_apps`/`selected_provider` 时反序列化不 Err，且有单测 `legacy_json_missing_new_fields_uses_defaults` 覆盖（settings.rs 第 127 行）。

`Default` impl 与 serde default 函数值一致，round-trip 测试通过。**serde 兼容性：通过，旧数据安全。**

### 3. 命令注册完整性

`lib.rs` invoke_handler 中注册命令清点：

| 域 | 命令 |
|---|---|
| clipboard | list_clip_items, delete_clip_item, toggle_favorite_clip, get_clip_image_original |
| translate | translate_text, list_translate_history |
| settings | get_hotkeys, set_hotkey, get_exclude_list, set_exclude_list, get_translate_providers, get_selected_provider, set_selected_provider, get_pause_capture, set_pause_capture, get_skip_sensitive, set_skip_sensitive, get_stay_in_tray, set_stay_in_tray, get_auto_update, set_auto_update, get_theme, set_theme, get_launch_on_login, set_launch_on_login |
| system | get_storage_stats, cleanup_history, open_accessibility_settings, paste_to_front |

`ipc/settings.rs` 中声明了 19 个 `#[tauri::command]`，lib.rs 全部注册。`ipc/system.rs` 中 4 个命令全部注册。**命令注册：完整，无遗漏。**

### 4. 前端接线核查

- **cancelled ref 防卸载**：`useGeneralSettings`、`PrivacyPanel`、`StoragePanel`、`ClipboardPage` 均在 `useEffect` 内以 `{ current: false }` 对象传入异步函数，cleanup 置 `true`，async resolve 时有 `if (cancelled.current) return` guard。**通过。**
- **错误处理**：所有 IPC 调用有 `try/catch`，UI 显示中文友好提示，不暴露内部错误细节。**通过。**
- **`useGeneralSettings` 接口零改动**：对外暴露 `{ launchOnLogin, stayInTray, autoUpdate, setLaunchOnLogin, setStayInTray, setAutoUpdate }`，GeneralPanel 直接解构使用，无破坏性改动。**通过。**
- **themeStore 竞争防御**：`hydrateFromIpc` 记录调用前 pref 快照，IPC 返回后比对当前 pref，用户期间手动改则丢弃 IPC 结果（第 74 行）。逻辑正确，有测试覆盖。**通过。**
- **setter async 化影响调用方**：`useGeneralSettings` 的三个 setter 为 `async (v) => Promise<void>`，SettingToggle 的 `onChange` 签名为 `(checked: boolean) => void`。TypeScript 允许 `Promise<void>` 赋值给 `void` 回调（结构化子类型），Promise 静默丢弃。无运行时错误，TS 编译通过。若需要明确 UI 加载态，未来可包一层 `() => { void setter(v); }` 并加 pending state，但当前实现在语言层面合规。**通过（现有实现可接受，无类型错误）。**
- **paste 降级实现**：`paste_to_front_impl` 从 DB 按参数化查询取文本，写回 arboard；图片类型显式返回 `Err`；`id` 空值有前置 guard；outcome 字段用 `"write_back_only"` 字面量，前端 `PasteOutcome` 类型匹配。**通过。**

### 5. 错误处理 / SQL 参数化

生产代码无 `unwrap()`/`expect()`/`panic!`（测试辅助除外），全部 `?` 传播。`paste_to_front_impl` 的 SQL 使用 `rusqlite::params![id]` 参数化，无字符串拼接注入风险。错误消息为中文描述，不暴露内部路径或堆栈。**通过。**

---

## 逐维度核查（code-standards 检查清单）

| 维度 | 结论 |
|---|---|
| 格式（缩进/行宽/空行） | Rust 4 空格，TS 2 空格，项目内一致。通过。 |
| 函数长度 ≤ 50 行 | `start_clipboard_poll` 恰好 50 行（边界合规）；其余均在 50 行内。通过。 |
| 命名描述性 | Rust snake_case/PascalCase，TS camelCase/PascalCase，布尔量前缀 is/has 使用正确（`is_deleted`、`isFavorite`、`is_concealed`）。通过。 |
| 注释写为什么 | 各模块注释均描述设计意图（为何 Relaxed、为何 ExcludeList 借用而非持有、为何双轨持久化）。无装饰性横线，无 TODO/FIXME 残留。通过。 |
| 类型：无 any | TS 文件无 `any`，接口类型完整。通过。 |
| 安全：无明文密钥/SQL 注入 | SQL 参数化；密钥由 keychain 管理；错误消息不含路径/堆栈。通过。 |
| 测试质量 | AAA 结构，名称描述行为（`legacy_json_missing_new_fields_uses_defaults`、`set_pause_capture_preserves_existing_fields`、`paste_to_front_impl_empty_id_returns_err` 等）。themeStore 竞争防御测试模拟了真实 race。通过。 |

---

## 结论

**打回（必改 1 项）**

### 必改项

**M1（对应 Critical，置信度 95）**：`ExcludeList` 运行时不生效——轮询线程在启动时创建 `ExcludeList::default()` 后永不更新，`set_exclude_list` IPC 只持久化文件，对当前运行中的轮询无效。用户期望添加排除应用后立即生效，而实际需要重启，违背隐私功能承诺。

**推荐修复路径**：在 `CaptureState` 加 `excluded_apps: Arc<RwLock<HashSet<String>>>`（`src-tauri/src/lib.rs:43-50`），`init_capture_state` 从 settings 初始化，`set_exclude_list_impl` 在写文件后调用 `state.excluded_apps.write().unwrap().clear(); extend(...)` 更新运行时；轮询循环从 Arc 读取构建 `ExcludeList`（每 500ms 读一次 RwLock 开销极小）。

---

*（如复审通过，追加复审结论段并将 status 改为通过）*
