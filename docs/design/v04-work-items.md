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
- **状态**：done（与 P3-2 合成一次改动，commit `71d18f8`；活动卡 `2026-10-25-pasteboard-serialize-changecount.md`）
- **结果摘要**：新增 `pasteboard_serial` 主线程串行通道（带超时、panic 拦截、已在主线程则就地执行）；轮询两阶段（无锁观察 → 持锁入库）；P1-1 的 `MetaDispatch` / `MainThreadMetaReader` 被通用通道取代并删除。
- **验收实跑**：探针 `examples/pasteboard_serial_probe.rs` 4 后台线程 × 700 轮 + 主线程并发，3/3 次 0 崩溃；变异 `--no-dispatch` 3/3 崩溃；`cargo test` 810 通过。
- **待人工**：真机长跑（切 app / 粘贴 / 复制混合）；4K 大图主线程全量读体感。
- **未验证**：Windows 全路径未编译（仅 cfg 翻转的 Linux 副本 `cargo check --lib`）。
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
- **状态**：design（前置已满足：迁移框架 P4-7 已完成；**设计文档 `docs/design/pin-letter-design.md` 尚未落盘**，派发的 `pin-architect` 中断前无产出）
- **迁移编号**：占用 **v3**；P3-8 的 `copy_count` 预留 **v4**。实现前必须重审 P4-4 淘汰豁免条件（`is_favorite` 与新 pin 列的关系）。
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

### P2-12 · 悬停与键盘仲裁（鼠标真实移动才交还主导权）
- **状态**：done（lead 复核收口；活动卡 `2026-10-25-hover-arbitration-p2-12.md`）
- **结果**：`keyboard-nav.ts` 新增 `InputOwnership` 仲裁器（键盘导航收回主导权；仅坐标真实变化的 pointer 移动交还鼠标）；`PopoverList` 悬停改选中受 `isPointerOwner()` 守卫；`ClipPopoverApp` 在方向键/首尾/数字直达/搜索输入/面板重开处 `handToKeyboard()`。
- **验收**：clip-popover 164 通过；全量 `pnpm test` 75 文件 804 通过；`tsc -b` 0；变异（去掉悬停守卫）→ 仲裁测试 4 条红。
- **未验证 / 待人工**：真机长列表上键盘翻页时静止鼠标的手感；**未做独立 reviewer 审查**（自检 + 变异自证）。
- **来源**：roadmap Phase 2「悬停与键盘仲裁」，原台账无条目，本轮登记。
- **NSPanel 失焦隐藏重入回归（已修，commit `7edc63d`）**：`9719c88`（clip-popover 改用非激活 NSPanel）引入——`hide_clip_panel_now` 先 `resign_key_window()` 把键焦点交还前台，而该方法会**同步** post `NSWindowDidResignKeyNotification`，`observe_resign_key_hide` 的回调又调回隐藏逻辑（通知在主线程投递，`on_main_thread_blocking` 发现已在主线程就内联执行），形成 `hide → resign → 通知 → hide → …` 无限递归，栈溢出 → SIGABRT（真机崩溃报告 `quickquick-2026-10-31-010436.ips`，EXC_BAD_ACCESS）。v0.3.8 不含此代码，属 v0.4 面板回归。**修法**：`static PANEL_HIDING: AtomicBool` + `hide_once(guard, resign, hide)`——入口 `swap(true)` 命中即视为「外层正在隐藏」直接返回 true；真正执行的那一层用 RAII `PanelHidingGuard` 保证正常返回 / 提前 return / panic 都复位（重入层不复位，否则会清掉外层标记）。**验证**：`cargo test popover` 13 passed（重入只 resign/hide 各 1 次、守卫复位、panic 复位 3 条）；变异（去掉 `swap` 短路）→ 重入用例红（`resign` 计数 2≠1）；`cargo test` 896 passed。**未验证**：真机重跑失焦隐藏的稳定性（修复后未在新构建上实测长时使用）。

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
- **状态**：阶段 A done（独立 helper 原型已验证，**未接入 app**）；阶段 B（IPC、入库回填 title、打包）未开始，依赖 P4-5a。
- **结果**：`tools/ocr-helper/` 提供可独立复跑的 CLI：ImageIO 读图 + `VNRecognizeTextRequest`（`.accurate`，`zh-Hans/zh-Hant/en-US`，`usesLanguageCorrection=true`），stdout 固定键序单行 JSON，退出码 2/3/4 区分参数 / 读图 / 识别失败，HEIC 明确拒绝。配自造 CoreGraphics/CoreText 夹具（真值即渲染输入）、`verify.sh`（14 项契约校验，含 PNG/JPEG/TIFF 真转验证）与 `bench.sh`。commits `90c3e25`（原型）、`1dc8704`（活动卡 hash）。`src/`、`src-tauri/`、`Makefile`、`Cargo.toml`、`tauri.conf.json`、`package.json` 零改动。
- **`box` 坐标系（已定）**：保留 Vision 原生归一化、左下原点、y 向上，helper 内不翻转；落库或渲染时按 `topY = 1 - (minY + height)` 换算。
- **质量（Apple M4 Pro / macOS 27.0.1，合成白底黑字夹具，口径偏严，不代表真实截图）**：行级完全命中 47/62（纯英文 100%、纯简体 75%、混排 75%、1080p 密集 30 行 77%）；字符准确率 96.4%–100%（密集图 1254 字符错 9）。错例全为整行错 1 字，无漏行、无串行；h↔n 最高频且仅 22–28px 出现；唯一大面积错误为全角括号 `(a) [b] {c}` → `（a） ［b］icr`。
- **耗时**：1920×1080 中位 409ms（7 次，区间 397–419）；3840×2160 中位 426ms（区间 377–602）；小图约 250ms，即每次调用有约 250ms 固定成本，进程 spawn 仅约 10ms。阶段 A 据此采用进程隔离，常驻进程仅在批量导入场景出现后再评估。
- **阶段 B 必须带入的约束**：① 首次识别需 ANE 编译约 17–25s，且缓存按**可执行文件名**命名 → helper 名字必须固定，首次调用必须异步并给状态提示；② `confidence` 在本机只出现 0.30 / 0.50 / 1.00 三档，正确行也可能是 0.30，**不可做阈值过滤**；③ 语言顺序是硬性前提：`en-US` 置前会毁掉中文，中文必须排在英文前；④ `.accurate` 需可写 HOME，不可写时抛出无语义的通用错误（易误判为图坏），`.fast` 反而正常；⑤ OCR 文本的匹配语义（精确 vs 模糊）需在阶段 B 决定，否则全角括号与 h↔n 单字错误会吃掉「搜图」价值。
- **未验证**：多语言回退（本机支持 33 种，仅测三语言）；旋转 / 竖排（`cgImage` 路径不处理 EXIF 方向）；10–14px 极小字号；真实截图（深色 / 渐变 / Retina）；大图内存与超时；多 helper 并发下的 ANE 争用；PDF / 多帧 TIFF；Windows（不适用）。

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
- **状态**：部分完成（**P4-2a 增量渲染 done**；DB 分页与真虚拟滚动**待定**，理由见下）
- **结果**：commits `481019c`、`39d8031`、`f4ad0d2`。`filteredItems` 仍是全量，只把 `.map()` 改成 `.slice(0, visibleCount).map()`：首屏 100 条、滚到底部阈值（240px）按块追加 100。
- **三条明确不做（各有理由，勿"顺手"改回）**：
  1. **不做 DB 分页**：搜索/类型筛选是在内存里对全量做匹配；只给 DB 加分页会让搜索只能命中「已加载的那一段」，是正确性回归。等 P4-5 的两段式加载（轻量索引全量 + 可见行取正文）落地才可做。
  2. **不做固定行高窗口化**：`.clip-text` 两行截断导致行高在 41/54/60px 之间变化，固定行高必须强制等高 → 改视觉。窗口是「前缀 0..renderedCount」，选中项越窗时按块扩窗。
  3. **不用 `content-visibility: auto`**：它会让离屏行**退出无障碍树**，对纯键盘/读屏用户是隐蔽的可达性回归；实测撤掉后 DOM 节点数**逐位不变**（250 条时 3066 → 1266），收益 100% 来自 slice。
- **正确性关键**：键盘导航仍遍历完整 `filteredItems`（selection 是 id 态）；选中项越出窗口时按块扩窗（`ceil((idx+1)/100)*100`），且**扩窗后下一帧才滚动**（`scrolledHighlightRef` 防止追加渲染把列表拽回高亮行）。顺带补了主窗缺失的 `⌘↑`/`⌘↓`（复用既有 `firstIndex`/`lastIndex`）。
- **审查结论**：非 BLOCK，3 处变异全红；且审出并已修一个盲区——`filterKey` 若从「类型 + 搜索词」退化成按 `filteredItems.length` 判定，**全量 705 测试 0 红**（「搜索 A → 搜索 B 且命中条数相同」时不重置）。
- **残留（重要）**：窗口是前缀而非真虚拟滚动，重度滚动后 DOM 仍会回到全量（2000 条时 `⌘↓` 到末条 = 2000 个节点）。真实收益是「首屏 100 条 + 增量挂载」把一次性 2000 次节点创建摊到滚动过程中。
- **补真虚拟滚动的触发信号**：用户库常态到数千条**且真机滚动掉帧**；届时需要先解决「行高可变」的测量问题（`ResizeObserver` 或测量缓存），不能靠固定行高糊过去。
- **未验证**：真机滚动流畅度、`⌘↑`/`⌘↓` 真机键位；`PopoverList` 仍是全量渲染（分组标题 + 扁平索引是另一套模型，未做）。
- **P5-5 审查挂号约束**：若分页落地，`first_copied` 排序（P5-5）只能在已取回页内稳定重排，跨页顺序会错乱；分页时须改为 SQL 层排序或服务端全量重排。当前全量取回无 LIMIT，不是 bug。
- **挂载期清零（已修，commit `93ef41b`）**：`ClipboardPage` 驱动扩窗的 effect 是无条件 `setRenderCount(prev => nextRenderCount(prev, safeHighlight, filteredItems.length))`，而 `nextRenderCount` 内部先 `clampToTotal(current, total)`——原先 **`total = 0`（挂载期条目还没取回）时窗口被压成 0**，违背「永不小于当前窗口」的契约，导致数据到达后的第一个提交渲染 0 行（下一提交才补齐）。**修法**（Maccy 无对应问题：HistoryListView 用 LazyVStack 惰性渲染，没有窗口状态）：`clampToTotal` 在 `total <= 0` 时保留窗口（只把负值收敛到 0），`initialRenderCount(0)` 取首屏块，`growRenderCount(current, 0)` 不变也不清零。**验收**：`render-window` 单测三种 total=0 取值；ClipboardPage 集成用例用 MutationObserver 断言「数据到达后的首个渲染批次即全部行」（反证：把 `clampToTotal` 改回旧实现 → snapshots 变 `[0,3]` 断言红）。测试侧 `waitForListReady` 保留不回退。**补充（`8eb7a12`）**：反过来也有坑——窗口被滚到底扩到几百条后若列表被删空，回填时会一次性渲染几百行；现在「由非空变空」时收回首屏块并清空选中（`setHighlightIndex(0)`，否则回填后 highlightIndex 停在旧位置，窗口又为包住它扩回去）。**更正**（`015bba8`）：此前登记「该 effect 必须声明在扩窗 effect 之后」，实测（对调两者、clipboard-page 39 条用例仍全绿）**顺序并不影响结果**——扩窗那条在数据为空时原样返回收到的窗口。真正承担行为的是同一 effect 里的 `setHighlightIndex(0)`：不清选中时，回填后 highlightIndex 停在旧位置，扩窗 effect 会为包住它把窗口扩回几百行。注释与测试名已按真实结论改正。

