---
status: done
category: research
subject: 用 REA 逆向 Maccy 2.6.1 并产出 QuickQuick 对标方案
summary: 通过 REA 发布态取证 + Maccy 2.6.1 源码对照 + QuickQuick 全域只读审计，产出六阶段对标路线图；推翻三个初始误判（Maccy 有粘贴栈、持久化是 Core Data、非激活面板在 Tauri 不可行）。
evidence:
  commits: []
  commands:
    - "rea inspect_macho / inspect_plist / inspect_signature（Maccy.app）"
    - "rea doctor --json（Ghidra 12.1.4 识别，native decompiler 缺 arm64 二进制）"
    - "git clone --depth 1 ayangweb/EcoPaste（参考实现取证）"
  urls:
    - "https://github.com/ayangweb/EcoPaste"
    - "https://crates.io/crates/tauri-nspanel"
---

# AI 活动记录 · Maccy 逆向与对标方案

## 摘要

目标是用 REA 分析 Maccy 并优化 QuickQuick，用户后续把范围明确为「全面看齐 Maccy，交互是最痛环节」。

做了三件事：① REA 对 Maccy 2.6.1 发布态取证（依赖、资源、entitlements、字符串、Mach-O 结构）；② Maccy 2.6.1 源码对照，得出完整能力矩阵与设计决策；③ QuickQuick 全域只读审计（剪贴板域深度 + 其余六域广度）。

产出 `docs/design/maccy-parity-roadmap.md`（六阶段、约 10–13 周、分两版交付）。

**推翻的三个初始误判**：

1. **Maccy 有粘贴栈** → 实际没有（全量 76 个 Swift 文件无 queue/multi-select）。它用字母 pin（`b`–`y`，20 键位持久化）+ 数字直达（前 10 条）替代。所以「粘贴栈」是超越项而非看齐项。
2. **Maccy 用 Core Data** → 实际是 SwiftData（`.xcdatamodeld` 在 pbxproj 引用次数 0、`import CoreData` 0 次、bundle 内无 `.momd`）。
3. **非激活面板在 Tauri 不可行（单点成败风险）** → 已实证可行：`tauri-nspanel`（crates.io 2.1.1）+ EcoPaste（Tauri v2 剪贴板管理器）同构落地，且解决了「面板 key 状态吞掉合成 ⌘V」的坑。风险降级为中。

**关键发现（审计侧）**：QuickQuick 有 7 处「UI 已承诺、代码未接线」——隐私三件套（`is_concealed`/`source_app` 硬编码、`has_self_marker` 恒 false）、翻译缓存/重试/取消/错误归一（零生产调用）、`portable.rs` 可移植备份（只被测试调用）、`tiered_cleanup`（零调用，但存储页已承诺）。判定这类问题优先级高于新功能。

**反编译器**：装了 OpenJDK 21 + Ghidra 12.1.4，REA 识别通过，但官方包不含 macOS arm64 的 native decompiler，本地编译因 Makefile 硬编码 x86_64 与 BFD include 未透传失败两次。判定为非关键路径（Maccy 开源、二进制已 strip、REA Ghidra provider 对 universal Mach-O 需先 thin slice），保留 Ghidra 待未来验证闭源目标。

## 证据

- 方案文档：`docs/design/maccy-parity-roadmap.md`
- Maccy 发布态：`/Applications/Maccy.app`，2.6.1 build 60，sha256 `0426e15d…`，依赖仅 Sparkle 2.6.4，31 lproj，`LSUIElement=true`
- 参考实现：`ayangweb/EcoPaste` `src-tauri/src/window/macos.rs`（panel 生命周期）、`window/lifecycle/mod.rs:650`（`to_panel` 逆操作）
- QuickQuick 关键位置：`src-tauri/src/pipeline.rs:187`（隐私硬编码）、`ipc/translate.rs:226-279`（缓存未接线）、`image.rs:413`（`tiered_cleanup` 死代码）、`db.rs:745-812`（零索引）、`ClipboardPage.tsx:156-159`（主窗 Enter 无动作）
