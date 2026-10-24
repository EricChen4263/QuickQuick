# v0.4 / v0.5 对标 Maccy · Work Items

- 依据方案：[maccy-parity-roadmap.md](maccy-parity-roadmap.md)
- 状态口径：`ready` / `blocked` / `done`
- 每个 item 独立 commit；review 结论需能对应到各自验收条件

---

## Phase 0 收尾

### P0-1 · 修「主线程超时后误报 full_paste」
- **状态**：done
- **结果**：`on_main_thread_blocking` / `hide_clip_popover` / `hide_clip_panel_now` 返回 `bool`；新增 `should_inject_paste` 纯函数；两臂在交出失败时 `return "write_back_only"` 且 `send_paste` 不可达；六个 bool 函数加 `#[must_use]`。独立审查 **非 BLOCK**（6 点全部成立），顺手修掉 ADR 示意代码与实现矛盾、改动范围表漏列 `Cargo.lock`。
- **验收**：全部达成；`cargo check/clippy/test`（557 passed）、`pnpm test`（519 passed）通过。

---

## Phase 1 · 还诚信债（不依赖面板人工验证）

> 主题：**UI 已承诺、代码未接线**。每项都要保证「配置项真的生效」或「从 UI 下线」。

### P1-1 · 敏感内容过滤接线
- **状态**：done
- **做什么**：`is_concealed` 不再硬编码 `false`。新增 `pasteboard_meta` 模块（平台无关纯谓词 + `cfg` 平台 FFI）：macOS 读 `NSPasteboard` 类型集合是否含 `org.nspasteboard.ConcealedType` / `TransientType`；Windows 读 `ExcludeClipboardContentFromMonitorProcessing` 格式存在或 `CanIncludeInClipboardHistory == 0`。
- **结果**：commits `8ee670a`（接线）、`4241255`（复审修复）、`985ac6d`（残留风险登记）。独立审查首轮 **BLOCK**：`NSPasteboard.types()` 在 500ms 轮询线程直调，与主线程 AppKit 并发会 SIGSEGV（reviewer 探针 3/3～4/4 必崩）；修法是把平台标记读取整体搬到主线程（`PasteboardMetaReader` 注入式接缝 + `TauriMainThreadDispatch`，`run_on_main_thread` + 300ms 超时降级 + 节流留痕）。复审结论 **B1 已闭环**，并反向证明「串行化到主线程」是该崩溃的充分条件（串行 3/3 不崩 vs 并行 3/3 崩）。
- **验收**：判定链路真机探针走通（`ConcealedType` → `is_concealed == true`）；纯谓词单测 9 条 + 派发契约/节流 5 条；`skip_sensitive=false` 仍可入库（既有单测未动、绿）。**待人工**：从 1Password/Bitwarden 复制不入库、关开关后可入库（需真实密码管理器 + 查库）。

### P1-2 · 来源应用排除接线
- **状态**：done
- **做什么**：`source_app` 不再恒 `None`。新增 `SourceApp{bundle_id, display_name}`，身份取自**主线程** `NSWorkspaceDidActivateApplicationNotification` 观察者（不在轮询线程查 `frontmostApplication`——该 API 跨线程是 stale，见 `ipc/system.rs:76-79`）；`ExcludeList` 改为大小写不敏感、bundle id 与显示名**双匹配**，空名单先短路。
- **为什么必须双匹配**：前端输入框 placeholder 是「应用名称（如 1Password）」，用户输入的是人类可读名字；只匹配 bundle id 等于让名单永不命中——那还是「设置页撒谎」。
- **结果**：commit `c731277`。独立审查 **非 BLOCK**（偏差理由经核实成立、自我激活不污染、空名单零开销、无调用点绕过来源判定）。Windows 用**剪贴板所有者进程**的可执行名（比前台窗口准：QuickQuick 自己的 popover 会抢焦点）。
- **验收**：8 条隐私单测（名字命中/bundle id 命中/大小写/未命中/空名单/敏感优先）+ 4 条元数据单测。**已知边界**：身份是「最近一次被激活的外部应用」，复制后 500ms 内切走应用可能误判。**待人工**：名单内 app 真机复制不入库。
- **未验证**：Windows 全路径未编译验证（本机无 windows target），符号与 feature 归属已按 `windows-sys 0.61.2` 源码逐一核对。

### P1-3 · 私有 UTI 自产标记
- **状态**：done
- **做什么**：写回剪贴板时追加自定义类型 `com.quickquick.self-marker`，读取时识别并跳过。macOS 用 `addTypes:owner:` + `setData:forType:`（**绝不用** `declareTypes:owner:`，后者会清空 arboard 刚写入的正文）。
- **结果**：commit `d916035`。独立审查 **非 BLOCK**，且 reviewer **自建探针独立复核**了正文保全（逐字节相同、原类型不丢、图片 RGBA 不变、下次正常复制标记必被清）。写入口覆盖完整（文本/图片唯一入口都挂标记）。
- **Windows 降级（明确记录）**：arboard 在 Windows 独占写剪贴板、无法追加自定义格式，改用「写入门闩」一次消费语义；原因写在 `SelfWriteLatch` 与 `mark_self_write` 文档注释里，不是静默糊过去。
- **验收**：4 条纯逻辑单测 + 真机探针。**待人工**：从历史粘贴后历史列表不新增条目、顺序不乱。