### P4-3 · 缩略图移出 IPC payload（消除 base64 全量重传）
- **状态**：done
- **结果**：commits `c661590`（后端）、`77fc4cb`（前端）、`1469e79`（复审加固）。`ClipItemDto.thumbnail_data_url` → `has_thumbnail: bool`；新增只读函数 `db::list_items_with_image_flag`，缩略图 BLOB **连出库都不做**（投影 `CASE WHEN thumbnail IS NOT NULL AND length(thumbnail) > 0 THEN 1 ELSE 0`）；新增 `get_clip_thumbnail(image_id)` 复用 `image::get_image_thumbnail`（不另写 SQL），不存在/软删/零字节 BLOB 一律 `Ok(None)`；前端模块级 `settled` 缓存 + 在途去重，`invoke` 不进组件（可测「只拉一次」）。
- **为什么保留 `list_items_with_images`**：`src-tauri/tests/capture_contract.rs` 有 3 处断言 `row.thumbnail` 非空，改行结构会让刚落地的 P3-0 契约护栏编译不过。故走「加只读函数 + 两个变体共用 SQL 骨架」；计划测试用 `assert_eq!(flag_plan, blob_plan)` 防「改一个忘另一个」。
- **payload 口径（可复现，勿引用单一数字）**：改后 10 条含图条目 JSON = **2,301 B（230 B/条），与缩略图体积完全解耦**（三档输入——高熵噪点/渐变/纯色——均为 2,301 B）；改前 = `2,301 + 1.33 × 缩略图字节总和`，所以「改前」不是一个固定值：噪点图档 523,955 B（228×）、渐变档 20,423 B（8.9×）、纯色档 5,275 B（2.3×）。**对外只说「改后固定 230 B/条」**，不要说「228×」。
- **失败策略（刻意）**：取图失败也缓存为 null、不自动重试——取不到通常是该行已被清理或 BLOB 为空，重试只会把滚动放大成请求风暴；代价是瞬时故障要重开窗口才自愈，已写进注释。
- **审查打回的两处（已闭环）**：① 计划断言只在「计划串」层比较，**去掉 JOIN 的 `cimg.is_deleted = 0` 不会红**（两变体共用宏，计划逐字相同但语义漏了软删图）；已加 SQL 字面锚（`cimg.is_deleted = 0` / `length(...) > 0` / `ci.is_deleted = 0`），删任一条件必红。② 缓存读取有两处互相兜底（改一处仍绿）；已收敛到单一入口 `trySettledClipThumbnail` + hook 去掉 state 副本（渲染期派生读 + `forceRender`），两个调用点各有独立断言。
- **未验证**：真机列表滚动时的实际 IPC 频率与首屏延迟、失败永久缓存在 SQLCipher 大库上的可接受性、占位 `min-height` 与真实缩略图的视觉衔接。
### P4-4 · 自动淘汰 + 可配上限（1–999，pin 豁免）
- **状态**：done
- **做什么**：`cleanup_keep_recent` 此前只有「立即清理」按钮能用、保留条数硬编码 500 且不可配。现在新增设置 `max_history_items`（区间 **1–999**），自动淘汰挂在 `capture_and_ingest` 的写库事务 **RELEASE 之后**，判据是「本轮有新增行」（**任意类型**的 `Inserted`）——去重命中只刷新时间戳、空轮次根本不进这条路径，故常态轮次零额外 SQL。
- **默认值取舍（与 Maccy 的 200 不同，明确记录）**：默认 **500**。依据：本项之前「手动清理」就是硬编码 500 条，沿用默认值让存量用户升级后的历史规模不变；设置页此前**没有**对「条目数上限」做过具体数字承诺——页面上那个 `500 MB` 是**图片占用**上限（P1-9 已把图片占用与库文件体积 / 条数拆成不同口径），与条数上限不是同一个量。想对齐 Maccy 只改 `settings::DEFAULT_MAX_HISTORY_ITEMS` 一个常量。
- **「pin 豁免」的落点**：本项目目前没有独立的 `pin` 列（字母 pin 是 P2-4，仍 blocked），等价能力是 `is_favorite`。淘汰判据 `WHERE is_deleted = 0 AND is_favorite = 0` 让收藏项**既不计数也不被删除**（对齐 Maccy 的「pin 无限豁免」）。**P2-4 落地后必须重审这条判据**：若「字母 pin」要与「收藏」区分，需要新增独立列并让它进同一条豁免条件，不能把 `pin` 塞进 `is_favorite`（那会改变收藏既有语义）。
- **三处改动与理由**：
  1. `db::evict_over_limit`：`cleanup_keep_recent` 的**带短路**入口——先 `COUNT(*)`（走 `idx_clip_items_live_favorite_modified` 的等值前缀，不扫表、不临时排序），只有超限才去取候选 id 列表。入库路径每次新增行都调用，未超限是绝对常态，直接复用 `cleanup_keep_recent` 会白把全部非收藏 id 取回内存再一行不删。
  2. `pipeline::IngestLimits`：把 `max_image_bytes` / `max_history_items` / `image_cleanup` 收进一个结构体。原来 `capture_and_ingest` 已是 5 个位置参数，再加一个数字会变成 6 个、且两个都是不同单位的数字（字节 / 条数），极容易传反。测试专用的 `capture_and_ingest_with_cleanup` 因此**整个删掉**——阈值都能注入后，它和生产函数没有差别，留着等于让测试跑在一条生产走不到的路径上。
  3. `set_max_history_items` 命令：校验 1–999 → 落盘 → **立刻**按新上限淘汰并返回本轮淘汰条数。只等下一次复制会让「上限已经改小、列表还有 400 条」同时成立。
- **手动清理改成同一口径**：`cleanup_history_impl(conn, keep_count)` 不再读已删除的 `KEEP_RECENT_COUNT`，上限由命令层从设置读取。手动与自动各持一个数字，迟早漂成两套口径——这本来就是「硬编码 500」的形态。
- **不产生迁移步骤（P4-7 约束下有意为之）**：本项**没有**新增 DB 列 / 索引 / 表——上限是纯配置，落在 `settings.json`（与 `max_image_bytes` 同源），淘汰复用既有的 `is_deleted` / `is_favorite` / `last_modified_utc` 与 v1 已有索引。因此 `CURRENT_SCHEMA_VERSION` 不动，`MIGRATIONS` 不追加，也没有任何「探测列是否存在」的 DDL。
- **非法值三道防线**：前端输入拦截（`^\d+$` + 1..999，**不做「悄悄取整」**：`2.5` 直接拒收而不是变成 2）→ 命令层拒收且不落盘 → 读侧 `AppSettings::effective_max_history_items()` 把手工改坏的 0 / 10000 收敛回边界（0 收敛到 1，而不是「每次复制清空历史」）。
- **验收**：`cargo test` **459 passed**（lib）；全部集成套件合计 **745 passed**，0 failed（`cargo test --tests`，取数于 60846c1 隔离 worktree）；`cargo clippy --all-targets -- -D warnings` 干净；`cargo fmt --all --check` 干净；`pnpm exec tsc -b` 干净；`pnpm test` **766 passed / 74 files**（60846c1；7df0e60 时为 764 passed）。（自检阶段曾误写为 480 / 768，经独立复审更正。）。
- **可失败性（6 处变异全部打红）**：① 摘掉入库路径的淘汰触发（`if false &&`）→ 3 红；② 候选 SQL 去掉 `is_favorite = 0` → 3 红（含列表索引计划测试）；③ `MAX_HISTORY_ITEMS` 改 1000 → 3 红；④ `set_max_history_items_impl` 跳过立即淘汰 → 1 红；⑤ `counts_as_new_row` 退化成「非空即真」→ 2 红；⑥ 前端非法值不回滚输入框 → 4 红。
- **未验证**：真机（打包 GUI）改上限的即时手感与 `clipboard-changed` → 列表刷新联动（headless 只验到事件「被发送」的调用点存在，没有真机订阅方实跑）；加密大库（数万条）首次收敛的耗时（`cleanup_keep_recent` 是逐行 UPDATE，超出部分很多时是一批写事务）；Windows。
### P4-5 · 搜索升级（预计算 title + 4 模式 + 多词 AND + 命中高亮）
- **状态**：部分完成（P4-5a 后端 title 预计算落库与分批回填已完成；**尾部搜索（>1000 字符不可搜）未解决**，依赖两段式加载设计；设计文档 `docs/design/search-two-stage-design.md` 尚未落盘，`search-architect` 中断前无产出）
### P4-6 · 翻译历史分页
- **状态**：done
- **结果**：commits `95d3045`、`fb5d366`（复审补强）。`TRANSLATE_HISTORY_PAGE_LIMIT = 200`（历史栏一屏上限，钉住 payload）+ `TRANSLATE_HISTORY_RETENTION = 500`（2.5× 分页上限，给表体量上界）；查询加 `LIMIT ?1` 且保留 `created_utc DESC, rowid DESC`；写入后 best-effort 清理，**先 `COUNT(*)` 短路**，只有超限才跑一次 `DELETE ... OFFSET 500`。
- **收敛点**：只在写入路径（唯一写入口），读路径保持纯读——`list_translate_history` 会被挂载与事件反复调用，读里顺手 DELETE 会把纯读变隐式写、还可能撞写锁。代价：**旧库只在用户下一次翻译时收敛**。
- **清理失败不影响主链路**：吞错 + `eprintln`。测试用 `BEFORE DELETE` 触发器「先插审计行再 `RAISE(FAIL)`」——比只读连接好在「写入成功、清理失败」这个真形态能构造出来，且审计行证明清理**真的被尝试过**，避免恒真假绿。
- **审查打回的「测试拦不住退化」（已闭环）**：把 `total <= RETENTION` 改成 `total < 0`（每次写入都跑 DELETE）原本 **0 红**。触发器方案在这里**必然假绿**——未超限时那条 DELETE 即使执行也是删 0 行，而行级触发器 0 行匹配就 0 次触发。改用 `sqlite3_set_authorizer` 在 **prepare 阶段**统计 `SQLITE_DELETE`，删 0 行也会计数；并配**正向对照**（超限写入后计数必须变 1），否则「0 次」可能只是探针没接上。
- **未验证**：加密库上 `COUNT(*)` 与 `DELETE ... OFFSET` 的实际耗时、旧库万级首次翻译的一次性收敛耗时（与主写同锁的尾延迟）、Windows。
### P4-7 · 迁移框架版本化（`PRAGMA user_version` 承担 schema 版本）
- **状态**：done
- **结果**：commits `e085606`（框架）、`63fd95a`（错误文案）、`5102013`（原因透出到前端）。`CURRENT_SCHEMA_VERSION = 1` + `struct Migration { to_version, apply: fn(&Transaction) -> Result<(), DbError> }` + `const MIGRATIONS`；所有步骤 + 版本写入在**单个事务**内，任一步失败连第一步的 DDL 一起回滚。
- **两条不可退让的设计**：
  1. **`create_tables` / `ensure_indexes` 冻结为 v1 基线快照**，不再每次开库跑。理由：版本号必须是「库是什么形态」的唯一真相；若把 `IF NOT EXISTS` DDL 留在开库路径上，未来往那里加列就会**绕过版本号**在用户开库时生效，同一个 `user_version` 下长出两种表形态，后续迁移的前提立刻失效。代价（刻意接受）：**外力删掉表/索引后不再自动补回**。
  2. **迁移错误不复用 `DbError::Sqlite`**：`open_or_recover` 会把 `Sqlite` 当「库已损坏」去改名备份，那样一次版本超前就会把完好的库搬走。新增 `SchemaTooNew` / `MigrationFailed` 两个变体，`classify_failure` 归 Permanent。
