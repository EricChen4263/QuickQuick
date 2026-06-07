#!/usr/bin/env bash
# freeze-lint.test.sh — freeze-lint.sh 的自带回归测试（正向 + 负向）
#
# 目的：把"空洞声明能被拦住"这类判别力固化成回归测试，防止日后改 freeze-lint.sh
# 时悄悄退化成恒过的橡皮图章（与工作流"凡 AI 的自觉换成机制的强制"同源）。
# 负向核心用例复刻了 v0 实测发现的场景：coverage_check 声明某类 covered，
# 但 acceptance 无任何该 category 的条目 = 空洞，必须被拦（exit 1）。
# 测试自包含（内嵌 fixture，不耦合任何项目路径），故全局 skill 可随处重跑。
#
# 用法：bash freeze-lint.test.sh
# 退出码：0=全部用例通过；1=有用例未达预期（CI 可据此阻断）

set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
lint="$here/freeze-lint.sh"
workdir="$(mktemp -d "${TMPDIR:-/tmp}/freeze-lint-test-XXXXXX")"
trap 'rm -rf "$workdir"' EXIT

pass_count=0
fail_count=0

# run_case <用例名> <fixture内容文件> <期望退出码> [期望命中的输出子串]
# Arrange 已由调用方写好 fixture；本函数负责 Act（跑 lint）+ Assert（核对退出码与输出）。
run_case() {
  local name="$1" fixture="$2" want_exit="$3" want_substr="${4:-}"
  local out got_exit
  out="$(bash "$lint" "$fixture" 2>&1)"; got_exit=$?

  if [[ "$got_exit" -ne "$want_exit" ]]; then
    echo "✗ FAIL ${name}：期望 exit=${want_exit}，实际 exit=${got_exit}"
    echo "    输出：${out}"
    fail_count=$((fail_count + 1))
    return
  fi
  if [[ -n "$want_substr" ]] && ! grep -qF "$want_substr" <<<"$out"; then
    echo "✗ FAIL ${name}：exit 正确但输出未含期望子串 '${want_substr}'"
    echo "    输出：${out}"
    fail_count=$((fail_count + 1))
    return
  fi
  echo "✓ PASS ${name}（exit=${got_exit}）"
  pass_count=$((pass_count + 1))
}

# run_case_s <用例名> <acceptance fixture> <strategy fixture> <期望退出码> [期望子串]
# 与 run_case 同构，但给 lint 传两个参数（acceptance + STRATEGY.md）。
run_case_s() {
  local name="$1" acc="$2" strat="$3" want_exit="$4" want_substr="${5:-}"
  local out got_exit
  out="$(bash "$lint" "$acc" "$strat" 2>&1)"; got_exit=$?

  if [[ "$got_exit" -ne "$want_exit" ]]; then
    echo "✗ FAIL ${name}：期望 exit=${want_exit}，实际 exit=${got_exit}"
    echo "    输出：${out}"
    fail_count=$((fail_count + 1))
    return
  fi
  if [[ -n "$want_substr" ]] && ! grep -qF "$want_substr" <<<"$out"; then
    echo "✗ FAIL ${name}：exit 正确但输出未含期望子串 '${want_substr}'"
    echo "    输出：${out}"
    fail_count=$((fail_count + 1))
    return
  fi
  echo "✓ PASS ${name}（exit=${got_exit}）"
  pass_count=$((pass_count + 1))
}

# 战略层 fixture：合法 STRATEGY.md（含 G1/G2，strategy_freeze=STRATEGY@TEST）。
write_valid_strategy() {
  cat > "$1" <<'MD'
---
product: 测试产品
frozen_at: 2026-06-02T00:00:00Z
strategy_freeze: STRATEGY@TEST
change_log: []
---
# 北极星
让测试更可信

## 目标 Goals
- id: G1                  # 永久稳定 ID，不复用；下游 serves_goals 靠它引用
  statement: "目标一"
  signal: "信号一"
- id: G2                  # 第二个目标
  statement: "目标二"
  signal: "信号二"

## 非目标 Non-Goals
- "不做的事"

## 约束 Constraints
- "某约束"
MD
}

# 战略层 fixture：合法 acceptance（serves_goals=[G1]，strategy_freeze 与上面一致）。
write_valid_acc_strat() {
  cat > "$1" <<'YAML'
version: VTEST
strategy_freeze: STRATEGY@TEST
serves_goals: [G1]
coverage_check:
  功能正确性:  covered
acceptance:
  - id: VTEST-A01
    assertion: "战略层基线"
    category: 功能正确性
    source_anchor: doc§1
    kind: objective
    verify: { type: command, ref: "true", expect: pass }
change_log: []
YAML
}

# 合法基线：覆盖↔条目一一对上、溯源锚齐、objective 项 verify 齐。
# 同时覆盖两种 verify 写法——行内 flow 式 + 多行块式（freeze-lint 两者都要认）。
write_valid_baseline() {
  cat > "$1" <<'YAML'
version: VTEST
coverage_check:
  功能正确性:  covered
  安全:        covered
  人工确认点:  covered
  资源规范:    N/A (测试基线无新增资源)
acceptance:
  - id: VTEST-A01
    assertion: "行内 flow 式 verify"
    category: 功能正确性
    source_anchor: doc§1
    kind: objective
    verify: { type: test_id, ref: foo, runner: "cargo test foo", path: tests/foo.rs, expect: pass }
  - id: VTEST-A02
    assertion: "多行块式 verify"
    category: 安全
    source_anchor: doc§2
    kind: objective
    verify:
      type: command
      ref: "cargo build"
      expect: pass
  - id: VTEST-A03
    assertion: "纯审美项，无可执行手段"
    category: 人工确认点
    source_anchor: doc§3
    kind: manual_confirm
    verify: null
    collect_env: real_device
change_log: []
YAML
}

