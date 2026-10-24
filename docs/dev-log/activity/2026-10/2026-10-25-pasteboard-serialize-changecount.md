---
status: done
category: implementation
subject: Maccy 对标 · P1-8 + P3-2（全部 NSPasteboard 访问串行到主线程 + changeCount 替代内容哈希轮询，去重语义不变）
summary: macOS 剪贴板轮询改为「主线程读 changeCount 标量 → 变了才在主线程做一次全量读 → 快照交后台入库」，复制/粘贴写回与 MacOsPasteBackend 的读写也经同一主线程通道。探针（主线程 + 4 个后台线程同时跑读写全路径，每轮 2800 次 × 3 次运行）0 崩溃；去掉主线程派发的变异 3/3 崩溃（SIGSEGV ×1、SIGTRAP ×2）。changeCount 检测改为恒真时契约测试红 2 条。用户已裁决去重语义保持现状，repeated_identical_text_copy_dedups_and_bumps 未改断言。
evidence:
  commits: ["71d18f8"]
  commands:
    - "cargo test（810 passed / 37 suites）"
    - "cargo clippy --all-targets -- -D warnings（干净）"
    - "cargo fmt --all --check（干净）"
    - "cargo run --example pasteboard_serial_probe -- --iterations 700 --workers 4"
    - "cargo run --example pasteboard_serial_probe -- --iterations 700 --workers 4 --no-dispatch"
  urls: []
---

# AI 活动记录 · P1-8 + P3-2

## 改了什么

| 文件 | 变化 |
|---|---|
| `src-tauri/src/pasteboard_serial.rs`（新） | `MainThreadExecutor` 接缝 + `SerialPasteboard`（带超时、panic 拦截、已在主线程则就地执行）+ 进程级 `install_global/run_global` + macOS 生产后端 `MainThreadPasteboardBackend` |
| `src-tauri/src/clipboard.rs` | `ClipboardBackend::read_observed`（默认实现等价旧行为）、`ObservedClipboard`、`observe_backend`、`ClipboardSnapshot::empty` |
| `src-tauri/src/lib.rs` | 轮询循环拆成「无锁观察 → 持锁入库」两阶段；`build_clipboard_backend` 取代 `build_meta_reader` |
| `src-tauri/src/macos_paste.rs` | macOS 上 `write_*_to_clipboard`、`MacOsPasteBackend` 的 changeCount/写/读文本全部经串行通道；写正文与追加自产标记合成一个主线程任务 |
| `src-tauri/src/pasteboard_meta.rs` | 删掉旧的 `MetaDispatch`/`MainThreadMetaReader`/`TauriMainThreadDispatch`（被通用通道取代）；`PasteboardMetaReader`/`DirectMetaReader` 限定非 macOS |
| `src-tauri/src/pipeline.rs` | `ArboardBackend`（内容哈希模拟计数）限定非 macOS 编译 |
| `src-tauri/tests/capture_contract.rs` | 新增 6 条观察层契约；`repeated_identical_text_copy_dedups_and_bumps` 仅改了说明注释，断言未动 |
| `src-tauri/examples/pasteboard_serial_probe.rs`（新） | 并发探针，见下 |

## 关键决策与被否决的方案

1. **主线程只付一次标量读取**：每 500ms 在主线程读 `NSPasteboard.changeCount`；只有变了才做一次全量读（计数 + 文本 + HTML + 图片 + 类型标记在同一个主线程任务里）。否决「把现有内容哈希轮询整体搬到主线程」：每轮要读一遍图片 RGBA 并哈希，主线程明显卡顿，换来的只是崩溃概率降低。
2. **两阶段轮询、观察阶段不持锁**：主线程同步 IPC 命令会取 `AppDb` 锁；若轮询线程持库锁等主线程、主线程等库锁，就互等到超时。因此「碰剪贴板」放在取库锁之前，之后用 `ObservedClipboard`（回放既成事实的后端）继续走原有 `capture_and_ingest`，隐私/基线/去重判定一行未复制。
3. **读取失败不推进基线**：`read_observed` 返回 `None`（主线程忙/超时）时本轮整体跳过，下一轮重试；不会把「读失败」当成空剪贴板吞掉内容。
4. **基线取「读取时刻」的计数**：计数在全量读之前取，读取期间若再变化，下一轮会再读一次，重复内容由库去重（Bump）吸收；不会吞掉中途变化。
5. **写正文 + 追加自产标记合成一个主线程任务**：避免轮询读到「正文已写、标记未到」的中间态把自己的写入再记一遍。
6. **Windows 路径原样保留**：`ArboardBackend` + `DirectMetaReader` 仅 cfg 隔离，行为未改。**未在 Windows 编译验证**；仅用「把 `target_os="macos"` 翻转成 `linux` 的临时副本」做过一次非 macOS 分支的 `cargo check --lib`（0 error），这不是 Windows 编译。
7. 去重语义保持现状（用户裁决 2026-10-25）：同一文本再次复制仍 Bump 置顶、不新增行。

## 实测

- `cargo test`：810 passed / 37 suites；`cargo clippy --all-targets -- -D warnings`：干净；`cargo fmt --all --check`：干净。
- **探针**（`examples/pasteboard_serial_probe.rs`，会改写系统剪贴板）：
  - `--iterations 700 --workers 4`：每次 2800 轮后台读写（changeCount + 全量读 + 文本/HTML/图片写回 + MacOsPasteBackend 读写），同时主线程泵任务并自己访问 14004 次。**3 次运行全部 exit 0**，约 2.2s/次，共 8400 轮后台 + 全部读写路径，0 崩溃。
  - `--no-dispatch`（变异：执行器改为就地执行、去掉主线程派发）：**3/3 崩溃**（exit 139 SIGSEGV ×1，exit 133 SIGTRAP ×2）。
- **契约变异**：把 `observe_backend` 的「计数相同则 None」判断删掉（检测恒真）→ `observe_unchanged_count_returns_none_and_never_reads`、`observe_then_ingest_keeps_dedup_and_bump_semantics` 两条红；还原后 20/20 绿。

## 未验证 / 待人工

- 真机长跑（切 app / 粘贴 / 复制混合）未做，**待人工**。
- 大图（4K 截图）主线程全量读的卡顿体感未测；全量读等待上限 3s、标量 300ms，超时只丢本轮。
- Windows 全路径未编译验证。
- 探针里的主线程是自建的「泵」线程模型，与 Tauri 主线程（AppKit run loop）等价性靠 `is_main_thread()` 的 `MainThreadMarker` 判定，未在真实 Tauri 事件循环里跑探针。