- **给后来人的硬约束**（已写进 `db.rs` 模块头）：新增列/索引/表 → 追加 `Migration { to_version: N+1 }` + bump 常量，步骤内写**无条件** DDL；禁止再写「探测列是否存在」。
- **P4-5 的前置约束**：给预计算 `title` 列做回填时，**不要把全表 `UPDATE` 塞进迁移步骤**（会在启动期同步路径上把写锁拖到秒级，同时 5 条 `CREATE INDEX` 也要全表扫）；正确做法是迁移只加列，回填走开库后的**分批幂等 backfill**（`WHERE title IS NULL` 分批）。
- **文案与出口**：`DbError::Display` 是唯一真相来源（不在 lib.rs 再写一份子串分类）；`AppDb` 携带 `open_error` 把原因透出到前端文案——因为**打包后的 GUI 应用看不到 `eprintln` 的 stderr**，「诚实只存在于用户看不到的地方」等于没做。
- **待用户裁决**：`SchemaTooNew`/`MigrationFailed` 时是否弹系统对话框并退出（当前是「记日志 + 以无库状态运行 + 前端显示准确原因」）。
- **登记的风险（P6-3 口径）**：`db::open_or_recover` 生产 0 调用点（只有 4 处测试调用），且与生产 `reset_and_reopen` **判据分裂**（一个认 `DbError::Sqlite`、一个认 `is_resettable_open_error` 的字符串标记）——未来若被接进启动路径会**静默改变恢复策略**。它是 `pub fn` + `pub mod`，`dead_code` 与 cargo-udeps 都发现不了。
- **审查结论（复审，前一个 reviewer 跑 2.5 小时后失败、无产出，故重派一次限时长复审）**：**非 BLOCK**，6 个必核点全部成立，2 处变异均有效（把版本超前检查后移 → 红 2；让步骤用原始 `COMMIT` 逃出事务 → 红 1，证明回滚断言不是结构性空转）。它纠正了两处事实：① 改后 `PRAGMA foreign_keys = ON` 共 3 处（`open_with_key`、`ensure_schema` 事务外、测试 helper），全仓唯一开库入口是 `open_or_create`，故所有建库路径都覆盖到；② `AppDb` 测试构造点实际是 **5 处**（`ipc_clipboard.rs` 2 + `ipc/mod.rs` 4），不是执行者说的 2 处——但没有遗漏导致失败的构造点，`cargo check --all-targets` 与 `cargo test` 均绿。
- **未验证**：真机旧库升级、大库首次建 5 条索引的启动期墙钟、前端是否原样展示这些文案（未实跑界面）、Windows。

---

## Phase 5 · 产品化

### P5-1 · 托盘菜单补齐（暂停/清空/设置/检查更新/版本）
- **状态**：done（图标显示最近内容为可选项，**未做**）
- **结果**：commits `d057ae2`（菜单补齐：暂停 / 打开设置 / 检查更新 / 版本号）、`e92c87d`（「清空未固定…」二次确认后清空非收藏历史）、`6a8bd62`（审计活动卡）、`dadfe2f`（编排顺序锚定）、`c101813`（活动卡数字订正）。
- **关键约束**：清空只删 `is_favorite=0`；前端确认文案明示「收藏项会保留」；「检查更新」复用 `ipc/update.rs`；版本号取自配置而非硬编码。
- **验收**：`cargo test --lib tray::` 32 通过；编排顺序变异（交换 show/emit、删 show）各使 2 条测试变红；清空谓词有 `is_favorite` 锚定测试（变异下全量 5 条红）。
- **待人工**：托盘菜单观感与分组；打包 WKWebView 中 `window.confirm` 模态表现；清空后列表经 `clipboard-changed` 刷新的真机联调。
- **未验证**：Windows（`current_modifiers` 的 `GetKeyState` 分支未编译）。
### P5-2 · 系统通知
### P5-3 · 音效（捕获/取用，与通知开关解耦）
### P5-4 · 首次引导向导（多步 + 授权后自动恢复）
- **状态**：done（commits `ca26682`、`a51a48f`）
- **结果**：四步向导；新只读 IPC `get_onboarding_status`（body 在 `onboarding.rs`，`lib.rs` 仅 1 行注册）；「授权后收起」**只收起辅助功能那一步**，不跳过整个向导，以免跳过自启确认。
- **待裁决（已标注）**：路线图字面写的是「授权后自动恢复」，当前实现是「收起授权步骤」。若需字面一致，只需把收起目标从 `AutostartConfirm→Done` 两侧同改一行；lead 尚未裁定。
- **验收**：22 条前端测试；Rust 侧 `tests/onboarding.rs`；全量 `cargo test` 与 vitest 通过（以 `7f72e43` 为准，见下方验证记录）。
- **待人工**：系统辅助功能授权弹窗的实际行为、授权后即时恢复手感、打包 GUI 呈现。
### P5-5 · 设置项扩充（排序、图片高度、预览延迟、命中高亮、容量上限、弹出位置）
- **状态**：done（首轮 `07fd8f6`；**收尾按 Maccy 2.6.1 源码对齐** `cd1af1f`（高亮/范围）、`e36db71`（分组口径）、`8bb7ed1`（全部 tooltip））。**容量上限归 P4-4**（已完成，未重复做）；**弹出位置归 P2-9（blocked）**；设置页搜索归 P5-7、i18n 归 P5-6。
- **Maccy 对照依据（源码 `/tmp/maccy-src`，tag 2.6.1，commit `dec6601`）**：
  - 排序：`Maccy/Sorter.swift` 只有 `lastCopiedAt`（默认）/ `firstCopiedAt` / `numberOfCopies`，且是**全局排序 + pin 单独置顶**，没有日期分组。
  - 高亮：`Maccy/HighlightMatch.swift` 就是 `color / bold / italic / underline`；`Defaults.Keys+Names.swift` 里 `highlightMatch` 默认 `.bold`；渲染在 `Observables/HistoryItemDecorator.swift:148-173`（bold→加粗、italic→斜体、underline→下划线、color→`findHighlightColor` 整段染色）。
  - 图片高度：`Defaults.Keys+Names.swift` 的 `imageMaxHeight` 默认 40；`Settings/AppearanceSettingsPane.swift:98` 的 Stepper 范围 `1...200`（输入框 formatter 另设 30–200 软约束，本项以 Stepper 为准）。
  - 预览延迟：`previewDelay` 默认 1500，Stepper `200...100_000`（`AppearanceSettingsPane.swift:108`）；节流逻辑 `HistoryItemDecorator.swift:26-28` 是「首次 previewDelay、之后固定 0.2s」，与我们一致。
  - 设置存储：Maccy 用 UserDefaults **逐键**存储——一个坏键只影响它自己 → 已落地为 P5-8。
  - tooltip：Maccy 每个设置项都有 `.help(...)`（全仓 17 处，如 `AppearanceSettingsPane.swift:97` 的 `ImageHeightTooltip`）。