### P1-4 · `tiered_cleanup` 自动触发
- **状态**：done
- **做什么**：`image::tiered_cleanup` 此前零调用。触发点放在 `capture_and_ingest` 的写库事务 **RELEASE 之后**，且仅在「本轮新增了图片行」（图片 + `Inserted`）时执行——文本条目、图片同 hash 去重（`Bumped`）、空轮次全部零额外 SQL，避免每 500ms 付一次 `SUM(length(...))`。阈值沿用 `image::DEFAULT_MAX_TOTAL`（500 MiB），与存储页 `StoragePanel` 的「上限 500 MB」同一常量，未新造阈值。
- **结果**：commit `b2245da`。清理在事务外 + best-effort：失败只留痕，不回滚捕获（用 SQL 触发器造真实失败验证）。
- **验收**：9 条新单测（阈值边界「等于不动手」、只腾到回到阈值内、只剩收藏时能正常返回不无限删、报告字段逐级对应、四种入库组合的触发判据、端到端触发、文本轮次不触发、清理失败不阻断捕获、best-effort 无半清理状态）。

### P1-5 · 可移植备份导出/导入接线
- **状态**：done
- **做什么**：`portable.rs` 的密码学层（Argon2id + AES-256-GCM + magic/version 头）此前只被测试调用。新增载荷层（版本化自描述 JSON `{format:"qqbackup",version:1,items:[…]}`，未知字段忽略、未知版本显式报错）与 `ipc/portable.rs` 三个命令（导出/导入/在 Finder 中显示）。
- **关键取舍**：
  - **导入不复用 `db::ingest`**——`ingest` 命中即 bump 置顶，用在导入上会打乱现有顺序；改为「已存在就跳过」的原语，插入按备份原样写 `created_utc`/`is_favorite`。
  - **图片判重键随载荷携带**：降级条目（`originalPresent=false`）原图从未落库、缩略图是 WebP 重编码，字节上都反算不出 `imageHash`。
  - **先解密解析、后写库**：任一步在写库前失败都不会留下半个改动；合并整体在一个 SAVEPOINT 内，任一条失败整批回滚。
  - 落盘 `document_dir → download_dir → app_data_dir` 逐级回退，`.part` + 同目录 rename 保证原子，同秒重名探测 100 次全撞则报错，**绝不覆盖既有备份**。
- **结果**：commits `50e112f`（后端）、`3703ed1`（前端）、`b707ca0`（复审收尾：载荷大小上限 1 GiB + 回退留痕 `usedFallbackDir` + `ipc-client` invoke 三件套）。独立审查 **非 BLOCK**，20 个必核点逐条核实（含用干净 worktree 复跑 625/528 全绿），并印证了三条关键设计判断（imageHash 必要性、口令措辞的密码学正确性、reveal 无执行面）。
- **验收**：跨两个独立加密库导出→导入内容/HTML/收藏/时间戳全等；导入两次第二次 0 新增；已有 `text_hash` 跳过；错误口令明确中文报错且库快照不变（错误口令探针还反证了「体积上限检查在解密之前」）；导入前 `window.confirm` 二次确认有测试锚定取消不调后端。
- **未做（记录在案）**：`skipped` 计数混合了「判重命中 / 缺 image 字段 / 未知 kind」三种原因，前端统一显示「跳过已存在 M 条」，将来做导入体验时应拆成 `duplicates` / `malformed`。
- **待人工**：真机点按钮的端到端（`AppHandle` 目录解析、Finder 弹出、`<input type="file">` 与 `window.confirm` 的真实交互）、macOS 写 `~/Documents` 可能触发的 TCC 授权弹窗、跨机导入（真机密钥派生路径）。
- **未验证**：Windows 全路径未编译验证。

### P1-6 · 翻译缓存接线
- **状态**：done
- **做什么**：`cache_get`/`cache_get_at`/`cache_put` 此前零生产调用。方向解析前移到 provider 构造之前，保证缓存键用的是**最终**语言对；命中缓存零网络、不读凭据、照常写历史。
- **关键取舍**：**只缓存 `Plain`，Dict 不写不读**——缓存表只有一列 `translated`，写 Dict 等于命中时静默退化成机翻样式（用户可见能力损失）；由此得到「表里任何记录必然来自 Plain」的不变量，读取侧无需判 kind。
- **结果**：commit `e43210b`。独立审查 **非 BLOCK**（缓存键语义、不变量封闭性、降级留痕、LRU 不退化、5 个测试的「删接线会红」可失败性全部核实）。
- **验收**：二次同请求 `call_count == 1`、换语言对/换 provider 必 miss、命中仍写历史、Dict 不写缓存（`cache_row_count == 0`）。

