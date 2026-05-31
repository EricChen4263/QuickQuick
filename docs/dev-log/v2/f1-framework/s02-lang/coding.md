---
id: V2-F1-S02-code
type: coding_record
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T00:00:16Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A02]
evidence:
  - src-tauri/src/translate/lang.rs
  - src-tauri/src/translate/mod.rs
  - src-tauri/tests/translate.rs
author: coder
---

# 编码记录 · S02 语言归一

## 做了什么

新增 `src-tauri/src/translate/lang.rs` 子模块，实现语言归一三大能力：本地 Unicode 检测源语言、智能双向定方向、每 provider 映射表抹平 zh 变体。在 `mod.rs` 以 `pub mod lang` 暴露，集成测试追加 14 个 A02 用例，与原有 12 个 S01 用例共 26 个全绿。

## 关键决策与理由

- **CJK 区间判断而非 regex**：只需区分"含中文汉字"与"不含"，按 Unicode 区间逐字符扫描最简且无外部依赖。日文假名（U+3040–U+30FF）不在区间内，避免误判纯日文为中文，符合§4.3 智能双向的业务语义。
- **检测粒度：含即判定**：文本只要含一个 CJK 字符就判为中文输入，适合混合文本（"hello 你好"），与产品"智能双向"意图一致；若后续需阈值判断可在此函数内扩展。
- **zh 变体归一两步走**：先 `normalize_zh_variant` 把 zh/zh-CN/zh-Hans/zh-SG 都收敛到内部 "zh"，再由各 provider 映射函数各自输出期望格式（MyMemory/Google → "zh-CN"；百度 → "zh"；DeepL → "ZH"）。职责分离，新增 provider 时只加一个映射函数。
- **未知 provider 透传不 panic**：`map_lang_for_provider` 的 `_` 分支原样返回内部代码，对未来新 provider 安全兼容，避免硬崩。
- **不引入新 crate**：检测逻辑纯标准库实现，不依赖 `lingua` / `whatlang` 等，保持编译时间与依赖树稳定；S02 只要求区分中/非中文，当前精度足够。

## 改动文件

- `src-tauri/src/translate/lang.rs` — 新增，实现 `detect_is_chinese`、`detect_lang`、`resolve_direction`、`map_lang_for_provider` 及私有辅助函数
- `src-tauri/src/translate/mod.rs` — 追加 `pub mod lang;` 暴露子模块
- `src-tauri/tests/translate.rs` — 追加 A02 集成测试 14 个（lang_norm_ 前缀），更新顶部注释与 use 引入

## 自测结论（TDD 红-绿-重构）

**RED**：先在 `tests/translate.rs` 追加全部 A02 测试并引入 `translate::lang` 模块路径，运行后确认编译失败（`could not find 'lang' in 'translate'`），错误原因是功能未实现，非语法问题。

**GREEN**：创建 `lang.rs` 实现四个公开函数及私有辅助，`mod.rs` 暴露模块，重新运行 `cargo test --test translate`：26 passed / 0 failed。

**REFACTOR**：提取 `normalize_zh_variant` 避免各 provider 映射函数重复判断 zh 变体；提取 `is_cjk_char` 单字符判断供 `detect_is_chinese` 复用；各 provider 映射拆为独立私有函数（`map_for_mymemory` 等），单一职责，每函数均在 20 行以内。

**code-standards 逐项自检**：
- 格式：4 空格缩进（Rust 惯例），行宽不超 100 字符，文件末尾保留换行
- 函数：最长函数 `map_lang_for_provider` 约 15 行，全部远低于 50 行上限；嵌套不超 2 层
- 命名：`detect_is_chinese`（动词+名词+布尔语义）、`resolve_direction`、`map_lang_for_provider` 均描述性命名；无魔术字符串（区间用具名常量 `CJK_RANGES`）
- 注释：解释"为什么选此区间""为什么未知 provider 透传"等决策理由；无装饰性分隔注释（grep 验证 deco=1）
- 类型：所有公开函数显式签名，返回 `String` / `Lang` / `bool` 明确
- 安全：无密钥、无用户输入直接执行、无 panic 路径（unwrap/expect 仅在测试 arrange 阶段）
- 测试：AAA 结构、行为化命名（`lang_norm_detect_is_chinese_returns_true_for_cjk_text` 等）、非恒真、headless
- clippy：零警告（clippy=0）
- TODO/FIXME：无（grep 验证 todo=1）

## 审查打回修复记录（第 1 次，2026-05-31）

### I-1：清 tests 装饰注释
`tests/translate.rs` 第 14/50/128/206 行的 `// ──── … ────` 四处装饰横线全部去掉，改为普通 `// 段落名` 格式。`grep -rnE '──|═══|━━━' src-tauri/src src-tauri/tests` 无残留（deco=1）。

### I-2：消除 resolve_direction 重复检测
`lang.rs` 的 `resolve_direction` 原本在调用 `detect_lang(text)` 之后又直接调用 `detect_is_chinese(text)` 决定默认方向，造成二次扫描。修复为复用 `source` 的结果：`if source.as_str() == "zh"` 判断，不再额外扫描文本。

### I-3：补中英混合文本用例
在 A02 组（`detect_is_chinese` 系列用例）补 `lang_norm_detect_is_chinese_returns_true_for_mixed_text`：输入 `"hello 你好"` 应返回 `true`（§4.3：含 CJK 即判中文）。AAA 结构、非恒真。

### 回归结论
- translate：27 passed / 0 failed（含新增混合文本用例）
- clippy：0 警告（clippy=0）
- 装饰注释：deco=1（清零）
- TODO/FIXME：todo=1（无）