- **新增设置页子项「外观」**（6 项 → 7 项，保留；对齐 Maccy 的 Appearance 面板）：四个控件 + 默认值 tooltip。导航 / `sections.ts` 及其测试同步改成 7 项。
- **排序方式**：`recent`（默认）/ `first_copied` 两档。**`numberOfCopies` 本轮不做，已登记为「依赖 P3-8 的 `copy_count` 列（迁移 v4）」阻塞**。默认档**逐条等同改动前顺序**（直接沿用库里 `is_favorite DESC, last_modified_utc DESC, rowid DESC`，不做任何额外排序）；`first_copied` 按 `(is_favorite DESC, created_utc DESC)` 做**稳定重排**——同 key 沿用库顺序兜底，收藏优先级仍是第一键（Maccy 的 pin 同样置顶）。**实现不在 SQL 里换 ORDER BY 列**：列表索引是 `(is_deleted, is_favorite, last_modified_utc)`，改按 `created_utc` 会退化成全表扫 + 临时 B-tree，而新建索引要动迁移/schema（本项明令禁止）。**不改去重、收藏、淘汰语义**；`pick_latest_translate_text_impl` 固定走 `recent`，翻译取数不随展示偏好漂移（有专门用例）。
- **分组口径（收尾决定）**：Maccy **没有分组**，分组是本项目自己的设计。按「排序口径与分组口径必须一致」处理：`first_copied` 时弹窗按 **`created_utc`** 划分「今天 / 更早」，跨组全局顺序成立；`recent` 维持 `last_modified_utc`（默认行为不变）。列表 DTO 因此增加 `createdUtc`（`camelCase`；Rust 侧有「首次入库时间 ≠ 最近复制时间」的断言），列表 payload 基线 **+54,000 B**（2000 行 × 27 B；混合形态 2,105,877 → **2,159,877 B = +2.6%**，全多行 923,648 → **977,648 B = +5.8%**），`tests/search_title.rs` 的基线数字与说明已同步修订（该测试仍守「不得再加 title」）。
- **图片高度**：`image_height_px` **1–200（默认 40）**，对齐 Maccy 的 `imageMaxHeight`。**⚠️ 默认视觉变化（有意）**：由首轮的 24px 改为 40px，弹窗行内图片会变高——**待真机确认观感**。仍**只作用于弹出面板行内缩略图**（主窗口列表的 44×32 不受影响，故标签写作「弹出面板图片高度」）；经 `.popover-shell` 上的 CSS 自定义属性 `--popover-thumb-height` 下发，宽度固定、`object-fit: cover` 裁切。
- **首次预览延迟**：`preview_first_delay_ms` **200–100_000（默认 1500 不变）**，上限照抄 Maccy；首轮曾收紧到 5000，**收尾按 Maccy 放宽**（「等太久等于没预览」只是观感偏好，不该替用户否决）。只作用于「面板本次打开后的首个预览」；**后续切换固定 200ms，不开放配置**（与 Maccy 的 `minimumDelay = 0.2` 一致）。弹窗在每次开面板与收到列表刷新事件时重读设置。
- **命中高亮样式**：**四档，与 Maccy 同名同默认**：`bold`（默认）/ `color` / `italic` / `underline`。**⚠️ 默认视觉变化（有意）**：由首轮的「背景 + 加粗」改为 Maccy 的「纯加粗（无背景）」——**待真机确认**；自造的 `default` 档已删除，`color` 档沿用我们原来的背景高亮视觉（Maccy 用 `findHighlightColor`，我们用 OKLch accent token）。实现是根容器上的 `data-hit-style` 属性 + CSS 四分支，命中区间与 `<mark class="clip-hit">` 结构不变，主窗与弹窗共用同一设置。
- **tooltip（收尾补齐）**：按 Maccy 的 `.help` 约定给**全部**设置项加 tooltip 并写明默认值，默认值逐个从代码读出（`autostart.rs` 默认开 / `hotkey.rs` 三键 ⌘⇧C·⌘⇧T·⌘⇧M / `settings.rs` 各 serde 默认 / `image.rs::DEFAULT_MAX_TOTAL` 500 MiB / `providers.rs` 的 `google_free`）：通用 3 项 + 外观 4 项 + 热键 3 项 + 翻译源 1 组 + 隐私 5 项 + 存储 6 项。**一次性动作行**（立即检查更新 / 立即清理 / 备份恢复）tooltip 注明「一次性操作，无默认值」；纯状态展示行（图片占用 / 库文件体积 / 条目数）不是设置项，未加。
- **读写与校验**：新增 5 个 IPC（`get_appearance_settings` 一次读四项 + 4 个 setter）。写路径对非法枚举与越界数值**拒收且不落盘**；读路径再收敛一次（`effective_*`），兜手工改坏的 settings.json；前端另有 `normalizeAppearanceSettings` 在 IPC 边界归一。**落盘后何时生效**：排序与命中高亮落盘后发既有的 `clipboard-changed` —— 该事件在本工程里是「列表需要重新取数」的信号（先例：`set_max_history_items` 淘汰后），**刻意不是「剪贴板内容变了」的字面语义**；图片高度与预览延迟不发，避免为纯 CSS/时序变更白传一次列表 payload。
- **验收（实跑，收尾在 `8bb7ed1` 上）**：
  - `rtk pnpm exec tsc -b` → 退出 0。
  - `rtk pnpm test` → **80 文件 / 868 测试全绿**（首轮末为 79/853）。
  - `cd src-tauri && cargo fmt --all --check` → 退出 0；`cargo clippy --all-targets -- -D warnings` → 干净；`cargo test` → **849 passed / 0 failed**（首轮末为 838）。
  - **变异自证（收尾两轮，均在最终树上做并还原）**：① `settings.rs::DEFAULT_IMAGE_HEIGHT_PX` 40→24 → 红 2（默认值专项 + 外观读口默认值）；② 前端 `DEFAULT_HIGHLIGHT_STYLE` `"bold"`→`"color"` → 红 4（默认值断言、下拉首项、弹窗回退默认档、主窗回退默认档）。还原后子集复跑全绿、工作区无残留。
- **已知边界（已决定，不再是待裁决）**：
  1. 「首次复制」下分组口径已改为按 `created_utc`，**跨组全局顺序成立**；但 Maccy 本身没有分组，这套「收藏 / 今天 / 更早」仍是本项目自有设计——**真机观察项**：老条目被重新复制后不再跳组，是否符合直觉。
  2. tooltip 已覆盖全部设置项；**默认值随代码变化时 tooltip 不会自动跟着变**（硬编码中文文案），改默认值需同步改文案——由 `settings-tooltips.test.tsx` 的逐面板断言兜住明显缺漏。
  3. `numberOfCopies` 排序依赖 P3-8（迁移 v4），未做。
  4. 枚举写坏的爆炸半径已由 P5-8 修掉（类型错误逐字段容错；**语法错误仍整份回退且写路径拒写**，见 P5-8 已知边界）。
  5. **旧高亮值 `"default"` 不做 serde alias 迁移**：v0.4 尚未发布，不存在需要兼容的存量文件；读路径遇到 `"default"` 会按「未知枚举值」回退成 `Bold`（默认档），写路径只会在用户实际改动该项时用新值覆盖。
