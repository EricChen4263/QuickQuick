---
id: RT1-F2-S03-code
type: coding_record
level: 小功能
parent: RT1-F2
children: []
created: 2026-06-07T00:00:00Z
status: 通过
commit: PENDING
acceptance_ids: [RT1-F2-S03]
evidence:
  - src/panels/clipboard/rich-link.ts
  - src-tauri/src/lib.rs
  - src-tauri/capabilities/default.json
author: coder
---

# 编码记录 · RT1-F2-S03 富文本链接点击走外部浏览器（防 webview 劫持）

## 缘起
RT1-M01 真机验证暴露的真 bug：富文本预览渲染的 `<a href>` 被点击时，Tauri webview 直接导航到链接地址、把 app 顶掉退不回来。本小功能为派生修复（不回溯改 RT1 已 done 裁决）。

## 做了什么
点击富文本预览里的链接，preventDefault 阻止 webview 导航，改用系统默认浏览器打开（Tauri opener 插件）。

## 关键决策与理由
- **新增 tauri-plugin-opener**（项目原无开链接能力）：跨平台正解（本功能跨平台）；Rust `.plugin(tauri_plugin_opener::init())` + capability `opener:allow-open-url`（覆盖 main/clip-popover/trans-popover 三窗口）。
- **事件委托 + scheme 白名单**：`rich-link.ts::resolveRichLinkClick` 用 `closest('a')`（处理链接内子元素点击）+ `getAttribute('href')`（避免 jsdom base url 误判）+ 仅 http/https/mailto 白名单（file://、javascript:、data: 一律 null；javascript: 另有 DOMPurify 上游剥离，双重防御）。两处预览复用 `handleRichLinkClick`。
- **openExternalUrl 薄封装**：便于测试 mock，arboard 式隔离真实 GUI 行为。

## 改动文件
- `src-tauri/Cargo.toml`/`Cargo.lock`、`src-tauri/src/lib.rs` — 接入 opener 插件
- `src-tauri/capabilities/default.json` — `opener:allow-open-url`
- `package.json`/`pnpm-lock.yaml` — `@tauri-apps/plugin-opener`
- `src/panels/translate/browser-api.ts` — `openExternalUrl(url)`
- `src/panels/clipboard/rich-link.ts`（新）— 委托逻辑 + 纯函数
- `src/panels/clipboard/ClipPreview.tsx`、`src/clip-popover/PopoverPreview.tsx` — 容器 onClick 接入
- 测试：rich-link.test.ts、rich-link-click.test.tsx

## 自测结论（TDD 红-绿-重构）
- 先写 resolveRichLinkClick 纯函数 + 点击委托测试（RED），实现后 GREEN。
- `pnpm test` 482 passed；`cargo test` 531 passed/0 failed（加插件后能编译、boot_smoke 守卫过）；`tsc` 0 错；`clippy -D warnings` exit 0。
- 审查 I-1 补强：追加 javascript:/data: 显式断言。
- capability ACL 名 `opener:allow-open-url` 经 acl-manifests.json 核实合法；真机开浏览器归 RT1-M01 manual_confirm。
