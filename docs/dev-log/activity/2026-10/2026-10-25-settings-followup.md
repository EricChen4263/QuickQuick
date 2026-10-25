---
status: done
category: implementation
subject: Maccy 对标 · P5-5 收尾 + P5-8（逐键设置、分组口径、高亮/范围/tooltip 对齐 Maccy）
summary: 按 Maccy 2.6.1 源码对齐四件事：① settings.json 改逐键读写（P5-8，坏字段只回退自己、写入只改目标键、语法坏文件先备份 .bak）；② 「首次复制」排序下弹窗分组改按 created_utc 划分今天/更早（Maccy 无分组，按「排序口径 = 分组口径」处理）；③ 高亮改 Maccy 的 color/bold/italic/underline 四档、默认 bold，图片高度默认 40 / 1–200，预览延迟上限放宽到 100000（两处默认视觉变化已在台账登记待真机确认）；④ 全部设置项补 tooltip 并写明默认值（默认值逐项从代码读出）。四个独立 commit，变异自证三轮（A 红 5、C 红 2 与 4）。
evidence:
  commits:
    - "03a84d0 fix(settings): settings.json 改逐键读写，坏字段不再连累整份设置（P5-8）"
    - "cd1af1f feat(settings): 高亮四档、图片高度与预览延迟范围对齐 Maccy（P5-5）"
    - "e36db71 feat(popover): 「首次复制」排序下分组按 createdUtc 划分今天/更早（P5-5）"
    - "8bb7ed1 feat(settings): 全部设置项补齐 tooltip 并写明默认值（P5-5）"
  files:
    - "src-tauri/src/settings.rs（load_lenient / from_json_object_lenient / write_setting_field / update_settings_value / 备份）"
    - "src-tauri/src/ipc/settings.rs（16 个 setter 改逐键写；高亮四档解析与校验）"
    - "src-tauri/src/ipc/clipboard.rs + tests/search_title.rs（DTO 加 createdUtc；payload 基线修订）"
    - "src/clip-popover/grouping.ts（GroupBasis / groupingTimestamp）"
    - "src/panels/settings/appearance-settings.ts + AppearancePanel.tsx（默认值/区间/四档）"
    - "src/panels/settings/*.tsx（全部设置项 tooltip）+ settings-tooltips.test.tsx"
  commands:
    - "rtk pnpm exec tsc -b  # 退出 0"
    - "rtk pnpm test  # 80 files / 868 passed"
    - "cd src-tauri && cargo fmt --all --check  # 退出 0"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings  # 干净"
    - "cd src-tauri && cargo test  # 849 passed / 0 failed"
  urls: []
---

# AI 活动记录 · P5-5 收尾 + P5-8（Maccy 对齐）

## 数字出处（本次实跑，可复算）

| 指标 | 数值 | 取数 commit | 命令 |
|---|---|---|---|
| 前端测试（最终） | **80 files / 868 passed** | `8bb7ed1` | `rtk pnpm test` |
| 前端测试（P5-5 首轮末，对照） | 79 files / 853 passed | `5118cc6` | 同上 |
| Rust 测试（最终） | **849 passed / 0 failed** | `8bb7ed1` | `cargo test` |
| Rust 测试（P5-8 单独提交时） | 848 passed | `03a84d0` | 同上 |
| Rust 测试（P5-5 首轮末，对照） | 838 passed | `5118cc6` | 同上 |
| 列表 payload 基线（混合形态） | 2,105,877 → **2,159,877 B**（+54,000 B，+2.6%） | `e36db71` | `cargo test --test search_title` |
| 列表 payload 基线（全多行形态） | 923,648 → **977,648 B**（+54,000 B，+5.8%） | `e36db71` | 同上 |
| 变异①（整份写回）红测试数 | **5** | `03a84d0` + 变异 | `cargo test --lib settings` |
| 变异②（图片高度默认 40→24）红测试数 | **2** | `8bb7ed1` + 变异 | `cargo test --lib settings` |
| 变异③（高亮默认 bold→color）红测试数 | **4** | `8bb7ed1` + 变异 | `npx vitest run src/panels/settings …` |

