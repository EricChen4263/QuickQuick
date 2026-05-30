---
id: V0-F2-S03-test
type: test_report
level: 小功能
parent: V0-F2
created: 2026-05-30T21:22:40Z
status: 通过
commit: WIP
acceptance_ids: [V0-F2-A03, V0-F2-A04, V0-F2-A05]
author: tester
---

# 测试报告 · V0-F2-S03 预热窗口 + 托盘 + 全局热键

## 运行的测试命令

```bash
# A03：windowRoute 路由逻辑单测（4 例）
pnpm test windowRoute > /tmp/T3a3.log 2>&1
# 结论行：grep -E 'Tests ' /tmp/T3a3.log | tail -1

# A05：图标资源文件存在性验证
(test -d src-tauri/icons && ls src-tauri/icons/*.png src-tauri/icons/*.ico src-tauri/icons/*.icns) > /tmp/T3a5.log 2>&1

# 工程基线 - Rust 后端构建
cargo build --manifest-path src-tauri/Cargo.toml > /tmp/T3b.log 2>&1

# 工程基线 - clippy（-D warnings 严格模式）
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings > /tmp/T3c.log 2>&1

# 工程基线 - Rust 单测（含 window_pos 定位计算单测）
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/T3ct.log 2>&1

# 工程基线 - TypeScript 类型检查
pnpm exec tsc --noEmit > /tmp/T3t.log 2>&1

# 工程基线 - 前端 Vite 构建
pnpm build > /tmp/T3fb.log 2>&1

# 工程基线 - 无遗留 TODO/FIXME（exit=1 即通过，grep 无输出）
grep -rn 'TODO\|FIXME' src src-tauri/src; echo "todo_exit=$?"
```

## 结果

**全部客观项通过（A03 pass / A05 pass / 工程基线全绿）**
A04 为 manual_confirm，headless 无法自动验证，标"待人工 GUI 确认"，不阻塞放行。

## 用例清单

| 用例 | 结果 | 对应验收项 | 备注 |
|---|---|---|---|
| window_route_by_hotkey: CmdOrCtrl+Shift+V 路由到 history 视图 | pass | V0-F2-A03 | |
| window_route_by_hotkey: CmdOrCtrl+Shift+T 路由到 translate 视图 | pass | V0-F2-A03 | |
| window_route_by_hotkey: 相同热键连续触发保持同一视图不切换 | pass | V0-F2-A03 | |
| window_route_by_hotkey: 从 history 切换到 translate 视图 | pass | V0-F2-A03 | |
| src-tauri/icons/*.png 存在（含 128x128、64x64 等多尺寸） | pass | V0-F2-A05 | 17 个 PNG 文件 |
| src-tauri/icons/*.ico 存在（icon.ico） | pass | V0-F2-A05 | |
| src-tauri/icons/*.icns 存在（icon.icns） | pass | V0-F2-A05 | |
| cargo build（Rust 后端编译） | pass | 工程基线 | finished in 1.38s |
| cargo clippy -D warnings（无 lint 警告） | pass | 工程基线 | |
| pnpm exec tsc --noEmit（TypeScript 无类型错误） | pass | 工程基线 | |
| pnpm build（Vite 前端打包） | pass | 工程基线 | dist/assets/ gzip 49.79 kB |
| grep TODO/FIXME（无遗留标记，exit=1 即通过） | pass | 工程基线 | grep 无匹配行 |
| window_pos::tests::center_top_x_is_centered | pass | 工程基线（window_pos 单测） | |
| window_pos::tests::center_top_y_is_fifteen_percent | pass | 工程基线（window_pos 单测） | |
| window_pos::tests::center_top_accounts_for_monitor_offset | pass | 工程基线（window_pos 单测） | |
| window_pos::tests::find_monitor_at_no_monitors_returns_none | pass | 工程基线（window_pos 单测） | |
| tests::smoke_lib_loads | pass | 工程基线（Rust 冒烟） | |
| tests::hotkey_defaults_match_spec | pass | 工程基线（hotkey 单测） | |
| tests::hotkey_defaults_and_rebind（integration test） | pass | 工程基线（hotkey 集成测试） | |
| tests::hotkey_conflict_rejected（integration test） | pass | 工程基线（hotkey 集成测试） | |
| tests::hotkey_rebind_translate_isolates_field（integration test） | pass | 工程基线（hotkey 集成测试） | |
| 托盘图标常驻显示 | manual | V0-F2-A04 | headless 无法验证，待人工 GUI 确认 |
| 热键实际唤起窗口定位活动屏上中 | manual | V0-F2-A04 | headless 无法验证，待人工 GUI 确认 |
| 预热瞬开无延迟体感 | manual | V0-F2-A04 | headless 无法验证，待人工 GUI 确认 |

**A03 windowRoute：4 passed (4)**
**Rust 单测总计：6 passed (lib.rs unittests) + 3 passed (hotkey.rs integration) = 9 passed，0 failed**

## 覆盖缺口

**已覆盖：**
- V0-F2-A03：windowRoute 纯逻辑单测 4 例，涵盖 history/translate 路由、连续触发同视图、视图切换
- V0-F2-A05：图标目录含多尺寸 PNG（128x128、128x128@2x、64x64、32x32 等 17 个）、icon.ico、icon.icns
- 工程基线：build/clippy/tsc/fe_build 全绿，window_pos 定位计算单测（4 例）、Rust 冒烟（1 例）、hotkey 集成测试（3 例）全通过，无 TODO/FIXME

**不在覆盖范围（正常，非缺口）：**
- V0-F2-A04（manual_confirm）：托盘显示、热键唤起、瞬开体感属 GUI 运行时行为，headless 判不准，需人工确认。按 acceptance.yaml 定义为 `kind: manual_confirm`，不阻塞客观门禁
- V0-F3 系列（加密数据库、KeyProvider、schema）：属后续小功能，不在本次验收范围

## 失败项详情

无失败项。

## 结论

**放行——允许进入下一任务。**

V0-F2-S03 全部客观验收项通过：
- A03 = pass（windowRoute 4/4）
- A05 = pass（图标资源齐备，png/ico/icns 均存在）
- 工程基线全绿（build=0 / clippy=0 / tsc=0 / fe_build=0 / Rust 单测 9/9 / 无 TODO）
- A04 为 `manual_confirm`，标"待人工 GUI 确认"，按标准不列入自动门禁
