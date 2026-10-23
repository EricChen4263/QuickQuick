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

### P1-8 · 串行化全部 `NSPasteboard` 访问（新增 · 由 P1-1 复审挂号）- **状态**：ready
- **为什么挂**：P1-1 复审时探针实证：**同一进程内两个线程并发访问 `NSPasteboard`（含 `types()` 与 arboard 正文读）会 SIGSEGV**。这条崩溃类**早于本轮 Phase 1 就存在**（轮询线程 arboard 读 + IPC 命令线程 arboard 写自 `f4c7211`/`73a64f4` 起并存），P1-1 只消除了自己新增的那一面（把 `types()` 移到主线程）。强行在 P1-1 闭环等于提前做 P3-2。
- **做什么**：把 `pipeline::ArboardBackend` 的全部 `NSPasteboard` 访问（`change_count` 的三段读 + `read` 的三段读 + 写回路径）统一走单一「主线程串行执行 + 结果快照」通道；与 P3-2「`changeCount` 替代内容哈希轮询」一起做（后者会改变去重语义：同一内容复制两次各记一条，属用户可见变化，需单独验收）。
- **验收**：探针「主线程与后台线程同时跑全部读路径，连续 N 次不崩」+ 真机长跑（切 app / 粘贴 / 复制混合）无崩溃。
- **前置**：Phase 3 的集成测试覆盖。

### P1-9 · 存储页占用口径对齐（新增 · 由 P1-4 复审挂号）
- **状态**：ready
- **问题**：存储页拿 `get_storage_stats.file_size_bytes`（**库文件大小**）与「上限 500 MB」比较并画进度条，而 P1-4 的清理删除的是**图片逻辑字节**。SQLite 的 `DELETE` 只把页标记为可复用、不收缩文件，所以清理后进度条不会下降——用户点「立即清理」看不到任何变化，又是一次「UI 承诺与实现脱节」。
- **做什么（按优先级）**：① 让上限判定与展示口径一致——「图片占用」用 `image::total_image_bytes` 这类**逻辑字节**（与清理阈值同源），库文件体积单列展示（它代表磁盘占用，两者含义不同，不是二选一）；② 文案写清两个数字各是什么；③ 是否提供「压缩数据库（VACUUM）」作为**低频手动动作**可讨论——加密库 VACUUM 代价高且锁库，不适合挂在每次自动清理上。
- **验收**：清理后「图片占用」数字立刻下降并与上限判定一致；库文件体积与图片占用分别标注；新增 IPC/纯函数单测；前端测试覆盖两个数字的渲染。

---

## Phase 2 · 交互地基（依赖 Phase 0 人工验证）

> **blocked**：需先完成 ADR §6.2 的 8 项人工验证。若面板语义不成立，本阶段整体改写。

### P2-1 · 面板高度自适应
### P2-2 · 键盘模型升级（`⌃N/P/J/K`、`⌘↑↓`、`⌃W/U/H`、`⌥⌫`、`⌥P`、`⌘,`）
### P2-3 · 数字直达（`1`–`0` 取前 10 条）
### P2-4 · 字母 pin（`b`–`y` 持久化键位 + 设置页管理）
### P2-5 · cycle 模式（按住热键连按翻页、松手即粘贴）
### P2-6 · 输入法防御（composition 期间禁用方向键导航）
### P2-7 · 主窗键盘流修复（Enter 粘贴、`⌘1-9` 直接粘贴）
### P2-8 · popover 管理动作（pin / 删除 / 清空）
### P2-9 · 唤起位置可选（光标/图标/中心/上次 + 多屏）
### P2-10 · 行内预览分级延迟（首 1500ms → 后续 200ms）
### P2-11 · 快捷键徽标随修饰键变

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