> payload 的 +54,000 B 是 **2000 行 × 27 B**（`"createdUtc":1792897518450,`）的固定成本，来自任务 B；`tests/search_title.rs` 的绝对基线数字与说明已同步修订，该测试仍守「不得再加 title」。

## 摘要

1. **P5-8 · settings.json 逐键读写**（`03a84d0`）：读走 `load_lenient`（逐字段容错，坏字段只回退自己并给出问题清单，`load_or_default` 是它的静默包装以免 500ms 轮询刷日志）；写走 `write_setting_field` / `update_settings_value`（读原始 JSON 对象 → 只改目标键 → 写回，未知键原样保留），16 个 setter 全改；语法坏文件**先备份 `settings.json.bak` 再写**（备份失败拒绝写入）。依据：Maccy 用 UserDefaults 逐键存储，一个坏键只影响它自己。
2. **B · 「首次复制」的分组口径**（`e36db71`）：`first_copied` 下按 `created_utc` 划分「今天 / 更早」，`recent` 维持 `last_modified_utc`。Maccy **没有分组**（`Sorter.swift` 只做整体排序 + pin 置顶），所以这是「排序口径 = 分组口径」的业界常识解法，不是照抄。列表 DTO 因此加 `createdUtc`。
3. **C · 三处对齐 Maccy**（`cd1af1f`）：高亮四档 `color/bold/italic/underline`、默认 `bold`（`HighlightMatch.swift` + `Defaults` 默认值 + `HistoryItemDecorator.swift:148-173` 的渲染分支）；图片高度默认 **40**、范围 **1–200**（`imageMaxHeight` / Stepper `1...200`）；预览延迟上限放宽到 **100_000**（Stepper `200...100_000`），默认 1500 不变。
4. **D · 全部 tooltip**（`8bb7ed1`）：Maccy 每个设置项都有 `.help(...)`（全仓 17 处），按其约定给通用 3 + 外观 4 + 热键 3 + 翻译源 1 组 + 隐私 5 + 存储 6 全部补 tooltip 并写明默认值；默认值逐个从代码读出。

## 两处有意的默认视觉变化（需真机确认）

| 项 | 首轮（P5-5） | 现在（对齐 Maccy） | 依据 |
|---|---|---|---|
| 命中高亮默认 | 背景染色 + 加粗（自造 `default` 档） | **纯加粗、无背景**（`bold`） | `highlightMatch` 默认 `.bold` |
| 弹窗缩略图默认高度 | 24px | **40px** | `imageMaxHeight` 默认 40 |

首轮把高亮默认做成「背景+加粗」是自己的设计，不是 Maccy 的；删掉自造档后默认视觉必然变。两处都已写进 `docs/design/v04-work-items.md` P5-5 的「未验证 / 待人工」。

## 三个实现判断

### 1. 字段级容错为什么读路径不自动修坏键

宽松解析只负责「让坏字段不连累别人」，**不改写文件**。把坏键自动"修"成默认值等于悄悄改用户设置，而且用户下次打开文件会以为自己的值从来没写错过。所以：读侧回退 + 留痕清单；写侧只动本次要改的键；坏键原地留着等人修。

写后体检也只在**写路径**做（用户动作触发，低频），读热路径（入库轮询每约 500ms 一次）刻意不打日志——否则一个坏字段就是一秒两行 stderr。

### 2. 分组口径不是抄 Maccy，而是「两处口径必须一致」

Maccy 只有一条全局列表，没有分组，所以它的 `firstCopiedAt` 天然全局有序。我们多了「收藏 / 今天 / 更早」三层分组，**排序口径与分组口径一旦不一致，跨组顺序就不再单调**——首轮就是这个状态（按 created 排、却按 modified 归档），已登记为待裁决项。收尾按「两处同源」处理：同一个 `appearance.sortOrder` 同时决定后端排序键与前端分组时间戳，`recent` 档完全不变。

### 3. 高亮 CSS 改成「基础规则只留公共属性 + 四分支」

首轮是「`.clip-hit` 自带背景与字重 + 两档覆盖」，删掉自造默认档后这种写法会留下"默认档样式散落在基础规则里"的隐患。现在基础规则只有 `color: inherit; border-radius; padding`，四档各自一条 `data-hit-style` 分支——加第五档只需加一条分支，不会碰默认档。

