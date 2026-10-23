---
status: done
category: implementation
subject: 对标 Maccy · Phase 1 还诚信债（P1-1 ~ P1-7 全接线 + 新增 P1-8/P1-9 挂号）
summary: 把「UI 已承诺、代码未接线」的七项能力真正接上：敏感内容过滤、来源应用排除、私有 UTI 自产标记、tiered_cleanup 自动触发、便携备份导出/导入、翻译缓存、翻译重试/取消/错误归一。两项在独立审查中被打回（NSPasteboard 跨线程必崩、第二级清理产生幽灵条目），修复后复审闭环；由此新增 P1-8（串行化全部 pasteboard 访问）与 P1-9（存储页占用口径）两个挂号项。
evidence:
  commits:
    - 8ee670a feat(privacy): 敏感内容标记真实读取，concealed 过滤不再恒假
    - c731277 feat(privacy): 来源应用排除接线，应用名与 bundle id 双匹配
    - d916035 feat(privacy): 私有自产标记接线，粘贴后不再自捕一条
    - b2245da feat(storage): tiered_cleanup 自动触发（P1-4）
    - 50e112f feat(backup): 可移植备份导出/导入接线（P1-5）
    - 3703ed1 feat(settings): 备份/恢复入口（P1-5）
    - e43210b feat(translate): 翻译缓存接入主链路（四元组键 + LRU 淘汰）
    - 970f369 feat(translate): 接入重试 / 取消 / 错误归一（P1-7）
    - 4241255 fix(privacy): 剪贴板标记读取改为主线程派发，消除跨线程 types() 崩溃（P1-1 复审修复）
    - 985ac6d docs(privacy): 标明跨线程 pasteboard 残留风险归既有问题并挂 P1-8
    - 2d9b8c7 fix(storage): 第二级清理同步软删父条目，消除幽灵条目与不释放的 BLOB（P1-4 复审修复）
    - 62e591e perf(storage): 第三级回收仅在本次清理生效时执行，并把幂等断言锚到 purge 自身
    - b707ca0 fix(backup): 导入载荷大小上限 + 目录回退留痕 + ipc-client invoke 测试
    - 7eee773 docs(translate): 修正同步命令在主线程执行的注释与事实偏差（P1-7 复审修复）
    - 2ce8e95 test(translate): 补 is_timeout 超时链路单测并锁定 ureq 版本敏感点
    - 55f24b3 docs(privacy): 修掉 SelfWriteLatch 文档链接告警并补 .Exe 测试锚
    - 74cfe71 docs(storage): 更正分级清理调用第三级回收的时机描述
  commands:
    - "cd src-tauri && cargo check --all-targets"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "cd src-tauri && cargo fmt --all --check"
    - "cd src-tauri && cargo test"
    - "pnpm exec tsc -b && pnpm test"
  urls:
    - "https://developer.apple.com/documentation/appkit/nspasteboard"
    - "https://github.com/p0deje/Maccy"
---

# AI 活动记录 · Maccy 对标 Phase 1（还诚信债）

## 摘要

`docs/design/maccy-parity-roadmap.md` 判断四的核心指控是：一批能力「UI 已承诺、代码未接线」，设置页在向用户撒谎。本轮把七项全部接通，并逐项走了「执行 → 独立审查 → 独立 commit」的无人值守闭环。

七个 work item 的最终落点：

| item | 落点 |
|---|---|
| P1-1 敏感内容过滤 | 新增 `pasteboard_meta`（纯谓词 + 平台 FFI），macOS 读 `ConcealedType`/`TransientType`，Windows 读两个剪贴板格式 |
| P1-2 来源应用排除 | `SourceApp{bundle_id, display_name}`，身份取自**主线程**激活观察者；名单**名字与 bundle id 双匹配**（前端提示的正是应用名） |
| P1-3 私有 UTI 自产标记 | macOS `addTypes:owner:` 追加 `com.quickquick.self-marker`；Windows 明确降级为写入门闩并写明原因 |
| P1-4 `tiered_cleanup` | 事务提交后、仅「本轮新增图片行」时触发；阈值沿用 `DEFAULT_MAX_TOTAL`（与存储页同一常量） |
| P1-5 便携备份 | 版本化载荷 + 三个 IPC 命令 + 存储页「备份/恢复」；导入**不复用 `ingest`**（避免 bump 打乱顺序） |
| P1-6 翻译缓存 | 方向解析前移，键为最终语言四元组；**只缓存 Plain**（Dict 结构无处存放，写缓存等于静默降级） |
| P1-7 重试/取消/错误归一 | `retry_with_backoff` 包住整个 provider 编排；`InflightTracker` 校验在副作用之前；`UreqExecutor` 走归一映射 |

## 独立审查打回的两处（本轮真正的价值）

无人值守不等于「自己写自己过」。两次 BLOCK 都是审查者用**隔离探针**打出来的，而不是读代码猜出来的：

