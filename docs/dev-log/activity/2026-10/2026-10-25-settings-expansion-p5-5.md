---
status: done
category: implementation
subject: Maccy 对标 · Phase 5（P5-5 设置项扩充：排序方式 / 图片高度 / 首次预览延迟 / 命中高亮）
summary: 设置页新增「外观」子项，落地四个纯展示型设置：排序方式（最近复制 = 默认，与改动前逐条一致 / 首次复制按 created_utc 稳定重排，不改 SQL、不加索引、不动 schema）、弹出面板图片高度（16–48px，默认 24px）、首次预览延迟（200–5000ms，默认 1500ms，后续切换固定 200ms 不开放）、命中高亮样式（背景高亮默认 / 下划线 / 纯加粗）。四项 tooltip 均写明默认值。排序与高亮落盘后复用既有 clipboard-changed 信号让主窗与弹窗立刻生效。变异自证三轮（排序分支 / 切换档 200 / 首个预览默认 1500）分别红 3、5、5。
evidence:
  commits:
    - "07fd8f6 feat(settings): 外观设置四项（排序/图片高度/首次预览延迟/命中高亮）（P5-5）"
    - "docs(work-items): P5-5 回写 done + 活动卡（P5-5）"
  files:
    - "src-tauri/src/settings.rs（SortOrder / HighlightStyle / 区间常量 / effective_* 收敛）"
    - "src-tauri/src/ipc/settings.rs（AppearanceSettingsDto + 批量读 + 4 个 setter + 校验文案）"
    - "src-tauri/src/ipc/clipboard.rs（list_clip_items_ordered_impl / sort_rows_for_order / resolve_sort_order）"
    - "src-tauri/src/db.rs（列表查询带出 created_utc，ORDER BY 与索引计划不变）"
    - "src/panels/settings/AppearancePanel.tsx + appearance-settings.ts + useAppearanceSettings.ts"
    - "src/clip-popover/preview-delay.ts + useDeferredPreview.ts + ClipPopoverApp.tsx"
    - "src/panels/clipboard/ClipboardPage.tsx（data-hit-style 下发 + 事件后重读）"
  commands:
    - "rtk pnpm exec tsc -b  # 退出 0"
    - "rtk pnpm test  # 79 files / 853 passed"
    - "cd src-tauri && cargo fmt --all --check  # 退出 0"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings  # 干净"
    - "cd src-tauri && cargo test  # 838 passed / 0 failed"
    - "变异三轮（见下表格），每次还原后子集复跑全绿"
  urls: []
---

# AI 活动记录 · P5-5 设置项扩充（外观四项）

## 数字出处（本次实跑，可复算）

| 指标 | 数值 | 取数 commit | 命令 |
|---|---|---|---|
| 前端类型检查 | 退出 0 | `07fd8f6` | `rtk pnpm exec tsc -b` |
| 前端测试 | **79 files / 853 passed** | `07fd8f6` | `rtk pnpm test` |
| 前端基线（改动前同机实跑） | 75 files / **804**（其中 1 例 `clipboard-page.test.tsx` 富文本断言为既知 flaky，单跑必绿） | `85f836b` | `rtk pnpm test` |
| Rust 格式 | `cargo fmt --all --check` 退出 0 | `07fd8f6` | 同左 |
| Rust 静态检查 | `clippy -D warnings` 干净 | `07fd8f6` | `cargo clippy --all-targets -- -D warnings` |
| Rust 测试 | **838 passed / 0 failed** | `07fd8f6` | `cargo test` |
| Rust lib 子集 | 526 passed（基线 500） | `07fd8f6` | `cargo test --lib` |
| 本轮新增测试文件 | **4** 个（appearance-settings / AppearancePanel / popover-appearance / clipboard highlight-style） | `07fd8f6` | — |
| 本轮新增前端用例 | **+49**（853 − 804） | `07fd8f6` | 前后两次 `pnpm test` 之差 |