### P1-7 · 翻译重试/取消/错误归一再接
- **状态**：done
- **做什么**：`retry_with_backoff` 包住整个 provider 编排（含多步源，绝不跨源切换）；`InflightTracker` 接进命令层单例，`ensure_still_current` 位于写缓存/写历史**之前**；`UreqExecutor` 改走 `map_provider_error` / `classify_timeout`，401/403→Auth、429→RateLimit、5xx→ServerError、超时→Network 真正可达。
- **结果**：commits `970f369`、`7eee773`（复审修复）、`2ce8e95`（补 `is_timeout` 单测）。独立审查 **非 BLOCK**，并逐层查证依赖源码指出一处**注释与事实相反**：同步 `#[tauri::command]` 在主线程执行（`body_blocking` + `WKScriptMessageHandler: MainThreadOnly`），已改正并登记根治方向（`async` + `spawn_blocking`）。
- **已知既有缺陷（非本 item 引入）**：`translate_text` 作为同步命令，其同步网络 I/O 与新增退避都阻塞主线程（既有最坏 10s，现最坏 +1.5s）；根治需改线程模型，超出本 item 范围。
- **验收**：瞬时错误重试 2 次且退避序列断言 `[500]`、永久错误 1 次不重试、被取代的 generation 丢弃且零缓存/零历史、回环 HTTP 实测 401/403/429/503 归一、`is_timeout` 覆盖 TimedOut 与 ConnectionRefused 两种。

### P1-8 · 串行化全部 `NSPasteboard` 访问（新增 · 由 P1-1 复审挂号）
- **状态**：ready
- **为什么挂**：P1-1 复审时探针实证：**同一进程内两个线程并发访问 `NSPasteboard`（含 `types()` 与 arboard 正文读）会 SIGSEGV**。这条崩溃类**早于本轮 Phase 1 就存在**（轮询线程 arboard 读 + IPC 命令线程 arboard 写自 `f4c7211`/`73a64f4` 起并存），P1-1 只消除了自己新增的那一面（把 `types()` 移到主线程）。强行在 P1-1 闭环等于提前做 P3-2。
- **做什么**：把 `pipeline::ArboardBackend` 的全部 `NSPasteboard` 访问（`change_count` 的三段读 + `read` 的三段读 + 写回路径）统一走单一「主线程串行执行 + 结果快照」通道；与 P3-2「`changeCount` 替代内容哈希轮询」一起做（后者会改变去重语义：同一内容复制两次各记一条，属用户可见变化，需单独验收）。
- **验收**：探针「主线程与后台线程同时跑全部读路径，连续 N 次不崩」+ 真机长跑（切 app / 粘贴 / 复制混合）无崩溃。
- **前置**：Phase 3 的集成测试覆盖 → 已由 **P3-0**（`f6ea50e`）完成。
- **实施判断（本轮记录）**：**P1-8 不能单独做，必须与 P3-2 合成一次改动。** 如果只做「把现有内容哈希轮询搬到主线程」，主线程每 500ms 就要做一次全量读（文本 + HTML + 图片 RGBA 哈希，一张 4K 截图 30MB+），明显可见的卡顿换来的只是「崩溃概率降低」而不是结构性消除；如果把 `change_count` 也搬走却仍用内容哈希，问题同上。**正确形态是**：改成 `NSPasteboard.changeCount`（标量，极便宜）在主线程做变化检测 → 只有真的变了才做一次全量读（也在主线程）→ 所有 pasteboard 访问天然串行，崩溃类被结构性消除，且主线程每 500ms 只付一次标量读取。
- **需要用户裁决的一点**：`changeCount` 语义会让**同一内容重复复制各记一条**（现在内容哈希会把它去重/或按 Bump 处理——具体现有语义见 P3-0 新补的契约测试）。这是用户可见的行为变化（Maccy 就是这个语义），需要明确接受后才动。