- **未验证 / 待人工**：**两处默认视觉变化需真机确认**（高亮默认从「背景+加粗」变「纯加粗」；弹窗缩略图默认 24px → 40px）；各档预览延迟的实际体感；`color` 档用 accent token 替代 `findHighlightColor` 的观感差异；长列表切排序后的滚动位置；打包 GUI 下的实际表现；Windows（本项无平台相关代码，但整条链路未在 Windows 编译验证）。
- **独立审查（lead 另派 `settings-expansion-review`，对象 `07fd8f6`）**：结论 **非 BLOCK**。默认值逐项对照 `07fd8f6^` 一致；契约（57 个命令注册、5 个新命令前后端参数名一致、枚举 snake_case、DTO camelCase）核对通过；`tsc`、设置子集 288 测试、`cargo check --all-targets` 退出 0。审查提出的既有风险 **已登记为 P5-8** 并已在收尾中修复（见下）。已据审查修正 `ipc/clipboard.rs` 注释：行数 = 历史上限 + 收藏数，并无硬上界。**收尾改动（`cd1af1f` / `e36db71` / `8bb7ed1`）尚未经独立审查。**
### P5-8 · settings.json 读失败后写回覆盖（既有缺陷，P5-5 审查挂号）
- **状态**：done（`03a84d0` 主体；`6c2e7a7` 按审查裁决改为「解析失败拒写 + 时间戳备份 + 启动留痕」，见下）。**对齐 Maccy 的逐键存储语义**：Maccy 用 UserDefaults 逐键存储，一个坏键只影响它自己，不会被整体覆盖。
- **问题（保留作背景）**：`settings.rs` 的 `load_or_default` 原先对任一解析失败静默回退 `Default`；`save` 是整文件 `fs::write`。于是任一非法字段或 JSON 语法错 → 读回默认值 → 下一次任意 setter 把**整份默认值**写回，覆盖排除应用、翻译源、历史上限等无关设置（约 20 处 setter 同构）。
- **结果**：
  1. **读侧字段级容错**：`AppSettings::load_lenient` 把 JSON 解析成对象后**逐字段**反序列化，坏字段只回退它自己的默认值，并返回「哪个字段、为什么」的清单；`from_json_object_lenient` 是纯函数主体。`load_or_default` 变成它的**静默**包装（入库轮询每约 500ms 读一次盘，在热路径打日志会刷屏）。缺文件仍视为正常首启动（零问题，不算损坏）。
  2. **写侧逐键更新**：`write_setting_field` / `update_settings_value` =「读原始 JSON 对象 → 只改目标键 → 写回」，**其他字段与未知键原样保留**。16 个 setter 全部改走它；`AppSettings::save` 保留但文档标明只给测试 / 整份重写用。
  3. **语法损坏 = 备份留痕 + 拒绝写入**（`SettingsError::Unparsable`）：文件存在但 JSON 语法坏 / 根不是对象时，先把原文件 `fs::copy` 到 `settings.json.<seq>.<ms>.bak`（`seq` 为单调写入序号，**不覆盖既有备份**），然后**返回中文 Err 拒写**——原文件逐字节不变、不丢任何键。错误文案：「settings.json 无法解析（原因），已拒绝写入以免覆盖你的设置；请先修复该文件，本次未改动原文件。备份副本：<路径>」，经命令层 `to_string()` 原样传给前端。**依据**：业界通行做法（无法解析的用户配置不覆盖、给明确错误）；Maccy 用 UserDefaults 逐键存储、无整文件概念，故无对应逻辑。备份失败（目录只读等）同样拒写且原文件不动。
     **空文件例外（读写两侧一致）**：0 字节 / 只有空白按「还没有设置」处理（里面没有可丢的用户数据）——写路径允许首次写入、不备份、不报错；读路径同样返回默认值且**不计入整份回退**、不产生问题清单、**不消耗「已提示过损坏」的标记**（否则 `touch settings.json` 就会被当成语法错误并吃掉那唯一一次提示）。
  4. **写后体检留痕**：写完对已知字段跑一次宽松解析，仍有坏字段就 `eprintln` 点名，不改变文件内容（坏键留在文件里让用户修）。
  5. **读盘失败也留痕（`af8563f`）**：`load_lenient` 的 I/O 失败（非「文件不存在」）此前只把原因写进返回值、没有任何日志；现在补一次 eprintln「settings.json 读取失败（…），本次使用默认设置」，并复用损坏提示的同一个去重标记（抽出 `warn_once` 公共实现）。**回退行为未变**：仍返回默认值、仍把问题记入清单。
  6. **读路径整份回退只提示一次，且解析成功即复位**：语法坏 / 根不是对象时读路径仍回退整份默认值（行为不变），但用进程级 `AtomicBool` 只打一次 `eprintln`（读盘每约 500ms 一轮，不能刷屏）；**一旦读到可解析的 JSON 对象就把标记复位**，使「损坏 → 提示 → 修复 → 再次损坏」还能再提示一次（否则同一进程里修过一次之后永久静默）；字段级坏值不触发该提示、但同样复位（文件已经能打开）。`apply_corrupt_warning` 的 flag 由调用方注入，测试可验证完整时序而不受其它用例干扰。Maccy 无对应；业界做法是启动期提示一次。
  7. **并发前提已写进代码文档**：`update_settings_value` 注释写明「当前设置类 IPC 全是同步命令，读改写无锁；若有命令改成 async 必须加互斥」。
  8. **错误文案透到界面**（`7bbad50`）：外观面板四处保存失败、通用面板三个开关保存失败都改为显示**后端中文原文**（含「已拒绝写入……请先修复该文件，备份副本：settings.json.<ms>.bak」），不再只落 `console`；通用面板此前连提示都没有（表现为「点了没反应」）。**本批补齐**（`93bb299`）：通道改为**按来源隔离**（`reportSaveError(source, msg)` / `clearSaveError(source)` / `runSave(source, action)`），六个面板全部带来源；失败显示后端原文、成功只清同源。其余补齐：各开关错误互不覆盖（C）、无 Provider 时兜底改 `console.error` 不静默（G）、存储统计**读**失败改用面板内 `statsLoadError`、不冒充保存失败（J）。
     再修（`9085ad1` / `9325f09`）：**通用面板删掉 `reportedSources` ref 与中间 effect**，三个开关直接 `runSave("general.<key>", …)`，`useGeneralSettings` 的 setter 改为失败即抛（不再自存错误状态）——错误清除发生在**成功路径**上，不再依赖组件挂载/卸载时机（卸载重挂会残留旧错误的根因）。**隐私名单增删只清各自来源**（`privacy.addApp` / `privacy.removeApp` / `privacy.addRegex` / `privacy.removeRegex` 各自清理），避免「添加失败 + 删除成功」把添加的原因抹掉。热键损坏提示不再重复显示备份路径（`648f9ff`，hint 里已有）。
     数量订正（A）：`9085ad1` 提交信息里写的「187 → 188」不准确，实测 `17fa785` 与当前 HEAD 的 settings 子集**均为 189**（该提交删了 2 条 hook 用例、新增 1 条面板用例，净 -1）。
     覆盖归属（B）：`9085ad1` 删掉的 hook 级用例「另一开关失败不被本开关成功清掉」，其覆盖已由 `settings-save-error.test.tsx` 的面板级用例承担（通用 A 失败 / B 成功场景），非覆盖缺口。
  9. **热键读取容错 + 修复指引**（`ae0727b`）：`HotkeyConfig::load_lenient` 逐键读到可用值；整份损坏时仍返回默认键位，并在 `corrupt { file, backupPath, hint }` 里给指引（读路径先做一次保护性备份，保证提示里的路径真实存在）。`get_hotkeys_impl` 不再返回 Err，面板在键位行上方展示「已备份到 X，请修复或删除该文件后重启」，**去掉「稍后重试」误导文案**，键位行照常渲染；`lib.rs` 启动期注册改用同一函数（K）。Maccy 无对应（UserDefaults 逐键、无整文件损坏形态）。
  10. **同键早退先校验可写**（`7fe43b4`）：同键早退原本直接 `Ok(())` 跳过落盘校验，损坏文件下用户会以为改键成功；现在早退前 `ensure_writable_json_object`，损坏即走「备份 + 拒写」。回滚注销新键失败、改键后注销旧键失败都补 `eprintln` 留痕（返回值不变）。
  11. **备份次序与墙钟解耦**（`d2f68f1`，取代 `7b22f51` 的做法）：文件名改为 `settings.json.<seq>.<ms>.bak`——`seq` 是单调写入序号（**取磁盘上已有备份的最大 seq + 1**，跨重启有效、墙钟回拨也不会倒退），`ms` **永远是真实墙钟**（不做任何 max 修正）。排序只按 `seq`；旧格式 `<ms>.bak`（含中间实验格式 `<ms>-<n>.bak`）没有 seq，一律排在所有新格式之前、彼此按 `ms` 排。`prune_backups` 按 seq 保留最近 5 份并跳过刚创建的那份。
      为什么推翻 `max(now, newest+1)`：**一次未来时间戳会永久传染**——新备份的时间戳再也不是真实时间，用户按时间找「今天改坏的那份」找不到。
      **为什么不用 mtime 排序**（更正此前写错的依据）：macOS 上 `fs::copy` **保留源文件 mtime**（实测：源设 2001 年，目标也是 2001 年），且 mtime 可被外部工具（`touch`、`cp -p`、归档还原）任意设定，不代表本进程的写入次序（Linux 行为未验证）。
  12. **备份 prune 跳过刚创建的那份**（`a4543f9`）：时钟回拨时新备份时间戳可能小于旧备份，纯按时间排序会把它当最旧的删掉；`prune_backups(path, keep)` 显式排除 `keep`，时间源可注入（`backup_broken_settings_at`）便于测试模拟回拨。