# 正向 1：合法基线应通过。
fx_valid="$workdir/valid.yaml"
write_valid_baseline "$fx_valid"
run_case "合法基线通过（含 flow式+块式 verify）" "$fx_valid" 0 "通过"

# 负向 1（核心，复刻 v0 注入）：在合法基线上加 '性能: covered'，但无任何 category=性能 条目 → 空洞，必拦。
fx_hollow="$workdir/hollow.yaml"
awk '/^coverage_check:/{print;print "  性能:        covered";next}1' "$fx_valid" > "$fx_hollow"
run_case "空洞声明被拦（性能 covered 但无条目）" "$fx_hollow" 1 "无任何 category=性能 的条目"

# 负向 2：objective 项缺溯源锚 → 必拦。
fx_noanchor="$workdir/no-anchor.yaml"
awk '/source_anchor: doc§1/{next}1' "$fx_valid" > "$fx_noanchor"
run_case "缺 source_anchor 被拦" "$fx_noanchor" 1 "缺 source_anchor"

# 负向 3：objective 项缺可执行 verify → 必拦（把 A01 的 flow 式 verify 整行删掉）。
fx_noverify="$workdir/no-verify.yaml"
awk '/id: VTEST-A01/{a=1} a&&/^[[:space:]]+verify:/{next}1' "$fx_valid" > "$fx_noverify"
run_case "objective 缺 verify 被拦" "$fx_noverify" 1 "缺可执行 verify"

# 反向证伪护栏：把负向 1 的空洞补上对应条目后，应重新通过——证明拦截是因空洞本身，非恒拦。
# 用 awk 在 change_log 行前插入一条 category=性能 的合法条目（awk 多行 print 跨平台稳定，
# 不依赖 BSD/GNU sed 对替换串里 \n 的差异行为）。
fx_fixed="$workdir/hollow-fixed.yaml"
awk '/^change_log:/{
  print "  - id: VTEST-A04"
  print "    assertion: \"补的性能项\""
  print "    category: 性能"
  print "    source_anchor: doc§4"
  print "    kind: objective"
  print "    verify: { type: command, ref: \"bench\", expect: pass }"
}1' "$fx_hollow" > "$fx_fixed"
run_case "补齐条目后空洞消除、重新通过" "$fx_fixed" 0 "通过"

# 战略层 C1/C2/C3 用例
fx_strat="$workdir/STRATEGY.md"
write_valid_strategy "$fx_strat"
fx_accs="$workdir/acc-strat.yaml"
write_valid_acc_strat "$fx_accs"

# 正向：战略基线（serves_goals=[G1] 真实存在、strategy_freeze 一致）应通过。
run_case_s "战略基线通过（serves_goals 有效且锚一致）" "$fx_accs" "$fx_strat" 0 "通过"

# 旁路堵死：acceptance 含 serves_goals 但只传单参（不给 STRATEGY.md）→ 用法错 exit 2，不得放行。
run_case "启用战略层却不传 STRATEGY.md → exit 2" "$fx_accs" 2 "未传 STRATEGY.md"

# C1 失锚：删掉 serves_goals 行但仍传 STRATEGY.md（第二参触发战略层）→ 必拦。
fx_c1="$workdir/acc-c1.yaml"
awk '!/^serves_goals:/' "$fx_accs" > "$fx_c1"
run_case_s "C1 失锚被拦（serves_goals 缺失）" "$fx_c1" "$fx_strat" 1 "未锚定战略目标"

# C2 悬空锚：serves_goals 引用不存在的 G9 → 必拦。
fx_c2="$workdir/acc-c2.yaml"
sed -E 's/^serves_goals:.*/serves_goals: [G9]/' "$fx_accs" > "$fx_c2"
run_case_s "C2 悬空锚被拦（引用不存在的 G9）" "$fx_c2" "$fx_strat" 1 "不存在的战略目标：G9"

# C3 过期锚：acceptance 的 strategy_freeze 与 STRATEGY.md 不一致 → 必拦。
fx_c3="$workdir/acc-c3.yaml"
sed -E 's/^strategy_freeze:.*/strategy_freeze: STRATEGY@OLD/' "$fx_accs" > "$fx_c3"
run_case_s "C3 过期锚被拦（strategy_freeze 不一致）" "$fx_c3" "$fx_strat" 1 "战略已变更"

# 反向证伪护栏：把 C2 的 G9 修回 G1 后应重新通过——证明拦截因悬空本身，非恒拦。
fx_c2fix="$workdir/acc-c2-fixed.yaml"
sed -E 's/^serves_goals:.*/serves_goals: [G1]/' "$fx_c2" > "$fx_c2fix"
run_case_s "C2 修回 G1 后重新通过" "$fx_c2fix" "$fx_strat" 0 "通过"

echo
echo "freeze-lint 自测结果：通过 ${pass_count}，失败 ${fail_count}"
[[ "$fail_count" -eq 0 ]] && exit 0 || exit 1