### P1-9 · 存储页占用口径对齐（新增 · 由 P1-4 复审挂号）
- **状态**：done
- **问题**：存储页拿 `get_storage_stats.file_size_bytes`（**库文件大小**）与「上限 500 MB」比较并画进度条，而 P1-4 的清理删除的是**图片逻辑字节**。SQLite 的 `DELETE` 只把页标记为可复用、不收缩文件，所以清理后进度条不会下降——用户点「立即清理」看不到任何变化，又是一次「UI 承诺与实现脱节」。
- **结果**：commit `c6391ad`（主体）、`6848d6f`（flaky 修复 + 文案收紧）。两个量拆开：`imageBytes`（**图片逻辑字节**，直接复用 `image::total_image_bytes`，未另写 SQL → 与自动清理阈值同一个函数）画进度条并挂「图片占用上限 500 MB」；`fileSizeBytes`（库文件体积/磁盘占用）单列并明说「SQLite 删除只把页标记为可复用，VACUUM 前不会缩小」。图片占用查询失败降级为 0 + 留痕，不拖垮整页统计。
- **验收**：独立审查 **非 BLOCK**，并用 4 组变异证明锚定的是行为不是文本（进度条改回取文件体积 → 2 红；图片占用改用内联 SQL 去掉 `is_deleted=0` → 370 vs 870 红；降级改成 `?` 传播 → 红；清理后不重拉 → 2 红）。前端构例刻意让「库文件 500 MB / 图片占用 100 MB」断言进度条约 20%，并复现「文件不缩、条要降」场景断言 80%→20%。
- **顺带修掉一个既有 flaky**：`StoragePanel.test.tsx` 的阈值用例只 `await` 了「mock 被调用」就同步断言 DOM，负载下读到组件初始值（`expected 10 MB, received 20 MB`）。**审查者给的根因（mock 漏设默认值）被确定性探针证伪**——补默认值单独修不好；真实修法是把 DOM 断言并入 `waitFor`（等状态落地）。commit message 据实写成「竞态 flaky」。
- **未做（记录在案）**：① `file_size_bytes` 只统计主库文件，**不含 `-wal`/`-shm`**，与「磁盘实际占用」的文案严格说仍有轻微出入（既有行为）；② VACUUM 手动入口未做（文案已如实告知「VACUUM 前不会缩小」）。

---

## Phase 2 · 交互地基

> **状态：部分解冻。** 原先把整阶段标 blocked（等 ADR §6.2 的 8 项人工验证），代价是把与面板语义无关的前端交互条目一起冻住。拆分依据与逐项判据见 [`phase2-gate-verification.md`](phase2-gate-verification.md)：
> - **不依赖面板语义**（webview 键盘/交互逻辑，即使面板回退也不需改写）→ 已解冻并推进；
> - **依赖面板语义** → 仍 blocked，等真人结论。
> - 另：§6.2 的 8 项里经评估有 2 项（#1 面板不夺焦、#6 失焦自动隐藏）**可探针化**成可复跑证据，尚未开工。

