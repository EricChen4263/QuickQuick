---
id: V5-F4-S03-review
type: review
level: 小功能
parent: V5-F4
children: []
created: 2026-06-02T00:00:00Z
status: 通过
commit: 6f7ab78
acceptance_ids: []
evidence: []
author: code-reviewer
---

# 审查结论 · 单张图片阈值 规范审查

## 审查维度

项目规范（CLAUDE.md）+ code-standards（格式/命名/函数/注释/类型/性能/测试/安全）。
覆盖后端 d60fd93 与前端 6f7ab78 两段 diff。

## 发现问题（置信度 ≥ 80 才报）

| 严重度 | 问题 | 文件:行 | 规范依据/修复建议 |
|---|---|---|---|
| Important | 轮询注释与实际行为不符（见下）| `src-tauri/src/lib.rs:232-242` | 注释写"ingest 只在真正捕获到新图时发生，读文件开销可接受"，但实际上 `load_or_default` 在 `poll_once_with_policy` 调用之前执行，每 500ms 无条件读一次 settings.json，与"只在 ingest 时读"表述不符。误导性注释违反"注释写为什么"规范，且对未来维护者隐藏了真实 I/O 频率。修复：将注释改为"每轮轮询（500ms）读一次 settings.json（<1KB）；桌面端频率可接受，如需降频可改为仅 ingest 路径内读取或用 AtomicU64 缓存"，如实描述行为。 |

### 第 4 点（轮询读配置开销）专项判断

**结论：注释有误导，但实际性能可接受；不属于高危，属于 Important 级别。**

- 每 500ms 调用一次 `AppSettings::load_or_default`（`fs::read_to_string` + `serde_json::from_str`），settings.json 通常 <1KB，桌面端此频率对磁盘/CPU 影响极小。
- 与同轮已有的 `RwLock::read()`（排除名单）、`Ordering::Relaxed` 原子读相比，额外文件读属同量级运行时开销，不构成性能瓶颈。
- **注意**：若后续阈值频繁被用户调整（每 500ms 的读会保证立即生效），这是设计意图；若希望改为惰性加载，可在 `ingest_clips` 分支内读取，或用 `AtomicU64` 缓存并在 `set_image_threshold` 时失效。
- 当前注释误称"ingest 只在真正捕获到新图时发生，读文件开销可接受"——这在逻辑上是以"ingest 低频"为读文件做辩护，但读文件本身并不跟随 ingest 的频率，实际每轮都读；辩护逻辑有误，应如实说明。

## 各文件详细审查

### settings.rs

- `#[serde(default = "default_max_image_bytes")]` 正确，向后兼容完备。
- `Default impl` 与 `default_max_image_bytes()` 均为 `20 * 1024 * 1024`，两者严格一致。
- `u64` 类型合理，测试覆盖 legacy JSON、round-trip、Default 一致性三条路径。
- 无问题。

### ipc/settings.rs

- `MIN_IMAGE_THRESHOLD = 1 * 1024 * 1024`（1MiB）、`MAX_IMAGE_THRESHOLD = 500 * 1024 * 1024`（500MiB），区间端点含入，前后一致（`bytes < MIN || bytes > MAX` 覆盖两端）。
- 越界路径在 `load_or_default` 之前 early return，不触发文件写，符合"先校验后 load/save"要求。
- `set_image_threshold_out_of_range_does_not_modify_file` 测试明确验证此行为。
- 命令薄壳 + `_impl` 可测架构正确。
- 无问题。

### db.rs

- `max_image_bytes` 贯穿 `ingest_image_as_clip` → `insert_image_clip` → `try_insert_image_clip`，参数链传递清晰，不破坏既有去重/孤儿领养逻辑（哈希计算在阈值判断之前，`Bumped` 路径不受影响）。
- `as usize`（`max_image_bytes as usize`）：阈值上限 500MiB = 524288000 字节，远小于 32 位 usize::MAX（4294967295）；macOS/Windows/Linux 桌面均 64 位，无溢出风险，安全。
- `OversizePolicy { max_original_bytes: max_image_bytes as usize }` 构造正确。
- 新增两条阈值测试（小阈值/大阈值）覆盖核心语义。
- 无问题。

### lib.rs 轮询

- 见上方"第4点专项判断"，注释措辞有误导，需修正；行为本身可接受。
- fallback 双保险（`ok().map(...)` + `unwrap_or_else(AppSettings::default)`)结构正确，不 panic。

### ipc/clipboard.rs

- 测试调用处补传 `20 * 1024 * 1024`，与函数签名对齐，无问题。

### ipc-client.ts

- `getImageThreshold` / `setImageThreshold` 封装规范，`try/catch` 用 `toError` 统一转换，无 `any`，无 `catch promise`。
- 注释中 `524288000 = 500 * 1024 * 1024` 数值正确（实际 500MiB = 524288000，非 500MB）。
- 无问题。

### StoragePanel.tsx

- `cancelled` flag 防卸载后 `setState` 竞态，正确实现。
- `IMAGE_THRESHOLD_OPTIONS = [5, 10, 20, 50, 100]`（MiB），全在 `1..=500` 合法区间内，安全。
- `handleThresholdChange`：先调 `setImageThreshold`（后端 MB→字节：`mb * 1024 * 1024`）成功后再 `setImageThresholdMB`；失败仅 `console.error`，UI 保持旧值，无 unhandled promise。
- `Math.round(bytes / (1024 * 1024))` 读取反算 MB，与写入换算对称。
- `loadThreshold` 中 `cancelled.current` 检查顺序正确（await 后检查，再 setState）。
- `handleThresholdChange` 失败时不显示 UI 错误提示（仅 console.error），这是现有其他设置项的一致做法，不单独报告。
- 无问题。

### TranslateWorkspace.tsx

- `onTranslate: (textOverride?: string) => void` 签名扩展向后兼容，按钮 `onClick={() => onTranslate()}` 等价于原 `onClick={onTranslate}`（无参调用），不引入新问题。
- 无问题。

## 是否合规

- 后端：serde 向后兼容正确；Default 与 serde default 一致；校验先于文件操作；`as usize` 安全；无 panic；无死代码；无装饰性分隔注释；函数均 ≤50 行。
- 前端：无 `any`；无 unhandled promise；cancelled flag 防竞态；换算对称；select 预设值合法。
- 唯一偏差：`lib.rs` 注释措辞与实际行为不符，属 Important 级别，建议修正，不属于功能性缺陷。

## 结论

**审查通过（无未决高危）。** 发现 1 条 Important 级别问题（`lib.rs` 注释措辞误导），建议在下一次触碰该文件时一并修正；不阻塞本小功能交付。