## 变异自证（三轮，均在最终树上做，还原后子集复跑全绿、无残留）

| # | 变异 | 红测试 |
|---|---|---|
| ① | `write_setting_field` 改回「宽松读出整份设置 → 只改目标键 → 全量落盘」 | **5 条**：`setter_preserves_other_fields_when_one_field_is_invalid`、`setter_keeps_unknown_keys`、`setter_backs_up_syntax_broken_file_before_writing`、`write_setting_field_preserves_other_fields_and_unknown_keys`、`write_setting_field_backs_up_syntax_broken_file` |
| ② | `DEFAULT_IMAGE_HEIGHT_PX` 40 → 24 | **2 条**：`settings::legacy_json_missing_appearance_fields_uses_pre_change_defaults`、`ipc::settings::get_appearance_settings_defaults_match_pre_change_behaviour` |
| ③ | `DEFAULT_HIGHLIGHT_STYLE` `"bold"` → `"color"` | **4 条**：`appearance-settings` 默认值、下拉首项为默认档、`popover-appearance` 读取失败回退默认、`highlight-style` 读取失败回退默认 |

## 已知边界

- 坏字段的值不会被自动改写；`.bak` 每次覆盖不累积；`autostart.json` / `hotkey.json` 仍是整份读写（键少、未纳入本轮）。
- 备份与写入不是原子对（先 copy 再 write）：中间崩溃会留下完整 `.bak` + 未更新的原文件，不丢数据。
- tooltip 文案是硬编码中文：**改默认值必须同步改文案**，目前靠 `settings-tooltips.test.tsx` 的逐面板断言兜住明显缺漏（断言的是「有 tooltip 且写明默认口径」，不是把每个默认值都与 Rust 常量比对）。
- `numberOfCopies` 排序依赖 P3-8 的 `copy_count`（迁移 v4），未做。

## 未验证 / 待人工

- **两处默认视觉变化**（高亮「纯加粗」、缩略图 40px）未在真机看过；`color` 档用 OKLCH accent token 替代 Maccy 的 `findHighlightColor`，观感差异也未验证。
- 各档预览延迟（200ms / 1500ms / 100000ms）的实际体感、长列表切排序后的滚动位置、分组口径切换后的观感。
- 打包 GUI 下的表现（含 P5-8 的 `eprintln` 留痕在打包应用里基本不可见——与 P4-7 同一限制）。
- Windows 全链路（本项无平台相关代码，但未在 Windows 编译验证）。
- **收尾三个 commit 未经独立审查**（首轮 `07fd8f6` 的审查结论为「非 BLOCK」，见台账）。

## 补记：审查裁决后的调整（commit `6c2e7a7` / `282eac3` / `4d11dc4`）

审查（`822719e..222810d`）结论非 BLOCK，几处裁决已按「先 Maccy、无则业界」落地：

1. **写路径改为拒写**（原先是「备份后从空对象重建」）：语法坏 / 根不是对象时返回 `SettingsError::Unparsable`，中文文案带修复指引与备份路径，**原文件逐字节不变**。理由：无法解析的文件里可能全是用户真实设置，重建后写回等于删掉它们。业界通行做法；Maccy 无对应（UserDefaults 逐键、无整文件概念）。
2. **备份带时间戳不覆盖**：`settings.json.<unix_ms>.bak`，同毫秒同名加 `-1`/`-2` 序号；连续两次损坏留两份留痕。备份失败（目录只读）同样拒写且原文件不动。
3. **空文件例外**：0 字节/只有空白按「还没有设置」处理，允许首次写入（否则 `touch settings.json` 之后任何设置都写不进去，且大量既有测试用 `NamedTempFile::new()` 的空文件桩）。
4. **读路径整份回退提示一次**：进程级 `AtomicBool` 去重，flag 由调用方注入以便单测「第一次 true、第二次 false」；回退行为本身没变。
5. `update_settings_value` 文档补并发前提（同步命令、无锁；改 async 必须加互斥）。
6. 前端两处一致性小修：`popover.css` 缩略图 fallback `24px → 40px`（与新默认一致）、`ClipPopoverApp` 首个预览注释改为「默认 1500ms，可在设置中改」。
7. tooltip 测试从 `/^默认：/` 收紧为字面值断言（最近复制 / 加粗 / 40px / 1500ms / 开），默认值改了但文案没改时必须红。