### P2-1 · 面板高度自适应
- **状态**：blocked（面板尺寸/定位语义）
### P2-2 · 键盘模型升级（`⌃N/P/J/K`、`⌘↑↓`、`⌃W/U/H`、`⌥⌫`、`⌥P`、`⌘,`）
- **状态**：done（可用子集：`⌃N/P/J/K`、`⌘↑↓`、`⌃W/U/H`；`⌥⌫`/`⌥P` 属 P2-8、`⌘,` 未做）
- **结果**：commits `944e03b`、`2577580`（`⌥↑↓` 回归修复）、`e1fccbd`（注释）、`4e3ce3d`（测试加固）。独立审查要求补的可失败性已补：删掉 `useLayoutEffect` 光标恢复 → 3 条测试红（含新增「光标中段」用例，避开受控 input 自然落尾的巧合通过）。
- **已知边界**：`⌃W` 只按空白+常见中英文标点分词，中文无标点长段会整段删除（如 `"昨天的会议记录整理稿"` 一次删 8 字）——已写进 JSDoc，加中英边界谓词属后续项。
### P2-3 · 数字直达（`1`–`0` 取前 10 条）
- **状态**：done
- **结果**：commits `f86d714`、`c11f85f`（复审修复）、`4e3ce3d`。仅「搜索框为空且无修饰键」时生效；50ms 高亮反馈后取用；越界不做任何事。
- **审查打回的 BLOCK（已闭环）**：50ms 待取用定时器原先只被「数字 / Esc / 卸载」取消，Enter / ⌥Enter / 打字 / 方向键都会留下它继续触发——探针实证「同一条目粘两次」「粘贴+复制双发」「用户已在打字却粘了一条」。修法是非数字键在 `handleKeyDown` 顶层统一取消（放在 IME guard **之前**，因为组词期间的按键同样意味着用户在打字）。
### P2-4 · 字母 pin（`b`–`y` 持久化键位 + 设置页管理）
- **状态**：blocked（需 `pin` 列 + 迁移框架；建议与 Phase 4 的迁移版本化一起做）
### P2-5 · cycle 模式（按住热键连按翻页、松手即粘贴）
- **状态**：blocked（依赖面板吞 ⌘V 语义与全局 `flagsChanged`）
### P2-6 · 输入法防御（composition 期间禁用方向键导航）
- **状态**：done
- **结果**：commit `4a69788`。纯函数 `isImeComposing` 同时覆盖 `isComposing` 与 `keyCode === 229`，popover 与主窗两个入口都加防御。
- **一处被审查明确保留的设计**：Esc 位于 composition 防御**之前**（组词期间 Esc 仍能关窗）。理由：被丢的只是我们自己搜索框里未上屏的拼音，代价极低；而「让 Esc 穿透」在本 webview 上不保证输入法会消费，可能出现「Esc 什么都不做、面板关不掉」。**该结论仍需真机定性**（见下）。
### P2-7 · 主窗键盘流修复（Enter 粘贴、`⌘1-9` 直接粘贴）
- **状态**：done
- **结果**：commits `f76fc51`、`4b44e37`（复审修复）。`resolveEnter` 的返回值不再被丢弃，Enter 与 `⌘1-9` 都复用鼠标点击的同一条取用路径（`write_back_only` 降级横幅照常出现）；复审后 `⌥Enter` 也改成「仅复制」与 popover 对齐。
### P2-8 · popover 管理动作（pin / 删除 / 清空）
- **状态**：blocked（要在面板内操作 + 后端 IPC 与刷新时序）
### P2-9 · 唤起位置可选（光标/图标/中心/上次 + 多屏）
- **状态**：blocked（多屏/全屏 Space 行为与面板耦合，等 §6.2 #5）
### P2-10 · 行内预览分级延迟（首 1500ms → 后续 200ms）
- **状态**：done
- **结果**：commits `db717d0`、`9887f23`（复审修复）。档位判据用「本次面板会话是否已显示过预览」（不是时间戳），「只显示最后一个」靠 effect 里 `clearTimeout` + 重排实现。独立审查 **非 BLOCK**（3 处变异全红：首档改 200→红 3 条、删 `clearTimeout`→红 2 条、改组合优先级→红 2 条）。
- **复审修掉一个用户可见问题**：延迟窗口里列表有高亮项、预览区却显示「无选中项」，语义相反。改为 discriminated 三态（`item`/`waiting`/`empty`）+ 纯函数 `resolvePreviewState`，把判定从 JSX 里拿出来；两个方向都被变异夹住（退回「null 一律 empty」→3 红；改成「null 一律 waiting」→2 红）。
- **本项的真实目的**：预览数据本来就在内存，延迟不是为了省请求，而是**快速翻页时不闪**。
- **待人工**：1500/200ms 手感、「预览稍后显示」措辞与样式。
- **如实标注**：复审修复本身未做第二遍独立审查（改动小、附双向变异自证）。
### P2-11 · 快捷键徽标随修饰键变
- **状态**：done
- **结果**：commit `39511f4`。四层徽标（默认/⌃/⌘/⌥），默认层收下「`1–0` 直达」——这是上一轮新增但零 UI 线索的能力，discoverability 的主缺口。独立审查 **非 BLOCK**，并独立核实「⌘ > ⌃ > ⌥、⇧ 无独立层」的优先级与真实分发一致（`handleMoveKey` 首行 `if (e.metaKey) return false`，所以按住 ⌘ 时 `⌃N/⌃P` 根本不生效）。
- **已知边界**：只跟踪修饰键自身的 keydown/keyup，不读 `e.ctrlKey` 标志位——「先按住 ⌃ 再唤面板」会显示默认层（键位仍生效）；`onKeyUp` 只挂 input，焦点在面板内但不在 input 时松开会短暂卡层，已有 blur 与 `onFocusChanged(false)` 两条兜底。
- **待人工**：5 个徽标在底部条是否溢出、「换一批」缺 opacity 过渡的视觉跳动。

### Phase 2 遗留挂号（本轮记录，未立项）
- 主窗 `⌘0` 取不到第 10 条（`quickSelectIndex` 只认 1–9）。
- `⌃W` 中文无标点长段整段删除。
- `⇧↑↓` 在搜索框内不能选文字（**既有问题，非本批引入**）。
- 新键位与组合键矩阵（`⌘⇧↑↓`、`⌃⇧N` 等）需要一次统一决策。

---

## Phase 3 · 捕获与规则

### P3-0 · 补集成测试覆盖现有捕获/粘贴路径（前置 · 已完成）
- **状态**：done
- **结果**：commit `f6ea50e`。新增 `src-tauri/tests/capture_contract.rs`（14 条契约测试，纯测试、`src-tauri/src/**` 零改动）。覆盖审计列出 12 条真缺口并逐条补上，重点包括：重复复制时的 Bump/刷新语义、混合图文产生两条且 HTML 端到端进库、图片阈值降级 + 「正好等于阈值不降级」边界、隐私三层在 poll 层的可观察结果**且跳过时基线仍推进**、计数回退零读取、空快照、`source_app_identity()` 侧通道（此前全仓零覆盖）、粘贴降级 `write_back_only` 的联合事实。
- **独立审查**：**非 BLOCK**。审计表 35 处引用逐一 grep 核实零造假；14 条全部锁可观察行为而非实现细节；审查者另做 5 处变异全部实证打红（含 `>` 改 `>=` 打红阈值边界、图文只产 1 条打红、写入即粘贴打红降级）；确定性无隐患（零 sleep/零网络/不依赖 `top_id`）。
- **审查结论**：这套护栏能拦住 P3-1，且**没有一条会堵死 P3-1/P3-2/P3-3 的路**——只有第 1 条在 P3-2 改语义时**必须有意修改**，已在 P3-2 条目里显式挂钩。

