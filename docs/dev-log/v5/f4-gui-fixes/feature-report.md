---
id: V5-F4-report
type: feature_report
level: 大功能
parent: V5
children: [f4-s01-tray-icon, f4-s02-clip-translate-jump, f4-s03-image-threshold, f4-s04-auto-paste, f4-s05-auto-update, f4-s07-close-to-tray, f4-s08-clip-autorefresh, f4-s09-unmount-race, f4-s10-event-const]
created: 2026-06-02T00:00:00Z
status: 已闭合（含 1 项外部阻塞）
author: orchestrator
---

# 大功能报告 · V5-F4 GUI 修复与遗留技术债收口

## 目标
收口里程碑4 GUI 验证暴露的 bug + REMAINING-TODO 第三节遗留技术债，把"半接真/占位"项做成真功能。

## 小功能闭合清单

| 小功能 | 内容 | 验证 | 锚点 commit |
|---|---|---|---|
| s01 托盘图标白圆 | 根因：tray.png 未 bundle 进 resource_dir→回退彩色应用图标却套 icon_as_template→实心白圆。改用 `include_bytes!` 编译期嵌入 + 回退不套 template | cargo check/test 全绿；GUI 待用户重启确认 | 4bd6ea1 |
| s02 一键翻译跳转 | 剪贴板预览「一键翻译」→ App translateSeed{text,nonce} → TranslatePage seed useEffect 注入并自动翻译；handleTranslate 重构接 textOverride 避 setState 异步陷阱 | tester 4 变异全红；reviewer 通过；340 测试 | 0d5ab46 |
| s03 单张图片阈值 | OversizePolicy.max_original_bytes 从硬编码改读用户配置；settings 字段+IPC get/set(校验 1MiB..=500MiB)+StoragePanel 预设档位 select 接真 | tester 5 变异全红；reviewer 通过(修注释)；前端 346 + cargo 全绿 | d60fd93/6f7ab78 |
| s04 真实自动粘贴 | macOS 生产后端(AXIsProcessTrusted/NSPasteboard.changeCount/CGEvent ⌘V) + system.rs 接 write_and_confirm(保 A15 changeCount)→hide→send_paste；trusted→full_paste，未授权/超时→write_back_only 降级 | tester 3 变异全红；reviewer 3 高危**已收口**；cargo build 链接+81 测试全绿 | de538d9/143ff71/001ccd8 |
| s05 自动更新 endpoint | **外部阻塞**：需真实更新服务器+minisign 私钥+发布 CI，仓库内无法完成。如实评估留痕，不伪造 | 见 s05-auto-update/assessment.md | —（不改代码）|
| s07 关闭按钮隐藏到后台 | 主窗只处理失焦无 CloseRequested→点关闭走默认销毁退出。新增 `CloseRequested{api,..}` 分支：stay_in_tray 时 prevent_close+hide；函数更名 setup_main_window_behavior | cargo test 168+ 全绿；启动冒烟无 panic；GUI 待用户实测 | 9d49cc6 |
| s08 剪贴板自动刷新 | 列表仅挂载时读一次、复制新内容不刷新（页面 display 切换不卸载）。后端写库非空 emit `clipboard-changed`（抽纯函数 should_notify_clip_change 作可测接缝），前端 listen→loadItems 重读 | tester 后端4+前端 命中+变异全红；reviewer 通过；cargo 301 + 前端 356 全绿 | fcfd997 |
| s09 删除/收藏卸载竞态 | handler 用局部 {current:false}（永不置 true）→卸载后 setState。改共享组件生命周期 cancelledRef（卸载 cleanup 置 true） | tester：正向流有判别力；**实证 React18 下该竞态不可黑盒证伪**；reviewer 通过；前端 358 全绿 | c208198 |
| s10 事件名常量 | 纯重构：clipboard-changed 字面量抽为前端 src/ipc/events.ts + 后端 lib.rs 各一处常量 CLIPBOARD_CHANGED_EVENT（消除同语言内重复，I-01），两端注释互指 | tester：前端偏离常量后测试变红、跨语言值一致；reviewer 通过；cargo 301 + 前端 358 全绿 | ee38d39 |

## 关键决策与亮点
- **托盘 bug 根因定位**：template image 只取 alpha 轮廓——彩色应用图标套 template 必成实心色块，故必须用真正的单色模板图（tray.png 本身正确，问题在加载路径）。`include_bytes!` 摆脱 resource_dir 依赖，dev/prod 一致。
- **一键翻译跨页**：主窗内部用 props 提升（translateSeed nonce 自增重触发），不滥用 tauri 事件；textOverride + typeof 守卫避开 React setState 异步陷阱。
- **图片阈值**：复用既有 OversizePolicy 语义（超阈值留缩略图、原图标记未存），只把阈值从常量变配置，最小侵入。
- **自动粘贴 A15 保证**：reviewer 抓出 macOS 生产路径绕过 changeCount 轮询（会粘旧内容）的高危——收口时抽出 `write_and_confirm`（写入+轮询确认，DRY 复用于 write_then_paste），macOS 路径确认写入后才 hide+send_paste，Timeout 则降级不盲发。这是"独立 reviewer 防自评自过"的真实兑现。
- **诚实边界**：自动粘贴的真实键盘注入无法 headless 单测（OS 注入/AX/窗口 hide 隔离在 fake 之外，仅测决策映射）；自动更新外部阻塞——均如实标注，不谎报。

## 待用户手动确认（manual，不阻塞代码门禁）
- **s01 托盘图标**：重启 app（`make dev`，改了 tray.rs 需重新构建 Rust）肉眼确认菜单栏显示环形双 Q 而非白圆。
- **s04 自动粘贴**：授予 QuickQuick「辅助功能(Accessibility)」权限后，GUI 实测 ⌘V 注入是否真把内容粘到目标 App（含焦点回归、100ms 延时是否够）。务实焦点方案未做 RecordFrontmost/ActivateOriginalApp，慢机/特殊 App 下可能需调延时或补完整焦点链。
- **s07 关闭到后台**：重启 app（改了 lib.rs 需重新构建），点主窗关闭按钮→应隐藏到托盘/Dock 继续运行而非退出；托盘「退出」菜单→才真正退出。
- **s08 剪贴板自动刷新**：重启 app，保持主窗开着复制一段新文本→列表应在约半秒内自动冒出新条目，无需手动操作。

## 仍遗留（外部阻塞 / 后续）
- s05 自动更新：待更新服务器 + 签名密钥 + 发布 CI 就绪后，做客户端 check() 接线（纯代码，届时可在本仓完成）。详见 assessment.md。
