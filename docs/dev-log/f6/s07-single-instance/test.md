# test.md — f6/s07 单实例保护动态证伪报告

**日期**：2026-06-04  
**被测改动**：`tauri-plugin-single-instance` 接入（`lib.rs` + `tray.rs` + `tests/single_instance.rs`）

---

## 0. 开工 / 收工快照（工作树完整性）

开工 `git status --porcelain`：
```
 M src-tauri/Cargo.lock
 M src-tauri/Cargo.toml
 M src-tauri/src/lib.rs
 M src-tauri/src/tray.rs
?? src-tauri/tests/single_instance.rs
```

收工（变异全部还原后）：同上，逐行一致，无新增/丢失。

---

## 1. 命中校验

**命令**：`cargo test --test single_instance`

**结果**：
```
running 1 test
test single_instance_init_accepts_app_argv_cwd_callback ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- passed = 1，filtered out = 0，**非空匹配，无假绿**。

**该测试的证伪力评价**：

- **能抓住什么**：
  1. 依赖不存在（`Cargo.toml` 漏写或版本错）→ 编译失败（红）。
  2. 插件 `init` 回调签名漂移（如插件升级改了参数列表）→ 编译失败（红）。
  3. 返回类型不再是 `TauriPlugin<Wry>` → 编译失败（红）。
  4. 插件 `name()` 返回空字符串 → `assert!(!name.is_empty())` 运行期失败（红）。

- **抓不住什么**（已知局限，符合设计意图）：
  1. 多实例互斥的核心行为——依赖真实多进程 + OS 级具名锁，无法用单进程单测验证，需手动启动两个进程验证。
  2. `show_and_focus_window` 在第二实例触发时的实际表现（窗口是否真正显示并聚焦）——同上，依赖 GUI 运行时。
  3. `#[cfg(desktop)]` 在 mobile 目标下是否正确排除——需要交叉编译目标验证（本机无 mobile 目标，不强制）。

**结论**：该测试是诚实的编译期契约守卫，对签名漂移有效，对运行期多进程行为无能为力。作为"多实例行为不可单测"场景下的最优覆盖，证伪力定性评分：**中（契约有效，行为无法覆盖）**。

---

## 2. 变异 sanity

### 变异 1：lib.rs 回调改调不存在的函数

- **改动**：`tray::show_and_focus_window(app)` → `tray::nonexistent_function_xyz(app)`（lib.rs 第 121 行）
- **备份/还原**：`cp lib.rs /tmp/lib.rs.bak` → 改坏 → `cargo build` → `cp /tmp/lib.rs.bak lib.rs`
- **期望**：编译失败
- **实际**：
  ```
  error[E0425]: cannot find function `nonexistent_function_xyz` in module `tray`
  error: could not compile `quickquick` (lib) due to 1 previous error
  ```
- **结论**：如期变红。证明 `#[cfg(desktop)]` 守卫下回调真实参与编译、函数连通性真实有效。

### 变异 2：single_instance.rs 测试回调改成错误参数数量

- **改动**：`tauri_plugin_single_instance::init(|_app, _argv, _cwd| {})` → `tauri_plugin_single_instance::init(|_app| {})`
- **备份/还原**：`cp single_instance.rs /tmp/single_instance_test.rs.bak` → 改坏 → `cargo test --test single_instance` → `cp /tmp/single_instance_test.rs.bak single_instance.rs`
- **期望**：编译失败
- **实际**：
  ```
  error[E0593]: closure is expected to take 3 arguments, but it takes 1 argument
  error: could not compile `quickquick` (test "single_instance") due to 1 previous error
  ```
- **结论**：如期变红。证明测试的签名约束是真实有效的，不是恒真测试。

**还原验证**：两次变异均从 `/tmp/*.bak` 完整复原，收工 git 快照与开工逐行一致，无残留。

---

## 3. 全量回归（连跑 3 次）

**命令**：`cargo test`（全量套件）

| 轮次 | passed | failed | 备注 |
|------|--------|--------|------|
| Run 1 | **351** | 0 | 全绿 |
| Run 2 | **351** | 0 | 全绿 |
| Run 3 | **351** | 0 | 全绿 |

- 原有 351 passed 全部保留，无回归。
- 新增 `single_instance_init_accepts_app_argv_cwd_callback` 计入 351 中（整合测试套件包含）。
- `register_plugins` 函数本身无具名单测，通过全量编译通过作为隐性验证（doc 注释明确说明其测试策略：泛型函数靠编译期保证，MockRuntime 路径由其他集成测试覆盖）。

---

## 4. 平台守卫探测

`#[cfg(desktop)]` 守卫的效果：
- 默认 `cargo test`（macOS desktop）：`single-instance` 插件注册代码参与编译，测试全绿（已验证）。
- Mobile 目标：本机无交叉编译目标，无法直接验证；但 `#[cfg(desktop)]` 是 Tauri 官方守卫，mobile 编译路径不会引用该 crate，不会因缺失 OS 锁实现而报错——此为已知设计，不算缺陷。

---

## 5. 诚实结论：自动化边界

| 验证项 | 可自动化？ | 方式 |
|--------|-----------|------|
| 依赖接线（Cargo.toml + 编译） | 是 | `cargo build` / `cargo test` |
| 回调签名契约 | 是 | 编译期类型检查（本测试） |
| `show_and_focus_window` pub(crate) 可见性 | 是 | 编译期（`lib.rs` 内 run() 直接引用） |
| 多实例互斥（第二实例被拒） | **否** | 需手动启动两个进程 |
| 已运行实例窗口被显示/聚焦 | **否** | 需 GUI + 多进程手动验证 |
| Mobile 目标无编译错误 | 否（本机无目标） | 需交叉编译环境 |

---

## 6. 门禁结论

**PASS**

理由：
1. 命中校验：1 passed，0 filtered out，无假绿。
2. 变异 sanity：2 处变异均如期变红（E0425 + E0593），证明测试真有判别力、回调真实连通。
3. 全量回归：3 次全量跑 351 passed，0 failed，稳定无 flaky。
4. 平台守卫：desktop 下编译运行正常；多实例行为不可单测属已知且可接受。
