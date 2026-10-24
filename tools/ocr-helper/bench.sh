#!/usr/bin/env bash
# 两档分辨率的耗时基准。RUNS 控制单档次数（默认 7，取中位数）。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$SCRIPT_DIR/bin"
FIXTURE_DIR="$SCRIPT_DIR/fixtures"
HELPER="$BIN_DIR/ocr-helper"
RUNS="${RUNS:-7}"

# 受限环境（只允许写工作区）里 Vision 的 accurate 路径需要可写 HOME 才能加载模型，
# 否则会以 generic error 失败。正常开发机 HOME 可写、无需设置；
# 在沙箱里复跑验收时设 OCR_SANDBOX_HOME=1，把 HOME 指到本目录内的缓存。
if [[ "${OCR_SANDBOX_HOME:-0}" == "1" ]]; then
  export HOME="$SCRIPT_DIR/.build-cache/home"
  export CFFIXED_USER_HOME="$HOME"
  export TMPDIR="$SCRIPT_DIR/.build-cache/tmp"
  mkdir -p "$HOME" "$TMPDIR"
  echo "注意：HOME/TMPDIR 已重定向到 $SCRIPT_DIR/.build-cache（沙箱模式）"
fi

bash "$SCRIPT_DIR/build.sh" >/dev/null
"$BIN_DIR/make-fixtures" "$FIXTURE_DIR" 2>/dev/null

echo "机器：$(sysctl -n machdep.cpu.brand_string)"
echo "系统：$(sw_vers -productName) $(sw_vers -productVersion)（$(sw_vers -buildVersion)）"
echo "架构：$(uname -m)"
echo "Swift：$(swift --version 2>&1 | head -1)"
echo "次数：每档 $RUNS 次（另有 1 次预热不计入）"
echo

python3 - "$HELPER" "$FIXTURE_DIR" "$RUNS" <<'PYTHON'
import json
import pathlib
import statistics
import subprocess
import sys
import time

helper = sys.argv[1]
fixture_dir = pathlib.Path(sys.argv[2])
runs = int(sys.argv[3])

targets = [
    ("bench-1920x1080.png", "1920x1080"),
    ("bench-3840x2160.png", "3840x2160"),
]

print("| 分辨率 | 次数 | elapsedMs 中位数 | elapsedMs 区间 | 墙钟中位数（含进程启动） | 识别行数 |")
print("|---|---|---|---|---|---|")
raw = []
for file_name, label in targets:
    image_path = fixture_dir / file_name
    subprocess.run([helper, str(image_path)], capture_output=True, text=True, check=True)
    inner = []
    wall = []
    line_count = 0
    for _ in range(runs):
        started = time.perf_counter()
        completed = subprocess.run([helper, str(image_path)], capture_output=True, text=True, check=True)
        wall.append((time.perf_counter() - started) * 1000.0)
        payload = json.loads(completed.stdout)
        inner.append(payload["elapsedMs"])
        line_count = len(payload["lines"])
    print(
        f"| {label} | {runs} | {statistics.median(inner):.0f} | "
        f"{min(inner)}–{max(inner)} | {statistics.median(wall):.0f} ms | {line_count} |"
    )
    raw.append((label, inner, wall))

print()
for label, inner, wall in raw:
    print(f"{label} 各次 elapsedMs：{inner}")
    print(f"{label} 各次墙钟 ms：{[round(value, 1) for value in wall]}")
PYTHON
