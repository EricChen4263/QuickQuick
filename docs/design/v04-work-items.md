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
- **状态**：ready
- **做什么**：`pipeline.rs` / `macos_paste.rs` 里 `is_concealed` 硬编码 `false`，改为真实读取 macOS `org.nspasteboard.ConcealedType` / `TransientType`（Windows 对应 `ExcludeClipboardContentFromMonitorProcessing`）。
- **验收**：从 1Password / Bitwarden 复制密码不入库；`skip_sensitive=false` 时仍可入库；新增单测覆盖判定。

### P1-2 · 来源应用排除接线
- **状态**：ready
- **做什么**：`source_app` 硬编码 `None`，改为真实取前台应用 bundle id，与 `excluded_apps` 名单匹配。
- **验收**：名单内 app 复制不入库；名单外正常入库；`excluded_apps` 为空时零开销；新增单测。

### P1-3 · 私有 UTI 自产标记
- **状态**：ready
- **做什么**：`has_self_marker` 恒 `false`，实现真正的私有标记写入（写回剪贴板时附加自定义 pasteboard type），读取时识别并跳过。
- **验收**：从历史粘贴/复制后不产生新条目、不打乱顺序；标记不影响其他 app 读取剪贴板；新增单测。

### P1-4 · `tiered_cleanup` 自动触发
- **状态**：ready
- **做什么**：`image.rs:413` 的 500MB 两级清理零调用，接上自动触发（捕获后或定期）。
- **验收**：超过阈值时自动先剥最旧原图保缩略图、再整条删非收藏；`is_favorite` 的图片确实豁免；新增单测。

### P1-5 · 可移植备份导出/导入接线
- **状态**：ready
- **做什么**：`portable.rs` 的 `export_portable` / `import_portable` 格式加密已完整但只被测试调用。加 IPC 命令 + 设置页「备份/恢复」入口。
- **验收**：能导出加密包并在干净环境导入；口令错误时明确报错不损坏现有库；导入前有确认；新增 IPC 层测试。

### P1-6 · 翻译缓存接线
- **状态**：ready
- **做什么**：`cache.rs` 的 `cache_get`/`cache_put` 零调用，接进 `translate_text_impl`。
- **验收**：同文本同语言对同 provider 二次翻译零网络请求；LRU 淘汰生效；缓存 key 四元组正确；新增单测。

### P1-7 · 翻译重试/取消/错误归一再接
- **状态**：ready
- **做什么**：`retry.rs` / `cancel.rs` / `error.rs` 三模块零生产调用，接入翻译主链路。
- **验收**：瞬时失败（超时/5xx）自动退避重试；连续请求时旧结果被丢弃不覆盖新结果；错误信息经 `map_provider_error` 归一；新增单测。

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
