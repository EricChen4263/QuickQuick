---
id: RT1-F1-S02-review
type: review
level: 小功能
parent: RT1-F1
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F1-A02]
evidence: []
author: code-reviewer
---

# 审查结论 · 捕获层读 HTML + 变化检测纳入 html（RT1-F1-S02）

## 审查范围

| 文件 | 改动说明 |
|---|---|
| `src-tauri/src/pipeline.rs` | 新增纯函数 `composite_hash_bytes`；`compute_composite_hash` 委托它并加入 `cb.get().html().ok()`；`read()` 填充 `html` 字段（原硬编码 `None`） |
| `src-tauri/src/clipboard.rs` | 新增 `snapshot_to_clips_for_test`（`#[doc(hidden)]` 测试导出，沿用既有约定） |
| `src-tauri/tests/richtext_capture.rs`（新增）| `composite_hash_differs_when_html_differs`、`snapshot_to_clips_propagates_html` 集成测试 |

参照标准：项目规范 + code-standards（§1 通用原则 / §3 函数 / §5 注释 / §6 类型 / §8 测试 / §10 安全）+ 项目既有约定（注释写"为什么"、持久化哈希用显式 FNV-1a、测试断言验具体值非恒真、禁装饰性分隔注释）。

---

## 审查维度逐项核对

### 1. 哈希拼接确定性与分隔符防碰撞（通过）

拼接顺序固定：`text bytes` + `0xFF` + `html bytes` + `0xFF` + `image RGBA bytes`。

`0xFF` 分隔符不是合法 UTF-8 字节（UTF-8 编码中 `0xFF` 不独立出现），因此 `text` 和 `html` 段内部不可能包含 `0xFF`，跨段混淆不会发生。`image` 为最后一段（无后续），RGBA 字节中出现的 `0xFF` 不会与后续段产生歧义。防碰撞设计有效，注释亦说明了"相邻段间 0xFF 分隔符避免 ab+'' 与 a+b 碰撞"的理由。**通过**。

### 2. `composite_hash_bytes` 可见性：`pub` 合理性（通过）

函数在 `pipeline.rs` 内部由 `compute_composite_hash` 调用，同时在集成测试 `tests/richtext_capture.rs` 中通过 `use quickquick_lib::pipeline::composite_hash_bytes` 引入。集成测试属于 crate 外部消费者，`pub(crate)` 不可见于 `tests/` 目录。`pub` 是集成测试可见的最低必要可见性，不存在过度暴露问题（函数纯函数、无副作用、不暴露内部状态）。**通过**。

### 3. `get().html().ok()` 降级语义（通过）

arboard `get()` 每次返回全新一次性 builder，`.html()` 返回 `Result<String, Error>`；剪贴板无 HTML 格式时返回 `Err`（`ContentNotAvailable` 等），`.ok()` 转 `None` 不 panic。注释说明"get() 是一次性 builder，无 HTML 时返回 Err，用 .ok() 降级为 None 不 panic"。语义正确，降级路径安全。**通过**。

### 4. None vs Some("") 边界碰撞（观察，不阻塞）

tester 识别的边界：`html=None` 与 `html=Some("")` 产生相同拼接字节序列，故哈希相同，`change_count` 无法区分二者。

实际可达性分析：arboard `get().html()` 在剪贴板没有 HTML 格式时返回 `Err`（不返回 `Ok("")`）；浏览器与富文本编辑器从不写入空字符串 HTML。生产路径中 `Some("")` 不会由 `cb.get().html().ok()` 产生，理论碰撞无实际触发场景。

即使极端情况下产生 `Some("")`，后果仅为"空 HTML 变化无法触发 change_count 递增"，而空字符串 HTML 本身不含可保真格式信息，对数据正确性无影响。

**定性：置信度 30（低于报告阈值 80），归观察，不要求修改。**

### 5. read() 三字段读取逻辑（通过）

`get_text()`、`get().html()`、`get_image()` 三次独立调用，每次 `get()` 产生全新 builder，无共享可变状态。三段读取相互独立，正确填充 `ClipboardSnapshot.text/.html/.image`。**通过**。

### 6. FNV 常量跨模块一致性（通过，技术债观察）

