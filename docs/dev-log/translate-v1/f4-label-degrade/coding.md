---
id: TV1-F4-S01-code
type: coding_record
level: 小功能
parent: TV1-F4
status: 通过
commit: PENDING
acceptance_ids: [TV1-F4-A01]
---

# TV1-F4-S01 编码留痕：非官方翻译源 UI 标注 + 失败降级提示

## 做了什么

为非官方/自建翻译接口（免 key 源）在翻译源选择器加「⚠ 非官方」标注，并在翻译失败、
当前选中源为非官方时追加可区分的降级提示，引导用户切换其它源（设计文档§三.决策3）。
能力标志 `is_unofficial` 从 Rust `ProviderCapability` 一路透出到前端 `Provider` 类型。

非官方源定义（设计文档§二，免 key 源均为非官方/自建接口）：
- `is_unofficial=true`：lingva、google_free、yandex、transmart、bing（5 源）
- `is_unofficial=false`：baidu、deepl_free、google（3 官方 keyed 源）

## TDD 红→绿证据

### Rust（cargo test --lib）
- RED：`no field is_unofficial on type ProviderCapability/ProviderDto`（编译失败，功能未实现）。
- GREEN：
  - `test translate::providers::tests::capability_is_unofficial_flags_free_sources_only ... ok`
  - `test ipc::settings::tests::get_translate_providers_impl_exposes_is_unofficial ... ok`
  - 全量 `test result: ok. 188 passed; 0 failed`。

### 前端（vitest）
- RED：标注/降级提示 2 个断言失败（功能未实现），既有 28 个翻译页测试仍绿。
- GREEN（`src/panels/translate/label-degrade.test.tsx`，3 tests passed）：
  - `nonofficial_source_label_and_degrade_hint: isUnofficial 源在选择器 option 显示非官方标注、官方源不显示 ✓`
  - `nonofficial_source_label_and_degrade_hint: 当前源 isUnofficial 时翻译失败追加降级提示，官方源不追加 ✓`
  - `nonofficial_source_label_and_degrade_hint: 当前源为官方时翻译失败不追加非官方降级提示 ✓`
  - 全量前端 `Tests 465 passed (465)`，无回归。
  - 原始证据：`artifacts/fe-green.log`。

## 改动文件

### Rust
- `src-tauri/src/translate/mod.rs`：`ProviderCapability` 新增 `is_unofficial: bool`；
  顺手修第 65 行过时注释（原写「MyMemory 为 false（默认源）」，MyMemory 已移除）
  → 改为「免 key 源（如 lingva）false、需 key 源（如 baidu）true」。
- `src-tauri/src/translate/providers.rs`：8 个 provider 的 `capability()` 填 `is_unofficial`
  （lingva/google_free/yandex/transmart/bing=true；baidu/deepl_free/google=false）；新增能力单测。
- `src-tauri/src/ipc/settings.rs`：`ProviderDto` 加 `is_unofficial`（serde camelCase → `isUnofficial`），
  `get_translate_providers_impl` 透传；新增 DTO 透出单测。

### 前端
- `src/ipc/ipc-client.ts`：`Provider` 接口加 `isUnofficial: boolean`。
- `src/panels/translate/DirBar.tsx`：非官方源 option label 追加「⚠ 非官方」（`isUnofficial` 守卫）。
- `src/panels/translate/TranslateWorkspace.tsx`：派生当前选中源是否非官方，
  失败时在 `role=alert` 错误区追加降级提示常量 `UNOFFICIAL_DEGRADE_HINT`。
- `src/panels/translate/label-degrade.test.tsx`：新增（3 用例，测试名含 `nonofficial_source_label_and_degrade_hint`）。
- 既有 mock provider 补 `isUnofficial` 字段（DirBar.test.tsx / translate-page.test.tsx），满足 tsc。

## 关键决策

1. **标注落在 label 文本而非改 Select 组件**：`SelectOption.label` 为 string、`{option.label}` 直接渲染。
   在 DirBar 映射时按 `isUnofficial` 拼接标注，零侵入 Select 组件，复用既有渲染路径，保持单一职责。
2. **降级提示在 Workspace 派生、不新增 prop**：TranslateWorkspace 已持有 `providers` + `selectedProviderId`，
   `some(p.id===selected && p.isUnofficial)` 即可派生，避免父组件多传状态、避免接口膨胀。
3. **`isUnofficial` 用 `=== true` / 真值守卫**：旧测试 mock 未带该字段时为 undefined，不误标注，
   保证既有翻译测试零回归。
4. **未动既有源翻译逻辑**：只增能力元数据字段，build_request/parse_response/translate 全部未改；
   未参考/抄录 pot 代码（仅按设计文档§二的源分类填标志）。

## 收尾：既有测试夹具回归处理

标注拼进 option 的 `label` 文本后，会改变 option 的 accessible name；
若既有测试 mock 把免 key 源标 `isUnofficial: true`，其 `getByRole("option", { name: "..." })`
精确匹配会落空。处理（不削弱断言）：

- `DirBar.test.tsx`：该文件只验证「列出/禁用/选中」行为、与标注无关，
  夹具 `isUnofficial` 统一置 false 避免标注后缀污染精确 name 匹配；
  标注渲染改由 `label-degrade.test.tsx` 用独立夹具 `MIXED_PROVIDERS` 覆盖。
- 其余既有 mock（ipc-client / settings 两测 / translate-page）按真实分类补 `isUnofficial`
  （免 key 源 true、需 key 源 false），其用例的精确 name 匹配对象（「百度翻译」「英文」等）
  均为不被标注或非匹配项，未受影响。
- `tests/translate.rs` 桩 provider 补 `is_unofficial: false`（测试桩，中性置 false）。

## 自测

- Rust：`cargo test`（含集成测试）全绿；lib `test result: ok. 188 passed; 0 failed`；
  目标用例逐行命中：
  `test translate::providers::tests::capability_is_unofficial_flags_free_sources_only ... ok`、
  `test ipc::settings::tests::get_translate_providers_impl_exposes_is_unofficial ... ok`。
- `cargo clippy --all-targets -- -D warnings` exit 0。
- 前端：`pnpm test` `Tests 465 passed (465)`；目标文件 `label-degrade.test.tsx (3 tests)` 全绿；
  `npx tsc --noEmit` exit 0 无错。
- 无 TODO/FIXME；无装饰性分隔注释；安全无敏感值入日志（仅元数据布尔字段）。
- mod.rs:65 过时注释已修正。
