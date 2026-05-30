---
id: V0-F1-S01-test
type: test_report
level: 小功能
parent: V0-F1
children: []
created: 2026-05-30T20:46:00Z
status: 通过
commit: WIP
acceptance_ids: [V0-F1-A01, V0-F1-A02, V0-F1-A03, V0-F1-A04, V0-F1-A06]
evidence: [/tmp/t_build.log, /tmp/t_febuild.log, /tmp/t_clippy.log, /tmp/t_tsc.log, /tmp/t_betest.log, /tmp/t_fetest.log]
author: tester
---

# 测试报告 · V0-F1-S01 Tauri 脚手架初始化

## 运行的测试命令

```bash
# A01 后端构建
cargo build --manifest-path src-tauri/Cargo.toml > /tmp/t_build.log 2>&1

# A02 前端构建
pnpm build > /tmp/t_febuild.log 2>&1

# A03a cargo clippy 工程质量
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings > /tmp/t_clippy.log 2>&1

# A03b tsc 类型检查
pnpm exec tsc --noEmit > /tmp/t_tsc.log 2>&1

# A03c 无遗留 TODO/FIXME
grep -rn 'TODO\|FIXME' src src-tauri/src

# A04a 后端单测
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/t_betest.log 2>&1

# A04b 前端 vitest
pnpm test > /tmp/t_fetest.log 2>&1

# A06 updater 插件接入检查
grep -q 'updater' src-tauri/tauri.conf.json
```

## 结果

**全部通过**

## 用例清单 + 结果

| 用例 | 结果 | 对应验收项 |
|---|---|---|
| cargo build（Rust 后端编译） | pass | V0-F1-A01 |
| pnpm build（React Vite 打包） | pass | V0-F1-A02 |
| cargo clippy -D warnings（无 Lint 警告） | pass | V0-F1-A03 |
| tsc --noEmit（TypeScript 类型检查） | pass | V0-F1-A03 |
| grep TODO/FIXME（无遗留标记，exit=1 即通过） | pass | V0-F1-A03 |
| tests::smoke_lib_loads（Rust 后端冒烟） | pass | V0-F1-A04 |
| smoke: 前端模块可加载（vitest） | pass | V0-F1-A04 |
| window_route_by_hotkey: CmdOrCtrl+Shift+V 路由到 history 视图 | pass | V0-F1-A04 |
| window_route_by_hotkey: CmdOrCtrl+Shift+T 路由到 translate 视图 | pass | V0-F1-A04 |
| window_route_by_hotkey: 相同热键连续触发保持同一视图不切换 | pass | V0-F1-A04 |
| window_route_by_hotkey: 从 history 切换到 translate 视图 | pass | V0-F1-A04 |
| grep updater in tauri.conf.json（updater 段存在） | pass | V0-F1-A06 |

后端用例：1 passed（tests::smoke_lib_loads）
前端用例：5 passed（1 smoke + 4 windowRoute）

## 覆盖率

**已覆盖：**
- A01：cargo build 构建通过，Rust 后端无编译错误
- A02：pnpm build 通过，Vite 产物生成于 dist/，gzip 后 45.88 kB
- A03：clippy 无警告、tsc 无类型错误、源码无 TODO/FIXME 残留
- A04：后端至少 1 例冒烟（smoke_lib_loads），前端 vitest 5 例全绿
- A06：tauri.conf.json 含 updater 配置段

**未覆盖（本小功能范围外，属后续小功能）：**
- V0-F1-A05（autostart 插件，证据路径 s04-autostart/test.md）
- V0-F2 系列（托盘/热键/预热窗口实际运行时行为，windowRoute 单测已通过但 GUI 层待 A04 人工确认）
- V0-F3 系列（加密数据库、KeyProvider、schema 等，待 s05/s06 小功能实现）
- V0-A-LOG（留痕三联完整性，本 test.md 产出后 s01 三联完成，后续大功能报告待 coding/review 补齐）

## 失败项详情

无失败项。

## 结论

**放行——允许进入下一步。**

V0-F1-S01 覆盖的 5 个验收项（A01/A02/A03/A04/A06）全部客观验证通过，后端 1 例冒烟 + 前端 5 例 vitest 均绿，clippy/tsc/TODO 三项工程质量基线达标，updater 插件接入确认存在。
