#!/usr/bin/env bash
# 逐张夹具校验 helper 的识别结果：真值来自 make_fixtures.swift 导出的 expected.json。
# 断言口径：期望行在「空白归一化」后必须出现在识别全文里；缺口如实打印，不放宽断言。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$SCRIPT_DIR/bin"
FIXTURE_DIR="$SCRIPT_DIR/fixtures"
HELPER="$BIN_DIR/ocr-helper"
GENERATOR="$BIN_DIR/make-fixtures"

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
"$GENERATOR" "$FIXTURE_DIR"

python3 - "$HELPER" "$FIXTURE_DIR" <<'PYTHON'
import difflib
import json
import pathlib
import subprocess
import sys

helper = sys.argv[1]
fixture_dir = pathlib.Path(sys.argv[2])


def normalize(text):
    # 只归一化空白：OCR 可能在句中断行，但字符本身必须对得上，因此不做模糊匹配。
    return " ".join(text.split())


def char_error_rate(expected_lines, recognized_text):
    # 字符级错误率 = (替换 + 删除 + 插入) / 期望字符数，用 difflib 的匹配块反推。
    # 行级命中率看不出「差一个字」，这个指标才反映识别完整度。
    expected = normalize("\n".join(expected_lines))
    observed = normalize(recognized_text)
    matched = sum(
        block.size
        for block in difflib.SequenceMatcher(None, expected, observed).get_matching_blocks()
    )
    if not expected:
        return 0.0
    return max(0.0, (len(expected) - matched) / len(expected))


def run_helper(image_path):
    completed = subprocess.run([helper, str(image_path)], capture_output=True, text=True)
    return completed


failures = 0
print("| 图片 | 尺寸 | 期望行 | 命中行 | 行命中率 | 字符错误率 | elapsedMs | ok |")
print("|---|---|---|---|---|---|---|---|")

summary = []
for fixture in json.loads((fixture_dir / "expected.json").read_text(encoding="utf-8"))["fixtures"]:
    image_path = fixture_dir / fixture["image"]
    completed = run_helper(image_path)
    if completed.returncode != 0:
        print(
            f"| {fixture['image']} | - | {len(fixture['lines'])} | - | - | - | - "
            f"| 退出码 {completed.returncode} |"
        )
        print(f"  失败输出：{completed.stdout.strip()} {completed.stderr.strip()}")
        failures += len(fixture["lines"])
        continue

    payload = json.loads(completed.stdout)
    if not payload.get("ok"):
        print(
            f"| {fixture['image']} | - | {len(fixture['lines'])} | - | - | - | - | ok=false |"
        )
        print(f"  失败输出：{payload}")
        failures += len(fixture["lines"])
        continue

    recognized_text = normalize(payload["text"])
    recognized_lines = [
        (normalize(line["text"]), line["confidence"]) for line in payload["lines"]
    ]
    hits = []
    misses = []
    for expected in fixture["lines"]:
        if normalize(expected) in recognized_text:
            hits.append(expected)
        else:
            misses.append(expected)
    hit_rate = len(hits) / len(fixture["lines"]) if fixture["lines"] else 1.0
    error_rate = char_error_rate(fixture["lines"], payload["text"])
    print(
        f"| {fixture['image']} | {fixture['width']}x{fixture['height']} | {len(fixture['lines'])} "
        f"| {len(hits)} | {hit_rate:.0%} | {error_rate:.1%} | {payload['elapsedMs']} | true |"
    )
    summary.append((fixture["image"], len(hits), len(fixture["lines"]), payload["elapsedMs"]))

    for expected in misses:
        failures += 1
        best_ratio = -1.0
        best_line = ""
        best_confidence = 0.0
        for candidate, candidate_confidence in recognized_lines:
            ratio = difflib.SequenceMatcher(None, normalize(expected), candidate).ratio()
            if ratio > best_ratio:
                best_ratio = ratio
                best_line = candidate
                best_confidence = candidate_confidence
        print(f"  未命中期望行：{expected}")
        print(
            f"    最接近的识别行（相似度 {best_ratio:.2f}，置信度 {best_confidence:.2f}）：{best_line}"
        )

print()
print("契约与失败路径校验：")
checks = []


def check(name, condition, detail):
    checks.append((name, condition, detail))
    print(f"  {'PASS' if condition else 'FAIL'} {name}：{detail}")


# 1. 成功输出必须是合法 JSON 且字段齐全
first_image = fixture_dir / json.loads((fixture_dir / "expected.json").read_text(encoding="utf-8"))["fixtures"][0]["image"]
completed = run_helper(first_image)
check("stdout 是单行 JSON", completed.stdout.count("\n") == 1, f"换行数 {completed.stdout.count(chr(10))}")
payload = json.loads(completed.stdout)
check(
    "成功字段齐全",
    set(payload.keys()) == {"ok", "text", "lines", "elapsedMs"},
    f"键 {sorted(payload.keys())}",
)
first_line = payload["lines"][0]
check(
    "lines 字段齐全且 box 为 4 元素",
    set(first_line.keys()) == {"text", "confidence", "box"} and len(first_line["box"]) == 4,
    f"line 键 {sorted(first_line.keys())} box 长度 {len(first_line['box'])}",
)
check(
    "box 全部分量落在 [0,1]",
    all(0.0 <= value <= 1.0 for line in payload["lines"] for value in line["box"]),
    f"{len(payload['lines'])} 行",
)
check(
    "confidence 落在 [0,1]",
    all(0.0 <= line["confidence"] <= 1.0 for line in payload["lines"]),
    f"{len(payload['lines'])} 行",
)