**复核数字（`4d11dc4` 之前一树，含本轮全部改动）**：`cargo test` **854 passed / 0 failed**；设置相关 vitest 子集 **31 files / 350 passed**；前端全量 **80 files / 868 passed**；`tsc` 退出 0；`cargo fmt --check` 退出 0；`clippy -D warnings` 干净。

**已知边界的更正**：`03a84d0` 的提交描述写「坏字段不再连累整份设置」只覆盖**类型错误**。**语法错误仍然整份回退默认值**（读路径拿不到任何字段），只是现在「写路径拒写 + 读路径启动期提示一次」，所以默认值不会被写回文件。这条已写进 P5-8 台账的「已知边界」。

## 补记二：错误文案透到界面（commit `7bbad50`）

审查验收项要求「确认 IPC 错误能传到 UI，不要吞」。核查后发现两处真正的缺口：
- `AppearancePanel` 四处保存失败只显示通用文案（后端原文只进 console）；
- `GeneralPanel` 的三个开关（开机自启动 / 托盘常驻 / 自动检查更新）失败时**只 console.warn**，
  连提示都没有——对用户表现为「点了没反应」。

改法：`useGeneralSettings` 增加 `saveError`（成功一次即清空），通用面板用 `role=alert` 展示；
外观面板改透出后端原文。两处都补了用例，其中外观面板的用例直接断言
「已拒绝写入」与备份文件名出现在 UI 上——这条就是「IPC 错误没被吞」的证据。

其余面板（隐私 / 存储 / 热键 / 翻译源）仍是各自的通用文案，统一成设置页级错误条属独立小项，未做，已登记在 P5-8 已知边界。

## 补记三：两处契约锚定（commit `902f7d5`）

审查里有两条属于「配置/注释」层面，原本无测试兜底，补了两个便宜但有效的锚：
- `popover-style.test.ts`：读 `popover.css` 断言缩略图高度走 `--popover-thumb-height` 且 fallback = **40px**，并与前端常量 `DEFAULT_IMAGE_HEIGHT_PX` 逐值相等——审查指出的 `24px` 遗留由此锁死。
- `settings.rs::legacy_highlight_default_value_falls_back_to_bold`：旧高亮值 `"default"` 不做 serde alias，读路径回退默认档 `bold` 并计入字段级问题清单。

**最终复核（`902f7d5`）**：前端全量 **81 files / 871 passed**，设置相关子集 **45 files / 495 passed**，`tsc` 退出 0；`cargo test` **855 passed / 0 failed**，`cargo test settings` **111 passed**，`cargo fmt --check` 与 `clippy -D warnings` 干净。

## 补记四：复审两条（settings）+ clipboard flake（commit `30de19d` / `8c9fbd7`）

**settings 复审两条**（`30de19d`）：
1. **空文件读写一致**：读路径 `lenient_from_json_str` 对 0 字节/纯空白直接返回默认值，不计入整份回退、不产生问题清单、不消耗 `CORRUPT_SETTINGS_WARNED`——此前空文件会被当成「语法错误」并吃掉那唯一一次提示（写路径早已按空对象处理，两侧终于同口径）。
2. **提示复位**：提示逻辑抽成 `apply_corrupt_warning(flag, whole_file_fallback, problems)`——整份回退只提示一次；**成功解析为 JSON 对象（含只有字段级坏值）时复位标记**，于是「损坏→提示→修复→再损坏→再次提示」成立。否则同一进程里用户修过一次之后，再坏就永久静默。
3. 台账两处表述订正：`-1/-2` 序号是**同一毫秒内的重名冲突计数**，不是备份总量上限；同内容备份不去重（只在用户主动操作时产生、增长缓慢，暂不改）。
4. 测试（注入局部 `AtomicBool`，风格不变）：只提示一次、修复后复位再损坏可再提示、空文件不算整份回退且不消耗提示机会、`load_lenient` 完整链路（空文件零问题 → 随后损坏报 1 条）。

