---
id: V1-F1-S03-test
type: test_report
level: 小功能
parent: V1-F1
created: 2026-05-30T22:53:40Z
status: 通过
commit: WIP
acceptance_ids: [V1-F1-A06, V1-F1-A07, V1-F1-A08]
author: tester
---

# 测试报告 · 隐私门控（V1-F1-S03）

## 运行命令

```bash
# 1. S03 专项：privacy 集成测试（A06/A07）
cargo test --manifest-path src-tauri/Cargo.toml --test privacy > /tmp/Tv1s3p.log 2>&1
echo "privacy=$?"

# 2. S01/S02 + A08 回归：clipboard 集成测试
cargo test --manifest-path src-tauri/Cargo.toml --test clipboard > /tmp/Tv1s3c.log 2>&1
echo "clip=$?"

# 3. 全量测试
cargo test --manifest-path src-tauri/Cargo.toml > /tmp/Tv1s3all.log 2>&1
echo "all=$?"

# 4. Clippy 静态检查（-D warnings 零容忍）
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings > /tmp/Tv1s3cl.log 2>&1
echo "clippy=$?"

# 5. 无 TODO/FIXME
grep -rn 'TODO\|FIXME' src-tauri/src/
echo "todo=$?(期望1)"
```

## 执行结果汇总

| 检查项 | 退出码 | 结论 |
|--------|--------|------|
| `cargo test --test privacy` | 0 | 5 passed, 0 failed |
| `cargo test --test clipboard` | 0 | 8 passed, 0 failed |
| `cargo test`（全量） | 0 | 全部测试套件绿 |
| `cargo clippy --all-targets -- -D warnings` | 0 | 无 warning，无 error |
| `grep TODO\|FIXME src-tauri/src/` | 1（无匹配） | 无遗留标记 |

### 测试输出原文（privacy）

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
     Running tests/privacy.rs

running 5 tests
test app_exclude_list ... ok
test app_exclude_none_source ... ok
test app_not_in_exclude_list ... ok
test concealed_no_heuristic ... ok
test concealed_skipped ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 测试输出原文（clipboard，含 A08 回归）

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.15s
     Running tests/clipboard.rs

running 8 tests
test capture_dual_field ... ok
test poll_changecount_triggers_capture ... ok
test pause_false_captures_normally ... ok
test pause_stops_capture ... ok
test poll_count_reset_defense ... ok
test self_write_marker_skipped ... ok
test bump_no_new_record ... ok
test dedup_and_bump ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 用例与验收映射表

### S03 新增用例（privacy 测试套件）

| 用例函数名 | 对应验收 ID | 验证行为 | 结果 |
|---|---|---|---|
| `concealed_skipped` | V1-F1-A06 | `is_concealed=true` → `should_skip` 返回 `Some(Concealed)`，由平台标记驱动，不看内容 | **pass** |
| `concealed_no_heuristic` | V1-F1-A06 | 内容含密码特征字符串但 `is_concealed=false` → 返回 `None`，证明无内容启发式识别 | **pass** |
| `app_exclude_list` | V1-F1-A07 | `source_app` 命中排除名单 → 返回 `Some(Excluded)` | **pass** |
| `app_not_in_exclude_list` | V1-F1-A07 | `source_app` 不在名单 → 返回 `None` | **pass** |
| `app_exclude_none_source` | V1-F1-A07 | `source_app=None`（来源未知）→ 返回 `None`，不因名单误跳过 | **pass** |

### A08 用例（clipboard 测试套件，S03 新增）

| 用例函数名 | 对应验收 ID | 验证行为 | 结果 |
|---|---|---|---|
| `pause_stops_capture` | V1-F1-A08 | `paused=true` 时 `poll_once_with_policy` 对正常可捕快照返回 `None`，`last_seen` 仍推进 | **pass** |
| `pause_false_captures_normally` | V1-F1-A08 | `paused=false` 时同一快照正常捕获，验证开关双向有效 | **pass** |

### S01/S02 回归用例（clipboard 测试套件）

| 用例函数名 | 对应验收 ID | 验证行为 | 结果 |
|---|---|---|---|
| `capture_dual_field` | V1-F1-A01 | 双字段同存，`last_seen` 更新 | **pass** |
| `poll_changecount_triggers_capture` | V1-F1-A02 | count 驱动捕获，一次变化只捕获一次 | **pass** |
| `self_write_marker_skipped` | V1-F1-A03 | 自写标记跳过，`last_seen` 仍推进 | **pass** |
| `poll_count_reset_defense` | I-01 | OS 计数重置（降序）不误捕，恢复后正常捕获 | **pass** |
| `dedup_and_bump` | V1-F1-A04 | 重复文本返回 `Bumped`，行数不增，置顶刷新后原条目成为最前 | **pass** |
| `bump_no_new_record` | V1-F1-A05 | `bump_to_top` 不产生新记录，置顶后行数仍为 2 | **pass** |

## 覆盖缺口

| 缺口 | 说明 | 风险等级 |
|---|---|---|
| OS 真实后端未测 | `should_skip` 在真实 macOS 剪贴板（`NSPasteboardTypeConcealed`）下的端到端行为未验证；当前靠 `FakeBackend`/纯逻辑层覆盖，OS 层留待 E2E 阶段 | 低 |
| 名单大小写归一化 | `ExcludeList` 目前为精确匹配；若 app bundle ID 大小写不一致，可能漏判——此为已知 trade-off，可在后续迭代加 `.to_lowercase()` 处理 | 低 |
| `paused` 状态持久化 | 暂停开关的持久化与跨会话恢复未覆盖（属于配置模块范畴，不在 S03 范围内） | 低 |

## 回归确认

S01（A01/A02/A03）与 S02（A04/A05）所有既有测试在本次全量运行中仍 **全部绿**，无回归。

## 结论

**门禁判定：放行**

- A06（concealed 标记门控）：2/2 用例通过
- A07（app 排除名单）：3/3 用例通过
- A08（paused 全局暂停）：2/2 用例通过
- S01/S02 回归：6/6 用例通过
- Clippy：退出码 0，零 warning
- TODO/FIXME：无

全部验收项满足，可进入下一阶段。
