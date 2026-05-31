# S10 托盘单一来源 — 编码留痕

**Story**: V4/F3/S10  
**日期**: 2026-05-31  
**实现者**: coder agent

---

## 改动文件清单

| 文件 | 类型 | 说明 |
|------|------|------|
| `src-tauri/tauri.conf.json` | 配置删除 | 删除 `app.trayIcon` 块，消除 Tauri 自动建的第二个托盘图标 |
| `src-tauri/tests/boot_smoke.rs` | 测试新增 | 新增 `tray_single_source_no_auto_trayicon_in_conf` 守卫测试；同步更新模块头注释，补充第 3 条路径说明 |

---

## 缺陷根因与修法

**根因**：`tauri.conf.json` 中存在 `app.trayIcon` 配置块。Tauri 框架在 app 启动时检测到该字段会自动创建一个常驻托盘图标（无菜单）；而 `src-tauri/src/tray.rs` 的 `setup_tray()` 又通过 `TrayIconBuilder` 显式创建第二个带菜单的托盘图标。两条路径并存，导致 macOS 菜单栏出现两个图标。

**修法**：删除 `tauri.conf.json` 中的 `app.trayIcon` 块，使托盘完全由 `tray.rs::setup_tray()` 单一管理。`tray.rs` 内部通过 `default_window_icon()` 获取图标资源，与 conf 字段无依赖关系，删除后编译与运行均不受影响。

---

## 守卫测试设计

测试函数：`tray_single_source_no_auto_trayicon_in_conf`（位于 `src-tauri/tests/boot_smoke.rs`）

**测试策略**：与既有两个守卫相同——直接解析 `tauri.conf.json`，使用 `serde_json::Value::pointer("/app/trayIcon")` 检查路径是否存在。这条路线绕开了 mock_builder 在非主线程触发 `Tray(NotMainThread)` 错误的假绿问题。

**断言**：`tray_icon_entry.is_none()` 验具体缺失状态，非弱断言。失败时打印检测到的实际值，便于定位。

**覆盖的回归场景**：将来若有人在 conf 中重新加回 `trayIcon` 块（例如误以为需要配置图标路径），测试立即变红，阻止双图标 bug 复现。

---

## 编译与测试实跑结论

**cargo test（过滤串 `tray_single_source`）**：

```
test tray_single_source_no_auto_trayicon_in_conf ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
EXIT:0
```

**cargo build**：

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s
EXIT:0
```

boot_smoke.rs 既有两测试（`conf_json_is_valid` / `autostart_conf_deserializes_as_unit`）仍在，编译无任何 error。

---

## 假设与未决项

| 项目 | 状态 | 说明 |
|------|------|------|
| 真机单图标确认 | **待手动验证** | `cargo test` 只能守卫配置层面；macOS 菜单栏实际只出现一个图标需要 `cargo tauri dev` 运行后目视确认，已记入 `pending-manual.yaml` |
| tray.rs 图标资源路径 | 已确认无影响 | `tray.rs` 使用 `default_window_icon()` 而非 conf 字段，删除 `app.trayIcon` 不影响图标加载 |

---

## code-standards 自检

| 检查项 | 状态 |
|--------|------|
| 装饰性分隔注释（`═══`/`───`/`━━━`/`=====`） | 无（grep 无命中） |
| 函数 ≤ 50 行 / 嵌套 ≤ 3 层 | 测试函数约 15 行，嵌套 1 层 |
| 断言非恒真、非旁路 | 断言 `.is_none()` 验具体缺失，调 `conf.pointer()` 而非旁路 |
| 无 TODO / FIXME 残留 | 无 |
| 过滤测试命令真命中 | `tray_single_source` 匹配到 `tray_single_source_no_auto_trayicon_in_conf ... ok`，N=1≥1 |
| 安全 / 持久化敏感项 | 本 story 不涉及 |
| 注释写「为什么」 | 测试头注释、函数文档注释均解释根因与守卫意图 |

---

## 修订 R1（reviewer I-1）

**日期**：2026-05-31

**改动文件**：`src-tauri/src/tray.rs` — 仅改第 3、4、5 行模块头文档注释，不动任何代码逻辑。

**改动内容**：

| 行 | 改前 | 改后 |
|----|------|------|
| 3 | `//! 策略：tauri.conf.json 已声明 \`app.trayIcon\`（自动建图标+常驻托盘）。` | `//! 策略：托盘由本模块 setup_tray() 唯一构建（带右键菜单+事件回调）；` |
| 4 | `//! 本模块在 setup 阶段额外附加右键菜单与事件回调。` | `//! tauri.conf.json 不声明 app.trayIcon，避免"配置自动建 + 代码建"双图标。` |
| 5 | 不变（图标来源说明保留） | 不变 |

**为什么**：S10 已删除 tauri.conf.json 的 `app.trayIcon` 块，原注释"已声明 app.trayIcon""额外附加"均与现状矛盾，会误导后续维护者认为 conf 仍有声明。新注释准确描述单一来源策略：托盘完全由 `setup_tray()` 负责，conf 刻意不声明以防双图标。

**编译确认**：

```
cargo build --manifest-path src-tauri/Cargo.toml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.50s
EXIT:0
```

无 error，无 warning，编译通过。
