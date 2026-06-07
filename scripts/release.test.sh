#!/usr/bin/env bash
# release.test.sh — release.sh 预检的自带回归测试（CHANGELOG 门禁）
#
# 目的：把"发版前必须在 CHANGELOG.md 写好本版段"这条规则固化成回归测试，
# 防止日后改 release.sh 时悄悄退化（与 freeze-lint.test.sh 同源思路）。
# 核心负向用例：CHANGELOG 缺 `## v<version>` 段时，release.sh --dry-run 必须 die（退非零）；
# 正向：补上对应段后预检通过。
#
# 用法：bash release.test.sh
# 退出码：0=全部用例通过；1=有用例未达预期（CI 可据此阻断）

set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
release_sh="$here/release.sh"
pass_count=0
fail_count=0

# 在临时 git 仓库里造一份最小可发版工作树（满足 release.sh 预检的环境前提：
# 在 main、工作树干净、版本号低于目标），唯一变量是 CHANGELOG 是否含目标段。
# 返回仓库路径（调用方负责清理）。
make_repo() {
  local with_changelog_section="$1"  # true=写 v9.9.9 段；false=只写无关段
  local repo
  repo="$(mktemp -d "${TMPDIR:-/tmp}/release-test-XXXXXX")"

  git -C "$repo" init -q
  git -C "$repo" config user.email test@test.local
  git -C "$repo" config user.name test
  git -C "$repo" symbolic-ref HEAD refs/heads/main

  printf '{\n  "version": "0.1.0"\n}\n' > "$repo/package.json"
  mkdir -p "$repo/src-tauri"
  printf '{\n  "version": "0.1.0"\n}\n' > "$repo/src-tauri/tauri.conf.json"
  printf 'version = "0.1.0"\n' > "$repo/src-tauri/Cargo.toml"
  printf 'name = "quickquick"\nversion = "0.1.0"\n' > "$repo/src-tauri/Cargo.lock"

  if [[ "$with_changelog_section" == "true" ]]; then
    printf '# Changelog\n\n## v9.9.9\n\n- 测试版本更新内容\n' > "$repo/CHANGELOG.md"
  else
    printf '# Changelog\n\n## v0.1.0\n\n- 旧版内容，无目标段\n' > "$repo/CHANGELOG.md"
  fi

  cp "$release_sh" "$repo/release.sh"
  git -C "$repo" add -A
  git -C "$repo" commit -qm init
  echo "$repo"
}

# run_case <用例名> <with_changelog_section> <期望退出码> [期望命中的输出子串]
run_case() {
  local name="$1" with_section="$2" want_exit="$3" want_substr="${4:-}"
  local repo out got_exit
  repo="$(make_repo "$with_section")"
  out="$(cd "$repo" && bash release.sh 9.9.9 --dry-run 2>&1)"; got_exit=$?
  rm -rf "$repo"

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

# 负向（核心）：CHANGELOG 无 `## v9.9.9` 段 → 预检 die，退非零。
run_case "缺 CHANGELOG 段被拦（die 退非零）" false 1 "CHANGELOG.md 写 v9.9.9"

# 正向证伪护栏：补上 `## v9.9.9` 段后 --dry-run 走到底（演练成功 exit 0）——
# 证明拦截因缺段本身，非恒拦。
run_case "有 CHANGELOG 段则预检过（dry-run exit 0）" true 0 "预检通过"

echo
echo "release 自测结果：通过 ${pass_count}，失败 ${fail_count}"
[[ "$fail_count" -eq 0 ]] && exit 0 || exit 1