# 2. 文件不存在：必须 ok=false 且退出码 3
missing = subprocess.run([helper, str(fixture_dir / "no-such-file.png")], capture_output=True, text=True)
missing_payload = json.loads(missing.stdout)
check(
    "不存在的文件报错退出 3",
    missing.returncode == 3 and missing_payload.get("ok") is False,
    f"退出码 {missing.returncode}，error={missing_payload.get('error')}",
)

# 3. 参数缺失：必须 ok=false 且退出码 2
usage = subprocess.run([helper], capture_output=True, text=True)
usage_payload = json.loads(usage.stdout)
check(
    "缺少参数报错退出 2",
    usage.returncode == 2 and usage_payload.get("ok") is False,
    f"退出码 {usage.returncode}，error={usage_payload.get('error')}",
)

# 3.5 契约要求的三类格式都必须能读：PNG 已由夹具覆盖，这里补 JPEG 与 TIFF（用系统 sips 转换）
english_fixture_lines = next(
    item["lines"]
    for item in json.loads((fixture_dir / "expected.json").read_text(encoding="utf-8"))["fixtures"]
    if item["image"] == "en-only.png"
)
for sips_format, extension in (("jpeg", "jpg"), ("tiff", "tiff")):
    converted_path = fixture_dir / f"converted.{extension}"
    conversion = subprocess.run(
        ["sips", "-s", "format", sips_format, str(first_image), "--out", str(converted_path)],
        capture_output=True,
        text=True,
    )
    if conversion.returncode != 0 or not converted_path.exists():
        print(f"  SKIP {sips_format.upper()} 校验：sips 转换失败（{conversion.stderr.strip()[:120]}）")
        continue
    outcome = run_helper(converted_path)
    outcome_payload = json.loads(outcome.stdout)
    outcome_text = normalize(outcome_payload.get("text", "")) if outcome_payload.get("ok") else ""
    matched_lines = sum(1 for line in english_fixture_lines if normalize(line) in outcome_text)
    check(
        f"{sips_format.upper()} 格式可读且全部英文行命中",
        outcome.returncode == 0 and outcome_payload.get("ok") is True and matched_lines == len(english_fixture_lines),
        f"退出码 {outcome.returncode}，{matched_lines}/{len(english_fixture_lines)} 行命中",
    )

# 4. 不支持的格式（真实 HEIC）：必须明确拒绝，不能解出来就往下走
heic_path = fixture_dir / "unsupported.heic"
converted = subprocess.run(
    ["sips", "-s", "format", "heic", str(first_image), "--out", str(heic_path)],
    capture_output=True,
    text=True,
)
if converted.returncode == 0 and heic_path.exists():
    heic = subprocess.run([helper, str(heic_path)], capture_output=True, text=True)
    heic_payload = json.loads(heic.stdout)
    check(
        "HEIC 被明确拒绝并退出 3",
        heic.returncode == 3 and heic_payload.get("ok") is False and "HEIC" in heic_payload.get("error", "").upper(),
        f"退出码 {heic.returncode}，error={heic_payload.get('error')}",
    )
else:
    print(f"  SKIP HEIC 拒绝校验：sips 转换失败（{converted.stderr.strip()[:120]}）")

# 5. --capabilities 诊断可用
captured = subprocess.run([helper, "--capabilities"], capture_output=True, text=True)
capability_payload = json.loads(captured.stdout)
check(
    "--capabilities 输出支持语言",
    captured.returncode == 0
    and capability_payload.get("ok") is True
    and "zh-Hans" in capability_payload.get("accurateLanguages", []),
    f"当前 revision {capability_payload.get('currentRevision')}，"
    f"{len(capability_payload.get('accurateLanguages', []))} 种语言，"
    f"含 zh-Hans={'zh-Hans' in capability_payload.get('accurateLanguages', [])}",
)
for requested in ("zh-Hans", "zh-Hant", "en-US"):
    present = requested in capability_payload.get("accurateLanguages", [])
    check(f"契约语言 {requested} 本机可用", present, f"present={present}")

failed_checks = [name for name, condition, _ in checks if not condition]
print()
print(f"夹具总行数：{sum(item[2] for item in summary)}，命中行数：{sum(item[1] for item in summary)}")
print(f"未命中行数：{failures}，契约校验失败项：{len(failed_checks)}")
print("退出码：0 全绿 / 1 契约或失败路径校验有问题（脚本或 helper 的缺陷）/ 2 仅识别命中不足（已知质量缺口）")
if failed_checks:
    sys.exit(1)
sys.exit(2 if failures else 0)
PYTHON