**clipboard-page flake**（`8c9fbd7`，仅测试侧）：
- 根因：`waitFor` 谓词用 `screen.getAllByText("Hello World")`，会被**预览区**提前满足；挂载期 `renderCount` 被清零 → 列表某提交 0 行、预览已有首条 → 断言落在中间提交，随 RTL 收尾 `setTimeout(0)` 与 React 调度先后抖动。搜索/类型筛选用例则是**靠预览区的未切分原文**才通过（列表里命中片段被 `<mark>` 切开了）。
- 修法：`waitForListReady()`（`findByRole("listbox")` + 在子树内等首行）替换全文 33 处同模式就绪门；首条用例三条断言并入同一 `waitFor`；筛选类断言改「行数 + 整行 `textContent`」。
- 业务代码未改。`total=0` 时把 `renderCount` 清成 0 的缺陷本身**未修**，已登记到 P4-2（需先补 `total=0` 单测）。
- 验收：单文件 37 passed；**14 路并发各跑一次全部通过**（14 × 37）；`tsc` 0；全量 871 passed 连跑两次。反证：把就绪门期望文本改成不存在 → 报 `Unable to find an element with the text: …`（真回归仍会红）。

## 补记五：按 Maccy「有则照搬、无则业界」裁决的第二批（`93ef41b` / `e9e73ad` / `1e5dde7` / `3f00c05`）

四条独立 commit，裁决依据统一：Maccy 用 UserDefaults 逐键存储、列表用 LazyVStack 惰性渲染，
所以它既没有「窗口状态被压成 0」，也没有「整文件被覆盖」和「设置备份」这些概念——凡 Maccy 没有的，
按业界成熟做法补。

1. **挂载期窗口不清零**（`93ef41b`，P4-2 修真缺陷）：`clampToTotal` 在 `total <= 0` 时保留窗口；
   `initialRenderCount(0)` 取首屏块；`growRenderCount(current, 0)` 不动也不清零。数据到达后的
   第一个提交即渲染全部行。集成用例用 MutationObserver 观察「首个渲染批次的行数」——反证时把实现改回旧版
   会看到 `[0,3]`，断言红；保留测试侧的 `waitForListReady` 不回退。
2. **设置页级错误条**（`e9e73ad`）：新增 Context 通道（`reportSaveError` / `clearSaveError` / `runSave`）
   + `SettingsSaveErrorBanner`，六个面板全部接入，失败显示后端中文原文、成功自动清空；控件仍受控回滚。
   顺手消除了三处「点了没反应」：通用面板只 `console.warn`、隐私/热键/翻译源只给通用文案、存储的图片阈值失败只打日志。
3. **hotkey/autostart 逐键读写**（`1e5dde7`）：抽出通用 `update_json_object`，两个配置各自得到
   `save_enabled` / `save_accelerator`；损坏文件备份后拒写。关键坑：热键 setter 原本在**写之前**用严格读，
   损坏文件根本走不到写路径 → 改成 `load_or_default` 容错读，才让「备份 + 拒写」生效（落盘失败仍注销新键）。
   `SettingsError::Unparsable` 因此多了 `file` 字段，错误文案按实际文件名生成。
4. **备份滚动保留 + 去重**（`3f00c05`）：只留最近 5 份，与最近一份字节相同则复用；`list_backups`
   只认 `<文件名>.<ms>.bak` / `<文件名>.<ms>-<n>.bak`，不会误删别的配置的备份。

**复核（HEAD 上实跑）**：`cargo test` 869 passed / 0 failed、子集 `cargo test settings` 见汇报；
前端 `tsc -b` 退出 0、全量 vitest 82 files / 881 passed（设置页级错误条新增 8 条用例后）。

## 补记六：复审批次 A–L（`ae0727b` / `7fe43b4` / `dd0a913` / `a4543f9` / `93bb299` / `8eb7a12`）

按「Maccy 有则照搬、无则业界」逐条落地：

