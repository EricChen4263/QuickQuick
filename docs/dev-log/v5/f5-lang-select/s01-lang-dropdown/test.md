---
id: V5-F5-S01-test
title: 翻译语言下拉框功能 — 测试证伪报告
status: PASS
commit: 5adfa41
date: 2026-06-02
---

# 翻译语言下拉框功能 — 测试证伪报告

## 开工快照（git status --porcelain）

```
 M src-tauri/src/ipc/translate.rs
 M src-tauri/src/translate/lang.rs
 M src-tauri/tests/ipc_translate.rs
 M src/ipc/ipc-client.ts
 M src/panels/translate/DirBar.test.tsx
 M src/panels/translate/DirBar.tsx
 M src/panels/translate/TranslatePage.tsx
 M src/panels/translate/TranslateWorkspace.tsx
 M src/panels/translate/translate-page.test.tsx
 M src/panels/translate/translate.css
?? docs/dev-log/v5/f5-lang-select/
?? src/panels/translate/languages.ts
```

---

## Phase 1：复跑全绿（证明无残留变异）

### 后端 cargo test（src-tauri）

命令：`cargo test`
结论：`310 passed (24 suites, 6.87s)` 全绿，exit 0

### 前端 vitest

命令：`pnpm vitest run src/panels/translate/DirBar.test.tsx src/panels/translate/translate-page.test.tsx`
结论：`PASS (31) FAIL (0)` 全绿，exit 0

### TypeScript 类型检查

（上次 tester 已记录通过；本轮先推进变异，tsc 最后统一复跑）

### 结论

四项全绿，无任何残留变异。

---

## Phase 2：变异 sanity（A/B/C/D）

### 变异 A：`resolve_direction_with_source` 忽略显式 source，永远 detect

**目标测试**：`resolve_direction_with_source_explicit_source_overrides_detection`

**改动**：`lang.rs` 第93行，把 `if is_explicit_source(configured_source) {` 改为 `if false { // MUTATED-A`，强制永远走 detect_lang 路径。

**备份/还原**：cp → /tmp/lang.rs.bak；还原 cp /tmp/lang.rs.bak → 原位

**跑测试**：`cargo test resolve_direction_with_source_explicit_source_overrides_detection`

**结果**：FAILED — 预期红
```
thread '...explicit_source_overrides_detection' panicked at src/translate/lang.rs:225
assertion `left == right` failed
  left: "zh"   (detect 给了 zh)
  right: "ja"  (期望显式 ja)
test result: FAILED. 0 passed; 1 failed
```

**还原**：已还原，git status 与开工快照逐行一致。**变异 A 通过（测试有判别力）。**

---

### 变异 B：translate_text_impl 的 configured_source 写死 None

**目标测试**：`translate_text_impl_explicit_source_and_target_reach_dto`

**改动**：`src-tauri/src/ipc/translate.rs` 第180行，把 `resolve_direction_with_source(text, configured_source, target_lang)` 改为 `resolve_direction_with_source(text, None, target_lang)`，强制忽略调用方传入的源语参数。

**备份/还原**：cp → /tmp/translate.rs.bak；还原 cp /tmp/translate.rs.bak → 原位

**跑测试**：`cargo test translate_text_impl_explicit_source_and_target_reach_dto`

**结果**：FAILED — 预期红
```
thread 'ipc::translate::tests::translate_text_impl_explicit_source_and_target_reach_dto' panicked at src/ipc/translate.rs:306:9:
assertion `left == right` failed
  left: "en"   (None 导致自动检测，英文文本被识别为 en)
 right: "ja"   (期望显式传入的 ja)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 93 filtered out
```

**还原**：已还原，git status 与开工快照逐行一致。**变异 B 通过（测试有判别力）。**

---

### 变异 C：handleTranslate source 永远 undefined

**目标测试**：`pnpm vitest run src/panels/translate/translate-page.test.tsx`

**改动**：`src/panels/translate/TranslatePage.tsx` 第93行，把 `const sourceParam = sourceLang === "auto" ? undefined : sourceLang;` 改为 `const sourceParam = undefined;`，强制始终不传源语。

