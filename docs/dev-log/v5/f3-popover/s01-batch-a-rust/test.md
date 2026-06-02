---
id: f3-popover-s01-batch-a-rust-test
title: 里程碑4 popover Batch A 骨架 — 动态证伪报告
status: 测试通过
commit: 331a9e6
date: 2026-06-02
---

# 测试报告：里程碑4 popover · Batch A 骨架

## 1. 命中校验（杀假绿）

命令：`cargo test window_pos`

结果：**7 passed，0 failed**，非空匹配（N≥7 符合要求）。

具体命中的测试名：

```
test window_pos::tests::center_top_accounts_for_monitor_offset ... ok
test window_pos::tests::center_top_with_custom_width_y_unchanged ... ok
test window_pos::tests::center_top_with_width_320_gives_correct_x ... ok
test window_pos::tests::center_top_x_is_centered ... ok
test window_pos::tests::center_top_with_width_720_gives_correct_x ... ok
test window_pos::tests::center_top_y_is_fifteen_percent ... ok
test window_pos::tests::find_monitor_at_no_monitors_returns_none ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 60 filtered out
```

结论：无假绿，测试真实命中。

## 2. 变异 sanity（杀恒真/旁路）

### 变异 A：居中公式 `width` → `WINDOW_WIDTH`（忽略传入 width）

- 改动：第 90 行 `(mon_w as i32 - width) / 2` → `(mon_w as i32 - WINDOW_WIDTH) / 2`
- 预期变红：width=720 和 width=320 用例
- 实际结果：**如期变红**
  ```
  test window_pos::tests::center_top_with_width_720_gives_correct_x ... FAILED
  test window_pos::tests::center_top_with_width_320_gives_correct_x ... FAILED
  test result: FAILED. 5 passed; 2 failed
  ```
- 还原：`cp /tmp/window_pos.rs.bak` 还原，git status 干净，全量回绿确认

### 变异 B：y 公式 `TOP_RATIO` (0.15) → `0.25`

- 改动：第 91 行 `mon_h as f64 * TOP_RATIO` → `mon_h as f64 * 0.25`
- 预期变红：y 相关用例
- 实际结果：**如期变红**
  ```
  test window_pos::tests::center_top_y_is_fifteen_percent ... FAILED
  test window_pos::tests::center_top_with_custom_width_y_unchanged ... FAILED
  test window_pos::tests::center_top_accounts_for_monitor_offset ... FAILED
  test result: FAILED. 4 passed; 3 failed
  ```
- 还原：`cp /tmp/window_pos.rs.bak` 还原，git status 干净，全量回绿确认

结论：测试有真实判别力，非恒真、非旁路。

## 3. 边界探测

分析 `center_top_position` 在边界输入下的行为（静态推导，公式：`x = mon_x + (mon_w as i32 - width) / 2`）：

| 边界 | x 结果 | 行为 |
|------|--------|------|
| width=0（1920px 显示器）| x = 960 | 不 panic，语义奇怪（宽0窗口），属已知设计边界 |
| width=2560（大于1920显示器宽）| x = -320，窗口溢出屏幕左侧 | 不 panic，部分内容不可见，属已知设计边界（实际 popover 宽远小于屏宽） |

结论：两类边界均无崩溃、无 panic，属已知设计限制而非程序缺陷；实际使用中不会触发（popover 宽度由调用方控制，远小于屏幕宽）。

## 4. 构建集成 sanity

命令：`pnpm build`

结果：exit 0，三入口全部产出：

```
/dist/index.html                         1.3K
/dist/src/clip-popover/index.html        942B
/dist/src/trans-popover/index.html       940B
```

结论：多入口 vite 构建正常，popover 入口集成无问题。

## 5. 最终 git status 干净证明

```
$ git status --porcelain
（空输出）
```

工作树与开工快照一致（开工时干净，结束时也干净）。所有变异均已从备份还原，无残留改动。

## 6. 门禁结论

**测试通过** — 放行。

- 命中校验：7 个 window_pos 测试全部命中通过，无假绿
- 变异 sanity：A/B 两处变异均如期使对应用例变红，测试有真实判别力
- 边界探测：width=0 / 超宽均属已知设计边界，无崩溃缺陷
- 构建 sanity：三入口全部产出，exit 0
- 工作树：干净，无残留变异