**A + K 热键读取容错**（`ae0727b`）：`HotkeyConfig::load_lenient` 逐键读到可用值，整份损坏时
仍返回默认键位并附 `corrupt { file, backupPath, hint }`；读路径顺手做一次保护性备份，保证提示里的
路径真实存在。`get_hotkeys_impl` 不再返回 Err，面板在键位行上方展示「已备份到 X，请修复或删除后重启」，
删掉「稍后重试」。`lib.rs` 启动期注册改用同一函数——坏文件不再导致「没有热键可用」。

**E + I 同键早退与回滚日志**（`7fe43b4`）：早退前先 `ensure_writable_json_object`，损坏即「备份 + 拒写」；
回滚注销失败、改键后注销旧键失败都补 `eprintln`（返回值不变），残留风险记入台账边界。

**F 备份 prune 防时钟回拨**（`a4543f9`）：`prune_backups(path, keep)` 显式排除刚创建的那份；
时间源可注入（`backup_broken_settings_at`）。反证：去掉 `keep` 过滤后用例立刻红（「刚创建的备份必须还在」）。

**C + B + D + G + J 错误条按来源隔离**（`93bb299`）：通道改为
`reportSaveError(source, msg)` / `clearSaveError(source)` / `runSave(source, action)`，
六个面板全部带来源；成功只清同源；`useGeneralSettings` 改为按开关分别记录 `saveErrors`，
GeneralPanel 用 `reportedSources` 记住报过谁——挂载时不误清别处错误。无 Provider 时兜底改
`console.error` 不静默；存储统计**读**失败改用面板内 `statsLoadError`，不再冒充保存失败。
（G/B 并入本提交：同一模块与同一批成功路径的原子改写，拆开会出现编译不过的中间态。）

**H 删空收回窗口**（`8eb7a12`）：`filteredItems` 由非空变空时收回首屏块并清空选中，
避免「滚到底扩到 300 → 删空 → 回填」一次性渲染 300 行。反证：去掉收回那一行 → 用例红（300 ≠ 100）。
注意该 effect 必须声明在扩窗 effect 之后（同提交内后写者生效），注释与用例都锁住了顺序。

**一处过程失误**：`ae0727b` 里的测试写了 `len() >= 1` 触发 clippy `len_zero`，追了一个 `dd0a913` 修正——
之后每轮提交前都先跑 clippy（不再在提交后才看）。

## 补记七：复审四条真实问题（`9085ad1` / `9325f09` / `7b22f51` / `015bba8` / `648f9ff`）

1. **通用面板错误残留**（`9085ad1`）：原实现靠 `reportedSources` ref + effect 推断「报过谁、何时该清」，
   错误清除依赖组件挂载/卸载时机，卸载重挂后旧错误不会被清；且 hook 里再存一份 `saveErrors` 等于两个真源。
   改为业界做法——**在成功路径显式清除**：hook 的 setter 失败即抛，面板三个开关直接
   `runSave("general.<key>", …)`。补测试：失败 → 切外观 → 切回通用（重挂）→ 同开关成功 → 错误条消失。
2. **隐私跨来源误清**（`9325f09`）：`handleAdd` 成功把 addApp/removeApp 两条一起清（反向亦然），
   于是「添加失败 + 删除成功」会把添加的原因抹掉。改为各清各自；正则来源统一改名为
   `privacy.addRegex` / `privacy.removeRegex`。补测试覆盖该序列。
3. **备份回拨仍会误删上一份**（`7b22f51`）：上一版只让 prune 跳过当次 keep，排序仍看墙钟——
   连续两次回拨时第二次的时间戳更小，会把**第一次刚创建的备份**淘汰掉。改为「写入次序单调化」：
   文件名时间戳取 `max(now, 既有最大值 + 1)`；不选 mtime 的理由是 `fs::copy` 把 mtime 设成当前时间、
   与墙钟同源。反证：改回 `now_ms` 后用例立刻红。
4. **注释与测试名不实**（`015bba8`）：原注释宣称「effect 顺序敏感」，实测对调两者后 39 条用例仍全绿；
   真正承担行为的是 `setHighlightIndex(0)`。注释与测试名已按真实结论改正。
5. **备注 B**（`648f9ff`）：热键损坏提示里路径出现两次（hint 已带 + 面板又渲染一行），删掉多余那行，
   测试收紧为「路径只出现一次」。

