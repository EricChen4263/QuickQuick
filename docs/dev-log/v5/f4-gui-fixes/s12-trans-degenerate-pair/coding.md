---
id: s12-trans-degenerate-pair
title: 翻译同语种退化对兜底（zh→zh 修复）
status: done
commit: pending
date: 2026-06-03
---

## 根因

主翻译页（含剪贴板「一键翻译」跳转）翻译**中文**文本时报「翻译失败，请稍后重试」。

真实因果链（已用真实 API 证实）：
1. `src/panels/translate/TranslatePage.tsx` 默认 `targetLang = "zh"`，且 `handleTranslate` 总把它**显式**传后端（`translateText(text, targetLang, sourceParam)`）。
2. 后端 `src-tauri/src/translate/lang.rs` 的 `resolve_direction_with_source`：显式 `configured_target=Some("zh")` 覆盖了智能双向默认（原 `unwrap_or(default_target)`）。
3. 中文「接受」检测为 zh + 目标被钉死 zh → **zh→zh 退化对**。
4. 一键翻译把中文剪贴板内容自动喂入，必然撞上；手动翻中文留默认目标也一样。
5. 对照 trans-popover 路径（`translateText(text)` 不传 target）→ 后端走智能默认 zh→en → 成功，所以历史里都是成功的 zh→en。

curl 实证：

| langpair | 结果 |
|---|---|
| `zh-CN\|zh-CN` | **403 "PLEASE SELECT TWO DISTINCT LANGUAGES"** → 后端解析非 200 → Err → 前端「翻译失败」 |
| `zh-CN\|en` | 200 → "Accept the Invite" |

这是 f5「方向栏改下拉框 + 显式源语」（commit 5adfa41）引入的回归：主翻译页改为总传显式 target 后，打死了原本的智能双向默认。

## 修法（后端兜底守卫，用户确认只改后端）

源语==目标语的退化对本就无意义、任何 provider 都拒绝。在后端定方向处加守卫：**解析出的 source==target 时回退 `default_target`**（zh→en、其余→zh）。

`default_target` 按构造恒 ≠ source（source=="zh"→en；其余→zh，而其余≠zh），故回退后必得「两种不同语言」，不会再产生退化对。最小、覆盖所有调用方、且不影响任何当前能正常工作的翻译（只改原本必失败的 X→X）。

## 改动文件

### `src-tauri/src/translate/lang.rs`（唯一改动文件）

两处定方向函数把 `let target = configured_target.unwrap_or(default_target);` 改为守卫 match：

```rust
// 守卫：configured_target 只在与 source 不同时采用，否则回退 default_target。
// 源==目标无法翻译（provider 报需两种不同语言，已 curl 证实 MyMemory 403）。
let target = match configured_target {
    Some(t) if t.as_str() != source.as_str() => t,
    _ => default_target,
};
```

- `resolve_direction_with_source`（生产路径，`translate_text_impl` 实际调用）：加守卫 + 更新 doc 注释。
- `resolve_direction`（旧变体）：经 grep 确认**生产代码零调用、仅测试用**；为规范一致、防未来误用，同样加守卫 + 1 条测试。未做 DRY 委托重构（保持改动最小）。

新增 6 个单测（核心 bug 场景 + 显式同语种 + 不过度修正的反例）。

## TDD 红绿过程

**RED**：先加 6 个测试（守卫未实现），`cargo test resolve_direction` → `FAILED. 7 passed; 4 failed`，失败为断言值不符（`left: "zh" right: "en"` 等），属功能未实现。

**GREEN**：两处实现守卫后 → `11 passed`（4 新 + 7 既有全绿）。

## 实跑测试输出摘要

```
# resolve_direction 专项
cargo test -p quickquick resolve_direction: 11 passed

# 全量
cargo test -p quickquick: 317 passed; 0 failed

# 格式 / lint
cargo fmt -p quickquick --check: exit 0
cargo clippy -p quickquick: exit 0（无新增 warning）
```

## 已知边界 / 后续

- **变体漏网（理论边界，主路径不可达）**：守卫在 `normalize_zh_variant` 归一**之前**用字面量比较。若调用方直接传 `source="zh-CN"` + `target="zh"`，`"zh-CN" != "zh"` 为真→守卫不介入→经 provider 映射归一后仍可能组成 `zh-CN|zh-CN`。但前端 `languages.ts` 下拉选项全为短码（`"zh"`/`"en"`/`"ja"`…），不含变体，故主路径不触发。标为已知边界，未来若扩 locale 选项需同步处理。
- **[I-01] DRY（reviewer 提，置信度 82，非阻塞）**：`resolve_direction` 与 `resolve_direction_with_source` 守卫代码重复。因前者生产零调用，建议后续小功能改为委托 `resolve_direction(text, t) = resolve_direction_with_source(text, None, t)`，集中维护守卫逻辑。本次保持最小范围未做。
