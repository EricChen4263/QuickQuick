---
id: RT1-REL-review
type: review
level: 小功能
parent: RT1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: []
author: code-reviewer
---

# 审查报告：发版流程改动（编码修复 + Release 正文组装 + CHANGELOG 门禁）

## 审查范围

- `src-tauri/tools/gen_ecdict_db.py`：Windows stdout 编码修复（`reconfigure`）
- `src-tauri/tools/test_gen_ecdict_db.py`（新）：编码修复回归测试
- `.github/workflows/release.yml`：PYTHONUTF8 env + Release 正文组装步骤 + `releaseBody` 引用
- `.github/release-install-guide.md`（新）：分语言安装指南
- `scripts/release.sh`：CHANGELOG 门禁新增
- `scripts/release.test.sh`（新）：门禁正负向 bash 测试
- `CHANGELOG.md`（新）：v0.3.0 段

## Critical 问题

无。

## Important 问题

### I-01：release.sh 门禁正则与 release.yml awk 匹配标准不一致

- severity: Important · confidence: 85
- file: `scripts/release.sh:90` + `.github/workflows/release.yml:88`
- evidence: `release.sh` 门禁用 `grep -qE "^## v${VERSION//./\\.}([[:space:]]|$)"` 允许 header 带空格后缀（如 `## v0.3.0 2026-06-01`），但 `release.yml` awk 用精确等价 `$0 == tag` 匹配，后缀存在时等价失败 → awk 抽取返回空 → RELEASE_BODY 退化为占位串（"本版未在 CHANGELOG.md 找到..."），Release 正文只剩安装指南，更新内容丢失。门禁的设计目的（保证 Release 能抽到内容）在此情形下失效。
- 当前状态：现有 `CHANGELOG.md` 格式为无后缀 `## v0.3.0`，本版不受影响；但门禁与 awk 语义不对齐，未来版本存在踩坑风险。
- 修复建议：将 awk 精确等价改为前缀匹配，与门禁标准对齐：
  ```awk
  # 改前：
  $0 == tag { capture = 1; next }
  # 改后（允许 header 带后缀）：
  index($0, tag) == 1 && (length($0) == length(tag) || substr($0, length(tag)+1, 1) ~ /[[:space:]]/) { capture = 1; next }
  ```
  或更简洁：将 tag 变量仅设为 `v${tag}` 前缀，改用 `/^## ` + 版本号正则匹配。

## 合规确认

### release.yml YAML 结构

- YAML 经 `yaml.safe_load` 解析验证，10 个步骤顺序正确：步骤 [8]「组装 Release 正文」在步骤 [9] tauri-action 之前，`${{ env.RELEASE_BODY }}` 引用时序正确。
- `env: PYTHONUTF8: "1"` 在步骤级别生效，不污染其他步骤，符合最小作用域原则。
- `shell: bash` 已指定，Windows 上走 Git Bash（内置 GNU awk），跨平台可用。
- `releaseBody: ${{ env.RELEASE_BODY }}`：YAML 解析为字符串类型；GitHub Actions runner 在 with: 值里做表达式展开，多行字符串在此是已知支持的标准模式，无风险。
- heredoc 定界符 `__RELEASE_BODY_EOF__` 在 Markdown 发布内容出现概率极低，无注入面。

### awk 抽取逻辑

- `capture && /^## / { exit }` 仅匹配两级标题，`###` 三级标题（`### 🌐 翻译` 等）不触发终止，CHANGELOG 内部 `---` 分隔线作为内容正常 capture，逻辑正确。
- 首尾空行裁剪逻辑（`start`/`end` 游标）正确。
- 空 changelog 回退占位串保证 RELEASE_BODY 非空，Release 正文不会因抽取失败而崩溃。
- 对现有 `CHANGELOG.md`（`## v0.3.0` 无后缀）实测抽取 25 行，内容完整。

### gen_ecdict_db.py 编码修复

- `if hasattr(sys.stdout, "reconfigure")` 守卫合理：`reconfigure` 在 Python 3.7+ 的真实 `sys.stdout` 上可用；`io.TextIOWrapper` 同样支持（经测试验证）。守卫仅影响 stdout 编码，不改变 db 写入逻辑，无副作用。
- `PYTHONUTF8: "1"` 与 `reconfigure` 双保险：PYTHONUTF8 在 Python 启动时生效（影响所有 I/O），reconfigure 是运行时补丁，两者不冲突。
- 注释写"为什么"（Windows cp1252 崩溃根因），符合注释规范。

### test_gen_ecdict_db.py

- `MainStdoutEncodingTest`：用 `io.TextIOWrapper(io.BytesIO(), encoding="cp1252")` 替换 `sys.stdout`，`importlib.reload` 触发模块顶层 `reconfigure`，断言 `"已写入".encode("utf-8")` 在底层 buffer 中，逻辑正确、验证有效。
- `BuildDbTest`：3 行 CSV 输入（2 有效 + 1 空 translation），断言写入 2 行，跳过逻辑覆盖充分。
- 无 TODO/FIXME，无装饰性分隔注释，符合 `code-general` 规范。

### release.sh 门禁

- 正则 `^## v${VERSION//./\\.}([[:space:]]|$)` 点号已转义，`v0.3.0` 不误匹配 `v0.30.0`；`([[:space:]]|$)` 边界防止 `v0.3.00` 误匹配——验证正确。
- die 信息含可操作指引（"发版前先在 $CHANGELOG 写 v$VERSION 更新内容"）。
- `readonly CHANGELOG` 命名符合常量规范。

### release.test.sh

- 负向用例（缺 CHANGELOG 段 → die 退非零）+ 正向护栏（补段后 dry-run exit 0），两用例设计完整，满足"正负向"双覆盖要求。
- `make_repo` 构造最小可发版仓库（main 分支、工作树干净、版本低于目标），隔离测试环境，无依赖外部状态。
- 无 TODO/FIXME，无装饰性分隔注释。

### release-install-guide.md

- 内容与原 inline `releaseBody` 等价：macOS 三步骤 + 权限说明、Windows 两步骤、默认热键均保留；中文段在前、English 段在后，格式一致。
- 无密钥硬编，无安全风险。

### CHANGELOG.md

- 格式符合文件头注释约定（中文段 → `---` → English 段，分语言成段）。
- `## v0.3.0` 无后缀，awk 精确等价匹配有效（当前版本）。

## 结论

**WARNING**

无 Critical 问题，发版流程功能完整，可合入。

存在一个 Important 非阻塞建议（I-01）：`release.sh` 门禁正则允许带后缀的 header，而 `release.yml` awk 用精确等价匹配——两者标准不一致。当前 `CHANGELOG.md` 格式无后缀，本版不受影响；建议在下次修改 awk 段时将精确等价改为前缀匹配以消除隐患。