### P3-1 · 换掉 arboard，直连 `NSPasteboard`
- 前置：~~先补集成测试覆盖现有捕获路径~~ → 已由 **P3-0** 完成。动手前先读 P3-2 的「验收时同步改测试」与 P4-1 的 `top_id` 子项。
### P3-2 · `changeCount` 替代内容哈希轮询（消除每 500ms 全量读取）
- **前置**：与 **P1-8** 合成一次改动（见 P1-8 的「实施判断」：只搬线程不改 changeCount 等于用主线程全量读换概率降低）。
- **验收时同步改测试**：`tests/capture_contract.rs::repeated_identical_text_copy_dedups_and_bumps`。该测试当前锁定「同文本二次复制 → `Bumped(原 id)` + 行数不变 + `last_modified_utc` 刷新」。若采纳 Maccy「同一内容重复复制各记一条」的语义，这条的「行数仍为 2」「`Bumped(原 id)`」需改为「新增一行 + `Inserted`」，并在 commit 里说明语义变更理由——**红本身就是信号，不要悄悄改绿**。
### P3-3 · pasteboard 类型白名单（可配）
### P3-4 · 文件类型条目（捕获 + 图标 + 复制回写）
### P3-5 · 忽略规则三层增强（应用黑/白名单 + 类型集合 + 内容正则）
- **状态**：部分完成（内容正则 + 名单黑白模式 done；类型集合 blocked）
- **结果**：commits `67dac67`（后端）、`74faf56`（前端）、`a9345c4`（复审修复：阶梯测试锁住 Concealed↔ContentRegex 相邻对 + 白名单来源未知提示）。
- **判定顺序（契约，已钉死）**：`Paused → SkipOnce → Concealed → ContentRegex → 名单规则 → SelfMark → None`。**加变体的坑**：原来 `CapturePolicy` 在 `src-tauri/tests/**` 有 30+ 处字面量构造，新增字段会让刚落地的 P3-0 契约护栏整体 E0063；故新规则挂到 `IgnoreRules`（`ExcludeList` 的语义化别名，应用名单与正则是两个独立字段），`should_skip*` 签名不变。
- **失败方向（已定）**：非法正则只留痕 + 丢弃该条 + **不跳过**（写错规则的代价应是「这条没生效」，不是「剪贴板被一条写错的正则全吞」）；设置侧原文照存，用户才能在面板里修正半成品。
- **白名单语义（已定）**：空名单 = 视同未启用（一开就全吞会先丢数据）；非空但来源身份未知 = 不记录（白名单是「证明它在名单内才记」），设置页已补提示文案。
- **审查打回的 BLOCK（已闭环）**：优先级阶梯测试漏了 `Concealed ↔ ContentRegex` 这一相邻对——第 3 层用无正则规则集、第 4 层显式 `is_concealed=false`，两层从未同现，把正则提前也不会红。修法是把阶梯改成「每步同时命中所有更弱层、只关掉一颗条件」，并另加最小对照 + 反向对照的相邻对专测。
- **类型集合忽略 = blocked**：依赖 P3-1 的 `types()` 遍历能力，建议与 P3-1 合并落地（规则集已是现成的挂载点）。
- **未验证**：Windows `GetKeyState` 分支未编译（符号/feature 已按 `windows-sys 0.61.2` 核对）；真机面板 chip 交互。
### P3-6 · 暂停语义补齐（托盘 `⌥` 点击、`⌥⇧` 仅忽略下一次）
- **状态**：done
- **结果**：commits `2e67530`、`a9345c4`。`tray_click_action` 纯函数（`⇧` 单独按退回「显示」，避免按错键时什么都不发生）；修饰键读取 macOS `NSEvent::modifierFlags_class()`、Windows `GetKeyState`；tooltip 三态 + 菜单文案反映暂停态。
- **`skip-once` 的关键取舍**：消费点在**轮询循环层**，判据是 `change_count() > last_seen`（与 `poll_once_with_policy` 同口径）。**不能**用「本轮入库结果为空」——那时内容已进库；**也不能**让空轮消费——用户按完 `⌥⇧` 后往往几轮没复制，一次 swap 就把机会白吃了。与暂停的可观察行为一致，故合流进 `policy.paused`，原因靠当轮规则副本的 `RoundSilence::SkipOnce` 区分。
- **已知边界（注释已写明）**：若置位后第一次变化恰好是本工具自写回（自产标记），这次机会会被用掉——探针只看到「剪贴板变了」，与用户「这一次别记」的心智一致。
- **审查结论**：**BLOCK 1 条（纯测试缺口）已闭环**；其余 15 点成立，含独立核实「空轮不消费」「当轮标记不泄漏到下一轮」「tooltip 无永久停留路径」「托盘与设置页共用同一份实现」。
- **风险（中，只能真机定）**：Tauri `TrayIconEvent::Click` 不含 flags 字段，只能在 handler 里读「当前」修饰键；事件经 tao event-loop proxy 转发，用户若在 handler 执行前松开 ⌥/⇧ 会退化成「显示窗口」。**未编译验证 Windows**。
### P3-7 · 多 pasteboard item 合并
### P3-8 · 去重精度提升（集合包含判定 + 复制次数累加）
### P3-9 · OCR（图片可被文字搜索命中，Swift helper + IPC）

