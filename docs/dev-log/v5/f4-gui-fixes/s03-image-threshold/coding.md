---
id: f4-gui-fixes-s03-backend
title: 单张图片阈值 后端接真
status: 实现完成
commit: d60fd93
date: 2026-06-02
---

## 改动文件清单

| 文件 | 改动说明 |
|------|---------|
| `src-tauri/src/settings.rs` | 新增 `max_image_bytes: u64` 字段（serde default = 20MiB），含 legacy JSON 兼容测试 |
| `src-tauri/src/ipc/settings.rs` | 新增 `get_image_threshold_impl` / `set_image_threshold_impl` 纯函数 + Tauri 命令封装，校验区间 1MiB..=500MiB，含 4 条单元测试 |
| `src-tauri/src/lib.rs` | 注册 `get_image_threshold` / `set_image_threshold` 命令；轮询循环每轮从 settings.json 读取 `max_image_bytes` 后传入 `capture_and_ingest` |
| `src-tauri/src/db.rs` | `ingest_image_as_clip` 新增 `max_image_bytes: u64` 参数，透传至 `ingest_image_with_policy`；含阈值生效测试（小阈值/大阈值各一条） |
| `src-tauri/src/pipeline.rs` | `capture_and_ingest` / `ingest_clips` 新增 `max_image_bytes: u64` 参数，传至 db 层 |
| `src-tauri/src/ipc/clipboard.rs` | 测试辅助调用点（`ingest_image_as_clip` 单测内）传入 `20 * 1024 * 1024` |
| `src-tauri/tests/boot_pipeline.rs` | 集成测试调用 `capture_and_ingest` 补第 5 参数 `20 * 1024 * 1024`（收尾补全） |
| `src-tauri/tests/capture_image.rs` | 集成测试调用 `capture_and_ingest` 补第 5 参数 `20 * 1024 * 1024`（收尾补全） |

## TDD 阈值生效测试

`db::tests::ingest_image_as_clip_respects_small_threshold`：传 `max_image_bytes=1`，断言 `original_present=0` 且 BLOB 为空；`ingest_image_as_clip_respects_large_threshold`：传 `100MiB`，断言 `original_present=1` 且 BLOB 非空——双向非恒真断言。

## ingest 调用点配置读取方式

`lib.rs` 轮询循环每轮调用 `AppSettings::load_or_default(&settings_json_path).max_image_bytes`，失败时 fallback 到 `AppSettings::default().max_image_bytes`（20MiB），保证阈值变更轮询下一次即生效。

## 校验区间

`MIN_IMAGE_THRESHOLD = 1MiB`，`MAX_IMAGE_THRESHOLD = 500MiB`；越界返回中文 Err、不写文件。

## cargo test / check 结果

- `cargo check`：exit 0，零 error，零 warning
- `cargo test`：**全部通过，共 76 个 unit tests + 57 个 integration tests**（含 `ingest_image_as_clip_respects_small_threshold` / `ingest_image_as_clip_respects_large_threshold` / `set_image_threshold_*` 四条校验测试）

## 给前端的命令契约

| 命令 | 签名 | 返回 | 说明 |
|------|------|------|------|
| `get_image_threshold` | `get_image_threshold() -> Result<u64, String>` | 当前阈值字节数 | 默认 20MiB，文件不存在时也返回默认值 |
| `set_image_threshold` | `set_image_threshold(bytes: u64) -> Result<(), String>` | Ok 或中文 Err | 合法范围 1MiB(1048576)..=500MiB(524288000)，越界返回 Err，不修改文件 |

单位：字节（u64）。前端 UI 建议以 MiB 为单位展示，提交时换算为字节传入。
