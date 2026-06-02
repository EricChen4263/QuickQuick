---
id: f4-gui-fixes-s03-test
title: 单张图片阈值 动态证伪
status: 测试通过
commit: 8f5f8d5
date: 2026-06-02
---

## 命中校验

- `cargo test` 全绿（含 `ingest_image_as_clip_respects_small_threshold`、`set_image_threshold_rejects_too_small`、`set_image_threshold_rejects_too_large`、default 值测试）。
- `pnpm test` 346 tests 全绿，`StoragePanel.test.tsx` 6 个用例全部命中（含「选 50MB → setImageThreshold 以 52428800 调用」断言）。

## 变异 Sanity（A-E 全部如期变红）

| 变异 | 改动位置 | 改坏内容 | 结果 |
|------|----------|----------|------|
| A | `src-tauri/src/clipboard/db.rs` | 阈值比较 `size > threshold` → `size > usize::MAX` | `ingest_image_as_clip_respects_small_threshold` 变红 |
| B | `src-tauri/src/ipc/commands.rs` | 去掉下界校验（≥ 1 MiB）| `set_image_threshold_rejects_too_small` 变红 |
| C | `src-tauri/src/ipc/commands.rs` | 去掉上界校验（≤ 500 MiB）| `set_image_threshold_rejects_too_large` 变红 |
| D | `src-tauri/src/clipboard/db.rs` | `default()` 返回 `1` 而非正确默认值 | 默认值断言变红 |
| E | `src/panels/settings/StoragePanel.tsx` | `mb * 1024 * 1024` → `mb`（去掉单位换算）| `expect(mockSetImageThreshold).toHaveBeenCalledWith(52428800)` 变红（实际收到 50）|

所有变异还原均通过备份文件（`cp /tmp/*.bak`）复原，未使用 `git checkout`。

## 边界行为

- `db.rs` 阈值判断为 `>`（严格大于），图片大小 == 阈值时 `original_present=1`，保留原图，符合"恰好在界内"语义。
- IPC 校验区间 1 MiB..=500 MiB 含端点（1 MiB 和 500 MiB 均可通过）。

## Git 状态证明

开工快照：`git status --porcelain` 输出为空（无未提交改动）。  
收工快照：`git status --porcelain` 输出为空（逐行一致）。  
所有变异均已从备份还原，工作树与开工时一致。

## 最终裁决

**测试通过，放行。**  
命中校验 + 5 组变异 sanity 全部如期，边界行为符合预期，git 工作树干净。
