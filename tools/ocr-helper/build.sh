#!/usr/bin/env bash
# 构建 OCR helper 与夹具生成器。产物落在 bin/，不进版本库。
#
# 模块缓存与临时目录都放在本目录内：swiftc 默认写 $TMPDIR/clang/ModuleCache，
# 在受限环境（只允许写工作区）里会报 "Operation not permitted"，本地化后构建不依赖外部状态。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$SCRIPT_DIR/bin"
CACHE_DIR="$SCRIPT_DIR/.build-cache"
SWIFTC="${SWIFTC:-swiftc}"

mkdir -p "$BIN_DIR" "$CACHE_DIR/module-cache" "$CACHE_DIR/tmp"
export TMPDIR="$CACHE_DIR/tmp"

if ! command -v "$SWIFTC" >/dev/null 2>&1; then
  echo "找不到 swiftc（可用 SWIFTC=/path/to/swiftc 指定）" >&2
  exit 1
fi

echo "swiftc: $(command -v "$SWIFTC")"
"$SWIFTC" --version | head -2

# 两个独立可执行文件各自单文件编译：helper 是交付物，make-fixtures 只服务于验证。
"$SWIFTC" -O -module-cache-path "$CACHE_DIR/module-cache" \
  "$SCRIPT_DIR/ocr_helper.swift" -o "$BIN_DIR/ocr-helper"
"$SWIFTC" -O -module-cache-path "$CACHE_DIR/module-cache" \
  "$SCRIPT_DIR/make_fixtures.swift" -o "$BIN_DIR/make-fixtures"

echo "已生成：$BIN_DIR/ocr-helper"
echo "已生成：$BIN_DIR/make-fixtures"
