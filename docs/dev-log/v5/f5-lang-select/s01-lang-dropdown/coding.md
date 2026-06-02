---
id: V5-F5-S01
title: 翻译源语下拉框——后端显式源语支持
status: 完成（测试 PASS + 审查通过）
commit: PENDING
date: 2026-06-02
---

# 后端

## 动机

原 `translate_text` 命令只接受 `target`，源语永远靠 `detect_lang` 自动检测（仅能区分 zh/en）。
前端要做源语下拉框（含"自动检测" + zh/en/ja/ko/fr/de/es/ru），需后端接受显式源语：
- `source` 为具体语言码（非空、非 "auto"）→ 跳过检测直接用
- `source` 为 "auto"/空/None → 回退现有检测逻辑

## resolve_direction_with_source 设计

新增纯函数 `resolve_direction_with_source`（`src-tauri/src/translate/lang.rs`），
用具名常量 `AUTO_SOURCE = "auto"` 避免魔术串。
辅助谓词 `is_explicit_source` 封装"有效显式源语"判定（None / 空白 / "auto" 均为无效），
保持主函数嵌套 ≤ 2 层、行数 ≤ 20 行。

旧函数 `resolve_direction` 保留不动（未被删除，无破坏性变更）。

## 改动文件与函数

| 文件 | 改动 |
|---|---|
| `src-tauri/src/translate/lang.rs` | 新增 `AUTO_SOURCE` const、`is_explicit_source`、`resolve_direction_with_source`；新增 6 个单测 |
| `src-tauri/src/ipc/translate.rs` | `translate_text_impl` 加参数 `configured_source: Option<&str>`；import 换为 `resolve_direction_with_source`；`translate_text` 命令加 `source: Option<String>` 参数；新增 3 个单测 |

## TDD 红绿记录

### lang.rs

- RED：先在测试模块写 5 个调用 `resolve_direction_with_source` 的测试 + 1 个多语言直通 sanity 测试，`cargo test` 报 `cannot find function` 5 次——确认红。
- GREEN：实现 `AUTO_SOURCE` const + `is_explicit_source` + `resolve_direction_with_source`，`cargo test --lib translate::lang` 9 passed。

### ipc/translate.rs

- RED：在新 `tests` 模块写 3 个测试（含 `configured_source` 第 5 参数），`cargo test` 报 `this function takes 4 arguments but 5 arguments were supplied`——确认红。
- GREEN：
  1. import 从 `resolve_direction` 换为 `resolve_direction_with_source`
  2. `translate_text_impl` 加 `configured_source: Option<&str>` 参数，内部改用 `resolve_direction_with_source`
  3. `translate_text` 命令加 `source: Option<String>`，透传 `.as_deref()`
  4. 修正测试内建表 SQL（列名 `translated_text` 与 history.rs 一致）
  5. `cargo test --lib ipc::translate` 3 passed。

## 多语言直通验证

`map_lang_for_provider("mymemory", &Lang::new("ja"))` 等 6 种语言均原样返回（通过 `map_lang_for_provider_mymemory_passes_through_non_zh_langs` 单测覆盖），确保拼 langpair 时不会出现空串。

## 实跑输出摘要

```
cargo test --lib   : 94 passed (1 suite, 2.41s)
cargo check        : exit 0, 0 warnings
```

## 前端命令签名（前端据此对接）

```typescript
// Tauri invoke 参数
invoke("translate_text", {
  text: string,
  source: string | null,   // 具体语言码 | "auto" | null
  target: string | null,
})
// 返回 TranslateResultDto { translated, sourceLang, targetLang }
```

---

---

# 前端

## 动机

将翻译方向栏从"静态药丸 auto⇄zh + swap 按钮"改为"左右两个语言下拉框"，
用户可独立选择源语（含自动检测）和目标语，后端接受显式 source 参数跳过检测。

## 改动文件

| 文件 | 改动 |
|---|---|
| `src/panels/translate/languages.ts` | 新建：导出 `LanguageOption` interface、`SOURCE_LANGUAGES`（含 auto）、`TARGET_LANGUAGES`（过滤 auto）9 种语言 |
| `src/panels/translate/DirBar.tsx` | 核心改造：删 `lang-pill`/`isSwappable`/`onSwap`；新增源语/目标语两个 `<select>`；props 改为 `onSourceChange`/`onTargetChange` |
| `src/panels/translate/TranslateWorkspace.tsx` | 删 `onSwap` prop；加 `sourceLang`/`targetLang`/`onSourceChange`/`onTargetChange`，透传给 DirBar |
| `src/panels/translate/TranslatePage.tsx` | 加 `sourceLang`/`targetLang` state（默认 auto/zh）；`handleTranslate` 加 source 参数（auto→undefined）；删 `handleSwap` |
| `src/ipc/ipc-client.ts` | `translateText` 签名加第三参 `source?: string`，invoke 透传 `{ text, target, source }` |
| `src/panels/translate/translate.css` | 删 `.lang-pill`/`.lang-pill .swap`；新增 `.lang-selects`/`.lang-select`，与 provider select 同视觉 token |
| `src/panels/translate/DirBar.test.tsx` | 重写：删 swap 相关旧断言（7 个）；新增 5 个测试（渲染两个 select / auto 选项 / onSourceChange / onTargetChange / 无 swap 按钮） |
| `src/panels/translate/translate-page.test.tsx` | 删旧 swap 集成测试（3 个）；新增 4 个测试（默认 source=auto/target=zh / 改 target / source 具体语言 / source=auto→undefined） |

