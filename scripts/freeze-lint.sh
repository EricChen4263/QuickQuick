#!/usr/bin/env bash
# freeze-lint.sh — 冻结前机检 acceptance.yaml（goal-dev-workflow 版本启动·织全三网）
#
# 把"织全三网"从人的自觉变成机制强制：版本启动冻结前必跑，不过则阻断冻结。
# 检查项（冻结时可机检的结构性部分）：
#   1. coverage_check 里每个声明 covered 的类别，acceptance 列表至少有 1 个 category 匹配条目（防空洞声明）
#   2. 每个 acceptance 项有非空 source_anchor（溯源锚，防造）
#   3. kind: objective 项的 verify 必须存在且非 null（可执行验证；manual_confirm 可 null）
#   4. 战略锚（启用时）：C1 serves_goals 非空 / C2 引用的目标 ID 在 STRATEGY.md 真实存在 / C3 strategy_freeze 两边一致
# 不在此查的：runner↔实际测试名是否真命中——TDD 红阶段测试尚未写，留到裁决前（producer/tester 的命中校验）。
#
# 用法：bash freeze-lint.sh <path/to/acceptance.yaml> [path/to/STRATEGY.md]
# 战略层（C1/C2/C3）：传了 STRATEGY.md 或 acceptance 含 serves_goals 即启用；启用却不传 STRATEGY.md → 用法错(2)。
# 退出码：0=通过可冻结；1=有缺陷，阻断冻结（缺陷逐条打印到 stderr）

set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "用法: bash freeze-lint.sh <acceptance.yaml> [STRATEGY.md]" >&2
  exit 2
fi

file="$1"
if [[ ! -f "$file" ]]; then
  echo "freeze-lint: 文件不存在: $file" >&2
  exit 2
fi

fail=0
emit() { echo "freeze-lint ✗ $1" >&2; fail=1; }

