#!/usr/bin/env bash
#
# QuickQuick 发版脚本：把版本号 bump → 提交 → 推送 → 打 tag 触发 CI 全流程固化成一条命令。
#
# 用法：
#   scripts/release.sh <version> [--dry-run] [--yes]
#   例：scripts/release.sh 0.2.0
#
# 参数：
#   <version>   纯语义化版本 X.Y.Z（不带前导 v）；tag 取 v<version>。
#   --dry-run   只演练改动并打印 diff，不提交/不推送/不打 tag。
#   --yes       跳过交互确认（无人值守用；默认会让你过目 diff 再继续）。
#
# 它做什么：
#   1. 预检：在仓库根、当前在 main、已跟踪工作树干净、与 origin/main 同步。
#   2. 同步 bump 四处版本号——package.json / src-tauri/tauri.conf.json /
#      src-tauri/Cargo.toml / src-tauri/Cargo.lock（仅 quickquick 包块）。
#   3. 提交 `chore(release): bump v<version>`，推送 main。
#   4. 打带注解 tag `v<version>` 并推送 → 触发 .github/workflows/release.yml
#      构建 macOS(Universal)+Windows、签名 updater 制品、生成 latest.json，
#      发到 GitHub Releases 草稿（需人工过目后点 Publish 才正式放出）。
#
# 前置（一次性，非本脚本职责，见 docs 发版记忆）：
#   - GitHub Secrets 已配 TAURI_SIGNING_PRIVATE_KEY / *_PASSWORD（updater 验签私钥）。
#   - tauri.conf.json 的 updater.pubkey 已填公钥、endpoint 指向 releases/latest/download/latest.json。
#
# 坑（脚本已规避，改动这里前先读）：
#   - Cargo.lock 里多个第三方 crate 也叫 version = "0.x.y"（如 objc2-exception-helper），
#     必须只改 `name = "quickquick"` 那一块的 version，故用 name 锚定的 awk，不能全局替换。
#   - tag 必须是 v* 才触发 workflow（on.push.tags: 'v*'）。

set -euo pipefail

readonly MAIN_BRANCH="main"
readonly PKG_JSON="package.json"
readonly TAURI_CONF="src-tauri/tauri.conf.json"
readonly CARGO_TOML="src-tauri/Cargo.toml"
readonly CARGO_LOCK="src-tauri/Cargo.lock"
readonly CRATE_NAME="quickquick"
readonly CHANGELOG="CHANGELOG.md"

# 统一红色错误退出，给可定位的失败信息（错误处理在边界完成）。
die() {
  echo "错误：$*" >&2
  exit 1
}

# 解析入参，把结果写进全局 VERSION / DRY_RUN / ASSUME_YES。
parse_args() {
  VERSION=""
  DRY_RUN="false"
  ASSUME_YES="false"
  for arg in "$@"; do
    case "$arg" in
      --dry-run) DRY_RUN="true" ;;
      --yes) ASSUME_YES="true" ;;
      -h|--help) grep '^#' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
      -*) die "未知选项：$arg" ;;
      *) [[ -z "$VERSION" ]] || die "只接受一个版本参数（多余：${arg}）"; VERSION="$arg" ;;
    esac
  done
  [[ -n "$VERSION" ]] || die "缺少版本参数。用法：scripts/release.sh <version> [--dry-run] [--yes]"
  [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "版本格式须为 X.Y.Z（不带前导 v），收到：$VERSION"
}

# 预检：环境与工作树状态满足发版前提，任一不满足即中止（避免脏发布）。
preflight() {
  command -v git >/dev/null || die "未找到 git"
  cd "$(git rev-parse --show-toplevel)" || die "不在 git 仓库内"

  local branch
  branch="$(git rev-parse --abbrev-ref HEAD)"
  [[ "$branch" == "$MAIN_BRANCH" ]] || die "当前在 '$branch'，发版须在 '$MAIN_BRANCH'"

  [[ -z "$(git status --porcelain --untracked-files=no)" ]] \
    || die "已跟踪文件有未提交改动，请先提交或还原后再发版"

  local current
  current="$(read_current_version)"
  [[ "$current" != "$VERSION" ]] || die "目标版本 $VERSION 与当前版本相同，无需发版"
  # 用 sort -V 做语义化比较，禁止往回降版（防误发旧号覆盖 latest）。
  [[ "$(printf '%s\n%s\n' "$current" "$VERSION" | sort -V | tail -1)" == "$VERSION" ]] \
    || die "目标版本 $VERSION 低于当前 ${current}，不允许降版"

  git tag -l "v$VERSION" | grep -q . && die "tag v$VERSION 已存在" || true

  # CHANGELOG 门禁：本版必须在 CHANGELOG.md 有对应 `## v<version>` 段，
  # 否则 release.yml 抽不到更新内容、Release 正文只剩安装指南。把"每次写进去"变硬门禁。
  [[ -f "$CHANGELOG" ]] || die "缺少 $CHANGELOG，发版前先在 $CHANGELOG 写 v$VERSION 更新内容"
  grep -qE "^## v${VERSION//./\\.}([[:space:]]|$)" "$CHANGELOG" \
    || die "$CHANGELOG 缺 v$VERSION 段，发版前先在 $CHANGELOG 写 v$VERSION 更新内容"

  echo "预检通过：$current → ${VERSION}（分支 ${branch}，工作树干净）"
}