- **已知边界（两条失效路径必须区分清楚）**：
  - **JSON 语法错误 / 根不是对象 = 整份回退默认 + 启动期提示一次**：读路径拿不到任何字段（连哪个键都不知道），只能整份按默认运行；此时**写路径已经拒写**，所以默认值不会被写回文件——用户修好文件再重启即可。`03a84d0` 的提交描述写的是「坏字段不再连累整份设置」，那句话只覆盖**类型错误**，语法错误仍整份回退，这里更正以免误读。
  - **字段类型错误 = 字段级回退**：只有那个字段取默认值（并在写入路径体检时 `eprintln` 点名），其余字段原样生效；坏键的值**不会被自动改写**（自动"修"成默认值等于悄悄改用户设置）。
  - **备份滚动保留最近 5 份 + 同内容去重**（`3f00c05`）：Maccy 无备份概念，按业界做法只留最近 `BACKUP_KEEP_COUNT = 5` 份（新建后删最旧，删除失败只 eprintln）；与最近一份备份**字节相同则复用**、不新建。三个配置文件（settings / hotkey / autostart）共用同一套逻辑。
    命名细节：`<seq>.<ms>.bak` —— `seq` 是写入序号（**同一毫秒由 seq 区分，不存在重名后缀**），`ms` 是真实墙钟只作展示；`seq` 取磁盘最大值 +1，因此跨重启递增、墙钟回拨也不倒退。
  - **`backupPath` 是无 UI 消费者的死字段**（E）：`HotkeyCorruptInfo.backupPath` 目前只有测试 fixture 使用，前端渲染只用了 `hint`（路径已内含）。删除它属于 IPC 契约变更（前后端 DTO 同步），另立项处理；现阶段视为**可接受的死数据**。
  - **ClipboardPage 自启开关（已修，`35fabad` / `5846d5e`）**：原实现「先改界面、失败只弹通用提示」属于界面撒谎。先补了回滚（`35fabad`），随后改为**悲观更新**（`5846d5e`）：先 `await setLaunchOnLogin(enabled)`，成功才改界面，失败保持原值并显示后端原文——与设置页通用面板机制相同（**两边都是悲观更新**，不是「都是回滚」）。失败文案用独立状态 `launchOnLoginError`，**发起切换时即清**（不是成功后清，`94701c5`），与面板级 `opError` 分开存放；两者**并存渲染**（`f90ce12`，各占一行、带 `data-error-source`），不再用 `??` 互相遮挡。测试：失败保持原值 + 显示原文、后端未返回前界面不动、失败后再成功无残留文案、两来源并存可见。
     **系统登录项同步（`b8c8a57`）**：`set_launch_on_login` 原先「写 autostart.json → apply_to_os 失败只 eprintln、命令仍 Ok」，界面显示已开启而系统未注册。现在 `apply_to_os` 返回 `Result`，新增 `autostart::set_enabled_with_apply`（apply 可注入）：应用失败即回滚配置（文件原本不存在则删除）并返回「系统登录项设置失败：…，已恢复为原设置」，前端走悲观更新失败路径（界面不变 + 显示原文）。
  - **备份序号的并发前提（G，源码阅读结论）**：`register_hotkeys` 跑在 `lib.rs` 的 `.setup()` 里（同步闭包）；设置类 IPC 命令（`get_hotkeys` / `set_hotkey` / 其余 35 个）全部是**同步** `fn`，没有 `async fn`、也没有 `#[tauri::command(async)]`。**源码阅读确认**：Tauri 2.11.2 的宏默认 `ExecutionContext::Blocking`（`tauri-macros-2.6.2/src/command/wrapper.rs`），走 `body_blocking`——在 scheme 回调里**内联执行、不 spawn**。**运行时线程归属（是否主线程）待验证**：未在真机用线程 id 断言，Windows/Linux 亦未验证。
    **settings.json 的四类写入者（已 grep 全仓核实）**：
    1. **设置类 IPC 命令**（37 个同步 `#[tauri::command]`，**不含** `get_selected_provider`——它的写回单列在第 4 类；含 `set_hotkey` / `set_pause_capture` / 各 setter）；
    2. **托盘**：`tray.rs::toggle_pause_capture`（`tray.rs:470` 同步回调）→ `ipc::settings::set_pause_capture_impl`；
    3. **翻译路径的迁移写回**：`ipc/translate.rs:381` 的调用者是 `translate_text_with_runtime`（由 `translate_text` 直接调用），它调 `get_selected_provider_impl`；
    4. **`get_selected_provider` 自身的迁移写回**：`get_selected_provider_impl`（`settings.rs:357`）在读到已移除的 provider 时用 `write_setting_field` 写回默认值。
    **结论**：所有写入者都是**同步路径**（已 grep 核实，异步 `spawn` / `spawn_blocking` 中没有任何设置写入）；**运行时线程归属待验证**（未用线程 id 断言）。
    **核实命令（可复现）**：
    ```bash
    # 1) 写入口（每个文件截断到 #[cfg(test)] 之前，避免把测试里的调用算进去）
    grep -rn 'write_setting_field(\|update_settings_value(\|update_json_object(\|save_accelerator(\|save_enabled(\|set_enabled_with_apply(\|set_[a-z_]*_impl(\|get_selected_provider_impl(\|\.save(' src-tauri/src
    # 2) 落盘原语（应只出现在 settings.rs 的 write_json_atomically 与 portable.rs 的 write_backup_file）
    grep -rn 'fs::write\|fs::rename\|fs::copy\|File::create\|OpenOptions' src-tauri/src
    # 3) 命令计数与 async 计数
    grep -c '#\[tauri::command\]' src-tauri/src/ipc/settings.rs   # 37
    grep -rn 'async fn' src-tauri/src/ipc/settings.rs              # 0
    ```
    **跨线程读取者**：后台剪贴板轮询线程（`lib.rs:508` 起的 `std::thread::spawn`）经
    `resolve_ingest_limits`（`lib.rs:595`）读 `settings.json` 取入库限额。它是**读**不是写，
    但在此之前「截断+写入」会让它读到半份文件 → 解析失败 → 临时回退默认限额（淘汰策略短暂变宽）。
    `f7a507a` 改为「临时文件 + rename」原子替换后，读侧只会看到旧快照或新快照，**撕裂读取风险消除**。旁证（现有代码注释）：`ipc/translate.rs:62-70` 论证同步命令由宏生成 `body_blocking`；回调**默认走 Tauri 自己的 `ipc://` 自定义协议（wry `WKURLSchemeHandler`，`objc2-web-kit` 声明 `MainThreadOnly`），回退路径才是 `WKScriptMessageHandler`**——两条通道的绑定层都要求主线程。**这是同一机制的另一处阅读结论，不是本条的实测**。若将来任何一侧移入异步上下文（`async fn` 命令、`spawn`、跨线程事件回调），`seq = 磁盘最大 + 1` 就有并发写窗口，必须补互斥。
  - **自启开关的「快速连点」与乱序响应（备注，未做代码防护）**：失败文案在**发起**切换时就清，所以连点两次且都失败时最终只显示最后一次的文案（探针结论，符合预期，不累积）。**未验证乱序响应**：两次写请求并发时，响应到达顺序没有保证，后到的旧响应可能把新状态或新文案覆盖回旧值——当前没有请求序号/序列化保护，属已知边界（真机上 IPC 串行执行，实际很难触发）。
  - **外来文件的归属边界（F，行为已由测试锁定）**：只有**形态严格为 `<配置文件>.<数字>.<数字>.bak`** 的文件归本应用所有（参与排序与 prune，**首次 prune 时可能被删除**，哪怕它是用户手工放进去的）；非此形态的文件（`<ms>.bak`、`<ms>-<n>.bak`、`settings.json.bak`、另一个配置的备份等）一律保留不动，也不参与「最近 5 份」的计数。测试：`foreign_backup_like_files_are_ignored_and_never_pruned`。
  - **启动期 apply 失败对界面不可见**（G）：`lib.rs` setup 里应用登录项偏好失败只 `eprintln`（不中断启动），用户重启后界面按配置显示「已开启」、而系统条目可能没注册——**这类失败没有任何界面反馈**，只能查日志。用户主动切换时才会走「回滚 + 报错」路径。
  - **`opError` 是单槽位**（I，既有语义，未改）：页面级操作（删除/粘贴/收藏/复制/打开系统设置）共用一条错误文案，**成功即清**（`4739507`），不分来源、不排队；同一时刻只显示最近一次失败的文案。与设置页错误条的「按来源隔离」是有意的差异：那边是持久配置项，这边是瞬时操作。
  - **Windows 残留登录项**（不修代码）：Windows 上若用户在「任务管理器 → 启动」里禁用过 QuickQuick，
    Run 值仍存在但 `is_enabled()` 返回 `false`；应用内关闭自启会因「条目已不存在即幂等成功」
    而短路，**不会清除那个残留的 Run 值**（后续若修，需要直接读/清 Run 键本身，而不是只看 `is_enabled`）。
    依据：`auto-launch` 0.5.0 `windows.rs:72-82` 的源码阅读；未验证（本机无法验证 Windows 注册表行为，
    故不做猜测性修法）。
  - **Windows 未编译验证**：`apply_with` 的「条目已不存在即幂等成功」（A）依据 `auto-launch` 0.5.0 的删除语义**源码阅读**，不是 Windows 实测；Windows 分支未编译、未运行。
  - **测试过滤器陷阱**（F）：`cargo test autostart` 只按**测试名子串**过滤，本项目里回滚类用例名不含 `autostart`（如 `set_enabled_rolls_back_config_when_os_apply_fails`），因此该过滤器**会漏跑回滚用例**。要跑整个集成测试文件必须用 `cargo test --test autostart`（本轮起以它为准）。
  - **错误条位置**：设置页内容区顶部（页面级固定条），业界做法保留现状；**未做视觉验证**（窄窗口换行、长文案 + 备份路径的观感）——L。
  - **读路径会产生备份副作用**（A，**有意取舍，不改代码**）：热键读取发现损坏时会先做一次保护性备份（为了让「已备份到 X」指向真实文件）。同内容去重后不会无限增长；但「坏 → 读（留备份）→ 修好 → 再坏 → 再读」会再留一份（内容不同）。总量受 prune 上限（5 份）约束，可接受。
  - **同源竞态**（C）：同一来源连续两次失败时，后一次覆盖前一次的错误文案（只保留最新一条）。错误条是提示而非审计日志，未做队列/合并。
  - **错误条展示顺序**（D）：按 source 字符串排序，不是按发生时间；多来源同时失败时顺序固定但与用户操作顺序无关。
  - **启动期备份未实跑**（E）：`lib.rs` 启动注册路径上的「损坏 → 保护性备份」只在单测里验证过（构造损坏文件 + 调 `load_lenient`），未在真实打包应用启动时实跑观察备份文件生成与 stderr 文案。
  - **热键回滚注销失败**（I）：只 `eprintln` 留痕、不改返回值。极端情况下新键仍处于注册状态，可能出现「改键失败但这个键仍能唤起」；日志可追踪，未做自动重试或注册状态回滚。
  - **备份与写入不是原子对**（先 copy 再 write）：极端情况下两次操作之间崩溃会留下完整 `.bak` + 未更新的原文件——**不会丢数据**。
  - **写失败提示已全站统一**（`e9e73ad`，不再是边界）：Maccy 无对应逻辑（UserDefaults 写入没有失败路径）；按业界做法抽了页面级 `settings-save-error` 通道（Context + `runSave` + `SettingsSaveErrorBanner`），**六个面板全部接入**（通用 / 外观 / 隐私 / 存储 / 热键 / 翻译源），失败显示后端中文原文、成功自动清空，控件仍受控回滚（aria-checked 不撒谎）。降级说明：面板单独渲染（无 Provider）时通道走兜底实现——`reportSaveError` / `runSave` 改为 `console.error` 留痕（**不再 warn、不静默**，有测试锚定该分支），`runSave` 仍如实返回 `ok:false`。
  - **hotkey.json / autostart.json 已改为逐键读写**（`1e5dde7`；**写路径逐键，读路径逐键于本批补齐** `ae0727b`）：`AutostartConfig::save_enabled` / `HotkeyConfig::save_accelerator` 走通用 `update_json_object`，只改目标键、保留未知键与其他动作；语法坏 / 根非对象时先备份再**拒写**。热键 setter 的前置读取同步改成容错版（`HotkeyConfig::load_or_default`），否则损坏文件会在「注册新键之前」就失败，永远走不到「备份 + 拒写」分支；落盘失败仍先注销刚注册的新键。
- **验收（实跑）**：
  - 新增 Rust 用例：坏字段只回退自身（其余字段与 `problems` 清单）、语法坏文件读路径回退默认并留痕、缺文件不算问题、`Default` 与 serde 缺省值逐字段一致、单字段写入保留其他字段与未知键、**语法坏文件被拒写且原文件逐字节不变（同时留一份备份、错误文案带备份名）**、**连续两次损坏留两份备份**、**备份失败（目录只读）返回 Err 且原文件不变**、**整份回退标记只在语法坏/根非对象时置位**、**启动提示只打一次**、**空文件允许首次写入**、缺文件不产生备份；IPC 层：坏字段下 setter 保留其余字段（含坏键原样）、语法坏文件 setter 返回中文 Err 且文件不变、读侧兄弟字段坏掉不影响别的 getter、未知键保留。
  - `cargo test` → **855 passed / 0 failed**（`902f7d5` 上实跑）；`cargo test settings` 过滤跑 **111 passed**。
  - 前端：`tsc -b` 退出 0；设置相关 vitest 子集（settings + clip-popover + clipboard）**45 files / 495 passed**；全量 **81 files / 871 passed**（`902f7d5` 上实跑，含「后端错误显示到 UI」与「CSS fallback 契约」的新用例）。
  - **变异自证**：把 `write_setting_field` 改回「宽松读出整份设置 → 只改目标键 → 全量落盘」 → **红 5**（两份「保留其他字段 / 未知键」、两组「语法坏文件备份」、IPC 侧保留用例）。还原后绿、无残留。
- **未验证**：真机上手工改坏 settings.json 后重启的表现（日志文案在打包 GUI 里是否可见——与 P4-7 同一限制）；Windows。

### P5-6 · i18n 框架 + 中英双语（namespace 分区 + 出货白名单）
### P5-7 · 设置页搜索

---