# 检查 1：coverage_check 声明 covered 的类别，须有 category 匹配条目
# coverage_check 块：从 'coverage_check:' 到 'acceptance:' 之间；covered 行形如 "  功能正确性:  covered"
covered_cats=$(awk '
  /^coverage_check:/ {inblk=1; next}
  /^acceptance:/     {inblk=0}
  inblk && /:[[:space:]]*covered/ {
    line=$0
    sub(/:.*/, "", line)      # 取冒号前
    gsub(/[[:space:]]/, "", line)
    if (line != "") print line
  }
' "$file")

# acceptance 项里出现过的 category 值（形如 "    category: 功能正确性"，可能带行尾注释）
# 去重用 awk 而非 `sort -u`：macOS BSD sort 在 UTF-8 locale 下会把不同 CJK 串误判相等而误删。
item_cats=$(grep -E '^[[:space:]]+category:' "$file" \
  | sed -E 's/^[[:space:]]+category:[[:space:]]*//; s/[[:space:]]*#.*$//; s/[[:space:]]+$//' \
  | awk '!seen[$0]++')

while IFS= read -r cat; do
  [[ -z "$cat" ]] && continue
  if ! grep -qxF "$cat" <<<"$item_cats"; then
    emit "coverage_check 声明 '$cat' 为 covered，但 acceptance 无任何 category=$cat 的条目（空洞声明）"
  fi
done <<<"$covered_cats"

# 检查 2 & 3：逐个 acceptance 项：有 source_anchor；objective 项 verify 非空
# 用 awk 按 '- id:' 切块，块内检查 source_anchor / kind / verify
awk '
  function flush() {
    if (id == "") return
    if (!has_src) { print "freeze-lint ✗ " id ": 缺 source_anchor（溯源锚必填）" > "/dev/stderr"; rc=1 }
    if (kind == "objective" && !has_verify) { print "freeze-lint ✗ " id ": kind=objective 但缺可执行 verify" > "/dev/stderr"; rc=1 }
  }
  BEGIN {inacc=0; id=""; rc=0}
  /^acceptance:/ {inacc=1; next}
  /^change_log:/ {inacc=0}
  !inacc {next}
  /^[[:space:]]*-[[:space:]]*id:/ {
    flush()
    id=$0; sub(/.*id:[[:space:]]*/, "", id); gsub(/[[:space:]]/, "", id)
    has_src=0; kind=""; has_verify=0
    next
  }
  /^[[:space:]]+source_anchor:/ {has_src=1}
  /^[[:space:]]+kind:/ {k=$0; sub(/.*kind:[[:space:]]*/, "", k); gsub(/[[:space:]]/, "", k); kind=k}
  /^[[:space:]]+verify:/ {
    v=$0; sub(/.*verify:[[:space:]]*/, "", v)
    if (v !~ /^null$/ && v !~ /^[[:space:]]*$/) has_verify=1   # 行内 flow 式 verify: { type:... }
  }
  /^[[:space:]]+type:/ { has_verify=1 }   # 多行块式 verify: 下缩进的 type:，凭其判定 verify 存在
  END {flush(); exit rc}
' "$file" || fail=1

# 战略层校验（C1 失锚 / C2 悬空锚 / C3 过期锚）
# 是否启用：传了第二参 STRATEGY.md，或 acceptance 含 serves_goals 键。
strategy="${2:-}"
has_serves=$(grep -cE '^serves_goals:' "$file" || true)
engaged=0
[[ -n "$strategy" ]] && engaged=1
[[ "$has_serves" -gt 0 ]] && engaged=1

if [[ "$engaged" -eq 1 ]]; then
  # 启用却没传 STRATEGY.md：无法校验，报用法错（堵死"不传就绕过"旁路）。
  if [[ -z "$strategy" ]]; then
    echo "freeze-lint: 本版启用战略层（acceptance 含 serves_goals）但未传 STRATEGY.md，无法校验。用法: bash freeze-lint.sh <acceptance.yaml> <STRATEGY.md>" >&2
    exit 2
  fi
  if [[ ! -f "$strategy" ]]; then
    echo "freeze-lint: STRATEGY.md 不存在: $strategy" >&2
    exit 2
  fi

  # 取 acceptance 的 serves_goals（flow 式 [G1, G3]），拆成多行 id 集。
  serves_raw=$(grep -E '^serves_goals:' "$file" | head -1 | sed -E 's/^serves_goals:[[:space:]]*//; s/#.*$//' || true)
  serves_ids=$(printf '%s\n' "$serves_raw" | sed -E 's/[][]//g; s/,/\n/g' | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//' | awk 'NF')

  # C1 失锚：serves_goals 缺失或为空。
  if [[ "$has_serves" -eq 0 || -z "$serves_ids" ]]; then
    emit "版本未锚定战略目标（serves_goals 缺失/为空）"
  fi

  # STRATEGY.md 的 Goals id 集（'## 目标' 段内、下一个 '## ' 之前的 '- id:' 行）。
  goal_ids=$(awk '
    /^##[[:space:]]+目标/ {inblk=1; next}
    /^##[[:space:]]/      {inblk=0}
    inblk && /^-[[:space:]]+id:/ {
      g=$0; sub(/.*id:[[:space:]]*/, "", g); sub(/#.*/, "", g); gsub(/[[:space:]]/, "", g); if (g!="") print g
    }
  ' "$strategy")

  # C2 悬空锚：serves 里的 id 不在 STRATEGY Goals。
  while IFS= read -r sid; do
    [[ -z "$sid" ]] && continue
    if ! grep -qxF "$sid" <<<"$goal_ids"; then
      emit "serves_goals 引用了不存在的战略目标：$sid"
    fi
  done <<<"$serves_ids"

  # C3 过期锚：两边 strategy_freeze 不一致。
  acc_fz=$(grep -E '^strategy_freeze:' "$file"     | head -1 | sed -E 's/^strategy_freeze:[[:space:]]*//; s/#.*$//; s/[[:space:]]+$//' || true)
  str_fz=$(grep -E '^strategy_freeze:' "$strategy" | head -1 | sed -E 's/^strategy_freeze:[[:space:]]*//; s/#.*$//; s/[[:space:]]+$//' || true)
  if [[ "$acc_fz" != "$str_fz" ]]; then
    emit "战略已变更（${acc_fz} → ${str_fz}），本版须重对齐后重冻结"
  fi
fi

if [[ $fail -ne 0 ]]; then
  echo "freeze-lint: 存在缺陷，阻断冻结。修正上述项后重跑。" >&2
  exit 1
fi
echo "freeze-lint ✓ $file 通过（覆盖无空洞 / 溯源锚齐 / objective 项 verify 齐）。runner↔测试名一致性留裁决前命中校验。"
exit 0
