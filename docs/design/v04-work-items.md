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
- **前置**：Phase 3 的集成测试覆盖。

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
- **状态**：ready（不依赖面板语义）
### P2-11 · 快捷键徽标随修饰键变
- **状态**：ready（不依赖面板语义；**同时收下 P2-2/P2-3 引入的新键位零提示问题**——footer 目前仍只有 ↵/⌥↵/↑↓/esc，1–0、`⌃N/P/J/K`、`⌘↑↓`、`⌃W/U/H` 全部无 UI 线索）

### Phase 2 遗留挂号（本轮记录，未立项）
- 主窗 `⌘0` 取不到第 10 条（`quickSelectIndex` 只认 1–9）。
- `⌃W` 中文无标点长段整段删除。
- `⇧↑↓` 在搜索框内不能选文字（**既有问题，非本批引入**）。
- 新键位与组合键矩阵（`⌘⇧↑↓`、`⌃⇧N` 等）需要一次统一决策。

---

## Phase 3 · 捕获与规则

### P3-1 · 换掉 arboard，直连 `NSPasteboard`
- 前置：先补集成测试覆盖现有捕获路径，再动手
### P3-2 · `changeCount` 替代内容哈希轮询（消除每 500ms 全量读取）
### P3-3 · pasteboard 类型白名单（可配）
### P3-4 · 文件类型条目（捕获 + 图标 + 复制回写）
### P3-5 · 忽略规则三层增强（应用黑/白名单 + 类型集合 + 内容正则）
### P3-6 · 暂停语义补齐（托盘 `⌥` 点击、`⌥⇧` 仅忽略下一次）
### P3-7 · 多 pasteboard item 合并
### P3-8 · 去重精度提升（集合包含判定 + 复制次数累加）
### P3-9 · OCR（图片可被文字搜索命中，Swift helper + IPC）

---

## Phase 4 · 存储与性能

### P4-1 · 建索引（`is_deleted+last_modified_utc`、`text_hash`、`image_hash`）
### P4-2 · 分页 + 虚拟滚动
### P4-3 · 缩略图移出 IPC payload（消除 base64 全量重传）
### P4-4 · 自动淘汰 + 可配上限（1–999，pin 豁免）
### P4-5 · 搜索升级（预计算 title + 4 模式 + 多词 AND + 命中高亮）
### P4-6 · 翻译历史分页
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