## languages 列表

SOURCE_LANGUAGES：auto（自动检测）/ zh / en / ja / ko / fr / de / es / ru，共 9 项。
TARGET_LANGUAGES：同上过滤 auto，共 8 项。

## DirBar 改造要点

- 删除：`lang-pill` span、swap button、swap SVG、`isSwappable` 函数、`onSwap` prop。
- 新增：两个 `<select class="lang-select">`，分别 aria-label="源语言"/"目标语言"，
  套 `.wrap` 容器复用 chevron SVG（与 provider select 结构一致）。
- `.lang-selects` 容器（`display:inline-flex; gap:8px`）包裹两个语言 select。
- `src-select` provider 选择器原样保留。

## translateText 签名扩展

```typescript
// 新签名（target 在前、source 在后，现有调用不破坏）
translateText(text: string, target?: string, source?: string): Promise<TranslateResult>

// handleTranslate 传参方式
const sourceParam = sourceLang === "auto" ? undefined : sourceLang;
await translateText(text, targetLang, sourceParam);
```

## 删除的 swap 逻辑

- `handleSwap` 函数（TranslatePage）：整体删除。
- `onSwap` prop 链路（TranslatePage → TranslateWorkspace → DirBar）：全链路清除。
- `isSwappable` 函数（DirBar）：删除。
- 旧 translate-page.test.tsx 3 个 swap 集成测试：全部移除。

## TDD 红绿记录

### RED

运行更新后的 DirBar.test.tsx（旧实现），5 个新测试报错：
- `Unable to find an accessible element with the role "combobox" and name "源语言"`（4 个）
- `expect(element).not.toBeInTheDocument()` 失败（swap 按钮仍在，1 个）

确认：因功能未实现而失败，不是语法/环境错误。

### GREEN

按序实现：
1. `languages.ts`：语言列表常量
2. `DirBar.tsx`：替换 lang-pill 为两个 select，删 swap 逻辑
3. `translate.css`：新增 `.lang-selects`/`.lang-select`，删旧 `.lang-pill` 样式
4. `ipc-client.ts`：translateText 加 source 第三参
5. `TranslateWorkspace.tsx`：更新 props，删 onSwap
6. `TranslatePage.tsx`：加 state，更新 handleTranslate，删 handleSwap
7. 更新两个测试文件

## 实跑摘要

```
pnpm test --run   : 359 passed (43 files), EXIT 0
pnpm tsc --noEmit : No errors found, EXIT 0
```

装饰注释检查：无命中。TODO/FIXME 检查：无残留。

---

# review 修复：语言下拉箭头 CSS

**优先级**：reviewer I-高危（视觉破损）

## 问题根因

`DirBar.tsx` 中两个语言 `<select>` 各被 `.wrap` 容器包裹、内含 chevron SVG，
但 `translate.css` 只定义了 `.src-select .wrap` / `.src-select .wrap svg` 的 `position: absolute` 定位，
缺少对 `.lang-selects .wrap` 的对应规则。
结果 SVG 以正常文档流渲染在 select 之后，而非叠在 select 右侧——
select 已预留的 `padding-right: 26px` 空位里没有箭头，视觉破损。

## 改动（仅 `translate.css`）

1. **追加定位规则**（位于 `.lang-select:focus` 之后）：
   - `.lang-selects .wrap { position: relative; }`
   - `.lang-selects .wrap svg`：`position: absolute; right: 7px; top: 50%; transform: translateY(-50%); width/height: 13px; color: var(--muted); pointer-events: none;`
   完全对齐 `.src-select .wrap svg` 的数值与风格，复用现有 token 变量，无新值引入。

2. **修复第 34 行过时死注释**：`lang-pill` 类已于前端改造时删除，
   注释更正为 `lang-selects 左对齐 + src-select 推到右侧`。

## 验证结果

```
pnpm tsc --noEmit       : exit 0，No errors found
pnpm vitest run DirBar  : PASS (12) FAIL (0)
```