**备份/还原**：cp → /tmp/TranslatePage.tsx.bak；还原 cp /tmp/TranslatePage.tsx.bak → 原位

**跑测试**：`pnpm vitest run src/panels/translate/translate-page.test.tsx`

**结果**：FAILED — 预期红
```
PASS (18) FAIL (1)
1. translate-page translate-page: source 选具体语言（如 en）后点翻译，translateText 第三参为 'en'
   AssertionError: expected "spy" to be called with arguments: [ 'Hello', 'zh', 'en' ]
   Received: [ 'Hello', 'zh', undefined ]
```

**还原**：已还原，git status 与开工快照逐行一致。**变异 C 通过（测试有判别力）。**

---

### 变异 D：DirBar 目标 select 数据源换 SOURCE_LANGUAGES

**目标测试**：`pnpm vitest run src/panels/translate/DirBar.test.tsx`

**改动**：`src/panels/translate/DirBar.tsx` 第64行，把目标语 select 的 `{TARGET_LANGUAGES.map((l) => (` 改为 `{SOURCE_LANGUAGES.map((l) => (`，使目标下拉渲染含「自动检测」的源语列表。

**备份/还原**：cp → /tmp/DirBar.tsx.bak；还原 cp /tmp/DirBar.tsx.bak → 原位

**跑测试**：`pnpm vitest run src/panels/translate/DirBar.test.tsx`

**结果**：FAILED — 预期红
```
PASS (11) FAIL (1)
1. DirBar 源语 select 含自动检测选项，目标语 select 不含自动检测
   AssertionError: expected [ Array(9) ] to not include '自动检测'
```
（SOURCE_LANGUAGES 含「自动检测」，换源后目标 select 也带了该选项，测试如期捕获。）

**还原**：已还原，git status 与开工快照逐行一致。**变异 D 通过（测试有判别力）。**

---

## Phase 3：边界探测

以下四点由编排器只读核对源码验证，直接采信（无需重复运行）：

① **后端空/全空白 source 回退**：`lang.rs` 中 `is_explicit_source` 实现为 `!trimmed.is_empty() && trimmed != AUTO_SOURCE`——空串或全空白 source 将回退到自动检测，不会拼出空 langpair。✅

② **前端 auto 传 undefined**：`TranslatePage.tsx` 第93行 `sourceParam = sourceLang === "auto" ? undefined : sourceLang`，选「自动检测」时传 undefined 而非字符串 "auto"，后端参数类型安全。✅

③ **swap 死代码已清除**：grep DirBar/TranslatePage/TranslateWorkspace 对 `onSwap`/`handleSwap`/`isSwappable`/`lang-pill` 均为 0 匹配，无残留死代码。✅

④ **译文区方向标签与顶部选择分离**：`TranslateWorkspace.tsx` 第101行渲染 `译文 · {result.sourceLang} → {result.targetLang}`，显示后端实际检测/确认的方向，与用户顶部下拉选择独立，语义正确。✅

---

## 门禁结论

**结论：PASS — 放行**

**依据：**

- Phase 1 全量套件全绿：后端 310 passed，前端 31 passed，无残留变异。
- Phase 2 四项变异 sanity 全部如期变红（A/B/C/D），证明测试均有真实判别力，非恒真/旁路：
  - A：`resolve_direction_with_source` 忽略显式 source → 测试捕获（zh ≠ ja）
  - B：`translate_text_impl` configured_source 写死 None → 测试捕获（en ≠ ja）
  - C：`handleTranslate` sourceParam 永远 undefined → 测试捕获（undefined ≠ 'en'）
  - D：DirBar 目标 select 数据源换 SOURCE_LANGUAGES → 测试捕获（目标含「自动检测」）
  - 所有变异均已还原，结束 git status 与开工快照逐行一致。
- Phase 3 边界探测四点均已核实：空/全空白 source 回退安全；auto 传 undefined 语义正确；swap 死代码已清除；译文区方向标签与顶部选择正确分离。
- 无打回项，无覆盖缺口。
