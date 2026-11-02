# Popover 大图预览（对齐 Maccy 行内预览）

## 背景
- 用户希望点击图片条目的缩略图时能看大图，并记得 Maccy 有这个功能。
- 核对 Maccy 2.6.1 源码（tag `2.6.1`）：Maccy 没有「点击放大」。点击行 = 选中并粘贴；大图来自选中后延迟弹出的行内 popover（`HistoryItemView` 的 `.popover(isPresented: $item.showPreview)`），图片按屏幕可见区缩放，触发延迟沿用 `previewDelay`。
- 我们的主窗口 `ClipPreview` 已经用原图，但 Popover 右侧预览只用 256px 缩略图，放大后偏糊，这是主要差距。

## 改动
- `src-tauri/src/image.rs`：抽出 `encode_webp_within`，缩略图与预览共用「解码 → 最长边缩放 → WebP 编码」流水线；新增 `make_preview`（最长边上限 1600px，质量 85）。缩略图行为不变。
- `src-tauri/src/ipc/clipboard.rs`：新增 `encode_preview_data_url` 与 async 命令 `get_clip_image_preview`。DB 锁只在读取原图字节时持有，解码编码放到 `spawn_blocking`。
- `src-tauri/src/lib.rs`：注册新命令。
- `src/ipc/ipc-client.ts`：新增 `getClipImagePreview`。
- `src/clip-popover/useClipPreviewImage.ts`（新）：按 imageId 懒取，不做模块级缓存，返回值只对当前 imageId 有效。
- `src/clip-popover/PopoverPreview.tsx`：回落链改为 预览 → 缩略图 → `[图片]` 占位。
- `src/clip-popover/popover.css`：`.popover-preview-img` 增加 `max-height: 70vh`。

## 取舍
- 尺寸：没有按整屏可见区缩放（Maccy 的做法）。Popover 预览区远小于屏幕，按屏幕缩放只会多传字节，所以用固定上限 1600px。
- 缓存：不做无上限缓存。预览图是 WebP，单张数百 KB；重复查看同一张时重新生成，成本可接受。
- 主窗口 `ClipPreview` 保持原图方案不动，本次不扩大范围。

## 验证
- `cargo test --lib`：598 通过（新增 5 个预览用例：缩放上限与等比、不放大、无原图返回 None、损坏原图返回带上下文的 Err、入库→预览端到端）。
- `vitest run`：全量 932 通过（新增 `popover-image-preview.test.tsx` 5 个回落链用例、`popover-style.test.ts` 1 个 max-height 契约）。
- `tsc --noEmit`：0 错误。`cargo clippy --all-targets -D warnings`：通过。`cargo fmt --check`：通过。
- 构建：`pnpm tauri build --bundles app`（需 `CI=true`，否则后台 pnpm 因无 TTY 中止删除 node_modules）；已替换 `/Applications/QuickQuick.app`，原版备份在 `~/QuickQuick-before-preview.app`。
- 真机（第一版）：⌘⇧C 打开 Popover，选中一张图片后右侧预览正常显示；进程存活，无新的崩溃报告。
- 独立只读审查（subagent，非 BLOCK）的建议处理：
  - 补竞态与卸载测试：A→B 切换时上一张迟到的预览被丢弃，卸载后 resolve 不报错、不输出日志。变异检验：去掉 `cancelled` 守卫后竞态用例失败，恢复后通过。
  - 修正注释：预览替换缩略图会改变显示尺寸，这是已知的一次性视觉变化，不是「不跳动」；缩略图同样经 IPC 按需拉取。
  - 失败与 null 回落用例改为可控 Promise，等请求结算后再断言，不再依赖缩略图先到的时序。
  - `popover-style.test.ts` 的 max-height 断言从 `≤80` 收紧为 `=70`。
  - CSS：`.popover-preview-img` 增加 `min-height: 0`，允许作为预览列 flex 子项被压缩，避免 `min-height:auto` 锁死导致底部被 overflow:hidden 裁掉。
  - Rust 文档：`decode_webp_dims` 的注释改为描述其真实行为（解码并返回宽高）。
- 复验（第二版，含 CSS 修正后重新构建安装）：vitest 全量 934 通过；tsc 0 错误；clippy `-D warnings`、fmt、clipboard 27 项通过。真机：选中图片行后预览约 380pt 宽，四角圆角完整，未见底部裁切；Esc 关闭，未粘贴；进程存活。

## 未覆盖
- 没有用像素比对证明预览一定是 1600px 版本，源头由单测与代码路径保证。
- 预览的生成耗时（大原图）只做了设计层面的隔离，未在真机量测。
- 未验证切换图片时的视觉闪烁（缩略图先出、预览后替换）是否可接受，留待用户体验确认。
- 未用超高图实测 70vh 裁切（本次真机用的是普通比例截图）。CSS 已加 `min-height: 0` 作防御，但超高图的真机表现待测。
- 33 个左右的测试文件 mock `ipc-client` 时没有包含 `getClipImagePreview`；只要渲染到图片预览分支就会抛错。当前全量通过，说明暂未触发，属潜在脆弱点。
- 超大原图解码内存偏高，与既有缩略图路径相同，未在本次统一加解码尺寸上限。
