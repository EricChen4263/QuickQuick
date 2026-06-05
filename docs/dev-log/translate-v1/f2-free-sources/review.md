---
id: TV1-F2-S01-review
type: review
level: 小功能
parent: TV1-F2
created: 2026-06-06T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [TV1-F2-A01]
author: code-reviewer
---

# TV1-F2-S01 审查留痕：Google 免费翻译源（google_free）

## 一、审查范围与依据

- 改动文件：`src-tauri/src/translate/providers.rs`（新增 `GoogleFreeProvider` + 注册）、`src-tauri/src/translate/lang.rs`（新增 `map_for_google_free`）、`src-tauri/tests/translate.rs`（同步 2 个旧测试）。
- 对照设计文档：`docs/design/translation-sources-pot.md`（§〇 许可红线、§二.2.1 Google 免费）。
- 对照验收标准：`TV1-F2-A01`（Google 免费源 build_request/parse_response 正确）、`TV1-A-SEC`（安全：免 key 源不读凭据、日志不打敏感信息）。
- 规范依据：`code-standards` skill + `code-general.md`（函数≤50行、嵌套≤3层、注释写 why、无 TODO/FIXME、安全红线）。

## 二、审查维度逐项核查

### 2.1 许可合规（GPL-3.0 红线）

- `GoogleFreeProvider` 实现为原创 Rust，与 pot 的 JavaScript 实现表达完全不同。
- providers.rs 注释（第 144-153 行）明确标注：按 Google translate_a/single 公开接口协议事实独立实现，不参考任何第三方源码，未引用 pot 的任何 URL 或代码路径。
- 未发现近似 pot 源码结构的模式（pot 使用 JS fetch/async 模式，本实现为 Rust trait 三件职责的薄层结构）。
- 结论：GPL-3.0 许可红线合规，未抄 pot 代码。

### 2.2 parse_response 拼接逻辑与错误处理

- `parse_response`（第 198-221 行）共 23 行，函数内最深嵌套 2 层（for 循环内 ok_or_else），符合规范。
- 错误路径全覆盖：非法 JSON、顶层非数组（v[0].as_array() 返回 None）、segments 为空、分句第 0 元素非字符串，四路均映射到 `TranslateError::ParseError`，无 unwrap/expect/panic。
- 拼接逻辑：取 v[0] 作 segments 数组，遍历各 segment 取 segment[0].as_str() 拼接，与设计文档§二.2.1「result[0][*][0] 拼接」完全对应。
- 结论：parse_response 正确，错误处理完整，无 panic 路径。

### 2.3 命名与不越界

- 新源 id=`google_free`（needs_key=false），与既有 `google`（needs_key=true）id 完全不同。
- `registry()` 中 lingva 仍为首位，google_free 排第二，既有源顺序/实现未被改动。
- `build_provider` match 中 "google_free" 臂独立，与 "google" 臂互不干扰。
- diff 确认既有 google/deepl_free/baidu/lingva 实现行为无变化。
- 结论：命名无冲突，既有实现未被影响。

### 2.4 修复后旧测试是否仍有判别力（非恒真）

- `static_registry_lists_five_providers`：断言 `providers.len() == 5`（精确等于），若误删源即失败，非恒真。
- `static_registry_keyed_providers_need_key`：维护免 key 集合 `["lingva","google_free"]`，对全部 provider 双向约束——集合内断言 needs_key=false，集合外断言 needs_key=true。若新增免 key 源忘记补集合，外层断言会立即红（needs_key=false != true），判别力更强于原来的"非 lingva 即需 key"逻辑。
- 结论：两个旧测试修复后判别力充分，改动合理，非被改弱成恒真。

### 2.5 安全（TV1-A-SEC）

- providers.rs 全文无 eprintln/println/log/tracing 输出，不打印待译文本或译文。
- `build_provider("google_free", credentials)` 直接 `Ok(Box::new(GoogleFreeProvider::new()))` 返回，credentials 切片未被读取，完全不访问凭据存储。
- `needs_key: false` 标注正确，UI 层据此不触发凭据输入流程。
- 结论：TV1-A-SEC 满足。

### 2.6 代码规范（函数大小/嵌套/注释/清洁度）

- `build_request`：14 行，嵌套 1 层。`parse_response`：23 行，嵌套 2 层。`map_for_google_free`：8 行，嵌套 1 层。全部合规（≤50 行，≤3 层）。
- 注释写 why：「实测 Google 不在分句间补空格」解释拼接不补分隔符的原因；「独立成函数以便两源各自演进」解释 map_for_google_free 不复用 map_for_google 的理由。
- 无装饰性分隔注释、无死注释、无 TODO/FIXME（grep 确认）。
- coding.md 无异常工具标签残留，格式正常。
- 结论：代码质量合规。

## 三、高置信度问题清单

经逐项核查，未发现置信度 ≥80 的问题。

- Critical（影响正确性/安全）：无
- Important（建议改、不阻塞）：无

## 四、审查结论

TV1-F2-S01 改动符合项目规范与 code-standards 要求：许可合规（无 pot 代码）、实现原创、错误处理完整、命名无冲突、安全约束满足、旧测试判别力有效保留。

**APPROVE**