> 口径提醒：前端基线那次全量跑出 1 例 `clipboard-page.test.tsx` 富文本断言失败，单文件复跑 37 全绿；这是 P6-2 已登记的观察项，不是本项引入的回归（本项也未改该断言）。

## 摘要

- **排序方式**：`recent`（默认）与 `first_copied` 两档。默认档**一行排序代码都不额外执行**，直接沿用库里 `is_favorite DESC, last_modified_utc DESC, rowid DESC`；`first_copied` 对已取回的行做**稳定重排**（`is_favorite DESC, created_utc DESC`）。排序只影响展示，去重/收藏/淘汰语义零改动。
- **图片高度**：`image_height_px` 16–48（默认 24，等于改动前 `.popover-row-thumb` 的硬编码），经 `.popover-shell` 上的 CSS 变量 `--popover-thumb-height` 下发。
- **首次预览延迟**：`preview_first_delay_ms` 200–5000（默认 1500，直接引用 `preview-delay.ts` 的常量，不再两处各写 1500）；后续切换档 200ms 固定不开放。
- **命中高亮样式**：`default` / `underline` / `bold`，靠根元素 `data-hit-style` + CSS 覆盖实现，`<mark class="clip-hit">` 结构与命中区间不变。
- 设置页 6 项 → **7 项**（新增「外观」），四项 tooltip 都写明默认值。

## 三个值得记住的取舍

### 1. 排序为什么不在 SQL 里换 ORDER BY 列

列表主查询的索引是 `idx_clip_items_live_favorite_modified (is_deleted, is_favorite, last_modified_utc)`，且有一条**钉住生产语句本身**的查询计划测试（`list_query_plan_seeks_index_and_drops_temp_sort`）。把 ORDER BY 换成 `created_utc` 会让计划退化成 `USE TEMP B-TREE FOR ORDER BY`（全表取回再排），要修就得建新索引——而建索引在本项目等于写一条迁移（`CURRENT_SCHEMA_VERSION` 递增），P5-5 明令禁止动 schema。

所以换了思路：**取数顺序不动，在内存里重排**。行数上界是历史条目上限（默认 500 / 最高 999），一次稳定排序的代价可以忽略；而「稳定」这个性质正好用来兜底——同 created、同收藏的行沿用库里的 rowid 顺序，与默认档的兜底口径一致。

为了让翻译取数不被这个设置带跑，`list_clip_items_impl` 保留成 `list_clip_items_ordered_impl(conn, Recent)` 的薄包装，`pick_latest_translate_text_impl` 继续走它，并补了一条用例：把列表切到「首次复制」时，翻译源仍取最近复制的那条。

### 2. 数字可收敛、枚举不可猜

数值字段（图片高度、预览延迟）在**三个位置**都有边界：前端输入框拦截（`parseIntInRange` 拒收小数/科学计数/全角）、后端写路径拒收越界（不落盘）、读路径 `effective_*` 再 clamp 一次（兜手工改坏的 settings.json）。枚举则不同——`by_magic` 这种值没有「收敛」的合理目标，只能拒收；而 serde 对枚举字段的反序列化失败会让**整份设置**回退默认（与 `exclude_mode` 的历史行为一致）。这个爆炸半径不小，但改它要动 settings 的反序列化策略（超出本项范围），所以用一条测试把它显式钉住（`invalid_sort_order_string_falls_back_to_all_defaults`），免得日后有人以为「只坏一个字段只丢一个字段」。

### 3. 落盘后什么时候生效：复用既有事件，而不是新造一个

主窗的三个页面是常驻挂载（`display: none/block` 切换），弹窗更是 hide/show 复用——只写盘不发信号，用户会看到「设置里选了首次复制、列表还是旧顺序」，这正是路线图批评的「设置页在向用户撒谎」。