# 从 package.json 读取当前版本号（顶层 version 字段为第一处 "version"）。
read_current_version() {
  grep -m1 '"version"' "$PKG_JSON" | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
}

# 把 JSON 文件里第一处 "version": "x.y.z" 改成目标版本（顶层版本字段，不动依赖）。
bump_json_first_version() {
  local file="$1"
  perl -i -pe '
    if (!$done && /"version"\s*:\s*"\d+\.\d+\.\d+"/) {
      s/"version"\s*:\s*"\d+\.\d+\.\d+"/"version": "'"$VERSION"'"/;
      $done = 1;
    }
  ' "$file"
}

# 改 Cargo.toml [package] 版本：第一处行首 version = "x.y.z" 即包版本。
bump_cargo_toml() {
  perl -i -pe '
    if (!$done && /^version\s*=\s*"\d+\.\d+\.\d+"/) {
      s/^version\s*=\s*"\d+\.\d+\.\d+"/version = "'"$VERSION"'"/;
      $done = 1;
    }
  ' "$CARGO_TOML"
}

# 改 Cargo.lock 中本 crate 的版本：用 name 锚定，只改 quickquick 块紧随的 version 行，
# 避开其它同号第三方 crate（这是本脚本最易踩错的一处）。
bump_cargo_lock() {
  awk -v ver="$VERSION" -v crate="$CRATE_NAME" '
    $0 == "name = \"" crate "\"" { in_crate = 1 }
    in_crate && /^version = / {
      sub(/version = ".*"/, "version = \"" ver "\"")
      in_crate = 0
    }
    { print }
  ' "$CARGO_LOCK" > "$CARGO_LOCK.tmp" && mv "$CARGO_LOCK.tmp" "$CARGO_LOCK"
}

# 执行四处 bump。
bump_all_versions() {
  bump_json_first_version "$PKG_JSON"
  bump_json_first_version "$TAURI_CONF"
  bump_cargo_toml
  bump_cargo_lock
  echo "已 bump 版本号 → ${VERSION}（${PKG_JSON} / ${TAURI_CONF} / ${CARGO_TOML} / ${CARGO_LOCK}[quickquick]）"
}

# 交互确认（--yes / --dry-run 时跳过）；用户输入非 y/Y 即中止。
confirm_or_abort() {
  [[ "$ASSUME_YES" == "true" ]] && return 0
  local reply
  read -r -p "以上改动将提交、推送 main 并打 tag v$VERSION 触发发版。继续？[y/N] " reply
  [[ "$reply" == "y" || "$reply" == "Y" ]] || die "已取消（未提交任何改动）"
}

# 提交、推送 main、打带注解 tag 并推送（推送 tag 即触发 CI 发版）。
commit_push_tag() {
  git add "$PKG_JSON" "$TAURI_CONF" "$CARGO_TOML" "$CARGO_LOCK"
  git commit -m "chore(release): bump v$VERSION"
  git push origin "$MAIN_BRANCH"
  git tag -a "v$VERSION" -m "v$VERSION"
  git push origin "v$VERSION"
}

main() {
  parse_args "$@"
  preflight
  bump_all_versions

  echo "── 版本号改动 diff ──"
  git --no-pager diff -- "$PKG_JSON" "$TAURI_CONF" "$CARGO_TOML" "$CARGO_LOCK"

  if [[ "$DRY_RUN" == "true" ]]; then
    echo "（--dry-run：仅演练，未提交/未推送/未打 tag；正在还原工作树改动）"
    git checkout -- "$PKG_JSON" "$TAURI_CONF" "$CARGO_TOML" "$CARGO_LOCK"
    exit 0
  fi

  confirm_or_abort
  commit_push_tag

  echo "已推送 tag v${VERSION}，CI 发版已触发。后续："
  echo "  - 查看构建：gh run list --workflow=release.yml"
  echo "  - 构建完成后到 GitHub Releases 过目草稿，确认无误再点 Publish 正式放出。"
  echo "  - Publish 后 latest.json 生效，旧版客户端的自动更新即可拉到 v${VERSION}。"
}

main "$@"