**1. P1-1：`NSPasteboard` 跨线程访问必崩**
初版把 `NSPasteboard.types()` 放在 500ms 轮询线程里。审查者用 10 组探针二分定位：单线程读不崩，**两个线程都调 `types()` 3/3～4/4 必崩**（SIGSEGV/SIGTRAP）。修法是把平台标记读取整体搬到主线程（注入式 `PasteboardMetaReader` + `TauriMainThreadDispatch`，300ms 超时降级 + 节流留痕），复审又用反向探针证明「串行化到主线程」正是消除该崩溃的充分条件（串行 3/3 不崩 vs 并行 3/3 崩）。

修复过程中执行者还查出更深一层：只搬 `types()` 只消掉一半，**后台 arboard 正文读与主线程 pasteboard 访问并发同样会崩**（3/3）。这条崩溃类**早于本 Phase 就存在**（轮询线程 arboard 读 + 命令线程 arboard 写自 `f4c7211`/`73a64f4` 起并存），因此没有在 P1-1 里强行闭环，而是挂号 **P1-8**（串行化全部 pasteboard 访问，与 Phase 3 的 `changeCount` 改造一起做）——理由写进了 `pasteboard_meta` 模块文档，不是静默带过。

**2. P1-4：第二级清理产生幽灵条目**
`tiered_cleanup` 的第二级只软删 `clip_images`、从不软删父条目 `clip_items`。这个 bug 在「零调用」状态下潜伏了很久，一接线就变成用户可见缺陷：前端列表以 `clip_items` 为主表，于是渲染出无缩略图、不可粘贴的 `[图片]` 占位条目；且 `gc_purge_deleted` 只删 `clip_items` 墓碑，级联永不触发，BLOB 永久留在库文件里。审查者用真实入库路径 + 极小阈值复现了它。修法是第二级复用 `db::soft_delete` 同步软删父条目，并新增第三级 `purge_orphan_image_tombstones` 回收无父条目的孤立墓碑。

## 过程中确认/踩到的坑

- **前端提示语决定匹配语义**：排除名单输入框写的是「应用名称（如 1Password）」，只匹配 bundle id 等于让用户输入永远不命中——那还是撒谎。最终实现成名字与 bundle id 大小写不敏感双匹配。
- **不在轮询线程查 `frontmostApplication`**：仓库里已有实测结论（`ipc/system.rs`）——AppKit 前台状态跨线程读是 stale 的。改用主线程激活观察者记录「最近一次被外部应用激活」，并把这条语义边界（复制后 500ms 内切走应用可能误判）写进文档。
- **`declareTypes:` 会清空剪贴板**：追加私有标记只能用 `addTypes:owner:` + `setData:forType:`。审查者另建探针独立复核了正文逐字节完好、原类型不丢、下次正常复制标记必被清。
- **Windows 无法追加自定义剪贴板格式**（arboard 独占写），因此自产标记在 Windows 降级为「写入门闩」一次性消费语义，原因写进代码注释而非含糊带过。
- **同步 `#[tauri::command]` 跑在主线程**：审查者逐层查 tauri-macros `body_blocking` → wry `WKScriptMessageHandler: MainThreadOnly` 证实，据此改掉了 P1-7 里一句「不阻塞 UI」的错误注释。这是既有缺陷（同步命令本来就在主线程做 10s 网络 I/O），退避只是在其上叠加最多 1.5s，根治需 `async` + `spawn_blocking`，未在本轮越界处理。
- **`file_size_bytes` 在 `DELETE` 后不会缩小**：SQLite 只标记页可复用、不收缩文件。存储页拿库文件体积去比「图片总量 500 MB」上限，口径本就不一致，清理后进度条不会下降——挂号 **P1-9**。

## 验证

Lead 侧在最终 HEAD（`74cfe71`）实跑：`cargo check --all-targets` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --all --check` 全绿，`cargo test` 无失败；`pnpm exec tsc -b` 无错，`pnpm test` 全绿。每个 commit 各自的数字见 work items 记录。

## 未闭环（必须人工）

Phase 1 的验收里有四条**只能真机 GUI 验证**，本轮全部如实标注为 pending-manual：

1. 从 1Password / Bitwarden 复制密码不入库；关掉「跳过敏感」后仍可入库。
2. 名单内 app 复制不入库、名单外正常入库。
3. 从历史粘贴后不产生新条目、不打乱顺序。
4. 备份导出→换空库导入的端到端（`AppHandle` 目录解析、Finder 弹出、`<input type="file">` 与 `window.confirm`、macOS TCC 授权弹窗）。

另有两条环境边界：Windows 全路径未编译验证（本机只有 `aarch64-apple-darwin`，符号与 feature 归属已按 `windows-sys 0.61.2` 源码逐一核对）。

## 为什么 Phase 2 停在这里

路线图 Phase 2 的前置是 `docs/design/clip-popover-nspanel.md` §6.2 的 8 项人工验证（面板是否真不夺焦、中文输入法 composition、面板吞 ⌘V、全屏 Space 可见性等）。这批是**真人操作 GUI** 才能给结论的项，属于无人值守的停止条件，需要用户安排或授权后才继续。