## Phase 6 · 工程加固

### P6-1 · CI 加验证（PR/push 跑 `make verify`）
- **状态**：done（workflow 已落地并通过静态校验；**未在 CI 实跑**，见下）
- **做什么**：新增 `.github/workflows/verify.yml`（`push` 到 `main` / 所有 `pull_request` / `workflow_dispatch`），两个 job：
  - `verify`：`checkout → pnpm 11 → node 22 → rust-toolchain(stable + rustfmt,clippy) → rust-cache → pnpm install --frozen-lockfile → make verify`。
  - `deadcode`：同款前端依赖，跑 `make deadcode-gate`（阻塞）+ knip 全量与后端清单（`continue-on-error: true`，只报告）。
  - `concurrency` 按 ref 分组并 `cancel-in-progress`，避免 PR 连推叠跑。
- **三处不要把事实写反**：
  1. **`release.yml` 本来就不跑验证**——它只在 `v*` tag 上打包发布（版本守卫 + 生成 ECDICT + tauri-action）。Makefile 里 `verify` 上方原注释「与 CI release 工作流复用同一条标准」是误记，已改成指向 `verify.yml`。本次**零改动** `release.yml`。
  2. **verify 不需要 ECDICT 词库**：实测干净树（`resources/` 空目录、无 `ecdict.db`）下 `cargo test` 照常编过——`bundle.resources` 的 glob 取不到文件只是空映射，不报错。所以没有复制 release.yml 的生成步骤，少一步网络依赖、少一个失败面。
  3. **干净克隆真正缺的是 `dist/`（本次只在干净树上实跑才暴露）**：`tauri::generate_context!` 是编译期宏，要求 `tauri.conf.json` 的 `frontendDist`（`"../dist"`）**目录存在**，否则 `proc macro panicked: ... this path doesn't exist`，clippy 与 cargo test 双双退出 101。本机一直有残留 `dist/`，所以本地 `make verify` 从来没暴露过这个前提。已修在 Makefile：新增 `dist` 前置目标（`pnpm exec vite build`，**仅在 `dist/` 不存在时触发**），并挂到 `check` / `test` / `test-rs` / `verify` 上。这不是业务代码改动，但它是「`make verify` 在干净树上可跑」这个前提的必要条件——不修，新 workflow 第一次跑必红。
- **平台矩阵：只跑 macOS（`macos-latest`）**，与发布矩阵里的 macOS 腿一致。Windows 不纳入，理由是**事实而非取舍**：后端 Windows 分支从未编译验证过（P1-1/P1-2/P1-3/P1-5/P1-7 均登记「Windows 全路径未编译验证」），挂上 `windows-latest` 会立刻红，而修它必须动 `src-tauri/src/**`（本 item 明令禁止）；且本机只有 `aarch64-apple-darwin` target，无法先本地预演。**开启条件**：Windows 分支能本地/临时 workflow 编过并通过 `cargo test` 后，再把 `windows-latest` 加进矩阵（届时 `make verify` 在 Windows runner 上还需要 `make`，GitHub 的 windows 镜像不自带，得改成等价的直跑命令）。
- **验收**：
  - `actionlint 1.7.12`（含 `shellcheck 0.11.0` 集成）对 `verify.yml` 与 `release.yml` **均 0 报错**。
  - **干净树实跑（最接近 CI 的一次）**：`git archive HEAD` 解出 HEAD + 只覆盖本 item 的改动，**无 `dist/`、无 `ecdict.db`**，`pnpm install --frozen-lockfile && make verify` → `VERIFY_EXIT=0`（`vite build` 由 `dist` 前置目标自动触发 398ms；tsc 无错误；fmt 干净；clippy 干净；vitest 74 文件 **751** 测试；cargo test **718 passed / 0 failed**）。之所以比工作树少，是因为工作树里还有别人未提交的新代码与新测试。
  - **工作树上也跑通过一次**（当时 `dist/` 与若干未提交改动并存）：vitest 74 文件 **753** 测试、cargo test **739 passed / 0 failed**。之后再跑工作树会红在 `cargo fmt --check`，原因是别人正在改 `db.rs` / `lib.rs`（P4-4 自动淘汰），与本 item 无关——这也说明「验证必须跑在能复现的树上」，不能拿脏工作树的一次红/绿当结论。
- **未在 CI 实跑（如实标注）**：本机无 GitHub runner，`verify.yml` 一次都没在真实 Actions 上跑过。已用 actionlint 保证「语法 / 事件 / runner 标签 / 表达式 / shell」层面成立，但**首次真跑仍可能暴露镜像差异**（例如 rust-cache 键、pnpm store 行为、macOS 镜像的 make 版本）。第一次 PR 请当作「验证 workflow 本身」的复盘对象，红了先看是不是镜像差异而不是代码。

### P6-2 · 覆盖率基线
- **状态**：done（前端 + 后端基线已实跑；**不设任何阈值**；`make coverage` 不参与 `make verify`；CI 工作流未在 GitHub 实跑，见未验证项）。commits `04a9d0e`（接线）、`4fcb261`（活动卡）。
- **做什么**：前端用 vitest v8 provider，`@vitest/coverage-v8` 钉精确版本 `2.1.9`（与 lockfile 中 vitest 解析版本一致）；脚本 `pnpm test:coverage`；配置在 `vite.config.ts` 的 `test.coverage`，统计 `src/**/*.{ts,tsx}`，排除测试文件与 `*.d.ts`，并**显式设置 `reportOnFailure: true`**（默认 false：测试失败时不产出任何报告且不报错，极易误判为 provider 未装上）。后端用 `cargo-llvm-cov`（前置 `llvm-tools-preview` 与 `cargo-llvm-cov`，Makefile 缺依赖时给出安装命令并退出 1）。Makefile 新增 `coverage` / `coverage-js` / `coverage-rs`；`verify` / `dist` / `check` / `test` / `test-rs` 的既有接线零改动。
- **CI**：新增 `.github/workflows/coverage.yml`，**仅 `workflow_dispatch`**，报告作为 artifact 上传；`verify.yml` 零改动。不挂进每个 PR：后端插桩全量重编（`target/llvm-cov-target` 约 715MB），比 `make verify` 慢一个量级，且此刻没有阈值可拦。
- **基线（取数 commit `d057ae2`，干净 worktree）**：
  - 前端（74 文件 / 768 测试全绿）：lines **91.32%**（4908/5374）、branches **90.72%**（1301/1434）、functions **87.83%**（325/370）。洼地是形态而非测试少：`src/ipc` 48.48%（全量 Tauri 命令薄封装，测试只覆盖 UI 实际走到的命令）、三个 `main.tsx` 0%、`panels/translate/browser-api.ts` 18.18%（浏览器兜底路径在 jsdom 中无法触发）。
  - 后端（35 个测试二进制 / 771 测试全绿）：lines **85.44%**（12958/15166）、regions 84.98%、functions 79.40%；**Rust 侧不产出分支覆盖（branches 0/0），两端分支覆盖不可比**。洼地是必须真窗口 / 系统事件才能跑到的路径：`popover.rs` 14.29%、`lib.rs` 19.84%、`ipc/update.rs` 23.86%、`macos_paste.rs` 48.91%、`window_pos.rs` 57.32%。反向证据：`tray.rs` 因 P5-1 补了菜单测试，从 19.74% 抬到 58.19%，基线对「补测试」敏感。
- **两个工具层面的坑（已实测，写进 Makefile）**：① `cargo llvm-cov` 不能同时给 `--html` 与 `--lcov`，需先跑一次测试，再用 `cargo llvm-cov report` 复用缓存出各报告；② macOS 自带的 Apple LLVM 21 的 `llvm-profdata` 只认 raw profile v10，rustc 1.99.1 产出 v11，合并直接失败，必须用 rustup 的 `llvm-tools-preview`。
- **未验证 / 未闭环**：① `coverage.yml` 未在真实 GitHub Actions 跑过（actionlint 1.7.7 通过），`taiki-e/install-action` 与 `rust-cache` 面对约 715MB 目录的行为未验证；② `clipboard-page.test.tsx` 富文本断言 flake **已定位并修（测试侧，commit `8c9fbd7`）**：根因是 `waitFor` 谓词用 `screen.getAllByText("Hello World")`，会被**预览区**提前满足——挂载期 `renderCount` 被清零让列表在某一提交里是 0 行、而预览已显示首条，随后被动 effect 才补齐，断言就落在中间提交上，结果随 RTL 收尾 `setTimeout(0)` 与 React 调度的先后抖动；同文件的搜索/类型筛选用例还依赖「预览区渲染未切分原文」才通过。修法：新增 `waitForListReady()`（先 `findByRole("listbox")` 再在**该子树内**等首行），全文 33 处同模式就绪门统一改用它，首条用例三条断言并入同一个 `waitFor`，筛选类断言改「行数 + 整行 `textContent`」；另补两条**预览区断言**（`within(previewRegion)`，commit `c8be745`）——搜索后预览须含匹配文案、筛 richtext 后预览须含富文本文案，反证（把预览来源从 `filteredItems` 换成未过滤的 `items`）会让类型筛选用例红在「Unable to find an element with the text: 富文本内容示例」。**业务代码未改**；`ClipboardPage` 挂载期把 `renderCount` 清成 0 的缺陷**未修**，已登记到 P4-2（需先补 `total=0` 单测）。验收：单文件 37 passed、14 路并发各一次全通过、全量 81 files / 871 passed；反证：把就绪门期望文本改成不存在时报 `Unable to find an element with the text: …`；③ 基线是快照，trunk 前进后需按活动卡取数方式在干净 worktree 重跑，不可直接引用旧数字比较；④ 本机后端工具链是 `/tmp` 临时副本（沙箱限制），仓库侧硬性前提仅为 `llvm-tools-preview` 与 `cargo-llvm-cov`。
- **明确未做**：不设阈值、不接第三方覆盖率服务、不挂进 `make verify`。
### P6-3 · 死代码检测（knip / cargo-udeps）
- **状态**：done（工具接入 + 基线清单 + 门禁分流；告警本身**零删除**，待裁决项见下）
- **前端 · knip**：`knip.json` 只钉三件事——三个 HTML 入口（`index.html` / `src/clip-popover/index.html` / `src/trans-popover/index.html`，与 `vite.config.ts` 的 `rollupOptions.input` 对齐）、`project: src/**/*.{ts,tsx}`、默认忽略。`knip` + `knip.json` 的版本随 `package.json` 的 devDependencies 走（当前 knip 6.42.1）。
  - **入口钉死是必须的**：去掉 `src/trans-popover/index.html` 后 `src/trans-popover/main.tsx` 立刻被判为未使用文件，说明第二个 webview 的接线只靠 HTML 入口这一根线，不能靠自动推断。
  - **灵敏度已实测**：临时新建一个未被任何文件引用的 `src/**/unused-probe.ts`，knip 会报 `Unused files (1)`；探针已删除。
