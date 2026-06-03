---
id: s12-trans-degenerate-pair
title: 翻译同语种退化对兜底（zh→zh 修复）测试留痕
status: passed
commit: acdbf05
date: 2026-06-03
---

# 测试留痕：同语种退化对守卫（s12）· 动态证伪

## 开工 git status 快照

```
 M src-tauri/src/translate/lang.rs
```

---

## 一、命中校验（杀假绿）

`cargo test -p quickquick resolve_direction` —— 6 个新增测试逐一精确命中（每个 `... ok`，`1 passed`）：

| 测试 | 结果 |
|---|---|
| `resolve_direction_with_source_chinese_text_with_zh_target_falls_back_to_en` | ok |
| `resolve_direction_with_source_auto_source_zh_target_falls_back_to_en` | ok |
| `resolve_direction_with_source_explicit_same_lang_pair_falls_back_to_default_target` | ok |
| `resolve_direction_with_source_english_text_with_zh_target_not_affected` | ok |
| `resolve_direction_with_source_cross_language_pair_not_affected` | ok |
| `resolve_direction_same_lang_pair_falls_back_to_default_target` | ok |

全量：`cargo test -p quickquick` → **317 passed; 0 failed**。

---

## 二、变异 sanity（杀恒真/旁路）

### 变异A — 守卫失效

把守卫 match 改回 `let target = configured_target.unwrap_or(default_target);`（移除 source==target 检测）。

结果：守卫相关 3 个测试同时变红 `FAILED. 0 passed; 3 failed`（`assertion left == right failed` ×3）。证明这些测试直接校验守卫行为，非恒真非旁路。还原后复绿。

### 变异B — 过度修正（恒用 default_target）

把守卫改成 `let target = default_target;`（无论如何忽略 configured_target）。

结果：`resolve_direction_with_source_cross_language_pair_not_affected`（ja→ko 合法跨语种对）变红 `FAILED. 0 passed; 1 failed`（被错误改成 ja→zh）。证明测试能拦住「过度修正吃掉合法跨语种对」。还原后复绿。

两个变异均从 /tmp 备份还原，lang.rs MD5 与备份逐字节一致，无残留业务代码改动。

---

## 三、真值边界探测

### 1. default_target 永不等于 source 的不变量 —— 成立

`default_target` 规则「source=="zh"→en；其余→zh」：
- source=="zh" → default="en"（≠zh）✓
- source 为任何非 "zh" 值 → default="zh"，而 source≠"zh"，故 default≠source ✓

不存在任何 source 取值使 default_target==source。守卫回退后永远保证 target≠source（守卫不是白守卫）。

### 2. 变体漏网（zh-CN vs zh）—— 理论边界，主路径不可达

守卫在 `normalize_zh_variant` 归一前用字面量比较。若直接传 `source="zh-CN"`+`target="zh"`，`"zh"!="zh-CN"` 为真→守卫不介入→经映射归一后仍可能 `zh-CN|zh-CN` 漏出。

实际影响：**无**。前端 `src/panels/translate/languages.ts` 下拉选项 code 全为短码，不含 "zh-CN"/"zh-Hans"。当前主路径不可达，标为已知边界（外部直接调 IPC 才可触发）。

### 3. end-to-end 串联

`src-tauri/src/ipc/translate.rs:184`：`let (source, target) = resolve_direction_with_source(text, configured_source, target_lang);`——守卫确在 `translate_text_impl` 生产路径上，无旁路。

---

## 四、最终门禁结论

**PASS（放行）**

- 6 个新增测试精确命中、非空匹配，317 全量零失败
- 变异A（移除守卫）3 红、变异B（过度修正）跨语种反例红——测试有判别力
- default_target≠source 不变量成立，守卫回退后必得不同语言对
- 变体边界理论存在但主路径不可达，已知边界如实标注
- 守卫在生产路径，无旁路；工作树还原干净

## 结束 git status --short（业务代码部分）

```
 M src-tauri/src/translate/lang.rs
```

与开工快照一致，无残留业务代码改动。