---

## Phase 4 · 存储与性能

### P4-1 · 建索引（`is_deleted+last_modified_utc`、`text_hash`、`image_hash`）
- **状态**：done
- **结果**：commits `af474c1`（索引 + 计划测试）、`153f066`（复审修复：计划测试的空断言与列序可证伪）。5 条索引，全部 `CREATE INDEX IF NOT EXISTS`、无表结构变更：
  `idx_clip_items_live_favorite_modified(is_deleted,is_favorite,last_modified_utc)` / `idx_clip_items_text_hash(text_hash,is_deleted)` / `idx_clip_images_image_hash(image_hash,is_deleted)` / `idx_clip_images_live_favorite_created(is_deleted,is_favorite,created_utc)` / `idx_clip_images_clip_item_id(clip_item_id)`。
- **三处按实测而非字面做的判断**：① `is_favorite` 必须进列表索引——否则计划里仍留 `USE TEMP B-TREE FOR ORDER BY`（照 roadmap 字面建会退化，审查用变异确认这是承重墙）；② hash 索引必须带第二列 `is_deleted`——只建单列时在生产那些**没有 `sqlite_stat1` 统计**的库上，规划器会改走 `is_deleted` 前缀索引即扫全部未软删行；③ `clip_images(clip_item_id)` 是必要项——外键级联删除实测 10k 父行 × 2k 子行 0.47s → 0.01s，且消除列表 LEFT JOIN 的自动索引。
- **明确不建**：`(is_deleted, last_modified_utc)` 单列变体（不消除临时排序，白付写入成本）；`clip_images(is_deleted)`（`total_image_bytes` 是全量聚合，索引无法覆盖）；`top_id` 专用索引（见下方子项）。
- **审查打回的弱点（已闭环）**：计划测试里有一句 `!after.contains("SCAN clip_items")` 是恒真断言（SQL 用别名 `ci`，计划串写 `SCAN ci`），且「hash 索引列序为什么必须是 `text_hash` 在前」这条理由没有任何测试钉住（变异成 `(is_deleted, text_hash)` 是 0 红）。已修正别名、把断言升级为带约束串（`(text_hash=? AND is_deleted=?)`）、并新增「不得退化成另一条索引」的反向断言；现在列序互换会红。
- **量级观测（不作时间断言）**：2 万条 / 500 图，文本去重 1.6ms → 53µs（~30×），图片去重 62µs → 19µs；**列表查询墙钟基本持平**（35.7ms → 25.3ms，大头是 Rust 侧全量行映射），其收益是计划形态（去掉临时排序）而非可测墙钟——不要对外声称「列表提速」。
- **未验证**：真机落盘 SQLCipher 大库首次升级的建索引代价、5 条索引对 ingest 尾延迟与开库成本的影响、Windows。
- **顺带子项（P3-0 审查登记，不单独立项）**：`db::top_id` 用 `ORDER BY last_modified_utc DESC LIMIT 1`，缺 `rowid DESC` 兜底，与 `list_items_with_images` / `list_ordered` 不一致——同毫秒并列时两处「最前条目」口径会给出**相反**答案（审查用 2 万轮探针实测：并列 4904 次里 `top_id` 与 `list_ordered` 首元素 100% 不一致）。但生产代码无调用点（仅 `tests/clipboard.rs` 两处测试用），且当前不可复现 flaky。**修法不是简单加一行**：加 `, rowid DESC` 会让那两个测试红（它们的期望行恰好是低 rowid），正确做法是同步把断言改成不依赖并列时选哪行（参考 `capture_contract.rs` 第 1 条的哨兵时间戳 + 相对关系写法）。
### P4-2 · 分页 + 虚拟滚动
### P4-3 · 缩略图移出 IPC payload（消除 base64 全量重传）
- **状态**：done
- **结果**：commits `c661590`（后端）、`77fc4cb`（前端）、`1469e79`（复审加固）。`ClipItemDto.thumbnail_data_url` → `has_thumbnail: bool`；新增只读函数 `db::list_items_with_image_flag`，缩略图 BLOB **连出库都不做**（投影 `CASE WHEN thumbnail IS NOT NULL AND length(thumbnail) > 0 THEN 1 ELSE 0`）；新增 `get_clip_thumbnail(image_id)` 复用 `image::get_image_thumbnail`（不另写 SQL），不存在/软删/零字节 BLOB 一律 `Ok(None)`；前端模块级 `settled` 缓存 + 在途去重，`invoke` 不进组件（可测「只拉一次」）。
- **为什么保留 `list_items_with_images`**：`src-tauri/tests/capture_contract.rs` 有 3 处断言 `row.thumbnail` 非空，改行结构会让刚落地的 P3-0 契约护栏编译不过。故走「加只读函数 + 两个变体共用 SQL 骨架」；计划测试用 `assert_eq!(flag_plan, blob_plan)` 防「改一个忘另一个」。
- **payload 口径（可复现，勿引用单一数字）**：改后 10 条含图条目 JSON = **2,301 B（230 B/条），与缩略图体积完全解耦**（三档输入——高熵噪点/渐变/纯色——均为 2,301 B）；改前 = `2,301 + 1.33 × 缩略图字节总和`，所以「改前」不是一个固定值：噪点图档 523,955 B（228×）、渐变档 20,423 B（8.9×）、纯色档 5,275 B（2.3×）。**对外只说「改后固定 230 B/条」**，不要说「228×」。
- **失败策略（刻意）**：取图失败也缓存为 null、不自动重试——取不到通常是该行已被清理或 BLOB 为空，重试只会把滚动放大成请求风暴；代价是瞬时故障要重开窗口才自愈，已写进注释。
- **审查打回的两处（已闭环）**：① 计划断言只在「计划串」层比较，**去掉 JOIN 的 `cimg.is_deleted = 0` 不会红**（两变体共用宏，计划逐字相同但语义漏了软删图）；已加 SQL 字面锚（`cimg.is_deleted = 0` / `length(...) > 0` / `ci.is_deleted = 0`），删任一条件必红。② 缓存读取有两处互相兜底（改一处仍绿）；已收敛到单一入口 `trySettledClipThumbnail` + hook 去掉 state 副本（渲染期派生读 + `forceRender`），两个调用点各有独立断言。
- **未验证**：真机列表滚动时的实际 IPC 频率与首屏延迟、失败永久缓存在 SQLCipher 大库上的可接受性、占位 `min-height` 与真实缩略图的视觉衔接。
### P4-4 · 自动淘汰 + 可配上限（1–999，pin 豁免）
### P4-5 · 搜索升级（预计算 title + 4 模式 + 多词 AND + 命中高亮）
### P4-6 · 翻译历史分页
- **状态**：done
- **结果**：commits `95d3045`、`fb5d366`（复审补强）。`TRANSLATE_HISTORY_PAGE_LIMIT = 200`（历史栏一屏上限，钉住 payload）+ `TRANSLATE_HISTORY_RETENTION = 500`（2.5× 分页上限，给表体量上界）；查询加 `LIMIT ?1` 且保留 `created_utc DESC, rowid DESC`；写入后 best-effort 清理，**先 `COUNT(*)` 短路**，只有超限才跑一次 `DELETE ... OFFSET 500`。
- **收敛点**：只在写入路径（唯一写入口），读路径保持纯读——`list_translate_history` 会被挂载与事件反复调用，读里顺手 DELETE 会把纯读变隐式写、还可能撞写锁。代价：**旧库只在用户下一次翻译时收敛**。
- **清理失败不影响主链路**：吞错 + `eprintln`。测试用 `BEFORE DELETE` 触发器「先插审计行再 `RAISE(FAIL)`」——比只读连接好在「写入成功、清理失败」这个真形态能构造出来，且审计行证明清理**真的被尝试过**，避免恒真假绿。
- **审查打回的「测试拦不住退化」（已闭环）**：把 `total <= RETENTION` 改成 `total < 0`（每次写入都跑 DELETE）原本 **0 红**。触发器方案在这里**必然假绿**——未超限时那条 DELETE 即使执行也是删 0 行，而行级触发器 0 行匹配就 0 次触发。改用 `sqlite3_set_authorizer` 在 **prepare 阶段**统计 `SQLITE_DELETE`，删 0 行也会计数；并配**正向对照**（超限写入后计数必须变 1），否则「0 次」可能只是探针没接上。
- **未验证**：加密库上 `COUNT(*)` 与 `DELETE ... OFFSET` 的实际耗时、旧库万级首次翻译的一次性收敛耗时（与主写同锁的尾延迟）、Windows。
### P4-7 · 迁移框架版本化（`PRAGMA user_version` 承担 schema 版本）

---

## Phase 5 · 产品化

### P5-1 · 托盘菜单补齐（暂停/清空/设置/检查更新/版本）
### P5-2 · 系统通知
### P5-3 · 音效（捕获/取用，与通知开关解耦）
### P5-4 · 首次引导向导（多步 + 授权后自动恢复）
### P5-5 · 设置项扩充（排序、图片高度、预览延迟、命中高亮、容量上限、弹出位置）
### P5-6 · i18n 框架 + 中英双语（namespace 分区 + 出货白名单）
### P5-7 · 设置页搜索

---

## Phase 6 · 工程加固

### P6-1 · CI 加验证（PR/push 跑 `make verify`）
### P6-2 · 覆盖率基线
### P6-3 · 死代码检测（knip / cargo-udeps）