信号复用既有的 `clipboard-changed`：它在 `lib.rs` 的常量文档与 `set_max_history_items` 的先例里已经是「列表数据需要刷新」的语义，不是字面「剪贴板内容变了」。因此 `set_sort_order` 与 `set_highlight_style` 落盘后发它，两个前端在既有的事件回调里多做一件事（递增外观刷新序号 / 重读设置）；而 `set_image_height`、`set_preview_first_delay` **刻意不发**——纯 CSS 尺寸与时序变更重取一次列表只会白传 payload。

## 已知边界（如实登记）

1. **「首次复制」只在分组内严格有序**：弹窗仍按 `last_modified_utc` 分「收藏 / 今天 / 更早」，所以一条今天被重新复制的旧条目会落进「今天」组，跨组的全局单调性不成立。是否在 `first_copied` 下改分组口径属产品决策，本轮未动分组。
2. **tooltip 只覆盖新增四项**：既有约 15 项设置（通用/隐私/存储/热键）的「默认值 tooltip」没做，属路线图 Phase 5 的独立一行，且必须逐项核对真实默认值（写错默认值比不写更糟）。
3. 图片高度**只影响弹窗列表**，主窗列表仍是 44×32；设置项标题因此写作「弹出面板图片高度」，避免承诺它管不到的地方。

## 验收（实跑）

**测试面**：Rust 侧新增 26 条 lib 用例（默认值、序列化形状、6 组 clamp、4 个 setter 的越界拒收与落盘、读口）、6 条列表排序用例（默认档与改动前一致 / 首次复制顺序 / 两档必须不同 / 收藏优先 / 默认档不重排 / 翻译取数不随排序漂移）；前端新增 4 个测试文件共 35 条用例，另在 `ipc-client`（+10）、`preview-delay`（+2）、`settings-page`（+2）补用例，并给 9 个既有测试文件的 IPC mock 补上 `getAppearanceSettings` 桩。

**变异自证（三轮，全部在最终树上做，每次还原后确认子集复跑全绿、工作区无残留）**：

| # | 变异 | 红测试 |
|---|---|---|
| ① | `ipc/clipboard.rs::sort_rows_for_order` 里把 `FirstCopied` 分支短路（`if order != FirstCopied \|\| true { return; }`） | **3 条**：`list_clip_items_first_copied_order_ignores_recopy_bump`、`recent_and_first_copied_orders_differ_on_same_data`、`pick_latest_translate_text_ignores_first_copied_display_order` |
| ② | `preview-delay.ts::PREVIEW_SWITCH_DELAY_MS` 200 → 300 | **5 条**：preview-delay ×2、preview-defer ×2、popover-appearance「首个预览档可配，但首个之后切换仍固定 200ms」 |
| ③ | `preview-delay.ts::PREVIEW_FIRST_DELAY_MS` 1500 → 800 | **5 条**：preview-delay ×2、appearance-settings「默认值与改动前行为一致」、AppearancePanel / settings-page 两处「tooltip 含默认值」断言 |

变异①顺带暴露一个覆盖事实：`first_copied_order_keeps_favorites_first` 在这轮变异下**不红**——它的数据布局在默认顺序下也满足收藏优先，所以它锚的是「收藏优先级」，不是「按 created 排序」。这符合设计（两件事分开锚），但也说明不能拿它当排序分支的证据。

## 未验证 / 待人工

- **真机手感**：各档首次预览延迟的实际体验、16/48px 缩略图的观感、下划线/纯加粗两档高亮在长文本里的可读性、切换排序后长列表滚动位置是否跳动，均未在打包 GUI 上点过。
- **打包 GUI**：`invoke` 参数名（`order` / `px` / `ms` / `style`）只在单测里按字符串断言，未在真实 Tauri 运行时点过一次设置页。
- **Windows**：本项未新增平台相关代码，但整条链路（含 `cargo test`）仍未在 Windows 编译验证。
- **未做独立审查**：本卡只含执行端自检与变异自证，独立 reviewer 结论由 lead 另行派发。