**备注裁决的落地**：A（读路径备份副作用）与 C/D/E（同源竞态、展示顺序、启动期备份未实跑）已按 lead
的意思写进台账「已知边界」，不改代码。

## 补记八：备份次序重做 + 三处修正（`d2f68f1` / `662bc0b` / `35fabad`）

1. **备份次序与墙钟解耦**（`d2f68f1`）：`max(now, newest+1)` 会让「一次未来时间戳永久传染」，
   新备份的时间戳再也不是真实时间。改为 `settings.json.<seq>.<ms>.bak`：seq 是写入序号
   （取磁盘最大 seq + 1，跨重启有效），ms 永远是真实墙钟、只作展示；排序只看 seq，
   旧格式备份排在所有新格式之前。同毫秒写入靠 seq 区分，旧的 `-1`/`-2` 后缀分支删除。
   **mtime 依据更正**：实测 macOS 上 `fs::copy` **保留源 mtime**（源 2001 年 → 目标 2001 年），
   此前写的「fs::copy 把 mtime 设成当前时间」是错的；不用 mtime 的真正理由是它可被外部工具
   任意设定、不代表本进程写入次序（Linux 未验证）。
2. **注释修正**（`662bc0b`）：`useGeneralSettings` 的「对外接口形状不变」已过时（saveErrors 已删）；
   `ClipboardPage` 注释里的「实测对调 39 条仍绿」属实验记录，按规范删除、只留行为结论。
3. **自启开关回滚**（`35fabad`）：首次引导的自启开关写盘失败只弹通用提示、不回滚界面（界面撒谎），
   与通用面板同类问题；改为回滚 + 显示后端原文。反证：去掉回滚那一行用例立刻红。

**过程失误（如实记录）**：`35fabad` 的行为改动因暂存失误随 `662bc0b`（注释提交）一并落地，
随后单独提交补了测试与说明，未改写历史。

## 补记九：自启文案残留/悲观更新/备份命名对齐（`94701c5` / `5846d5e` / `7202ffd` / `1c534ce` / `4cf09ec`）

1. **自启成功清旧文案**（`94701c5`）：新增独立状态 `launchOnLoginError`，成功即清。
   不复用 `opError` 的理由：那是面板级共用槽位（删除/粘贴失败也在里面），自启成功而清它
   等于抹掉别人的错误——与本轮「按来源隔离」同一原则。
2. **自启改悲观更新**（`5846d5e`）：先 `await setLaunchOnLogin`，成功才改界面；失败界面不动、
   只显示后端原文。去掉了 `previous` 闭包与回滚逻辑。顺带修正台账/注释表述：与通用面板
   一致的点是「都是悲观更新」，而不是「都是回滚」。反证：退回乐观更新 → 「后端没返回前界面不动」
   用例立刻红。
3. **备份命名对齐 seq 方案**（`7202ffd`）：台账两处旧描述（`<unix_ms>.bak`、「同名加序号」、
   `-1/-2` 冲突计数）改为 `settings.json.<seq>.<ms>.bak`；`backup_path_for` 文档、测试注释同步；
   删掉源码里的过程性记录（「中间实验格式」「分支已删除」）。
4. **删除未发布旧格式解析**（`1c534ce`）：旧格式从未进入用户环境，保留两种「什么算备份」的定义
   只会让 prune 候选含混。`parse_backup_name` 只认 `<seq>.<ms>.bak`；不匹配的文件既不入排序
   也不入候选，因此不会被误删。新增用例：目录里预置 4 个外来文件（`<ms>.bak`、`<ms>-<n>.bak`、
   `settings.json.bak`、另一个配置的备份）+ 连写 6 次 → 本应用格式只剩 5 份，外来文件全在。
5. **并发前提核查**（台账）：`.setup()` 在主线程；37 个设置类命令全是同步 `fn`，Tauri 2.11.2
   宏默认 `ExecutionContext::Blocking` → 内联执行不 spawn；macOS invoke 主线程投递 ⇒ 串行，
   `seq = 磁盘最大 + 1` 无同进程并发写。边界：未用线程 id 断言、Windows/Linux 未验证。