- **门禁分流（关键取舍）**：`make deadcode-gate` = `knip --include files,unlisted`，**只拦「写完没接线的文件」与「import 了没声明的依赖」**——这两类修复方向唯一（接线或删除），不会长期挂着。未使用依赖与多余 `export` 进**报告**（`make deadcode`，CI 里 `continue-on-error`），因为删依赖/改导出面需要人裁决，直接拦会把 PR 常红，最终逼人加 ignore 掩盖。
- **现存告警（11 项，均为「已接线但不该导出 / 该不该留待定」，**没有一项是死文件**）**：

  | 类别 | 项 | 位置 | 判断 |
  |---|---|---|---|
  | 未使用依赖 | `@tauri-apps/plugin-autostart` | `package.json` | **待裁决**：前端 0 import；自启能力走自写 IPC（`settings.rs` 的 `apply_autostart_preference`），capabilities 里的 `autostart:allow-*` 来自 Rust crate 而非 npm 包。删前需确认不打算用插件 JS API |
  | 未使用依赖 | `@tauri-apps/plugin-updater` | `package.json` | **待裁决**：同上，更新检查走 `ipc/update.rs` 自写命令 |
  | 多余 `export` | `SECTION_LABELS` | `src/panels/settings/SectionNav.tsx:5` | 只在本文件用（第 75 行），`export` 多余；不是死代码 |
  | 多余导出类型 | `Matcher` | `src/clip-popover/clip-search.ts:32` | 本文件内用（41 / 112 / 148 行） |
  | 多余导出类型 | `ActionBarVariant` / `ActionBarAlign` | `src/components/ActionBar.tsx:5,8` | 本文件内用 |
  | 多余导出类型 | `PosDefinition` / `PasteOutcome` | `src/ipc/ipc-client.ts:41,512` | 本文件内用（33/34 行、516 行） |
  | 多余导出类型 | `ClipboardSub` / `TranslateSub` / `SettingsSub` | `src/main-window/nav.ts:10-12` | 本文件内用 |

  前端**零**「零引用导出」，说明 `tsc --noUnusedLocals` + 早期整理已经把「写了没人用」的前端符号清干净了；剩下的是 `export` 关键字冗余，降噪手段是 `ignoreExportsUsedInFile`，但**默认不开**——不开才能长期看见这 9 项，开了等于把它们永久藏起来。

- **后端 · 为什么不是 cargo-udeps**：`cargo-udeps` 要 nightly 工具链，且它查的是「Cargo.toml 里未被使用的 crate」；本项目最典型的事故形态是**模块/函数写好了、`pub fn` 也在、但生产路径零调用点**（`portable.rs`、`tiered_cleanup`、翻译 `cache/retry/cancel` 全是这个形态）。`rustc` 的 `dead_code` 也看不见：`pub` 条目在 lib crate 里天然对外可见，只有 `pub(crate)`/私有才会被报。
  所以落地为 `scripts/rust-dead-code-inventory.mjs`（人工清单口径，只读、恒 0 退出、CI 只报告）：扫 `src-tauri/src/**` 的 `pub` 声明，按三个引用面（**其它生产文件** / 同文件内 / `src-tauri/tests/**`）计数并分类，**先剔除注释与字符串字面量**再计数（否则「注释里提到过它」会把孤儿伪装成接线），并排除 `#[tauri::command]`（由前端按名字字符串调用，代码里没有标识符引用）。
- **脚本可信度用两条已知 ground truth 校准**（不是自证）：
  - `db::open_or_recover` —— P4-7 已登记「生产 0 调用点，只有 4 处测试调用」，脚本报「仅测试引用 / 4」。
  - `db::top_id` —— P4-1 子项已登记「生产无调用点，仅 tests/clipboard.rs 两处」，脚本报「仅测试引用 / 4 处引用」。
- **现存候选（190 项；本次扫描为 2026-10-25 快照，工作树含他人未提交改动）**：零引用 **1**、仅测试引用 **17**、仅本文件引用 172（参考项，`--all` 才逐条列出）、`#[allow(dead_code)]` 0 处。

  | 分类 | 项 | 位置（测试引用数） |
  |---|---|---|
  | **零引用** | `map_deepl_http_status` | `translate/providers.rs:2263`（0 / 0）——文档注释写着「供框架层在拿到 HTTP 状态码后调用」，实际**全仓无人调用**，DeepL 的状态码归一走的仍是响应体文案。这是本次唯一一个真孤儿 |
  | 仅测试引用 | `db::open_or_recover` `db::classify_failure` `db::recovery_action` `db::apply_recovery_action` `db::transient_keychain_error` | `db.rs:238 / 157 / 173 / 188 / 121`（4 / 5 / 4 / 2 / 2）——P4-7 已登记的「恢复策略与生产 `reset_and_reopen` 判据分裂」整体都在这一类 |
  | 仅测试引用 | `db::top_id` `db::list_ordered` | `db.rs:746 / 542`（4 / 2）——P4-1 子项已登记 |
  | 仅测试引用 | `clipboard::poll_once` `clipboard::snapshot_to_clips_for_test` | `clipboard.rs:312 / 267`（11 / 6）——生产只走 `poll_once_with_policy`，而 `clipboard.rs` 模块头注释仍写「实际运行时由后台线程循环调用 `poll_once`」，**注释与事实不一致** |
  | 仅测试引用 | `ipc::settings::set_hotkey_impl` | `ipc/settings.rs:134`（7）——`set_hotkey` 命令实际调 `set_hotkey_runtime_impl`；这个 impl 被 7 条测试养着，**测试测的是一条生产不走的路径** |
  | 仅测试引用 | `translate::history::translate_clip_item` | `translate/history.rs:137`（2）——注释称其为「唯一的写入口」，但生产没有调用点 |
  | 仅测试引用 | `translate::providers::on_quota_or_failure` | `providers.rs:2410`（6）——配额/失败后的 `UserPrompt` 决策函数，生产零调用 |
  | 仅测试引用 | `image::ingest_image` `image::is_degraded` | `image.rs:234 / 375`（3 / 2）——生产走 `ingest_image_with_policy` |
  | 仅测试引用 | `onboarding::onboarding_action` `onboarding::paste_capability` `paste::focus_restore_sequence` | `onboarding.rs:72 / 84`、`paste.rs:95`（3 / 3 / 2）——纯判定函数，生产未接 |

- **处置方式（明确写下，避免下一轮误删）**：本次**不删除任何代码**。逐项判断原则沿用 roadmap Q4 的「逐项接线，不搞一刀切」，且这些条目的共同特征是「语义仍需保留、只是缺接线」——
  - `map_deepl_http_status`：**已接线（commit `bf89578`，lead 裁定接线而非删除）**。接线前它不是单纯死代码，而是**缺陷**：DeepL 配额超限返回非标准 HTTP 456，落进 `map_by_http_status` 的 `ServerError` 兜底，`retry::is_transient` 把它判为瞬时，于是配额耗尽被退避重试满 3 次（`TRANSLATE_MAX_ATTEMPTS=3`，500/1000/2000ms）。接线点是 `error::map_by_http_status`（路由到 `providers::map_deepl_http_status`），**不在 provider 内部**：`HttpExecutor::execute` 返回 `Result<String, TranslateError>`，`ipc/translate.rs` 的 `normalize_ureq_error` 在非 2xx 处已丢弃状态码与 body，provider 侧永远拿不到 456。456 → 既有 `Quota` 变体（未新增变体），属永久失败、不进重试。验收：`cargo test translate::` 174 passed、全量 `cargo test` 485 passed、`clippy --all-targets -- -D warnings` 与 `fmt --check` 干净；变异（删除 456 路由分支）→ 2 条测试红。旁证：死代码清单「零引用」由 1 → 0（候选合计 188）。**未做**：`on_quota_or_failure` 仍是测试-only，UI 不会弹「填 Key / 升级套餐」引导；真机未撞配额，456 的实际响应形状来自 DeepL 文档而非实测；Windows 未编译验证。**根治方向（后续项，需动 `ipc/*`）**：让执行器返回 `(status, body)`，或让 `TranslateError` 携带 status，把状态码映射权整体交回 provider。
  - `db::*` 六项：与 P4-7 登记的「判据分裂」是同一件事，应随启动恢复路径那次改动一起处理（接线或统一删除），单独删会丢掉唯一的测试覆盖。
  - `poll_once` / `snapshot_to_clips_for_test` / `ingest_image` / `is_degraded`：**保留**——它们是契约测试的锚点（「无策略版必须保持同样基线推进语义」），删了等于自废护栏；但 `clipboard.rs` 模块头那句「运行时调用 `poll_once`」要改。
  - `set_hotkey_impl`：**保留待接线**——要么让命令回到它（并删 `set_hotkey_runtime_impl`），要么删它并把 7 条测试改指向运行时路径；两种都要 lead 裁决，因为涉及热键行为的验收口径。
  - `translate_clip_item` / `on_quota_or_failure` / `onboarding::*` / `focus_restore_sequence`：属「写了没接」的产品能力，**保留**，纳入对应功能项接线，不在本项删除。
- **未验证 / 已知盲区**：
  - **未在 CI 实跑**：`deadcode` job 与 `verify` job 一样，只过了 actionlint，没有真跑过。
  - knip 的 `project` 只含 `ts/tsx`，**不覆盖 CSS**（未引用的 `.css` 不会被报）；`.html` 只作入口不作分析对象。
  - 后端清单是**词法级启发式**，不是语义分析：宏生成的调用点、`Box<dyn Trait>` 的动态派发、trait 默认方法体内的调用都可能漏计；同名条目在多文件重复声明时计数会互相加成。它的定位是「给出候选 + 让人去看语义」，**不能当删除依据直接用**。