`pipeline.rs` 顶层 `const FNV_PRIME` / `FNV_OFFSET` 与 `db.rs` `text_hash` 函数内的同名局部常量值完全一致（均为 `0x0000_0100_0000_01B3` / `0xcbf2_9ce4_8422_2325`），符合"持久化哈希用显式稳定算法 FNV-1a"约定。`db.rs` 的常量定义在私有函数内部，Rust 可见性不支持直接跨模块复用，两处独立声明是当前唯一可行方式，无错误风险。**通过**。

### 7. `snapshot_to_clips_for_test` 约定一致性（通过）

沿用既有 `rgba_to_png_for_test` 命名约定：`#[doc(hidden)]` + `pub fn xxx_for_test` 薄包装私有函数，注释说明"仅供集成测试调用，不用于生产路径"。额外加 `#[must_use]` 属于合理加强（`rgba_to_png_for_test` 无此属性，但返回值忽略同样无意义），不违规。**通过**。

### 8. 代码规范（通过）

- **函数长度**：`composite_hash_bytes`（24 行）、`compute_composite_hash`（7 行）、`read()` 增量（约 12 行）均 ≤ 50 行。
- **注释**：`composite_hash_bytes` 文档注释写"为什么把 html 纳入"（"同纯文本但新增/变更了 html 需触发 change_count 递增"）；`compute_composite_hash` 注释写"实际哈希组合委托给纯函数"设计意图；`read()` 内联注释写"get() 是一次性 builder，无 HTML 时返回 Err"原理。符合"注释写为什么"规范。
- **无 TODO/FIXME/装饰性分隔注释**：全文无发现。
- **`#[must_use]`**：`composite_hash_bytes` 加了 `#[must_use]`，防止调用方忽略返回值，规范合理。
- **命名**：函数名描述性，符合"动词+名词"风格。

**通过**。

### 9. 测试质量（通过）

| 测试名 | 验证内容 | 断言质量 |
|---|---|---|
| `composite_hash_differs_when_html_differs` | plain≠bold、bold≠italic（html 变化触发不同哈希）；bold==bold_again（确定性）；全 None→None | 具体值 `assert_ne!` / `assert_eq!`，非恒真 |
| `snapshot_to_clips_propagates_html` | snapshot.html 原样透传到 CapturedItem.html | 具体值 `assert_eq!(item.html, Some("<b>hello world</b>"))` |

两个测试直接覆盖验收项 RT1-F1-A02 的两个核心断言。断言均为具体行为验证，不存在恒真风险。AAA 结构清晰，测试名描述行为。**通过**。

---

## 发现问题（置信度 ≥ 80 才报）

**无置信度 ≥ 80 的问题。**

以下为置信度 < 80 的观察，供参考，不阻塞：

| 置信度 | 严重度 | 位置 | 描述 |
|---|---|---|---|
| 30 | 观察 | `src-tauri/src/pipeline.rs:115-127` | `html=None` 与 `html=Some("")` 产生相同哈希。arboard 实际不返回 `Ok("")`，生产路径无触发场景；即使触发，后果仅为空 HTML 无法触发 change_count 递增，数据正确性不受影响。不要求修改。 |
| 40 | 观察 | `pipeline.rs:26-28` vs `db.rs:899-900` | `FNV_PRIME`/`FNV_OFFSET` 两处独立声明，值一致。`db.rs` 定义在私有函数体内，无法跨模块复用；当前无错误风险，长期可考虑将常量提升至模块级并设 `pub(crate)` 统一引用，但超出本次范围。 |

---

## 是否合规

符合项目规范与 code-standards 的全部核查维度：

- `composite_hash_bytes` 拼接顺序固定、`0xFF` 分隔符防碰撞有效
- 可见性 `pub` 是集成测试可达的最低必要可见性，不过度暴露
- `get().html().ok()` 降级语义正确，不 panic
- `read()` 三段读取相互独立，正确填充快照
- 函数 ≤ 50 行，注释写"为什么"，无 TODO/FIXME/装饰性注释
- `snapshot_to_clips_for_test` 沿用既有测试导出约定，风格一致
- 测试断言验具体值，无恒真风险

---

## 结论

**通过（APPROVE）**

核心改动正确，无置信度 ≥ 80 的 Critical 或 Important 问题。两条低置信度观察均不阻塞。

---

**VERDICT: APPROVE**
